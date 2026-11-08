//! True SSO and logout (phase 3 WP3-3; addendum-1 4.2 "Logout" and "True SSO", 4.3, 4.4):
//!
//! - Local-first sign-out, then RP-initiated logout at the provider's `end_session_endpoint`
//!   (OIDC RP-Initiated Logout 1.0, section 2) with `id_token_hint` (kept per
//!   session, encrypted), `client_id`, `post_logout_redirect_uri` and `state`.
//! - Back-channel logout tokens (OIDC Back-Channel Logout 1.0, 2.4 and 2.6):
//!   signature against the provider's JWKS, `iss`, `aud`, `iat`, `exp`, the logout event, no
//!   `nonce`, `sub` and/or `sid`, and a `jti` replay cache; sessions end by `sid`, else every
//!   session of the identity signed in through that provider (2.7).
//! - Silent sign-in with `prompt=none` as a top-level redirect (Core 3.1.2.1); the errors of
//!   3.1.2.6 that ask for the person lead to the interactive request; `server_error` and
//!   `temporarily_unavailable` (RFC 6749 4.1.2.1) and network failures are the outage path,
//!   capped at [`OUTAGE_CAP`] failures in [`OUTAGE_WINDOW_MINUTES`] per process.
//! - The provider Test's probes (OD-19, od-12 4.4 step 5) and the re-auth round trip with
//!   `max_age=0` (Core 3.1.2.1 `max_age`, DEC-191).
//!
//! Counters (outages, `jti`s, the back-channel budget) live in memory per process, like the
//! sign-in throttle (DEC-155).

use std::collections::HashMap;
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use domain::ids::UserId;
use openidconnect::JsonWebKey;
use openidconnect::core::CoreJwsSigningAlgorithm;
use serde_json::{Value, json};
use uuid::Uuid;

use super::cookie::{self, Hint, Mode, PendingLogin};
use super::provider::Provider;
use super::resolve::{Outcome, Verified};
use super::{Failure, FailureCode, Oidc, Started};
use crate::fresh::OIDC_AUTH_TIME_MAX_AGE_SECS;
use crate::store::SessionSecret;
use crate::{Auth, AuthError, SessionToken, token_hash};

/// Cookie kind of the hint cookie.
pub(crate) const HINT_KIND: &str = "hint";
/// Lifetime of the hint cookie (the longest browsers keep a cookie, RFC 6265bis 4.1.2.2).
pub const HINT_DAYS: i64 = 400;
/// One silent attempt per browser within this window (addendum-1 4.2 step 4).
pub const SILENT_GUARD_MINUTES: i64 = 5;
/// Outage failures per provider within [`OUTAGE_WINDOW_MINUTES`] before automatic
/// redirects stop (addendum-1 4.4).
pub const OUTAGE_CAP: usize = 3;
pub const OUTAGE_WINDOW_MINUTES: i64 = 10;
/// How long a logout token's `jti` is remembered, and the oldest `iat` accepted.
pub const JTI_MINUTES: i64 = 10;
/// Back-channel requests per minute and process (the endpoint is public).
pub const BACKCHANNEL_PER_MINUTE: u32 = 60;
/// At most one forced discovery per provider within this many seconds after a signature that
/// no key verifies (key rotation never needs more; the back-channel endpoint is public, UX3-11).
pub const REFETCH_COOLDOWN_SECS: i64 = 60;
/// Clock skew allowed for `iat` and `exp`.
const SKEW_SECS: i64 = 60;
/// `jti`s kept at most.
const JTI_CAPACITY: usize = 10_000;
/// The member of `events` a logout token must carry (Back-Channel Logout 2.4).
pub const LOGOUT_EVENT: &str = "http://schemas.openid.net/event/backchannel-logout";
/// The errors of Core 3.1.2.6 that mean "the person has to do something".
const INTERACTION_ERRORS: [&str; 4] = [
    "login_required",
    "interaction_required",
    "account_selection_required",
    "consent_required",
];

/// In-memory guards of one process.
#[derive(Default)]
pub(crate) struct Guards {
    outages: Mutex<HashMap<Uuid, Vec<DateTime<Utc>>>>,
    jtis: Mutex<HashMap<String, DateTime<Utc>>>,
    budget: Mutex<Option<(DateTime<Utc>, u32)>>,
    refetched: Mutex<HashMap<Uuid, DateTime<Utc>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Guards {
    /// `true` (and remembered) when `provider` had no forced discovery within
    /// [`REFETCH_COOLDOWN_SECS`].
    pub(crate) fn may_refetch(&self, provider: Uuid, now: DateTime<Utc>) -> bool {
        let mut r = lock(&self.refetched);
        if r.get(&provider)
            .is_some_and(|t| now - *t < Duration::seconds(REFETCH_COOLDOWN_SECS))
        {
            return false;
        }
        r.insert(provider, now);
        true
    }

    fn outage(&self, provider: Uuid, now: DateTime<Utc>) {
        let mut o = lock(&self.outages);
        let v = o.entry(provider).or_default();
        v.retain(|t| now - *t < Duration::minutes(OUTAGE_WINDOW_MINUTES));
        v.push(now);
    }

    fn capped(&self, provider: Uuid, now: DateTime<Utc>) -> bool {
        lock(&self.outages).get(&provider).is_some_and(|v| {
            v.iter()
                .filter(|t| now - **t < Duration::minutes(OUTAGE_WINDOW_MINUTES))
                .count()
                >= OUTAGE_CAP
        })
    }

    fn recovered(&self, provider: Uuid) {
        lock(&self.outages).remove(&provider);
    }

    /// `true` the first time `key` is seen within [`JTI_MINUTES`].
    fn first_jti(&self, key: &str, now: DateTime<Utc>) -> bool {
        let mut j = lock(&self.jtis);
        j.retain(|_, until| *until > now);
        if j.contains_key(key) {
            return false;
        }
        if j.len() >= JTI_CAPACITY
            && let Some(oldest) = j.iter().min_by_key(|(_, t)| **t).map(|(k, _)| k.clone())
        {
            j.remove(&oldest);
        }
        j.insert(key.to_string(), now + Duration::minutes(JTI_MINUTES));
        true
    }

    fn forget_jti(&self, key: &str) {
        lock(&self.jtis).remove(key);
    }

    fn take_budget(&self, now: DateTime<Utc>) -> bool {
        let mut b = lock(&self.budget);
        match b.as_mut() {
            Some((start, n)) if now - *start < Duration::minutes(1) => {
                if *n >= BACKCHANNEL_PER_MINUTE {
                    return false;
                }
                *n += 1;
            }
            _ => *b = Some((now, 1)),
        }
        true
    }
}

/// Why a logout token was refused.
#[derive(Debug, thiserror::Error)]
pub enum LogoutTokenError {
    /// The per-process budget is used up (answer 429).
    #[error("too many back-channel logout requests")]
    TooMany,
    /// Back-Channel Logout 2.8: answer 400.
    #[error("invalid logout token: {0}")]
    Invalid(String),
    #[error("storage: {0}")]
    Store(String),
}

/// What a valid logout token ended.
#[derive(Debug, Clone)]
pub struct BackchannelLogout {
    pub provider: Provider,
    pub sessions: u64,
    /// Ended by `sid` (else by `sub`).
    pub by_sid: bool,
}

/// The automatic redirect of a signed-out page visit in sign-in mode `auto`.
#[derive(Debug, Clone)]
pub struct AutoStart {
    pub started: Started,
    pub provider: Provider,
    /// `prompt=none` (the probe succeeded and no attempt in the last 5 minutes).
    pub silent: bool,
}

fn bad(d: &str) -> LogoutTokenError {
    LogoutTokenError::Invalid(d.to_string())
}

/// The signing algorithm of a JWS header: asymmetric ones only (no `none`, no HMAC with the
/// client secret).
fn signing_alg(header: &Value) -> Option<CoreJwsSigningAlgorithm> {
    serde_json::from_value::<CoreJwsSigningAlgorithm>(header["alg"].clone())
        .ok()
        .filter(|a| {
            !matches!(
                a,
                CoreJwsSigningAlgorithm::HmacSha256
                    | CoreJwsSigningAlgorithm::HmacSha384
                    | CoreJwsSigningAlgorithm::HmacSha512
                    | CoreJwsSigningAlgorithm::None
            )
        })
}

/// Back-Channel Logout 2.6 steps 3-6 (step 7, the `jti`, is the caller's).
fn check_logout_claims(c: &Value, now: DateTime<Utc>) -> Result<(), LogoutTokenError> {
    let now = now.timestamp();
    let exp = c["exp"].as_i64().ok_or_else(|| bad("exp"))?;
    if exp + SKEW_SECS <= now {
        return Err(bad("expired"));
    }
    let iat = c["iat"].as_i64().ok_or_else(|| bad("iat"))?;
    if iat > now + SKEW_SECS || iat < now - JTI_MINUTES * 60 {
        return Err(bad("iat out of range"));
    }
    if !c["events"].get(LOGOUT_EVENT).is_some_and(Value::is_object) {
        return Err(bad("no logout event"));
    }
    if c.get("nonce").is_some() {
        return Err(bad("nonce present"));
    }
    if !c["sub"].is_string() && !c["sid"].is_string() {
        return Err(bad("neither sub nor sid"));
    }
    Ok(())
}

/// Back-Channel Logout 2.7: the sessions of the OP session `sid`, else every session of the
/// `sub` identity's account signed in through `provider`.
async fn end_sessions(
    auth: &Auth,
    provider: &Provider,
    claims: &Value,
) -> Result<u64, app::ports::PortError> {
    if let Some(sid) = claims["sid"].as_str() {
        return auth.store().delete_sessions_by_sid(provider.id, sid).await;
    }
    let sub = claims["sub"].as_str().unwrap_or_default();
    match auth.store().identity(&provider.issuer, sub).await? {
        Some(identity) => {
            auth.store()
                .delete_sessions_via(identity.user_id, provider.id)
                .await
        }
        None => Ok(0),
    }
}

fn decode_part(s: &str) -> Option<Value> {
    URL_SAFE_NO_PAD
        .decode(s)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

impl Oidc {
    pub(crate) fn record_outage(&self, provider: Uuid) {
        self.guards.outage(provider, self.now());
    }

    pub(crate) fn recovered(&self, provider: Uuid) {
        self.guards.recovered(provider);
    }

    /// Did the provider fail [`OUTAGE_CAP`] times within [`OUTAGE_WINDOW_MINUTES`]? Then
    /// automatic redirects and retries stop (addendum-1 4.4, outage row).
    #[must_use]
    pub fn outage_capped(&self, provider: Uuid) -> bool {
        self.guards.capped(provider, self.now())
    }

    /// `<public_url>/signed-out`, registered at the provider as post-logout redirect URI.
    #[must_use]
    pub fn post_logout_redirect_uri(&self) -> String {
        format!("{}/signed-out", self.public_url())
    }

    /// The signed hint cookie value after a sign-in through `v.provider`.
    pub(crate) fn hint(&self, v: &Verified) -> String {
        let login_hint = v.claims["preferred_username"]
            .as_str()
            .or_else(|| v.claims["email"].as_str())
            .map(String::from);
        let exp = (self.now() + Duration::days(HINT_DAYS)).timestamp();
        cookie::seal(
            self.keys(),
            HINT_KIND,
            &Hint {
                provider: v.provider.id,
                login_hint,
            },
            exp,
        )
    }

    /// A hint cookie, if it is ours and not expired.
    #[must_use]
    pub fn open_hint(&self, raw: &str) -> Option<Hint> {
        cookie::open(self.keys(), HINT_KIND, raw, self.now().timestamp()).ok()
    }

    /// Keep the ID token of a sign-in with its session, for `id_token_hint` at logout.
    pub(crate) async fn keep_id_token(&self, auth: &Auth, token: &SessionToken, id_token: &str) {
        if let Err(e) = auth
            .store()
            .set_session_secret(
                &token_hash(&token.0),
                SessionSecret::IdToken,
                Some(id_token),
            )
            .await
        {
            tracing::warn!(error = %e, "keeping the id token for logout failed");
        }
    }

    /// The RP-initiated logout request (RP-Initiated Logout 1.0, 2): `None` when the
    /// provider has no `end_session_endpoint`.
    #[must_use]
    pub fn end_session_url(&self, p: &Provider, id_token: Option<&str>) -> Option<String> {
        let endpoint = p.capabilities.end_session_endpoint.as_deref()?;
        let mut u = openidconnect::url::Url::parse(endpoint).ok()?;
        {
            let mut q = u.query_pairs_mut();
            if let Some(t) = id_token {
                q.append_pair("id_token_hint", t);
            }
            q.append_pair("client_id", &p.client_id);
            q.append_pair("post_logout_redirect_uri", &self.post_logout_redirect_uri());
            q.append_pair(
                "state",
                &URL_SAFE_NO_PAD.encode(crate::random_bytes::<16>()),
            );
        }
        Some(u.to_string())
    }

    /// Local-first sign-out (addendum-1 4.2 "Logout"): the session (and its ID token) is
    /// deleted first; answers where the browser goes next when the session came from a
    /// provider with an `end_session_endpoint`.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn sign_out(&self, auth: &Auth, token: &str) -> Result<Option<String>, AuthError> {
        let hash = token_hash(token);
        let session = auth.store().session(&hash).await?;
        let id_token = match &session {
            Some(_) => {
                auth.store()
                    .session_secret(&hash, SessionSecret::IdToken)
                    .await?
            }
            None => None,
        };
        auth.sign_out(token).await?;
        let Some(provider) = session.and_then(|s| s.provider_id) else {
            return Ok(None);
        };
        let Some(p) = self
            .parts
            .store
            .provider(provider)
            .await?
            .filter(|p| p.enabled)
        else {
            return Ok(None);
        };
        Ok(self.end_session_url(&p, id_token.as_ref().map(|t| t.as_str())))
    }

    /// Sign-in mode `auto`: the redirect of a signed-out visit to the default provider,
    /// silent first when the probe succeeded and the browser made no attempt in the last
    /// [`SILENT_GUARD_MINUTES`] (OD-19, addendum-1 4.2 step 4). `Ok(None)` without an enabled
    /// default provider (then the sign-in page applies).
    ///
    /// # Errors
    /// [`FailureCode::Unavailable`] (with `capped` once the retry cap is reached).
    pub async fn auto_start(
        &self,
        next: &str,
        silent_tried: bool,
        hint: Option<&str>,
    ) -> Result<Option<AutoStart>, Failure> {
        let Ok(p) = self.pick(None).await else {
            return Ok(None);
        };
        if self.outage_capped(p.id) {
            let mut f = Failure::new(FailureCode::Unavailable, "retry cap reached");
            f.capped = true;
            return Err(f);
        }
        let probed = p
            .last_test
            .as_ref()
            .is_some_and(|t| t["silent_probe"] == "silent_ok");
        let silent = probed && !silent_tried;
        let login_hint = hint
            .and_then(|h| self.open_hint(h))
            .filter(|h| h.provider == p.id)
            .and_then(|h| h.login_hint);
        let mode = if silent {
            Mode::Silent
        } else {
            Mode::Interactive
        };
        let started = self
            .start_for(&p, mode, Some(next), None, login_hint.as_deref())
            .await
            .map_err(|mut f| {
                f.capped = self.outage_capped(p.id);
                f
            })?;
        Ok(Some(AutoStart {
            started,
            provider: p,
            silent,
        }))
    }

    /// The enabled providers `user` has an identity at; the one the session `token` was
    /// signed in through first.
    async fn linked_providers(
        &self,
        auth: &Auth,
        user: UserId,
        token: &str,
    ) -> Result<Vec<Provider>, AuthError> {
        let via = auth
            .store()
            .session(&token_hash(token))
            .await?
            .and_then(|s| s.provider_id);
        let ids = auth.store().identities_of(user).await?;
        let mut linked: Vec<Provider> = self
            .parts
            .store
            .providers()
            .await?
            .into_iter()
            .filter(|p| p.enabled && ids.iter().any(|i| i.provider_id == Some(p.id)))
            .collect();
        linked.sort_by_key(|p| Some(p.id) != via);
        Ok(linked)
    }

    /// The re-auth round trip (DEC-191, od-12 4.4): `max_age=0` at the provider the session
    /// was signed in through, else (or `slug`) one the account has an identity at.
    ///
    /// # Errors
    /// [`FailureCode::NoLinkedProvider`]; discovery failures.
    pub async fn start_reauth(
        &self,
        auth: &Auth,
        user: UserId,
        token: &str,
        slug: Option<&str>,
        next: Option<&str>,
    ) -> Result<Started, Failure> {
        let linked = self.linked_providers(auth, user, token).await?;
        let p = linked
            .iter()
            .find(|p| slug.is_none_or(|s| p.slug == s))
            .ok_or_else(|| Failure::new(FailureCode::NoLinkedProvider, "no linked provider"))?;
        self.start_for(p, Mode::Reauth, next, Some(user), None)
            .await
    }

    /// The provider a re-auth round trip of this session would use.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn reauth_provider(
        &self,
        auth: &Auth,
        user: UserId,
        token: &str,
    ) -> Result<Option<Provider>, AuthError> {
        Ok(self
            .linked_providers(auth, user, token)
            .await?
            .into_iter()
            .next())
    }

    /// An `error` on the callback (Core 3.1.2.6, RFC 6749 4.1.2.1), by mode.
    pub(crate) async fn provider_error(
        &self,
        p: &Provider,
        pending: &PendingLogin,
        e: &str,
    ) -> Result<Outcome, Failure> {
        let outage = matches!(e, "temporarily_unavailable" | "server_error");
        let interaction = INTERACTION_ERRORS.contains(&e);
        match pending.mode {
            _ if outage => Err(Failure::new(FailureCode::Unavailable, e)),
            // the provider needs the person (or cannot answer silently): ask again,
            // interactively, without showing our page (addendum-1 4.2 step 3)
            Mode::Silent if e != "access_denied" => {
                let s = self
                    .start_for(p, Mode::Interactive, Some(&pending.return_to), None, None)
                    .await?;
                Ok(Outcome::Restart(s))
            }
            Mode::ProbeSilent => {
                let r = if interaction {
                    "silent_login_required"
                } else {
                    "silent_unsupported"
                };
                Ok(self.probe_done(p, pending, r).await)
            }
            Mode::ProbeMaxAge => Ok(self.probe_done(p, pending, "max_age_error").await),
            _ => Err(Failure::new(FailureCode::Denied, e)),
        }
    }

    /// The probe after a successful exchange.
    pub(crate) async fn probe_verified(&self, v: &Verified, pending: &PendingLogin) -> Outcome {
        let r = if pending.mode == Mode::ProbeSilent {
            "silent_ok"
        } else if v
            .auth_time
            .is_some_and(|t| t >= self.now() - Duration::seconds(OIDC_AUTH_TIME_MAX_AGE_SECS))
        {
            "max_age_ok"
        } else {
            "max_age_ignored"
        };
        self.probe_done(&v.provider, pending, r).await
    }

    /// Record a probe outcome with the provider's last Test (its time stays the Test's).
    async fn probe_done(&self, p: &Provider, pending: &PendingLogin, result: &str) -> Outcome {
        let now = self.now();
        let mut t = p.last_test.clone().unwrap_or_else(|| json!({}));
        if !t.is_object() {
            t = json!({});
        }
        match result {
            "silent_ok" | "silent_login_required" | "silent_unsupported" => {
                t["silent_probe"] = json!(result);
                t["silent_probe_at"] = json!(now.to_rfc3339());
            }
            "max_age_ok" | "max_age_ignored" => {
                t["max_age_probe"] = json!(result == "max_age_ok");
                t["max_age_probe_at"] = json!(now.to_rfc3339());
            }
            _ => {}
        }
        if let Err(e) = self
            .parts
            .store
            .record_test(p.id, &t, p.last_test_at.unwrap_or(now))
            .await
        {
            tracing::warn!(error = %e, "storing the probe result failed");
        }
        Outcome::Probe {
            return_to: pending.return_to.clone(),
            result: result.to_string(),
        }
    }

    /// Mode `reauth` after the exchange: `auth_time` within 300 s, the identity belongs to
    /// the session's account (`Auth::confirm_oidc_reauth`), never passed through (od-12
    /// 4.4 steps 3-4).
    pub(crate) async fn reauthed(
        &self,
        auth: &Auth,
        v: &Verified,
        pending: &PendingLogin,
        session: Option<(UserId, &str)>,
    ) -> Result<Outcome, Failure> {
        let Some((_, token)) = session.filter(|(u, _)| Some(u.0) == pending.user) else {
            return Err(Failure::failed("re-authentication without its session"));
        };
        let not_fresh = |d: &str| Failure::new(FailureCode::ReauthNotFresh, d);
        let at = v.auth_time.ok_or_else(|| not_fresh("no auth_time"))?;
        let fresh = auth
            .confirm_oidc_reauth(token, &v.issuer, &v.subject, &v.provider.slug, at)
            .await
            .map_err(|e| match e {
                AuthError::ReauthRequired => not_fresh("auth_time too old or another identity"),
                e => e.into(),
            })?;
        self.keep_id_token(auth, &fresh, &v.id_token).await;
        Ok(Outcome::Reauthed {
            token: fresh,
            return_to: pending.return_to.clone(),
        })
    }

    /// Validate a back-channel logout token and end its sessions (Back-Channel Logout 1.0,
    /// 2.6 and 2.7). Role rules are not recomputed: the token carries no groups (2.4); the
    /// next sign-in does that.
    ///
    /// # Errors
    /// [`LogoutTokenError`].
    pub async fn backchannel_logout(
        &self,
        auth: &Auth,
        token: &str,
    ) -> Result<BackchannelLogout, LogoutTokenError> {
        let now = self.now();
        if !self.guards.take_budget(now) {
            return Err(LogoutTokenError::TooMany);
        }
        let parts: Vec<&str> = token.trim().split('.').collect();
        let [head_b64, body_b64, sig_b64] = parts.as_slice() else {
            return Err(bad("not a signed JWT"));
        };
        let header = decode_part(head_b64).ok_or_else(|| bad("header"))?;
        let claims = decode_part(body_b64).ok_or_else(|| bad("claims"))?;
        let alg = signing_alg(&header).ok_or_else(|| bad("algorithm"))?;
        if let Some(typ) = header["typ"].as_str()
            && !typ.eq_ignore_ascii_case("logout+jwt")
            && !typ.eq_ignore_ascii_case("jwt")
        {
            return Err(bad("typ"));
        }
        let iss = claims["iss"].as_str().ok_or_else(|| bad("iss"))?;
        let aud: Vec<&str> = match &claims["aud"] {
            Value::String(one) => vec![one.as_str()],
            Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        };
        let provider = self
            .parts
            .store
            .providers()
            .await
            .map_err(|err| LogoutTokenError::Store(err.0))?
            .into_iter()
            .find(|x| x.enabled && x.issuer == iss && aud.contains(&x.client_id.as_str()))
            .ok_or_else(|| bad("no provider for this iss and aud"))?;
        let message = format!("{head_b64}.{body_b64}");
        self.verify_jws(&provider, &alg, &header, &message, sig_b64)
            .await?;
        check_logout_claims(&claims, now)?;
        let jti = claims["jti"].as_str().ok_or_else(|| bad("jti"))?;
        let key = format!("{}|{jti}", provider.id);
        if !self.guards.first_jti(&key, now) {
            return Err(bad("jti seen before"));
        }
        let sessions = end_sessions(auth, &provider, &claims)
            .await
            .map_err(|err| {
                self.guards.forget_jti(&key);
                LogoutTokenError::Store(err.0)
            })?;
        Ok(BackchannelLogout {
            by_sid: claims["sid"].is_string(),
            provider,
            sessions,
        })
    }

    /// The JWS signature with the provider's keys (the `kid` when given); one fresh
    /// discovery when no key verifies (key rotation), at most one per provider per minute.
    async fn verify_jws(
        &self,
        p: &Provider,
        alg: &CoreJwsSigningAlgorithm,
        header: &Value,
        message: &str,
        signature: &str,
    ) -> Result<(), LogoutTokenError> {
        let sig = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| bad("signature encoding"))?;
        let kid = header["kid"].as_str();
        for force in [false, true] {
            if force && !self.guards.may_refetch(p.id, self.now()) {
                break;
            }
            let d = self
                .discover(p, force)
                .await
                .map_err(|f| bad(&format!("discovery: {}", f.detail)))?;
            let ok = d
                .meta
                .jwks()
                .keys()
                .iter()
                .filter(|k| kid.is_none_or(|id| k.key_id().is_some_and(|x| x.as_str() == id)))
                .any(|k| k.verify_signature(alg, message.as_bytes(), &sig).is_ok());
            if ok {
                return Ok(());
            }
        }
        Err(bad("signature"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims(now: i64) -> Value {
        json!({"iss": "https://i", "aud": "c", "iat": now, "exp": now + 60, "jti": "1",
               "sub": "s", "events": {LOGOUT_EVENT: {}}})
    }

    #[test]
    fn logout_claims_follow_section_2_6() {
        let t = Utc::now();
        let now = t.timestamp();
        assert!(check_logout_claims(&claims(now), t).is_ok());
        let mut c = claims(now);
        c["nonce"] = json!("n");
        assert!(check_logout_claims(&c, t).is_err(), "nonce");
        let mut c = claims(now);
        c["events"] = json!({});
        assert!(check_logout_claims(&c, t).is_err(), "event");
        let mut c = claims(now);
        c["iat"] = json!(now - JTI_MINUTES * 60 - 1);
        assert!(
            check_logout_claims(&c, t).is_err(),
            "older than the jti window"
        );
        let mut c = claims(now);
        c.as_object_mut().unwrap().remove("sub");
        assert!(check_logout_claims(&c, t).is_err(), "neither sub nor sid");
        c["sid"] = json!("x");
        assert!(check_logout_claims(&c, t).is_ok(), "sid alone");
    }

    #[test]
    fn symmetric_and_none_algorithms_are_refused() {
        for (alg, ok) in [
            ("RS256", true),
            ("ES256", true),
            ("HS256", false),
            ("none", false),
        ] {
            assert_eq!(signing_alg(&json!({"alg": alg})).is_some(), ok, "{alg}");
        }
    }

    #[test]
    fn guards_cap_outages_and_replays() {
        let g = Guards::default();
        let p = Uuid::now_v7();
        let t = Utc::now();
        for _ in 0..OUTAGE_CAP {
            assert!(!g.capped(p, t));
            g.outage(p, t);
        }
        assert!(g.capped(p, t));
        assert!(!g.capped(p, t + Duration::minutes(OUTAGE_WINDOW_MINUTES)));
        assert!(g.first_jti("a", t));
        assert!(!g.first_jti("a", t));
        assert!(g.first_jti("a", t + Duration::minutes(JTI_MINUTES + 1)));
    }
}
