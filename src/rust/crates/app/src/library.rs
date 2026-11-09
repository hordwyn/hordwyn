use domain::DomainError;
use domain::Timestamp;
use domain::hlc::{Hlc, Lww};
use domain::ids::{EntryId, EventId, ItemId, ReviewId};
use domain::kind::Status;
use domain::library::{LibraryEntry, Rating};
use domain::list::Visibility;
use domain::progress::{Progress, ProgressEvent, StatusChange, effective};
use domain::review::{Review, validate_review_body};
use domain::user::Actor;

use crate::error::OptionExt;
use crate::ports::{AppEvent, LibraryCursor, LibrarySort, SortValue};
use crate::{App, AppError, AppResult};

/// How long after logging an event it can still be undone (hours).
pub const UNDO_WINDOW_HOURS: i64 = 24;

impl App {
    /// Add an item to the actor's library. Returns the existing entry if already present.
    ///
    /// # Errors
    /// Unknown item or a status the kind does not allow.
    pub async fn add_to_library(
        &self,
        actor: Actor,
        item_id: ItemId,
        status: Option<Status>,
    ) -> AppResult<LibraryEntry> {
        let item = self.catalog_item(item_id).await?;
        let kind = self.kind(&item.kind)?;
        if let Some(s) = &status {
            kind.check_status(s)?;
        }
        if let Some(entry) = self.repos.library.get(actor.user_id, item_id).await? {
            return Ok(entry);
        }
        // Defaults carry the smallest stamp, so any real write (also an older offline edit
        // synced later) wins over "not set yet" (DEC-128).
        let unset = Hlc::new(0, 0, 0);
        let status_stamp = if status.is_some() {
            self.stamp(None)
        } else {
            unset
        };
        let entry = LibraryEntry {
            id: EntryId(self.uuid()),
            user_id: actor.user_id,
            item_id,
            kind: item.kind,
            status: Lww::new(status.unwrap_or_else(Status::planned), status_stamp),
            rating: Lww::new(None, unset),
            notes: Lww::new(None, unset),
            added_at: self.now(),
        };
        self.save_entry(&entry).await?;
        Ok(entry)
    }

    /// # Errors
    /// Storage errors.
    pub async fn entry(&self, actor: Actor, item_id: ItemId) -> AppResult<Option<LibraryEntry>> {
        Ok(self.repos.library.get(actor.user_id, item_id).await?)
    }

    /// # Errors
    /// Storage errors.
    pub async fn library(&self, actor: Actor) -> AppResult<Vec<LibraryEntry>> {
        Ok(self.repos.library.by_user(actor.user_id).await?)
    }

    /// Largest library page.
    pub const MAX_PAGE: u32 = 500;

    /// One page of the library, most recently added first, with the total count and the
    /// cursor of the next page when more follow (keyset paging, DEC-416). `after` is a
    /// previous page's cursor; `limit` is clamped to 1..=[`App::MAX_PAGE`].
    ///
    /// # Errors
    /// A cursor of another order; storage errors.
    pub async fn library_page(
        &self,
        actor: Actor,
        after: Option<&LibraryCursor>,
        limit: u32,
    ) -> AppResult<(Vec<LibraryEntry>, u64, Option<LibraryCursor>)> {
        let after = match after {
            None => None,
            Some(LibraryCursor {
                sort: LibrarySort::Added,
                descending: true,
                key: SortValue::Time(at),
                id,
            }) => Some((*at, *id)),
            Some(_) => {
                return Err(
                    DomainError::invalid("after", "the cursor belongs to another order").into(),
                );
            }
        };
        let page = self
            .repos
            .library
            .page(actor.user_id, after, limit.clamp(1, Self::MAX_PAGE))
            .await?;
        let next = page
            .entries
            .last()
            .filter(|_| page.more)
            .map(|e| LibraryCursor {
                sort: LibrarySort::Added,
                descending: true,
                key: SortValue::Time(e.added_at),
                id: e.id,
            });
        Ok((page.entries, page.total, next))
    }

    /// Set the status (LWW). Adds the item to the library if needed.
    ///
    /// # Errors
    /// Unknown item or disallowed status.
    pub async fn set_status(
        &self,
        actor: Actor,
        item_id: ItemId,
        status: Status,
        stamp: Option<Hlc>,
    ) -> AppResult<LibraryEntry> {
        let mut entry = self.add_to_library(actor, item_id, None).await?;
        self.kind(&entry.kind)?.check_status(&status)?;
        if entry.status.set(status, self.stamp(stamp)) {
            self.save_entry(&entry).await?;
        }
        Ok(entry)
    }

    /// Set or clear the rating (LWW).
    ///
    /// # Errors
    /// Unknown item.
    pub async fn rate(
        &self,
        actor: Actor,
        item_id: ItemId,
        rating: Option<Rating>,
        stamp: Option<Hlc>,
    ) -> AppResult<LibraryEntry> {
        let mut entry = self.add_to_library(actor, item_id, None).await?;
        if entry.rating.set(rating, self.stamp(stamp)) {
            self.save_entry(&entry).await?;
        }
        Ok(entry)
    }

    /// Set or clear private notes (LWW).
    ///
    /// # Errors
    /// Unknown item.
    pub async fn set_notes(
        &self,
        actor: Actor,
        item_id: ItemId,
        notes: Option<String>,
        stamp: Option<Hlc>,
    ) -> AppResult<LibraryEntry> {
        let mut entry = self.add_to_library(actor, item_id, None).await?;
        if entry.notes.set(notes, self.stamp(stamp)) {
            self.save_entry(&entry).await?;
        }
        Ok(entry)
    }

    async fn save_entry(&self, entry: &LibraryEntry) -> AppResult<()> {
        self.repos.library.upsert(entry).await?;
        self.publish(AppEvent::entity_updated(
            "library_entry",
            entry.id.0,
            vec![entry.user_id],
        ))
        .await;
        Ok(())
    }

    /// Append a progress event. Idempotent per `client_op_id`. Adds the item to the library
    /// and moves the status to `in_progress` (or `completed` for `Finished`) unless the user
    /// already set a later status. The status move is stored on the event, so undo can
    /// take it back.
    ///
    /// # Errors
    /// Unknown item or progress that does not fit the kind's progress model.
    pub async fn log_progress(
        &self,
        actor: Actor,
        item_id: ItemId,
        progress: Progress,
        occurred_at: Option<Timestamp>,
        client_op_id: Option<uuid::Uuid>,
    ) -> AppResult<ProgressEvent> {
        self.log_progress_stamped(actor, item_id, progress, occurred_at, client_op_id, None)
            .await
    }

    /// [`App::log_progress`] with the client's stamp for the status the log implies (an
    /// offline log synced later counts as made when it was queued, so a status set after it,
    /// offline or on another device, wins; DEC-455). `None` stamps it now.
    ///
    /// # Errors
    /// As [`App::log_progress`].
    pub async fn log_progress_stamped(
        &self,
        actor: Actor,
        item_id: ItemId,
        progress: Progress,
        occurred_at: Option<Timestamp>,
        client_op_id: Option<uuid::Uuid>,
        status_hlc: Option<Hlc>,
    ) -> AppResult<ProgressEvent> {
        let item = self.catalog_item(item_id).await?;
        let kind = self.kind(&item.kind)?;
        progress.validate_for(kind.progress_model)?;
        let mut entry = self.add_to_library(actor, item_id, None).await?;
        let target = match progress {
            Progress::Finished => Some(Status::completed()),
            _ if entry.status.value.as_str() == Status::PLANNED => Some(Status::in_progress()),
            _ => None,
        };
        let status_change = target
            .filter(|t| entry.status.value != *t)
            .map(|to| StatusChange {
                from: entry.status.value.clone(),
                to,
                stamp: self.stamp(status_hlc),
            });
        let now = self.now();
        let event = ProgressEvent {
            id: EventId(self.uuid()),
            user_id: actor.user_id,
            item_id,
            occurred_at: occurred_at.unwrap_or(now),
            progress,
            client_op_id,
            recorded_at: now,
            status_change,
        };
        let stored = self.repos.progress.append(&event).await?;
        if stored.id != event.id {
            return Ok(stored);
        }
        if let Some(c) = &stored.status_change
            && entry.status.set(c.to.clone(), c.stamp)
        {
            self.save_entry(&entry).await?;
        }
        self.publish(AppEvent::entity_updated(
            "progress_event",
            stored.id.0,
            vec![actor.user_id],
        ))
        .await;
        Ok(stored)
    }

    /// Take back one of the actor's recent progress events by appending a compensating
    /// [`Progress::Undo`] event (append-only, D-12). The status move the event caused is
    /// reverted while the entry still carries it; a logged play (`Session`) is removed with
    /// its session. Undoing twice returns the first compensating event.
    ///
    /// # Errors
    /// `NotFound` for someone else's or an unknown event; `Conflict` for an undo event or
    /// one recorded more than [`UNDO_WINDOW_HOURS`] ago.
    pub async fn undo_progress(&self, actor: Actor, id: EventId) -> AppResult<ProgressEvent> {
        let target = self
            .repos
            .progress
            .get(id)
            .await?
            .filter(|e| e.user_id == actor.user_id)
            .found("progress event")?;
        if matches!(target.progress, Progress::Undo { .. }) {
            return Err(AppError::Conflict("an undo cannot be undone".into()));
        }
        let history = self
            .repos
            .progress
            .for_item(actor.user_id, target.item_id)
            .await?;
        if let Some(done) = history
            .iter()
            .find(|e| matches!(e.progress, Progress::Undo { event_id } if event_id == id))
        {
            return Ok(done.clone());
        }
        let now = self.now();
        if now - target.recorded_at > chrono::Duration::hours(UNDO_WINDOW_HOURS) {
            return Err(AppError::Conflict("the event is too old to undo".into()));
        }
        let undo = ProgressEvent {
            id: EventId(self.uuid()),
            user_id: actor.user_id,
            item_id: target.item_id,
            occurred_at: now,
            progress: Progress::Undo { event_id: id },
            client_op_id: Some(uuid::Uuid::new_v5(&id.0, b"undo")),
            recorded_at: now,
            status_change: None,
        };
        let stored = self.repos.progress.append(&undo).await?;
        if stored.id != undo.id {
            return Ok(stored);
        }
        self.revert_status(actor, &target).await?;
        if let Progress::Session { session_id } = target.progress
            && let Some(s) = self.repos.sessions.get(session_id).await?
            && s.owner == actor.user_id
        {
            self.repos.sessions.delete(session_id).await?;
            self.publish_session(&s).await?;
        }
        self.publish(AppEvent::entity_updated(
            "progress_event",
            stored.id.0,
            vec![actor.user_id],
        ))
        .await;
        Ok(stored)
    }

    async fn revert_status(&self, actor: Actor, target: &ProgressEvent) -> AppResult<()> {
        let Some(c) = &target.status_change else {
            return Ok(());
        };
        if let Some(mut entry) = self
            .repos
            .library
            .get(actor.user_id, target.item_id)
            .await?
            && entry.status.value == c.to
            && entry.status.stamp == c.stamp
            && entry.status.set(c.from.clone(), self.stamp(None))
        {
            self.save_entry(&entry).await?;
        }
        Ok(())
    }

    /// Every stored event of the item, undo events included, oldest first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn progress(&self, actor: Actor, item_id: ItemId) -> AppResult<Vec<ProgressEvent>> {
        Ok(self.repos.progress.for_item(actor.user_id, item_id).await?)
    }

    /// The item's effective history (undone events and undo events left out), oldest first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn history(&self, actor: Actor, item_id: ItemId) -> AppResult<Vec<ProgressEvent>> {
        let all = self.progress(actor, item_id).await?;
        Ok(effective(&all).into_iter().cloned().collect())
    }

    /// Write or replace the actor's review of an item.
    ///
    /// # Errors
    /// Unknown item or invalid body.
    pub async fn write_review(
        &self,
        actor: Actor,
        item_id: ItemId,
        body: &str,
        contains_spoilers: bool,
        visibility: Option<Visibility>,
    ) -> AppResult<Review> {
        validate_review_body(body)?;
        self.catalog_item(item_id).await?;
        let now = self.now();
        let review = match self.repos.reviews.get(actor.user_id, item_id).await? {
            Some(mut r) => {
                r.body = body.to_string();
                r.contains_spoilers = contains_spoilers;
                if let Some(v) = visibility {
                    r.visibility = v;
                }
                r.updated_at = now;
                r
            }
            None => Review {
                id: ReviewId(self.uuid()),
                user_id: actor.user_id,
                item_id,
                body: body.to_string(),
                contains_spoilers,
                visibility: visibility.unwrap_or(Visibility::Instance),
                created_at: now,
                updated_at: now,
            },
        };
        self.repos.reviews.upsert(&review).await?;
        self.publish(AppEvent::entity_updated(
            "review",
            review.id.0,
            vec![actor.user_id],
        ))
        .await;
        Ok(review)
    }

    /// # Errors
    /// Storage errors.
    pub async fn delete_review(&self, actor: Actor, item_id: ItemId) -> AppResult<()> {
        if let Some(r) = self.repos.reviews.get(actor.user_id, item_id).await? {
            self.repos.reviews.delete(r.id).await?;
        }
        Ok(())
    }
}
