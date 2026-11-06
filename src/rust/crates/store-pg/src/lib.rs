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

/// Apply pending migrations.
///
/// # Errors
/// If a migration fails.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
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
