//! OIDC routes (phase 3 WP3-2): start and callback for every provider, "Is this
//! your account?" (addendum-1 4.2 step 3), linked sign-ins under Settings > Account, the
//! public button list and the admin's provider API. Handlers call `auth::oidc` and
//! `crate::admin`; failures of the browser flow are redirects to the sign-in page
//! (`/sign-in?error=<code>`) or, in link mode, back to Settings.

use auth::oidc::{Callback, Failure, FailureCode, LinkError, Mode, Outcome};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header::{CACHE_CONTROL, LOCATION, SET_COOKIE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use contracts::admin::{
    LinkedIdentityDto, OidcAdminDto, OidcButtonDto, OidcLinkConfirmDto, OidcLinkPendingDto,
    OidcProviderDto, OidcProviderInputDto, OidcSecretDto, OidcTestDto,
};
use contracts::{ErrorDto, SessionDto};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::admin::{self, Caller};
use crate::error::ApiResult;
use crate::extract::{cookie, session_token};
use crate::{ApiError, ApiState, Authed, AuthedToken};

/// Where the pending cookies live (start, callback and the link ticket routes).
const COOKIE_PATH: &str = "/api/v1/auth/oidc";

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(oidc_providers))
        .routes(routes!(oidc_start))
        .routes(routes!(oidc_callback))
        .routes(routes!(link_pending, confirm_link))
        .routes(routes!(my_identities))
        .routes(routes!(unlink_identity))
        .routes(routes!(admin_oidc))
        .routes(routes!(create_provider))
        .routes(routes!(update_provider, delete_provider))
        .routes(routes!(set_provider_secret))
        .routes(routes!(test_provider))
}

pub(crate) fn oidc(s: &ApiState) -> ApiResult<&auth::oidc::Oidc> {
    s.oidc
        .as_deref()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "OIDC is not configured"))
}

pub(crate) fn set_cookie(s: &ApiState, name: &str, value: &str, max_age: i64) -> HeaderValue {
    let secure = if s.cookie_secure() { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{name}={value}; Path={COOKIE_PATH}; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// A redirect that is never cached; the pending cookie is always cleared.
pub(crate) fn redirect(s: &ApiState, to: &str) -> Response {
    let mut r = StatusCode::FOUND.into_response();
    let h = r.headers_mut();
    if let Ok(v) = HeaderValue::from_str(to) {
        h.insert(LOCATION, v);
    }
    h.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.append(SET_COOKIE, set_cookie(s, &s.naming.oidc_cookie, "", 0));
    r
}

/// `path` with one more query parameter.
pub(crate) fn with_query(path: &str, key: &str, value: &str) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{path}{sep}{key}={value}")
}

/// Where a failed flow lands (the codes are `FailureCode::as_str`): Settings for `link`,
/// the gated page for `reauth` (`reauth_error=`) and the probes (`probe_error=`), else
/// the sign-in page, with `retry=0` once the outage retry cap is reached (addendum-1 4.4).
pub(crate) fn failure_location(f: &Failure) -> String {
    let code = f.code.as_str();
    match (f.mode, f.return_to.as_deref()) {
        (Mode::Link, _) => format!("/settings/account?error={code}"),
        (Mode::Reauth, Some(back)) => with_query(back, "reauth_error", code),
        (Mode::ProbeSilent | Mode::ProbeMaxAge, Some(back)) => {
            with_query(back, "probe_error", code)
        }
        _ if f.capped => format!("/sign-in?error={code}&retry=0"),
        _ => format!("/sign-in?error={code}"),
    }
}

/// A cookie for every path (the page middleware reads it), `HttpOnly`, `SameSite=Lax`.
pub(crate) fn site_cookie(s: &ApiState, name: &str, value: &str, max_age: i64) -> HeaderValue {
    let secure = if s.cookie_secure() { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// The redirect to the provider with its pending cookie.
pub(crate) fn to_provider(s: &ApiState, started: &auth::oidc::Started) -> Response {
    let mut r = redirect(s, &started.url);
    let max_age = auth::oidc::PENDING_MINUTES * 60;
    r.headers_mut().insert(
        SET_COOKIE,
        set_cookie(s, &s.naming.oidc_cookie, &started.cookie, max_age),
    );
    r
}

/// The session cookie and the token of a request (cookie or bearer).
async fn signed_in(
    s: &ApiState,
    parts: &axum::http::request::Parts,
) -> ApiResult<Option<(domain::user::Actor, String)>> {
    let Some(token) = session_token(parts, &s.naming.session_cookie) else {
        return Ok(None);
    };
    Ok(s.auth.session(&token).await?.map(|a| (a, token)))
}

/// The sign-in page's provider buttons (enabled providers).
#[utoipa::path(get, path = "/auth/oidc/providers", tag = "auth",
    responses((status = 200, body = Vec<OidcButtonDto>)))]
async fn oidc_providers(State(s): State<ApiState>) -> ApiResult<Json<Vec<OidcButtonDto>>> {
    Ok(Json(admin::oidc_buttons(&s).await?))
}

#[derive(Deserialize, IntoParams)]
struct StartQuery {
    /// Provider slug; default: the default (or only) provider.
    provider: Option<String>,
    /// Local path to return to.
    next: Option<String>,
    /// `interactive` (default); `silent` (`prompt=none`); `link` (signed in, fresh proof);
    /// `reauth` (signed in, `max_age=0`, DEC-191); `probe_silent` / `probe_max_age` (admins,
    /// the provider Test's round trips, OD-19).
    mode: Option<String>,
}

fn unauthorized() -> ApiError {
    ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "sign in first")
}

/// Redirect to the provider with a signed, single-use pending cookie (PKCE S256).
#[utoipa::path(get, path = "/auth/oidc/start", tag = "auth", params(StartQuery),
    responses((status = 302), (status = 400, body = ErrorDto), (status = 401, body = ErrorDto),
              (status = 403, body = ErrorDto), (status = 404, body = ErrorDto),
              (status = 409, body = ErrorDto)))]
async fn oidc_start(
    State(s): State<ApiState>,
    Query(q): Query<StartQuery>,
    req: axum::extract::Request,
) -> ApiResult<Response> {
    let o = oidc(&s)?;
    let mode = match q.mode.as_deref() {
        None => Mode::Interactive,
        Some(m) => Mode::parse(m).ok_or_else(|| ApiError::bad_request("unknown mode"))?,
    };
    let (parts, _) = req.into_parts();
    let session = signed_in(&s, &parts).await?;
    let (slug, next) = (q.provider.as_deref(), q.next.as_deref());
    let started = match mode {
        Mode::Interactive | Mode::Silent => o.start(slug, mode, next, None).await,
        Mode::Link => {
            let (_, token) = session.ok_or_else(unauthorized)?;
            // linking a sign-in is gated like the other sign-in changes (DEC-191)
            let user = s.auth.require_fresh(&token).await?.user_id;
            o.start(slug, mode, next, Some(user)).await
        }
        Mode::Reauth => {
            let (a, token) = session.ok_or_else(unauthorized)?;
            o.start_reauth(&s.auth, a.user_id, &token, slug, next).await
        }
        Mode::ProbeSilent | Mode::ProbeMaxAge => {
            let (a, _) = session.ok_or_else(unauthorized)?;
            crate::admin::require_admin(a)?;
            o.start(slug, mode, next, Some(a.user_id)).await
        }
    };
    match started {
        Ok(started) => Ok(to_provider(&s, &started)),
        Err(f) if f.code == FailureCode::NotFound => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no such sign-in provider",
        )),
        Err(f) if f.code == FailureCode::NoLinkedProvider => Err(ApiError::new(
            StatusCode::CONFLICT,
            "no_linked_provider",
            "this account has no sign-in at an enabled provider",
        )),
        Err(f) => {
            tracing::info!(code = f.code.as_str(), detail = %f.detail, "oidc start failed");
            Ok(redirect(&s, &failure_location(&f)))
        }
    }
}

#[derive(Deserialize, IntoParams)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    /// RFC 9207 issuer identifier.
    iss: Option<String>,
    error: Option<String>,
}

/// The provider's redirect back (one URI for every provider). Answers a redirect: the
/// return path (signed in; `?reauth=ok` after a re-authentication, `?probe=<outcome>`
/// after a probe), the provider again (a silent attempt that needs the person), or the
/// failure location ([`failure_location`]).
#[utoipa::path(get, path = "/auth/oidc/callback", tag = "auth", params(CallbackQuery),
    responses((status = 302)))]
async fn oidc_callback(
    State(s): State<ApiState>,
    Query(q): Query<CallbackQuery>,
    req: axum::extract::Request,
) -> ApiResult<Response> {
    let o = oidc(&s)?;
    let (parts, _) = req.into_parts();
    let pending = cookie(&parts, &s.naming.oidc_cookie);
    let session = signed_in(&s, &parts).await?;
    let cb = Callback {
        code: q.code,
        state: q.state,
        iss: q.iss,
        error: q.error,
    };
    let outcome = o
        .callback(
            &s.auth,
            pending.as_deref(),
            &cb,
            session.as_ref().map(|(a, t)| (a.user_id, t.as_str())),
        )
        .await;
    let days = i64::from(s.auth.settings().session_days);
    Ok(match outcome {
        Ok(Outcome::SignedIn {
            token,
            return_to,
            hint,
        }) => {
            let mut resp = redirect(&s, &return_to);
            let headers = resp.headers_mut();
            headers.append(
                SET_COOKIE,
                crate::routes::session_cookie(&s, &token.0, days * 86_400),
            );
            if let Some(hint) = hint {
                let age = auth::oidc::HINT_DAYS * 86_400;
                headers.append(
                    SET_COOKIE,
                    site_cookie(&s, &s.naming.oidc_hint_cookie, &hint, age),
                );
            }
            resp
        }
        Ok(Outcome::Reauthed { token, return_to }) => {
            let mut r = redirect(&s, &with_query(&return_to, "reauth", "ok"));
            r.headers_mut().append(
                SET_COOKIE,
                crate::routes::session_cookie(&s, &token.0, days * 86_400),
            );
            r
        }
        Ok(Outcome::Restart(started)) => to_provider(&s, &started),
        Ok(Outcome::Probe { return_to, result }) => {
            redirect(&s, &with_query(&return_to, "probe", &result))
        }
        Ok(Outcome::Linked { return_to }) => redirect(&s, &return_to),
        Ok(Outcome::ConfirmLink { ticket }) => {
            let mut r = redirect(&s, "/sign-in/link");
            let max_age = auth::oidc::PENDING_MINUTES * 60;
            r.headers_mut().append(
                SET_COOKIE,
                set_cookie(&s, &s.naming.oidc_link_cookie, &ticket, max_age),
            );
            r
        }
        Err(f) => redirect(&s, &failure_location(&f)),
    })
}

fn link_error(e: LinkError) -> ApiError {
    match e {
        LinkError::Ticket => ApiError::new(
            StatusCode::BAD_REQUEST,
            "link_expired",
            "this link request expired; sign in again",
        ),
        LinkError::InUse => ApiError::new(
            StatusCode::CONFLICT,
            "identity_in_use",
            "this sign-in is linked to another account",
        ),
        LinkError::Auth(e) => e.into(),
    }
}

/// "Is this your account?": the account the provider's verified email matched.
#[utoipa::path(get, path = "/auth/oidc/link", tag = "auth",
    responses((status = 200, body = OidcLinkPendingDto), (status = 400, body = ErrorDto)))]
async fn link_pending(
    State(s): State<ApiState>,
    req: axum::extract::Request,
) -> ApiResult<Json<OidcLinkPendingDto>> {
    let o = oidc(&s)?;
    let (parts, _) = req.into_parts();
    let ticket = cookie(&parts, &s.naming.oidc_link_cookie);
    let (_, user, provider, has_password) = o
        .link_pending(&s.auth, ticket.as_deref())
        .await
        .map_err(link_error)?;
    Ok(Json(OidcLinkPendingDto {
        username: user.username,
        display_name: user.display_name,
        provider: provider.display_name,
        has_password,
    }))
}

/// Confirm with the account's password (TOTP when enrolled): link and sign in.
#[utoipa::path(post, path = "/auth/oidc/link", tag = "auth", request_body = OidcLinkConfirmDto,
    responses((status = 200, body = SessionDto), (status = 400, body = ErrorDto),
              (status = 401, body = ErrorDto), (status = 409, body = ErrorDto)))]
async fn confirm_link(
    State(s): State<ApiState>,
    req: axum::extract::Request,
) -> ApiResult<Response> {
    let o = oidc(&s)?;
    let (parts, body) = req.into_parts();
    let ticket = cookie(&parts, &s.naming.oidc_link_cookie);
    let bytes = axum::body::to_bytes(body, crate::routes::JSON_BODY_LIMIT)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let b: OidcLinkConfirmDto =
        serde_json::from_slice(&bytes).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let (token, _) = o
        .confirm_link(&s.auth, ticket.as_deref(), &b.password, b.totp.as_deref())
        .await
        .map_err(link_error)?;
    let session =
        s.auth.session(&token.0).await?.ok_or_else(|| {
            ApiError::new(StatusCode::UNAUTHORIZED, "unauthorized", "not signed in")
        })?;
    let user = crate::dto::user(&s.app.user(session.user_id).await?);
    let days = i64::from(s.auth.settings().session_days);
    let mut r = Json(SessionDto {
        token: token.0.clone(),
        user,
    })
    .into_response();
    r.headers_mut().append(
        SET_COOKIE,
        crate::routes::session_cookie(&s, &token.0, days * 86_400),
    );
    r.headers_mut().append(
        SET_COOKIE,
        set_cookie(&s, &s.naming.oidc_link_cookie, "", 0),
    );
    Ok(r)
}

/// Settings > Account > Linked sign-ins.
#[utoipa::path(get, path = "/auth/me/identities", tag = "auth",
    responses((status = 200, body = Vec<LinkedIdentityDto>)))]
async fn my_identities(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<LinkedIdentityDto>>> {
    Ok(Json(admin::linked_identities(&s, a.user_id).await?))
}

/// Unlink a provider (gated); refused when no way to sign in would remain.
#[utoipa::path(delete, path = "/auth/me/identities/{id}", tag = "auth",
    params(("id" = String, Path)),
    responses((status = 204), (status = 403, body = ErrorDto), (status = 409, body = ErrorDto)))]
async fn unlink_identity(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    admin::unlink_identity(
        &s,
        Caller {
            actor: t.actor,
            token: &t.token,
        },
        &id,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn caller(t: &AuthedToken) -> Caller<'_> {
    Caller {
        actor: t.actor,
        token: &t.token,
    }
}

/// Admin > Sign-in and SSO: providers and the URIs to register.
#[utoipa::path(get, path = "/admin/oidc", tag = "admin",
    responses((status = 200, body = OidcAdminDto), (status = 403, body = ErrorDto)))]
async fn admin_oidc(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<OidcAdminDto>> {
    Ok(Json(admin::oidc_admin(&s, a).await?))
}

/// Add a provider (re-auth gated, audited).
#[utoipa::path(post, path = "/admin/oidc/providers", tag = "admin",
    request_body = OidcProviderInputDto,
    responses((status = 201, body = OidcProviderDto), (status = 403, body = ErrorDto),
              (status = 409, body = ErrorDto), (status = 422, body = ErrorDto)))]
async fn create_provider(
    State(s): State<ApiState>,
    t: AuthedToken,
    Json(b): Json<OidcProviderInputDto>,
) -> ApiResult<(StatusCode, Json<OidcProviderDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(admin::create_provider(&s, caller(&t), b).await?),
    ))
}

/// Change a provider (re-auth gated, audited); the environment provider is read-only.
#[utoipa::path(put, path = "/admin/oidc/providers/{id}", tag = "admin",
    params(("id" = String, Path)), request_body = OidcProviderInputDto,
    responses((status = 200, body = OidcProviderDto), (status = 409, body = ErrorDto),
              (status = 422, body = ErrorDto)))]
async fn update_provider(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
    Json(b): Json<OidcProviderInputDto>,
) -> ApiResult<Json<OidcProviderDto>> {
    Ok(Json(admin::update_provider(&s, caller(&t), &id, b).await?))
}

/// Remove a provider (re-auth gated, audited).
#[utoipa::path(delete, path = "/admin/oidc/providers/{id}", tag = "admin",
    params(("id" = String, Path)),
    responses((status = 204), (status = 409, body = ErrorDto)))]
async fn delete_provider(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    admin::delete_provider(&s, caller(&t), &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Replace or remove the client secret (write-only, encrypted, re-auth gated).
#[utoipa::path(put, path = "/admin/oidc/providers/{id}/secret", tag = "admin",
    params(("id" = String, Path)), request_body = OidcSecretDto,
    responses((status = 200, body = OidcProviderDto), (status = 409, body = ErrorDto)))]
async fn set_provider_secret(
    State(s): State<ApiState>,
    t: AuthedToken,
    Path(id): Path<String>,
    Json(b): Json<OidcSecretDto>,
) -> ApiResult<Json<OidcProviderDto>> {
    Ok(Json(
        admin::set_provider_secret(&s, caller(&t), &id, b).await?,
    ))
}

/// Test a provider: discovery, logout capabilities, groups claim seen.
#[utoipa::path(post, path = "/admin/oidc/providers/{id}/test", tag = "admin",
    params(("id" = String, Path)),
    responses((status = 200, body = OidcTestDto), (status = 404, body = ErrorDto)))]
async fn test_provider(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<OidcTestDto>> {
    Ok(Json(admin::test_provider(&s, a, &id).await?))
}
