//! Postgres adapter for [`IntegrationStore`] (store-pg migration `0001_v1_baseline.sql`).

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::{ItemId, UserId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{CallRecord, Integration, IntegrationStore, LastItem};

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
    last_status: Option<i16>,
    last_error_field: Option<String>,
    last_payload: Option<String>,
    last_item_id: Option<Uuid>,
    last_item_title: Option<String>,
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
            last_status: r.last_status.and_then(|s| u16::try_from(s).ok()),
            last_error_field: r.last_error_field,
            last_payload: r.last_payload,
            last_item: r
                .last_item_id
                .zip(r.last_item_title)
                .map(|(id, title)| LastItem {
                    id: ItemId(id),
                    title,
                }),
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
            "SELECT id, user_id, name, token_hash, created_at, last_seen_at, last_error,
                    last_status, last_error_field, last_payload, last_item_id, last_item_title
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
            "SELECT id, user_id, name, token_hash, created_at, last_seen_at, last_error,
                    last_status, last_error_field, last_payload, last_item_id, last_item_title
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

    async fn touch(&self, id: Uuid, call: &CallRecord) -> Result<(), PortError> {
        let status = i16::try_from(call.status).unwrap_or(i16::MAX);
        sqlx::query!(
            "UPDATE integrations SET last_seen_at = $2, last_error = $3, last_status = $4,
                    last_error_field = $5, last_payload = $6,
                    last_item_id = COALESCE($7, last_item_id),
                    last_item_title = COALESCE($8, last_item_title)
             WHERE id = $1",
            id,
            call.at,
            call.error,
            status,
            call.field,
            call.payload,
            call.item.as_ref().map(|i| i.id.0),
            call.item.as_ref().map(|i| i.title.clone()),
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }
}
