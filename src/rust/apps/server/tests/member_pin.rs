#![cfg(feature = "tvdb")]
//! The user-supported model of `TheTVDB` (design.md 4.1, DEC-246 follow-up): a member who
//! stored their own subscriber PIN under Settings > Data sources searches with it; others
//! search with the instance key alone.

mod support;

use std::time::Duration;

use serde_json::{Value, json};
use server::{Overrides, Server};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn call(
    app: &axum::Router,
    m: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (u16, Option<String>) {
    let mut req = axum::http::Request::builder()
        .method(m)
        .uri(format!("/api/v1{uri}"))
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let body = body.map_or_else(String::new, |b| b.to_string());
    let resp = support::send(app, req.body(axum::body::Body::from(body)).unwrap()).await;
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    (resp.status().as_u16(), cookie)
}

/// Login bodies `TheTVDB` received so far, waiting up to 5 s for `n` of them.
async fn logins(mock: &MockServer, n: usize) -> Vec<Value> {
    for _ in 0..100 {
        let got: Vec<Value> = mock
            .received_requests()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|r| r.url.path().ends_with("/login"))
            .map(|r| serde_json::from_slice(&r.body).unwrap())
            .collect();
        if got.len() >= n {
            return got;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("TheTVDB saw fewer than {n} logins");
}

#[tokio::test]
async fn a_members_search_uses_their_own_pin() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let tvdb = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {"token": "t"}})))
        .mount(&tvdb)
        .await;
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .mount(&tvdb)
        .await;
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n\
         [connectors.tvdb]\napi_key = \"project-key\"\nbase_url = \"{}\"\n",
        repo("config/brand/default"),
        tvdb.uri()
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    let account =
        |n: &str| json!({"username": n, "display_name": n, "password": "correct horse battery"});
    let admin = call(&app, "POST", "/auth/sign-up", None, Some(account("root")))
        .await
        .1
        .unwrap();
    let member = call(&app, "POST", "/auth/sign-up", None, Some(account("mia")))
        .await
        .1
        .unwrap();
    let (status, _) = call(
        &app,
        "PUT",
        "/me/credentials/tvdb",
        Some(&member),
        Some(json!({"fields": {"pin": "5678"}})),
    )
    .await;
    assert_eq!(status, 200);

    assert_eq!(
        call(&app, "GET", "/search?q=lost&kind=show", Some(&member), None)
            .await
            .0,
        200
    );
    let seen = logins(&tvdb, 1).await;
    assert_eq!(
        seen[0],
        json!({"apikey": "project-key", "pin": "5678"}),
        "the member's PIN"
    );

    assert_eq!(
        call(&app, "GET", "/search?q=lost&kind=show", Some(&admin), None)
            .await
            .0,
        200
    );
    let seen = logins(&tvdb, 2).await;
    assert_eq!(
        seen[1],
        json!({"apikey": "project-key"}),
        "no PIN of their own: the instance key"
    );
}

async fn json_of(app: &axum::Router, m: &str, uri: &str, cookie: &str) -> (u16, Value) {
    let req = axum::http::Request::builder()
        .method(m)
        .uri(format!("/api/v1{uri}"))
        .header("cookie", cookie)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = support::send(app, req).await;
    let status = resp.status().as_u16();
    let bytes = resp.into_body();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// UX2-S2: a member's wrong PIN never marks the instance key rejected for everyone; and
/// UX2-12: the member reads and removes their own credential over HTTP, values never shown.
#[tokio::test]
async fn a_wrong_member_pin_stays_the_members_problem() {
    use wiremock::matchers::body_partial_json;
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let tvdb = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .and(body_partial_json(json!({"pin": "0000"})))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"status": "failure"})))
        .with_priority(1)
        .mount(&tvdb)
        .await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {"token": "t"}})))
        .mount(&tvdb)
        .await;
    Mock::given(method("GET"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
        .mount(&tvdb)
        .await;
    let (_srv, app, admin, member) = with_member_pin(&tvdb, "0000").await;
    let (status, mine) = json_of(&app, "GET", "/me/credentials", &member).await;
    assert_eq!(status, 200);
    let tv = mine
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["source"] == "tvdb")
        .unwrap()
        .clone();
    assert_eq!(
        (tv["set"].as_bool(), tv["locked"].as_bool()),
        (Some(true), Some(false))
    );
    assert!(!mine.to_string().contains("0000"), "never returned: {mine}");
    let (_, theirs) = json_of(&app, "GET", "/me/credentials", &admin).await;
    assert!(
        theirs.as_array().unwrap().iter().all(|c| c["set"] == false),
        "the admin does not see the member's credential: {theirs}"
    );

    // the member's search logs in with the wrong PIN and is refused
    assert_eq!(
        call(&app, "GET", "/search?q=lost&kind=show", Some(&member), None)
            .await
            .0,
        200
    );
    let seen = logins(&tvdb, 1).await;
    assert_eq!(seen[0]["pin"], "0000");
    tokio::time::sleep(Duration::from_millis(200)).await;
    // the instance key is not marked rejected: the admin's sources and searches still use it
    let (_, sources) = json_of(&app, "GET", "/admin/sources", &admin).await;
    let st = sources
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["status"]["id"] == "tvdb")
        .unwrap()["status"]
        .clone();
    assert_ne!(st["state"], "key_rejected", "{st}");
    assert_eq!(st["last_auth_status"], Value::Null, "{st}");
    let (_, found) = json_of(&app, "GET", "/search?q=lost&kind=show", &admin).await;
    assert!(
        !found["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "tvdb"),
        "{found}"
    );

    // the member removes it again
    let (status, _) = json_of(&app, "DELETE", "/me/credentials/tvdb", &member).await;
    assert_eq!(status, 200);
    let (_, mine) = json_of(&app, "GET", "/me/credentials", &member).await;
    assert!(
        mine.as_array().unwrap().iter().all(|c| c["set"] == false),
        "{mine}"
    );
}

/// A server using `tvdb` with an instance key, an admin and a member who stored `pin`.
async fn with_member_pin(tvdb: &MockServer, pin: &str) -> (Server, axum::Router, String, String) {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n\
         [search]\nbackend = \"postgres\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n\
         [brand]\ndir = \"{}\"\n[connectors.tvdb]\napi_key = \"project-key\"\nbase_url = \"{}\"\n",
        repo("config/brand/default"),
        tvdb.uri()
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    let account =
        |n: &str| json!({"username": n, "display_name": n, "password": "correct horse battery"});
    let admin = call(&app, "POST", "/auth/sign-up", None, Some(account("root")))
        .await
        .1
        .unwrap();
    let member = call(&app, "POST", "/auth/sign-up", None, Some(account("mia")))
        .await
        .1
        .unwrap();
    let (status, _) = call(
        &app,
        "PUT",
        "/me/credentials/tvdb",
        Some(&member),
        Some(json!({"fields": {"pin": pin}})),
    )
    .await;
    assert_eq!(status, 200);
    (srv, app, admin, member)
}
