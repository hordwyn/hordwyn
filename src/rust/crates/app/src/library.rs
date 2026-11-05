use domain::Timestamp;
use domain::hlc::{Hlc, Lww};
use domain::ids::{EntryId, EventId, ItemId, ReviewId};
use domain::kind::Status;
use domain::library::{LibraryEntry, Rating};
use domain::list::Visibility;
use domain::progress::{Progress, ProgressEvent};
use domain::review::{Review, validate_review_body};
use domain::user::Actor;

use crate::ports::AppEvent;
use crate::{App, AppResult};

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
        let stamp = self.stamp(None);
        let entry = LibraryEntry {
            id: EntryId(self.uuid()),
            user_id: actor.user_id,
            item_id,
            kind: item.kind,
            status: Lww::new(status.unwrap_or_else(Status::planned), stamp),
            rating: Lww::new(None, stamp),
            notes: Lww::new(None, stamp),
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
    /// already set a later status.
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
        let item = self.catalog_item(item_id).await?;
        let kind = self.kind(&item.kind)?;
        progress.validate_for(kind.progress_model)?;
        let event = ProgressEvent {
            id: EventId(self.uuid()),
            user_id: actor.user_id,
            item_id,
            occurred_at: occurred_at.unwrap_or_else(|| self.now()),
            progress,
            client_op_id,
        };
        let stored = self.repos.progress.append(&event).await?;
        if stored.id != event.id {
            return Ok(stored);
        }
        let mut entry = self.add_to_library(actor, item_id, None).await?;
        let target = match stored.progress {
            Progress::Finished => Some(Status::completed()),
            _ if entry.status.value.as_str() == Status::PLANNED => Some(Status::in_progress()),
            _ => None,
        };
        if let Some(t) = target
            && entry.status.value != t
            && entry.status.set(t, self.stamp(None))
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

    /// # Errors
    /// Storage errors.
    pub async fn progress(&self, actor: Actor, item_id: ItemId) -> AppResult<Vec<ProgressEvent>> {
        Ok(self.repos.progress.for_item(actor.user_id, item_id).await?)
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
