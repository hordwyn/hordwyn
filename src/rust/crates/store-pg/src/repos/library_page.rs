//! Library pages with keyset paging (DEC-416) and the shared entry and tag writes.
//!
//! A page continues strictly after the previous page's last row in the order (its sort key,
//! then the entry id), so a deep page reads as few rows as the first one; `OFFSET` read and
//! threw away every row before the page (DEC-404: the last page of 50 000 sat at the 300 ms
//! budget under eight clients).

use app::ports::{
    EntryPage, HitPage, LibraryCursor, LibraryFilter, LibraryHit, LibrarySort, SortValue,
};
use domain::Timestamp;
use domain::ids::{EntryId, UserId};
use domain::library::LibraryEntry;
use domain::tag::tag_key;
use sqlx::{Postgres, Transaction};

use super::library::EntryRow;
use crate::{PgStore, Res, be};

/// Insert or update an entry by `(user, item)`; returns the stored id (an existing entry
/// keeps its id and `added_at`).
pub(super) async fn write_entry(
    tx: &mut Transaction<'static, Postgres>,
    e: &LibraryEntry,
) -> Res<uuid::Uuid> {
    sqlx::query_scalar!(
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
    .fetch_one(&mut **tx)
    .await
    .map_err(be)
}

/// Replace the tags of an entry (normalised, distinct by key).
pub(super) async fn write_tags(
    tx: &mut Transaction<'static, Postgres>,
    entry: uuid::Uuid,
    user: UserId,
    tags: &[String],
) -> Res<()> {
    sqlx::query!("DELETE FROM entry_tags WHERE entry_id = $1", entry)
        .execute(&mut **tx)
        .await
        .map_err(be)?;
    let keys: Vec<String> = tags.iter().map(|t| tag_key(t)).collect();
    sqlx::query!(
        "INSERT INTO entry_tags (entry_id, user_id, tag, tag_key)
         SELECT $1, $2, t, k FROM unnest($3::text[], $4::text[]) AS x (t, k)
         ON CONFLICT DO NOTHING",
        entry,
        user.0,
        tags,
        &keys
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    Ok(())
}

struct HitRow {
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
    last_activity: chrono::DateTime<chrono::Utc>,
    key_text: Option<String>,
    key_time: Option<chrono::DateTime<chrono::Utc>>,
    key_int: Option<i64>,
    total: i64,
}

const fn sort_str(s: LibrarySort) -> &'static str {
    match s {
        LibrarySort::LastActivity => "last_activity",
        LibrarySort::Title => "title",
        LibrarySort::Added => "added",
        LibrarySort::Year => "year",
        LibrarySort::Rating => "rating",
        LibrarySort::Plays => "plays",
    }
}

/// The cursor parameters: (text, time, number) key and entry id.
type CursorArgs = (
    Option<String>,
    Option<Timestamp>,
    Option<i64>,
    Option<uuid::Uuid>,
);

fn cursor_args(c: Option<&LibraryCursor>) -> CursorArgs {
    let Some(c) = c else {
        return (None, None, None, None);
    };
    let id = Some(c.id.0);
    match &c.key {
        SortValue::Text(t) => (Some(t.clone()), None, None, id),
        SortValue::Time(t) => (None, Some(*t), None, id),
        SortValue::Int(n) => (None, None, *n, id),
    }
}

impl PgStore {
    pub(super) async fn entries_after(
        &self,
        user: UserId,
        after: Option<(Timestamp, EntryId)>,
        limit: u32,
    ) -> Res<EntryPage> {
        let (at, id) = (after.map(|a| a.0), after.map(|a| a.1.0));
        // `added_at <= $2` lets the scan of `library_entries_recent` start at the cursor
        let mut rows = sqlx::query_as!(
            EntryRow,
            "SELECT id, user_id, item_id, kind, status, status_hlc, rating, rating_hlc, notes,
                    notes_hlc, added_at FROM library_entries
             WHERE user_id = $1
               AND ($2::timestamptz IS NULL
                    OR (added_at <= $2 AND (added_at < $2 OR id < $3::uuid)))
             ORDER BY added_at DESC, id DESC LIMIT $4",
            user.0,
            at,
            id,
            i64::from(limit) + 1
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        let more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let total = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!" FROM library_entries WHERE user_id = $1"#,
            user.0
        )
        .fetch_one(&self.pool)
        .await
        .map_err(be)?;
        Ok(EntryPage {
            entries: rows
                .into_iter()
                .map(LibraryEntry::try_from)
                .collect::<Res<_>>()?,
            total: u64::try_from(total).unwrap_or(0),
            more,
        })
    }

    /// One page of a library query after `f.after`, the number of matches, and the next
    /// cursor. An empty page after a cursor (its rows were removed meanwhile) counts again
    /// without one.
    pub(super) async fn hits_after(&self, user: UserId, f: &LibraryFilter) -> Res<HitPage> {
        let mut rows = self
            .query_rows(user, f, f.after.as_ref(), f.limit + 1)
            .await?;
        let total = match rows.first() {
            Some(r) => u64::try_from(r.total).unwrap_or(0),
            None if f.after.is_some() => self
                .query_rows(user, f, None, 1)
                .await?
                .first()
                .map_or(0, |r| u64::try_from(r.total).unwrap_or(0)),
            None => 0,
        };
        let more = rows.len() > f.limit as usize;
        rows.truncate(f.limit as usize);
        let next = rows.last().filter(|_| more).map(|r| LibraryCursor {
            sort: f.sort,
            descending: f.descending,
            key: match f.sort {
                LibrarySort::Title => SortValue::Text(r.key_text.clone().unwrap_or_default()),
                LibrarySort::Added | LibrarySort::LastActivity => {
                    SortValue::Time(r.key_time.unwrap_or(r.added_at))
                }
                LibrarySort::Year | LibrarySort::Rating | LibrarySort::Plays => {
                    SortValue::Int(r.key_int)
                }
            },
            id: EntryId(r.id),
        });
        Ok(HitPage {
            hits: rows.into_iter().map(hit_from).collect::<Res<_>>()?,
            total,
            next,
        })
    }

    /// Rows of a library query. Filters, the order and the cursor are parameters of one
    /// static statement (no SQL is built from input): the sort picks one of three key columns
    /// (text, time, number; the others stay NULL), the rows are ordered by that key and the
    /// entry id in the chosen direction (missing numbers last), and the cursor keeps rows
    /// strictly after its key and id. The total counts every match (an uncorrelated
    /// subquery over the same rows).
    async fn query_rows(
        &self,
        user: UserId,
        f: &LibraryFilter,
        after: Option<&LibraryCursor>,
        limit: u32,
    ) -> Res<Vec<HitRow>> {
        let FilterArgs {
            kinds,
            statuses,
            facets,
            items,
        } = FilterArgs::of(f);
        let (k_text, k_time, k_int, k_id) = cursor_args(after);
        sqlx::query_as!(
            HitRow,
            r#"WITH hits AS (
                 SELECT e.id, e.user_id, e.item_id, e.kind, e.status, e.status_hlc, e.rating,
                        e.rating_hlc, e.notes, e.notes_hlc, e.added_at,
                        GREATEST(e.added_at, COALESCE(e.last_event_at, e.added_at))
                            AS last_activity,
                        -- title and year are read from the catalog only when they are the key
                        CASE WHEN $7 = 'title' THEN (SELECT lower(c.title) FROM catalog_items c
                                                     WHERE c.id = e.item_id) END AS key_text,
                        CASE $7 WHEN 'added' THEN e.added_at
                                WHEN 'last_activity'
                                THEN GREATEST(e.added_at, COALESCE(e.last_event_at, e.added_at))
                        END AS key_time,
                        CASE $7 WHEN 'year' THEN (SELECT c.year::int8 FROM catalog_items c
                                                  WHERE c.id = e.item_id)
                                WHEN 'rating' THEN e.rating::int8
                                WHEN 'plays' THEN e.plays::int8
                        END AS key_int
                 FROM library_entries e
                 WHERE e.user_id = $1
                   AND (cardinality($2::text[]) = 0 OR e.kind = ANY($2))
                   AND (cardinality($3::text[]) = 0 OR e.status = ANY($3))
                   AND (cardinality($4::text[]) = 0
                        OR (SELECT COUNT(*) FROM entry_tags t
                            WHERE t.entry_id = e.id AND t.tag_key = ANY($4::text[]))
                           = cardinality($4::text[]))
                   AND (jsonb_array_length($5::jsonb) = 0 OR NOT EXISTS (
                       SELECT 1 FROM catalog_items c, jsonb_array_elements($5::jsonb) AS f
                       WHERE c.id = e.item_id AND NOT EXISTS (
                           SELECT 1 FROM jsonb_array_elements_text(
                               CASE WHEN jsonb_typeof(c.attributes -> (f ->> 0)) = 'array'
                                    THEN c.attributes -> (f ->> 0)
                                    ELSE jsonb_build_array(c.attributes -> (f ->> 0)) END) AS v
                           WHERE v = f ->> 1)))
                   AND ($6::uuid[] IS NULL OR e.item_id = ANY($6))
               )
               SELECT id AS "id!", user_id AS "user_id!", item_id AS "item_id!", kind AS "kind!",
                      status AS "status!", status_hlc AS "status_hlc!", rating,
                      rating_hlc AS "rating_hlc!", notes, notes_hlc AS "notes_hlc!",
                      added_at AS "added_at!", last_activity AS "last_activity!",
                      key_text, key_time, key_int,
                      (SELECT COUNT(*) FROM hits) AS "total!"
               FROM hits
               WHERE $13::uuid IS NULL OR CASE
                 WHEN $7 = 'title' THEN CASE WHEN $8
                      THEN (key_text, id) < ($10::text, $13)
                      ELSE (key_text, id) > ($10::text, $13) END
                 WHEN $7 IN ('added', 'last_activity') THEN CASE WHEN $8
                      THEN (key_time, id) < ($11::timestamptz, $13)
                      ELSE (key_time, id) > ($11::timestamptz, $13) END
                 -- numbers: missing ones come last in both directions
                 WHEN $12::int8 IS NULL
                      THEN key_int IS NULL AND CASE WHEN $8 THEN id < $13 ELSE id > $13 END
                 WHEN key_int IS NULL THEN true
                 WHEN $8 THEN (key_int, id) < ($12, $13)
                 ELSE (key_int, id) > ($12, $13)
               END
               ORDER BY
                 CASE WHEN NOT $8 THEN key_text END ASC,
                 CASE WHEN $8 THEN key_text END DESC,
                 CASE WHEN NOT $8 THEN key_time END ASC,
                 CASE WHEN $8 THEN key_time END DESC,
                 CASE WHEN NOT $8 THEN key_int END ASC NULLS LAST,
                 CASE WHEN $8 THEN key_int END DESC NULLS LAST,
                 CASE WHEN NOT $8 THEN id END ASC,
                 CASE WHEN $8 THEN id END DESC
               LIMIT $9"#,
            user.0,
            &kinds,
            &statuses,
            &f.tags,
            facets,
            items.as_deref(),
            sort_str(f.sort),
            f.descending,
            i64::from(limit),
            k_text,
            k_time,
            k_int,
            k_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)
    }
}

/// The filter as statement parameters (split out of `query_rows`, UX4-19).
struct FilterArgs {
    kinds: Vec<String>,
    statuses: Vec<String>,
    facets: serde_json::Value,
    items: Option<Vec<uuid::Uuid>>,
}

impl FilterArgs {
    fn of(f: &LibraryFilter) -> Self {
        Self {
            kinds: f.kinds.iter().map(|k| k.as_str().to_string()).collect(),
            statuses: f.statuses.iter().map(|s| s.as_str().to_string()).collect(),
            facets: serde_json::Value::Array(
                f.facets
                    .iter()
                    .map(|(k, v)| serde_json::json!([k, v]))
                    .collect(),
            ),
            items: f.items.as_ref().map(|v| v.iter().map(|i| i.0).collect()),
        }
    }
}

fn hit_from(r: HitRow) -> Res<LibraryHit> {
    let entry = LibraryEntry::try_from(EntryRow {
        id: r.id,
        user_id: r.user_id,
        item_id: r.item_id,
        kind: r.kind,
        status: r.status,
        status_hlc: r.status_hlc,
        rating: r.rating,
        rating_hlc: r.rating_hlc,
        notes: r.notes,
        notes_hlc: r.notes_hlc,
        added_at: r.added_at,
    })?;
    Ok(LibraryHit {
        entry,
        last_activity: r.last_activity,
    })
}
