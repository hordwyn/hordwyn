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
use crate::run_rules::Source;
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
    /// and moves the status as [`domain::rewatch::status_after`] says (Planned to In progress,
    /// `finished` to Completed, a rewatch start from Completed to In progress, the last episode
    /// of a rewatch run back to Completed) unless the user already set a later status. The
    /// status move is stored on the event, so undo can take it back.
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

    /// [`App::log_progress`] for an import replaying a file: its views are dated in the past
    /// on purpose, so one dated before the start of a running rewatch run is stored (and not
    /// counted in the run) instead of refused (DEC-587 (7), DEC-591).
    ///
    /// # Errors
    /// As [`App::log_progress`].
    pub async fn import_progress(
        &self,
        actor: Actor,
        item_id: ItemId,
        progress: Progress,
        occurred_at: Option<Timestamp>,
        client_op_id: Option<uuid::Uuid>,
    ) -> AppResult<ProgressEvent> {
        let log = (progress, occurred_at, client_op_id);
        self.log_from(actor, item_id, log, None, Source::Import)
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
        let log = (progress, occurred_at, client_op_id);
        self.log_from(actor, item_id, log, status_hlc, Source::Live)
            .await
    }

    /// Would [`App::log_progress`] take this log as far as a rewatch run is concerned
    /// ([`crate::run_rules`])? Nothing is written. For a caller that writes something else
    /// first (the session of "watched with").
    ///
    /// # Errors
    /// What the log would answer: `Conflict` or `invalid` (`occurred_at`).
    pub async fn check_progress(
        &self,
        actor: Actor,
        item_id: ItemId,
        progress: &Progress,
        occurred_at: Option<Timestamp>,
        client_op_id: Option<uuid::Uuid>,
    ) -> AppResult<()> {
        let item = self.catalog_item(item_id).await?;
        let Some(entry) = self.repos.library.get(actor.user_id, item_id).await? else {
            return crate::run_rules::without_entry(progress);
        };
        let history = self.repos.progress.for_item(actor.user_id, item_id).await?;
        if client_op_id.is_some() && history.iter().any(|e| e.client_op_id == client_op_id) {
            return Ok(());
        }
        let status = &entry.status.value;
        let now = self.now();
        crate::run_rules::occurred_at(
            &item,
            status,
            &history,
            progress,
            occurred_at,
            now,
            Source::Live,
        )
        .map(|_| ())
    }

    /// The log behind [`App::log_progress_stamped`] and [`App::import_progress`]. The rules
    /// of a rewatch run ([`crate::run_rules`]) are checked before anything is written; a
    /// replay of a stored log answers that log whatever they say now.
    async fn log_from(
        &self,
        actor: Actor,
        item_id: ItemId,
        (progress, sent, client_op_id): (Progress, Option<Timestamp>, Option<uuid::Uuid>),
        status_hlc: Option<Hlc>,
        source: Source,
    ) -> AppResult<ProgressEvent> {
        let item = self.catalog_item(item_id).await?;
        let kind = self.kind(&item.kind)?;
        progress.validate_for(kind.progress_model)?;
        let user = actor.user_id;
        let replayed = |history: &[ProgressEvent]| {
            let op = client_op_id?;
            history.iter().find(|e| e.client_op_id == Some(op)).cloned()
        };
        // the history is read only where a run can matter: for a marker, and for an episode
        // on an In progress entry
        let mut history = Vec::new();
        let marker = matches!(progress, Progress::Rewatch | Progress::RewatchStop);
        if marker {
            history = self.repos.progress.for_item(user, item_id).await?;
            if let Some(done) = replayed(&history) {
                return Ok(done);
            }
            // a marker never adds the item: there is no run without an entry
            if self.repos.library.get(user, item_id).await?.is_none() {
                crate::run_rules::without_entry(&progress)?;
            }
        }
        let mut entry = self.add_to_library(actor, item_id, None).await?;
        let in_progress = entry.status.value.as_str() == Status::IN_PROGRESS;
        if in_progress && matches!(progress, Progress::Episode { .. }) {
            history = self.repos.progress.for_item(user, item_id).await?;
            if let Some(done) = replayed(&history) {
                return Ok(done);
            }
        }
        let now = self.now();
        let status = &entry.status.value;
        let occurred_at =
            crate::run_rules::occurred_at(&item, status, &history, &progress, sent, now, source)?;
        let mut event = ProgressEvent {
            id: EventId(self.uuid()),
            user_id: user,
            item_id,
            occurred_at,
            progress,
            client_op_id,
            recorded_at: now,
            status_change: None,
        };
        // the episode that makes a running rewatch run whole gives Completed back (DEC-587),
        // and so does the stop marker of an open run (it was refused above without one)
        let closes = match event.progress {
            Progress::RewatchStop => true,
            Progress::Episode { .. } => {
                in_progress && crate::next_step::completes_run(&item, &history, &event)
            }
            _ => false,
        };
        event.status_change =
            domain::rewatch::status_after(&entry.status.value, &event.progress, closes)
                .filter(|to| entry.status.value != *to)
                .map(|to| StatusChange {
                    from: entry.status.value.clone(),
                    to,
                    stamp: self.stamp(status_hlc),
                    from_stamp: Some(entry.status.stamp),
                });
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
        // the move back is stored on the undo event, like every move (DEC-588)
        let revert = self.status_to_restore(actor, &target, &history).await?;
        let undo = ProgressEvent {
            id: EventId(self.uuid()),
            user_id: actor.user_id,
            item_id: target.item_id,
            occurred_at: now,
            progress: Progress::Undo { event_id: id },
            client_op_id: Some(uuid::Uuid::new_v5(&id.0, b"undo")),
            recorded_at: now,
            status_change: revert.as_ref().map(|(_, c)| c.clone()),
        };
        let stored = self.repos.progress.append(&undo).await?;
        if stored.id != undo.id {
            return Ok(stored);
        }
        if let Some((mut entry, c)) = revert
            && entry.status.set(c.to.clone(), c.stamp)
        {
            self.save_entry(&entry).await?;
        }
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

    /// The move that takes back the status move of `target`, with the entry to write it to:
    /// only while the entry still carries that write (DEC-208), also when an undo of a later
    /// event restored it meanwhile ([`StatusChange::carried`]; the Undo of a whole rewatch
    /// run takes its last episode back first, then its start). A status chosen by hand since
    /// stays.
    async fn status_to_restore(
        &self,
        actor: Actor,
        target: &ProgressEvent,
        history: &[ProgressEvent],
    ) -> AppResult<Option<(LibraryEntry, StatusChange)>> {
        let Some(c) = &target.status_change else {
            return Ok(None);
        };
        let entry = self
            .repos
            .library
            .get(actor.user_id, target.item_id)
            .await?
            .filter(|e| e.status.value == c.to)
            .filter(|e| StatusChange::carried(e.status.stamp, c.stamp, history));
        Ok(entry.map(|e| {
            let back = StatusChange {
                from: c.to.clone(),
                to: c.from.clone(),
                stamp: self.stamp(None),
                from_stamp: Some(e.status.stamp),
            };
            (e, back)
        }))
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
