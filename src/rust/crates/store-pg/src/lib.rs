//! Postgres adapter for the repository ports, migrations, outbox and per-user change feed.
//!
//! Every write runs in one transaction together with its `outbox` row and its per-user
//! `user_changes` rows (D-12, D-13), so a rolled-back write leaves no trace.
#![deny(unsafe_code)]

mod conv;
mod repos;
pub mod testing;
mod tx;

use app::ports::{RepoError, RepoResult};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

pub use repos::match_log::CatalogMatch;
pub use tx::Change;

/// Embedded migrations (`crates/store-pg/migrations`), shared by every crate that uses the
/// database (jobs, search fallback, ...).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Postgres implementation of every repository port.
#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Connect a pool.
///
/// # Errors
/// If the database is unreachable.
pub async fn connect(url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
}

/// Like [`connect`], but retries while the server is not reachable yet (container start-up:
/// DNS names and the listening socket may appear a moment after this process starts), with
/// backoff up to 5 s between attempts, for at most `patience`. Errors from a reachable server
/// (wrong password, missing database) are returned at once.
///
/// # Errors
/// The last error once `patience` is used up, or the first non-network error.
pub async fn connect_with_retry(
    url: &str,
    max_connections: u32,
    patience: std::time::Duration,
) -> Result<PgPool, sqlx::Error> {
    let started = std::time::Instant::now();
    let mut pause = std::time::Duration::from_millis(500);
    loop {
        // one attempt waits at most 3 s (sqlx's pool would wait 30 s for a refused socket)
        let attempt = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(std::time::Duration::from_secs(3).min(patience))
            .connect(url)
            .await;
        match attempt {
            Ok(pool) => return Ok(pool),
            Err(e @ (sqlx::Error::Io(_) | sqlx::Error::PoolTimedOut | sqlx::Error::Tls(_)))
                if started.elapsed() + pause < patience =>
            {
                tracing::warn!(error = %e, retry_in_ms = pause.as_millis(), "database not reachable yet");
                tokio::time::sleep(pause).await;
                pause = (pause * 2).min(std::time::Duration::from_secs(5));
            }
            Err(e) => return Err(e),
        }
    }
}

/// One migration of the embedded set that the database has not applied yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMigration {
    pub version: i64,
    /// The file name without the version (`secrets key backup`).
    pub description: String,
}

/// The migrations [`migrate`] would apply, in order, without changing anything (not even
/// the bookkeeping table of a database that was never migrated): the dry run of
/// `cli migrate --dry-run` and the list the server logs before it migrates (OPS-6).
///
/// # Errors
/// Database errors.
pub async fn pending_migrations(pool: &PgPool) -> Result<Vec<PendingMigration>, sqlx::Error> {
    // read-only on purpose: sqlx's own listing first creates its bookkeeping table
    let applied: Vec<i64> =
        match sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success")
            .fetch_all(pool)
            .await
        {
            Ok(v) => v,
            // undefined_table: this database was never migrated
            Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("42P01") => Vec::new(),
            Err(e) => return Err(e),
        };
    Ok(MIGRATOR
        .iter()
        .filter(|m| !m.migration_type.is_down_migration() && !applied.contains(&m.version))
        .map(|m| PendingMigration {
            version: m.version,
            description: m.description.to_string(),
        })
        .collect())
}

/// Apply pending migrations, naming each in the log before it runs.
///
/// # Errors
/// If a migration fails.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    match pending_migrations(pool).await {
        Ok(pending) => {
            for m in &pending {
                tracing::info!(version = m.version, name = %m.description, "applying database migration");
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not list the pending database migrations"),
    }
    MIGRATOR.run(pool).await
}

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn be(e: sqlx::Error) -> RepoError {
    if let sqlx::Error::Database(db) = &e
        && db.is_unique_violation()
    {
        return RepoError::Duplicate(db.message().to_string());
    }
    tracing::warn!(error = %e, "database error");
    RepoError::Backend(e.to_string())
}

pub(crate) type Res<T> = RepoResult<T>;

#[async_trait::async_trait]
impl app::ports::ChangeFeed for PgStore {
    async fn changes_since(
        &self,
        user: domain::ids::UserId,
        cursor: i64,
        limit: i64,
    ) -> Res<Vec<app::ports::ChangeRef>> {
        Ok(PgStore::changes_since(self, user, cursor, limit)
            .await?
            .into_iter()
            .map(|c| app::ports::ChangeRef {
                seq: c.seq,
                entity: c.entity,
                id: c.entity_id,
                op: c.op,
            })
            .collect())
    }
}
