use app::ports::AppEvent;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use contracts::{
    AttributionDto, CatalogItemDto, ChannelDto, CompanionDto, DeliveryDto, EventTypeDto,
    InboxItemDto, InviteDto, LibraryEntryDto, LinkRequestDto, ListDto, LocaleDto, LoggedSessionDto,
    NewChannelDto, NewProgressDto, PrivacyDto, ProgressEventDto, SearchResponseDto, SessionDto,
    ShareLinkDto, SharedListDto, SignInDto, SignUpDto, SubscriptionDto, TemplateOverrideDto,
    UserDto, WebPushKeyDto,
};
use domain::ids::ItemId;
use serde::Deserialize;
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiResult;
use crate::{ApiError, ApiState, Authed, dto, hooks, notifications, ops, sharing};

#[derive(OpenApi)]
#[openapi(info(description = "REST API for integrations, scripts and sync clients."))]
struct ApiDoc;

/// Largest JSON request body.
pub const JSON_BODY_LIMIT: usize = 2 << 20;
/// Largest import upload.
pub const IMPORT_BODY_LIMIT: usize = 64 << 20;

/// Notifications, integrations, sync and imports (kept apart so [`router`] stays short).
fn notify_sync_import_routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(notification_events))
        .routes(routes!(inbox))
        .routes(routes!(mark_read))
        .routes(routes!(mark_all_read))
        .routes(routes!(channels, create_channel))
        .routes(routes!(update_channel, delete_channel))
        .routes(routes!(test_channel))
        .routes(routes!(subscriptions, put_subscription))
        .routes(routes!(delete_subscription))
        .routes(routes!(deliveries))
        .routes(routes!(templates, set_template))
        .routes(routes!(web_push_key))
        .routes(routes!(integrations, create_integration))
        .routes(routes!(delete_integration))
        .routes(routes!(test_integration))
        .routes(routes!(watched_hook))
        .routes(routes!(sync_changes))
        .routes(routes!(sync_commands))
        .routes(routes!(import_status))
        .routes(routes!(rerun_import))
        .routes(routes!(cancel_import))
        .routes(routes!(match_import_row))
        .routes(routes!(export))
}

/// `/api/v1` routes plus `/api/v1/openapi.json`.
pub fn router(state: ApiState) -> axum::Router {
    let title = state.naming.api_title.clone();
    let (router, mut doc) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(sign_up))
        .routes(routes!(sign_in))
        .routes(routes!(username_available))
        .routes(routes!(sign_out))
        .routes(routes!(me))
        .routes(routes!(totp_begin))
        .routes(routes!(totp_confirm))
        .routes(routes!(get_item))
        .routes(routes!(search))
        .routes(routes!(sources))
        .routes(routes!(library, add_to_library))
        .routes(routes!(set_status))
        .routes(routes!(set_rating))
        .routes(routes!(item_progress, log_progress))
        .routes(routes!(log_play))
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
        .routes(routes!(set_locale))
        .routes(routes!(set_time_zone))
        .routes(routes!(reset_password))
        .routes(routes!(share_list))
        .routes(routes!(shared_list))
        .merge(notify_sync_import_routes())
        .routes(routes!(events))
        .routes(routes!(kinds))
        .routes(routes!(library_counts))
        .routes(routes!(library_query))
        .routes(routes!(crate::bulk::library_bulk))
        .routes(routes!(crate::bulk::library_bulk_undo))
        .routes(routes!(crate::bulk::remove_from_library))
        .routes(routes!(tags))
        .routes(routes!(rename_tag))
        .routes(routes!(add_tag))
        .routes(routes!(remove_tag))
        .routes(routes!(set_note))
        .routes(routes!(undo_progress))
        .routes(routes!(activity))
        .routes(routes!(stats_totals))
        .routes(routes!(stats_series))
        .routes(routes!(home))
        .routes(routes!(item_page))
        .routes(routes!(set_page_count))
        .merge(crate::admin_routes::routes())
        .merge(crate::people_routes::routes())
        .merge(crate::goals_routes::routes())
        .merge(crate::manual_routes::routes())
        .merge(crate::oidc_routes::routes())
        .merge(crate::sso_routes::routes())
        .merge(crate::usage_routes::routes())
        // JSON bodies are small (G2-10); only file imports may be large (the upload quota
        // limits what is kept)
        .layer(axum::extract::DefaultBodyLimit::max(JSON_BODY_LIMIT))
        .merge(
            OpenApiRouter::new()
                .routes(routes!(start_import, list_imports))
                .layer(axum::extract::DefaultBodyLimit::max(IMPORT_BODY_LIMIT)),
        )
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

pub(crate) fn session_cookie(state: &ApiState, token: &str, max_age_secs: i64) -> HeaderValue {
    let secure = if state.cookie_secure() {
        "; Secure"
    } else {
        ""
    };
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
    // `auto` mode: 403 `sso_only` (the break-glass path is `/auth/sign-in/local`)
    let token = crate::password_sign_in(&s, &b, false).await?;
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

/// Is a username free? Normalised like sign-up; answers only when sign-up is possible
/// (first account, open sign-up or a valid invite) and is rate limited (DEC-184).
#[utoipa::path(post, path = "/auth/username-available", tag = "auth",
    request_body = contracts::auth::UsernameCheckDto,
    responses((status = 200, body = contracts::auth::UsernameAvailability),
        (status = 403, body = contracts::ErrorDto), (status = 429, body = contracts::ErrorDto)))]
async fn username_available(
    State(s): State<ApiState>,
    Json(b): Json<contracts::auth::UsernameCheckDto>,
) -> ApiResult<Json<contracts::auth::UsernameAvailability>> {
    Ok(Json(
        s.auth
            .username_available(&b.username, b.invite.as_deref())
            .await?,
    ))
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

/// Start enrolling an authenticator; needs a fresh proof (DEC-191, `reauth_required`).
#[utoipa::path(post, path = "/auth/totp", tag = "auth", responses((status = 200, body = contracts::admin::TotpEnrolmentDto),
    (status = 403, body = contracts::ErrorDto)))]
async fn totp_begin(
    State(s): State<ApiState>,
    t: crate::AuthedToken,
) -> ApiResult<Json<contracts::admin::TotpEnrolmentDto>> {
    Ok(Json(crate::admin::totp_begin(&s, &t.token).await?))
}

#[derive(Deserialize, ToSchema)]
struct TotpCodeDto {
    code: String,
}

/// Turn the authenticator on; answers the recovery codes, shown once (OD-08).
#[utoipa::path(post, path = "/auth/totp/confirm", tag = "auth", request_body = TotpCodeDto,
    responses((status = 200, body = contracts::admin::RecoveryCodesDto)))]
async fn totp_confirm(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<TotpCodeDto>,
) -> ApiResult<Json<contracts::admin::RecoveryCodesDto>> {
    Ok(Json(crate::admin::totp_confirm(&s, a, &b.code).await?))
}

// ---------------------------------------------------------------- catalog

/// Stored data right away; a stale or partial item gets a background refresh and an
/// `entity.updated` event when it is done (D-6.1).
#[utoipa::path(get, path = "/items/{id}", tag = "catalog",
    params(("id" = String, Path, description = "catalog item id")),
    responses((status = 200, body = CatalogItemDto), (status = 404, body = contracts::ErrorDto)))]
async fn get_item(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<CatalogItemDto>> {
    Ok(Json(ops::item(&s, a, &id).await?))
}

#[derive(Deserialize, IntoParams)]
struct SearchParams {
    q: String,
    /// Kind id; all enabled kinds when omitted.
    kind: Option<String>,
    /// Kind group id (`watch`, `read`, ...): its enabled kinds; ignored with `kind`.
    group: Option<String>,
}

/// Local results immediately; each connector's results follow as `search.results` events
/// on `/events` with the same `search_id` (D-6.6). A member whose language is automatic
/// gets the sources' answers in the first language of `Accept-Language` (OT-34).
#[utoipa::path(get, path = "/search", tag = "catalog", params(SearchParams),
    responses((status = 200, body = SearchResponseDto)))]
async fn search(
    State(s): State<ApiState>,
    Authed(a): Authed,
    headers: HeaderMap,
    Query(p): Query<SearchParams>,
) -> ApiResult<Json<SearchResponseDto>> {
    let ui_language = crate::ops::accept_language(&headers);
    Ok(Json(
        ops::search(&s, a, &p.q, p.kind, p.group, ui_language).await?,
    ))
}

#[utoipa::path(get, path = "/sources", tag = "catalog",
    responses((status = 200, body = Vec<AttributionDto>)))]
async fn sources(State(s): State<ApiState>) -> Json<Vec<AttributionDto>> {
    Json(ops::sources(&s))
}

// ---------------------------------------------------------------- library

#[derive(Deserialize, IntoParams)]
struct PageParams {
    /// The previous page's `next` (keyset paging, DEC-416).
    after: Option<String>,
    /// 1 to 500, default 100.
    #[serde(default = "default_page")]
    limit: u32,
}

const fn default_page() -> u32 {
    100
}

/// The library, most recently added first, in pages.
#[utoipa::path(get, path = "/library", tag = "library", params(PageParams),
    responses((status = 200, body = contracts::LibraryPageDto)))]
async fn library(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<PageParams>,
) -> ApiResult<Json<contracts::LibraryPageDto>> {
    Ok(Json(
        ops::library(&s, a, p.after.as_deref(), p.limit).await?,
    ))
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

/// Log a play or gaming session (sessions progress model) with players, scores and the
/// winner; answers its progress event (for Undo).
#[utoipa::path(post, path = "/items/{id}/plays", tag = "library", request_body = contracts::NewPlayDto,
    params(("id" = String, Path)), responses((status = 200, body = ProgressEventDto)))]
async fn log_play(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<contracts::NewPlayDto>,
) -> ApiResult<Json<ProgressEventDto>> {
    Ok(Json(ops::log_play(&s, a, &id, b).await?))
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

#[derive(Deserialize, ToSchema)]
struct NewPersonBody {
    name: String,
    /// An optional note ("Add a person", design.md 6.7).
    #[serde(default)]
    notes: Option<String>,
}

#[utoipa::path(post, path = "/companions", tag = "companions", request_body = NewPersonBody,
    responses((status = 201, body = CompanionDto)))]
async fn create_companion(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NewPersonBody>,
) -> ApiResult<(StatusCode, Json<CompanionDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(crate::people::add_person(&s, a, &b.name, b.notes).await?),
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

/// The caller's language for pages, notifications and emails; `null` is automatic.
#[utoipa::path(put, path = "/auth/me/locale", tag = "auth", request_body = LocaleDto,
    responses((status = 200, body = UserDto), (status = 422, body = contracts::ErrorDto)))]
async fn set_locale(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<LocaleDto>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(sharing::set_locale(&s, a, b).await?))
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

// ---------------------------------------------------------------- notifications (D-17)

#[utoipa::path(get, path = "/notifications/events", tag = "notifications",
    responses((status = 200, body = Vec<EventTypeDto>)))]
async fn notification_events(Authed(a): Authed) -> Json<Vec<EventTypeDto>> {
    Json(notifications::catalogue(a))
}

#[utoipa::path(get, path = "/notifications/inbox", tag = "notifications",
    responses((status = 200, body = Vec<InboxItemDto>)))]
async fn inbox(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<Vec<InboxItemDto>>> {
    Ok(Json(notifications::inbox(&s, a).await?))
}

#[utoipa::path(post, path = "/notifications/inbox/{id}/read", tag = "notifications",
    params(("id" = String, Path)), responses((status = 204)))]
async fn mark_read(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    notifications::mark_read(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// "Mark all read": every unread inbox row of the caller.
#[utoipa::path(post, path = "/notifications/inbox/read-all", tag = "notifications",
    responses((status = 204)))]
async fn mark_all_read(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<StatusCode> {
    notifications::mark_all_read(&s, a).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/notifications/channels", tag = "notifications",
    responses((status = 200, body = Vec<ChannelDto>)))]
async fn channels(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<ChannelDto>>> {
    Ok(Json(notifications::channels(&s, a).await?))
}

#[utoipa::path(post, path = "/notifications/channels", tag = "notifications",
    request_body = NewChannelDto, responses((status = 201, body = ChannelDto)))]
async fn create_channel(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NewChannelDto>,
) -> ApiResult<(StatusCode, Json<ChannelDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(notifications::create_channel(&s, a, b).await?),
    ))
}

#[utoipa::path(put, path = "/notifications/channels/{id}", tag = "notifications",
    request_body = NewChannelDto, params(("id" = String, Path)),
    responses((status = 200, body = ChannelDto)))]
async fn update_channel(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<NewChannelDto>,
) -> ApiResult<Json<ChannelDto>> {
    Ok(Json(notifications::update_channel(&s, a, &id, b).await?))
}

#[utoipa::path(delete, path = "/notifications/channels/{id}", tag = "notifications",
    params(("id" = String, Path)), responses((status = 204)))]
async fn delete_channel(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    notifications::delete_channel(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Send a test message now; 502 `channel_error` carries what the channel answered.
#[utoipa::path(post, path = "/notifications/channels/{id}/test", tag = "notifications",
    params(("id" = String, Path)),
    responses((status = 204), (status = 502, body = contracts::ErrorDto)))]
async fn test_channel(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    notifications::test_channel(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/notifications/subscriptions", tag = "notifications",
    responses((status = 200, body = Vec<SubscriptionDto>)))]
async fn subscriptions(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<SubscriptionDto>>> {
    Ok(Json(notifications::subscriptions(&s, a).await?))
}

/// Create (no `id`) or update a subscription; admins set instance defaults with
/// `instance_default: true`.
#[utoipa::path(put, path = "/notifications/subscriptions", tag = "notifications",
    request_body = SubscriptionDto, responses((status = 200, body = SubscriptionDto)))]
async fn put_subscription(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<SubscriptionDto>,
) -> ApiResult<Json<SubscriptionDto>> {
    Ok(Json(notifications::put_subscription(&s, a, b).await?))
}

#[utoipa::path(delete, path = "/notifications/subscriptions/{id}", tag = "notifications",
    params(("id" = String, Path)), responses((status = 204)))]
async fn delete_subscription(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    notifications::delete_subscription(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/notifications/deliveries", tag = "notifications",
    responses((status = 200, body = Vec<DeliveryDto>)))]
async fn deliveries(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<DeliveryDto>>> {
    Ok(Json(notifications::deliveries(&s, a).await?))
}

#[utoipa::path(get, path = "/notifications/templates", tag = "notifications",
    responses((status = 200, body = Vec<TemplateOverrideDto>)))]
async fn templates(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<TemplateOverrideDto>>> {
    Ok(Json(notifications::templates(&s, a).await?))
}

#[utoipa::path(put, path = "/notifications/templates", tag = "notifications",
    request_body = TemplateOverrideDto, responses((status = 204)))]
async fn set_template(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<TemplateOverrideDto>,
) -> ApiResult<StatusCode> {
    notifications::set_template(&s, a, b).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/notifications/web-push-key", tag = "notifications",
    responses((status = 200, body = WebPushKeyDto)))]
async fn web_push_key(State(s): State<ApiState>) -> Json<WebPushKeyDto> {
    Json(WebPushKeyDto {
        public_key: s.notify.as_ref().and_then(|n| n.web_push_key.clone()),
    })
}

// ---------------------------------------------------------------- integrations (M14)

#[utoipa::path(get, path = "/integrations", tag = "integrations",
    responses((status = 200, body = Vec<contracts::IntegrationDto>)))]
async fn integrations(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<contracts::IntegrationDto>>> {
    Ok(Json(hooks::list(&s, a).await?))
}

/// Create an integration; the token in the answer is shown only once.
#[utoipa::path(post, path = "/integrations", tag = "integrations", request_body = NameDto,
    responses((status = 201, body = contracts::IntegrationCreatedDto)))]
async fn create_integration(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NameDto>,
) -> ApiResult<(StatusCode, Json<contracts::IntegrationCreatedDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(hooks::create(&s, a, &b.name).await?),
    ))
}

#[utoipa::path(delete, path = "/integrations/{id}", tag = "integrations",
    params(("id" = String, Path)), responses((status = 204)))]
async fn delete_integration(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    hooks::delete(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// "Send a test call from here" (design.md 5.9): the body is a sample `WatchedEvent`; it is
/// validated and matched like a real call, nothing is recorded, and the user's pages get an
/// `integration.call` marked `test`. A refused sample is a 422 with `field`.
#[utoipa::path(post, path = "/integrations/{id}/test", tag = "integrations",
    params(("id" = String, Path)),
    request_body(content = Object, description = "WatchedEvent (JSON Schema in the docs)"),
    responses((status = 200, body = contracts::IntegrationTestDto),
        (status = 404, body = contracts::ErrorDto), (status = 422, body = contracts::ErrorDto)))]
async fn test_integration(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    body: axum::body::Bytes,
) -> ApiResult<Json<contracts::IntegrationTestDto>> {
    Ok(Json(hooks::test(&s, a, &id, &body).await?))
}

/// The watched webhook: one finished movie or episode in our JSON format (see the docs,
/// "Integrations"). Authenticated with an integration token, not a session:
/// `Authorization: Bearer itk_...`. Posting the same play twice records it once. The token
/// is checked before the body, so a refused body (422, with the offending `field`) shows on
/// the sender's card in Settings > Integrations.
#[utoipa::path(post, path = "/hooks/watched", tag = "integrations",
    request_body(content = Object, description = "WatchedEvent (JSON Schema in the docs)"),
    responses((status = 200, body = contracts::WatchedRecordedDto),
        (status = 401, body = contracts::ErrorDto), (status = 422, body = contracts::ErrorDto)))]
async fn watched_hook(
    State(s): State<ApiState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> ApiResult<Json<contracts::WatchedRecordedDto>> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .ok_or_else(ApiError::unauthorized)?;
    Ok(Json(hooks::watched(&s, token, &body).await?))
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

/// The user's imports, newest first: state, counts, preview, errors and unmatched rows.
#[utoipa::path(get, path = "/imports", tag = "import",
    responses((status = 200, body = Vec<serde_json::Value>)))]
async fn list_imports(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    Ok(Json(ops::imports(&s, a).await?))
}

/// Run the file of an import again for real: after a dry run's preview, or as a retry.
#[utoipa::path(post, path = "/imports/{id}/run", tag = "import", params(("id" = String, Path)),
    responses((status = 202, body = serde_json::Value)))]
async fn rerun_import(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(ops::rerun_import(&s, a, &id).await?),
    ))
}

/// Stop a queued or running import at its next checkpoint (409 `import_finished` after it).
#[utoipa::path(post, path = "/imports/{id}/cancel", tag = "import", params(("id" = String, Path)),
    responses((status = 202, body = serde_json::Value)))]
async fn cancel_import(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    Ok((
        StatusCode::ACCEPTED,
        Json(ops::cancel_import(&s, a, &id).await?),
    ))
}

#[derive(Deserialize, ToSchema)]
struct MatchRowBody {
    /// The unmatched row's `source_id`.
    source_id: String,
    /// The catalog item it is.
    item_id: String,
}

/// Match an unmatched row of an import with a catalog item; the row is imported then.
#[utoipa::path(post, path = "/imports/{id}/match", tag = "import", params(("id" = String, Path)),
    request_body = MatchRowBody, responses((status = 200, body = serde_json::Value)))]
async fn match_import_row(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<MatchRowBody>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(
        ops::match_import_row(&s, a, &id, &b.source_id, &b.item_id).await?,
    ))
}

#[derive(Deserialize, IntoParams)]
struct ExportParams {
    /// `jsonl` (default) or `csv`.
    #[serde(default)]
    format: Option<String>,
}

/// The library in the generic format (re-importable): JSON Lines, or CSV with `format=csv`
/// (finished dates only). Sent as a download.
#[utoipa::path(get, path = "/export", tag = "import", params(ExportParams),
    responses((status = 200, content_type = "application/x-ndjson", body = String)))]
async fn export(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<ExportParams>,
) -> ApiResult<Response> {
    let (body, kind, name) = match p.format.as_deref() {
        None | Some("jsonl") => (
            ops::export(&s, a).await?,
            "application/x-ndjson",
            "library-export.jsonl",
        ),
        Some("csv") => (
            ops::export_csv(&s, a).await?,
            "text/csv; charset=utf-8",
            "library-export.csv",
        ),
        Some(other) => {
            return Err(ApiError::bad_request(format!(
                "unknown export format `{other}`"
            )));
        }
    };
    let disposition = format!("attachment; filename=\"{name}\"");
    Ok((
        [
            ("content-type", kind),
            ("content-disposition", disposition.as_str()),
        ],
        body,
    )
        .into_response())
}

// ---------------------------------------------------------------- events

/// Server-Sent Events for the signed-in user: `entity.updated`, `job.progress`,
/// `import.progress`, `search.results`, `notification.new` (a new inbox row),
/// `notification.delivery` (the outcome of a delivery to one of the user's channels was
/// written), `integration.call` (a call by one of the user's integrations), and `admin.*`
/// for admins. A client that reads too slowly gets `stream.lagged` in place of the events
/// it missed and should read its data again.
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

// ---------------------------------------------------------------- UX phase 1

#[utoipa::path(get, path = "/kinds", tag = "catalog",
    responses((status = 200, body = contracts::KindsDto)))]
async fn kinds(State(s): State<ApiState>, Authed(_): Authed) -> Json<contracts::KindsDto> {
    Json(crate::browse::kinds(&s))
}

/// Entry counts per group, kind and status of the signed-in user.
#[utoipa::path(get, path = "/library/counts", tag = "library",
    responses((status = 200, body = contracts::LibraryCountsDto)))]
async fn library_counts(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<contracts::LibraryCountsDto>> {
    Ok(Json(crate::browse::library_counts(&s, a).await?))
}

/// The Library page query: filters, text filter, sort, paging; rows carry the next step.
#[utoipa::path(post, path = "/library/query", tag = "library",
    request_body = contracts::LibraryQueryDto,
    responses((status = 200, body = contracts::LibraryRowsDto),
        (status = 400, body = contracts::ErrorDto), (status = 422, body = contracts::ErrorDto)))]
async fn library_query(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<contracts::LibraryQueryDto>,
) -> ApiResult<Json<contracts::LibraryRowsDto>> {
    Ok(Json(crate::browse::library_query(&s, a, b).await?))
}

#[utoipa::path(get, path = "/tags", tag = "library",
    responses((status = 200, body = Vec<contracts::TagDto>)))]
async fn tags(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<contracts::TagDto>>> {
    Ok(Json(crate::browse::tags(&s, a).await?))
}

/// Rename a tag on every entry (onto an existing tag merges them); answers the number of
/// entries changed.
#[utoipa::path(put, path = "/tags/{tag}", tag = "library", request_body = contracts::RenameTagDto,
    params(("tag" = String, Path)), responses((status = 200, body = u64)))]
async fn rename_tag(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(tag): Path<String>,
    Json(b): Json<contracts::RenameTagDto>,
) -> ApiResult<Json<u64>> {
    Ok(Json(crate::browse::rename_tag(&s, a, &tag, &b.name).await?))
}

/// Add a tag; answers the entry's tags.
#[utoipa::path(post, path = "/library/{item_id}/tags", tag = "library",
    request_body = contracts::TagBodyDto, params(("item_id" = String, Path)),
    responses((status = 200, body = Vec<String>)))]
async fn add_tag(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(item_id): Path<String>,
    Json(b): Json<contracts::TagBodyDto>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(crate::browse::add_tag(&s, a, &item_id, &b.tag).await?))
}

#[utoipa::path(delete, path = "/library/{item_id}/tags/{tag}", tag = "library",
    params(("item_id" = String, Path), ("tag" = String, Path)),
    responses((status = 200, body = Vec<String>)))]
async fn remove_tag(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path((item_id, tag)): Path<(String, String)>,
) -> ApiResult<Json<Vec<String>>> {
    Ok(Json(
        crate::browse::remove_tag(&s, a, &item_id, &tag).await?,
    ))
}

/// The private note of an entry (never shown to anyone else).
#[utoipa::path(put, path = "/library/{item_id}/note", tag = "library",
    request_body = contracts::NoteDto, params(("item_id" = String, Path)),
    responses((status = 200, body = LibraryEntryDto)))]
async fn set_note(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(item_id): Path<String>,
    Json(b): Json<contracts::NoteDto>,
) -> ApiResult<Json<LibraryEntryDto>> {
    Ok(Json(
        crate::browse::set_note(&s, a, &item_id, b.note).await?,
    ))
}

/// Undo one of your recent progress events: appends a compensating `undo` event.
#[utoipa::path(post, path = "/progress/{event_id}/undo", tag = "library",
    params(("event_id" = String, Path)),
    responses((status = 200, body = ProgressEventDto), (status = 404, body = contracts::ErrorDto),
        (status = 409, body = contracts::ErrorDto)))]
async fn undo_progress(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(event_id): Path<String>,
) -> ApiResult<Json<ProgressEventDto>> {
    Ok(Json(crate::browse::undo(&s, a, &event_id).await?))
}

#[derive(Deserialize, IntoParams)]
struct ActivityParams {
    /// `next` of the previous page.
    before: Option<String>,
    /// 1 to 100, default 20.
    limit: Option<u32>,
    group: Option<String>,
    kind: Option<String>,
    /// Companion id.
    person: Option<String>,
    /// `progress` or `session`.
    row_type: Option<String>,
    /// Only sessions shared with you.
    #[serde(default)]
    shared: bool,
    year: Option<i32>,
}

/// The activity feed (Home, History), newest first.
#[utoipa::path(get, path = "/activity", tag = "library", params(ActivityParams),
    responses((status = 200, body = contracts::ActivityPageDto)))]
async fn activity(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<ActivityParams>,
) -> ApiResult<Json<contracts::ActivityPageDto>> {
    let q = contracts::ActivityQueryDto {
        before: p.before,
        limit: p.limit,
        group: p.group,
        kind: p.kind,
        person: p.person,
        row_type: p.row_type,
        shared: p.shared,
        year: p.year,
    };
    Ok(Json(crate::browse::activity(&s, a, q).await?))
}

/// All-time totals overall and per group.
#[utoipa::path(get, path = "/stats/totals", tag = "stats",
    responses((status = 200, body = contracts::StatsTotalsDto)))]
async fn stats_totals(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<contracts::StatsTotalsDto>> {
    Ok(Json(crate::browse::stats_totals(&s, a).await?))
}

#[derive(Deserialize, IntoParams)]
struct SeriesParams {
    /// Group id or `all` (default).
    group: Option<String>,
    /// `month` (default) or `year`.
    bucket: Option<String>,
}

/// Activity over time per month or year.
#[utoipa::path(get, path = "/stats/series", tag = "stats", params(SeriesParams),
    responses((status = 200, body = contracts::StatsSeriesDto)))]
async fn stats_series(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<SeriesParams>,
) -> ApiResult<Json<contracts::StatsSeriesDto>> {
    Ok(Json(
        crate::browse::stats_series(&s, a, p.group, p.bucket).await?,
    ))
}

/// Everything the signed-in Home page shows.
#[utoipa::path(get, path = "/home", tag = "library",
    responses((status = 200, body = contracts::HomeDto)))]
async fn home(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<contracts::HomeDto>> {
    Ok(Json(crate::browse::home(&s, a).await?))
}

/// The item page: item, your entry, effective history, attribution, tags, next step.
#[utoipa::path(get, path = "/items/{id}/page", tag = "catalog",
    params(("id" = String, Path)), responses((status = 200, body = contracts::ItemPageDto)))]
async fn item_page(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<contracts::ItemPageDto>> {
    Ok(Json(ops::item_page(&s, a, &id).await?))
}

/// Correct the page total of a book (or another paged item). The value replaces the
/// source's estimate and survives its refreshes. A change of the shared item: every member
/// on a home instance, admins only on a hosted one.
#[utoipa::path(put, path = "/items/{id}/page-count", tag = "catalog", request_body = contracts::PageCountDto,
    params(("id" = String, Path)), responses((status = 200, body = CatalogItemDto),
        (status = 403, body = contracts::ErrorDto), (status = 422, body = contracts::ErrorDto)))]
async fn set_page_count(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<contracts::PageCountDto>,
) -> ApiResult<Json<CatalogItemDto>> {
    Ok(Json(ops::set_page_count(&s, a, &id, b.pages).await?))
}

#[utoipa::path(put, path = "/auth/me/time-zone", tag = "auth", request_body = contracts::TimeZoneDto,
    responses((status = 200, body = UserDto), (status = 422, body = contracts::ErrorDto)))]
async fn set_time_zone(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<contracts::TimeZoneDto>,
) -> ApiResult<Json<UserDto>> {
    Ok(Json(sharing::set_time_zone(&s, a, b).await?))
}

/// Use a one-time password reset link (printed by `cli admin reset-password`).
#[utoipa::path(post, path = "/auth/reset-password", tag = "auth",
    request_body = contracts::ResetPasswordDto,
    responses((status = 204), (status = 404, body = contracts::ErrorDto),
        (status = 422, body = contracts::ErrorDto)))]
async fn reset_password(
    State(s): State<ApiState>,
    Json(b): Json<contracts::ResetPasswordDto>,
) -> ApiResult<StatusCode> {
    sharing::reset_password(&s, b).await?;
    Ok(StatusCode::NO_CONTENT)
}
