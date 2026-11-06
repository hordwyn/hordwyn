//! OIDC discovery and the start of the authorization-code flow, against replayed responses
//! through the `HttpFetch` port (the governed client in production).

use std::sync::Arc;

use auth::oidc::{CONNECTOR, Oidc, OidcSettings};
use testkit::fakes::ReplayHttp;

const ISSUER: &str = "https://id.example.org";

fn discovery() -> serde_json::Value {
    serde_json::json!({
        "issuer": ISSUER,
        "authorization_endpoint": "https://id.example.org/authorize",
        "token_endpoint": "https://id.example.org/token",
        "jwks_uri": "https://id.example.org/jwks",
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"]
    })
}

#[tokio::test]
async fn start_builds_a_pkce_authorization_url() {
    let http = Arc::new(ReplayHttp::default());
    http.record(
        "GET",
        &format!("{ISSUER}/.well-known/openid-configuration"),
        200,
        discovery().to_string(),
    );
    http.record("GET", "https://id.example.org/jwks", 200, r#"{"keys":[]}"#);
    let oidc = Oidc::new(
        OidcSettings {
            issuer: ISSUER.into(),
            client_id: "client-1".into(),
            client_secret: Some("s3cret".into()),
            redirect_url: "https://app.example.org/api/v1/auth/oidc/callback".into(),
            auto_create: false,
        },
        http.clone(),
    );
    let (url, pending) = oidc.start().await.unwrap();
    let u = url::Url::parse(&url).unwrap();
    assert_eq!(u.host_str(), Some("id.example.org"));
    let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
    assert_eq!(q["client_id"], "client-1");
    assert_eq!(q["response_type"], "code");
    assert_eq!(q["code_challenge_method"], "S256");
    assert_eq!(q["state"], pending.csrf);
    assert_eq!(q["nonce"], pending.nonce);
    assert!(q["scope"].contains("openid"));
    let reqs = http.requests();
    assert!(
        reqs.iter().all(|r| r.connector == CONNECTOR),
        "governed connector id"
    );
    // discovery is cached: a second start does not fetch again
    oidc.start().await.unwrap();
    assert_eq!(http.requests().len(), reqs.len());
}
