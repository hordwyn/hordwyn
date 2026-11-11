//! UX phase 2 server functions over HTTP (the API the UI packages build on), wired through
//! the server's `web::admin_backend::AdminBackend`.

// One end-to-end scenario on purpose: the steps share one server and database.
#![allow(clippy::too_many_lines)]

use crate::support;

use leptos::server_fn::ServerFn;
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

const FORM: &str = "application/x-www-form-urlencoded";
const JSON: &str = "application/json";

async fn send(
    app: &axum::Router,
    uri: &str,
    cookie: Option<&str>,
    content_type: &str,
    body: String,
) -> (u16, String, Option<String>) {
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
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = resp.into_body();
    (status, String::from_utf8_lossy(&bytes).into_owned(), cookie)
}

fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
}

#[tokio::test]
async fn setup_admin_and_source_server_functions() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
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
    let app = srv.app_router(&options);

    let (status, body, _) = send(
        &app,
        web::admin_api::SetupState::PATH,
        None,
        FORM,
        String::new(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(json(&body)["step"], "account");
    let (status, body, cookie) = send(
        &app,
        web::admin_api::SetupAccount::PATH,
        None,
        FORM,
        "username=root&display_name=Root&password=correct+horse+battery".into(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let admin = cookie.expect("signed in");
    let a = Some(admin.as_str());
    let (status, body, _) = send(
        &app,
        web::admin_api::AdminOverview::PATH,
        a,
        FORM,
        String::new(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(json(&body)["health"].as_array().unwrap().len(), 8);
    let values = serde_json::json!({"values": {"storage.max_total_bytes": "2GiB"}}).to_string();
    let (status, body, _) = send(
        &app,
        web::admin_api::UpdateAdminSettings::PATH,
        a,
        JSON,
        values,
    )
    .await;
    assert_eq!(status, 200, "{body}");
    let (status, body, _) = send(
        &app,
        web::admin_api::ReauthStatus::PATH,
        a,
        FORM,
        String::new(),
    )
    .await;
    assert_eq!(
        (status, json(&body)["fresh"].as_bool()),
        (200, Some(true)),
        "{body}"
    );
    let (status, body, fresh) = send(
        &app,
        web::admin_api::Reauth::PATH,
        a,
        FORM,
        "password=correct+horse+battery".into(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert!(fresh.is_some_and(|c| c != admin), "new session id");

    // a member: status yes, admin functions no (the code travels to the page)
    let (_, _, member) = send(
        &app,
        "/api/v1/auth/sign-up",
        None,
        JSON,
        r#"{"username":"mia","display_name":"Mia","password":"correct horse battery"}"#.into(),
    )
    .await;
    let m = member.as_deref();
    let (status, body, _) = send(
        &app,
        web::admin_api::SourceStatus::PATH,
        m,
        FORM,
        String::new(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(json(&body)["is_admin"], false);
    for path in [
        web::admin_api::AdminOverview::PATH,
        web::admin_api::AdminUsers::PATH,
        web::admin_api::AdminSecurity::PATH,
        web::admin_api::StorageJobs::PATH,
    ] {
        let (status, body, _) = send(&app, path, m, FORM, String::new()).await;
        assert_ne!(status, 200, "{path}");
        assert!(body.contains("forbidden"), "{path}: {body}");
    }
    let (status, body, _) = send(
        &app,
        web::admin_api::AskAdmin::PATH,
        m,
        FORM,
        "group=play".into(),
    )
    .await;
    assert_eq!(status, 200, "{body}");
}
