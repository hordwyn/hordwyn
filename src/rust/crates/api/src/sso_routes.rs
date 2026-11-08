//! Sign-in modes, logout and the break-glass path (phase 3 WP3-3; addendum-1 4.2 "Logout",
//! 4.3, 4.4; OD-17 as changed, OD-20; DEC-191):
//!
//! - `POST /auth/logout`: local-first sign-out, then `303` to the provider's end-session
//!   endpoint (RP-Initiated Logout 1.0) or to `/signed-out`.
//! - `POST /auth/oidc/backchannel-logout`: the provider's logout token (Back-Channel
//!   Logout 1.0); public, rate-limited, `Cache-Control: no-store`, audited.
//! - `POST /auth/sign-in/local`: password (TOTP when enrolled) for admins in `auto` mode,
//!   throttled, audited, admins notified (`admin.local_sign_in`); `POST /auth/sign-in`
//!   answers `sso_only` in `auto` mode.
//! - `PUT` / `GET /auth/reauth/pending`: the gated action kept while the browser does the
//!   `max_age=0` round trip (od-12 4.4 step 2).
//! - [`auto_redirect`]: the `302` of a signed-out page visit in `auto` mode (the server's
//!   page middleware renders the `role=status` interstitial body); `/signed-out` and the
//!   other exemptions never redirect (OD-20).

use app::ports::{Audience, Notification};
use auth::SessionToken;
use auth::oidc::LogoutTokenError;
use axum::Json;
use axum::extract::State;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use contracts::{ErrorDto, SessionDto, SignInDto};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::error::ApiResult;
use crate::oidc_routes::{failure_location, redirect, site_cookie, to_provider};
use crate::{ApiError, ApiState, AuthedToken};

/// Where the person lands after signing out (never redirected, OD-20).
pub const SIGNED_OUT_PATH: &str = "/signed-out";

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(logout))
        .routes(routes!(backchannel_logout))
        .routes(routes!(local_sign_in))
        .routes(routes!(keep_pending, take_pending))
}

fn header_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, v)| *k == name && !v.is_empty())
        .map(|(_, v)| v.to_string())
}

// ---------------------------------------------------------------- password sign-in

async fn sso_only(s: &ApiState) -> ApiError {
    let provider = match s.oidc.as_deref() {
        Some(o) => o
            .store()
            .providers()
            .await
            .ok()
            .and_then(|ps| ps.into_iter().find(|p| p.enabled && p.is_default))
            .map(|p| p.display_name),
        None => None,
    };
    ApiError::new(
        StatusCode::FORBIDDEN,
        "sso_only",
        format!(
            "sign in through {}",
            provider.as_deref().unwrap_or("the identity provider")
        ),
    )
}

/// Password sign-in with the sign-in mode applied (addendum-1 4.3): in `auto` mode the
/// normal path (`local = false`) is refused with `sso_only` for everyone, the break-glass
/// path (`local = true`) admits admins only (OD-17 as changed: TOTP only when enrolled).
/// Every successful break-glass sign-in is audited (`auth.local_sign_in`, with the client
/// address) and notifies the admins (`admin.local_sign_in`).
///
/// # Errors
/// The sign-in errors, 403 `sso_only`.
pub async fn password_sign_in(s: &ApiState, b: &SignInDto, local: bool) -> ApiResult<SessionToken> {
    let auto = crate::admin::sign_in_auto(s).await;
    if auto && !local {
        return Err(sso_only(s).await);
    }
    let token = s
        .auth
        .sign_in(&b.username, &b.password, b.totp.as_deref())
        .await?;
    if !local {
        return Ok(token);
    }
    let actor = s
        .auth
        .session(&token.0)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    if auto && !actor.is_admin() {
        s.auth.sign_out(&token.0).await?;
        return Err(sso_only(s).await);
    }
    let user = s.app.user(actor.user_id).await?;
    let address = crate::request_meta::current().and_then(|m| m.client_addr);
    let method = s
        .auth
        .reauth_status(&token.0)
        .await
        .ok()
        .and_then(|r| r.method);
    crate::admin::audit(
        s,
        Some(actor),
        "auth.local_sign_in",
        Some(("user", user.id.0.to_string())),
        json!({"username": user.username, "mode": if auto { "auto" } else { "page" },
               "method": method}),
    )
    .await;
    s.app
        .notifications
        .emit(
            Notification::new(
                "admin.local_sign_in",
                Audience::Admins,
                format!("admin.local_sign_in:{}", Uuid::now_v7()),
            )
            .arg("username", user.username.clone())
            .arg("address", address.unwrap_or_else(|| "-".into()))
            .link("/admin/audit"),
        )
        .await;
    Ok(token)
}

fn session_response(s: &ApiState, token: &SessionToken, user: contracts::UserDto) -> Response {
    let days = i64::from(s.auth.settings().session_days);
    let mut r = Json(SessionDto {
        token: token.0.clone(),
        user,
    })
    .into_response();
    r.headers_mut().append(
        SET_COOKIE,
        crate::routes::session_cookie(s, &token.0, days * 86_400),
    );
    r
}

/// Break-glass sign-in (`/sign-in/local`, OD-17 as changed): admins only in `auto` mode.
#[utoipa::path(post, path = "/auth/sign-in/local", tag = "auth", request_body = SignInDto,
    responses((status = 200, body = SessionDto), (status = 401, body = ErrorDto),
              (status = 403, body = ErrorDto), (status = 429, body = ErrorDto)))]
async fn local_sign_in(State(s): State<ApiState>, Json(b): Json<SignInDto>) -> ApiResult<Response> {
    let token = password_sign_in(&s, &b, true).await?;
    let actor = s
        .auth
        .session(&token.0)
        .await?
        .ok_or_else(ApiError::unauthorized)?;
    let user = crate::dto::user(&s.app.user(actor.user_id).await?);
    Ok(session_response(&s, &token, user))
}

// ---------------------------------------------------------------- logout

/// Local-first sign-out (addendum-1 4.2): ends the session, then answers where the browser
/// goes: the provider's end-session request, or [`SIGNED_OUT_PATH`].
///
/// # Errors
/// Storage errors.
pub async fn sign_out_location(s: &ApiState, token: &str) -> ApiResult<String> {
    let end = if let Some(o) = s.oidc.as_deref() {
        o.sign_out(&s.auth, token).await?
    } else {
        s.auth.sign_out(token).await?;
        None
    };
    Ok(end.unwrap_or_else(|| SIGNED_OUT_PATH.into()))
}

/// Sign out here first, then at the provider (RP-initiated logout) or straight to
/// `/signed-out`. A form post from the account menu; answers `303 See Other`.
#[utoipa::path(post, path = "/auth/logout", tag = "auth", responses((status = 303)))]
async fn logout(State(s): State<ApiState>, req: axum::extract::Request) -> ApiResult<Response> {
    let (parts, _) = req.into_parts();
    let to = match crate::session_token(&parts, &s.naming.session_cookie) {
        Some(t) => sign_out_location(&s, &t).await?,
        None => SIGNED_OUT_PATH.into(),
    };
    let mut r = StatusCode::SEE_OTHER.into_response();
    let h = r.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&to) {
        h.insert(LOCATION, v);
    }
    h.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.append(SET_COOKIE, crate::routes::session_cookie(&s, "", 0));
    // an explicit sign-out forgets this browser's provider hint (shared computers)
    h.append(
        SET_COOKIE,
        site_cookie(&s, &s.naming.oidc_hint_cookie, "", 0),
    );
    Ok(r)
}

fn no_store(status: StatusCode, body: Option<Value>) -> Response {
    let mut r = match body {
        Some(b) => (status, Json(b)).into_response(),
        None => status.into_response(),
    };
    r.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

/// The provider's back-channel logout (Back-Channel Logout 1.0, 2.5: a form post with
/// `logout_token`). 200 when valid (also when no session matched), 400 with
/// `invalid_request` otherwise (2.8), 429 over the per-process budget.
#[utoipa::path(post, path = "/auth/oidc/backchannel-logout", tag = "auth",
    request_body(content = String, content_type = "application/x-www-form-urlencoded"),
    responses((status = 200), (status = 400), (status = 429)))]
async fn backchannel_logout(State(s): State<ApiState>, body: axum::body::Bytes) -> Response {
    let Some(o) = s.oidc.as_deref() else {
        return no_store(StatusCode::NOT_FOUND, None);
    };
    let token = url::form_urlencoded::parse(&body)
        .find(|(k, _)| k == "logout_token")
        .map(|(_, v)| v.into_owned());
    let invalid = || {
        no_store(
            StatusCode::BAD_REQUEST,
            Some(json!({"error": "invalid_request",
                        "error_description": "the logout token is not valid"})),
        )
    };
    let Some(token) = token else {
        return invalid();
    };
    match o.backchannel_logout(&s.auth, &token).await {
        Ok(out) => {
            crate::admin::audit(
                &s,
                None,
                "oidc.backchannel_logout",
                Some(("oidc_provider", out.provider.id.to_string())),
                json!({"provider": out.provider.slug, "sessions": out.sessions,
                       "by": if out.by_sid { "sid" } else { "sub" }}),
            )
            .await;
            no_store(StatusCode::OK, None)
        }
        Err(LogoutTokenError::TooMany) => no_store(
            StatusCode::TOO_MANY_REQUESTS,
            Some(json!({"error": "too_many_requests"})),
        ),
        Err(LogoutTokenError::Invalid(d)) => {
            tracing::info!(detail = %d, "back-channel logout refused");
            invalid()
        }
        Err(LogoutTokenError::Store(e)) => {
            tracing::error!(error = %e, "back-channel logout failed");
            no_store(StatusCode::INTERNAL_SERVER_ERROR, None)
        }
    }
}

// ---------------------------------------------------------------- pending gated action

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct PendingActionDto {
    /// Whatever the page needs to show the gated action again (no secrets are needed: it
    /// is stored encrypted with the session anyway).
    action: Value,
}

/// Keep the gated action while the browser does the OIDC re-auth round trip.
#[utoipa::path(put, path = "/auth/reauth/pending", tag = "auth", request_body = PendingActionDto,
    responses((status = 204), (status = 401, body = ErrorDto), (status = 413, body = ErrorDto)))]
async fn keep_pending(
    State(s): State<ApiState>,
    t: AuthedToken,
    Json(b): Json<PendingActionDto>,
) -> ApiResult<StatusCode> {
    if b.action.to_string().len() > auth::PENDING_ACTION_MAX_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "the pending action is too large",
        ));
    }
    s.auth.keep_pending_action(&t.token, &b.action).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The kept action, once (404 when none or older than 10 minutes).
#[utoipa::path(get, path = "/auth/reauth/pending", tag = "auth",
    responses((status = 200, body = PendingActionDto), (status = 404, body = ErrorDto)))]
async fn take_pending(
    State(s): State<ApiState>,
    t: AuthedToken,
) -> ApiResult<Json<PendingActionDto>> {
    s.auth
        .take_pending_action(&t.token)
        .await?
        .map(|action| Json(PendingActionDto { action }))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "nothing pending"))
}

// ---------------------------------------------------------------- auto mode

/// The answer to a signed-out page visit in `auto` mode.
pub struct AutoRedirect {
    /// `302` with the pending (and loop guard) cookies, no body yet.
    pub response: Response,
    /// To the provider: its display name and the URL, for the interstitial body
    /// (addendum-1 4.5 state 2). `None` for the outage page.
    pub provider: Option<(String, String)>,
}

/// Pages that never go to the provider: `/signed-out` (OD-20), the break-glass and linking
/// pages, the wizard, one-time links, public shared lists, and the sign-in page when it
/// shows a provider error (addendum-1 4.3).
fn exempt(path: &str, query: &str) -> bool {
    matches!(
        path,
        SIGNED_OUT_PATH | "/sign-in/local" | "/sign-in/link" | "/setup"
    ) || ["/reset-password/", "/invite/", "/shared/"]
        .iter()
        .any(|p| path.starts_with(p))
        || (path == "/sign-in"
            && url::form_urlencoded::parse(query.as_bytes()).any(|(k, _)| k == "error"))
}

/// Sign-in mode `auto` (addendum-1 4.3, 4.4): a signed-out visit of a page goes to the
/// default provider (silent first when its probe succeeded, OD-19), keeping the path as
/// `next`; with the provider down past the retry cap it goes to the outage page instead.
/// `None`: not in `auto` mode, an exempt page, or no usable default provider.
pub async fn auto_redirect(
    s: &ApiState,
    headers: &HeaderMap,
    path_and_query: &str,
) -> Option<AutoRedirect> {
    let o = s.oidc.as_deref()?;
    let (path, query) = path_and_query
        .split_once('?')
        .unwrap_or((path_and_query, ""));
    if exempt(path, query) || !crate::admin::sign_in_auto(s).await {
        return None;
    }
    let next = if path == "/sign-in" {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == "next")
            .map_or_else(|| "/".to_string(), |(_, v)| v.into_owned())
    } else {
        path_and_query.to_string()
    };
    let tried = header_cookie(headers, &s.naming.oidc_silent_cookie).is_some();
    let hint = header_cookie(headers, &s.naming.oidc_hint_cookie);
    match o.auto_start(&next, tried, hint.as_deref()).await {
        Ok(None) => None,
        Ok(Some(a)) => {
            let mut response = to_provider(s, &a.started);
            if a.silent {
                let age = auth::oidc::SILENT_GUARD_MINUTES * 60;
                response.headers_mut().append(
                    SET_COOKIE,
                    site_cookie(s, &s.naming.oidc_silent_cookie, "1", age),
                );
            }
            Some(AutoRedirect {
                response,
                provider: Some((a.provider.display_name, a.started.url)),
            })
        }
        Err(f) => {
            tracing::info!(code = f.code.as_str(), detail = %f.detail, "auto sign-in did not start");
            Some(AutoRedirect {
                response: redirect(s, &failure_location(&f)),
                provider: None,
            })
        }
    }
}

/// Set the body of an [`AutoRedirect`] (an HTML interstitial).
#[must_use]
pub fn with_body(r: AutoRedirect, html: String) -> Response {
    let (mut parts, _) = r.response.into_parts();
    parts.headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    Response::from_parts(parts, axum::body::Body::from(html))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exemptions() {
        for (p, q) in [
            ("/signed-out", ""),
            ("/sign-in/local", ""),
            ("/sign-in", "error=oidc_denied"),
            ("/invite/abc", ""),
            ("/reset-password/x", ""),
            ("/shared/lists/t", ""),
            ("/setup", ""),
        ] {
            assert!(exempt(p, q), "{p}?{q}");
        }
        for (p, q) in [("/", ""), ("/sign-in", "next=/x"), ("/library", "error=x")] {
            assert!(!exempt(p, q), "{p}?{q}");
        }
    }
}
