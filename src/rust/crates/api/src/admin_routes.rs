//! REST routes of UX phase 2 (`/api/v1/admin/...`, `/sources/status`, `/setup`,
//! `/auth/reauth`, `/me/credentials`). Handlers only call `crate::admin`.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use contracts::admin::{
    AdminInviteCreatedDto, AdminInviteDto, AdminOverviewDto, AdminSourceDto, AdminUserDto,
    AskAdminDto, AuditPageDto, AuditQueryDto, BrandCardDto, CredentialInputDto, DisabledInputDto,
    EvictionDto, HostingInputDto, InstanceSettingsDto, LicenceInputDto, NewAdminInviteDto,
    NotificationDefaultsDto, ReauthInputDto, ReauthStatusDto, RewrapDto, RoleInputDto, SecurityDto,
    SettingsPatchDto, SetupStateDto, SourceStatusPageDto, SourceTestDto, SourceTestInputDto,
    StorageJobsDto, UserCredentialDto,
};
use contracts::admin::{
    MailPasswordDto, MailSettingsDto, MailTestDto, MailTestInputDto, SourceEnabledDto,
};
use contracts::{ErrorDto, SessionDto, SignUpDto};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::admin::{self, Caller};
use crate::error::ApiResult;
use crate::{ApiState, Authed, AuthedToken};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(source_status))
        .routes(routes!(ask_admin))
        .routes(routes!(my_credentials))
        .routes(routes!(set_my_credential, clear_my_credential))
        .routes(routes!(reauth_status, reauth))
        .routes(routes!(setup_state))
        .routes(routes!(setup_account))
        .routes(routes!(setup_hosting))
        .routes(routes!(setup_sources))
        .routes(routes!(setup_complete))
        .routes(routes!(overview))
        .routes(routes!(settings, update_settings))
        .routes(routes!(users))
        .routes(routes!(set_role))
        .routes(routes!(set_disabled))
        .routes(routes!(delete_user))
        .routes(routes!(reset_user_totp))
        .routes(routes!(unlink_user_identity))
        .routes(routes!(invites, create_invite))
        .routes(routes!(revoke_invite))
        .routes(routes!(sources))
        .routes(routes!(set_credential, clear_credential))
        .routes(routes!(test_source))
        .routes(routes!(set_licence))
        .routes(routes!(set_source_enabled))
        .routes(routes!(mail))
        .routes(routes!(set_mail_password))
        .routes(routes!(send_test_mail))
        .routes(routes!(storage_jobs))
        .routes(routes!(run_eviction))
        .routes(routes!(retry_job))
        .routes(routes!(discard_job))
        .routes(routes!(notification_defaults))
        .routes(routes!(brand))
        .routes(routes!(security))
        .routes(routes!(rewrap))
        .routes(routes!(audit))
        .routes(routes!(audit_csv))
}

fn caller(t: &AuthedToken) -> Caller<'_> {
    Caller {
        actor: t.actor,
        token: &t.token,
    }
}

fn with_cookie(s: &ApiState, token: &str, body: impl IntoResponse) -> Response {
    let days = i64::from(s.auth.settings().session_days);
    let secure = if s.cookie_secure() { "; Secure" } else { "" };
    let v = format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{secure}",
        s.naming.session_cookie,
        days * 86_400
    );
    let mut r = body.into_response();
    if let Ok(v) = HeaderValue::from_str(&v) {
        r.headers_mut().append(SET_COOKIE, v);
    }
    r
}

// ---------------------------------------------------------------- sources

/// Every data source with its state (design.md 4.4); members get the short vocabulary.
#[utoipa::path(get, path = "/sources/status", tag = "sources",
    responses((status = 200, body = SourceStatusPageDto)))]
async fn source_status(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<SourceStatusPageDto>> {
    Ok(Json(admin::source_status(&s, a).await?))
}

/// "Ask admin": notify the admins (`admin.source_requested`, once per source and day).
#[utoipa::path(post, path = "/sources/ask-admin", tag = "sources", request_body = AskAdminDto,
    responses((status = 204), (status = 404, body = ErrorDto)))]
async fn ask_admin(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<AskAdminDto>,
) -> ApiResult<StatusCode> {
    admin::ask_admin(&s, a, b).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The member's own credentials (`TheTVDB` PIN); never shown to admins.
#[utoipa::path(get, path = "/me/credentials", tag = "sources",
    responses((status = 200, body = Vec<UserCredentialDto>)))]
async fn my_credentials(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<UserCredentialDto>>> {
    Ok(Json(admin::my_credentials(&s, a).await?))
}

#[utoipa::path(put, path = "/me/credentials/{source}", tag = "sources",
    params(("source" = String, Path)), request_body = CredentialInputDto,
    responses((status = 200, body = Vec<UserCredentialDto>), (status = 422, body = ErrorDto)))]
async fn set_my_credential(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(source): Path<String>,
    Json(b): Json<CredentialInputDto>,
) -> ApiResult<Json<Vec<UserCredentialDto>>> {
    Ok(Json(admin::set_my_credential(&s, a, &source, b).await?))
}

#[utoipa::path(delete, path = "/me/credentials/{source}", tag = "sources",
    params(("source" = String, Path)), responses((status = 200, body = Vec<UserCredentialDto>)))]
async fn clear_my_credential(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(source): Path<String>,
) -> ApiResult<Json<Vec<UserCredentialDto>>> {
    Ok(Json(admin::clear_my_credential(&s, a, &source).await?))
}

// ---------------------------------------------------------------- re-authentication

/// Is the session's proof fresh (DEC-191: 15 minutes sliding, 1 hour cap)?
#[utoipa::path(get, path = "/auth/reauth", tag = "auth",
    responses((status = 200, body = ReauthStatusDto)))]
async fn reauth_status(
    State(s): State<ApiState>,
    t: AuthedToken,
) -> ApiResult<Json<ReauthStatusDto>> {
    Ok(Json(admin::reauth_status(&s, &t.token).await?))
}

/// Prove it is you (password, then TOTP or a recovery code). The session id is
/// regenerated: the answer sets a new session cookie.
#[utoipa::path(post, path = "/auth/reauth", tag = "auth", request_body = ReauthInputDto,
    responses((status = 200, body = ReauthStatusDto), (status = 401, body = ErrorDto),
        (status = 429, body = ErrorDto)))]
async fn reauth(
    State(s): State<ApiState>,
    t: AuthedToken,
    Json(b): Json<ReauthInputDto>,
) -> ApiResult<Response> {
    let (status, token) = admin::reauth(&s, &t.token, b).await?;
    Ok(with_cookie(&s, &token, Json(status)))
}

// ---------------------------------------------------------------- setup wizard

async fn maybe_actor(s: &ApiState, headers: &HeaderMap) -> Option<domain::user::Actor> {
    let token = crate::extract::cookie_or_bearer(headers, &s.naming.session_cookie)?;
    s.auth.session(&token).await.ok().flatten()
}

/// Where the setup wizard stands (no session needed before the first account exists);
/// 404 `setup_done` once completed.
#[utoipa::path(get, path = "/setup", tag = "setup",
    responses((status = 200, body = SetupStateDto), (status = 404, body = ErrorDto)))]
async fn setup_state(
    State(s): State<ApiState>,
    headers: HeaderMap,
) -> ApiResult<Json<SetupStateDto>> {
    let actor = maybe_actor(&s, &headers).await;
    Ok(Json(admin::setup_state(&s, actor).await?))
}

/// Step 1: create the first account (admin) and sign in.
#[utoipa::path(post, path = "/setup/account", tag = "setup", request_body = SignUpDto,
    responses((status = 201, body = SessionDto), (status = 409, body = ErrorDto)))]
async fn setup_account(State(s): State<ApiState>, Json(b): Json<SignUpDto>) -> ApiResult<Response> {
    let session = admin::setup_account(&s, b).await?;
    let token = session.token.clone();
    Ok(with_cookie(
        &s,
        &token,
        (StatusCode::CREATED, Json(session)),
    ))
}

#[utoipa::path(post, path = "/setup/hosting", tag = "setup", request_body = HostingInputDto,
    responses((status = 200, body = SetupStateDto)))]
async fn setup_hosting(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<HostingInputDto>,
) -> ApiResult<Json<SetupStateDto>> {
    Ok(Json(admin::setup_hosting(&s, a, b).await?))
}

#[utoipa::path(post, path = "/setup/sources", tag = "setup",
    responses((status = 200, body = SetupStateDto)))]
async fn setup_sources(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<SetupStateDto>> {
    Ok(Json(admin::setup_sources(&s, a).await?))
}

#[utoipa::path(post, path = "/setup/complete", tag = "setup", responses((status = 204)))]
async fn setup_complete(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<StatusCode> {
    admin::setup_complete(&s, a).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- admin pages

#[utoipa::path(get, path = "/admin/overview", tag = "admin",
    responses((status = 200, body = AdminOverviewDto), (status = 403, body = ErrorDto)))]
async fn overview(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<AdminOverviewDto>> {
    Ok(Json(admin::overview(&s, a).await?))
}

#[utoipa::path(get, path = "/admin/settings", tag = "admin",
    responses((status = 200, body = InstanceSettingsDto)))]
async fn settings(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<InstanceSettingsDto>> {
    Ok(Json(admin::settings(&s, a).await?))
}

/// Change settings (`null` resets to the default). Sign-in related keys need a fresh
/// proof (`reauth_required`); env/file keys are refused (`fixed_by_config`).
#[utoipa::path(put, path = "/admin/settings", tag = "admin", request_body = SettingsPatchDto,
    responses((status = 200, body = InstanceSettingsDto), (status = 403, body = ErrorDto),
        (status = 409, body = ErrorDto), (status = 422, body = ErrorDto)))]
async fn update_settings(
    State(s): State<ApiState>,
    t: AuthedToken,
    Json(b): Json<SettingsPatchDto>,
) -> ApiResult<Json<InstanceSettingsDto>> {
    Ok(Json(admin::update_settings(&s, caller(&t), b).await?))
}

#[utoipa::path(get, path = "/admin/users", tag = "admin",
    responses((status = 200, body = Vec<AdminUserDto>)))]
async fn users(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<Vec<AdminUserDto>>> {
    Ok(Json(admin::users(&s, a).await?))
}

/// Grant or revoke admin (fresh proof; `self_role_change`, `last_admin`).
#[utoipa::path(put, path = "/admin/users/{id}/role", tag = "admin", params(("id" = String, Path)),
    request_body = RoleInputDto, responses((status = 200, body = AdminUserDto), (status = 409, body = ErrorDto)))]
async fn set_role(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
    Json(b): Json<RoleInputDto>,
) -> ApiResult<Json<AdminUserDto>> {
    Ok(Json(admin::set_role(&s, caller(&t), &id, &b.role).await?))
}

/// Disable or enable an account (fresh proof; `self_disable`, `last_admin`).
#[utoipa::path(put, path = "/admin/users/{id}/disabled", tag = "admin", params(("id" = String, Path)),
    request_body = DisabledInputDto, responses((status = 200, body = AdminUserDto), (status = 409, body = ErrorDto)))]
async fn set_disabled(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
    Json(b): Json<DisabledInputDto>,
) -> ApiResult<Json<AdminUserDto>> {
    Ok(Json(
        admin::set_disabled(&s, caller(&t), &id, b.disabled).await?,
    ))
}

/// Delete an account (fresh proof; `self_delete`, `last_admin`).
#[utoipa::path(delete, path = "/admin/users/{id}", tag = "admin", params(("id" = String, Path)),
    responses((status = 204), (status = 409, body = ErrorDto)))]
async fn delete_user(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    admin::delete_user(&s, caller(&t), &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Users row menu: remove the person's authenticator (fresh proof; `no_authenticator`).
#[utoipa::path(post, path = "/admin/users/{id}/reset-totp", tag = "admin", params(("id" = String, Path)),
    responses((status = 200, body = AdminUserDto), (status = 403, body = ErrorDto), (status = 409, body = ErrorDto)))]
async fn reset_user_totp(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
) -> ApiResult<Json<AdminUserDto>> {
    Ok(Json(admin::reset_user_totp(&s, caller(&t), &id).await?))
}

/// Users row menu: unlink one of the person's providers (fresh proof; `last_sign_in_method`).
#[utoipa::path(delete, path = "/admin/users/{id}/identities/{identity}", tag = "admin",
    params(("id" = String, Path), ("identity" = String, Path)),
    responses((status = 204), (status = 403, body = ErrorDto), (status = 404, body = ErrorDto), (status = 409, body = ErrorDto)))]
async fn unlink_user_identity(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path((id, identity)): Path<(String, String)>,
) -> ApiResult<StatusCode> {
    admin::unlink_user_identity(&s, caller(&t), &id, &identity).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/admin/invites", tag = "admin",
    responses((status = 200, body = Vec<AdminInviteDto>)))]
async fn invites(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<AdminInviteDto>>> {
    Ok(Json(admin::invites(&s, a).await?))
}

#[utoipa::path(post, path = "/admin/invites", tag = "admin", request_body = NewAdminInviteDto,
    responses((status = 201, body = AdminInviteCreatedDto)))]
async fn create_invite(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NewAdminInviteDto>,
) -> ApiResult<(StatusCode, Json<AdminInviteCreatedDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(admin::create_invite(&s, a, b).await?),
    ))
}

#[utoipa::path(delete, path = "/admin/invites/{id}", tag = "admin", params(("id" = String, Path)),
    responses((status = 200, body = AdminInviteDto)))]
async fn revoke_invite(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<AdminInviteDto>> {
    Ok(Json(admin::revoke_invite(&s, a, &id).await?))
}

#[utoipa::path(get, path = "/admin/sources", tag = "admin",
    responses((status = 200, body = Vec<AdminSourceDto>)))]
async fn sources(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<AdminSourceDto>>> {
    Ok(Json(admin::sources(&s, a).await?))
}

/// Replace the instance key (write-only; encrypted; takes effect without a restart).
#[utoipa::path(put, path = "/admin/sources/{id}/credential", tag = "admin", params(("id" = String, Path)),
    request_body = CredentialInputDto, responses((status = 200, body = AdminSourceDto), (status = 422, body = ErrorDto)))]
async fn set_credential(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<CredentialInputDto>,
) -> ApiResult<Json<AdminSourceDto>> {
    Ok(Json(admin::set_source_credential(&s, a, &id, b).await?))
}

#[utoipa::path(delete, path = "/admin/sources/{id}/credential", tag = "admin", params(("id" = String, Path)),
    responses((status = 200, body = AdminSourceDto)))]
async fn clear_credential(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<AdminSourceDto>> {
    Ok(Json(admin::clear_source_credential(&s, a, &id).await?))
}

/// Test the given fields (before saving) or the stored key.
#[utoipa::path(post, path = "/admin/sources/{id}/test", tag = "admin", params(("id" = String, Path)),
    request_body = SourceTestInputDto, responses((status = 200, body = SourceTestDto)))]
async fn test_source(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<SourceTestInputDto>,
) -> ApiResult<Json<SourceTestDto>> {
    Ok(Json(admin::test_source(&s, a, &id, b).await?))
}

#[utoipa::path(put, path = "/admin/sources/{id}/licence", tag = "admin", params(("id" = String, Path)),
    request_body = LicenceInputDto, responses((status = 200, body = AdminSourceDto)))]
async fn set_licence(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<LicenceInputDto>,
) -> ApiResult<Json<AdminSourceDto>> {
    Ok(Json(admin::set_source_licence(&s, a, &id, b).await?))
}

#[utoipa::path(get, path = "/admin/storage-jobs", tag = "admin",
    responses((status = 200, body = StorageJobsDto)))]
async fn storage_jobs(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<StorageJobsDto>> {
    Ok(Json(admin::storage_jobs(&s, a).await?))
}

#[utoipa::path(post, path = "/admin/storage/evict", tag = "admin",
    responses((status = 200, body = EvictionDto)))]
async fn run_eviction(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<EvictionDto>> {
    Ok(Json(admin::run_eviction(&s, a).await?))
}

#[utoipa::path(post, path = "/admin/jobs/{id}/retry", tag = "admin", params(("id" = String, Path)),
    responses((status = 204), (status = 404, body = ErrorDto)))]
async fn retry_job(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    admin::failed_job(&s, a, &id, true).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/admin/jobs/{id}", tag = "admin", params(("id" = String, Path)),
    responses((status = 204), (status = 404, body = ErrorDto)))]
async fn discard_job(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    admin::failed_job(&s, a, &id, false).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/admin/notification-defaults", tag = "admin",
    responses((status = 200, body = NotificationDefaultsDto)))]
async fn notification_defaults(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<NotificationDefaultsDto>> {
    Ok(Json(admin::notification_defaults(&s, a).await?))
}

#[utoipa::path(get, path = "/admin/brand", tag = "admin",
    responses((status = 200, body = BrandCardDto)))]
async fn brand(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<BrandCardDto>> {
    Ok(Json(admin::brand(&s, a)?))
}

#[utoipa::path(get, path = "/admin/security", tag = "admin",
    responses((status = 200, body = SecurityDto)))]
async fn security(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<SecurityDto>> {
    Ok(Json(admin::security(&s, a).await?))
}

/// "Re-encrypt now": re-wrap every value on an older key version.
#[utoipa::path(post, path = "/admin/secrets/rewrap", tag = "admin",
    responses((status = 200, body = RewrapDto)))]
async fn rewrap(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<RewrapDto>> {
    Ok(Json(admin::rewrap(&s, a).await?))
}

#[derive(Deserialize, IntoParams)]
struct AuditParams {
    actor: Option<String>,
    /// Exact action or a prefix ending in `.`.
    action: Option<String>,
    target_type: Option<String>,
    before: Option<String>,
    after: Option<String>,
    limit: Option<u32>,
}

impl From<AuditParams> for AuditQueryDto {
    fn from(p: AuditParams) -> Self {
        Self {
            actor: p.actor,
            action: p.action,
            target_type: p.target_type,
            before: p.before,
            after: p.after,
            limit: p.limit,
        }
    }
}

#[utoipa::path(get, path = "/admin/audit", tag = "admin", params(AuditParams),
    responses((status = 200, body = AuditPageDto)))]
async fn audit(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<AuditParams>,
) -> ApiResult<Json<AuditPageDto>> {
    Ok(Json(admin::audit_log(&s, a, p.into()).await?))
}

/// The same rows as CSV (Export CSV).
#[utoipa::path(get, path = "/admin/audit.csv", tag = "admin", params(AuditParams),
    responses((status = 200, content_type = "text/csv", body = String)))]
async fn audit_csv(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<AuditParams>,
) -> ApiResult<Response> {
    let csv = admin::audit_csv(&s, a, p.into()).await?;
    Ok((
        [
            (CONTENT_TYPE, "text/csv; charset=utf-8"),
            (CONTENT_DISPOSITION, "attachment; filename=\"audit.csv\""),
        ],
        csv,
    )
        .into_response())
}

#[utoipa::path(put, path = "/admin/sources/{id}/enabled", tag = "admin",
    request_body = SourceEnabledDto,
    responses((status = 200, body = AdminSourceDto), (status = 403, body = ErrorDto)))]
async fn set_source_enabled(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<SourceEnabledDto>,
) -> ApiResult<Json<AdminSourceDto>> {
    Ok(Json(
        admin::set_source_enabled(&s, a, &id, b.enabled).await?,
    ))
}

#[utoipa::path(get, path = "/admin/mail", tag = "admin",
    responses((status = 200, body = MailSettingsDto), (status = 403, body = ErrorDto)))]
async fn mail(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<MailSettingsDto>> {
    Ok(Json(admin::mail(&s, a).await?))
}

/// Replace or remove the SMTP password (write-only, encrypted, gated).
#[utoipa::path(put, path = "/admin/mail/password", tag = "admin", request_body = MailPasswordDto,
    responses((status = 200, body = MailSettingsDto), (status = 403, body = ErrorDto),
        (status = 409, body = ErrorDto)))]
async fn set_mail_password(
    State(s): State<ApiState>,
    t: AuthedToken,
    Json(b): Json<MailPasswordDto>,
) -> ApiResult<Json<MailSettingsDto>> {
    Ok(Json(admin::set_mail_password(&s, caller(&t), b).await?))
}

/// Send a test message with the settings in effect; the outcome is recorded.
#[utoipa::path(post, path = "/admin/mail/test", tag = "admin", request_body = MailTestInputDto,
    responses((status = 200, body = MailTestDto), (status = 422, body = ErrorDto)))]
async fn send_test_mail(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<MailTestInputDto>,
) -> ApiResult<Json<MailTestDto>> {
    Ok(Json(admin::send_test_mail(&s, a, b).await?))
}
