use app::ports::HttpRequest;
use catalog_core::{ConnectorError, Ctx};
use serde::de::DeserializeOwned;

/// GET `url` through the context's governed HTTP port and decode JSON.
///
/// # Errors
/// HTTP errors, 404 as `NotFound`, other non-2xx as `Status`, undecodable bodies as `Parse`.
pub async fn get_json<T: DeserializeOwned>(
    ctx: &Ctx,
    connector: &str,
    op: &str,
    url: &str,
    headers: Vec<(String, String)>,
) -> Result<T, ConnectorError> {
    Ok(get_json_with_headers(ctx, connector, op, url, headers)
        .await?
        .0)
}

/// [`get_json`] that also returns the answer's headers (`Retry-After` of an error object
/// in an HTTP 200 answer, WD-4).
///
/// # Errors
/// As [`get_json`].
pub async fn get_json_with_headers<T: DeserializeOwned>(
    ctx: &Ctx,
    connector: &str,
    op: &str,
    url: &str,
    headers: Vec<(String, String)>,
) -> Result<(T, Vec<(String, String)>), ConnectorError> {
    let resp = ctx
        .http
        .fetch(HttpRequest {
            connector: connector.into(),
            op: op.into(),
            method: "GET".into(),
            url: url.into(),
            headers,
            body: None,
            lane: ctx.lane,
            skip_auth_record: ctx.skip_auth_record(),
            user: ctx.user,
        })
        .await?;
    match resp.status {
        200..=299 => serde_json::from_slice(&resp.body)
            .map(|v| (v, resp.headers))
            .map_err(|e| ConnectorError::Parse(e.to_string())),
        404 => Err(ConnectorError::NotFound),
        status => Err(status_error(ctx, status)),
    }
}

/// Year from an ISO date prefix such as `1999-10-15`.
pub(crate) fn year_of(date: Option<&str>) -> Option<i32> {
    date.and_then(|d| d.get(..4)).and_then(|y| y.parse().ok())
}

pub(crate) fn date_of(date: Option<&str>) -> Option<chrono::NaiveDate> {
    date.and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

pub(crate) fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty())
}

/// POST `body` to `url` through the governed HTTP port and decode JSON.
///
/// # Errors
/// As [`get_json`].
pub async fn post_json<T: DeserializeOwned>(
    ctx: &Ctx,
    connector: &str,
    op: &str,
    url: &str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
) -> Result<T, ConnectorError> {
    let resp = ctx
        .http
        .fetch(HttpRequest {
            connector: connector.into(),
            op: op.into(),
            method: "POST".into(),
            url: url.into(),
            headers,
            body: Some(body),
            lane: ctx.lane,
            skip_auth_record: ctx.skip_auth_record(),
            user: ctx.user,
        })
        .await?;
    match resp.status {
        200..=299 => {
            serde_json::from_slice(&resp.body).map_err(|e| ConnectorError::Parse(e.to_string()))
        }
        404 => Err(ConnectorError::NotFound),
        status => Err(status_error(ctx, status)),
    }
}

/// A non-2xx answer: a rejection of the member's own credential is theirs, not the source's.
fn status_error(ctx: &Ctx, status: u16) -> ConnectorError {
    if matches!(status, 401 | 403) && ctx.uses_user_key() {
        ConnectorError::UserCredentialRejected { status }
    } else {
        ConnectorError::Status { status }
    }
}
