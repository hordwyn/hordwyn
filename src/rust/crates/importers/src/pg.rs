//! Postgres adapter for [`ImportStore`] (store-pg migrations `0008_imports.sql`,
//! `0022_imports_page.sql`).

use app::ports::PortError;
use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{ImportRecord, ImportStore};

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("imports db: {e}"))
}

#[derive(Clone)]
pub struct PgImportStore {
    pool: PgPool,
}

impl PgImportStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ImportStore for PgImportStore {
    async fn insert(&self, r: &ImportRecord) -> Result<(), PortError> {
        self.save(r).await
    }

    async fn get(&self, id: Uuid) -> Result<Option<ImportRecord>, PortError> {
        let v = sqlx::query_scalar!("SELECT record FROM imports WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?;
        v.map(|v| serde_json::from_value(v).map_err(|e| PortError(e.to_string())))
            .transpose()
    }

    async fn save(&self, r: &ImportRecord) -> Result<(), PortError> {
        let v = serde_json::to_value(r).map_err(|e| PortError(e.to_string()))?;
        sqlx::query!(
            "INSERT INTO imports (id, user_id, record, updated_at) VALUES ($1, $2, $3, $4)
             ON CONFLICT (id) DO UPDATE SET record = $3, updated_at = $4",
            r.id,
            r.user_id,
            v,
            r.updated_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn list(&self, user: Uuid, limit: u32) -> Result<Vec<ImportRecord>, PortError> {
        let rows = sqlx::query_scalar!(
            "SELECT record FROM imports WHERE user_id = $1 ORDER BY id DESC LIMIT $2",
            user,
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?;
        rows.into_iter()
            .map(|v| serde_json::from_value(v).map_err(|e| PortError(e.to_string())))
            .collect()
    }

    async fn request_cancel(&self, id: Uuid) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE imports SET cancel_requested = true WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn cancel_requested(&self, id: Uuid) -> Result<bool, PortError> {
        let v = sqlx::query_scalar!("SELECT cancel_requested FROM imports WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?;
        Ok(v.unwrap_or(false))
    }

    async fn item_for(
        &self,
        user: Uuid,
        format: &str,
        source_id: &str,
    ) -> Result<Option<Uuid>, PortError> {
        sqlx::query_scalar!(
            "SELECT item_id FROM import_rows WHERE user_id = $1 AND format = $2 AND source_id = $3",
            user,
            format,
            source_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)
    }

    async fn set_item_for(
        &self,
        user: Uuid,
        format: &str,
        source_id: &str,
        item: Uuid,
    ) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO import_rows (user_id, format, source_id, item_id) VALUES ($1, $2, $3, $4)
             ON CONFLICT (user_id, format, source_id) DO UPDATE SET item_id = $4",
            user,
            format,
            source_id,
            item
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }
}
