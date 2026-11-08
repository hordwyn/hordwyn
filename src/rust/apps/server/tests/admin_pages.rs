//! UX phase 2 admin area and settings area (addendum-1 3.4, design.md 3.7, 6.10, 6.12,
//! 6.13): the server renders each of the eight admin pages for an admin and a clear
//! "admins only" page for a member; the settings pages render under `/settings/*` with the
//! side nav; the old paths are gone (no redirects, DEC-171).

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

async fn page(app: &axum::Router, uri: &str, cookie: Option<&str>) -> (u16, String) {
    let mut req = axum::http::Request::builder()
        .uri(uri)
        .header("accept-language", "en");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let resp = support::send(app, req.body(axum::body::Body::empty()).unwrap()).await;
    let status = resp.status().as_u16();
    let body = resp.into_body();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

async fn sign_up(app: &axum::Router, username: &str) -> String {
    let body = format!(
        r#"{{"username":"{username}","display_name":"{username}","password":"correct horse battery"}}"#
    );
    let resp = support::send(
        app,
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/auth/sign-up")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status(), 201, "sign-up");
    let cookie = resp.headers()["set-cookie"].to_str().unwrap();
    cookie.split(';').next().unwrap().to_string()
}

fn services() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return false;
    }
    true
}

/// The eight admin pages of addendum-1 3.4 (DEC-255 reversed): path, H1 and one thing only
/// the admin view renders.
const ADMIN_PAGES: [(&str, &str, &str); 8] = [
    ("/admin", ">Overview<", "data-testid=\"brand-swatch\""),
    ("/admin/instance", ">Instance<", "How you host"),
    (
        "/admin/sso",
        ">Sign-in and SSO<",
        "/api/v1/auth/oidc/callback",
    ),
    ("/admin/users", ">Users<", "data-testid=\"user-row\""),
    (
        "/admin/sources",
        ">Data sources<",
        "data-testid=\"src-table\"",
    ),
    (
        "/admin/notifications",
        ">Notification defaults<",
        "data-testid=\"defaults-table\"",
    ),
    ("/admin/storage", ">Storage and jobs<", "Run eviction now"),
    ("/admin/audit", ">Audit log<", "/api/v1/admin/audit.csv"),
];

// multi-threaded like production (thread-local page state aborts the stream there)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_admin_page_renders_for_an_admin_and_refuses_a_member() {
    if !services() {
        return;
    }
    let app = app().await;
    // the first account is the instance admin
    let admin = sign_up(&app, "root").await;
    let member = sign_up(&app, "mia").await;
    for (uri, h1, marker) in ADMIN_PAGES {
        let (status, html) = page(&app, uri, Some(&admin)).await;
        assert_eq!(status, 200, "{uri}");
        assert!(html.contains(h1), "{uri}: heading");
        assert!(html.contains(marker), "{uri}: {marker}");
        assert!(html.contains("data-testid=\"admin-page\""), "{uri}");
        // the side nav with both groups, this page marked current
        assert!(html.contains("aria-label=\"Settings\""), "{uri}: side nav");
        assert!(html.contains(">Instance (admin)<"), "{uri}: admin group");
        assert!(
            html.contains(&format!("href=\"{uri}\" aria-current=\"page\"")),
            "{uri}: current link"
        );
        let (status, html) = page(&app, uri, Some(&member)).await;
        assert_eq!(status, 200, "{uri} member");
        assert!(
            html.contains("data-testid=\"admin-only\""),
            "{uri}: refused"
        );
        assert!(
            !html.contains("data-testid=\"admin-page\""),
            "{uri}: no data"
        );
        assert!(!html.contains(marker), "{uri}: member sees no {marker}");
        assert!(
            !html.contains(">Instance (admin)<"),
            "{uri}: no admin group"
        );
    }
    // page details an admin relies on
    let (_, html) = page(&app, "/admin/users", Some(&admin)).await;
    assert!(html.contains(">mia<"), "users table lists the member");
    assert!(html.contains("(you)"), "the admin's own row");
    // sign-up mode lives on Instance now, one place per setting (addendum-1 3.4)
    assert!(
        !html.contains("Who can create an account"),
        "no sign-up mode on Users"
    );
    instance_sso_and_sign_out(&app, &admin).await;
    // UX2-W1, W2: the TMDB card says how to get a key, with the developer page; home mode
    // has no licence record
    let (_, html) = page(&app, "/admin/sources", Some(&admin)).await;
    assert!(
        html.contains("data-testid=\"key-howto\""),
        "how to get a key"
    );
    assert!(html.contains(">How to get a key<"), "its label");
    assert!(
        html.contains("href=\"https://developer.themoviedb.org/reference/intro/getting-started\""),
        "developer link"
    );
    assert!(
        !html.contains("Agreement on file"),
        "home mode: no licence checkbox"
    );
    // UX2-W21: a signed-out visitor gets the refusal without the "You" side nav
    let (status, html) = page(&app, "/admin/sources", None).await;
    assert_eq!(status, 200);
    assert!(html.contains("data-testid=\"admin-only\""), "refused");
    assert!(!html.contains("aria-label=\"Settings\""), "no side nav");
    assert!(
        html.contains("Sign in with an admin account"),
        "points to sign-in"
    );
    // the admin entry (account menu and You hub) leads to the admin area
    let (_, html) = page(&app, "/you", Some(&admin)).await;
    assert!(html.contains("href=\"/admin\""), "admin entry");
    let (_, html) = page(&app, "/you", Some(&member)).await;
    assert!(
        !html.contains("href=\"/admin\""),
        "no admin entry for members"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn settings_pages_live_under_settings_and_the_old_paths_are_gone() {
    if !services() {
        return;
    }
    let app = app().await;
    let _admin = sign_up(&app, "root").await;
    let member = sign_up(&app, "max").await;
    for (uri, h1) in [
        ("/settings/account", ">Account<"),
        ("/settings/notifications", ">Notifications<"),
        ("/settings/integrations", ">Integrations<"),
        ("/settings/sources", ">Data sources<"),
        ("/settings/imports", ">Imports and export<"),
        ("/settings/privacy", ">Privacy<"),
    ] {
        let (status, html) = page(&app, uri, Some(&member)).await;
        assert_eq!(status, 200, "{uri}");
        assert!(html.contains(h1), "{uri}: heading");
        assert!(html.contains("aria-label=\"Settings\""), "{uri}: side nav");
        assert!(
            html.contains(&format!("href=\"{uri}\" aria-current=\"page\"")),
            "{uri}: current link"
        );
    }
    let (_, html) = page(&app, "/settings/account", Some(&member)).await;
    assert!(html.contains("Change password"), "password form");
    assert!(html.contains("Set up an authenticator"), "TOTP enrolment");
    assert!(html.contains("role=\"radiogroup\" aria-label=\"Language\""));
    // phase 3: linked sign-ins and the time zone with its suggestions (DEC-306, DEC-312)
    assert!(html.contains(">Linked sign-ins<"), "linked sign-ins");
    assert!(html.contains("No provider linked."), "none linked yet");
    assert!(html.contains(">Time zone<"), "time zone");
    assert!(
        html.contains("Times are shown in UTC"),
        "no zone chosen yet"
    );
    assert!(html.contains("list=\"set-tz-zones\""), "suggestions list");
    let (_, html) = page(&app, "/settings/sources", Some(&member)).await;
    assert!(
        html.contains("data-testid=\"member-sources\""),
        "status list"
    );
    assert!(html.contains("id=\"attribution\""), "attribution list");
    for uri in ["/notifications", "/integrations", "/sources"] {
        let (status, _) = page(&app, uri, Some(&member)).await;
        assert_eq!(status, 404, "{uri}");
    }
    // the bell and the You hub point at the new paths
    let (_, html) = page(&app, "/you", Some(&member)).await;
    assert!(html.contains("href=\"/settings/notifications\""), "bell");
    assert!(html.contains("href=\"/settings/account\""), "settings link");
}

/// Phase 3 (WP3-4): Instance, Sign-in and SSO, the folded-in pages and the sign-out form.
async fn instance_sso_and_sign_out(app: &axum::Router, admin: &str) {
    let (_, html) = page(app, "/admin/instance", Some(admin)).await;
    assert!(html.contains("Who can create an account"), "sign-up mode");
    for mode in ["Home", "Hosted"] {
        assert!(
            html.contains(&format!("id=\"adm-hosting-{}\"", mode.to_lowercase())),
            "hosting radio {mode}"
        );
    }
    // the public address is read-only, from the configuration
    assert!(
        html.contains("data-testid=\"public-url\""),
        "public address"
    );
    // the disclosures: Mail, Secrets at rest, Advanced (sessions and cookies)
    for (id, title) in [
        ("adm-mail", "Mail"),
        ("adm-secrets", "Secrets at rest"),
        ("adm-advanced", "Advanced"),
    ] {
        assert!(html.contains(&format!("id=\"{id}\"")), "{title} disclosure");
    }
    assert!(html.contains("Save and send a test"), "mail test");
    assert!(
        html.contains("Session length (days)"),
        "sessions in Advanced"
    );
    // the test config sets the cookie flag in the file: read-only with its origin
    assert!(html.contains("Set in the configuration file"), "provenance");
    assert!(html.contains("Confirmed for"), "re-auth window state");
    // UX2-W9: the http warning once, next to the cookie switch
    assert_eq!(
        html.matches("The public address does not use https.")
            .count(),
        1,
        "https warning shown once"
    );
    // Sign-in and SSO: the URIs to register, the modes and the front-channel sentence
    let (_, html) = page(app, "/admin/sso", Some(admin)).await;
    for uri in [
        "/api/v1/auth/oidc/callback",
        "/signed-out",
        "/api/v1/auth/oidc/backchannel-logout",
    ] {
        assert!(html.contains(uri), "register {uri}");
    }
    assert!(html.contains("Add a provider"), "add a provider");
    assert!(html.contains("name=\"sign-in-mode\""), "mode radios");
    assert!(
        html.contains("Front-channel logout is not used"),
        "front-channel sentence"
    );
    // Branding and Security are folded in: 404, no redirects (DEC-171)
    for uri in ["/admin/brand", "/admin/security"] {
        let (status, _) = page(app, uri, Some(admin)).await;
        assert_eq!(status, 404, "{uri}");
    }
    // the account menu signs out with a form post, local first (DEC-326)
    let (_, html) = page(app, "/you", Some(admin)).await;
    assert!(
        html.contains("action=\"/api/v1/auth/logout\""),
        "sign-out form"
    );
}
