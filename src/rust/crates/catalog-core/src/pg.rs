//! Postgres index of the HTTP cache (store-pg migration `0006_http_cache.sql`). Bodies live in
//! the budgeted `BlobStore`; this table only holds validators and freshness. Also the shared
//! data-source counters (`0024_admin_ops.sql`).

use std::collections::HashMap;

use app::ports::PortError;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::cache::{CacheEntry, HttpCacheIndex};
use crate::stats::{ConnectorStats, StatsMarks, StatsStore};

#[derive(Clone)]
pub struct PgCacheIndex {
    pool: PgPool,
}

impl PgCacheIndex {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("http cache db: {e}"))
}

#[async_trait]
impl HttpCacheIndex for PgCacheIndex {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>, PortError> {
        let row = sqlx::query_scalar!("SELECT entry FROM http_cache WHERE key = $1", key)
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?;
        row.map(|v| serde_json::from_value(v).map_err(|e| PortError(e.to_string())))
            .transpose()
    }

    async fn put(&self, key: &str, entry: CacheEntry) -> Result<(), PortError> {
        let v = serde_json::to_value(&entry).map_err(|e| PortError(e.to_string()))?;
        sqlx::query!(
            "INSERT INTO http_cache (key, entry, fresh_until) VALUES ($1, $2, $3)
             ON CONFLICT (key) DO UPDATE SET entry = $2, fresh_until = $3",
            key,
            v,
            entry.fresh_until
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn purge_stale_before(
        &self,
        before: chrono::DateTime<chrono::Utc>,
    ) -> Result<(usize, Vec<String>), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        let gone = sqlx::query_scalar!(
            r#"DELETE FROM http_cache WHERE fresh_until < $1 RETURNING entry->>'body_hash' AS "h!""#,
            before
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(pe)?;
        let n = gone.len();
        // a second statement: a data-modifying CTE's main query would still see the rows
        let orphans = sqlx::query_scalar!(
            r#"SELECT DISTINCT h AS "h!" FROM unnest($1::text[]) AS g(h)
               WHERE NOT EXISTS (SELECT 1 FROM http_cache c WHERE c.entry->>'body_hash' = g.h)"#,
            &gone
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(pe)?;
        tx.commit().await.map_err(pe)?;
        Ok((n, orphans))
    }
}

/// The counters of every process (migration `0024_admin_ops.sql`, DEC-247/326).
#[derive(Clone)]
pub struct PgSourceStats {
    pool: PgPool,
}

impl PgSourceStats {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Hourly rows kept: two days (the status reads 24 hours).
const KEEP_HOURS: i64 = 48;

#[async_trait]
impl StatsStore for PgSourceStats {
    async fn add(&self, deltas: &HashMap<String, ConnectorStats>) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        let mut newest = 0;
        for (id, c) in deltas {
            for (hour, (calls, limited)) in &c.hours {
                newest = newest.max(*hour);
                sqlx::query!(
                    "INSERT INTO source_stats_hours (connector, hour, calls, rate_limited)
                     VALUES ($1, $2, $3, $4)
                     ON CONFLICT (connector, hour) DO UPDATE
                     SET calls = source_stats_hours.calls + EXCLUDED.calls,
                         rate_limited = source_stats_hours.rate_limited + EXCLUDED.rate_limited",
                    id,
                    hour,
                    i64::try_from(*calls).unwrap_or(i64::MAX),
                    i64::try_from(*limited).unwrap_or(i64::MAX)
                )
                .execute(&mut *tx)
                .await
                .map_err(pe)?;
            }
            let m = &c.marks;
            // each mark keeps the newer value; a text or status goes with its time
            sqlx::query!(
                "INSERT INTO source_stats (connector, last_ok_at, last_error_at, last_error,
                     last_auth_status, last_auth_failure_at, last_rate_limited_at,
                     circuit_opened_at, circuit_open_until, key_changed_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                 ON CONFLICT (connector) DO UPDATE SET
                     last_ok_at = GREATEST(source_stats.last_ok_at, EXCLUDED.last_ok_at),
                     last_error = CASE WHEN EXCLUDED.last_error_at > source_stats.last_error_at
                         OR source_stats.last_error_at IS NULL
                         THEN COALESCE(EXCLUDED.last_error, source_stats.last_error)
                         ELSE source_stats.last_error END,
                     last_error_at = GREATEST(source_stats.last_error_at, EXCLUDED.last_error_at),
                     last_auth_status = CASE
                         WHEN EXCLUDED.last_auth_failure_at > source_stats.last_auth_failure_at
                           OR source_stats.last_auth_failure_at IS NULL
                         THEN COALESCE(EXCLUDED.last_auth_status, source_stats.last_auth_status)
                         ELSE source_stats.last_auth_status END,
                     last_auth_failure_at = GREATEST(source_stats.last_auth_failure_at,
                         EXCLUDED.last_auth_failure_at),
                     last_rate_limited_at = GREATEST(source_stats.last_rate_limited_at,
                         EXCLUDED.last_rate_limited_at),
                     circuit_open_until = CASE
                         WHEN EXCLUDED.circuit_opened_at > source_stats.circuit_opened_at
                           OR source_stats.circuit_opened_at IS NULL
                         THEN COALESCE(EXCLUDED.circuit_open_until, source_stats.circuit_open_until)
                         ELSE source_stats.circuit_open_until END,
                     circuit_opened_at = GREATEST(source_stats.circuit_opened_at,
                         EXCLUDED.circuit_opened_at),
                     key_changed_at = GREATEST(source_stats.key_changed_at, EXCLUDED.key_changed_at)",
                id,
                m.last_ok_at,
                m.last_error_at,
                m.last_error,
                m.last_auth_status.map(|s| i16::try_from(s).unwrap_or(i16::MAX)),
                m.last_auth_failure_at,
                m.last_rate_limited_at,
                m.circuit_opened_at,
                m.circuit_open_until,
                m.key_changed_at
            )
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        }
        if newest > 0 {
            sqlx::query!(
                "DELETE FROM source_stats_hours WHERE hour < $1",
                newest - KEEP_HOURS
            )
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        }
        tx.commit().await.map_err(pe)
    }

    async fn load(&self, since_hour: i64) -> Result<HashMap<String, ConnectorStats>, PortError> {
        let mut out: HashMap<String, ConnectorStats> = HashMap::new();
        let marks = sqlx::query!(
            "SELECT connector, last_ok_at, last_error_at, last_error, last_auth_status,
                    last_auth_failure_at, last_rate_limited_at, circuit_opened_at,
                    circuit_open_until, key_changed_at
             FROM source_stats"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?;
        for r in marks {
            out.entry(r.connector).or_default().marks = StatsMarks {
                last_ok_at: r.last_ok_at,
                last_error_at: r.last_error_at,
                last_error: r.last_error,
                last_auth_status: r.last_auth_status.and_then(|s| u16::try_from(s).ok()),
                last_auth_failure_at: r.last_auth_failure_at,
                last_rate_limited_at: r.last_rate_limited_at,
                circuit_opened_at: r.circuit_opened_at,
                circuit_open_until: r.circuit_open_until,
                key_changed_at: r.key_changed_at,
            };
        }
        let hours = sqlx::query!(
            "SELECT connector, hour, calls, rate_limited FROM source_stats_hours WHERE hour >= $1",
            since_hour
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?;
        for r in hours {
            out.entry(r.connector).or_default().hours.insert(
                r.hour,
                (
                    u64::try_from(r.calls).unwrap_or(0),
                    u64::try_from(r.rate_limited).unwrap_or(0),
                ),
            );
        }
        Ok(out)
    }
}

/// The pace of a source for every process that uses this database (store-pg migration
/// `0100`, table `source_pace`): each turn reserves the next free slot of the source in one
/// statement and waits until it starts. The row lock orders the callers, and the clock is
/// the database's, so the web role, a separate worker and the shell commands never send two
/// requests to one source closer together than the interval (review BOOK-8).
#[derive(Clone)]
pub struct PgPace {
    pool: PgPool,
}

impl PgPace {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Reserve the next slot of `source` and answer how long until it starts.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn reserve(
        &self,
        source: &str,
        every: std::time::Duration,
    ) -> Result<std::time::Duration, PortError> {
        let every = every.as_secs_f64();
        let wait = sqlx::query_scalar!(
            r#"INSERT INTO source_pace AS p (source, next_free)
               VALUES ($1, clock_timestamp() + make_interval(secs => $2))
               ON CONFLICT (source) DO UPDATE
                   SET next_free = GREATEST(p.next_free, clock_timestamp())
                                   + make_interval(secs => $2)
               RETURNING (EXTRACT(EPOCH FROM p.next_free - clock_timestamp()) - $2)::float8
                   AS "wait!""#,
            source,
            every
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pe)?;
        Ok(std::time::Duration::try_from_secs_f64(wait).unwrap_or_default())
    }
}

#[async_trait]
impl crate::http::SharedPace for PgPace {
    async fn turn(&self, source: &str, every: std::time::Duration) {
        match self.reserve(source, every).await {
            Ok(wait) => tokio::time::sleep(wait).await,
            // without the database nothing else works either; this client's own token
            // bucket still holds
            Err(e) => tracing::warn!(source, error = %e.0, "shared pace not available"),
        }
    }
}
