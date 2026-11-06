use std::time::Duration;

use app::ports::{JobQueue, Lane, NewJob, PortError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::{lane_from, lane_num};

#[derive(Debug, Error)]
pub enum QueueError {
    #[error("database: {0}")]
    Db(#[from] sqlx::Error),
    #[error("invalid cron schedule `{0}`: {1}")]
    Cron(String, String),
}

/// A claimed job.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub id: Uuid,
    pub lane: Lane,
    pub kind: String,
    pub payload: serde_json::Value,
    pub dedupe_key: Option<String>,
    /// Including the current attempt.
    pub attempts: i32,
    pub max_attempts: i32,
}

#[derive(Clone)]
pub struct PgQueue {
    pool: PgPool,
}

/// Delay before retry number `attempt` (1-based): 2 s doubling, capped at one hour, with
/// +-20 % jitter so failed bursts do not retry in lockstep.
#[must_use]
pub fn backoff(attempt: i32) -> Duration {
    let exp = u32::try_from(attempt.clamp(1, 12) - 1).unwrap_or(0);
    let base_ms = (2_000u64 << exp).min(3_600_000);
    let jitter = 0.8 + fastrand::f64() * 0.4;
    // Truncation is fine: millisecond precision is plenty for retry delays.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    Duration::from_millis((base_ms as f64 * jitter) as u64)
}

impl PgQueue {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Enqueue a job. If a queued or running job has the same dedupe key, its id is returned
    /// and nothing is inserted.
    ///
    /// # Errors
    /// Database errors.
    pub async fn push(&self, job: NewJob) -> Result<Uuid, QueueError> {
        loop {
            let id = Uuid::now_v7();
            let inserted = sqlx::query_scalar!(
                "INSERT INTO jobs (id, lane, kind, payload, dedupe_key, priority, run_at)
                 VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, now()))
                 ON CONFLICT (dedupe_key) WHERE state IN ('queued', 'running') DO NOTHING
                 RETURNING id",
                id,
                lane_num(job.lane),
                job.kind,
                job.payload,
                job.dedupe_key,
                job.priority,
                job.run_at
            )
            .fetch_optional(&self.pool)
            .await?;
            if let Some(id) = inserted {
                return Ok(id);
            }
            let existing = sqlx::query_scalar!(
                "SELECT id FROM jobs WHERE dedupe_key = $1 AND state IN ('queued', 'running')",
                job.dedupe_key
            )
            .fetch_optional(&self.pool)
            .await?;
            // The holder may have finished between the two statements; then insert again.
            if let Some(id) = existing {
                return Ok(id);
            }
        }
    }

    /// Claim the most important ready job in `lanes`, holding it for `lease`.
    /// Jobs whose lease expired (crashed worker) are claimable again.
    ///
    /// # Errors
    /// Database errors.
    pub async fn claim(&self, lanes: &[Lane], lease: Duration) -> Result<Option<Job>, QueueError> {
        let lanes: Vec<i16> = lanes.iter().map(|l| lane_num(*l)).collect();
        let lease_ms = i64::try_from(lease.as_millis()).unwrap_or(i64::MAX);
        let row = sqlx::query!(
            "UPDATE jobs SET state = 'running', attempts = attempts + 1, updated_at = now(),
                             locked_until = now() + make_interval(secs => $2::bigint / 1000.0)
             WHERE id = (
                 SELECT id FROM jobs
                 WHERE lane = ANY($1)
                   AND ((state = 'queued' AND run_at <= now())
                        OR (state = 'running' AND locked_until < now()))
                 ORDER BY lane, priority DESC, run_at, id
                 LIMIT 1
                 FOR UPDATE SKIP LOCKED)
             RETURNING id, lane, kind, payload, dedupe_key, attempts, max_attempts",
            &lanes,
            lease_ms
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| Job {
            id: r.id,
            lane: lane_from(r.lane),
            kind: r.kind,
            payload: r.payload,
            dedupe_key: r.dedupe_key,
            attempts: r.attempts,
            max_attempts: r.max_attempts,
        }))
    }

    /// Extend the lease of a running job (heartbeat of a long job, so no other worker
    /// claims it meanwhile).
    ///
    /// # Errors
    /// Database errors.
    pub async fn extend(&self, id: Uuid, lease: Duration) -> Result<(), QueueError> {
        let lease_ms = i64::try_from(lease.as_millis()).unwrap_or(i64::MAX);
        sqlx::query!(
            "UPDATE jobs SET locked_until = now() + make_interval(secs => $2::bigint / 1000.0)
             WHERE id = $1 AND state = 'running'",
            id,
            lease_ms
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark a claimed job as done.
    ///
    /// # Errors
    /// Database errors.
    pub async fn complete(&self, id: Uuid) -> Result<(), QueueError> {
        sqlx::query!(
            "UPDATE jobs SET state = 'done', locked_until = NULL, updated_at = now() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a failure: retry after [`backoff`], or mark the job dead after `max_attempts`.
    ///
    /// # Errors
    /// Database errors.
    pub async fn fail(&self, job: &Job, error: &str) -> Result<(), QueueError> {
        if job.attempts >= job.max_attempts {
            tracing::warn!(job = %job.id, kind = %job.kind, error, "job is dead");
            sqlx::query!(
                "UPDATE jobs SET state = 'dead', last_error = $2, locked_until = NULL, updated_at = now()
                 WHERE id = $1",
                job.id,
                error
            )
            .execute(&self.pool)
            .await?;
        } else {
            let delay_ms = i64::try_from(backoff(job.attempts).as_millis()).unwrap_or(i64::MAX);
            sqlx::query!(
                "UPDATE jobs SET state = 'queued', last_error = $2, locked_until = NULL, updated_at = now(),
                                 run_at = now() + make_interval(secs => $3::bigint / 1000.0)
                 WHERE id = $1",
                job.id,
                error,
                delay_ms
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Release a claimed job without counting the attempt (used on shutdown).
    ///
    /// # Errors
    /// Database errors.
    pub async fn release(&self, id: Uuid) -> Result<(), QueueError> {
        sqlx::query!(
            "UPDATE jobs SET state = 'queued', attempts = greatest(attempts - 1, 0),
                             locked_until = NULL, updated_at = now()
             WHERE id = $1 AND state = 'running'",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Create or replace a cron schedule (5-field cron, UTC).
    ///
    /// # Errors
    /// Invalid schedule or database errors.
    pub async fn schedule(
        &self,
        name: &str,
        cron: &str,
        lane: Lane,
        kind: &str,
        payload: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<(), QueueError> {
        let next = next_run(cron, now)?;
        sqlx::query!(
            "INSERT INTO cron_schedules (name, schedule, lane, kind, payload, next_run_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (name) DO UPDATE SET schedule = EXCLUDED.schedule, lane = EXCLUDED.lane,
                 kind = EXCLUDED.kind, payload = EXCLUDED.payload, next_run_at = EXCLUDED.next_run_at",
            name,
            cron,
            lane_num(lane),
            kind,
            payload,
            next
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Enqueue every schedule due at `now` (once per occurrence, deduped by name + time) and
    /// advance it. Safe to call from several workers concurrently. Returns jobs enqueued.
    ///
    /// # Errors
    /// Database errors or a schedule that no longer parses.
    pub async fn tick_cron(&self, now: DateTime<Utc>) -> Result<usize, QueueError> {
        let mut tx = self.pool.begin().await?;
        let due = sqlx::query!(
            "SELECT name, schedule, lane, kind, payload, next_run_at FROM cron_schedules
             WHERE next_run_at <= $1 FOR UPDATE SKIP LOCKED",
            now
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut n = 0;
        for s in due {
            let key = format!("cron:{}:{}", s.name, s.next_run_at.timestamp());
            let inserted = sqlx::query_scalar!(
                "INSERT INTO jobs (id, lane, kind, payload, dedupe_key) VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (dedupe_key) WHERE state IN ('queued', 'running') DO NOTHING
                 RETURNING id",
                Uuid::now_v7(),
                s.lane,
                s.kind,
                s.payload,
                key
            )
            .fetch_optional(&mut *tx)
            .await?;
            n += usize::from(inserted.is_some());
            let next = next_run(&s.schedule, now)?;
            sqlx::query!(
                "UPDATE cron_schedules SET next_run_at = $2 WHERE name = $1",
                s.name,
                next
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(n)
    }
}

fn next_run(cron: &str, after: DateTime<Utc>) -> Result<DateTime<Utc>, QueueError> {
    let parsed: croner::Cron = cron
        .parse()
        .map_err(|e: croner::errors::CronError| QueueError::Cron(cron.into(), e.to_string()))?;
    parsed
        .find_next_occurrence(&after, false)
        .map_err(|e| QueueError::Cron(cron.into(), e.to_string()))
}

#[async_trait]
impl JobQueue for PgQueue {
    async fn enqueue(&self, job: NewJob) -> Result<Uuid, PortError> {
        self.push(job).await.map_err(|e| PortError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_is_capped() {
        for _ in 0..50 {
            let b1 = backoff(1).as_millis();
            assert!((1_600..=2_400).contains(&b1), "{b1}");
            let b3 = backoff(3).as_millis();
            assert!((6_400..=9_600).contains(&b3), "{b3}");
            assert!(backoff(40).as_millis() <= 4_320_000);
        }
    }

    #[test]
    fn cron_next_run() {
        let t = DateTime::parse_from_rfc3339("2026-01-01T10:00:30Z")
            .unwrap()
            .with_timezone(&Utc);
        let n = next_run("*/5 * * * *", t).unwrap();
        assert_eq!(n.to_rfc3339(), "2026-01-01T10:05:00+00:00");
        assert!(next_run("not cron", t).is_err());
    }
}
