//! WP4-6 (migration 0026): yearly goals, game groups with their members, and the statistics
//! of a time window (the current month of "this year so far").

use std::collections::HashMap;

use app::KindAmounts;
use app::ports::RepoError;
use domain::Timestamp;
use domain::game_group::GameGroup;
use domain::goal::{Goal, GoalMetric};
use domain::ids::{CompanionId, GameGroupId, GoalId, UserId};
use domain::kind::{KindGroup, KindId};

use crate::conv::corrupt;
use crate::{PgStore, Res, be};

struct GoalRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    year: i32,
    kind_group: String,
    metric: String,
    target: i32,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<GoalRow> for Goal {
    type Error = RepoError;
    fn try_from(r: GoalRow) -> Result<Self, Self::Error> {
        Ok(Goal {
            id: GoalId(r.id),
            user_id: UserId(r.user_id),
            year: r.year,
            group: (r.kind_group != "all").then(|| KindGroup::parse(&r.kind_group)),
            metric: GoalMetric::parse(&r.metric).map_err(|_| corrupt("metric", &r.metric))?,
            target: u32::try_from(r.target)
                .map_err(|_| corrupt("target", &r.target.to_string()))?,
            created_at: r.created_at,
        })
    }
}

fn group_str(g: Option<KindGroup>) -> &'static str {
    g.map_or("all", KindGroup::as_str)
}

fn count(n: Option<i64>) -> u64 {
    n.and_then(|n| u64::try_from(n).ok()).unwrap_or(0)
}

impl PgStore {
    pub(crate) async fn pg_goals(&self, user: UserId) -> Res<Vec<Goal>> {
        sqlx::query_as!(
            GoalRow,
            "SELECT id, user_id, year, kind_group, metric, target, created_at
             FROM goals WHERE user_id = $1",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(Goal::try_from)
        .collect()
    }

    pub(crate) async fn pg_goal(&self, id: GoalId) -> Res<Option<Goal>> {
        sqlx::query_as!(
            GoalRow,
            "SELECT id, user_id, year, kind_group, metric, target, created_at
             FROM goals WHERE id = $1",
            id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(Goal::try_from)
        .transpose()
    }

    pub(crate) async fn pg_upsert_goal(&self, g: &Goal) -> Res<()> {
        let target =
            i32::try_from(g.target).map_err(|_| RepoError::Backend("target too large".into()))?;
        sqlx::query!(
            "INSERT INTO goals (id, user_id, year, kind_group, metric, target, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET year = EXCLUDED.year,
                 kind_group = EXCLUDED.kind_group, metric = EXCLUDED.metric,
                 target = EXCLUDED.target",
            g.id.0,
            g.user_id.0,
            g.year,
            group_str(g.group),
            g.metric.as_str(),
            target,
            g.created_at
        )
        .execute(&self.pool)
        .await
        .map_err(be)?;
        Ok(())
    }

    pub(crate) async fn pg_delete_goal(&self, id: GoalId) -> Res<()> {
        sqlx::query!("DELETE FROM goals WHERE id = $1", id.0)
            .execute(&self.pool)
            .await
            .map_err(be)?;
        Ok(())
    }

    async fn groups_where(
        &self,
        owner: Option<UserId>,
        id: Option<GameGroupId>,
    ) -> Res<Vec<GameGroup>> {
        let rows = sqlx::query!(
            "SELECT id, owner_id, name, removed_at, created_at FROM game_groups
             WHERE ($1::uuid IS NULL OR owner_id = $1) AND ($2::uuid IS NULL OR id = $2)",
            owner.map(|o| o.0),
            id.map(|i| i.0)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        let ids: Vec<uuid::Uuid> = rows.iter().map(|r| r.id).collect();
        let mut members: HashMap<uuid::Uuid, Vec<CompanionId>> = HashMap::new();
        for m in sqlx::query!(
            "SELECT group_id, companion_id FROM game_group_members
             WHERE group_id = ANY($1) ORDER BY group_id, position",
            &ids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        {
            members
                .entry(m.group_id)
                .or_default()
                .push(CompanionId(m.companion_id));
        }
        Ok(rows
            .into_iter()
            .map(|r| GameGroup {
                id: GameGroupId(r.id),
                owner: UserId(r.owner_id),
                name: r.name,
                members: members.remove(&r.id).unwrap_or_default(),
                removed_at: r.removed_at,
                created_at: r.created_at,
            })
            .collect())
    }

    pub(crate) async fn pg_game_groups(&self, owner: UserId) -> Res<Vec<GameGroup>> {
        self.groups_where(Some(owner), None).await
    }

    pub(crate) async fn pg_game_group(&self, id: GameGroupId) -> Res<Option<GameGroup>> {
        Ok(self.groups_where(None, Some(id)).await?.pop())
    }

    pub(crate) async fn pg_upsert_game_group(&self, g: &GameGroup) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO game_groups (id, owner_id, name, removed_at, created_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, removed_at = EXCLUDED.removed_at",
            g.id.0,
            g.owner.0,
            g.name,
            g.removed_at,
            g.created_at
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        sqlx::query!("DELETE FROM game_group_members WHERE group_id = $1", g.id.0)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        let members: Vec<uuid::Uuid> = g.members.iter().map(|m| m.0).collect();
        sqlx::query!(
            "INSERT INTO game_group_members (group_id, companion_id, position)
             SELECT $1, m, (n - 1)::integer FROM unnest($2::uuid[]) WITH ORDINALITY AS u (m, n)",
            g.id.0,
            &members
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        tx.commit().await.map_err(be)
    }

    /// The rules of `app::window_amounts` in SQL: the effective events of the items with an
    /// event in `[from, to)`, forward moves measured over all their events, summed per kind
    /// for the events inside the window.
    pub(crate) async fn pg_window_amounts(
        &self,
        user: UserId,
        from: Timestamp,
        to: Timestamp,
    ) -> Res<Vec<KindAmounts>> {
        let rows = sqlx::query!(
            r#"WITH touched AS (
                 SELECT item_id FROM progress_events
                 WHERE user_id = $1 AND occurred_at >= $2 AND occurred_at < $3
                 UNION
                 SELECT item_id FROM library_entries
                 WHERE user_id = $1 AND completed_at >= $2 AND completed_at < $3
               ), live AS (
                 SELECT x.id, x.item_id, x.occurred_at, x.status_change, c.kind, c.attributes,
                        x.progress ->> 'type' AS t,
                        CASE WHEN x.progress ->> 'type' = 'paged'
                             THEN jsonb_u64(x.progress -> 'page') END AS page,
                        CASE WHEN x.progress ->> 'type' = 'position'
                             THEN jsonb_u64(x.progress -> 'seconds') END AS pos,
                        s.duration_minutes AS duration
                 FROM progress_events x
                 JOIN touched USING (item_id)
                 JOIN catalog_items c ON c.id = x.item_id
                 LEFT JOIN sessions s
                   ON x.progress ->> 'type' = 'session' AND s.owner_id = x.user_id
                  AND s.id = (CASE WHEN x.progress ->> 'type' = 'session'
                                   THEN x.progress ->> 'session_id' END)::uuid
                 WHERE x.user_id = $1 AND x.undoes IS NULL
                   AND NOT EXISTS (SELECT 1 FROM progress_events u WHERE u.undoes = x.id)
                 UNION ALL
                 -- LB-14: the completion no event made is one completion on its day (one
                 -- without a day is in no window, LB-15)
                 SELECT e.id, e.item_id, e.completed_at, jsonb_build_object('to', 'completed'),
                        c.kind, c.attributes, 'status', NULL, NULL, NULL
                 FROM library_entries e
                 JOIN touched USING (item_id)
                 JOIN catalog_items c ON c.id = e.item_id
                 WHERE e.user_id = $1 AND e.completed_at IS NOT NULL
               ), moves AS (
                 SELECT l.*,
                        COALESCE(lag(page) OVER (PARTITION BY item_id, page IS NULL
                                                 ORDER BY occurred_at, id), 0) AS prev_page,
                        COALESCE(lag(pos) OVER (PARTITION BY item_id, pos IS NULL
                                                ORDER BY occurred_at, id), 0) AS prev_pos
                 FROM live l
               )
               SELECT kind AS "kind!",
                      COUNT(*) FILTER (WHERE t <> 'status') AS "events?",
                      COUNT(*) FILTER (WHERE status_change ->> 'to' = 'completed'
                                        AND t <> 'rewatch_stop') AS "completions?",
                      COUNT(*) FILTER (WHERE t = 'finished') AS "finished?",
                      COUNT(*) FILTER (WHERE t = 'episode') AS "episodes?",
                      COALESCE(SUM(GREATEST(page - prev_page, 0)), 0)::bigint AS "pages?",
                      COUNT(*) FILTER (WHERE t = 'session') AS "plays?",
                      COALESCE(SUM(CASE t
                            WHEN 'finished'
                              THEN COALESCE(jsonb_u64(attributes -> 'runtime_minutes'), 0) * 60
                            WHEN 'episode'
                              THEN COALESCE(jsonb_u64(attributes -> 'episode_runtime_minutes'),
                                            0) * 60
                            WHEN 'position' THEN GREATEST(pos - prev_pos, 0)
                            WHEN 'session' THEN COALESCE(duration, 0)::bigint * 60
                            ELSE 0 END), 0)::bigint AS "seconds?"
               FROM moves
               WHERE occurred_at >= $2 AND occurred_at < $3
               GROUP BY kind
               ORDER BY kind COLLATE "C""#,
            user.0,
            from,
            to
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        rows.into_iter()
            .map(|r| {
                Ok(KindAmounts {
                    kind: KindId::new(&r.kind).map_err(|_| corrupt("kind", &r.kind))?,
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
