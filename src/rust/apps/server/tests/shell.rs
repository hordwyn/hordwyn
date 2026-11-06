//! UX phase 0 (design.md 3.2, 6.15): the server renders the app shell from the session, so
//! a signed-in page never shows "Create account", and the renamed routes (`/history`,
//! `/people`) render while the old ones fall through to the 404 page (no redirects).

use server::{Overrides, Server};
use tower::ServiceExt;

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn app() -> axum::Router {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
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
    let resp = app
        .clone()
        .oneshot(req.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// Signs up `username` and returns the `name=value` part of the session cookie.
async fn sign_up(app: &axum::Router, username: &str, display_name: &str) -> String {
    let body = format!(
        r#"{{"username":"{username}","display_name":"{display_name}","password":"correct horse battery"}}"#
    );
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/sign-up")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
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

#[tokio::test]
async fn the_shell_follows_the_session() {
    if !services() {
        return;
    }
    let app = app().await;

    let (status, html) = page(&app, "/", None).await;
    assert_eq!(status, 200);
    assert!(html.contains(">Sign in<"), "signed out: sign-in link");
    assert!(
        html.contains(">Create account<"),
        "signed out: sign-up link"
    );
    assert!(!html.contains("aria-haspopup=\"menu\""), "no account menu");
    assert!(html.contains("\"signed_in\":false"), "config for hydration");

    // the first account is the instance admin
    let cookie = sign_up(&app, "anna", "Anna Example").await;
    let (status, html) = page(&app, "/", Some(&cookie)).await;
    assert_eq!(status, 200);
    assert!(
        !html.contains("Create account"),
        "signed in: no sign-up anywhere"
    );
    assert!(!html.contains(">Sign in<"), "signed in: no sign-in link");
    assert!(html.contains("aria-haspopup=\"menu\""), "account menu");
    assert!(
        html.contains("Account menu for Anna Example"),
        "account menu names the person"
    );
    assert!(html.contains("\"signed_in\":true"));
    assert!(html.contains("\"display_name\":\"Anna Example\""));
    assert!(html.contains("\"is_admin\":true"));
    assert!(html.contains("\"unread\":"));
    // the five sections and the mobile tab bar with the You hub
    for href in ["/library", "/history", "/lists", "/people", "/you"] {
        assert!(html.contains(&format!("href=\"{href}\"")), "link to {href}");
    }
    assert!(html.contains("aria-label=\"Notifications"), "bell");
    assert!(html.contains("href=\"#main\""), "skip link");
}

#[tokio::test]
async fn renamed_routes_render_and_old_ones_are_gone() {
    if !services() {
        return;
    }
    let app = app().await;
    let cookie = sign_up(&app, "ben", "Ben").await;
    for uri in ["/history", "/people", "/you"] {
        let (status, html) = page(&app, uri, Some(&cookie)).await;
        assert_eq!(status, 200, "{uri}");
        assert!(!html.contains("There is nothing at this address"), "{uri}");
    }
    for uri in ["/sessions", "/companions"] {
        let (status, html) = page(&app, uri, Some(&cookie)).await;
        assert_eq!(status, 404, "{uri}");
        assert!(html.contains("There is nothing at this address"), "{uri}");
        assert!(
            html.contains("Account menu for Ben"),
            "404 inside the shell"
        );
    }
}

/// Per-user language (owner ruling DEC-171, DEC-201): a signed-in user who chose German gets
/// `<html lang="de">` and the German shell, whatever the browser asks for; going back to
/// automatic follows `Accept-Language` again.
#[tokio::test]
async fn the_page_follows_the_users_language() {
    if !services() {
        return;
    }
    let app = app().await;
    let cookie = sign_up(&app, "clara", "Clara").await;
    let set = |locale: &'static str| {
        let (app, cookie) = (app.clone(), cookie.clone());
        let body = format!(r#"{{"locale":{locale}}}"#);
        async move {
            let resp = app
                .oneshot(
                    axum::http::Request::builder()
                        .method("PUT")
                        .uri("/api/v1/auth/me/locale")
                        .header("cookie", cookie)
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), 200, "set locale {locale}");
        }
    };
    set("\"de\"").await;
    // `page` asks for English
    let (status, html) = page(&app, "/", Some(&cookie)).await;
    assert_eq!(status, 200);
    assert!(html.contains("<html lang=\"de\""), "lang follows the user");
    assert!(
        html.contains("Kontomen\u{fc} von Clara"),
        "German account menu"
    );
    assert!(html.contains(">Bibliothek<"), "German navigation");
    assert!(
        html.contains("\"locale_setting\":\"de\""),
        "picker state for hydration"
    );
    let (_, you) = page(&app, "/you", Some(&cookie)).await;
    assert!(
        you.contains("id=\"you-language-de\""),
        "language picker on the You hub"
    );
    assert!(
        you.contains("lang=\"en\""),
        "the picker names English in English"
    );
    set("null").await;
    let (_, html) = page(&app, "/", Some(&cookie)).await;
    assert!(html.contains("<html lang=\"en\""), "automatic again");
    assert!(html.contains(">Library<"));
}
