use app::ports::AppEvent;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use contracts::{
    AttributionDto, CatalogItemDto, CompanionDto, InviteDto, LibraryEntryDto, LibraryItemDto,
    LinkRequestDto, ListDto, LoggedSessionDto, NewProgressDto, PrivacyDto, ProgressEventDto,
    SearchResponseDto, SessionDto, ShareLinkDto, SharedListDto, SignInDto, SignUpDto, UserDto,
};
use domain::ids::ItemId;
use serde::Deserialize;
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiResult;
use crate::extract::cookie;
use crate::{ApiError, ApiState, Authed, dto, ops, sharing};

#[derive(OpenApi)]
#[openapi(info(description = "REST API for integrations, scripts and sync clients."))]
struct ApiDoc;

/// `/api/v1` routes plus `/api/v1/openapi.json`.
pub fn router(state: ApiState) -> axum::Router {
    let title = state.naming.api_title.clone();
    let (router, mut doc) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(sign_up))
        .routes(routes!(sign_in))
        .routes(routes!(sign_out))
        .routes(routes!(me))
        .routes(routes!(totp_begin))
        .routes(routes!(totp_confirm))
        .routes(routes!(oidc_start))
        .routes(routes!(oidc_callback))
        .routes(routes!(get_item))
        .routes(routes!(search))
        .routes(routes!(sources))
        .routes(routes!(library, add_to_library))
        .routes(routes!(set_status))
        .routes(routes!(set_rating))
        .routes(routes!(item_progress, log_progress))
        .routes(routes!(companions, create_companion))
        .routes(routes!(lists, create_list))
        .routes(routes!(add_to_list))
        .routes(routes!(request_link))
        .routes(routes!(link_requests))
        .routes(routes!(accept_link))
        .routes(routes!(decline_link))
        .routes(routes!(unlink_companion))
        .routes(routes!(merge_companions))
        .routes(routes!(invite_companion))
        .routes(routes!(my_sessions))
        .routes(routes!(shared_sessions))
        .routes(routes!(claim_session))
        .routes(routes!(set_privacy))
        .routes(routes!(share_list))
        .routes(routes!(shared_list))
        .routes(routes!(sync_changes))
        .routes(routes!(sync_commands))
        .routes(routes!(start_import))
        .routes(routes!(import_status))
        .routes(routes!(export))
        .routes(routes!(events))
        .with_state(state)
        .split_for_parts();
    doc.info.title = title;
    let json = doc.to_pretty_json().unwrap_or_default();
    axum::Router::new().nest(
        "/api/v1",
        // imports upload whole files; the upload quota limits what is kept
        router
            .layer(axum::extract::DefaultBodyLimit::max(64 << 20))
            .route(
                "/openapi.json",
                axum::routing::get(move || {
                    let json = json.clone();
                    async move { ([("content-type", "application/json")], json) }
                }),
            ),
    )
}

fn session_cookie(state: &ApiState, token: &str, max_age_secs: i64) -> HeaderValue {
    let secure = if state.secure_cookies { "; Secure" } else { "" };
    let v = format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_secs}{secure}",
        state.naming.session_cookie
    );
    HeaderValue::from_str(&v).unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn with_session(state: &ApiState, token: &str, body: impl IntoResponse) -> Response {
    let days = i64::from(state.auth.settings().session_days);
    let mut r = body.into_response();
    r.headers_mut()
        .append(SET_COOKIE, session_cookie(state, token, days * 86_400));
    r
}

// ---------------------------------------------------------------- auth

#[utoipa::path(post, path = "/auth/sign-up", tag = "auth", request_body = SignUpDto,
    responses((status = 201, body = SessionDto), (status = 403, body = contracts::ErrorDto)))]
async fn sign_up(State(s): State<ApiState>, Json(b): Json<SignUpDto>) -> ApiResult<Response> {
    let (user, token) = s
        .auth
        .sign_up(auth::SignUp {
            username: b.username,
            display_name: b.display_name,
            email: b.email,
            password: b.password,
            invite: b.invite,
        })
        .await?;
    let body = SessionDto {
        token: token.0.clone(),
        user: dto::user(&user),
    };
    Ok(with_session(
        &s,
        &token.0,
        (StatusCode::CREATED, Json(body)),
    ))
}

#[utoipa::path(post, path = "/auth/sign-in", tag = "auth", request_body = SignInDto,
    responses((status = 200, body = SessionDto), (status = 401, body = contracts::ErrorDto)))]
async fn sign_in(State(s): State<ApiState>, Json(b): Json<SignInDto>) -> ApiResult<Response> {
    let token = s
        .auth
        .sign_in(&b.username, &b.password, b.totp.as_deref())
        .await?;
    let actor = s
        .auth
        .session(&token.0)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    let user = s.app.user(actor.user_id).await?;
    let body = SessionDto {
        token: token.0.clone(),
        user: dto::user(&user),
    };
    Ok(with_session(&s, &token.0, Json(body)))
}

#[utoipa::path(post, path = "/auth/sign-out", tag = "auth", responses((status = 204)))]
async fn sign_out(State(s): State<ApiState>, req: axum::extract::Request) -> ApiResult<Response> {
    let (parts, _) = req.into_parts();
    if let Some(t) = crate::session_token(&parts, &s.naming.session_cookie) {
        s.auth.sign_out(&t).await?;
    }
    let mut r = StatusCode::NO_CONTENT.into_response();
    r.headers_mut()
        .append(SET_COOKIE, session_cookie(&s, "", 0));
    Ok(r)
}

#[utoipa::path(get, path = "/auth/me", tag = "auth",
    responses((status = 200, body = UserDto), (status = 401, body = contracts::ErrorDto)))]
async fn me(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<UserDto>> {
    Ok(Json(dto::user(&s.app.user(a.user_id).await?)))
}

#[derive(serde::Serialize, ToSchema)]
struct TotpEnrolmentDto {
    otpauth_url: String,
    secret_base32: String,
}

#[utoipa::path(post, path = "/auth/totp", tag = "auth", responses((status = 200, body = TotpEnrolmentDto)))]
async fn totp_begin(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<TotpEnrolmentDto>> {
    let e = s.auth.totp_begin(a).await?;
    Ok(Json(TotpEnrolmentDto {
        otpauth_url: e.otpauth_url,
        secret_base32: e.secret_base32,
    }))
}

#[derive(Deserialize, ToSchema)]
struct TotpCodeDto {
    code: String,
}

#[utoipa::path(post, path = "/auth/totp/confirm", tag = "auth", request_body = TotpCodeDto,
    responses((status = 204)))]
async fn totp_confirm(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<TotpCodeDto>,
) -> ApiResult<StatusCode> {
    s.auth.totp_confirm(a, &b.code).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn oidc(s: &ApiState) -> ApiResult<&std::sync::Arc<auth::oidc::Oidc>> {
    s.oidc
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "OIDC is not configured"))
}

#[utoipa::path(get, path = "/auth/oidc/start", tag = "auth", responses((status = 302)))]
async fn oidc_start(State(s): State<ApiState>) -> ApiResult<Response> {
    let (url, pending) = oidc(&s)?.start().await?;
    let secure = if s.secure_cookies { "; Secure" } else { "" };
    let value = serde_json::to_string(&pending).unwrap_or_default();
    let cookie_value = format!(
        "{}={}; Path=/api/v1/auth/oidc; HttpOnly; SameSite=Lax; Max-Age=600{secure}",
        s.naming.oidc_cookie,
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, value)
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        LOCATION,
        HeaderValue::from_str(&url).map_err(|e| ApiError::bad_request(e.to_string()))?,
    );
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie_value).map_err(|e| ApiError::bad_request(e.to_string()))?,
    );
    Ok((StatusCode::FOUND, headers).into_response())
}

#[derive(Deserialize, IntoParams)]
struct OidcCallback {
    code: String,
    state: String,
}

#[utoipa::path(get, path = "/auth/oidc/callback", tag = "auth", params(OidcCallback),
    responses((status = 302)))]
async fn oidc_callback(
    State(s): State<ApiState>,
    Query(q): Query<OidcCallback>,
    req: axum::extract::Request,
) -> ApiResult<Response> {
    let (parts, _) = req.into_parts();
    let raw = cookie(&parts, &s.naming.oidc_cookie)
        .ok_or_else(|| ApiError::bad_request("no pending sign-in"))?;
    let json = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, raw)
        .map_err(|_| ApiError::bad_request("bad sign-in state"))?;
    let pending: auth::oidc::PendingLogin =
        serde_json::from_slice(&json).map_err(|_| ApiError::bad_request("bad sign-in state"))?;
    let token = oidc(&s)?
        .finish(&s.auth, &pending, &q.state, &q.code)
        .await?;
    let mut r = with_session(&s, &token.0, StatusCode::FOUND);
    r.headers_mut()
        .insert(LOCATION, HeaderValue::from_static("/"));
    Ok(r)
}

// ---------------------------------------------------------------- catalog

/// Stored data right away; a stale or partial item gets a background refresh and an
/// `entity.updated` event when it is done (D-6.1).
#[utoipa::path(get, path = "/items/{id}", tag = "catalog",
    params(("id" = String, Path, description = "catalog item id")),
    responses((status = 200, body = CatalogItemDto), (status = 404, body = contracts::ErrorDto)))]
async fn get_item(
    State(s): State<ApiState>,
    Authed(_): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<CatalogItemDto>> {
    Ok(Json(ops::item(&s, &id).await?))
}

#[derive(Deserialize, IntoParams)]
struct SearchParams {
    q: String,
    /// Kind id; all enabled kinds when omitted.
    kind: Option<String>,
}

/// Local results immediately; each connector's results follow as `search.results` events
/// on `/events` with the same `search_id` (D-6.6).
#[utoipa::path(get, path = "/search", tag = "catalog", params(SearchParams),
    responses((status = 200, body = SearchResponseDto)))]
async fn search(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<SearchParams>,
) -> ApiResult<Json<SearchResponseDto>> {
    Ok(Json(ops::search(&s, a, &p.q, p.kind).await?))
}

#[utoipa::path(get, path = "/sources", tag = "catalog",
    responses((status = 200, body = Vec<AttributionDto>)))]
async fn sources(State(s): State<ApiState>) -> Json<Vec<AttributionDto>> {
    Json(ops::sources(&s))
}

// ---------------------------------------------------------------- library

#[utoipa::path(get, path = "/library", tag = "library",
    responses((status = 200, body = Vec<LibraryItemDto>)))]
async fn library(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<LibraryItemDto>>> {
    Ok(Json(ops::library(&s, a).await?))
}

#[derive(Deserialize, ToSchema)]
struct AddDto {
    item_id: String,
    status: Option<String>,
}

#[utoipa::path(post, path = "/library", tag = "library", request_body = AddDto,
    responses((status = 200, body = LibraryEntryDto)))]
async fn add_to_library(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<AddDto>,
) -> ApiResult<Json<LibraryEntryDto>> {
    Ok(Json(ops::add(&s, a, &b.item_id, b.status).await?))
}

#[derive(Deserialize, ToSchema)]
struct StatusDto {
    status: String,
}

#[utoipa::path(put, path = "/library/{item_id}/status", tag = "library", request_body = StatusDto,
    params(("item_id" = String, Path)), responses((status = 200, body = LibraryEntryDto)))]
async fn set_status(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(item_id): Path<String>,
    Json(b): Json<StatusDto>,
) -> ApiResult<Json<LibraryEntryDto>> {
    Ok(Json(ops::set_status(&s, a, &item_id, b.status).await?))
}

#[derive(Deserialize, ToSchema)]
struct RatingDto {
    /// 0..=100, or null to clear.
    rating: Option<u8>,
}

#[utoipa::path(put, path = "/library/{item_id}/rating", tag = "library", request_body = RatingDto,
    params(("item_id" = String, Path)), responses((status = 200, body = LibraryEntryDto)))]
async fn set_rating(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(item_id): Path<String>,
    Json(b): Json<RatingDto>,
) -> ApiResult<Json<LibraryEntryDto>> {
    Ok(Json(ops::rate(&s, a, &item_id, b.rating).await?))
}

#[utoipa::path(get, path = "/items/{id}/progress", tag = "library",
    params(("id" = String, Path)), responses((status = 200, body = Vec<ProgressEventDto>)))]
async fn item_progress(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<ProgressEventDto>>> {
    Ok(Json(ops::progress(&s, a, &id).await?))
}

/// Log progress; with `with` (companion ids) a session with those companions and the user's
/// self-companion is logged as well ("watched with").
#[utoipa::path(post, path = "/items/{id}/progress", tag = "library", request_body = NewProgressDto,
    params(("id" = String, Path)), responses((status = 200, body = ProgressEventDto)))]
async fn log_progress(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<NewProgressDto>,
) -> ApiResult<Json<ProgressEventDto>> {
    Ok(Json(ops::log_progress(&s, a, &id, b).await?))
}

// ---------------------------------------------------------------- companions and lists

#[utoipa::path(get, path = "/companions", tag = "companions",
    responses((status = 200, body = Vec<CompanionDto>)))]
async fn companions(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<CompanionDto>>> {
    Ok(Json(ops::companions(&s, a).await?))
}

#[derive(Deserialize, ToSchema)]
struct NameDto {
    name: String,
}

#[utoipa::path(post, path = "/companions", tag = "companions", request_body = NameDto,
    responses((status = 201, body = CompanionDto)))]
async fn create_companion(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NameDto>,
) -> ApiResult<(StatusCode, Json<CompanionDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(ops::create_companion(&s, a, &b.name).await?),
    ))
}

#[utoipa::path(get, path = "/lists", tag = "lists", responses((status = 200, body = Vec<ListDto>)))]
async fn lists(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<Vec<ListDto>>> {
    Ok(Json(ops::lists(&s, a).await?))
}

#[utoipa::path(post, path = "/lists", tag = "lists", request_body = NameDto,
    responses((status = 201, body = ListDto)))]
async fn create_list(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NameDto>,
) -> ApiResult<(StatusCode, Json<ListDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(ops::create_list(&s, a, &b.name).await?),
    ))
}

#[derive(Deserialize, ToSchema)]
struct ListItemBody {
    item_id: String,
}

#[utoipa::path(post, path = "/lists/{id}/items", tag = "lists", request_body = ListItemBody,
    params(("id" = String, Path)), responses((status = 200, body = ListDto)))]
async fn add_to_list(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<ListItemBody>,
) -> ApiResult<Json<ListDto>> {
    Ok(Json(ops::add_to_list(&s, a, &id, &b.item_id).await?))
}

// ---------------------------------------------------------------- sharing (D-9)

#[derive(Deserialize, ToSchema)]
struct LinkRequestBody {
    /// Username of the person the companion stands for.
    username: String,
}

#[utoipa::path(post, path = "/companions/{id}/link-requests", tag = "sharing",
    request_body = LinkRequestBody, params(("id" = String, Path)),
    responses((status = 201, body = LinkRequestDto), (status = 409, body = contracts::ErrorDto)))]
async fn request_link(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<LinkRequestBody>,
) -> ApiResult<(StatusCode, Json<LinkRequestDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(sharing::request_link(&s, a, &id, &b.username).await?),
    ))
}

#[utoipa::path(get, path = "/link-requests", tag = "sharing",
    responses((status = 200, body = Vec<LinkRequestDto>)))]
async fn link_requests(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<LinkRequestDto>>> {
    Ok(Json(sharing::link_requests(&s, a).await?))
}

#[utoipa::path(post, path = "/link-requests/{id}/accept", tag = "sharing",
    params(("id" = String, Path)), responses((status = 200, body = CompanionDto)))]
async fn accept_link(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<CompanionDto>> {
    Ok(Json(sharing::accept_link(&s, a, &id).await?))
}

#[utoipa::path(post, path = "/link-requests/{id}/decline", tag = "sharing",
    params(("id" = String, Path)), responses((status = 204)))]
async fn decline_link(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    sharing::decline_link(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Demote: end the link. The companion keeps a snapshot of the name; history is unchanged.
#[utoipa::path(post, path = "/companions/{id}/unlink", tag = "sharing",
    params(("id" = String, Path)), responses((status = 200, body = CompanionDto)))]
async fn unlink_companion(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<CompanionDto>> {
    Ok(Json(sharing::unlink(&s, a, &id).await?))
}

#[derive(Deserialize, ToSchema)]
struct MergeBody {
    /// The duplicate that is merged into the companion of the path.
    remove: String,
}

#[utoipa::path(post, path = "/companions/{id}/merge", tag = "sharing", request_body = MergeBody,
    params(("id" = String, Path)), responses((status = 200, body = CompanionDto)))]
async fn merge_companions(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<MergeBody>,
) -> ApiResult<Json<CompanionDto>> {
    Ok(Json(sharing::merge(&s, a, &id, &b.remove).await?))
}

#[derive(Deserialize, ToSchema)]
struct InviteBody {
    #[serde(default)]
    email: Option<String>,
}

/// Promote: a single-use invite to create the account the companion stands for.
#[utoipa::path(post, path = "/companions/{id}/invite", tag = "sharing", request_body = InviteBody,
    params(("id" = String, Path)), responses((status = 201, body = InviteDto)))]
async fn invite_companion(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<InviteBody>,
) -> ApiResult<(StatusCode, Json<InviteDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(sharing::invite(&s, a, &id, b.email).await?),
    ))
}

#[utoipa::path(get, path = "/sessions", tag = "sharing",
    responses((status = 200, body = Vec<LoggedSessionDto>)))]
async fn my_sessions(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<LoggedSessionDto>>> {
    Ok(Json(sharing::my_sessions(&s, a).await?))
}

#[utoipa::path(get, path = "/sessions/shared", tag = "sharing",
    responses((status = 200, body = Vec<LoggedSessionDto>)))]
async fn shared_sessions(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<LoggedSessionDto>>> {
    Ok(Json(sharing::shared_sessions(&s, a).await?))
}

/// "Copy to my log": idempotent.
#[utoipa::path(post, path = "/sessions/{id}/claim", tag = "sharing",
    params(("id" = String, Path)), responses((status = 200, body = LoggedSessionDto)))]
async fn claim_session(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<LoggedSessionDto>> {
    Ok(Json(sharing::claim(&s, a, &id).await?))
}

#[utoipa::path(put, path = "/auth/me/privacy", tag = "sharing", request_body = PrivacyDto,
    responses((status = 200, body = UserDto)))]
async fn set_privacy(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<PrivacyDto>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(sharing::set_privacy(&s, a, b).await?))
}

/// Share by link; a new link revokes the previous one.
#[utoipa::path(post, path = "/lists/{id}/share", tag = "sharing",
    params(("id" = String, Path)), responses((status = 200, body = ShareLinkDto)))]
async fn share_list(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<ShareLinkDto>> {
    Ok(Json(sharing::share_list(&s, a, &id).await?))
}

/// A link-shared list; no account needed.
#[utoipa::path(get, path = "/shared/lists/{token}", tag = "sharing",
    params(("token" = String, Path)),
    responses((status = 200, body = SharedListDto), (status = 404, body = contracts::ErrorDto)))]
async fn shared_list(
    State(s): State<ApiState>,
    Path(token): Path<String>,
) -> ApiResult<Json<SharedListDto>> {
    Ok(Json(sharing::shared_list(&s, &token).await?))
}

// ---------------------------------------------------------------- sync

#[derive(Deserialize, IntoParams)]
struct ChangesParams {
    /// Last `seq` the client has seen (0 for everything).
    #[serde(default)]
    cursor: i64,
    #[serde(default = "default_limit")]
    limit: i64,
}

const fn default_limit() -> i64 {
    crate::sync::MAX_CHANGES
}

/// Changes since `cursor` for this user (D-12).
#[utoipa::path(get, path = "/sync/changes", tag = "sync", params(ChangesParams),
    responses((status = 200, body = contracts::SyncChangesDto)))]
async fn sync_changes(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<ChangesParams>,
) -> ApiResult<Json<contracts::SyncChangesDto>> {
    Ok(Json(crate::sync::pull(&s, a, p.cursor, p.limit).await?))
}

/// Apply a batch of offline commands (idempotent by `op_id`).
#[utoipa::path(post, path = "/sync/commands", tag = "sync", request_body = contracts::SyncBatchDto,
    responses((status = 200, body = Vec<contracts::SyncResultDto>)))]
async fn sync_commands(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<contracts::SyncBatchDto>,
) -> Json<Vec<contracts::SyncResultDto>> {
    Json(crate::sync::push(&s, a, b).await)
}

// ---------------------------------------------------------------- import and export

#[derive(Deserialize, IntoParams)]
struct ImportParams {
    /// `generic-jsonl`, `generic-csv` or `legacy-json`.
    format: String,
    #[serde(default)]
    dry_run: bool,
}

/// Upload a file to import (body: the raw file). Runs as a background job; progress arrives as
/// `import.progress` events, the report at `GET /imports/{id}`.
#[utoipa::path(post, path = "/imports", tag = "import", params(ImportParams),
    request_body(content = String, content_type = "application/octet-stream"),
    responses((status = 202, body = serde_json::Value)))]
async fn start_import(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<ImportParams>,
    body: axum::body::Bytes,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(ops::start_import(&s, a, &p.format, body.to_vec(), p.dry_run).await?),
    ))
}

#[utoipa::path(get, path = "/imports/{id}", tag = "import", params(("id" = String, Path)),
    responses((status = 200, body = serde_json::Value)))]
async fn import_status(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(ops::import_status(&s, a, &id).await?))
}

/// The library as JSON Lines in the generic format (re-importable).
#[utoipa::path(get, path = "/export", tag = "import",
    responses((status = 200, content_type = "application/x-ndjson", body = String)))]
async fn export(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Response> {
    let body = ops::export(&s, a).await?;
    Ok(([("content-type", "application/x-ndjson")], body).into_response())
}

// ---------------------------------------------------------------- events

/// Server-Sent Events for the signed-in user: `entity.updated`, `job.progress`,
/// `import.progress`, `search.results`, and `admin.*` for admins.
#[utoipa::path(get, path = "/events", tag = "events",
    responses((status = 200, content_type = "text/event-stream", body = String)))]
async fn events(State(s): State<ApiState>, Authed(a): Authed) -> impl IntoResponse {
    s.bus.sse(a)
}

impl ApiState {
    pub(crate) async fn publish(&self, e: AppEvent) {
        use app::ports::EventBus;
        self.bus.publish(e).await;
    }

    /// A library reference pins the item's cover (never evicted, D-8).
    pub(crate) async fn pin_cover(&self, item: ItemId) {
        let Some(svc) = &self.assets else { return };
        let Ok(i) = self.app.catalog_item(item).await else {
            return;
        };
        if let Some(id) = i.cover_asset.and_then(|c| uuid::Uuid::parse_str(&c).ok())
            && let Err(e) = svc.pin(id).await
        {
            tracing::warn!(error = %e, "could not pin cover");
        }
    }
}
