//! REST routes of the logging funnel (WP4-5): the beacon and the admin report. Handlers only
//! call `crate::usage`.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use contracts::usage::{UsageBeaconDto, UsageReportDto};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiResult;
use crate::{ApiState, Authed};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(funnel_beacon))
        .routes(routes!(usage_report))
        .routes(routes!(usage_csv))
}

/// The browser's funnel beacon: fixed words and buckets, no ids (204 also while counting is
/// off: the beacon is then dropped).
#[utoipa::path(post, path = "/usage/funnel", tag = "usage", request_body = UsageBeaconDto,
    responses((status = 204), (status = 401, body = contracts::ErrorDto),
        (status = 422, body = contracts::ErrorDto), (status = 429, body = contracts::ErrorDto)))]
async fn funnel_beacon(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<UsageBeaconDto>,
) -> ApiResult<StatusCode> {
    crate::usage::record(&s, a, b).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, IntoParams)]
struct UsageParams {
    /// Days back from today (default 28, at most 180).
    days: Option<u32>,
}

#[utoipa::path(get, path = "/admin/usage", tag = "admin", params(UsageParams),
    responses((status = 200, body = UsageReportDto), (status = 403, body = contracts::ErrorDto)))]
async fn usage_report(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<UsageParams>,
) -> ApiResult<Json<UsageReportDto>> {
    Ok(Json(crate::usage::report(&s, a, p.days).await?))
}

/// The same rows as CSV.
#[utoipa::path(get, path = "/admin/usage.csv", tag = "admin", params(UsageParams),
    responses((status = 200, content_type = "text/csv", body = String)))]
async fn usage_csv(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<UsageParams>,
) -> ApiResult<Response> {
    let csv = crate::usage::report_csv(&s, a, p.days).await?;
    Ok((
        [
            (CONTENT_TYPE, "text/csv; charset=utf-8"),
            (CONTENT_DISPOSITION, "attachment; filename=\"usage.csv\""),
        ],
        csv,
    )
        .into_response())
}
