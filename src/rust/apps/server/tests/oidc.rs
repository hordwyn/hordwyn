//! UX phase 3 WP3-2 over HTTP: several OIDC providers against the in-process test
//! issuer (`testkit::oidc`, RS256): the code exchange with PKCE, the signed single-use
//! pending cookie, state and mix-up checks (RFC 9207, RFC 9700 4.4), account resolution
//! (addendum-1 4.2 steps 1-4, OD-15), group rules (OD-16), the admin API with re-auth and
//! audit, the encrypted client secret and the read-only environment provider.

// End-to-end scenarios on purpose: the steps share one server and database, so a scenario
// stays one test (the longest, about 180 to 210 lines: `admin_resets_an_authenticator_and_
// unlinks_a_provider`, `emailed_link_links_a_passwordless_account_once`; UX4-19); short names
// (t: server, a: admin cookie, r: answer) keep the many calls readable.
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

mod support;

use std::sync::Arc;

use serde_json::{Value, json};
use server::{Overrides, Server};
use testkit::oidc::{AuthTime, Issuers, Login, TestIssuer};

const A: &str = "https://id-a.example.test";
const B: &str = "https://id-b.example.test";

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

struct T {
    app: axum::Router,
    /// The page routes too (the auto-mode redirect lives there).
    pages: axum::Router,
    srv: Server,
    a: Arc<TestIssuer>,
    b: Arc<TestIssuer>,
    /// The database, for a second process on it.
    db: String,
}

fn skip() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return true;
    }
    false
}

async fn start(extra: &str, env: Vec<(String, String)>) -> Option<T> {
    let db = store_pg::testing::fresh_db_url().await?;
    Some(start_on(&db, extra, env, None).await)
}

/// A server on `db`; with `issuers`, a second process sharing the test issuers of another.
async fn start_on(
    db: &str,
    extra: &str,
    env: Vec<(String, String)>,
    issuers: Option<(Arc<TestIssuer>, Arc<TestIssuer>)>,
) -> T {
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [server]\npublic_url = \"https://app.example.test\"\n\
         [brand]\ndir = \"{}\"\n[auth]\nsecure_cookies = false\n{extra}",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, env).unwrap();
    let (a, b) = issuers.unwrap_or_else(|| {
        (
            TestIssuer::new(A, "client-a", "secret-a"),
            TestIssuer::new(B, "client-b", "secret-b"),
        )
    });
    let http = Arc::new(Issuers(vec![a.clone(), b.clone()]));
    let srv = Server::build(
        cfg,
        config::ServerRole::All,
        Overrides {
            http: Some(http),
            ..Overrides::default()
        },
    )
    .await
    .unwrap();
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build();
    T {
        app: srv.base_router(),
        pages: srv.app_router(&options),
        srv,
        a,
        b,
        db: db.to_string(),
    }
}

struct Resp {
    status: u16,
    body: Value,
    text: String,
    cache_control: Option<String>,
    location: Option<String>,
    /// `name=value` of every Set-Cookie.
    cookies: Vec<String>,
}

impl Resp {
    fn cookie(&self, prefix: &str) -> Option<String> {
        self.cookies
            .iter()
            .find(|c| c.split('=').next().is_some_and(|n| n.ends_with(prefix)))
            .filter(|c| !c.ends_with('='))
            .cloned()
    }
}

/// One request as a browser or provider sends it.
struct Req<'a> {
    method: &'a str,
    /// Full path and query.
    uri: String,
    cookie: Option<&'a str>,
    body: String,
    content_type: &'a str,
    /// The socket peer (client address in audit rows).
    peer: Option<&'a str>,
}

impl<'a> Req<'a> {
    fn get(uri: &str, cookie: Option<&'a str>) -> Self {
        Self {
            method: "GET",
            uri: uri.into(),
            cookie,
            body: String::new(),
            content_type: "application/json",
            peer: None,
        }
    }
}

async fn send(router: &axum::Router, r: Req<'_>) -> Resp {
    let mut req = axum::http::Request::builder()
        .method(r.method)
        .uri(r.uri)
        .header("accept-language", "en")
        .header("content-type", r.content_type);
    if let Some(c) = r.cookie {
        req = req.header("cookie", c);
    }
    let mut req = req.body(axum::body::Body::from(r.body)).unwrap();
    if let Some(p) = r.peer {
        req.extensions_mut().insert(axum::extract::ConnectInfo(
            p.parse::<std::net::SocketAddr>().unwrap(),
        ));
    }
    let resp = support::send(router, req).await;
    let status = resp.status().as_u16();
    let header = |n: &str| {
        resp.headers()
            .get(n)
            .map(|v| v.to_str().unwrap().to_string())
    };
    let location = header("location");
    let cache_control = header("cache-control");
    let cookies = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string())
        .collect();
    let bytes = resp.into_body();
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        text: String::from_utf8_lossy(&bytes).into_owned(),
        cache_control,
        location,
        cookies,
    }
}

async fn call(t: &T, method: &str, uri: &str, cookie: Option<&str>, body: Option<Value>) -> Resp {
    send(
        &t.app,
        Req {
            method,
            uri: format!("/api/v1{uri}"),
            cookie,
            body: body.map_or_else(String::new, |b| b.to_string()),
            content_type: "application/json",
            peer: None,
        },
    )
    .await
}

/// A page visit (the web routes, not the API).
async fn page(t: &T, uri: &str, cookie: Option<&str>) -> Resp {
    send(&t.pages, Req::get(uri, cookie)).await
}

/// The query parameters of a URL.
fn params(url: &str) -> std::collections::HashMap<String, String> {
    let full = if url.starts_with('/') {
        format!("https://x.test{url}")
    } else {
        url.to_string()
    };
    url::Url::parse(&full)
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

/// The callback the provider's answer `back` (a query string) leads to.
async fn callback(t: &T, back: &str, cookies: &[Option<&str>]) -> Resp {
    call(
        t,
        "GET",
        &format!("/auth/oidc/callback?{back}"),
        Some(&join(cookies)),
        None,
    )
    .await
}

fn join(cookies: &[Option<&str>]) -> String {
    cookies
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join("; ")
}

/// The browser's whole round trip: start, the provider's page, callback. Answers the
/// callback response.
async fn round_trip(
    t: &T,
    issuer: &TestIssuer,
    start: &str,
    login: &Login,
    session: Option<&str>,
) -> Resp {
    let r = call(t, "GET", start, session, None).await;
    assert_eq!(r.status, 302, "start {start}: {}", r.body);
    let pending = r.cookie("_oidc").expect("pending cookie");
    let back = issuer.authorize(r.location.as_deref().unwrap(), login);
    call(
        t,
        "GET",
        &format!("/auth/oidc/callback?{back}"),
        Some(&join(&[Some(&pending), session])),
        None,
    )
    .await
}

fn account(name: &str, email: Option<&str>) -> Value {
    json!({"username": name, "display_name": name, "password": "correct horse battery",
           "email": email})
}

async fn first_admin(t: &T) -> String {
    let r = call(
        t,
        "POST",
        "/setup/account",
        None,
        Some(account("root", None)),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    r.cookie("_session").unwrap()
}

fn provider(slug: &str, issuer: &TestIssuer, extra: &Value) -> Value {
    let mut p = json!({
        "slug": slug,
        "display_name": format!("Provider {slug}"),
        "issuer": issuer.issuer,
        "client_id": issuer.client_id,
        "client_secret": issuer.client_secret,
        "auto_create": true,
    });
    for (k, v) in extra.as_object().unwrap() {
        p[k] = v.clone();
    }
    p
}

async fn add_provider(t: &T, admin: &str, slug: &str, issuer: &TestIssuer, extra: Value) -> Value {
    let r = call(
        t,
        "POST",
        "/admin/oidc/providers",
        Some(admin),
        Some(provider(slug, issuer, &extra)),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    r.body
}

async fn secret_rows(t: &T, admin: &str) -> u64 {
    let r = call(t, "GET", "/admin/security", Some(admin), None).await;
    r.body["secrets"]["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["rows"].as_u64().unwrap())
        .sum()
}

async fn me(t: &T, session: &str) -> Value {
    let r = call(t, "GET", "/auth/me", Some(session), None).await;
    assert_eq!(r.status, 200, "{}", r.body);
    r.body
}

#[tokio::test]
async fn env_provider_full_code_exchange_and_read_only_row() {
    if skip() {
        return;
    }
    let extra = format!(
        "[auth.oidc]\nissuer = \"{A}\"\nclient_id = \"client-a\"\ndisplay_name = \"Family SSO\"\n"
    );
    let env = vec![(
        "APP__AUTH__OIDC__CLIENT_SECRET".to_string(),
        "secret-a".to_string(),
    )];
    let Some(t) = start(&extra, env).await else {
        return;
    };
    // the very first account may come from the provider and becomes admin (as today)
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?next=/library",
        &Login::new("Anna.Admin").email("anna@example.test", true),
        None,
    )
    .await;
    assert_eq!(r.status, 302, "{}", r.body);
    assert_eq!(r.location.as_deref(), Some("/library"), "the return path");
    let session = r.cookie("_session").expect("signed in");
    assert!(
        r.cookies.iter().any(|c| c.ends_with("_oidc=")),
        "the pending cookie is cleared: {:?}",
        r.cookies
    );
    let u = me(&t, &session).await;
    assert_eq!(
        (u["username"].as_str(), u["role"].as_str()),
        (Some("anna.admin"), Some("admin"))
    );
    // PKCE: the token request carried the verifier (the issuer checked it against S256)
    let token =
        t.a.requests()
            .into_iter()
            .find(|r| r.url.ends_with("/token"))
            .unwrap();
    let body = String::from_utf8(token.body.unwrap()).unwrap();
    assert!(body.contains("code_verifier="), "{body}");
    assert!(
        t.a.authorize_requests()[0]["scope"].contains("email"),
        "default scopes"
    );

    // the environment provider is listed read-only with provenance env
    let r = call(&t, "GET", "/admin/oidc", Some(&session), None).await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        r.body["redirect_uri"],
        "https://app.example.test/api/v1/auth/oidc/callback"
    );
    let p = &r.body["providers"][0];
    assert_eq!(
        (p["provenance"].as_str(), p["display_name"].as_str()),
        (Some("env"), Some("Family SSO"))
    );
    assert_eq!(p["secret_set"], true);
    assert!(p.get("client_secret").is_none(), "{p}");
    assert!(p["last_callback_at"].is_string(), "{p}");
    let id = p["id"].as_str().unwrap().to_string();
    let r = call(
        &t,
        "PUT",
        &format!("/admin/oidc/providers/{id}"),
        Some(&session),
        Some(provider("sso", &t.a, &json!({}))),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("fixed_by_config"))
    );
    let r = call(
        &t,
        "DELETE",
        &format!("/admin/oidc/providers/{id}"),
        Some(&session),
        None,
    )
    .await;
    assert_eq!(r.status, 409);

    // the overview row comes from the stored callback result (no probe, DEC-285)
    let r = call(&t, "GET", "/admin/overview", Some(&session), None).await;
    let row = r.body["health"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["id"] == "identity_provider")
        .unwrap()
        .clone();
    assert_eq!(row["status"], "ok", "{row}");

    // the same identity signs in again as the same account; a second account is not made
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start",
        &Login::new("Anna.Admin"),
        None,
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/"));
    let again = r.cookie("_session").unwrap();
    assert_eq!(me(&t, &again).await["id"], u["id"]);
    // the public button list for the sign-in page
    let r = call(&t, "GET", "/auth/oidc/providers", None, None).await;
    assert_eq!(r.body[0]["display_name"], "Family SSO", "{}", r.body);
}

#[tokio::test]
async fn state_iss_and_replay_are_refused() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({})).await;
    let login = Login::new("mallory");
    let failed = |r: &Resp| {
        assert_eq!(r.status, 302, "{}", r.body);
        assert!(
            r.cookie("_session").is_none(),
            "no session: {:?}",
            r.cookies
        );
        r.location.clone().unwrap()
    };

    // a changed state
    let r = call(&t, "GET", "/auth/oidc/start?provider=a", None, None).await;
    let pending = r.cookie("_oidc").unwrap();
    let back = t.a.authorize(r.location.as_deref().unwrap(), &login);
    let bad = back.replace("state=", "state=x");
    let r = call(
        &t,
        "GET",
        &format!("/auth/oidc/callback?{bad}"),
        Some(&pending),
        None,
    )
    .await;
    assert_eq!(failed(&r), "/sign-in?error=oidc_failed");

    // a tampered pending cookie
    let r = call(&t, "GET", "/auth/oidc/start?provider=a", None, None).await;
    let pending = r.cookie("_oidc").unwrap();
    let back = t.a.authorize(r.location.as_deref().unwrap(), &login);
    let (name, value) = pending.split_once('=').unwrap();
    let mut forged = value.to_string();
    let flip = if forged.ends_with('A') { "B" } else { "A" };
    forged.replace_range(forged.len() - 1.., flip);
    let r = call(
        &t,
        "GET",
        &format!("/auth/oidc/callback?{back}"),
        Some(&format!("{name}={forged}")),
        None,
    )
    .await;
    assert_eq!(failed(&r), "/sign-in?error=oidc_failed");

    // a pending cookie works once (RFC 9700 4.2.4): the replay is refused
    let r = call(&t, "GET", "/auth/oidc/start?provider=a", None, None).await;
    let pending = r.cookie("_oidc").unwrap();
    let back = t.a.authorize(r.location.as_deref().unwrap(), &login);
    let uri = format!("/auth/oidc/callback?{back}");
    let ok = call(&t, "GET", &uri, Some(&pending), None).await;
    assert!(ok.cookie("_session").is_some(), "{:?}", ok.location);
    let r = call(&t, "GET", &uri, Some(&pending), None).await;
    assert_eq!(failed(&r), "/sign-in?error=oidc_failed");

    // RFC 9207: an advertised `iss` response parameter must name the issuer of the request
    t.a.behave(|b| {
        b.iss_parameter = true;
        b.callback_iss = Some(B.into());
    });
    let id = call(&t, "GET", "/admin/oidc", Some(&admin), None)
        .await
        .body["providers"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    // the Test refreshes the stored discovery (the provider now advertises the parameter)
    let r = call(
        &t,
        "POST",
        &format!("/admin/oidc/providers/{id}/test"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.body["iss_parameter"], true, "{}", r.body);
    let r = round_trip(&t, &t.a, "/auth/oidc/start?provider=a", &login, None).await;
    assert_eq!(failed(&r), "/sign-in?error=oidc_failed");
    // advertised but missing is refused too
    t.a.behave(|b| b.callback_iss = None);
    let r = call(&t, "GET", "/auth/oidc/start?provider=a", None, None).await;
    let pending = r.cookie("_oidc").unwrap();
    let back = t.a.authorize(r.location.as_deref().unwrap(), &login);
    let without: Vec<&str> = back.split('&').filter(|p| !p.starts_with("iss=")).collect();
    let r = call(
        &t,
        "GET",
        &format!("/auth/oidc/callback?{}", without.join("&")),
        Some(&pending),
        None,
    )
    .await;
    assert_eq!(failed(&r), "/sign-in?error=oidc_failed");
    // and the right one passes
    let r = round_trip(&t, &t.a, "/auth/oidc/start?provider=a", &login, None).await;
    assert!(r.cookie("_session").is_some(), "{:?}", r.location);

    // without the parameter, the ID token's `iss` is compared (Core 3.1.3.7 step 2)
    t.a.behave(|b| {
        b.iss_parameter = false;
        b.id_token_iss = Some(B.into());
    });
    call(
        &t,
        "POST",
        &format!("/admin/oidc/providers/{id}/test"),
        Some(&admin),
        None,
    )
    .await;
    let r = round_trip(&t, &t.a, "/auth/oidc/start?provider=a", &login, None).await;
    assert_eq!(failed(&r), "/sign-in?error=oidc_failed");
    t.a.behave(|b| b.id_token_iss = None);

    // the provider says no
    let r = call(&t, "GET", "/auth/oidc/start?provider=a", None, None).await;
    let pending = r.cookie("_oidc").unwrap();
    let back = t.a.deny(r.location.as_deref().unwrap(), "access_denied");
    let r = call(
        &t,
        "GET",
        &format!("/auth/oidc/callback?{back}"),
        Some(&pending),
        None,
    )
    .await;
    assert_eq!(failed(&r), "/sign-in?error=oidc_denied");
    // the callback result is stored for the overview row
    let r = call(&t, "GET", "/admin/oidc", Some(&admin), None).await;
    assert_eq!(
        r.body["providers"][0]["last_callback_result"], "failed: oidc_denied",
        "{}",
        r.body
    );
    // an open redirect is not a return path
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a&next=//evil.example",
        &login,
        None,
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/"));
    // unknown providers
    let r = call(&t, "GET", "/auth/oidc/start?provider=nope", None, None).await;
    assert_eq!(r.status, 404);
    let r = call(
        &t,
        "GET",
        "/auth/oidc/start?provider=a&mode=silent",
        None,
        None,
    )
    .await;
    assert_eq!(r.status, 302, "silent sign-in (WP3-3): {}", r.body);
    assert_eq!(
        params(r.location.as_deref().unwrap())["prompt"],
        "none",
        "Core 3.1.2.1"
    );
    let r = call(&t, "GET", "/auth/oidc/start?mode=nonsense", None, None).await;
    assert_eq!(r.status, 400);
}

#[tokio::test]
async fn two_providers_link_mode_and_unlink_guard() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({"is_default": true})).await;
    let b = add_provider(&t, &admin, "b", &t.b, json!({"auto_create": false})).await;
    let r = call(&t, "GET", "/auth/oidc/providers", None, None).await;
    assert_eq!(r.body.as_array().unwrap().len(), 2, "{}", r.body);

    // a new member through A (auto_create), no password
    let r = round_trip(&t, &t.a, "/auth/oidc/start", &Login::new("carla"), None).await;
    let carla = r.cookie("_session").expect("default provider A");
    assert_eq!(me(&t, &carla).await["username"], "carla");
    // B has no auto-create and sign-up is invite-only: no account for this sign-in
    let r = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b",
        &Login::new("carla-at-b"),
        None,
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/sign-in?error=no_account"));

    // the only sign-in method cannot be unlinked
    let r = call(&t, "GET", "/auth/me/identities", Some(&carla), None).await;
    assert_eq!(r.body.as_array().unwrap().len(), 1, "{}", r.body);
    assert_eq!(r.body[0]["can_unlink"], false);
    let a_id = r.body[0]["id"].as_str().unwrap().to_string();
    let r = call(
        &t,
        "DELETE",
        &format!("/auth/me/identities/{a_id}"),
        Some(&carla),
        None,
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("last_sign_in_method")),
        "{}",
        r.body
    );

    // link B from Settings (mode `link`), signed in as carla
    let r = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b&mode=link",
        &Login::new("carla-at-b"),
        Some(&carla),
    )
    .await;
    assert_eq!(
        r.location.as_deref(),
        Some("/settings/account"),
        "{}",
        r.body
    );
    let r = call(&t, "GET", "/auth/me/identities", Some(&carla), None).await;
    assert_eq!(r.body.as_array().unwrap().len(), 2, "{}", r.body);
    // now B signs carla in
    let r = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b",
        &Login::new("carla-at-b"),
        None,
    )
    .await;
    let via_b = r.cookie("_session").expect("linked");
    assert_eq!(me(&t, &via_b).await["username"], "carla");
    // link mode needs a signed-in browser
    let r = call(
        &t,
        "GET",
        "/auth/oidc/start?provider=b&mode=link",
        None,
        None,
    )
    .await;
    assert_eq!(r.status, 401);
    // another account cannot take an identity that is linked already
    let r = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b&mode=link",
        &Login::new("carla-at-b"),
        Some(&admin),
    )
    .await;
    assert_eq!(
        r.location.as_deref(),
        Some("/settings/account?error=identity_in_use")
    );
    // with two methods one may go
    let r = call(
        &t,
        "DELETE",
        &format!("/auth/me/identities/{a_id}"),
        Some(&via_b),
        None,
    )
    .await;
    assert_eq!(r.status, 204, "{}", r.body);

    // a provider removed between start and callback: the generic failure
    let r = call(&t, "GET", "/auth/oidc/start?provider=b", None, None).await;
    let pending = r.cookie("_oidc").unwrap();
    let back =
        t.b.authorize(r.location.as_deref().unwrap(), &Login::new("carla-at-b"));
    let id = b["id"].as_str().unwrap();
    let r = call(
        &t,
        "DELETE",
        &format!("/admin/oidc/providers/{id}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.status, 204, "{}", r.body);
    let r = call(
        &t,
        "GET",
        &format!("/auth/oidc/callback?{back}"),
        Some(&pending),
        None,
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/sign-in?error=oidc_failed"));
}

#[tokio::test]
async fn verified_email_never_links_without_the_password_and_usernames_get_a_suffix() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({})).await;
    // local accounts through admin invites
    for (name, email) in [("anna", "anna@example.test"), ("bob", "bob@example.test")] {
        let r = call(&t, "POST", "/admin/invites", Some(&admin), Some(json!({}))).await;
        assert_eq!(r.status, 201, "{}", r.body);
        let mut acc = account(name, Some(email));
        acc["invite"] = r.body["token"].clone();
        let r = call(&t, "POST", "/auth/sign-up", None, Some(acc)).await;
        assert_eq!(r.status, 201, "{}", r.body);
    }
    // OD-15: a verified email of a local account asks "Is this your account?"
    let login = Login::new("anna-sub").email("Anna@Example.test", true);
    let r = round_trip(&t, &t.a, "/auth/oidc/start?provider=a", &login, None).await;
    assert_eq!(r.location.as_deref(), Some("/sign-in/link"), "{}", r.body);
    assert!(
        r.cookie("_session").is_none(),
        "never signed in without the password"
    );
    let ticket = r.cookie("_oidc_link").expect("link ticket");
    let r = call(&t, "GET", "/auth/oidc/link", Some(&ticket), None).await;
    assert_eq!(
        (r.body["username"].as_str(), r.body["provider"].as_str()),
        (Some("anna"), Some("Provider a")),
        "{}",
        r.body
    );
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link",
        Some(&ticket),
        Some(json!({"password": "wrong password here"})),
    )
    .await;
    assert_eq!(r.status, 401, "{}", r.body);
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link",
        Some(&ticket),
        Some(json!({"password": "correct horse battery"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["user"]["username"], "anna");
    // the ticket is single use
    let again = call(
        &t,
        "POST",
        "/auth/oidc/link",
        Some(&ticket),
        Some(json!({"password": "correct horse battery"})),
    )
    .await;
    assert_eq!(again.status, 400, "{}", again.body);
    // from now on step 1 signs anna in directly
    let r = round_trip(&t, &t.a, "/auth/oidc/start?provider=a", &login, None).await;
    let s = r.cookie("_session").expect("linked identity");
    assert_eq!(me(&t, &s).await["username"], "anna");

    // an unverified email never matches; the taken name gets a suffix (step 4)
    let login = Login::new("Bob").email("bob@example.test", false);
    let r = round_trip(&t, &t.a, "/auth/oidc/start?provider=a", &login, None).await;
    let s = r.cookie("_session").expect("new account");
    let u = me(&t, &s).await;
    assert_eq!(
        (u["username"].as_str(), u["role"].as_str()),
        (Some("bob2"), Some("member"))
    );
}

#[tokio::test]
async fn group_rules_and_provider_roles() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    let p = add_provider(
        &t,
        &admin,
        "a",
        &t.a,
        json!({"allowed_groups": ["family", "admins"], "scopes": ["openid", "profile", "groups"]}),
    )
    .await;
    let id = p["id"].as_str().unwrap().to_string();
    // not in an allowed group: refused with the reason
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("eve").groups(&["guests"]),
        None,
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/sign-in?error=group_rule"));
    // groups only at UserInfo (authentik, Authelia defaults): read from there
    t.a.behave(|b| b.groups_in_id_token = false);
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("dora").groups(&["family"]),
        None,
    )
    .await;
    let dora = r
        .cookie("_session")
        .expect("allowed through UserInfo groups");
    assert_eq!(me(&t, &dora).await["role"], "member");
    t.a.behave(|b| b.groups_in_id_token = true);

    // OD-16: provider roles only with admin groups
    let mut body = provider("a", &t.a, &json!({"role_source": "provider"}));
    body.as_object_mut().unwrap().remove("client_secret");
    let r = call(
        &t,
        "PUT",
        &format!("/admin/oidc/providers/{id}"),
        Some(&admin),
        Some(body.clone()),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (422, Some("admin_groups_required")),
        "{}",
        r.body
    );
    body["admin_groups"] = json!(["admins"]);
    body["allowed_groups"] = json!(["family", "admins"]);
    let r = call(
        &t,
        "PUT",
        &format!("/admin/oidc/providers/{id}"),
        Some(&admin),
        Some(body.clone()),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    // a member of an admin group becomes admin at sign-in
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("dora").groups(&["family", "admins"]),
        None,
    )
    .await;
    let dora = r.cookie("_session").unwrap();
    assert_eq!(me(&t, &dora).await["role"], "admin");
    // the role is managed by the provider: badged and locked in Users
    let r = call(&t, "GET", "/admin/users", Some(&admin), None).await;
    let row = r
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "dora")
        .unwrap()
        .clone();
    assert_eq!(row["role_via_provider"], "Provider a", "{row}");
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{}/role", row["id"].as_str().unwrap()),
        Some(&admin),
        Some(json!({"role": "member"})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("role_managed_by_provider")),
        "{}",
        r.body
    );
    // and leaving the admin group demotes at the next sign-in
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("dora").groups(&["family"]),
        None,
    )
    .await;
    assert_eq!(
        me(&t, &r.cookie("_session").unwrap()).await["role"],
        "member"
    );

    // the acting admin links A while only in "family": a provider-role rule would demote
    // them, so it is refused (addendum-1 3.3)
    body["role_source"] = json!("local");
    call(
        &t,
        "PUT",
        &format!("/admin/oidc/providers/{id}"),
        Some(&admin),
        Some(body.clone()),
    )
    .await;
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a&mode=link",
        &Login::new("root-at-a").groups(&["family"]),
        Some(&admin),
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/settings/account"));
    body["role_source"] = json!("provider");
    let r = call(
        &t,
        "PUT",
        &format!("/admin/oidc/providers/{id}"),
        Some(&admin),
        Some(body.clone()),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("self_demotion")),
        "{}",
        r.body
    );
    // Test reports whether the groups claim was seen for the testing admin
    let r = call(
        &t,
        "POST",
        &format!("/admin/oidc/providers/{id}/test"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        (
            r.body["ok"].as_bool(),
            r.body["groups_claim_seen"].as_bool()
        ),
        (Some(true), Some(true)),
        "{}",
        r.body
    );
}

#[tokio::test]
async fn admin_api_secret_reauth_audit_and_test() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    // validation
    let mut bad = provider("Not A Slug", &t.a, &json!({}));
    let r = call(
        &t,
        "POST",
        "/admin/oidc/providers",
        Some(&admin),
        Some(bad.clone()),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (422, Some("invalid_slug"))
    );
    bad = provider("plain", &t.a, &json!({"issuer": "http://id.example.test"}));
    let r = call(&t, "POST", "/admin/oidc/providers", Some(&admin), Some(bad)).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (422, Some("insecure_issuer")),
        "plain http only for loopback while the public URL is https"
    );
    t.a.behave(|b| b.logout = true);
    let rows_before = secret_rows(&t, &admin).await;
    let p = add_provider(&t, &admin, "a", &t.a, json!({})).await;
    let id = p["id"].as_str().unwrap().to_string();
    assert_eq!(
        (p["secret_set"].as_bool(), p["provenance"].as_str()),
        (Some(true), Some("db"))
    );
    assert!(!p.to_string().contains("secret-a"), "{p}");
    let r = call(
        &t,
        "POST",
        "/admin/oidc/providers",
        Some(&admin),
        Some(provider("a", &t.a, &json!({}))),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("slug_taken"))
    );
    // the secret is encrypted at rest (purpose oidc_client_secret)
    let raw: Vec<u8> =
        sqlx::query_scalar("SELECT client_secret FROM oidc_providers WHERE slug = 'a'")
            .fetch_one(&t.srv.pool)
            .await
            .unwrap();
    assert!(
        !raw.windows(8).any(|w| w == b"secret-a"),
        "no plain text in the column"
    );
    // and counted by the key status (a `SecretRows` purpose: re-wrap and rotation see it)
    assert_eq!(secret_rows(&t, &admin).await, rows_before + 1);
    // the secret works (a sign-in through it), then is replaced and removed write-only
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("zoe"),
        None,
    )
    .await;
    assert!(r.cookie("_session").is_some(), "{:?}", r.location);
    let r = call(
        &t,
        "PUT",
        &format!("/admin/oidc/providers/{id}/secret"),
        Some(&admin),
        Some(json!({"secret": null})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["secret_set"], false);
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("zoe"),
        None,
    )
    .await;
    assert_eq!(
        r.location.as_deref(),
        Some("/sign-in?error=oidc_failed"),
        "the token endpoint refuses a client without its secret"
    );
    // audit rows in the change's transaction, never the value
    let r = call(
        &t,
        "GET",
        "/admin/audit?action=oidc_provider.",
        Some(&admin),
        None,
    )
    .await;
    assert!(
        r.body.to_string().contains("oidc_provider.created"),
        "{}",
        r.body
    );
    let r = call(&t, "GET", "/admin/audit?action=secret.", Some(&admin), None).await;
    let text = r.body.to_string();
    assert!(
        text.contains("secret.cleared") && text.contains("oidc_client_secret"),
        "{text}"
    );
    assert!(!text.contains("secret-a"));

    // Test: discovery, the logout capabilities, recorded for the overview
    let r = call(
        &t,
        "POST",
        &format!("/admin/oidc/providers/{id}/test"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(
        (
            r.body["ok"].as_bool(),
            r.body["end_session"].as_bool(),
            r.body["backchannel_logout"].as_bool(),
            r.body["pkce_s256"].as_bool(),
        ),
        (Some(true), Some(true), Some(true), Some(true)),
        "{}",
        r.body
    );
    assert!(
        r.body["groups_claim_seen"].is_null(),
        "the admin has no identity there"
    );
    t.a.behave(|b| b.down = true);
    let r = call(
        &t,
        "POST",
        &format!("/admin/oidc/providers/{id}/test"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.body["ok"], false, "{}", r.body);
    let r = call(&t, "GET", "/admin/overview", Some(&admin), None).await;
    let row = r.body["health"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["id"] == "identity_provider")
        .unwrap()
        .clone();
    assert_eq!(row["status"], "error", "{row}");
    // an outage during sign-in is the outage path, not a generic failure (the start uses
    // the cached discovery; the token endpoint does not answer)
    let r = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("zoe"),
        None,
    )
    .await;
    assert_eq!(
        r.location.as_deref(),
        Some("/sign-in?error=oidc_unavailable")
    );
    t.a.behave(|b| b.down = false);

    // DEC-191: provider changes need a fresh proof
    sqlx::query(
        "UPDATE auth_sessions SET reauth_at = now() - interval '2 hours', reauth_slide_at = NULL",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();
    let r = call(
        &t,
        "POST",
        "/admin/oidc/providers",
        Some(&admin),
        Some(provider("b", &t.b, &json!({}))),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("reauth_required"))
    );
    let r = call(
        &t,
        "DELETE",
        &format!("/admin/oidc/providers/{id}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.status, 403);
    // members never see the admin API
    let r = call(&t, "GET", "/admin/oidc", None, None).await;
    assert_eq!(r.status, 401);
}

// ------------------------------------------------------------------ WP3-3: SSO, logout, modes

/// An account with a password through an admin invite; answers its session cookie.
async fn invited(t: &T, admin: &str, name: &str) -> String {
    let r = call(t, "POST", "/admin/invites", Some(admin), Some(json!({}))).await;
    assert_eq!(r.status, 201, "{}", r.body);
    let mut acc = account(name, None);
    acc["invite"] = r.body["token"].clone();
    let r = call(t, "POST", "/auth/sign-up", None, Some(acc)).await;
    assert_eq!(r.status, 201, "{}", r.body);
    r.cookie("_session").unwrap()
}

/// Every session's proof is older than the re-auth window (DEC-191).
async fn stale(t: &T) {
    sqlx::query(
        "UPDATE auth_sessions SET reauth_at = now() - interval '2 hours', reauth_slide_at = NULL",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();
}

async fn set_mode(t: &T, admin: &str, mode: &str) -> Resp {
    call(
        t,
        "PUT",
        "/admin/settings",
        Some(admin),
        Some(json!({"values": {"auth.sign_in_mode": mode}})),
    )
    .await
}

async fn provider_id(t: &T, admin: &str, slug: &str) -> String {
    let r = call(t, "GET", "/admin/oidc", Some(admin), None).await;
    r.body["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["slug"] == slug)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn test_provider(t: &T, admin: &str, id: &str) -> Value {
    let r = call(
        t,
        "POST",
        &format!("/admin/oidc/providers/{id}/test"),
        Some(admin),
        None,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    r.body
}

/// Turn the authenticator of `username` on (the preconditions only read the flag).
async fn enrol_totp(t: &T, username: &str) {
    sqlx::query(
        "UPDATE credentials SET totp_enabled = true
         WHERE user_id = (SELECT id FROM users WHERE username = $1)",
    )
    .bind(username)
    .execute(&t.srv.pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn auto_mode_preconditions_probes_and_silent_sign_in() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({"is_default": true})).await;
    let id = provider_id(&t, &admin, "a").await;

    // addendum-1 4.3: `auto` needs a default provider with a passed Test in the last 24 h ...
    let r = set_mode(&t, &admin, "auto").await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("auto_needs_tested_provider")),
        "{}",
        r.body
    );
    let test = test_provider(&t, &admin, &id).await;
    assert_eq!(test["ok"], true, "{test}");
    assert!(
        test["silent_probe_url"]
            .as_str()
            .is_some_and(|u| u.contains("mode=probe_silent")),
        "{test}"
    );
    // ... and an admin with a password and an authenticator
    let r = set_mode(&t, &admin, "auto").await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("auto_needs_admin_authenticator")),
        "{}",
        r.body
    );
    enrol_totp(&t, "root").await;
    // ... and a fresh proof of the acting admin
    stale(&t).await;
    let r = set_mode(&t, &admin, "auto").await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("reauth_required"))
    );
    sqlx::query("UPDATE auth_sessions SET reauth_at = now()")
        .execute(&t.srv.pool)
        .await
        .unwrap();

    // OD-19: the silent probe, a prompt=none round trip by the testing admin
    let r = call(
        &t,
        "GET",
        "/auth/oidc/start?provider=a&mode=probe_silent&next=/admin",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.status, 302, "{}", r.body);
    let pending = r.cookie("_oidc").unwrap();
    let op = Login::new("root-at-a");
    let back = t.a.silent(r.location.as_deref().unwrap(), Some(&op));
    let r = callback(&t, &back, &[Some(&pending), Some(&admin)]).await;
    assert_eq!(r.location.as_deref(), Some("/admin?probe=silent_ok"));
    assert!(r.cookie("_session").is_none(), "a probe signs nobody in");
    let test = test_provider(&t, &admin, &id).await;
    assert_eq!(test["silent_probe"], "silent_ok", "{test}");

    let r = set_mode(&t, &admin, "auto").await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = call(
        &t,
        "GET",
        "/admin/audit?action=setting.",
        Some(&admin),
        None,
    )
    .await;
    assert!(
        r.body.to_string().contains("auth.sign_in_mode"),
        "{}",
        r.body
    );

    // a signed-out page visit goes straight to the provider (302), silent first; the body
    // is the interstitial for clients that do not follow redirects (role=status)
    let r = page(&t, "/library?view=grid", None).await;
    assert_eq!(r.status, 302, "{}", r.text);
    let to = r.location.clone().unwrap();
    assert!(
        to.starts_with("https://id-a.example.test/authorize?"),
        "{to}"
    );
    assert_eq!(params(&to)["prompt"], "none");
    assert!(r.text.contains("role=\"status\""), "{}", r.text);
    assert!(r.text.contains("Provider a"), "{}", r.text);
    assert_eq!(r.cache_control.as_deref(), Some("no-store"));
    let pending = r.cookie("_oidc").unwrap();
    let attempt = r.cookie("_oidc_silent").expect("loop guard cookie");
    // no OP session: login_required -> the interactive request, without our page
    let back = t.a.silent(&to, None);
    let r = callback(&t, &back, &[Some(&pending)]).await;
    assert_eq!(r.status, 302, "{}", r.body);
    let to = r.location.clone().unwrap();
    assert!(
        to.starts_with("https://id-a.example.test/authorize?"),
        "{to}"
    );
    assert!(!params(&to).contains_key("prompt"), "{to}");
    let pending = r.cookie("_oidc").unwrap();
    let back = t.a.authorize(&to, &Login::new("mia"));
    let r = callback(&t, &back, &[Some(&pending)]).await;
    assert_eq!(
        r.location.as_deref(),
        Some("/library?view=grid"),
        "next= kept"
    );
    let mia = r.cookie("_session").expect("signed in");
    assert!(
        r.cookie("_oidc_hint").is_some(),
        "hint cookie for the next silent try"
    );
    // signed in: pages are not redirected
    let r = page(&t, "/library", Some(&mia)).await;
    assert_eq!(r.location, None);

    // loop guard: within 5 minutes of a silent attempt the browser goes interactive
    let r = page(&t, "/", Some(&attempt)).await;
    let to = r.location.clone().unwrap();
    assert!(!params(&to).contains_key("prompt"), "{to}");

    // OD-20 and the exemptions: never redirected
    for path in [
        "/signed-out",
        "/sign-in/local",
        "/sign-in?error=oidc_denied",
        "/setup",
    ] {
        let r = page(&t, path, None).await;
        assert!(
            r.location.is_none(),
            "{path} redirected to {:?}",
            r.location
        );
    }
    // the sign-in page itself goes to the provider too
    let r = page(&t, "/sign-in?next=/stats", None).await;
    assert!(
        r.location
            .unwrap()
            .starts_with("https://id-a.example.test/")
    );

    // outage: the token endpoint does not answer -> outage path; after 3 failures in 10
    // minutes the redirect stops (no endless bouncing), page visits land on the outage page
    t.a.behave(|b| b.down = true);
    let mut last = String::new();
    for _ in 0..3 {
        let r = call(&t, "GET", "/auth/oidc/start", None, None).await;
        let pending = r.cookie("_oidc").unwrap();
        let back =
            t.a.authorize(r.location.as_deref().unwrap(), &Login::new("mia"));
        last = callback(&t, &back, &[Some(&pending)])
            .await
            .location
            .unwrap();
    }
    assert_eq!(last, "/sign-in?error=oidc_unavailable&retry=0");
    let r = page(&t, "/library", None).await;
    assert_eq!(
        r.location.as_deref(),
        Some("/sign-in?error=oidc_unavailable&retry=0")
    );
    t.a.behave(|b| b.down = false);

    // back to `page` (no precondition for that); visits are not sent to the provider
    let r = set_mode(&t, &admin, "page").await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = page(&t, "/library", None).await;
    assert!(
        !r.location
            .as_deref()
            .is_some_and(|l| l.starts_with("https://")),
        "{:?}",
        r.location
    );
}

#[tokio::test]
async fn break_glass_local_sign_in() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({"is_default": true})).await;
    let id = provider_id(&t, &admin, "a").await;
    test_provider(&t, &admin, &id).await;
    enrol_totp(&t, "root").await;
    invited(&t, &admin, "max").await;
    let ops = invited(&t, &admin, "ops").await;
    let ops_id = me(&t, &ops).await["id"].as_str().unwrap().to_string();
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{ops_id}/role"),
        Some(&admin),
        Some(json!({"role": "admin"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(set_mode(&t, &admin, "auto").await.status, 200);

    let creds = |name: &str| json!({"username": name, "password": "correct horse battery"});
    // auto mode: the normal password sign-in is gone for everyone ...
    let r = call(&t, "POST", "/auth/sign-in", None, Some(creds("ops"))).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("sso_only")),
        "{}",
        r.body
    );
    // ... the break-glass path refuses members ...
    let local = |body: Value| Req {
        method: "POST",
        uri: "/api/v1/auth/sign-in/local".into(),
        cookie: None,
        body: body.to_string(),
        content_type: "application/json",
        peer: Some("192.0.2.10:40000"),
    };
    let r = send(&t.app, local(creds("max"))).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("sso_only")),
        "{}",
        r.body
    );
    // ... uses TOTP when the account has it ...
    let r = send(&t.app, local(creds("root"))).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (401, Some("totp_required")),
        "{}",
        r.body
    );
    // ... and admits an admin with a password only (OD-17 as changed)
    let r = send(&t.app, local(creds("ops"))).await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert!(r.cookie("_session").is_some());
    // the audit row with the client address, and the admin notification
    let r = call(
        &t,
        "GET",
        "/admin/audit?action=auth.local_sign_in",
        Some(&admin),
        None,
    )
    .await;
    let text = r.body.to_string();
    assert!(
        text.contains("192.0.2.10") && text.contains("ops"),
        "{text}"
    );
    let r = call(&t, "GET", "/notifications/inbox", Some(&admin), None).await;
    let n = r
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["event"] == "admin.local_sign_in")
        .cloned()
        .unwrap_or_else(|| panic!("no notification: {}", r.body));
    assert!(n["body"].as_str().unwrap().contains("192.0.2.10"), "{n}");
    // wrong passwords are throttled like sign-in (DEC-155)
    let mut last = 0;
    for _ in 0..11 {
        last = send(
            &t.app,
            local(json!({"username": "ops", "password": "wrong password!!"})),
        )
        .await
        .status;
    }
    assert_eq!(last, 429);
    // in `page` mode the normal sign-in works again
    stale(&t).await;
    sqlx::query("UPDATE auth_sessions SET reauth_at = now()")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(set_mode(&t, &admin, "page").await.status, 200);
    let r = call(&t, "POST", "/auth/sign-in", None, Some(creds("max"))).await;
    assert_eq!(r.status, 200, "{}", r.body);
}

#[tokio::test]
async fn back_channel_logout_by_sid_and_by_sub() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    t.a.behave(|b| b.logout = true);
    add_provider(&t, &admin, "a", &t.a, json!({})).await;
    let start_a = "/auth/oidc/start?provider=a";
    let one = round_trip(&t, &t.a, start_a, &Login::new("mia").sid("s1"), None)
        .await
        .cookie("_session")
        .unwrap();
    let two = round_trip(&t, &t.a, start_a, &Login::new("mia").sid("s2"), None)
        .await
        .cookie("_session")
        .unwrap();
    let post = |token: &str| Req {
        method: "POST",
        uri: "/api/v1/auth/oidc/backchannel-logout".into(),
        cookie: None,
        body: format!("logout_token={token}"),
        content_type: "application/x-www-form-urlencoded",
        peer: None,
    };
    // Back-Channel Logout 2.7: the session with that sid ends, the other stays
    let token = t.a.logout_token(Some("mia"), Some("s1"));
    let r = send(&t.app, post(&token)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.cache_control.as_deref(), Some("no-store"));
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(&one), None).await.status,
        401
    );
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(&two), None).await.status,
        200
    );
    // 2.6 step 7: a jti seen within the window is refused
    let r = send(&t.app, post(&token)).await;
    assert_eq!(r.status, 400, "replay: {}", r.text);
    // the refusals of 2.6 (400, 2.8)
    let sign = |c: &Value| TestIssuer::sign_typed(c, "logout+jwt");
    let mut c = t.a.logout_claims(Some("mia"), None);
    c["nonce"] = json!("n");
    assert_eq!(send(&t.app, post(&sign(&c))).await.status, 400, "nonce");
    let mut c = t.a.logout_claims(Some("mia"), None);
    c.as_object_mut().unwrap().remove("events");
    assert_eq!(send(&t.app, post(&sign(&c))).await.status, 400, "events");
    let mut c = t.a.logout_claims(Some("mia"), None);
    c["aud"] = json!("another-client");
    assert_eq!(send(&t.app, post(&sign(&c))).await.status, 400, "aud");
    let mut c = t.a.logout_claims(Some("mia"), None);
    c["exp"] = json!(chrono::Utc::now().timestamp() - 600);
    assert_eq!(send(&t.app, post(&sign(&c))).await.status, 400, "exp");
    let c = t.a.logout_claims(None, None);
    assert_eq!(
        send(&t.app, post(&sign(&c))).await.status,
        400,
        "no sub, no sid"
    );
    let mut c = t.a.logout_claims(Some("mia"), None);
    c["iss"] = json!("https://unknown.example.test");
    assert_eq!(send(&t.app, post(&sign(&c))).await.status, 400, "issuer");
    let mut forged = t.a.logout_token(Some("mia"), None);
    let flip = if forged.ends_with('A') { "B" } else { "A" };
    let n = forged.len();
    forged.replace_range(n - 1.., flip);
    assert_eq!(send(&t.app, post(&forged)).await.status, 400, "signature");
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(&two), None).await.status,
        200
    );
    // only `sub`: every session of the identity through this provider ends
    let r = send(&t.app, post(&t.a.logout_token(Some("mia"), None))).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(&two), None).await.status,
        401
    );
    // the admin's own password session is untouched; the audit rows say what happened
    let r = call(
        &t,
        "GET",
        "/admin/audit?action=oidc.backchannel_logout",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    let text = r.body.to_string();
    assert!(
        text.contains("\"sessions\":1") && text.contains("sid"),
        "{text}"
    );
}

#[tokio::test]
async fn rp_initiated_logout_is_local_first() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    t.a.behave(|b| b.logout = true);
    add_provider(&t, &admin, "a", &t.a, json!({})).await;
    add_provider(&t, &admin, "b", &t.b, json!({})).await;
    let mia = round_trip(
        &t,
        &t.a,
        "/auth/oidc/start?provider=a",
        &Login::new("mia").sid("s1"),
        None,
    )
    .await
    .cookie("_session")
    .unwrap();
    // the ID token for `id_token_hint` is kept per session, encrypted
    let rows: Vec<Vec<u8>> = sqlx::query_scalar("SELECT value FROM session_secrets")
        .fetch_all(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].windows(3).any(|w| w == b"eyJ"), "not plain text");
    let logout = |cookie: Option<&str>| {
        let cookie = cookie.map(String::from);
        let app = t.app.clone();
        async move {
            send(
                &app,
                Req {
                    method: "POST",
                    uri: "/api/v1/auth/logout".into(),
                    cookie: cookie.as_deref(),
                    body: String::new(),
                    content_type: "application/x-www-form-urlencoded",
                    peer: None,
                },
            )
            .await
        }
    };
    let r = logout(Some(&mia)).await;
    assert_eq!(r.status, 303, "{}", r.text);
    let to = r.location.clone().unwrap();
    assert!(
        to.starts_with("https://id-a.example.test/end-session?"),
        "{to}"
    );
    let q = params(&to);
    assert_eq!(q["id_token_hint"], *t.a.id_tokens().last().unwrap());
    assert_eq!(q["client_id"], "client-a");
    assert_eq!(
        q["post_logout_redirect_uri"],
        "https://app.example.test/signed-out"
    );
    assert!(q.get("state").is_some_and(|s| s.len() >= 16), "{to}");
    // local first: our session is gone before the provider is asked
    assert!(
        r.cookies.iter().any(|c| c.ends_with("_session=")),
        "{:?}",
        r.cookies
    );
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(&mia), None).await.status,
        401
    );
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM session_secrets")
        .fetch_one(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(left, 0, "the hint is dropped with the session");
    // without an end_session_endpoint, and for password sessions: straight to /signed-out
    let via_b = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b",
        &Login::new("mia"),
        None,
    )
    .await
    .cookie("_session")
    .unwrap();
    for c in [Some(via_b.as_str()), Some(admin.as_str()), None] {
        let r = logout(c).await;
        assert_eq!(
            (r.status, r.location.as_deref()),
            (303, Some("/signed-out"))
        );
    }
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(&admin), None).await.status,
        401
    );
}

#[tokio::test]
async fn reauth_round_trip_with_max_age_zero() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({})).await;
    let dora_login = Login::new("dora");
    let dora = round_trip(&t, &t.a, "/auth/oidc/start?provider=a", &dora_login, None)
        .await
        .cookie("_session")
        .unwrap();
    let r = call(&t, "GET", "/auth/reauth", Some(&dora), None).await;
    assert_eq!(r.body["methods"], json!(["oidc"]), "{}", r.body);
    assert_eq!(r.body["oidc_provider"], "Provider a", "{}", r.body);
    stale(&t).await;
    // the gated action waits on the server while the browser is at the provider
    let action = json!({"kind": "unlink", "id": "x", "note": "kept"});
    let r = call(
        &t,
        "PUT",
        "/auth/reauth/pending",
        Some(&dora),
        Some(json!({"action": action})),
    )
    .await;
    assert_eq!(r.status, 204, "{}", r.body);
    // no session, no round trip
    let r = call(&t, "GET", "/auth/oidc/start?mode=reauth", None, None).await;
    assert_eq!(r.status, 401);
    let reauth = "/auth/oidc/start?mode=reauth&next=/settings/account";
    let r = call(&t, "GET", reauth, Some(&dora), None).await;
    assert_eq!(r.status, 302, "{}", r.body);
    let to = r.location.clone().unwrap();
    assert_eq!(params(&to)["max_age"], "0", "DEC-191: max_age=0");
    let pending = r.cookie("_oidc").unwrap();
    let back = t.a.authorize(&to, &dora_login);
    let r = callback(&t, &back, &[Some(&pending), Some(&dora)]).await;
    assert_eq!(
        r.location.as_deref(),
        Some("/settings/account?reauth=ok"),
        "{}",
        r.body
    );
    // the session id is regenerated with the proof
    let fresh = r.cookie("_session").expect("new session id");
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(&dora), None).await.status,
        401
    );
    let r = call(&t, "GET", "/auth/reauth", Some(&fresh), None).await;
    assert_eq!(
        (r.body["fresh"].as_bool(), r.body["method"].as_str()),
        (Some(true), Some("oidc:a")),
        "{}",
        r.body
    );
    // the pending action survives the new id and is handed back once
    let r = call(&t, "GET", "/auth/reauth/pending", Some(&fresh), None).await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["action"], action);
    let r = call(&t, "GET", "/auth/reauth/pending", Some(&fresh), None).await;
    assert_eq!(r.status, 404);

    // od-12 4.4 step 4: an old or missing auth_time is refused, never passed through
    for (behaviour, who) in [
        (AuthTime::SecondsAgo(900), dora_login.clone()),
        (AuthTime::Missing, dora_login.clone()),
        (AuthTime::Now, Login::new("someone-else")),
    ] {
        stale(&t).await;
        t.a.behave(|b| b.auth_time = behaviour);
        let r = call(&t, "GET", reauth, Some(&fresh), None).await;
        let pending = r.cookie("_oidc").unwrap();
        let back = t.a.authorize(r.location.as_deref().unwrap(), &who);
        let r = callback(&t, &back, &[Some(&pending), Some(&fresh)]).await;
        assert_eq!(
            r.location.as_deref(),
            Some("/settings/account?reauth_error=reauth_not_fresh"),
            "{behaviour:?}"
        );
        assert!(r.cookie("_session").is_none());
        let r = call(&t, "GET", "/auth/reauth", Some(&fresh), None).await;
        assert_eq!(r.body["fresh"], false, "{behaviour:?}");
    }

    // the provider Test's max_age probe (od-12 4.4 step 5)
    let id = provider_id(&t, &admin, "a").await;
    for (behaviour, outcome, supported) in [
        (AuthTime::Now, "max_age_ok", true),
        (AuthTime::SecondsAgo(900), "max_age_ignored", false),
    ] {
        t.a.behave(|b| b.auth_time = behaviour);
        let r = call(
            &t,
            "GET",
            "/auth/oidc/start?provider=a&mode=probe_max_age&next=/admin",
            Some(&admin),
            None,
        )
        .await;
        let to = r.location.clone().unwrap();
        assert_eq!(params(&to)["max_age"], "0");
        let pending = r.cookie("_oidc").unwrap();
        let back = t.a.authorize(&to, &Login::new("root-at-a"));
        let r = callback(&t, &back, &[Some(&pending), Some(&admin)]).await;
        assert_eq!(r.location, Some(format!("/admin?probe={outcome}")));
        let test = test_provider(&t, &admin, &id).await;
        assert_eq!(test["max_age_probe"], supported, "{test}");
    }
}

/// The owner's provider (authentik) and `max_age=0` (od-12 3.4, 4.4): run by hand with
/// `KK_OIDC_LIVE_ISSUER`, `KK_OIDC_LIVE_CLIENT_ID`, `KK_OIDC_LIVE_CLIENT_SECRET` and
/// `KK_OIDC_LIVE_PUBLIC_URL` (its `/api/v1/auth/oidc/callback` registered as redirect URI)
/// plus `TEST_DATABASE_URL`. The test prints an authorization URL; open it, sign in, and
/// paste the address the browser ends up on into the file `KK_OIDC_LIVE_CALLBACK_FILE`
/// (default `/tmp/oidc-live-callback`) within five minutes. It passes when the provider
/// reported a fresh `auth_time` (the probe outcome `max_age_ok`).
#[tokio::test]
#[ignore = "needs a real provider and a person at the browser"]
async fn live_provider_honours_max_age_zero() {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"));
    let issuer = var("KK_OIDC_LIVE_ISSUER");
    let public = var("KK_OIDC_LIVE_PUBLIC_URL");
    let file = std::env::var("KK_OIDC_LIVE_CALLBACK_FILE")
        .unwrap_or_else(|_| "/tmp/oidc-live-callback".into());
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [server]\npublic_url = \"{public}\"\n[brand]\ndir = \"{}\"\n[auth]\nsecure_cookies = false\n\
         [auth.oidc]\nissuer = \"{issuer}\"\nclient_id = \"{}\"\n",
        repo("config/brand/default"),
        var("KK_OIDC_LIVE_CLIENT_ID"),
    );
    let env = vec![(
        "APP__AUTH__OIDC__CLIENT_SECRET".to_string(),
        var("KK_OIDC_LIVE_CLIENT_SECRET"),
    )];
    let cfg = config::AppConfig::from_sources(&toml, env).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    let api = |uri: &str, cookie: Option<String>, body: Option<Value>| {
        let app = app.clone();
        let uri = uri.to_string();
        async move {
            send(
                &app,
                Req {
                    method: if body.is_some() { "POST" } else { "GET" },
                    uri: format!("/api/v1{uri}"),
                    cookie: cookie.as_deref(),
                    body: body.map_or_else(String::new, |b| b.to_string()),
                    content_type: "application/json",
                    peer: None,
                },
            )
            .await
        }
    };
    let r = api("/setup/account", None, Some(account("root", None))).await;
    let admin = r.cookie("_session").expect("first account");
    let r = api(
        "/auth/oidc/start?mode=probe_max_age&next=/admin",
        Some(admin.clone()),
        None,
    )
    .await;
    let pending = r.cookie("_oidc").expect("pending cookie");
    let _ = std::fs::remove_file(&file);
    eprintln!(
        "open this URL, sign in, then write the address you land on into {file}:\n{}",
        r.location.unwrap()
    );
    let mut landed = String::new();
    for _ in 0..300 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if let Ok(s) = std::fs::read_to_string(&file)
            && !s.trim().is_empty()
        {
            landed = s.trim().to_string();
            break;
        }
    }
    let query = landed.split_once('?').expect("a callback address").1;
    let r = api(
        &format!("/auth/oidc/callback?{query}"),
        Some(format!("{pending}; {admin}")),
        None,
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/admin?probe=max_age_ok"));
}

// ------------------------------------------------------------------ WP4-2: admin-ops

/// DEC-336: the Users row menu. An admin removes another person's authenticator and unlinks
/// one of their providers; both need a fresh proof, are audited, notify the person, and the
/// last way to sign in is refused. DEC-388 (1): the providers count their linked accounts.
#[tokio::test]
async fn admin_resets_an_authenticator_and_unlinks_a_provider() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({"is_default": true})).await;
    add_provider(&t, &admin, "b", &t.b, json!({})).await;
    // carla: through A (no password), then B linked from Settings
    let r = round_trip(&t, &t.a, "/auth/oidc/start", &Login::new("carla"), None).await;
    let carla = r.cookie("_session").unwrap();
    let carla_id = me(&t, &carla).await["id"].as_str().unwrap().to_string();
    let r = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b&mode=link",
        &Login::new("carla-at-b"),
        Some(&carla),
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/settings/account"));
    // dan: through A only; and a password account with an authenticator
    let r = round_trip(&t, &t.a, "/auth/oidc/start", &Login::new("dan"), None).await;
    let dan = r.cookie("_session").unwrap();
    let dan_id = me(&t, &dan).await["id"].as_str().unwrap().to_string();
    let eve = invited(&t, &admin, "eve").await;
    let eve_id = me(&t, &eve).await["id"].as_str().unwrap().to_string();
    enrol_totp(&t, "eve").await;

    // the providers table counts the accounts linked through each
    let r = call(&t, "GET", "/admin/oidc", Some(&admin), None).await;
    let count = |slug: &str| {
        r.body["providers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["slug"] == slug)
            .unwrap()["linked_accounts"]
            .clone()
    };
    assert_eq!((count("a"), count("b")), (json!(2), json!(1)), "{}", r.body);

    // Users lists each person's linked sign-ins for the row menu
    let r = call(&t, "GET", "/admin/users", Some(&admin), None).await;
    let row = |id: &str| {
        r.body
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["id"] == id)
            .unwrap()
            .clone()
    };
    let carla_row = row(&carla_id);
    let ids = carla_row["identities"].as_array().unwrap().clone();
    assert_eq!(ids.len(), 2, "{carla_row}");
    let a_identity = ids.iter().find(|i| i["provider_slug"] == "a").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let dan_identity = row(&dan_id)["identities"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(row(&dan_id)["identities"][0]["can_unlink"], false);

    // both need a fresh proof
    stale(&t).await;
    let reset = format!("/admin/users/{eve_id}/reset-totp");
    let unlink = format!("/admin/users/{carla_id}/identities/{a_identity}");
    let r = call(&t, "POST", &reset, Some(&admin), None).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("reauth_required"))
    );
    let r = call(&t, "DELETE", &unlink, Some(&admin), None).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("reauth_required"))
    );
    let r = call(
        &t,
        "POST",
        "/auth/reauth",
        Some(&admin),
        Some(json!({"password": "correct horse battery"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    let admin = r.cookie("_session").unwrap_or(admin);

    // members never reach them
    let r = call(&t, "POST", &reset, Some(&dan), None).await;
    assert_eq!(r.status, 403);

    // reset the authenticator: gone, audited, the person is told
    let r = call(&t, "POST", &reset, Some(&admin), None).await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert!(
        !r.body["signs_in_with"].to_string().contains("totp"),
        "{}",
        r.body
    );
    let enabled: bool =
        sqlx::query_scalar("SELECT totp_enabled FROM credentials WHERE user_id = $1::uuid")
            .bind(&eve_id)
            .fetch_one(&t.srv.pool)
            .await
            .unwrap();
    assert!(!enabled);
    let r = call(&t, "POST", &reset, Some(&admin), None).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("no_authenticator"))
    );
    let inbox = call(&t, "GET", "/notifications/inbox", Some(&eve), None).await;
    assert!(
        inbox.text.contains("account.authenticator_reset"),
        "{}",
        inbox.text
    );

    // unlink: the last way to sign in is refused; with two, one may go
    let r = call(
        &t,
        "DELETE",
        &format!("/admin/users/{dan_id}/identities/{dan_identity}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("last_sign_in_method")),
        "{}",
        r.body
    );
    let r = call(
        &t,
        "DELETE",
        &format!("/admin/users/{carla_id}/identities/{dan_identity}"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.status, 404, "another person's identity: {}", r.body);
    let r = call(&t, "DELETE", &unlink, Some(&admin), None).await;
    assert_eq!(r.status, 204, "{}", r.body);
    let r = call(&t, "GET", "/auth/me/identities", Some(&carla), None).await;
    assert_eq!(r.body.as_array().unwrap().len(), 1, "{}", r.body);
    let inbox = call(&t, "GET", "/notifications/inbox", Some(&carla), None).await;
    assert!(
        inbox.text.contains("account.sign_in_unlinked"),
        "{}",
        inbox.text
    );
    let r = call(&t, "GET", "/admin/oidc", Some(&admin), None).await;
    assert_eq!(r.body["providers"][0]["linked_accounts"], 1, "{}", r.body);

    // both audited with the proof
    for action in ["user.totp_reset", "user.identity_unlinked"] {
        let r = call(
            &t,
            "GET",
            &format!("/admin/audit?action={action}"),
            Some(&admin),
            None,
        )
        .await;
        let rows = r.body["entries"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{action}: {}", r.body);
        assert_eq!(rows[0]["detail"]["proof"], "password", "{}", r.body);
    }
}

/// Reads every message a fake SMTP server receives (the DATA part).
async fn smtp_outbox() -> (u16, Arc<std::sync::Mutex<Vec<String>>>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let outbox = Arc::new(std::sync::Mutex::new(Vec::new()));
    let out = outbox.clone();
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            let out = out.clone();
            tokio::spawn(async move {
                let (r, mut w) = sock.into_split();
                let mut lines = BufReader::new(r).lines();
                w.write_all(b"220 test ESMTP\r\n").await.unwrap();
                let mut data: Option<String> = None;
                while let Ok(Some(l)) = lines.next_line().await {
                    let reply: &[u8] = if let Some(msg) = data.as_mut() {
                        if l == "." {
                            out.lock().unwrap().push(data.take().unwrap_or_default());
                            b"250 queued\r\n"
                        } else {
                            msg.push_str(&l);
                            msg.push('\n');
                            continue;
                        }
                    } else if l.starts_with("EHLO") {
                        b"250-test\r\n250 AUTH PLAIN LOGIN\r\n"
                    } else if l.starts_with("DATA") {
                        data = Some(String::new());
                        b"354 go on\r\n"
                    } else if l.starts_with("QUIT") {
                        let _ = w.write_all(b"221 bye\r\n").await;
                        break;
                    } else {
                        b"250 ok\r\n"
                    };
                    if w.write_all(reply).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    (port, outbox)
}

/// The one-time code in the newest message.
fn mailed_code(outbox: &std::sync::Mutex<Vec<String>>) -> String {
    let all = outbox.lock().unwrap();
    // the body may be quoted-printable (RFC 2045 6.7): soft line breaks and `=3D`
    let msg = all
        .last()
        .expect("a message")
        .replace("=\n", "")
        .replace("=3D", "=");
    let at = msg.find("/sign-in/link?code=").expect("the link") + "/sign-in/link?code=".len();
    msg[at..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

/// DEC-312/316, addendum-1 4.2 step 3: a password-less account whose verified address a
/// second provider names gets "Email me a link" when mail works. The link works once, for 30
/// minutes, only in the browser holding the link ticket, and never as a password reset.
#[tokio::test]
async fn emailed_link_links_a_passwordless_account_once() {
    if skip() {
        return;
    }
    let (port, outbox) = smtp_outbox().await;
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    add_provider(&t, &admin, "a", &t.a, json!({"is_default": true})).await;
    add_provider(&t, &admin, "b", &t.b, json!({})).await;
    // anna's account comes from A: no password, the verified address is kept
    let anna_a = Login::new("anna").email("anna@example.test", true);
    let r = round_trip(&t, &t.a, "/auth/oidc/start", &anna_a, None).await;
    let anna = r.cookie("_session").unwrap();
    let anna_id = me(&t, &anna).await["id"].as_str().unwrap().to_string();
    // B names the same verified address: "Is this your account?"
    let link_via_b = |sub: &str| Login::new(sub).email("Anna@Example.test", true);
    let r = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b",
        &link_via_b("anna-b"),
        None,
    )
    .await;
    assert_eq!(r.location.as_deref(), Some("/sign-in/link"), "{}", r.body);
    let ticket = r.cookie("_oidc_link").unwrap();

    // without mail there is no offer
    let r = call(&t, "GET", "/auth/oidc/link", Some(&ticket), None).await;
    assert_eq!(
        (
            r.body["has_password"].as_bool(),
            r.body["can_email"].as_bool()
        ),
        (Some(false), Some(false)),
        "{}",
        r.body
    );
    let r = call(&t, "POST", "/auth/oidc/link/email", Some(&ticket), None).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("email_unavailable")),
        "{}",
        r.body
    );

    let values = json!({"values": {
        "mail.host": "127.0.0.1", "mail.port": port, "mail.security": "none",
        "mail.from": "Shelf <noreply@example.org>"
    }});
    let r = call(&t, "PUT", "/admin/settings", Some(&admin), Some(values)).await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = call(&t, "GET", "/auth/oidc/link", Some(&ticket), None).await;
    assert_eq!(r.body["can_email"], true, "{}", r.body);

    // the link goes to the stored address; the ticket now lasts as long as the link
    let r = call(&t, "POST", "/auth/oidc/link/email", Some(&ticket), None).await;
    assert_eq!(r.status, 202, "{}", r.body);
    let ticket = r.cookie("_oidc_link").expect("the ticket is renewed");
    assert_eq!(outbox.lock().unwrap().len(), 1);
    assert!(outbox.lock().unwrap()[0].contains("anna@example.test"));
    let code = mailed_code(&outbox);
    assert!(code.len() > 20, "{code}");
    let stored: String =
        sqlx::query_scalar("SELECT token_hash FROM password_resets WHERE purpose = 'oidc_link'")
            .fetch_one(&t.srv.pool)
            .await
            .unwrap();
    assert_ne!(stored, code, "only the hash is stored");

    // never a password reset
    let r = call(
        &t,
        "POST",
        "/auth/reset-password",
        None,
        Some(json!({"token": code, "password": "another horse battery"})),
    )
    .await;
    assert!(r.status >= 400, "{}", r.body);
    // another browser: no ticket, or the ticket of another link request
    let confirm = json!({"code": code});
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link/email/confirm",
        None,
        Some(confirm.clone()),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (400, Some("link_expired"))
    );
    let r = round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b",
        &link_via_b("anna-b2"),
        None,
    )
    .await;
    let other = r.cookie("_oidc_link").unwrap();
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link/email/confirm",
        Some(&other),
        Some(confirm.clone()),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (400, Some("code_invalid"))
    );

    // UX4-11: the offer exists only without a password; one set since the link was mailed
    // makes the link void (and does not use up the code)
    sqlx::query(
        "INSERT INTO credentials (user_id, password_hash) VALUES ($1::uuid, 'x')
         ON CONFLICT (user_id) DO UPDATE SET password_hash = 'x'",
    )
    .bind(&anna_id)
    .execute(&t.srv.pool)
    .await
    .unwrap();
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link/email/confirm",
        Some(&ticket),
        Some(confirm.clone()),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("email_unavailable")),
        "{}",
        r.body
    );
    sqlx::query("UPDATE credentials SET password_hash = NULL WHERE user_id = $1::uuid")
        .bind(&anna_id)
        .execute(&t.srv.pool)
        .await
        .unwrap();

    // the right browser: linked and signed in, audited
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link/email/confirm",
        Some(&ticket),
        Some(confirm.clone()),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["user"]["id"], anna_id.as_str());
    assert!(r.cookie("_session").is_some());
    let r = call(&t, "GET", "/auth/me/identities", Some(&anna), None).await;
    assert_eq!(r.body.as_array().unwrap().len(), 2, "{}", r.body);
    let r = call(
        &t,
        "GET",
        "/admin/audit?action=auth.link_emailed",
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(r.body["entries"].as_array().unwrap().len(), 1, "{}", r.body);
    // once
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link/email/confirm",
        Some(&ticket),
        Some(confirm),
    )
    .await;
    assert_eq!(r.status, 400, "{}", r.body);

    // an expired link is refused
    let r = call(&t, "POST", "/auth/oidc/link/email", Some(&other), None).await;
    assert_eq!(r.status, 202, "{}", r.body);
    let other = r.cookie("_oidc_link").unwrap();
    let code = mailed_code(&outbox);
    sqlx::query("UPDATE password_resets SET expires_at = created_at WHERE purpose = 'oidc_link'")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let r = call(
        &t,
        "POST",
        "/auth/oidc/link/email/confirm",
        Some(&other),
        Some(json!({"code": code})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (400, Some("code_invalid"))
    );

    // sending is throttled per account
    let mut last = 0;
    for _ in 0..5 {
        last = call(&t, "POST", "/auth/oidc/link/email", Some(&other), None)
            .await
            .status;
    }
    assert_eq!(last, 429);
}

/// Back-Channel Logout 2.6 step 7 across processes (DEC-326): a `jti` one process accepted
/// is refused by another process on the same database.
#[tokio::test]
async fn back_channel_jti_replay_through_a_second_process_is_refused() {
    if skip() {
        return;
    }
    let key = format!(
        "v1:{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [7u8; 32])
    );
    let env = || vec![("APP__SECRETS__KEYS".to_string(), key.clone())];
    let Some(t) = start("", env()).await else {
        return;
    };
    let admin = first_admin(&t).await;
    t.a.behave(|b| b.logout = true);
    add_provider(&t, &admin, "a", &t.a, json!({})).await;
    let two = start_on(&t.db, "", env(), Some((t.a.clone(), t.b.clone()))).await;
    let start_a = "/auth/oidc/start?provider=a";
    round_trip(&t, &t.a, start_a, &Login::new("mia").sid("s1"), None).await;
    let post = |token: &str| Req {
        method: "POST",
        uri: "/api/v1/auth/oidc/backchannel-logout".into(),
        cookie: None,
        body: format!("logout_token={token}"),
        content_type: "application/x-www-form-urlencoded",
        peer: None,
    };
    let token = t.a.logout_token(Some("mia"), Some("s1"));
    assert_eq!(send(&t.app, post(&token)).await.status, 200);
    let r = send(&two.app, post(&token)).await;
    assert_eq!(
        r.status, 400,
        "replay through the other process: {}",
        r.text
    );
    // a new token is fine on either
    let token = t.a.logout_token(Some("mia"), None);
    assert_eq!(send(&two.app, post(&token)).await.status, 200);
}

/// UX4-10: the row menu refuses the caller's own row, keeps an `auto` instance's break-glass
/// admin, and counts only sign-ins through an enabled provider as another way in.
#[tokio::test]
async fn admin_menu_refuses_self_keeps_break_glass_and_counts_enabled_providers() {
    if skip() {
        return;
    }
    let Some(t) = start("", vec![]).await else {
        return;
    };
    let admin = first_admin(&t).await;
    let admin_id = me(&t, &admin).await["id"].as_str().unwrap().to_string();
    add_provider(&t, &admin, "a", &t.a, json!({"is_default": true})).await;
    add_provider(&t, &admin, "b", &t.b, json!({})).await;
    let own = [
        ("POST", format!("/admin/users/{admin_id}/reset-totp")),
        (
            "DELETE",
            format!("/admin/users/{admin_id}/identities/{}", uuid::Uuid::nil()),
        ),
    ];
    for (method, uri) in &own {
        let r = call(&t, method, uri, Some(&admin), None).await;
        assert_eq!(
            (r.status, r.body["code"].as_str()),
            (409, Some("self_change")),
            "{uri}"
        );
    }

    // carla: A and B, no password; with B disabled, A is her only way in
    let r = round_trip(&t, &t.a, "/auth/oidc/start", &Login::new("carla"), None).await;
    let carla = r.cookie("_session").unwrap();
    let carla_id = me(&t, &carla).await["id"].as_str().unwrap().to_string();
    round_trip(
        &t,
        &t.b,
        "/auth/oidc/start?provider=b&mode=link",
        &Login::new("carla-at-b"),
        Some(&carla),
    )
    .await;
    sqlx::query("UPDATE oidc_providers SET enabled = false WHERE slug = 'b'")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let r = call(&t, "GET", "/auth/me/identities", Some(&carla), None).await;
    let a_identity = r
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["provider_slug"] == "a")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let uri = format!("/admin/users/{carla_id}/identities/{a_identity}");
    let r = call(&t, "DELETE", &uri, Some(&admin), None).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("last_sign_in_method")),
        "{}",
        r.body
    );

    // auto mode: eve is the only admin with a password and an authenticator
    let eve = invited(&t, &admin, "eve").await;
    let eve_id = me(&t, &eve).await["id"].as_str().unwrap().to_string();
    enrol_totp(&t, "eve").await;
    sqlx::query("UPDATE users SET role = 'admin' WHERE username = 'eve'")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let id = provider_id(&t, &admin, "a").await;
    test_provider(&t, &admin, &id).await;
    let r = set_mode(&t, &admin, "auto").await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = call(
        &t,
        "POST",
        &format!("/admin/users/{eve_id}/reset-totp"),
        Some(&admin),
        None,
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("auto_needs_admin_authenticator")),
        "{}",
        r.body
    );
}
