//! OIDC sign-in with several providers (addendum-1 4.2; supersedes DEC-072),
//! via `openidconnect` 4.1 (<https://docs.rs/openidconnect/4.1.0>).
//!
//! - Providers come from [`ProviderStore`] (admin area) plus the environment's `auth.oidc`
//!   (a read-only row, [`ENV_PROVIDER_ID`]). One redirect URI serves all of them.
//! - Discovery per provider, cached in memory for [`DISCOVERY_TTL_HOURS`] and refreshed
//!   early when an ID token signature does not verify (key rotation) or when the admin runs
//!   Test; the last document is stored for the overview.
//! - Authorization code flow with PKCE S256 always (RFC 7636; RFC 9700 2.1.1 recommends it
//!   for confidential clients too), `state` and `nonce` in a signed single-use cookie
//!   ([`cookie`]), mix-up defence (RFC 9700 4.4.2): the RFC 9207 `iss` response parameter
//!   when the provider advertises it, else the ID token's `iss`, which `openidconnect`
//!   compares with the discovered issuer (Core 3.1.3.7 step 2).
//! - Account resolution and group rules: [`resolve`].
//!
//! HTTP goes through the `HttpFetch` port as connector [`CONNECTOR`], i.e. through the
//! governed client whose host allowlist follows the configured providers
//! ([`HostAllowlist`]).

mod cookie;
mod email_link;
mod pg;
mod provider;
mod resolve;
mod sso;

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use app::ports::{Clock, HttpFetch, HttpRequest, Lane};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use domain::ids::UserId;
use openidconnect::core::{
    CoreAuthPrompt, CoreAuthenticationFlow, CoreClient, CoreJsonWebKeySet, CoreProviderMetadata,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret as OidcClientSecret, CsrfToken, LoginHint, Nonce,
    OAuth2TokenResponse, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde_json::Value;
use uuid::Uuid;
use zeroize::Zeroizing;

pub use cookie::{
    COOKIE_KEY_INFO, CookieError, CookieKeys, Hint, Mode, PENDING_MINUTES, PendingLink,
    PendingLogin, StaticCookieKey, open as open_signed, return_path, seal as seal_signed,
};
pub use email_link::{
    EMAIL_LINK_MINUTES, EMAIL_LINK_WINDOW_MINUTES, EMAIL_LINKS_PER_WINDOW, EmailedLink,
};
pub use pg::PgProviderStore;
pub use provider::{
    Capabilities, ClientSecret, DEFAULT_SCOPES, ENV_PROVIDER_ID, MemProviderStore, Provenance,
    Provider, ProviderStore, RoleSource, SECRET_PURPOSE, SecretChange,
};
pub use resolve::{LinkError, Outcome, sanitize_username};
pub use sso::{
    AutoStart, BACKCHANNEL_PER_MINUTE, BackchannelLogout, HINT_DAYS, JTI_MINUTES, LOGOUT_EVENT,
    LogoutTokenError, OUTAGE_CAP, OUTAGE_WINDOW_MINUTES, SILENT_GUARD_MINUTES,
};

use crate::AuthError;

/// Connector id of OIDC requests in the governed HTTP client.
pub const CONNECTOR: &str = "oidc";
/// How long a discovery document is used before it is fetched again.
pub const DISCOVERY_TTL_HOURS: i64 = 24;
/// Cookie kinds.
pub(crate) const LOGIN_KIND: &str = "login";
pub(crate) const LINK_KIND: &str = "link";

/// The governed client's host allowlist for [`CONNECTOR`], changed at run time when
/// providers are added, changed or discovered.
pub trait HostAllowlist: Send + Sync {
    /// Allow exactly these `host` / `host:port` entries from now on.
    fn set_hosts(&self, hosts: Vec<String>);
}

/// Why a sign-in failed; the code is what the sign-in page shows (`/sign-in?error=<code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureCode {
    /// Anything that must not be explained further (state, signature, unknown provider).
    Failed,
    /// The provider answered `access_denied` (or another refusal).
    Denied,
    /// Discovery or the token endpoint did not answer (outage path, addendum-1 4.4).
    Unavailable,
    /// Not in an allowed group.
    GroupRule,
    /// No account and none may be created.
    NoAccount,
    AccountDisabled,
    /// Link mode: the identity belongs to another account.
    IdentityInUse,
    /// No such (enabled) provider.
    NotFound,
    /// Re-authentication: the provider did not confirm a fresh sign-in of this account
    /// (`auth_time` missing or older than 300 s, or another identity; od-12 4.4 step 4).
    ReauthNotFresh,
    /// Re-authentication: the account has no identity at an enabled provider.
    NoLinkedProvider,
}

impl FailureCode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Failed => "oidc_failed",
            Self::Denied => "oidc_denied",
            Self::Unavailable => "oidc_unavailable",
            Self::GroupRule => "group_rule",
            Self::NoAccount => "no_account",
            Self::AccountDisabled => "account_disabled",
            Self::IdentityInUse => "identity_in_use",
            Self::NotFound => "not_found",
            Self::ReauthNotFresh => "reauth_not_fresh",
            Self::NoLinkedProvider => "no_linked_provider",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: FailureCode,
    /// The mode of the flow (`link` failures go back to Settings).
    pub mode: Mode,
    /// For the log only, never shown.
    pub detail: String,
    /// An outage after the retry cap (addendum-1 4.4): the page stops retrying by itself.
    pub capped: bool,
    /// Where the flow came from (re-auth and probe failures go back there).
    pub return_to: Option<String>,
}

impl Failure {
    fn new(code: FailureCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            mode: Mode::Interactive,
            detail: detail.into(),
            capped: false,
            return_to: None,
        }
    }

    fn failed(detail: impl std::fmt::Display) -> Self {
        Self::new(FailureCode::Failed, detail.to_string())
    }

    fn in_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }
}

impl From<AuthError> for Failure {
    fn from(e: AuthError) -> Self {
        match e {
            AuthError::AccountDisabled => Self::new(FailureCode::AccountDisabled, e.to_string()),
            e => Self::failed(e),
        }
    }
}

impl From<app::ports::PortError> for Failure {
    fn from(e: app::ports::PortError) -> Self {
        Self::failed(e)
    }
}

/// The redirect to the provider and the pending cookie to set.
#[derive(Debug, Clone)]
pub struct Started {
    pub url: String,
    pub cookie: String,
    pub provider: Uuid,
}

/// The query of the callback (`code`, `state`, RFC 9207 `iss`, or an `error`).
#[derive(Debug, Clone, Default)]
pub struct Callback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub iss: Option<String>,
    pub error: Option<String>,
}

/// A fresh discovery as Test reports it.
#[derive(Debug, Clone)]
pub struct Discovery {
    pub doc: Value,
    pub capabilities: Capabilities,
}

#[derive(Clone)]
struct Discovered {
    issuer: String,
    meta: CoreProviderMetadata,
    doc: Value,
    caps: Capabilities,
    at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct OidcHttpError(String);

/// Everything [`Oidc`] is built from.
pub struct OidcParts {
    pub store: Arc<dyn ProviderStore>,
    pub http: Arc<dyn HttpFetch>,
    pub keys: Arc<dyn CookieKeys>,
    pub clock: Arc<dyn Clock>,
    /// `None` when the HTTP client has no runtime allowlist (tests).
    pub hosts: Option<Arc<dyn HostAllowlist>>,
    /// Audit rows of provider-driven role changes.
    pub audit: Option<Arc<dyn crate::admin::AdminStore>>,
    /// `<public_url>` without a trailing slash.
    pub public_url: String,
}

/// The environment's provider (`auth.oidc`), installed as a read-only row.
#[derive(Clone)]
pub struct EnvProvider {
    pub provider: Provider,
    pub secret: Option<Zeroizing<String>>,
}

pub struct Oidc {
    parts: OidcParts,
    env_secret: Mutex<Option<Zeroizing<String>>>,
    cache: Mutex<HashMap<Uuid, Discovered>>,
    guards: sso::Guards,
}

fn host_of(url: &str) -> Option<String> {
    let u = openidconnect::url::Url::parse(url).ok()?;
    let host = u.host_str()?;
    Some(match u.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    })
}

/// Is `issuer` acceptable: https always; plain http only for a loopback host, or while the
/// instance itself runs on plain http (development).
#[must_use]
pub fn issuer_allowed(issuer: &str, public_url: &str) -> bool {
    let Ok(u) = openidconnect::url::Url::parse(issuer) else {
        return false;
    };
    match u.scheme() {
        "https" => u.host_str().is_some(),
        "http" => {
            let loopback = match u.host() {
                Some(openidconnect::url::Host::Domain(d)) => d == "localhost",
                Some(openidconnect::url::Host::Ipv4(a)) => a.is_loopback(),
                Some(openidconnect::url::Host::Ipv6(a)) => a.is_loopback(),
                None => false,
            };
            loopback || public_url.starts_with("http://")
        }
        _ => false,
    }
}

/// The verified claims of a JWS whose signature `openidconnect` checked already.
fn payload(jwt: &str) -> Value {
    jwt.split('.')
        .nth(1)
        .and_then(|p| URL_SAFE_NO_PAD.decode(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null)
}

type Client = CoreClient<
    openidconnect::EndpointSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointNotSet,
    openidconnect::EndpointMaybeSet,
    openidconnect::EndpointMaybeSet,
>;

impl Oidc {
    #[must_use]
    pub fn new(parts: OidcParts) -> Self {
        Self {
            parts,
            env_secret: Mutex::new(None),
            cache: Mutex::new(HashMap::new()),
            guards: sso::Guards::default(),
        }
    }

    #[must_use]
    pub fn store(&self) -> &Arc<dyn ProviderStore> {
        &self.parts.store
    }

    /// `<public_url>/api/v1/auth/oidc/callback`, the one redirect URI of every provider.
    #[must_use]
    pub fn redirect_uri(&self) -> String {
        format!("{}/api/v1/auth/oidc/callback", self.parts.public_url)
    }

    #[must_use]
    pub fn public_url(&self) -> &str {
        &self.parts.public_url
    }

    fn now(&self) -> DateTime<Utc> {
        self.parts.clock.now()
    }

    /// Install (or remove, `None`) the environment's provider row.
    ///
    /// # Errors
    /// Storage errors, or its slug is taken by a provider from the admin area.
    pub async fn install_env(&self, env: Option<EnvProvider>) -> Result<(), AuthError> {
        let store = &self.parts.store;
        if let Some(e) = env {
            let mut p = e.provider;
            p.id = ENV_PROVIDER_ID;
            p.provenance = Provenance::Env;
            let all = store.providers().await?;
            if let Some(old) = all.iter().find(|o| o.id == ENV_PROVIDER_ID) {
                p.created_at = old.created_at;
            }
            // the default chosen in the admin area wins over the environment's
            p.is_default =
                p.is_default && !all.iter().any(|o| o.id != ENV_PROVIDER_ID && o.is_default);
            if !store.save(&p, SecretChange::Clear, &[]).await? {
                return Err(AuthError::Oidc(format!(
                    "auth.oidc: the slug `{}` is used by another provider",
                    p.slug
                )));
            }
            *self
                .env_secret
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = e.secret;
        } else {
            store.delete(ENV_PROVIDER_ID, &[]).await?;
            *self
                .env_secret
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        }
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&ENV_PROVIDER_ID);
        self.sync_hosts().await;
        Ok(())
    }

    /// Is a client secret available for `p` (stored, or from the environment)?
    #[must_use]
    pub fn secret_set(&self, p: &Provider) -> bool {
        self.secret_of(p).is_some()
    }

    fn secret_of(&self, p: &Provider) -> Option<Zeroizing<String>> {
        match p.provenance {
            Provenance::Env => self
                .env_secret
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            Provenance::Db => match &p.client_secret {
                ClientSecret::Set(s) => Some(s.clone()),
                ClientSecret::None | ClientSecret::Locked => None,
            },
        }
    }

    /// Forget the cached discovery of `id` (its configuration changed) and update the host
    /// allowlist.
    pub async fn changed(&self, id: Uuid) {
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
        self.sync_hosts().await;
    }

    /// Allow the issuer hosts of every enabled provider and the endpoint hosts their
    /// discovery documents name (token, JWKS, `UserInfo`).
    pub async fn sync_hosts(&self) {
        self.allow(None, None).await;
    }

    /// [`Oidc::sync_hosts`] plus the provider being discovered (enabled or not: the admin's
    /// Test of a provider that is switched off, UX3-10) and the endpoints of `extra` (its
    /// document).
    async fn allow(&self, being: Option<&Provider>, extra: Option<&Value>) {
        let Some(hosts) = &self.parts.hosts else {
            return;
        };
        let providers = match self.parts.store.providers().await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "oidc providers could not be read for the allowlist");
                return;
            }
        };
        let cache = self
            .cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut set = BTreeSet::new();
        for p in providers.iter().filter(|p| p.enabled) {
            set.extend(host_of(&p.issuer));
            let doc = cache.get(&p.id).map(|d| &d.doc).or(p.discovery.as_ref());
            for key in ["token_endpoint", "jwks_uri", "userinfo_endpoint"] {
                set.extend(doc.and_then(|d| d[key].as_str()).and_then(host_of));
            }
        }
        set.extend(being.and_then(|p| host_of(&p.issuer)));
        for key in ["token_endpoint", "jwks_uri", "userinfo_endpoint"] {
            set.extend(extra.and_then(|d| d[key].as_str()).and_then(host_of));
        }
        hosts.set_hosts(set.into_iter().collect());
    }

    /// Adapter from the oauth2 HTTP client contract to the `HttpFetch` port.
    fn http_client(
        &self,
    ) -> impl Fn(
        openidconnect::HttpRequest,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<openidconnect::HttpResponse, OidcHttpError>>
                + Send,
        >,
    > + use<> {
        let fetch = self.parts.http.clone();
        move |req: openidconnect::HttpRequest| {
            let fetch = fetch.clone();
            Box::pin(async move {
                let headers = req
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
                    .collect();
                let resp = fetch
                    .fetch(HttpRequest {
                        connector: CONNECTOR.into(),
                        op: "oidc".into(),
                        method: req.method().to_string(),
                        url: req.uri().to_string(),
                        headers,
                        body: Some(req.body().clone()).filter(|b| !b.is_empty()),
                        lane: Lane::Interactive,
                        skip_auth_record: false,
                        user: None,
                    })
                    .await
                    .map_err(|e| OidcHttpError(e.to_string()))?;
                let mut b = http::Response::builder().status(resp.status);
                for (k, v) in &resp.headers {
                    b = b.header(k, v);
                }
                b.body(resp.body).map_err(|e| OidcHttpError(e.to_string()))
            })
        }
    }

    async fn get_json(&self, url: &str, op: &str, bearer: Option<&str>) -> Result<Value, Failure> {
        let mut headers = vec![("accept".to_string(), "application/json".to_string())];
        if let Some(t) = bearer {
            headers.push(("authorization".into(), format!("Bearer {t}")));
        }
        let resp = self
            .parts
            .http
            .fetch(HttpRequest {
                connector: CONNECTOR.into(),
                op: op.into(),
                method: "GET".into(),
                url: url.into(),
                headers,
                body: None,
                lane: Lane::Interactive,
                skip_auth_record: false,
                user: None,
            })
            .await
            .map_err(|e| Failure::new(FailureCode::Unavailable, format!("{op}: {e}")))?;
        if resp.status >= 500 {
            return Err(Failure::new(
                FailureCode::Unavailable,
                format!("{op}: HTTP {}", resp.status),
            ));
        }
        if resp.status != 200 {
            return Err(Failure::failed(format!("{op}: HTTP {}", resp.status)));
        }
        serde_json::from_slice(&resp.body).map_err(|e| Failure::failed(format!("{op}: {e}")))
    }

    /// Discovery (`<issuer>/.well-known/openid-configuration`, Discovery 1.0 4) and the
    /// JWKS, cached; `force` fetches again.
    async fn discover(&self, p: &Provider, force: bool) -> Result<Discovered, Failure> {
        let now = self.now();
        if !force
            && let Some(d) = self
                .cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&p.id)
            && d.issuer == p.issuer
            && now - d.at < Duration::hours(DISCOVERY_TTL_HOURS)
        {
            return Ok(d.clone());
        }
        self.allow(Some(p), None).await;
        let issuer = p.issuer.trim_end_matches('/');
        let doc = self
            .get_json(
                &format!("{issuer}/.well-known/openid-configuration"),
                "discovery",
                None,
            )
            .await?;
        let meta: CoreProviderMetadata = serde_json::from_value(doc.clone())
            .map_err(|e| Failure::failed(format!("discovery document: {e}")))?;
        // Discovery 1.0 4.3: the issuer "MUST be identical" to the one used for discovery
        let named = meta.issuer().as_str();
        if named != p.issuer {
            // not normalised: some providers do end their issuer in `/` (UX3-26)
            let hint = if named.trim_end_matches('/') == p.issuer.trim_end_matches('/') {
                "; enter the issuer exactly as the provider names it (the trailing slash differs)"
            } else {
                ""
            };
            return Err(Failure::failed(format!(
                "discovery names issuer {named}{hint}"
            )));
        }
        // the token, JWKS and UserInfo hosts may differ from the issuer's (Google)
        self.allow(Some(p), Some(&doc)).await;
        let jwks: CoreJsonWebKeySet = serde_json::from_value(
            self.get_json(meta.jwks_uri().url().as_str(), "jwks", None)
                .await?,
        )
        .map_err(|e| Failure::failed(format!("jwks: {e}")))?;
        let d = Discovered {
            issuer: p.issuer.clone(),
            caps: Capabilities::from_discovery(&doc),
            doc,
            meta: meta.set_jwks(jwks),
            at: now,
        };
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(p.id, d.clone());
        if let Err(e) = self
            .parts
            .store
            .record_discovery(p.id, &d.doc, &d.caps, now)
            .await
        {
            tracing::warn!(error = %e, "storing the discovery document failed");
        }
        Ok(d)
    }

    fn client(&self, p: &Provider, d: &Discovered) -> Result<Client, Failure> {
        Ok(CoreClient::from_provider_metadata(
            d.meta.clone(),
            ClientId::new(p.client_id.clone()),
            self.secret_of(p)
                .map(|s| OidcClientSecret::new(s.to_string())),
        )
        .set_redirect_uri(RedirectUrl::new(self.redirect_uri()).map_err(Failure::failed)?))
    }

    /// The enabled provider `slug`, else the default one, else the only enabled one.
    async fn pick(&self, slug: Option<&str>) -> Result<Provider, Failure> {
        let enabled: Vec<Provider> = self
            .parts
            .store
            .providers()
            .await?
            .into_iter()
            .filter(|p| p.enabled)
            .collect();
        let found = match slug {
            Some(s) => enabled.iter().find(|p| p.slug == s),
            None => enabled
                .iter()
                .find(|p| p.is_default)
                .or_else(|| enabled.first().filter(|_| enabled.len() == 1)),
        };
        found
            .cloned()
            .ok_or_else(|| Failure::new(FailureCode::NotFound, "no such provider"))
    }

    /// Where to send the browser, and the signed pending cookie to set. `user` is the
    /// signed-in account for `link` and `reauth`.
    ///
    /// # Errors
    /// [`FailureCode::NotFound`] for an unknown provider; discovery failures.
    pub async fn start(
        &self,
        slug: Option<&str>,
        mode: Mode,
        next: Option<&str>,
        user: Option<UserId>,
    ) -> Result<Started, Failure> {
        let p = self.pick(slug).await.map_err(|f| f.in_mode(mode))?;
        self.start_for(&p, mode, next, user, None).await
    }

    /// [`Oidc::start`] for the provider `p`, with a `login_hint` for silent requests (Entra
    /// wants one with `prompt=none`, addendum-1 A-23). Discovery outages count towards the
    /// retry cap.
    pub(crate) async fn start_for(
        &self,
        p: &Provider,
        mode: Mode,
        next: Option<&str>,
        user: Option<UserId>,
        login_hint: Option<&str>,
    ) -> Result<Started, Failure> {
        let d = match self.discover(p, false).await {
            Ok(d) => d,
            Err(f) => {
                if f.code == FailureCode::Unavailable {
                    self.record_outage(p.id);
                }
                return Err(f.in_mode(mode));
            }
        };
        let client = self.client(p, &d)?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let mut req = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .set_pkce_challenge(challenge);
        for s in p.scopes.iter().filter(|s| *s != "openid") {
            req = req.add_scope(Scope::new(s.clone()));
        }
        // silent sign-in (Core 3.1.2.1 `prompt=none`) and the re-auth round trip (`max_age=0`,
        // "equivalent to prompt=login" and makes `auth_time` required, DEC-191)
        req = match mode {
            Mode::Silent | Mode::ProbeSilent => {
                let r = req.add_prompt(CoreAuthPrompt::None);
                match login_hint {
                    Some(h) => r.set_login_hint(LoginHint::new(h.to_string())),
                    None => r,
                }
            }
            Mode::Reauth | Mode::ProbeMaxAge => req.set_max_age(std::time::Duration::ZERO),
            Mode::Interactive | Mode::Link => req,
        };
        let (url, csrf, nonce) = req.url();
        let default_next = if mode == Mode::Link {
            "/settings/account"
        } else {
            "/"
        };
        let pending = PendingLogin {
            provider: p.id,
            issuer: p.issuer.clone(),
            state: csrf.secret().clone(),
            nonce: nonce.secret().clone(),
            verifier: verifier.secret().clone(),
            return_to: match next {
                Some(n) => return_path(Some(n)),
                None => default_next.into(),
            },
            mode,
            user: user.map(|u| u.0),
        };
        let exp = (self.now() + Duration::minutes(PENDING_MINUTES)).timestamp();
        Ok(Started {
            url: url.to_string(),
            cookie: cookie::seal(self.parts.keys.as_ref(), LOGIN_KIND, &pending, exp),
            provider: p.id,
        })
    }

    /// Check the pending cookie and the callback parameters, exchange the code, verify the
    /// ID token, then resolve the account (addendum-1 4.2). `session` is the account of the
    /// browser's session cookie, if any. The result is stored on the provider for the
    /// overview.
    ///
    /// # Errors
    /// A [`Failure`] with the code the sign-in page shows.
    pub async fn callback(
        &self,
        auth: &crate::Auth,
        cookie: Option<&str>,
        q: &Callback,
        session: Option<(UserId, &str)>,
    ) -> Result<Outcome, Failure> {
        let pending = self.take_pending(cookie, q).await?;
        let mode = pending.mode;
        let mut result = self
            .finish(auth, &pending, q, session)
            .await
            .map_err(|mut f| {
                f.return_to = Some(pending.return_to.clone());
                f.in_mode(mode)
            });
        match &mut result {
            Err(f) if f.code == FailureCode::Unavailable => {
                self.record_outage(pending.provider);
                f.capped = self.outage_capped(pending.provider);
            }
            Ok(_) => self.recovered(pending.provider),
            Err(_) => {}
        }
        let note = match &result {
            Ok(_) => "ok".to_string(),
            Err(f) => format!("failed: {}", f.code.as_str()),
        };
        if let Err(e) = self
            .parts
            .store
            .record_callback(pending.provider, &note, self.now())
            .await
        {
            tracing::warn!(error = %e, "storing the callback result failed");
        }
        if let Err(f) = &result {
            tracing::info!(code = f.code.as_str(), detail = %f.detail, "oidc sign-in refused");
        }
        result
    }

    /// The pending sign-in: signature, expiry, single use, `state` (RFC 6749 10.12).
    async fn take_pending(
        &self,
        cookie: Option<&str>,
        q: &Callback,
    ) -> Result<PendingLogin, Failure> {
        let raw = cookie.ok_or_else(|| Failure::failed("no pending sign-in"))?;
        let now = self.now();
        let pending: PendingLogin =
            cookie::open(self.parts.keys.as_ref(), LOGIN_KIND, raw, now.timestamp())
                .map_err(Failure::failed)?;
        let fail = |d: &str| Failure::failed(d).in_mode(pending.mode);
        if q.state.as_deref() != Some(pending.state.as_str()) {
            return Err(fail("state mismatch"));
        }
        let used = cookie::used_key(LOGIN_KIND, &pending.state);
        let expires = now + Duration::minutes(PENDING_MINUTES + 1);
        if !self.parts.store.claim_once(&used, expires, now).await? {
            return Err(fail("pending sign-in used already"));
        }
        Ok(pending)
    }

    async fn finish(
        &self,
        auth: &crate::Auth,
        pending: &PendingLogin,
        q: &Callback,
        session: Option<(UserId, &str)>,
    ) -> Result<Outcome, Failure> {
        let p = self
            .parts
            .store
            .provider(pending.provider)
            .await?
            .filter(|p| p.enabled && p.issuer == pending.issuer)
            .ok_or_else(|| Failure::failed("unknown provider"))?;
        if let Some(e) = &q.error {
            return self.provider_error(&p, pending, e).await;
        }
        let d = self.discover(&p, false).await?;
        // RFC 9207 2.4: compare `iss` when present; required when advertised
        match &q.iss {
            Some(iss) if *iss != pending.issuer => {
                return Err(Failure::failed("iss response parameter mismatch"));
            }
            None if d.caps.authorization_response_iss_parameter_supported => {
                return Err(Failure::failed("iss response parameter missing"));
            }
            _ => {}
        }
        let code = q
            .code
            .as_deref()
            .ok_or_else(|| Failure::failed("no code"))?;
        let v = self.exchange(&p, d, pending, code).await?;
        match pending.mode {
            Mode::ProbeSilent | Mode::ProbeMaxAge => Ok(self.probe_verified(&v, pending).await),
            Mode::Reauth => self.reauthed(auth, &v, pending, session).await,
            Mode::Interactive | Mode::Silent | Mode::Link => {
                self.resolve(auth, v, pending, session.map(|s| s.0)).await
            }
        }
    }

    /// Code exchange with the PKCE verifier and ID token validation (Core 3.1.3.7 via
    /// `IdToken::claims`); one discovery refresh when the signature does not verify.
    async fn exchange(
        &self,
        p: &Provider,
        mut d: Discovered,
        pending: &PendingLogin,
        code: &str,
    ) -> Result<resolve::Verified, Failure> {
        let http = self.http_client();
        let token = self
            .client(p, &d)?
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .map_err(Failure::failed)?
            .set_pkce_verifier(PkceCodeVerifier::new(pending.verifier.clone()))
            .request_async(&http)
            .await
            .map_err(|e| match e {
                openidconnect::RequestTokenError::Request(e) => {
                    Failure::new(FailureCode::Unavailable, format!("token endpoint: {e}"))
                }
                // RFC 6749 4.1.2.1 names these two for an unavailable server
                openidconnect::RequestTokenError::ServerResponse(r)
                    if matches!(
                        r.error().to_string().as_str(),
                        "server_error" | "temporarily_unavailable"
                    ) =>
                {
                    Failure::new(FailureCode::Unavailable, format!("token endpoint: {r}"))
                }
                e => Failure::failed(format!("token endpoint: {e}")),
            })?;
        let id_token = token
            .id_token()
            .ok_or_else(|| Failure::failed("no id token"))?;
        let nonce = Nonce::new(pending.nonce.clone());
        let mut refreshed = false;
        let claims = loop {
            let client = self.client(p, &d)?;
            match id_token.claims(&client.id_token_verifier(), &nonce) {
                Ok(c) => break c.clone(),
                Err(openidconnect::ClaimsVerificationError::SignatureVerification(_))
                    if !refreshed && self.guards.may_refetch(p.id, self.now()) =>
                {
                    refreshed = true;
                    d = self.discover(p, true).await?;
                }
                Err(e) => return Err(Failure::failed(format!("id token: {e}"))),
            }
        };
        let raw = payload(&id_token.to_string());
        let mut groups = resolve::groups(&raw, &p.groups_claim);
        if groups.is_none()
            && p.needs_groups()
            && let Some(url) = &d.caps.userinfo_endpoint
        {
            // Core 5.3: claims at UserInfo; its `sub` MUST equal the ID token's (5.3.2)
            let info = self
                .get_json(url, "userinfo", Some(token.access_token().secret()))
                .await?;
            if info["sub"].as_str() != Some(claims.subject().as_str()) {
                return Err(Failure::failed("userinfo sub mismatch"));
            }
            groups = resolve::groups(&info, &p.groups_claim);
        }
        Ok(resolve::Verified {
            provider: p.clone(),
            issuer: claims.issuer().to_string(),
            subject: claims.subject().to_string(),
            sid: raw["sid"].as_str().map(String::from),
            auth_time: raw["auth_time"]
                .as_i64()
                .and_then(|t| DateTime::from_timestamp(t, 0)),
            id_token: id_token.to_string(),
            claims_hash: resolve::claims_hash(&raw, groups.as_deref()),
            claims: raw,
            groups,
        })
    }

    /// Fresh discovery for the admin's Test, recorded on the provider.
    ///
    /// # Errors
    /// Discovery failures (also recorded).
    pub async fn rediscover(&self, p: &Provider) -> Result<Discovery, Failure> {
        let d = self.discover(p, true).await?;
        Ok(Discovery {
            doc: d.doc,
            capabilities: d.caps,
        })
    }

    pub(crate) fn keys(&self) -> &dyn CookieKeys {
        self.parts.keys.as_ref()
    }

    pub(crate) fn clock(&self) -> &Arc<dyn Clock> {
        &self.parts.clock
    }

    pub(crate) fn audit_store(&self) -> Option<&Arc<dyn crate::admin::AdminStore>> {
        self.parts.audit.as_ref()
    }
}
