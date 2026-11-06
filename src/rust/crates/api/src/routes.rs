use std::collections::HashSet;

use app::ports::{AppEvent, SearchQuery};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use contracts::{
    AttributionDto, CatalogItemDto, CompanionDto, LibraryEntryDto, LibraryItemDto, NewProgressDto,
    ProgressEventDto, SearchResponseDto, SearchResultsEventDto, SessionDto, SignInDto, SignUpDto,
    UserDto,
};
use domain::ids::{CompanionId, ItemId};
use domain::kind::Status;
use domain::library::Rating;
use domain::progress::Progress;
use domain::session::Participant;
use serde::Deserialize;
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiResult;
use crate::extract::cookie;
use crate::{ApiError, ApiState, Authed, dto};

/// Topic of streamed connector search results.
pub const SEARCH_RESULTS: &str = "search.results";

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
        .routes(routes!(events))
        .with_state(state)
        .split_for_parts();
    doc.info.title = title;
    let json = doc.to_pretty_json().unwrap_or_default();
    axum::Router::new().nest(
        "/api/v1",
        router.route(
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

fn parse_id<T>(s: &str, wrap: impl FnOnce(uuid::Uuid) -> T) -> ApiResult<T> {
    uuid::Uuid::parse_str(s)
        .map(wrap)
        .map_err(|_| ApiError::bad_request(format!("invalid id `{s}`")))
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
    let item = s.app.item_detail(parse_id(&id, ItemId)?).await?;
    Ok(Json(dto::item(&s, &item).await))
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
    let text = p.q.trim().to_string();
    if text.is_empty() {
        return Err(ApiError::bad_request("empty query"));
    }
    let kinds = p.kind.map_or_else(|| s.default_kinds.clone(), |k| vec![k]);
    let mut filters = Vec::new();
    if kinds.len() == 1 {
        filters.push(("kind".to_string(), kinds[0].clone()));
    }
    let hits = s
        .search
        .search(SearchQuery {
            index: "catalog".into(),
            text: text.clone(),
            viewer: a.user_id,
            filters,
            limit: 20,
        })
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "local search failed");
            vec![]
        });
    let mut items = Vec::new();
    for h in hits {
        if let Ok(id) = uuid::Uuid::parse_str(&h.id)
            && let Ok(item) = s.app.catalog_item(ItemId(id)).await
        {
            items.push(dto::item(&s, &item).await);
        }
    }
    let search_id = uuid::Uuid::now_v7().to_string();
    let pending: Vec<String> = s
        .fanout
        .targets(&kinds)
        .into_iter()
        .map(|(c, _)| c)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let state = s.clone();
    let sid = search_id.clone();
    tokio::spawn(async move {
        state
            .fanout
            .run(&text, &kinds, 20, |r| {
                let state = state.clone();
                let sid = sid.clone();
                async move {
                    let mut dtos = Vec::new();
                    for i in &r.items {
                        dtos.push(dto::item(&state, i).await);
                    }
                    let payload = SearchResultsEventDto {
                        search_id: sid,
                        connector: r.connector,
                        items: dtos,
                        error: r.error,
                    };
                    state
                        .bus_publish(AppEvent {
                            topic: SEARCH_RESULTS.into(),
                            entity: None,
                            audience: vec![a.user_id],
                            payload: serde_json::to_value(payload).unwrap_or_default(),
                        })
                        .await;
                }
            })
            .await;
    });
    Ok(Json(SearchResponseDto {
        search_id,
        items,
        pending,
    }))
}

#[utoipa::path(get, path = "/sources", tag = "catalog",
    responses((status = 200, body = Vec<AttributionDto>)))]
async fn sources(State(s): State<ApiState>) -> Json<Vec<AttributionDto>> {
    Json(
        s.registry
            .infos()
            .into_iter()
            .map(|i| AttributionDto {
                connector: i.id,
                name: i.name,
                text: i.attribution.text,
                url: i.attribution.url,
                licence: i.attribution.licence,
            })
            .collect(),
    )
}

// ---------------------------------------------------------------- library

#[utoipa::path(get, path = "/library", tag = "library",
    responses((status = 200, body = Vec<LibraryItemDto>)))]
async fn library(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<LibraryItemDto>>> {
    let mut out = Vec::new();
    for e in s.app.library(a).await? {
        let item = s.app.catalog_item(e.item_id).await?;
        out.push(LibraryItemDto {
            entry: dto::entry(&e),
            item: dto::item(&s, &item).await,
        });
    }
    Ok(Json(out))
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
    let id = parse_id(&b.item_id, ItemId)?;
    let e = s
        .app
        .add_to_library(a, id, b.status.map(Status::new))
        .await?;
    s.pin_cover(id).await;
    Ok(Json(dto::entry(&e)))
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
    let e = s
        .app
        .set_status(a, parse_id(&item_id, ItemId)?, Status::new(b.status), None)
        .await?;
    Ok(Json(dto::entry(&e)))
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
    let rating = b
        .rating
        .map(Rating::new)
        .transpose()
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let e = s
        .app
        .rate(a, parse_id(&item_id, ItemId)?, rating, None)
        .await?;
    Ok(Json(dto::entry(&e)))
}

#[utoipa::path(get, path = "/items/{id}/progress", tag = "library",
    params(("id" = String, Path)), responses((status = 200, body = Vec<ProgressEventDto>)))]
async fn item_progress(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<ProgressEventDto>>> {
    let events = s.app.progress(a, parse_id(&id, ItemId)?).await?;
    Ok(Json(events.iter().map(dto::progress).collect()))
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
    let item_id = parse_id(&id, ItemId)?;
    let progress: Progress = serde_json::from_value(b.progress)
        .map_err(|e| ApiError::bad_request(format!("progress: {e}")))?;
    let occurred_at = b
        .occurred_at
        .map(|t| {
            chrono::DateTime::parse_from_rfc3339(&t)
                .map(|d| d.with_timezone(&chrono::Utc))
                .map_err(|_| ApiError::bad_request("occurred_at must be RFC 3339"))
        })
        .transpose()?;
    let op = b.client_op_id.map(|o| parse_id(&o, |u| u)).transpose()?;
    if !b.with.is_empty() {
        let me = s.app.user(a.user_id).await?;
        let mut participants = vec![Participant::new(me.self_companion)];
        for c in &b.with {
            participants.push(Participant::new(parse_id(c, CompanionId)?));
        }
        let mut session = app::NewSession::new(item_id, participants);
        session.started_at = occurred_at;
        // Idempotency key of the session, derived from the client op id (replays are no-ops).
        session.client_op_id = op.map(|o| {
            let h = blake3::hash(&[o.as_bytes().as_slice(), b"session"].concat());
            let mut b = [0u8; 16];
            b.copy_from_slice(&h.as_bytes()[..16]);
            uuid::Uuid::from_bytes(b)
        });
        s.app.log_session(a, session).await?;
    }
    let e = s
        .app
        .log_progress(a, item_id, progress, occurred_at, op)
        .await?;
    s.pin_cover(item_id).await;
    Ok(Json(dto::progress(&e)))
}

// ---------------------------------------------------------------- companions

#[utoipa::path(get, path = "/companions", tag = "companions",
    responses((status = 200, body = Vec<CompanionDto>)))]
async fn companions(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<CompanionDto>>> {
    Ok(Json(
        s.app
            .companions(a)
            .await?
            .iter()
            .map(dto::companion)
            .collect(),
    ))
}

#[derive(Deserialize, ToSchema)]
struct NewCompanionDto {
    name: String,
}

#[utoipa::path(post, path = "/companions", tag = "companions", request_body = NewCompanionDto,
    responses((status = 201, body = CompanionDto)))]
async fn create_companion(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NewCompanionDto>,
) -> ApiResult<(StatusCode, Json<CompanionDto>)> {
    let c = s.app.create_companion(a, &b.name).await?;
    Ok((StatusCode::CREATED, Json(dto::companion(&c))))
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
    async fn bus_publish(&self, e: AppEvent) {
        use app::ports::EventBus;
        self.bus.publish(e).await;
    }

    /// A library reference pins the item's cover (never evicted, D-8).
    async fn pin_cover(&self, item: ItemId) {
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
