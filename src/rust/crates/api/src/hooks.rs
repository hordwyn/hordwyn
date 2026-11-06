//! Inbound integrations (M14): managing integrations and the watched webhook.

use axum::http::StatusCode;
use contracts::{IntegrationCreatedDto, IntegrationDto, WatchedRecordedDto};
use domain::user::Actor;
use integrations::{Integration, IntegrationError, Integrations, WatchedEvent};

use crate::error::ApiResult;
use crate::ops::parse_id;
use crate::{ApiError, ApiState};

/// Path of the webhook under `/api/v1`.
pub const WATCHED_PATH: &str = "/hooks/watched";

fn api(s: &ApiState) -> ApiResult<&Integrations> {
    s.integrations.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "integrations are not enabled",
        )
    })
}

impl From<IntegrationError> for ApiError {
    fn from(e: IntegrationError) -> Self {
        match e {
            IntegrationError::Unauthorized => Self::unauthorized(),
            IntegrationError::Invalid(m) => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", m)
            }
            IntegrationError::NotFound => {
                Self::new(StatusCode::NOT_FOUND, "not_found", "integration not found")
            }
            IntegrationError::App(e) => e.into(),
            IntegrationError::Port(e) => {
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", e.0)
            }
        }
    }
}

fn dto(i: &Integration) -> IntegrationDto {
    IntegrationDto {
        id: i.id.to_string(),
        name: i.name.clone(),
        created_at: i.created_at.to_rfc3339(),
        last_seen_at: i.last_seen_at.map(|t| t.to_rfc3339()),
        last_error: i.last_error.clone(),
    }
}

/// # Errors
/// Integrations disabled, storage errors.
pub async fn list(s: &ApiState, a: Actor) -> ApiResult<Vec<IntegrationDto>> {
    Ok(api(s)?.list(a).await?.iter().map(dto).collect())
}

/// # Errors
/// Integrations disabled, invalid name.
pub async fn create(s: &ApiState, a: Actor, name: &str) -> ApiResult<IntegrationCreatedDto> {
    let (i, token) = api(s)?.create(a, name).await?;
    Ok(IntegrationCreatedDto {
        integration: dto(&i),
        token,
        webhook_url: format!(
            "{}/api/v1{WATCHED_PATH}",
            s.public_url.trim_end_matches('/')
        ),
    })
}

/// # Errors
/// Integrations disabled, unknown integration.
pub async fn delete(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    Ok(api(s)?.delete(a, parse_id(id, |u| u)?).await?)
}

/// # Errors
/// Unknown token, invalid event.
pub async fn watched(s: &ApiState, token: &str, e: &WatchedEvent) -> ApiResult<WatchedRecordedDto> {
    let r = api(s)?.receive(token, e).await?;
    Ok(WatchedRecordedDto {
        item_id: r.item_id.0.to_string(),
        event_id: r.event_id.to_string(),
    })
}
