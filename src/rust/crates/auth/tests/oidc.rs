//! OIDC against the in-process test issuer (`testkit::oidc`, RS256) through the `HttpFetch`
//! port, with in-memory stores: discovery caching, the PKCE authorization URL, the host
//! allowlist, a full code exchange and the single-use pending cookie.

use std::sync::{Arc, Mutex};

use auth::oidc::{
    CONNECTOR, Callback, FailureCode, HostAllowlist, MemProviderStore, Mode, Oidc, OidcParts,
    Outcome, Provider, ProviderStore, SecretChange, StaticCookieKey,
};
use auth::{Auth, AuthSettings, MemAuthStore};
use testkit::TestEnv;
use testkit::oidc::{Login, TestIssuer};

const ISSUER: &str = "https://id.example.org";

#[derive(Default)]
struct Hosts(Mutex<Vec<String>>);

impl HostAllowlist for Hosts {
    fn set_hosts(&self, hosts: Vec<String>) {
        *self.0.lock().unwrap() = hosts;
    }
}

struct T {
    oidc: Oidc,
    auth: Auth,
    issuer: Arc<TestIssuer>,
    hosts: Arc<Hosts>,
}

async fn setup() -> T {
    let env = TestEnv::new();
    let issuer = TestIssuer::new(ISSUER, "client-1", "s3cret");
    let store = Arc::new(MemProviderStore::default());
    let mut p = Provider::new(
        uuid::Uuid::now_v7(),
        "id",
        ISSUER,
        "client-1",
        chrono::Utc::now(),
    );
    p.is_default = true;
    store
        .save(&p, SecretChange::Set("s3cret"), &[])
        .await
        .unwrap();
    let hosts = Arc::new(Hosts::default());
    let oidc = Oidc::new(OidcParts {
        store,
        http: issuer.clone(),
        keys: Arc::new(StaticCookieKey([9; 32])),
        clock: Arc::new(testkit::FixedClock::default()),
        hosts: Some(hosts.clone()),
        audit: None,
        public_url: "https://app.example.org".into(),
    });
    let auth = Auth::new(
        env.app.clone(),
        Arc::new(MemAuthStore::default()),
        AuthSettings::default(),
    );
    T {
        oidc,
        auth,
        issuer,
        hosts,
    }
}

fn callback(back: &str) -> Callback {
    let q: std::collections::HashMap<String, String> = url::form_urlencoded::parse(back.as_bytes())
        .into_owned()
        .collect();
    Callback {
        code: q.get("code").cloned(),
        state: q.get("state").cloned(),
        iss: q.get("iss").cloned(),
        error: q.get("error").cloned(),
    }
}

#[tokio::test]
async fn start_builds_a_pkce_authorization_url_and_caches_discovery() {
    let t = setup().await;
    let started = t
        .oidc
        .start(None, Mode::Interactive, Some("/stats"), None)
        .await
        .unwrap();
    let u = url::Url::parse(&started.url).unwrap();
    assert_eq!(u.host_str(), Some("id.example.org"));
    let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
    assert_eq!(q["client_id"], "client-1");
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(
        q["redirect_uri"],
        "https://app.example.org/api/v1/auth/oidc/callback"
    );
    assert!(q["scope"].contains("openid") && q["scope"].contains("email"));
    let reqs = t.issuer.requests();
    assert!(
        reqs.iter().all(|r| r.connector == CONNECTOR),
        "governed connector id"
    );
    // the allowlist follows the provider (issuer host; the endpoints share it here)
    assert_eq!(
        *t.hosts.0.lock().unwrap(),
        vec!["id.example.org".to_string()]
    );
    // discovery is cached: a second start does not fetch again
    t.oidc
        .start(Some("id"), Mode::Interactive, None, None)
        .await
        .unwrap();
    assert_eq!(t.issuer.requests().len(), reqs.len());
    let err = t
        .oidc
        .start(Some("nope"), Mode::Interactive, None, None)
        .await
        .unwrap_err();
    assert_eq!(err.code, FailureCode::NotFound);
}

#[tokio::test]
async fn code_exchange_signs_in_once_per_pending_cookie() {
    let t = setup().await;
    let started = t
        .oidc
        .start(None, Mode::Interactive, Some("/stats"), None)
        .await
        .unwrap();
    let back = t.issuer.authorize(&started.url, &Login::new("Anna"));
    let q = callback(&back);
    let out = t
        .oidc
        .callback(&t.auth, Some(&started.cookie), &q, None)
        .await
        .unwrap();
    let Outcome::SignedIn {
        token, return_to, ..
    } = out
    else {
        panic!("signed in expected");
    };
    assert_eq!(return_to, "/stats");
    let actor = t.auth.session(&token.0).await.unwrap().unwrap();
    assert!(actor.is_admin(), "the first account is admin");
    // the same cookie a second time (RFC 9700 4.2.4)
    let again = t
        .oidc
        .callback(&t.auth, Some(&started.cookie), &q, None)
        .await
        .unwrap_err();
    assert_eq!(again.code, FailureCode::Failed);
    // a cookie of another key never passes
    let other = Oidc::new(OidcParts {
        store: Arc::new(MemProviderStore::default()),
        http: t.issuer.clone(),
        keys: Arc::new(StaticCookieKey([1; 32])),
        clock: Arc::new(testkit::FixedClock::default()),
        hosts: None,
        audit: None,
        public_url: "https://app.example.org".into(),
    });
    let err = other
        .callback(&t.auth, Some(&started.cookie), &q, None)
        .await
        .unwrap_err();
    assert_eq!(err.code, FailureCode::Failed);
}

/// Forwards to the issuer only for hosts on the allowlist, as the governed client does.
struct Guarded {
    hosts: Arc<Hosts>,
    issuer: Arc<TestIssuer>,
}

#[async_trait::async_trait]
impl app::ports::HttpFetch for Guarded {
    async fn fetch(
        &self,
        req: app::ports::HttpRequest,
    ) -> Result<app::ports::HttpResponse, app::ports::HttpError> {
        let host = url::Url::parse(&req.url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_default();
        if !self.hosts.0.lock().unwrap().contains(&host) {
            return Err(app::ports::HttpError::Blocked(host));
        }
        self.issuer.fetch(req).await
    }
}

/// UX3-10: the admin's Test of a provider that is switched off reaches its issuer: the
/// provider being discovered is on the allowlist for the call.
#[tokio::test]
async fn a_disabled_provider_can_be_tested() {
    let issuer = TestIssuer::new(ISSUER, "client-1", "s3cret");
    let store = Arc::new(MemProviderStore::default());
    let mut p = Provider::new(
        uuid::Uuid::now_v7(),
        "id",
        ISSUER,
        "client-1",
        chrono::Utc::now(),
    );
    p.enabled = false;
    store
        .save(&p, SecretChange::Set("s3cret"), &[])
        .await
        .unwrap();
    let hosts = Arc::new(Hosts::default());
    let oidc = Oidc::new(OidcParts {
        store,
        http: Arc::new(Guarded {
            hosts: hosts.clone(),
            issuer: issuer.clone(),
        }),
        keys: Arc::new(StaticCookieKey([9; 32])),
        clock: Arc::new(testkit::FixedClock::default()),
        hosts: Some(hosts.clone()),
        audit: None,
        public_url: "https://app.example.org".into(),
    });
    let d = oidc.rediscover(&p).await;
    assert!(d.is_ok(), "{:?}", d.err().map(|f| f.detail));
}

fn b64(v: &serde_json::Value) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string())
}

/// UX3-11: a logout token that no key verifies forces at most one discovery per provider
/// per minute, however many such tokens arrive.
#[tokio::test]
async fn forced_rediscovery_on_a_bad_signature_has_a_cooldown() {
    let t = setup().await;
    let head = b64(&serde_json::json!({"alg": "RS256", "typ": "logout+jwt", "kid": "nope"}));
    let body = b64(&serde_json::json!({"iss": ISSUER, "aud": "client-1"}));
    let garbage = format!("{head}.{body}.c2lnbmF0dXJl");
    let discoveries = || {
        t.issuer
            .requests()
            .iter()
            .filter(|r| r.url.ends_with("/.well-known/openid-configuration"))
            .count()
    };
    for _ in 0..5 {
        assert!(t.oidc.backchannel_logout(&t.auth, &garbage).await.is_err());
    }
    assert_eq!(discoveries(), 2, "the cached one and one forced");
}

/// UX3-26: an issuer typed with a trailing slash the provider does not use fails discovery
/// (Discovery 1.0 4.3, identical issuers) with a detail that says what to change.
#[tokio::test]
async fn a_trailing_slash_mismatch_is_explained() {
    let t = setup().await;
    let p = Provider::new(
        uuid::Uuid::now_v7(),
        "slash",
        &format!("{ISSUER}/"),
        "client-1",
        chrono::Utc::now(),
    );
    let err = t.oidc.rediscover(&p).await.unwrap_err();
    assert!(err.detail.contains("trailing slash"), "{}", err.detail);
}
