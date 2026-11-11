//! Library entries, progress events, reviews.

use app::ports::{
    EntryPage, HistorySpans, HistoryWindow, HitPage, LibraryFilter, LibraryRepo, ProgressRepo,
    RepoError, ReviewRepo,
};
use async_trait::async_trait;
use domain::Timestamp;
use domain::hlc::Lww;
use domain::ids::{EntryId, EventId, ItemId, ReviewId, UserId};
use domain::kind::{KindId, Status};
use domain::library::{LibraryEntry, Rating};
use domain::progress::{Progress, ProgressEvent};
use domain::review::Review;
use domain::tag::tag_key;

use super::library_page::{write_entry, write_tags};
use crate::conv::{corrupt, hlc, json, to_json};
use crate::tx::{Op, record};
use crate::{PgStore, Res, be};

pub(super) struct EntryRow {
    pub(super) id: uuid::Uuid,
    pub(super) user_id: uuid::Uuid,
    pub(super) item_id: uuid::Uuid,
    pub(super) kind: String,
    pub(super) status: String,
    pub(super) status_hlc: String,
    pub(super) rating: Option<i16>,
    pub(super) rating_hlc: String,
    pub(super) notes: Option<String>,
    pub(super) notes_hlc: String,
    pub(super) added_at: chrono::DateTime<chrono::Utc>,
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
            "SELECT id, user_id, item_id, kind, status, status_hlc, rating, rating_hlc, notes,
                    notes_hlc, added_at FROM library_entries WHERE user_id = $1 AND item_id = $2",
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
            "SELECT id, user_id, item_id, kind, status, status_hlc, rating, rating_hlc, notes,
                    notes_hlc, added_at FROM library_entries WHERE id = $1",
            id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(LibraryEntry::try_from)
        .transpose()
    }

    async fn existing_entry_ids(&self, ids: &[EntryId]) -> Res<Vec<EntryId>> {
        let ids: Vec<uuid::Uuid> = ids.iter().map(|i| i.0).collect();
        Ok(
            sqlx::query_scalar!("SELECT id FROM library_entries WHERE id = ANY($1)", &ids)
                .fetch_all(&self.pool)
                .await
                .map_err(be)?
                .into_iter()
                .map(EntryId)
                .collect(),
        )
    }

    async fn entries_for(&self, user: UserId, items: &[ItemId]) -> Res<Vec<LibraryEntry>> {
        let items: Vec<uuid::Uuid> = items.iter().map(|i| i.0).collect();
        sqlx::query_as!(
            EntryRow,
            "SELECT id, user_id, item_id, kind, status, status_hlc, rating, rating_hlc, notes,
                    notes_hlc, added_at FROM library_entries
             WHERE user_id = $1 AND item_id = ANY($2)",
            user.0,
            &items
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(LibraryEntry::try_from)
        .collect()
    }

    async fn upsert(&self, e: &LibraryEntry) -> Res<()> {
        let mut tx = self.begin().await?;
        let id = write_entry(&mut tx, e).await?;
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
            "SELECT id, user_id, item_id, kind, status, status_hlc, rating, rating_hlc, notes,
                    notes_hlc, added_at FROM library_entries WHERE user_id = $1 ORDER BY added_at, id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(LibraryEntry::try_from)
        .collect()
    }

    async fn write_many(&self, writes: &[(LibraryEntry, Option<Vec<String>>)]) -> Res<()> {
        let mut tx = self.begin().await?;
        for (e, tags) in writes {
            let id = write_entry(&mut tx, e).await?;
            if let Some(tags) = tags {
                write_tags(&mut tx, id, e.user_id, tags).await?;
            }
            record(&mut tx, "library_entry", id, Op::Upsert, &[e.user_id]).await?;
        }
        tx.commit().await.map_err(be)
    }

    async fn remove_many(&self, user: UserId, ids: &[EntryId]) -> Res<u64> {
        let ids: Vec<uuid::Uuid> = ids.iter().map(|e| e.0).collect();
        let mut tx = self.begin().await?;
        // tags go with the entry (ON DELETE CASCADE); events, sessions, reviews and list
        // items do not reference the entry and stay
        let gone = sqlx::query_scalar!(
            "DELETE FROM library_entries WHERE user_id = $1 AND id = ANY($2) RETURNING id",
            user.0,
            &ids
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(be)?;
        for id in &gone {
            record(&mut tx, "library_entry", *id, Op::Delete, &[user]).await?;
        }
        tx.commit().await.map_err(be)?;
        Ok(gone.len() as u64)
    }

    async fn page(
        &self,
        user: UserId,
        after: Option<(Timestamp, EntryId)>,
        limit: u32,
    ) -> Res<EntryPage> {
        self.entries_after(user, after, limit).await
    }

    async fn query(&self, user: UserId, f: &LibraryFilter) -> Res<HitPage> {
        self.hits_after(user, f).await
    }

    async fn next_in_series(&self, user: UserId, kinds: &[KindId], limit: u32) -> Res<Vec<ItemId>> {
        let kinds: Vec<String> = kinds.iter().map(|k| k.as_str().to_string()).collect();
        // `best` is one row per series of the user's completed entries of `kinds` (index
        // `library_entries_kind_status`); the planned side reads only planned entries
        let ids = sqlx::query_scalar!(
            r#"WITH done AS (
                 SELECT lower(btrim(c.attributes ->> 'series')) AS series,
                        max((c.attributes ->> 'series_position')::float8) AS best
                 FROM library_entries e
                 JOIN catalog_items c ON c.id = e.item_id
                 WHERE e.user_id = $1 AND e.kind = ANY($2) AND e.status = 'completed'
                   AND jsonb_typeof(c.attributes -> 'series') = 'string'
                   AND jsonb_typeof(c.attributes -> 'series_position') = 'number'
                   AND btrim(c.attributes ->> 'series') <> ''
                 GROUP BY 1
               )
               SELECT e.item_id AS "item_id!"
               FROM library_entries e
               JOIN catalog_items c ON c.id = e.item_id
               JOIN done d ON d.series = lower(btrim(c.attributes ->> 'series'))
               WHERE e.user_id = $1 AND e.kind = ANY($2) AND e.status = 'planned'
                 AND jsonb_typeof(c.attributes -> 'series_position') = 'number'
                 AND (c.attributes ->> 'series_position')::float8 > d.best
                 AND (c.attributes ->> 'series_position')::float8 <= d.best + 1
               ORDER BY e.added_at DESC, e.id DESC
               LIMIT $3"#,
            user.0,
            &kinds,
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        Ok(ids.into_iter().map(ItemId).collect())
    }

    async fn counts(&self, user: UserId) -> Res<Vec<(KindId, Status, u64)>> {
        sqlx::query!(
            r#"SELECT kind, status, COUNT(*) AS "n!" FROM library_entries WHERE user_id = $1
               GROUP BY kind, status"#,
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(|r| {
            Ok((
                KindId::new(&r.kind).map_err(|_| corrupt("kind", &r.kind))?,
                Status::new(r.status),
                u64::try_from(r.n).unwrap_or(0),
            ))
        })
        .collect()
    }

    async fn tags_of(&self, entries: &[EntryId]) -> Res<Vec<(EntryId, String)>> {
        let ids: Vec<uuid::Uuid> = entries.iter().map(|e| e.0).collect();
        Ok(sqlx::query!(
            "SELECT entry_id, tag FROM entry_tags WHERE entry_id = ANY($1) ORDER BY entry_id, tag_key",
            &ids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(|r| (EntryId(r.entry_id), r.tag))
        .collect())
    }

    async fn set_tags(&self, e: &LibraryEntry, tags: &[String]) -> Res<()> {
        let mut tx = self.begin().await?;
        write_tags(&mut tx, e.id.0, e.user_id, tags).await?;
        record(&mut tx, "library_entry", e.id.0, Op::Upsert, &[e.user_id]).await?;
        tx.commit().await.map_err(be)
    }

    async fn tag_counts(&self, user: UserId) -> Res<Vec<(String, u64)>> {
        Ok(sqlx::query!(
            r#"SELECT (array_agg(t.tag ORDER BY e.added_at, e.id))[1] AS "tag!", COUNT(*) AS "n!"
               FROM entry_tags t JOIN library_entries e ON e.id = t.entry_id
               WHERE t.user_id = $1 GROUP BY t.tag_key ORDER BY t.tag_key"#,
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(|r| (r.tag, u64::try_from(r.n).unwrap_or(0)))
        .collect())
    }

    async fn rename_tag(&self, user: UserId, from_key: &str, to: &str) -> Res<u64> {
        let mut tx = self.begin().await?;
        let to_key = tag_key(to);
        let entries = sqlx::query_scalar!(
            "SELECT entry_id FROM entry_tags WHERE user_id = $1 AND tag_key = $2",
            user.0,
            from_key
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(be)?;
        sqlx::query!(
            "DELETE FROM entry_tags WHERE entry_id = ANY($1) AND tag_key IN ($2, $3)",
            &entries,
            from_key,
            to_key
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        sqlx::query!(
            "INSERT INTO entry_tags (entry_id, user_id, tag, tag_key)
             SELECT e, $2, $3, $4 FROM unnest($1::uuid[]) AS e",
            &entries,
            user.0,
            to,
            to_key
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        for e in &entries {
            record(&mut tx, "library_entry", *e, Op::Upsert, &[user]).await?;
        }
        tx.commit().await.map_err(be)?;
        Ok(entries.len() as u64)
    }
}

struct EventRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    item_id: uuid::Uuid,
    occurred_at: chrono::DateTime<chrono::Utc>,
    progress: serde_json::Value,
    client_op_id: Option<uuid::Uuid>,
    recorded_at: chrono::DateTime<chrono::Utc>,
    status_change: Option<serde_json::Value>,
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
            recorded_at: r.recorded_at,
            status_change: r
                .status_change
                .map(|v| json("status change", v))
                .transpose()?,
        })
    }
}

fn events(rows: Vec<EventRow>) -> Res<Vec<ProgressEvent>> {
    rows.into_iter().map(ProgressEvent::try_from).collect()
}

fn count(n: Option<i64>) -> u64 {
    n.and_then(|n| u64::try_from(n).ok()).unwrap_or(0)
}

impl PgStore {
    /// The statistics of `user` computed from all their events, without the cache (the
    /// cache is checked against it, WP4-1).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn month_amounts_full(
        &self,
        user: UserId,
        zone: &str,
    ) -> Res<Vec<app::MonthAmounts>> {
        // the rules of `app::month_amounts` in one statement: effective events, forward
        // moves of pages and positions per item in (occurred_at, id) order, runtimes as
        // whole numbers, durations of the user's own sessions
        let zone = self.stats_zone(zone).await?;
        let rows = sqlx::query!(
            r#"WITH live AS (
                 SELECT x.id, x.item_id, x.occurred_at, x.status_change, c.kind, c.attributes,
                        x.progress ->> 'type' AS t,
                        CASE WHEN x.progress ->> 'type' = 'paged'
                             THEN jsonb_u64(x.progress -> 'page') END AS page,
                        CASE WHEN x.progress ->> 'type' = 'position'
                             THEN jsonb_u64(x.progress -> 'seconds') END AS pos,
                        CASE WHEN x.progress ->> 'type' = 'session'
                             THEN (SELECT s.duration_minutes FROM sessions s
                                   WHERE s.owner_id = x.user_id
                                     AND s.id = (x.progress ->> 'session_id')::uuid) END
                            AS duration
                 FROM progress_events x
                 JOIN catalog_items c ON c.id = x.item_id
                 WHERE x.user_id = $1 AND x.undoes IS NULL
                   AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)
               ), moves AS (
                 SELECT l.*,
                        COALESCE(lag(page) OVER (PARTITION BY item_id, page IS NULL
                                                 ORDER BY occurred_at, id), 0) AS prev_page,
                        COALESCE(lag(pos) OVER (PARTITION BY item_id, pos IS NULL
                                                ORDER BY occurred_at, id), 0) AS prev_pos
                 FROM live l
               )
               SELECT kind AS "kind!",
                      to_char(occurred_at AT TIME ZONE $2, 'YYYY-MM') AS "month!",
                      COUNT(*) AS events,
                      COUNT(*) FILTER (WHERE status_change ->> 'to' = 'completed'
                                        AND t <> 'rewatch_stop') AS completions,
                      COUNT(*) FILTER (WHERE t = 'finished') AS finished,
                      COUNT(*) FILTER (WHERE t = 'episode') AS episodes,
                      COUNT(*) FILTER (WHERE t = 'session') AS plays,
                      SUM(GREATEST(page - prev_page, 0))::bigint AS pages,
                      SUM(CASE t
                            WHEN 'finished'
                              THEN COALESCE(jsonb_u64(attributes -> 'runtime_minutes'), 0) * 60
                            WHEN 'episode'
                              THEN COALESCE(jsonb_u64(attributes -> 'episode_runtime_minutes'),
                                            0) * 60
                            WHEN 'position' THEN GREATEST(pos - prev_pos, 0)
                            WHEN 'session' THEN COALESCE(duration, 0)::bigint * 60
                            ELSE 0 END)::bigint AS seconds
               FROM moves
               GROUP BY 1, 2
               ORDER BY kind COLLATE "C", 2"#,
            user.0,
            zone
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        rows.into_iter()
            .map(|r| {
                Ok(app::MonthAmounts {
                    kind: KindId::new(&r.kind).map_err(|_| corrupt("kind", &r.kind))?,
                    month: r.month,
                    amounts: app::Amounts {
                        events: count(r.events),
                        completions: count(r.completions),
                        finished: count(r.finished),
                        episodes: count(r.episodes),
                        pages: count(r.pages),
                        plays: count(r.plays),
                        minutes: 0,
                    },
                    seconds: count(r.seconds),
                })
            })
            .collect()
    }
}

#[async_trait]
impl ProgressRepo for PgStore {
    async fn month_amounts(&self, user: UserId, zone: &str) -> Res<Vec<app::MonthAmounts>> {
        self.cached_month_amounts(user, zone).await
    }

    async fn window_amounts(
        &self,
        user: UserId,
        from: domain::Timestamp,
        to: domain::Timestamp,
    ) -> Res<Vec<app::KindAmounts>> {
        self.pg_window_amounts(user, from, to).await
    }

    async fn append(&self, e: &ProgressEvent) -> Res<ProgressEvent> {
        let mut tx = self.begin().await?;
        let undoes = match e.progress {
            Progress::Undo { event_id } => Some(event_id.0),
            _ => None,
        };
        let status_change = e.status_change.as_ref().map(to_json).transpose()?;
        let inserted = sqlx::query_scalar!(
            "INSERT INTO progress_events (id, user_id, item_id, occurred_at, progress, client_op_id,
                                          recorded_at, status_change, undoes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (user_id, client_op_id) DO NOTHING
             RETURNING id",
            e.id.0,
            e.user_id.0,
            e.item_id.0,
            e.occurred_at,
            to_json(&e.progress)?,
            e.client_op_id,
            e.recorded_at,
            status_change,
            undoes
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(be)?;
        if inserted.is_none() {
            let existing = sqlx::query_as!(
                EventRow,
                "SELECT id, user_id, item_id, occurred_at, progress, client_op_id, recorded_at,
                        status_change
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
        events(
            sqlx::query_as!(
                EventRow,
                "SELECT id, user_id, item_id, occurred_at, progress, client_op_id, recorded_at,
                        status_change
                 FROM progress_events WHERE user_id = $1 AND item_id = $2 ORDER BY occurred_at, id",
                user.0,
                item.0
            )
            .fetch_all(&self.pool)
            .await
            .map_err(be)?,
        )
    }

    async fn get(&self, id: EventId) -> Res<Option<ProgressEvent>> {
        sqlx::query_as!(
            EventRow,
            "SELECT id, user_id, item_id, occurred_at, progress, client_op_id, recorded_at,
                    status_change
             FROM progress_events WHERE id = $1",
            id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(ProgressEvent::try_from)
        .transpose()
    }

    async fn by_user(&self, user: UserId) -> Res<Vec<ProgressEvent>> {
        events(
            sqlx::query_as!(
                EventRow,
                "SELECT id, user_id, item_id, occurred_at, progress, client_op_id, recorded_at,
                        status_change
                 FROM progress_events WHERE user_id = $1 ORDER BY occurred_at, id",
                user.0
            )
            .fetch_all(&self.pool)
            .await
            .map_err(be)?,
        )
    }

    async fn for_items(&self, user: UserId, items: &[ItemId]) -> Res<Vec<ProgressEvent>> {
        let ids: Vec<uuid::Uuid> = items.iter().map(|i| i.0).collect();
        events(
            sqlx::query_as!(
                EventRow,
                "SELECT id, user_id, item_id, occurred_at, progress, client_op_id, recorded_at,
                        status_change
                 FROM progress_events WHERE user_id = $1 AND item_id = ANY($2)
                 ORDER BY occurred_at, id",
                user.0,
                &ids
            )
            .fetch_all(&self.pool)
            .await
            .map_err(be)?,
        )
    }

    async fn recent(&self, user: UserId, w: &HistoryWindow) -> Res<Vec<ProgressEvent>> {
        let kinds: Vec<String> = w.kinds.iter().map(|k| k.as_str().to_string()).collect();
        events(
            sqlx::query_as!(
                EventRow,
                "SELECT x.id, x.user_id, x.item_id, x.occurred_at, x.progress, x.client_op_id,
                        x.recorded_at, x.status_change
                 FROM progress_events x JOIN catalog_items c ON c.id = x.item_id
                 WHERE x.user_id = $1 AND x.undoes IS NULL
                   AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)
                   AND ($2::timestamptz IS NULL OR (x.occurred_at, x.id) < ($2, $3::uuid))
                   AND ($4::timestamptz IS NULL OR x.occurred_at >= $4)
                   AND (cardinality($5::text[]) = 0 OR c.kind = ANY($5))
                 ORDER BY x.occurred_at DESC, x.id DESC
                 LIMIT $6",
                user.0,
                w.before.map(|b| b.0),
                w.before.map_or(uuid::Uuid::nil(), |b| b.1),
                w.since,
                &kinds,
                i64::from(w.limit)
            )
            .fetch_all(&self.pool)
            .await
            .map_err(be)?,
        )
    }

    async fn recent_spans(&self, user: UserId, w: &HistorySpans) -> Res<Vec<ProgressEvent>> {
        let (from, to) = super::sessions::span_arrays(w);
        let kinds: Vec<String> = w.kinds.iter().map(|k| k.as_str().to_string()).collect();
        // DEC-494: the span rows first (one range scan of `progress_events_user_recent` per
        // span; with the LIMIT outside the planner walked every event of the user instead),
        // the kind per row by its key (a join or EXISTS hashed all catalog items)
        events(
            sqlx::query_as!(
                EventRow,
                r#"WITH x AS MATERIALIZED (
                     SELECT x.id, x.user_id, x.item_id, x.occurred_at, x.progress, x.client_op_id,
                            x.recorded_at, x.status_change, x.undoes
                     FROM unnest($2::timestamptz[], $3::timestamptz[]) AS sp(lo, hi)
                     JOIN progress_events x ON x.user_id = $1
                          AND x.occurred_at >= sp.lo AND x.occurred_at < sp.hi)
                 SELECT x.id AS "id!", x.user_id AS "user_id!", x.item_id AS "item_id!",
                        x.occurred_at AS "occurred_at!", x.progress AS "progress!",
                        x.client_op_id, x.recorded_at AS "recorded_at!", x.status_change
                 FROM x
                 WHERE x.undoes IS NULL
                   AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)
                   AND (cardinality($4::text[]) = 0
                        OR (SELECT c.kind FROM catalog_items c WHERE c.id = x.item_id) = ANY($4))
                 ORDER BY x.occurred_at DESC, x.id DESC
                 LIMIT $5"#,
                user.0,
                &from,
                &to,
                &kinds,
                i64::from(w.limit)
            )
            .fetch_all(&self.pool)
            .await
            .map_err(be)?,
        )
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
