//! An in-process OIDC provider for tests (phase 3 WP3-2, DEC-308).
//!
//! It answers discovery, JWKS, token and `UserInfo` requests through the `HttpFetch` port (no
//! sockets), and plays the browser's part at the authorization endpoint with
//! [`TestIssuer::authorize`]. ID tokens are RS256 (the OIDC Core 3.1.3.7 default)
//! signed with an RSA key generated once per test process; the signature comes from
//! `openidconnect`'s own `CoreRsaPrivateSigningKey`, so the code under test verifies a real
//! signature against the published JWKS. PKCE (RFC 7636 S256) is checked at the token
//! endpoint like a real provider would.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use app::ports::{HttpError, HttpFetch, HttpRequest, HttpResponse};
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use openidconnect::PrivateSigningKey;
use openidconnect::core::{CoreJwsSigningAlgorithm, CoreRsaPrivateSigningKey};
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::sha2::{Digest, Sha256};
use serde_json::{Value, json};

/// Key id of the test signing key in the JWKS.
pub const KEY_ID: &str = "test-key-1";

/// PKCS#1 PEM of the process-wide test key (2048 bits, generated on first use).
fn key_pem() -> &'static str {
    static PEM: OnceLock<String> = OnceLock::new();
    PEM.get_or_init(|| {
        let key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 2048).expect("rsa key");
        key.to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
            .expect("pem")
            .to_string()
    })
}

fn signing_key() -> CoreRsaPrivateSigningKey {
    CoreRsaPrivateSigningKey::from_pem(
        key_pem(),
        Some(openidconnect::JsonWebKeyId::new(KEY_ID.into())),
    )
    .expect("test key")
}

/// The person signing in at the provider.
#[derive(Debug, Clone, Default)]
pub struct Login {
    pub sub: String,
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
    pub email_verified: bool,
    /// `None`: no groups claim at all.
    pub groups: Option<Vec<String>>,
    /// Session id (`sid`, Back-Channel Logout 2.1) put into the ID token.
    pub sid: Option<String>,
}

impl Login {
    /// A login with `sub` and a matching `preferred_username`.
    #[must_use]
    pub fn new(sub: &str) -> Self {
        Self {
            sub: sub.into(),
            preferred_username: Some(sub.into()),
            name: Some(sub.into()),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn email(mut self, email: &str, verified: bool) -> Self {
        self.email = Some(email.into());
        self.email_verified = verified;
        self
    }

    #[must_use]
    pub fn groups(mut self, groups: &[&str]) -> Self {
        self.groups = Some(groups.iter().map(|g| (*g).to_string()).collect());
        self
    }

    #[must_use]
    pub fn sid(mut self, sid: &str) -> Self {
        self.sid = Some(sid.into());
        self
    }
}

/// Switches for provider behaviour.
// Independent test switches, one per provider behaviour; an enum would not read better.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct Behaviour {
    /// Advertise `authorization_response_iss_parameter_supported` and send `iss` on the
    /// redirect (RFC 9207).
    pub iss_parameter: bool,
    /// `iss` sent on the redirect instead of the issuer (mix-up tests).
    pub callback_iss: Option<String>,
    /// `iss` put into ID tokens instead of the issuer.
    pub id_token_iss: Option<String>,
    /// Put the groups claim into the ID token (else only into `UserInfo`, like authentik and
    /// Authelia by default).
    pub groups_in_id_token: bool,
    /// Advertise `end_session_endpoint` and back-channel logout.
    pub logout: bool,
    /// Discovery answers 503.
    pub down: bool,
    /// The `auth_time` claim of ID tokens (Core 2; `max_age` makes it required).
    pub auth_time: AuthTime,
}

/// What the provider puts into `auth_time`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthTime {
    /// The time of the token request (the person just signed in).
    #[default]
    Now,
    /// This many seconds ago (a provider that ignores `max_age=0`).
    SecondsAgo(i64),
    /// No `auth_time` at all.
    Missing,
}

impl Default for Behaviour {
    fn default() -> Self {
        Self {
            iss_parameter: false,
            callback_iss: None,
            id_token_iss: None,
            groups_in_id_token: true,
            logout: false,
            down: false,
            auth_time: AuthTime::Now,
        }
    }
}

struct Grant {
    login: Login,
    nonce: Option<String>,
    challenge: String,
    redirect_uri: String,
}

#[derive(Default)]
struct State {
    behaviour: Behaviour,
    grants: HashMap<String, Grant>,
    tokens: HashMap<String, Login>,
    authorize_params: Vec<HashMap<String, String>>,
    id_tokens: Vec<String>,
    counter: u64,
}

/// One provider at `issuer` with one registered client.
pub struct TestIssuer {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    state: Mutex<State>,
    requests: Mutex<Vec<HttpRequest>>,
}

fn response(status: u16, body: &Value) -> HttpResponse {
    HttpResponse {
        status,
        headers: vec![("content-type".into(), "application/json".into())],
        body: body.to_string().into_bytes(),
    }
}

fn query(url: &str) -> HashMap<String, String> {
    url::Url::parse(url)
        .map(|u| u.query_pairs().into_owned().collect())
        .unwrap_or_default()
}

fn form(body: Option<&[u8]>) -> HashMap<String, String> {
    url::form_urlencoded::parse(body.unwrap_or_default())
        .into_owned()
        .collect()
}

fn decode_form(s: &str) -> String {
    url::form_urlencoded::parse(format!("x={s}").as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl TestIssuer {
    #[must_use]
    pub fn new(issuer: &str, client_id: &str, client_secret: &str) -> Arc<Self> {
        Arc::new(Self {
            issuer: issuer.trim_end_matches('/').to_string(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            state: Mutex::new(State::default()),
            requests: Mutex::new(Vec::new()),
        })
    }

    /// Change the provider's behaviour.
    pub fn behave(&self, f: impl FnOnce(&mut Behaviour)) {
        f(&mut self.state.lock().expect("issuer").behaviour);
    }

    /// The discovery document (OIDC Discovery 1.0 section 3).
    #[must_use]
    pub fn discovery(&self) -> Value {
        let b = self.state.lock().expect("issuer").behaviour.clone();
        let i = &self.issuer;
        let mut d = json!({
            "issuer": i,
            "authorization_endpoint": format!("{i}/authorize"),
            "token_endpoint": format!("{i}/token"),
            "userinfo_endpoint": format!("{i}/userinfo"),
            "jwks_uri": format!("{i}/jwks"),
            "response_types_supported": ["code"],
            "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"],
            "code_challenge_methods_supported": ["S256"],
            "scopes_supported": ["openid", "profile", "email", "groups"],
            "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"]
        });
        if b.iss_parameter {
            d["authorization_response_iss_parameter_supported"] = json!(true);
        }
        if b.logout {
            d["end_session_endpoint"] = json!(format!("{i}/end-session"));
            d["backchannel_logout_supported"] = json!(true);
            d["backchannel_logout_session_supported"] = json!(true);
        }
        d
    }

    fn jwks() -> Value {
        let jwk = signing_key().as_verification_key();
        json!({ "keys": [jwk] })
    }

    /// Sign `claims` as a compact JWS with the test key (RS256).
    ///
    /// # Panics
    /// Never for valid JSON claims.
    #[must_use]
    pub fn sign(claims: &Value) -> String {
        Self::sign_typed(claims, "JWT")
    }

    /// [`TestIssuer::sign`] with another `typ` header (`logout+jwt`, Back-Channel Logout 2.4).
    ///
    /// # Panics
    /// Never for valid JSON claims.
    #[must_use]
    pub fn sign_typed(claims: &Value, typ: &str) -> String {
        let header = json!({"alg": "RS256", "typ": typ, "kid": KEY_ID});
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let sig = signing_key()
            .sign(
                &CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
                input.as_bytes(),
            )
            .expect("sign");
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig))
    }

    /// The browser at the authorization endpoint, with `login` signed in: checks the request
    /// like a provider does and answers the redirect's query string (`code`, `state`, and
    /// `iss` when RFC 9207 is on).
    ///
    /// # Panics
    /// On a request a real provider would refuse (wrong client, no PKCE S256, no openid).
    #[must_use]
    pub fn authorize(&self, auth_url: &str, login: &Login) -> String {
        let q = self.check_authorize(auth_url);
        let mut st = self.state.lock().expect("issuer");
        st.counter += 1;
        let code = format!("code-{}-{}", st.counter, login.sub);
        st.grants.insert(
            code.clone(),
            Grant {
                login: login.clone(),
                nonce: q.get("nonce").cloned(),
                challenge: q["code_challenge"].clone(),
                redirect_uri: q["redirect_uri"].clone(),
            },
        );
        let mut out = url::form_urlencoded::Serializer::new(String::new());
        out.append_pair("code", &code);
        if let Some(s) = q.get("state") {
            out.append_pair("state", s);
        }
        let b = &st.behaviour;
        if let Some(iss) = &b.callback_iss {
            out.append_pair("iss", iss);
        } else if b.iss_parameter {
            out.append_pair("iss", &self.issuer);
        }
        out.finish()
    }

    /// The provider refuses (`access_denied`, `login_required`, ...): the redirect's query.
    #[must_use]
    pub fn deny(&self, auth_url: &str, error: &str) -> String {
        let q = self.check_authorize(auth_url);
        let mut out = url::form_urlencoded::Serializer::new(String::new());
        out.append_pair("error", error);
        if let Some(s) = q.get("state") {
            out.append_pair("state", s);
        }
        out.finish()
    }

    fn check_authorize(&self, auth_url: &str) -> HashMap<String, String> {
        assert!(
            auth_url.starts_with(&format!("{}/authorize", self.issuer)),
            "authorization request for another provider: {auth_url}"
        );
        let q = query(auth_url);
        assert_eq!(q.get("client_id"), Some(&self.client_id), "client_id");
        assert_eq!(q.get("response_type").map(String::as_str), Some("code"));
        assert_eq!(
            q.get("code_challenge_method").map(String::as_str),
            Some("S256"),
            "PKCE S256 (RFC 7636 4.3)"
        );
        assert!(q.contains_key("code_challenge"), "code_challenge");
        assert!(q.contains_key("redirect_uri"), "redirect_uri");
        assert!(
            q.get("scope")
                .is_some_and(|s| s.split(' ').any(|x| x == "openid")),
            "openid scope"
        );
        self.state
            .lock()
            .expect("issuer")
            .authorize_params
            .push(q.clone());
        q
    }

    /// The browser at the authorization endpoint of a `prompt=none` request (OIDC Core
    /// 3.1.2.1): with an OP session (`Some(login)`) the code comes back without any page,
    /// without one the provider answers `login_required` (3.1.2.6).
    ///
    /// # Panics
    /// When the request does not carry `prompt=none`.
    #[must_use]
    pub fn silent(&self, auth_url: &str, session: Option<&Login>) -> String {
        assert_eq!(
            query(auth_url).get("prompt").map(String::as_str),
            Some("none"),
            "a silent request carries prompt=none: {auth_url}"
        );
        match session {
            Some(login) => self.authorize(auth_url, login),
            None => self.deny(auth_url, "login_required"),
        }
    }

    /// Every ID token issued so far, in order.
    #[must_use]
    pub fn id_tokens(&self) -> Vec<String> {
        self.state.lock().expect("issuer").id_tokens.clone()
    }

    /// The claims of a back-channel logout token (Back-Channel Logout 1.0, 2.4) for this
    /// provider's client: `iss`, `aud`, `iat`, `exp`, `jti`, the logout event and `sub`
    /// and/or `sid`. Change them before [`TestIssuer::sign_typed`] for the negative cases.
    #[must_use]
    pub fn logout_claims(&self, sub: Option<&str>, sid: Option<&str>) -> Value {
        let mut st = self.state.lock().expect("issuer");
        st.counter += 1;
        let mut c = json!({
            "iss": self.issuer,
            "aud": self.client_id,
            "iat": now(),
            "exp": now() + 120,
            "jti": format!("jti-{}", st.counter),
            "events": {"http://schemas.openid.net/event/backchannel-logout": {}},
        });
        if let Some(s) = sub {
            c["sub"] = json!(s);
        }
        if let Some(s) = sid {
            c["sid"] = json!(s);
        }
        c
    }

    /// A signed logout token with [`TestIssuer::logout_claims`].
    #[must_use]
    pub fn logout_token(&self, sub: Option<&str>, sid: Option<&str>) -> String {
        Self::sign_typed(&self.logout_claims(sub, sid), "logout+jwt")
    }

    /// The query parameters of every authorization request so far.
    #[must_use]
    pub fn authorize_requests(&self) -> Vec<HashMap<String, String>> {
        self.state.lock().expect("issuer").authorize_params.clone()
    }

    /// Back-channel requests received so far.
    #[must_use]
    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().expect("issuer").clone()
    }

    fn claims(login: &Login, with_groups: bool) -> Value {
        let mut c = json!({ "sub": login.sub });
        if let Some(v) = &login.preferred_username {
            c["preferred_username"] = json!(v);
        }
        if let Some(v) = &login.name {
            c["name"] = json!(v);
        }
        if let Some(v) = &login.email {
            c["email"] = json!(v);
            c["email_verified"] = json!(login.email_verified);
        }
        if with_groups && let Some(g) = &login.groups {
            c["groups"] = json!(g);
        }
        c
    }

    fn token(&self, req: &HttpRequest) -> HttpResponse {
        let f = form(req.body.as_deref());
        let (id, secret) = basic_auth(&req.headers).unwrap_or_else(|| {
            (
                f.get("client_id").cloned().unwrap_or_default(),
                f.get("client_secret").cloned().unwrap_or_default(),
            )
        });
        if id != self.client_id || secret != self.client_secret {
            return response(401, &json!({"error": "invalid_client"}));
        }
        let mut st = self.state.lock().expect("issuer");
        let Some(grant) = f.get("code").and_then(|c| st.grants.remove(c)) else {
            return response(400, &json!({"error": "invalid_grant"}));
        };
        let verifier = f.get("code_verifier").cloned().unwrap_or_default();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        if challenge != grant.challenge || f.get("redirect_uri") != Some(&grant.redirect_uri) {
            return response(400, &json!({"error": "invalid_grant"}));
        }
        let mut claims = Self::claims(&grant.login, st.behaviour.groups_in_id_token);
        claims["iss"] = json!(
            st.behaviour
                .id_token_iss
                .clone()
                .unwrap_or_else(|| self.issuer.clone())
        );
        claims["aud"] = json!(self.client_id);
        claims["iat"] = json!(now());
        claims["exp"] = json!(now() + 300);
        match st.behaviour.auth_time {
            AuthTime::Now => claims["auth_time"] = json!(now()),
            AuthTime::SecondsAgo(s) => claims["auth_time"] = json!(now() - s),
            AuthTime::Missing => {}
        }
        if let Some(n) = &grant.nonce {
            claims["nonce"] = json!(n);
        }
        if let Some(sid) = &grant.login.sid {
            claims["sid"] = json!(sid);
        }
        st.counter += 1;
        let access = format!("at-{}", st.counter);
        st.tokens.insert(access.clone(), grant.login);
        let id_token = Self::sign(&claims);
        st.id_tokens.push(id_token.clone());
        response(
            200,
            &json!({
                "access_token": access,
                "token_type": "Bearer",
                "expires_in": 300,
                "id_token": id_token,
            }),
        )
    }

    fn userinfo(&self, req: &HttpRequest) -> HttpResponse {
        let token = req
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .and_then(|(_, v)| v.strip_prefix("Bearer "))
            .unwrap_or_default()
            .to_string();
        let st = self.state.lock().expect("issuer");
        match st.tokens.get(&token) {
            Some(login) => response(200, &Self::claims(login, true)),
            None => response(401, &json!({"error": "invalid_token"})),
        }
    }

    fn handle(&self, req: &HttpRequest) -> HttpResponse {
        let path = req.url.strip_prefix(&self.issuer).unwrap_or_default();
        let path = path.split('?').next().unwrap_or_default();
        let down = self.state.lock().expect("issuer").behaviour.down;
        match (req.method.as_str(), path) {
            (_, _) if down => response(503, &json!({"error": "temporarily_unavailable"})),
            ("GET", "/.well-known/openid-configuration") => response(200, &self.discovery()),
            ("GET", "/jwks") => response(200, &Self::jwks()),
            ("POST", "/token") => self.token(req),
            ("GET" | "POST", "/userinfo") => self.userinfo(req),
            _ => response(404, &json!({"error": "not_found"})),
        }
    }
}

fn basic_auth(headers: &[(String, String)]) -> Option<(String, String)> {
    let v = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("authorization"))?
        .1
        .strip_prefix("Basic ")?;
    let raw = String::from_utf8(STANDARD.decode(v).ok()?).ok()?;
    let (id, secret) = raw.split_once(':')?;
    // RFC 6749 2.3.1: both parts are form-urlencoded before the Basic encoding
    Some((decode_form(id), decode_form(secret)))
}

/// Several providers behind one `HttpFetch`; a request goes to the issuer whose URL it
/// starts with.
#[derive(Default, Clone)]
pub struct Issuers(pub Vec<Arc<TestIssuer>>);

#[async_trait]
impl HttpFetch for Issuers {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let Some(i) = self
            .0
            .iter()
            .find(|i| req.url.starts_with(&format!("{}/", i.issuer)))
        else {
            return Err(HttpError::Network(format!(
                "no test issuer for {}",
                req.url
            )));
        };
        i.requests.lock().expect("issuer").push(req.clone());
        Ok(i.handle(&req))
    }
}

#[async_trait]
impl HttpFetch for TestIssuer {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        if !req.url.starts_with(&format!("{}/", self.issuer)) {
            return Err(HttpError::Network(format!(
                "no test issuer for {}",
                req.url
            )));
        }
        self.requests.lock().expect("issuer").push(req.clone());
        Ok(self.handle(&req))
    }
}
