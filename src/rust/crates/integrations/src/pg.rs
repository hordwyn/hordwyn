//! Postgres adapter for [`IntegrationStore`] (store-pg migration `0012_integrations.sql`).

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{Integration, IntegrationStore};

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("integrations db: {e}"))
}

#[derive(Clone)]
pub struct PgIntegrationStore {
    pool: PgPool,
}

impl PgIntegrationStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

struct Row {
    id: Uuid,
    user_id: Uuid,
    name: String,
    token_hash: String,
    created_at: DateTime<Utc>,
    last_seen_at: Option<DateTime<Utc>>,
    last_error: Option<String>,
}

impl From<Row> for Integration {
    fn from(r: Row) -> Self {
        Self {
            id: r.id,
            user: UserId(r.user_id),
            name: r.name,
            token_hash: r.token_hash,
            created_at: r.created_at,
            last_seen_at: r.last_seen_at,
            last_error: r.last_error,
        }
    }
}

#[async_trait]
impl IntegrationStore for PgIntegrationStore {
    async fn insert(&self, i: &Integration) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO integrations (id, user_id, name, token_hash, created_at)
             VALUES ($1, $2, $3, $4, $5)",
            i.id,
            i.user.0,
            i.name,
            i.token_hash,
            i.created_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn by_token_hash(&self, hash: &str) -> Result<Option<Integration>, PortError> {
        Ok(sqlx::query_as!(
            Row,
            "SELECT id, user_id, name, token_hash, created_at, last_seen_at, last_error
             FROM integrations WHERE token_hash = $1",
            hash
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(Integration::from))
    }

    async fn by_user(&self, user: UserId) -> Result<Vec<Integration>, PortError> {
        Ok(sqlx::query_as!(
            Row,
            "SELECT id, user_id, name, token_hash, created_at, last_seen_at, last_error
             FROM integrations WHERE user_id = $1 ORDER BY created_at, id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(Integration::from)
        .collect())
    }

    async fn delete(&self, id: Uuid) -> Result<(), PortError> {
        sqlx::query!("DELETE FROM integrations WHERE id = $1", id)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }

    async fn touch(
        &self,
        id: Uuid,
        at: DateTime<Utc>,
        error: Option<String>,
    ) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE integrations SET last_seen_at = $2, last_error = $3 WHERE id = $1",
            id,
            at,
            error
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }
}
