//! Inbound integrations (M14): managing integrations and the watched webhook. Calls are
//! attributed to their integration once the token is known, so a refused body (422) keeps
//! its offending field and its redacted body on the sender's card (design.md 6.9, R-38); a
//! 401 has no known token and is attributed to nothing.

use axum::http::StatusCode;
use contracts::{
    IntegrationCreatedDto, IntegrationDto, IntegrationTestDto, RecordedItemDto, WatchedRecordedDto,
};
use domain::user::Actor;
use integrations::{Integration, IntegrationError, Integrations, LastItem};

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
            IntegrationError::Refused { field, message } => {
                let e = Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", message);
                match field {
                    Some(f) => e.with_field(f),
                    None => e,
                }
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

fn item(i: &LastItem) -> RecordedItemDto {
    RecordedItemDto {
        id: i.id.0.to_string(),
        title: i.title.clone(),
    }
}

fn dto(i: &Integration) -> IntegrationDto {
    IntegrationDto {
        id: i.id.to_string(),
        name: i.name.clone(),
        created_at: i.created_at.to_rfc3339(),
        last_seen_at: i.last_seen_at.map(|t| t.to_rfc3339()),
        last_error: i.last_error.clone(),
        last_status: i.last_status,
        last_error_field: i.last_error_field.clone(),
        last_payload: i.last_payload.clone(),
        last_item: i.last_item.as_ref().map(item),
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

/// "Send a test call from here": validate and match a sample body, record nothing.
///
/// # Errors
/// Integrations disabled, not the caller's integration, a refused sample (422 with `field`).
pub async fn test(s: &ApiState, a: Actor, id: &str, body: &[u8]) -> ApiResult<IntegrationTestDto> {
    let t = api(s)?.test_call(a, parse_id(id, |u| u)?, body).await?;
    Ok(IntegrationTestDto {
        test: true,
        matched: t.matched.as_ref().map(item),
        title: t.title,
    })
}

/// # Errors
/// Unknown token, a refused body (422 with `field`).
pub async fn watched(s: &ApiState, token: &str, body: &[u8]) -> ApiResult<WatchedRecordedDto> {
    let r = api(s)?.receive_body(token, body).await?;
    Ok(WatchedRecordedDto {
        item_id: r.item_id.0.to_string(),
        event_id: r.event_id.to_string(),
    })
}
