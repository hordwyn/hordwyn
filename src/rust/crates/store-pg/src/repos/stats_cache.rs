//! The statistics cache (WP4-1, DEC-235, migration 0023): the amounts per user, item and
//! month of the user's zone in `stats_item_months`, summed per kind and month in
//! `stats_months`. Triggers mark changed `(user, item)` pairs in `stats_dirty`; a read
//! recomputes only those with the full statement restricted to them and the sums of the months
//! they touch, or everything when the zone stamp in `stats_users` differs.

use app::MonthAmounts;
use domain::ids::UserId;
use domain::kind::KindId;

use crate::conv::corrupt;
use crate::{PgStore, Res, be};

fn count(n: Option<i64>) -> u64 {
    n.and_then(|n| u64::try_from(n).ok()).unwrap_or(0)
}

impl PgStore {
    /// The zone Postgres buckets in: `zone`, or UTC when this server's tzdata does not know
    /// it (a zone chrono-tz knows but Postgres does not, UX3-20).
    pub(crate) async fn stats_zone<'a>(&self, zone: &'a str) -> Res<&'a str> {
        bucket_zone(&self.pool, zone).await
    }

    /// The statistics of `user` from the cache, brought up to date first.
    pub(crate) async fn cached_month_amounts(
        &self,
        user: UserId,
        zone: &str,
    ) -> Res<Vec<MonthAmounts>> {
        let state = sqlx::query!(
            r#"SELECT (SELECT s.zone FROM stats_users s WHERE s.user_id = $1) AS "stamp?",
                      EXISTS (SELECT 1 FROM stats_dirty d WHERE d.user_id = $1) AS "dirty!""#,
            user.0
        )
        .fetch_one(&self.pool)
        .await
        .map_err(be)?;
        if state.dirty || state.stamp.as_deref() != Some(zone) {
            self.refresh_stats(user, zone).await?;
        }
        let rows = sqlx::query!(
            r#"SELECT kind, month, events AS "events?", completions AS "completions?",
                      finished AS "finished?", episodes AS "episodes?", pages AS "pages?",
                      plays AS "plays?", seconds AS "seconds?"
               FROM stats_months WHERE user_id = $1
               ORDER BY kind COLLATE "C", month"#,
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        rows.into_iter()
            .map(|r| {
                Ok(MonthAmounts {
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

    /// Recompute the dirty items of `user` (all of them when the cache was built in another
    /// zone). One transaction holding the user's stamp row, so concurrent readers wait for
    /// one rebuild instead of each doing it.
    async fn refresh_stats(&self, user: UserId, zone: &str) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO stats_users (user_id, zone) VALUES ($1, '') ON CONFLICT DO NOTHING",
            user.0
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        let stamp = sqlx::query!(
            "SELECT zone, bucket_zone FROM stats_users WHERE user_id = $1 FOR UPDATE",
            user.0
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(be)?;
        // read before the recompute reads the events and taken only at the end, by
        // generation (UX4-1): a mark a write moves or holds meanwhile stays for the next read
        let marks = sqlx::query!(
            "SELECT item_id, gen AS mark FROM stats_dirty WHERE user_id = $1",
            user.0
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(be)?;
        let (dirty, gens): (Vec<uuid::Uuid>, Vec<i64>) =
            marks.into_iter().map(|m| (m.item_id, m.mark)).unzip();
        let full = stamp.zone != zone;
        let bucket_zone = if full {
            bucket_zone(&mut *tx, zone).await?.to_string()
        } else {
            stamp.bucket_zone
        };
        let items: Option<&[uuid::Uuid]> = if full {
            None
        } else if dirty.is_empty() {
            return tx.commit().await.map_err(be);
        } else {
            Some(&dirty)
        };
        // a rebuild sorts all of the user's events by item; keep that sort in memory
        sqlx::query!("SET LOCAL work_mem = '32MB'")
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        // the old rows of the items leave the sums per kind and month ...
        sqlx::query!(
            "WITH gone AS (
                 DELETE FROM stats_item_months
                 WHERE user_id = $1 AND ($2::uuid[] IS NULL OR item_id = ANY($2))
                 RETURNING kind, month, events, completions, finished, episodes, pages, plays,
                           seconds
             )
             INSERT INTO stats_months AS m (user_id, kind, month, events, completions,
                                            finished, episodes, pages, plays, seconds)
             SELECT $1, kind, month, -SUM(events), -SUM(completions), -SUM(finished),
                    -SUM(episodes), -SUM(pages), -SUM(plays), -SUM(seconds)
             FROM gone GROUP BY kind, month
             ON CONFLICT (user_id, kind, month) DO UPDATE SET
                 events = m.events + EXCLUDED.events,
                 completions = m.completions + EXCLUDED.completions,
                 finished = m.finished + EXCLUDED.finished,
                 episodes = m.episodes + EXCLUDED.episodes,
                 pages = m.pages + EXCLUDED.pages, plays = m.plays + EXCLUDED.plays,
                 seconds = m.seconds + EXCLUDED.seconds",
            user.0,
            items
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        // ... the new ones join them, and months without any item row go (a month can hold a
        // completion set by hand and no event, LB-13)
        insert_item_months(&mut tx, user, &bucket_zone, items).await?;
        sqlx::query!(
            "DELETE FROM stats_months WHERE user_id = $1 AND events = 0 AND completions = 0",
            user.0
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        sqlx::query!(
            "UPDATE stats_users SET zone = $2, bucket_zone = $3 WHERE user_id = $1",
            user.0,
            zone,
            bucket_zone
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        take_marks(&mut tx, user, &dirty, &gens).await?;
        tx.commit().await.map_err(be)
    }
}

/// The zone Postgres buckets in: `zone`, or UTC when this server's tzdata does not know it
/// (a zone chrono-tz knows but Postgres does not, UX3-20).
async fn bucket_zone<'a>(exec: impl sqlx::PgExecutor<'_>, zone: &'a str) -> Res<&'a str> {
    let known = sqlx::query_scalar!(
        r#"SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name = $1) AS "known!""#,
        zone
    )
    .fetch_one(exec)
    .await
    .map_err(be)?;
    if !known {
        tracing::warn!(zone, "time zone unknown to Postgres; statistics use UTC");
    }
    Ok(if known { zone } else { "UTC" })
}

/// Delete the marks the refresh read, unless a write moved one to a new generation meanwhile
/// or still holds it uncommitted (its event may be missing from the recompute): those stay
/// for the next read (UX4-1). Never waits for a writer, so no lock cycle with one either.
async fn take_marks(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    user: UserId,
    items: &[uuid::Uuid],
    gens: &[i64],
) -> Res<()> {
    if items.is_empty() {
        return Ok(());
    }
    sqlx::query!(
        "WITH seen AS (
             SELECT d.item_id FROM stats_dirty d
             JOIN unnest($2::uuid[], $3::bigint[]) AS r(item_id, gen)
               ON r.item_id = d.item_id AND r.gen = d.gen
             WHERE d.user_id = $1
             FOR UPDATE OF d SKIP LOCKED
         )
         DELETE FROM stats_dirty WHERE user_id = $1 AND item_id IN (SELECT item_id FROM seen)",
        user.0,
        items,
        gens
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    Ok(())
}

/// The rules of `app::month_amounts` per item and month (the full statement of
/// [`PgStore::month_amounts_full`] grouped by item as well), for `items` or all, added to the
/// sums per kind and month as well. A session's duration is joined (its id is the key)
/// instead of looked up per event.
async fn insert_item_months(
    tx: &mut sqlx::Transaction<'static, sqlx::Postgres>,
    user: UserId,
    zone: &str,
    items: Option<&[uuid::Uuid]>,
) -> Res<()> {
    sqlx::query!(
        r#"WITH live AS (
             SELECT x.id, x.item_id, x.occurred_at, x.status_change, c.kind, c.attributes,
                    x.progress ->> 'type' AS t,
                    CASE WHEN x.progress ->> 'type' = 'paged'
                         THEN jsonb_u64(x.progress -> 'page') END AS page,
                    CASE WHEN x.progress ->> 'type' = 'position'
                         THEN jsonb_u64(x.progress -> 'seconds') END AS pos,
                    s.duration_minutes AS duration
             FROM progress_events x
             JOIN catalog_items c ON c.id = x.item_id
             LEFT JOIN sessions s
               ON x.progress ->> 'type' = 'session' AND s.owner_id = x.user_id
              AND s.id = (CASE WHEN x.progress ->> 'type' = 'session'
                               THEN x.progress ->> 'session_id' END)::uuid
             WHERE x.user_id = $1 AND x.undoes IS NULL
               AND ($3::uuid[] IS NULL OR x.item_id = ANY($3))
               AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)
             UNION ALL
             -- LB-14: the completion no event made is one completion on its day; one without
             -- a day (LB-15) is in no month (the empty month)
             SELECT e.id, e.item_id, e.completed_at, jsonb_build_object('to', 'completed'),
                    c.kind, c.attributes, 'status', NULL, NULL, NULL
             FROM library_entries e
             JOIN catalog_items c ON c.id = e.item_id
             WHERE e.user_id = $1 AND (e.completed_at IS NOT NULL OR e.completed_undated)
               AND ($3::uuid[] IS NULL OR e.item_id = ANY($3))
           ), moves AS (
             SELECT l.*,
                    COALESCE(lag(page) OVER (PARTITION BY item_id, page IS NULL
                                             ORDER BY occurred_at, id), 0) AS prev_page,
                    COALESCE(lag(pos) OVER (PARTITION BY item_id, pos IS NULL
                                            ORDER BY occurred_at, id), 0) AS prev_pos
             FROM live l
           ), added AS (
             INSERT INTO stats_item_months (user_id, item_id, kind, month, events, completions,
                                            finished, episodes, pages, plays, seconds)
             SELECT $1, item_id, kind,
                    COALESCE(to_char(occurred_at AT TIME ZONE $2, 'YYYY-MM'), ''),
                    COUNT(*) FILTER (WHERE t <> 'status'),
                    COUNT(*) FILTER (WHERE status_change ->> 'to' = 'completed'
                                        AND t <> 'rewatch_stop'),
                    COUNT(*) FILTER (WHERE t = 'finished'),
                    COUNT(*) FILTER (WHERE t = 'episode'),
                    COALESCE(SUM(GREATEST(page - prev_page, 0)), 0)::bigint,
                    COUNT(*) FILTER (WHERE t = 'session'),
                    COALESCE(SUM(CASE t
                          WHEN 'finished'
                            THEN COALESCE(jsonb_u64(attributes -> 'runtime_minutes'), 0) * 60
                          WHEN 'episode'
                            THEN COALESCE(jsonb_u64(attributes -> 'episode_runtime_minutes'),
                                          0) * 60
                          WHEN 'position' THEN GREATEST(pos - prev_pos, 0)
                          WHEN 'session' THEN COALESCE(duration, 0)::bigint * 60
                          ELSE 0 END), 0)::bigint
             FROM moves
             GROUP BY item_id, kind, 4
             RETURNING kind, month, events, completions, finished, episodes, pages, plays,
                       seconds
           )
           INSERT INTO stats_months AS m (user_id, kind, month, events, completions, finished,
                                          episodes, pages, plays, seconds)
           SELECT $1, kind, month, SUM(events), SUM(completions), SUM(finished), SUM(episodes),
                  SUM(pages), SUM(plays), SUM(seconds)
           FROM added GROUP BY kind, month
           ON CONFLICT (user_id, kind, month) DO UPDATE SET
               events = m.events + EXCLUDED.events,
               completions = m.completions + EXCLUDED.completions,
               finished = m.finished + EXCLUDED.finished,
               episodes = m.episodes + EXCLUDED.episodes,
               pages = m.pages + EXCLUDED.pages, plays = m.plays + EXCLUDED.plays,
               seconds = m.seconds + EXCLUDED.seconds"#,
        user.0,
        zone,
        items
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    Ok(())
}
