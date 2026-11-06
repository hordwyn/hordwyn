//! Postgres index of the HTTP cache (store-pg migration `0006_http_cache.sql`). Bodies live in
//! the budgeted `BlobStore`; this table only holds validators and freshness.

use app::ports::PortError;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::cache::{CacheEntry, HttpCacheIndex};

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
}
