//! #18: the running version on every signed-in page (footer) and an About page with the
//! version, the links of the brand, the licence and the third-party notices of the image.

use crate::support;

use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn app(notices: &str) -> axum::Router {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [server]\nnotices_path = \"{notices}\"\n\
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

async fn get(app: &axum::Router, uri: &str, cookie: Option<&str>) -> (u16, String, String) {
    let mut req = axum::http::Request::builder()
        .uri(uri)
        .header("accept-language", "en");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let resp = support::send(app, req.body(axum::body::Body::empty()).unwrap()).await;
    let status = resp.status().as_u16();
    let ty = resp
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string())
        .unwrap_or_default();
    (
        status,
        ty,
        String::from_utf8(resp.into_body().to_vec()).unwrap(),
    )
}

async fn sign_up(app: &axum::Router) -> String {
    let body = r#"{"username":"anna","display_name":"Anna","password":"correct horse battery"}"#;
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

#[tokio::test]
async fn the_version_is_on_the_you_page_and_the_about_page() {
    if !services() {
        return;
    }
    let app = app("/nonexistent/notices.txt").await;
    let cookie = sign_up(&app).await;
    let version = env!("CARGO_PKG_VERSION");
    // the You page (and the account menu, rendered when opened) name the running version and
    // lead to About
    let (status, _, you) = get(&app, "/you", Some(&cookie)).await;
    assert_eq!(status, 200);
    assert!(
        you.contains(r#"data-testid="app-version""#),
        "version on the You page"
    );
    assert!(you.contains(&format!("{} {version}", brand("name"))));
    assert!(
        you.contains(r#"href="/about""#),
        "the You page links the About page"
    );

    let (status, _, about) = get(&app, "/about", Some(&cookie)).await;
    assert_eq!(status, 200);
    assert!(about.contains("<h1"), "About renders");
    assert!(about.contains(version));
    // the links of the default brand
    assert!(about.contains(&brand("source_url")), "source");
    assert!(
        about.contains(&format!("releases/v{version}.adoc")),
        "release notes of this version"
    );
    // the licence travels with the build; the notices and the data sources are linked
    assert!(about.contains("MIT License"));
    assert!(about.contains(r#"href="/about/notices.txt""#));
    assert!(about.contains(r#"href="/settings/sources#attribution""#));
}

#[tokio::test]
async fn signed_out_pages_show_no_version() {
    if !services() {
        return;
    }
    let app = app("/nonexistent/notices.txt").await;
    let _ = sign_up(&app).await;
    let (_, _, page) = get(&app, "/sign-in", None).await;
    assert!(!page.contains(r#"data-testid="app-version""#));
    assert!(!page.contains(env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn the_notices_of_the_image_are_served_as_text() {
    if !services() {
        return;
    }
    let path = std::env::temp_dir().join(format!("notices-{}.txt", uuid::Uuid::new_v4()));
    std::fs::write(&path, "serde 1.0 - MIT OR Apache-2.0\n").unwrap();
    let served = app(path.to_str().unwrap()).await;
    let (status, ty, body) = get(&served, "/about/notices.txt", None).await;
    assert_eq!(status, 200);
    assert!(ty.starts_with("text/plain"), "{ty}");
    assert_eq!(body, "serde 1.0 - MIT OR Apache-2.0\n");

    // a build without the file (a development build) says where it would be
    let bare = app("/nonexistent/notices.txt").await;
    let (status, _, _) = get(&bare, "/about/notices.txt", None).await;
    assert_eq!(status, 404);
}

/// A value of the default brand (`brand.toml`): the product name is the brand's, never the
/// code's (white-label guard).
fn brand(key: &str) -> String {
    let file = std::fs::read_to_string(repo("config/brand/default/brand.toml")).unwrap();
    let line = file
        .lines()
        .find(|l| l.starts_with(&format!("{key} = ")))
        .unwrap_or_else(|| panic!("{key} in brand.toml"));
    line.split_once(" = ")
        .unwrap()
        .1
        .trim_matches('"')
        .to_string()
}
