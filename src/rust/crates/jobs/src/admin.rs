//! What Admin > Storage and jobs shows (addendum-1 3.4, OD-14).

use app::ports::Lane;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{PgQueue, QueueError, lane_from};

/// Jobs per lane and state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneCounts {
    pub lane: Lane,
    pub queued: u64,
    pub running: u64,
    pub dead: u64,
}

/// A job that used up its attempts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadJob {
    pub id: Uuid,
    pub lane: Lane,
    pub kind: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronEntry {
    pub name: String,
    pub schedule: String,
    pub kind: String,
    pub next_run_at: DateTime<Utc>,
}

impl PgQueue {
    /// Counts per lane (every lane, also empty ones).
    ///
    /// # Errors
    /// Database errors.
    pub async fn lane_counts(&self) -> Result<Vec<LaneCounts>, QueueError> {
        let rows = sqlx::query!(
            r#"SELECT lane, state, COUNT(*) AS "n!" FROM jobs
               WHERE state IN ('queued', 'running', 'dead') GROUP BY lane, state"#
        )
        .fetch_all(self.pool())
        .await?;
        let lanes = [
            Lane::Interactive,
            Lane::UserImport,
            Lane::BackgroundRefresh,
            Lane::Prefetch,
        ];
        Ok(lanes
            .into_iter()
            .map(|lane| {
                let n = |state: &str| {
                    rows.iter()
                        .filter(|r| lane_from(r.lane) == lane && r.state == state)
                        .map(|r| u64::try_from(r.n).unwrap_or(0))
                        .sum()
                };
                LaneCounts {
                    lane,
                    queued: n("queued"),
                    running: n("running"),
                    dead: n("dead"),
                }
            })
            .collect())
    }

    /// Dead jobs, most recent first.
    ///
    /// # Errors
    /// Database errors.
    pub async fn dead_jobs(&self, limit: u32) -> Result<Vec<DeadJob>, QueueError> {
        Ok(sqlx::query!(
            "SELECT id, lane, kind, attempts, last_error, updated_at FROM jobs
             WHERE state = 'dead' ORDER BY updated_at DESC, id DESC LIMIT $1",
            i64::from(limit)
        )
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(|r| DeadJob {
            id: r.id,
            lane: lane_from(r.lane),
            kind: r.kind,
            attempts: r.attempts,
            last_error: r.last_error,
            updated_at: r.updated_at,
        })
        .collect())
    }

    /// Queue a dead job again (attempts reset). `false` if it is not dead.
    ///
    /// # Errors
    /// Database errors.
    pub async fn retry_dead(&self, id: Uuid) -> Result<bool, QueueError> {
        Ok(sqlx::query!(
            "UPDATE jobs SET state = 'queued', attempts = 0, run_at = now(), locked_until = NULL,
                 updated_at = now()
             WHERE id = $1 AND state = 'dead'",
            id
        )
        .execute(self.pool())
        .await?
        .rows_affected()
            == 1)
    }

    /// Delete a dead job. `false` if it is not dead.
    ///
    /// # Errors
    /// Database errors.
    pub async fn discard_dead(&self, id: Uuid) -> Result<bool, QueueError> {
        Ok(
            sqlx::query!("DELETE FROM jobs WHERE id = $1 AND state = 'dead'", id)
                .execute(self.pool())
                .await?
                .rows_affected()
                == 1,
        )
    }

    /// # Errors
    /// Database errors.
    pub async fn cron_list(&self) -> Result<Vec<CronEntry>, QueueError> {
        Ok(sqlx::query!(
            "SELECT name, schedule, kind, next_run_at FROM cron_schedules ORDER BY next_run_at, name"
        )
        .fetch_all(self.pool())
        .await?
        .into_iter()
        .map(|r| CronEntry {
            name: r.name,
            schedule: r.schedule,
            kind: r.kind,
            next_run_at: r.next_run_at,
        })
        .collect())
    }
}
