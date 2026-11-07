//! Authentication extractor: session cookie (browsers) or `Authorization: Bearer` (clients).

use axum::extract::FromRequestParts;
use axum::http::header::{AUTHORIZATION, COOKIE};
use axum::http::request::Parts;
use domain::user::Actor;

use crate::{ApiError, ApiState};

/// The signed-in caller.
#[derive(Debug, Clone, Copy)]
pub struct Authed(pub Actor);

/// Session token from the bearer header or the session cookie.
#[must_use]
pub fn session_token(parts: &Parts, cookie_name: &str) -> Option<String> {
    if let Some(v) = parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        && let Some(t) = v.strip_prefix("Bearer ")
    {
        return Some(t.trim().to_string());
    }
    cookie(parts, cookie_name)
}

/// Value of a cookie.
#[must_use]
pub fn cookie(parts: &Parts, name: &str) -> Option<String> {
    parts
        .headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_string())
}

impl FromRequestParts<ApiState> for Authed {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &ApiState) -> Result<Self, ApiError> {
        let token = session_token(parts, &state.naming.session_cookie)
            .ok_or_else(ApiError::unauthorized)?;
        state
            .auth
            .session(&token)
            .await?
            .map(Authed)
            .ok_or_else(ApiError::unauthorized)
    }
}

/// The signed-in caller and the session token (gated admin actions check its fresh proof).
#[derive(Debug, Clone)]
pub struct AuthedToken {
    pub actor: Actor,
    pub token: String,
}

impl FromRequestParts<ApiState> for AuthedToken {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &ApiState) -> Result<Self, ApiError> {
        let token = session_token(parts, &state.naming.session_cookie)
            .ok_or_else(ApiError::unauthorized)?;
        let actor = state
            .auth
            .session(&token)
            .await?
            .ok_or_else(ApiError::unauthorized)?;
        Ok(Self { actor, token })
    }
}

/// Session token from the bearer header or the session cookie of a header map.
#[must_use]
pub fn cookie_or_bearer(headers: &axum::http::HeaderMap, cookie_name: &str) -> Option<String> {
    if let Some(t) = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(t.trim().to_string());
    }
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, v)| *k == cookie_name && !v.is_empty())
        .map(|(_, v)| v.to_string())
}
