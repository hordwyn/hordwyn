//! OIDC sign-in (authorization code + PKCE), via `openidconnect` 4.1. Its HTTP
//! traffic goes through the `HttpFetch` port as connector `oidc`, i.e. through the governed
//! client with its host allowlist (D-15 Q4).

use std::sync::Arc;

use app::ports::{HttpFetch, HttpRequest, Lane};
use domain::user::{Actor, Role};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::OnceCell;

use crate::{Auth, AuthError, Credentials, OidcIdentity, SessionToken};

/// Connector id of OIDC requests in the governed HTTP client.
pub const CONNECTOR: &str = "oidc";

#[derive(Debug, Clone)]
pub struct OidcSettings {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    /// `<public_url>/api/v1/auth/oidc/callback`.
    pub redirect_url: String,
    /// Create accounts for unknown identities even if sign-up is not open.
    pub auto_create: bool,
}

/// State between redirect and callback. Kept client-side in an `HttpOnly` cookie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingLogin {
    pub csrf: String,
    pub nonce: String,
    pub pkce_verifier: String,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct OidcHttpError(String);

pub struct Oidc {
    settings: OidcSettings,
    http: Arc<dyn HttpFetch>,
    metadata: OnceCell<CoreProviderMetadata>,
}

fn oe(e: impl std::fmt::Display) -> AuthError {
    AuthError::Oidc(e.to_string())
}

impl Oidc {
    #[must_use]
    pub fn new(settings: OidcSettings, http: Arc<dyn HttpFetch>) -> Self {
        Self {
            settings,
            http,
            metadata: OnceCell::new(),
        }
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
        let fetch = self.http.clone();
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

    async fn metadata(&self) -> Result<&CoreProviderMetadata, AuthError> {
        self.metadata
            .get_or_try_init(|| async {
                let issuer = IssuerUrl::new(self.settings.issuer.clone()).map_err(oe)?;
                let http = self.http_client();
                CoreProviderMetadata::discover_async(issuer, &http)
                    .await
                    .map_err(oe)
            })
            .await
    }

    async fn client(
        &self,
    ) -> Result<
        CoreClient<
            openidconnect::EndpointSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointNotSet,
            openidconnect::EndpointMaybeSet,
            openidconnect::EndpointMaybeSet,
        >,
        AuthError,
    > {
        let meta = self.metadata().await?.clone();
        Ok(CoreClient::from_provider_metadata(
            meta,
            ClientId::new(self.settings.client_id.clone()),
            self.settings.client_secret.clone().map(ClientSecret::new),
        )
        .set_redirect_uri(RedirectUrl::new(self.settings.redirect_url.clone()).map_err(oe)?))
    }

    /// Where to send the browser, plus the state to keep in a cookie until the callback.
    ///
    /// # Errors
    /// Discovery failures.
    pub async fn start(&self) -> Result<(String, PendingLogin), AuthError> {
        let client = self.client().await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, csrf, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .add_scope(Scope::new("email".into()))
            .set_pkce_challenge(challenge)
            .url();
        Ok((
            url.to_string(),
            PendingLogin {
                csrf: csrf.secret().clone(),
                nonce: nonce.secret().clone(),
                pkce_verifier: verifier.secret().clone(),
            },
        ))
    }

    /// Finish the flow: check state, exchange the code, verify the ID token, then sign in the
    /// linked user (or create one when allowed).
    ///
    /// # Errors
    /// State mismatch, token or verification failures, or no account and no auto-create.
    pub async fn finish(
        &self,
        auth: &Auth,
        pending: &PendingLogin,
        state: &str,
        code: &str,
    ) -> Result<SessionToken, AuthError> {
        if state != pending.csrf {
            return Err(AuthError::Oidc("state mismatch".into()));
        }
        let client = self.client().await?;
        let http = self.http_client();
        let token = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .map_err(oe)?
            .set_pkce_verifier(PkceCodeVerifier::new(pending.pkce_verifier.clone()))
            .request_async(&http)
            .await
            .map_err(oe)?;
        let id_token = token
            .id_token()
            .ok_or_else(|| AuthError::Oidc("no id token".into()))?;
        let claims = id_token
            .claims(
                &client.id_token_verifier(),
                &Nonce::new(pending.nonce.clone()),
            )
            .map_err(oe)?;
        let issuer = claims.issuer().to_string();
        let subject = claims.subject().to_string();
        if let Some(id) = auth.store.identity(&issuer, &subject).await? {
            return auth.create_session(id.user_id, "oidc").await;
        }
        let first = auth.app.user_count().await? == 0;
        if !first && !self.settings.auto_create && auth.settings().signup != crate::SignupMode::Open
        {
            return Err(AuthError::SignupClosed);
        }
        let username = claims
            .preferred_username()
            .map(|u| sanitize_username(u))
            .filter(|u| u.len() >= 3)
            .unwrap_or_else(|| format!("user-{}", &blake3::hash(subject.as_bytes()).to_hex()[..8]));
        let display = claims
            .name()
            .and_then(|n| n.get(None))
            .map_or_else(|| username.clone(), |n| n.to_string());
        let email = claims.email().map(|e| e.to_string());
        let role = if first { Role::Admin } else { Role::Member };
        let user = auth
            .app
            .register_user(&username, &display, email, role)
            .await?;
        auth.store
            .put_credentials(&Credentials::empty(user.id))
            .await?;
        auth.store
            .link_identity(&OidcIdentity {
                issuer,
                subject,
                user_id: user.id,
            })
            .await?;
        let _ = Actor {
            user_id: user.id,
            role,
        };
        auth.create_session(user.id, "oidc").await
    }
}

/// Map a provider username onto our rules (`[a-z0-9_.-]`, 3..=32).
#[must_use]
pub fn sanitize_username(u: &str) -> String {
    u.to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .take(32)
        .collect()
}
