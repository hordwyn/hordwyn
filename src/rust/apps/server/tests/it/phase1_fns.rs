//! UX phase 1 server functions over HTTP, wired through the server's `web::Backend`.

use crate::support;

use leptos::server_fn::ServerFn;
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn app() -> axum::Router {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build();
    srv.app_router(&options)
}

async fn send(
    app: &axum::Router,
    uri: &str,
    cookie: Option<&str>,
    content_type: &str,
    body: String,
) -> (u16, serde_json::Value) {
    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", content_type)
        .header("accept", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let resp = support::send(app, req.body(axum::body::Body::from(body)).unwrap()).await;
    let status = resp.status().as_u16();
    let bytes = resp.into_body();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn sign_up(app: &axum::Router) -> String {
    let resp = support::send(
        app,
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/auth/sign-up")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                r#"{"username":"anna","display_name":"Anna","password":"correct horse battery"}"#,
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status(), 201, "sign-up");
    let cookie = resp.headers()["set-cookie"].to_str().unwrap();
    cookie.split(';').next().unwrap().to_string()
}

const FORM: &str = "application/x-www-form-urlencoded";

#[tokio::test]
async fn home_library_activity_and_stats_server_functions() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let app = app().await;
    let (status, _) = send(&app, web::api::Home::PATH, None, FORM, String::new()).await;
    assert_ne!(status, 200, "needs a session");
    let cookie = sign_up(&app).await;
    let c = Some(cookie.as_str());

    let (status, home) = send(&app, web::api::Home::PATH, c, FORM, String::new()).await;
    assert_eq!(status, 200, "{home}");
    assert_eq!(home["library_total"], 0);
    assert_eq!(home["stats"]["groups"][0]["group"], "watch");

    let (status, kinds) = send(&app, web::api::Kinds::PATH, c, FORM, String::new()).await;
    assert_eq!(status, 200, "{kinds}");
    assert_eq!(kinds["groups"][3]["id"], "play");

    let body = serde_json::json!({"query": {"group": "watch", "sort": "title"}}).to_string();
    let (status, rows) = send(
        &app,
        web::api::LibraryQuery::PATH,
        c,
        "application/json",
        body,
    )
    .await;
    assert_eq!(status, 200, "{rows}");
    assert_eq!(rows["total"], 0);

    let (status, counts) = send(&app, web::api::LibraryCounts::PATH, c, FORM, String::new()).await;
    assert_eq!((status, counts["total"].as_u64()), (200, Some(0)));

    let body = serde_json::json!({"query": {"limit": 10}}).to_string();
    let (status, feed) = send(
        &app,
        web::api::ActivityFeed::PATH,
        c,
        "application/json",
        body,
    )
    .await;
    assert_eq!(status, 200, "{feed}");
    assert_eq!(feed["rows"], serde_json::json!([]));

    let (status, series) = send(
        &app,
        web::api::StatsSeries::PATH,
        c,
        FORM,
        "group=read&bucket=year".into(),
    )
    .await;
    assert_eq!(status, 200, "{series}");
    assert_eq!(
        (series["group"].as_str(), series["bucket"].as_str()),
        (Some("read"), Some("year"))
    );

    let (status, tags) = send(&app, web::api::Tags::PATH, c, FORM, String::new()).await;
    assert_eq!((status, tags), (200, serde_json::json!([])));

    let (status, _) = send(
        &app,
        web::api::UndoProgress::PATH,
        c,
        FORM,
        "event_id=01900000-0000-7000-8000-000000000001".into(),
    )
    .await;
    assert_ne!(status, 200, "unknown event");
}
