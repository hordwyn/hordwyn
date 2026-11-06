//! Library entries, progress events, reviews.

use app::ports::{LibraryRepo, ProgressRepo, RepoError, ReviewRepo};
use async_trait::async_trait;
use domain::hlc::Lww;
use domain::ids::{EntryId, EventId, ItemId, ReviewId, UserId};
use domain::kind::{KindId, Status};
use domain::library::{LibraryEntry, Rating};
use domain::progress::ProgressEvent;
use domain::review::Review;

use crate::conv::{corrupt, hlc, json, to_json};
use crate::tx::{Op, record};
use crate::{PgStore, Res, be};

struct EntryRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    item_id: uuid::Uuid,
    kind: String,
    status: String,
    status_hlc: String,
    rating: Option<i16>,
    rating_hlc: String,
    notes: Option<String>,
    notes_hlc: String,
    added_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<EntryRow> for LibraryEntry {
    type Error = RepoError;
    fn try_from(r: EntryRow) -> Result<Self, Self::Error> {
        let rating = r
            .rating
            .map(|v| {
                u8::try_from(v)
                    .ok()
                    .and_then(|v| Rating::new(v).ok())
                    .ok_or_else(|| corrupt("rating", &v.to_string()))
            })
            .transpose()?;
        Ok(LibraryEntry {
            id: EntryId(r.id),
            user_id: UserId(r.user_id),
            item_id: ItemId(r.item_id),
            kind: KindId::new(&r.kind).map_err(|_| corrupt("kind", &r.kind))?,
            status: Lww::new(Status::new(r.status), hlc(&r.status_hlc)?),
            rating: Lww::new(rating, hlc(&r.rating_hlc)?),
            notes: Lww::new(r.notes, hlc(&r.notes_hlc)?),
            added_at: r.added_at,
        })
    }
}

#[async_trait]
impl LibraryRepo for PgStore {
    async fn get(&self, user: UserId, item: ItemId) -> Res<Option<LibraryEntry>> {
        sqlx::query_as!(
            EntryRow,
            "SELECT * FROM library_entries WHERE user_id = $1 AND item_id = $2",
            user.0,
            item.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(LibraryEntry::try_from)
        .transpose()
    }

    async fn get_by_id(&self, id: EntryId) -> Res<Option<LibraryEntry>> {
        sqlx::query_as!(
            EntryRow,
            "SELECT * FROM library_entries WHERE id = $1",
            id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(LibraryEntry::try_from)
        .transpose()
    }

    async fn upsert(&self, e: &LibraryEntry) -> Res<()> {
        let mut tx = self.begin().await?;
        let id = sqlx::query_scalar!(
            "INSERT INTO library_entries (id, user_id, item_id, kind, status, status_hlc, rating,
                                          rating_hlc, notes, notes_hlc, added_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (user_id, item_id) DO UPDATE SET status = EXCLUDED.status,
                 status_hlc = EXCLUDED.status_hlc, rating = EXCLUDED.rating,
                 rating_hlc = EXCLUDED.rating_hlc, notes = EXCLUDED.notes,
                 notes_hlc = EXCLUDED.notes_hlc
             RETURNING id",
            e.id.0,
            e.user_id.0,
            e.item_id.0,
            e.kind.as_str(),
            e.status.value.as_str(),
            e.status.stamp.to_string(),
            e.rating.value.map(|r| i16::from(r.value())),
            e.rating.stamp.to_string(),
            e.notes.value,
            e.notes.stamp.to_string(),
            e.added_at
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(be)?;
        record(&mut tx, "library_entry", id, Op::Upsert, &[e.user_id]).await?;
        tx.commit().await.map_err(be)
    }

    async fn any_for_item(&self, item: ItemId) -> Res<bool> {
        let found = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM library_entries WHERE item_id = $1) AS "e!""#,
            item.0
        )
        .fetch_one(&self.pool)
        .await
        .map_err(be)?;
        Ok(found)
    }

    async fn by_user(&self, user: UserId) -> Res<Vec<LibraryEntry>> {
        sqlx::query_as!(
            EntryRow,
            "SELECT * FROM library_entries WHERE user_id = $1 ORDER BY added_at, id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(LibraryEntry::try_from)
        .collect()
    }

    async fn page(&self, user: UserId, offset: u64, limit: u32) -> Res<(Vec<LibraryEntry>, u64)> {
        let rows = sqlx::query_as!(
            EntryRow,
            "SELECT * FROM library_entries WHERE user_id = $1
             ORDER BY added_at DESC, id DESC OFFSET $2 LIMIT $3",
            user.0,
            i64::try_from(offset).unwrap_or(i64::MAX),
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!" FROM library_entries WHERE user_id = $1"#,
            user.0
        )
        .fetch_one(&self.pool)
        .await
        .map_err(be)?;
        Ok((
            rows.into_iter()
                .map(LibraryEntry::try_from)
                .collect::<Res<_>>()?,
            u64::try_from(total).unwrap_or(0),
        ))
    }
}

struct EventRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    item_id: uuid::Uuid,
    occurred_at: chrono::DateTime<chrono::Utc>,
    progress: serde_json::Value,
    client_op_id: Option<uuid::Uuid>,
}

impl TryFrom<EventRow> for ProgressEvent {
    type Error = RepoError;
    fn try_from(r: EventRow) -> Result<Self, Self::Error> {
        Ok(ProgressEvent {
            id: EventId(r.id),
            user_id: UserId(r.user_id),
            item_id: ItemId(r.item_id),
            occurred_at: r.occurred_at,
            progress: json("progress", r.progress)?,
            client_op_id: r.client_op_id,
        })
    }
}

#[async_trait]
impl ProgressRepo for PgStore {
    async fn append(&self, e: &ProgressEvent) -> Res<ProgressEvent> {
        let mut tx = self.begin().await?;
        let inserted = sqlx::query_scalar!(
            "INSERT INTO progress_events (id, user_id, item_id, occurred_at, progress, client_op_id)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id, client_op_id) DO NOTHING
             RETURNING id",
            e.id.0,
            e.user_id.0,
            e.item_id.0,
            e.occurred_at,
            to_json(&e.progress)?,
            e.client_op_id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(be)?;
        if inserted.is_none() {
            let existing = sqlx::query_as!(
                EventRow,
                "SELECT id, user_id, item_id, occurred_at, progress, client_op_id
                 FROM progress_events WHERE user_id = $1 AND client_op_id = $2",
                e.user_id.0,
                e.client_op_id
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(be)?;
            tx.rollback().await.map_err(be)?;
            return existing.try_into();
        }
        record(&mut tx, "progress_event", e.id.0, Op::Upsert, &[e.user_id]).await?;
        tx.commit().await.map_err(be)?;
        Ok(e.clone())
    }

    async fn for_item(&self, user: UserId, item: ItemId) -> Res<Vec<ProgressEvent>> {
        sqlx::query_as!(
            EventRow,
            "SELECT id, user_id, item_id, occurred_at, progress, client_op_id
             FROM progress_events WHERE user_id = $1 AND item_id = $2 ORDER BY occurred_at, id",
            user.0,
            item.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(ProgressEvent::try_from)
        .collect()
    }
}

struct ReviewRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    item_id: uuid::Uuid,
    body: String,
    contains_spoilers: bool,
    visibility: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<ReviewRow> for Review {
    type Error = RepoError;
    fn try_from(r: ReviewRow) -> Result<Self, Self::Error> {
        Ok(Review {
            id: ReviewId(r.id),
            user_id: UserId(r.user_id),
            item_id: ItemId(r.item_id),
            body: r.body,
            contains_spoilers: r.contains_spoilers,
            visibility: json("visibility", r.visibility)?,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
    }
}

#[async_trait]
impl ReviewRepo for PgStore {
    async fn upsert(&self, r: &Review) -> Res<()> {
        let mut tx = self.begin().await?;
        let id = sqlx::query_scalar!(
            "INSERT INTO reviews (id, user_id, item_id, body, contains_spoilers, visibility, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (user_id, item_id) DO UPDATE SET body = EXCLUDED.body,
                 contains_spoilers = EXCLUDED.contains_spoilers, visibility = EXCLUDED.visibility,
                 updated_at = EXCLUDED.updated_at
             RETURNING id",
            r.id.0,
            r.user_id.0,
            r.item_id.0,
            r.body,
            r.contains_spoilers,
            to_json(&r.visibility)?,
            r.created_at,
            r.updated_at
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(be)?;
        record(&mut tx, "review", id, Op::Upsert, &[r.user_id]).await?;
        tx.commit().await.map_err(be)
    }

    async fn get(&self, user: UserId, item: ItemId) -> Res<Option<Review>> {
        sqlx::query_as!(
            ReviewRow,
            "SELECT * FROM reviews WHERE user_id = $1 AND item_id = $2",
            user.0,
            item.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(Review::try_from)
        .transpose()
    }

    async fn delete(&self, id: ReviewId) -> Res<()> {
        let mut tx = self.begin().await?;
        let user = sqlx::query_scalar!("DELETE FROM reviews WHERE id = $1 RETURNING user_id", id.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(be)?;
        let users: Vec<UserId> = user.map(UserId).into_iter().collect();
        record(&mut tx, "review", id.0, Op::Delete, &users).await?;
        tx.commit().await.map_err(be)
    }

    async fn for_item(&self, item: ItemId) -> Res<Vec<Review>> {
        sqlx::query_as!(
            ReviewRow,
            "SELECT * FROM reviews WHERE item_id = $1 ORDER BY created_at",
            item.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(Review::try_from)
        .collect()
    }
}
