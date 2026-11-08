//! UX phase 2 setup wizard page and admin nudge (design.md 4.3, 4.4, 5.1, 6.1): before the
//! first account exists every page redirects to `/setup` (the API and static files do not);
//! the wizard page renders and resumes; after completion `/setup` is the 404 page. The
//! source status carries `setup_open` for admins only, and the Home nudge shows for admins,
//! never for members.

// One scenario on purpose: the steps share one server and database.
#![allow(clippy::too_many_lines)]

mod support;

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

struct Resp {
    status: u16,
    location: Option<String>,
    cookie: Option<String>,
    body: String,
}

async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: &str,
) -> Resp {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("accept-language", "en")
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let resp = support::send(
        app,
        req.body(axum::body::Body::from(body.to_string())).unwrap(),
    )
    .await;
    let status = resp.status().as_u16();
    let header = |name| {
        resp.headers()
            .get(name)
            .map(|v: &axum::http::HeaderValue| v.to_str().unwrap().to_string())
    };
    let location = header(axum::http::header::LOCATION);
    let cookie =
        header(axum::http::header::SET_COOKIE).map(|c| c.split(';').next().unwrap().to_string());
    let bytes = resp.into_body();
    Resp {
        status,
        location,
        cookie,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    }
}

async fn get(app: &axum::Router, uri: &str, cookie: Option<&str>) -> Resp {
    send(app, "GET", uri, cookie, "").await
}

fn account(name: &str) -> String {
    format!(r#"{{"username":"{name}","display_name":"{name}","password":"correct horse battery"}}"#)
}

fn services() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return false;
    }
    true
}

#[tokio::test]
async fn every_page_leads_to_the_wizard_until_the_first_account_then_setup_is_404() {
    if !services() {
        return;
    }
    let app = app().await;

    // no account yet: every page redirects to the wizard (design.md 5.1)
    for uri in [
        "/",
        "/library",
        "/sign-in",
        "/sign-up",
        "/admin",
        "/settings/account",
        "/nope",
    ] {
        let r = get(&app, uri, None).await;
        assert_eq!(r.status, 303, "{uri}");
        assert_eq!(r.location.as_deref(), Some("/setup"), "{uri}");
    }
    // ... but not the API, health or static files
    assert_eq!(get(&app, "/healthz", None).await.status, 200);
    assert_eq!(get(&app, "/api/v1/setup", None).await.status, 200);
    assert_ne!(get(&app, "/pkg/app.css", None).await.status, 303);
    assert_ne!(get(&app, "/favicon.ico", None).await.status, 303);
    // the wizard itself renders its first step
    let r = get(&app, "/setup", None).await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("data-testid=\"wizard\""), "wizard");
    assert!(r.body.contains("Create the admin account"), "account step");
    assert!(r.body.contains("aria-label=\"Setup progress\""), "steps");

    // step 1 through the API: the account exists, pages render again
    let r = send(
        &app,
        "POST",
        "/api/v1/setup/account",
        None,
        &account("root"),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    assert_eq!(get(&app, "/sign-in", None).await.status, 200);
    assert_eq!(get(&app, "/", a).await.status, 200);
    // resumable: the wizard continues at the hosting step
    let r = get(&app, "/setup", a).await;
    assert_eq!(r.status, 200);
    assert!(r.body.contains("How do you host"), "hosting step");
    let r = send(&app, "POST", "/api/v1/auth/sign-up", None, &account("mia")).await;
    let member = r.cookie.unwrap();
    let m = Some(member.as_str());

    // palette status fields: admins learn that the setup is open, members never do
    let st: serde_json::Value =
        serde_json::from_str(&get(&app, "/api/v1/sources/status", a).await.body).unwrap();
    assert_eq!(st["is_admin"], true);
    assert_eq!(st["setup_open"], true, "{st}");
    assert!(st["groups_without_source"].as_array().is_some());
    let st: serde_json::Value =
        serde_json::from_str(&get(&app, "/api/v1/sources/status", m).await.body).unwrap();
    assert_eq!(st["is_admin"], false);
    assert_eq!(st["setup_open"], false, "{st}");

    // the Home nudge: admins only, status first with one action
    let html = get(&app, "/", a).await.body;
    assert!(
        html.contains("data-testid=\"nudge\""),
        "admin sees the nudge"
    );
    assert!(
        html.contains("href=\"/setup\""),
        "the open setup is the action"
    );
    let html = get(&app, "/", m).await.body;
    assert!(!html.contains("data-testid=\"nudge\""), "members never");

    // finish: the wizard is locked, `/setup` is the 404 page
    for step in ["hosting", "sources", "complete"] {
        let body = if step == "hosting" {
            r#"{"hosting":"home"}"#
        } else {
            ""
        };
        let r = send(&app, "POST", &format!("/api/v1/setup/{step}"), a, body).await;
        assert!(r.status < 300, "{step}: {} {}", r.status, r.body);
    }
    for c in [a, m, None] {
        let r = get(&app, "/setup", c).await;
        assert_eq!(r.status, 404);
        assert!(
            r.body.contains("There is nothing at this address"),
            "the 404 page"
        );
    }
    let st: serde_json::Value =
        serde_json::from_str(&get(&app, "/api/v1/sources/status", a).await.body).unwrap();
    assert_eq!(st["setup_open"], false, "{st}");
    // with the setup done the nudge speaks about sources only (no source for some groups)
    let html = get(&app, "/", a).await.body;
    assert!(
        html.contains("data-testid=\"nudge\""),
        "groups without source"
    );
    assert!(
        html.contains("href=\"/admin/sources\""),
        "one action: set up"
    );
}

/// The palette's scope segments (design.md 6.4): a search scoped to a kind group that no
/// source serves asks nobody and names the group, so the no-results card can say why.
#[tokio::test]
async fn a_search_scoped_to_a_group_without_a_source_names_it() {
    if !services() {
        return;
    }
    let app = app().await;
    let r = send(
        &app,
        "POST",
        "/api/v1/setup/account",
        None,
        &account("root"),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    let r = send(&app, "POST", "/api/v1/auth/sign-up", None, &account("mia")).await;
    let member = r.cookie.unwrap();
    let r = get(&app, "/api/v1/search?q=catan&group=play", Some(&member)).await;
    assert_eq!(r.status, 200, "{}", r.body);
    let v: serde_json::Value = serde_json::from_str(&r.body).unwrap();
    assert_eq!(
        v["groups_without_source"],
        serde_json::json!(["play"]),
        "{v}"
    );
    assert_eq!(v["pending"], serde_json::json!([]), "nobody to ask: {v}");
    // an unknown group is refused rather than searching everything
    let r = get(&app, "/api/v1/search?q=catan&group=nope", Some(&member)).await;
    assert_eq!(r.status, 400, "{}", r.body);
}

/// Hosted mode (design.md 4.1, 4.3, 6.10): the wizard's sources step reminds of the free
/// tiers and offers "Agreement on file" per source; afterwards the Data sources page shows
/// the hosting-mode banner until an agreement is recorded, and never in home mode.
#[tokio::test]
async fn hosted_mode_shows_the_licence_banner_until_agreements_are_recorded() {
    if !services() {
        return;
    }
    let app = app().await;
    let r = send(
        &app,
        "POST",
        "/api/v1/setup/account",
        None,
        &account("root"),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    let banner = "Hosted mode: record for each data source";

    // home mode (the default): no banner
    let html = get(&app, "/admin/sources", a).await.body;
    assert!(
        html.contains("data-testid=\"src-table\""),
        "the sources page renders"
    );
    assert!(!html.contains(banner), "home mode has no banner");

    let r = send(
        &app,
        "POST",
        "/api/v1/setup/hosting",
        a,
        r#"{"hosting":"hosted"}"#,
    )
    .await;
    assert!(r.status < 300, "{} {}", r.status, r.body);
    let html = get(&app, "/setup", a).await.body;
    assert!(
        html.contains("do not cover paid services"),
        "free-tier reminder"
    );
    assert!(
        html.contains("Agreement on file"),
        "licence checkbox per source"
    );
    let html = get(&app, "/admin/sources", a).await.body;
    assert!(html.contains(banner), "hosted: banner until recorded");
}
