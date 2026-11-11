//! M07 brand swap: the same binary renders a second brand from another brand directory, with
//! no code changes (D-10).

use crate::support;

use server::{Overrides, Server};

async fn home(brand_dir: &str, accept_language: &str) -> (String, String) {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\n[brand]\ndir = \"{brand_dir}\"\n"
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build();
    let app = srv.app_router(&options);
    // pages render once the first account exists (before it, they lead to `/setup`)
    let cookie = first_account(&app).await;
    let resp = support::send(
        &app,
        axum::http::Request::builder()
            .uri("/")
            .header("accept-language", accept_language)
            .body(axum::body::Body::empty())
            .unwrap(),
    )
    .await;
    let html = String::from_utf8(resp.into_body().to_vec()).unwrap();
    (html, cookie)
}

/// Signs up the first account (`anna`) and returns its `set-cookie` header.
async fn first_account(app: &axum::Router) -> String {
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
    resp.headers()["set-cookie"].to_str().unwrap().to_string()
}

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

#[tokio::test]
async fn a_second_brand_renders_without_code_changes() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let (default_html, default_cookie) = home(&repo("config/brand/default"), "en").await;
    let (html, cookie) = home(&repo("src/web/tests/brands/sample"), "de-DE,de;q=0.9").await;

    let default_name = brand::Brand::load(std::path::Path::new(&repo("config/brand/default")))
        .unwrap()
        .name;
    assert!(default_html.contains(&format!("<title>{default_name}</title>")));
    assert!(!default_html.contains("Reading Room"));

    assert!(html.contains("<title>Reading Room</title>"), "product name");
    assert!(
        html.contains("<html lang=\"de\""),
        "locale from the brand/Accept-Language"
    );
    assert!(
        html.contains("--primary:oklch(0.55 0.2 145);"),
        "theme tokens"
    );
    assert!(html.contains(">Regal<"), "brand string override");
    assert!(!html.contains(&default_name));
    assert!(cookie.starts_with("readingroom_session="), "{cookie}");
    assert!(!default_cookie.starts_with("readingroom_session="));
}

async fn get(app: &axum::Router, uri: &str) -> axum::http::Response<axum::body::Bytes> {
    let req = axum::http::Request::builder()
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap();
    support::send(app, req).await
}

/// design.md 2.8 / DEC-171: the default brand's font is declared in the head and served from
/// our own origin with a font media type and an immutable cache header.
#[tokio::test]
async fn brand_font_is_self_hosted() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [brand]\ndir = \"{}\"\n",
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
    first_account(&app).await;
    let resp = get(&app, "/").await;
    let html = String::from_utf8(resp.into_body().to_vec()).unwrap();
    assert!(html.contains("@font-face{"), "font face in the head");
    assert!(html.contains("font-display:swap"));
    // the brand style comes after the stylesheet, so its tokens win
    let link = html.find("/pkg/app.css").expect("stylesheet link");
    assert!(html.find("id=\"brand-theme\"").expect("brand style") > link);
    let start = html.find("src:url(\"").expect("font url") + "src:url(\"".len();
    let url = &html[start..start + html[start..].find('"').unwrap()];
    assert!(url.starts_with("/brand/fonts/"), "same origin: {url}");

    let resp = get(&app, url).await;
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers()["content-type"], "font/woff2");
    let cache = resp.headers()["cache-control"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(
        cache.contains("immutable") && cache.contains("max-age=31536000"),
        "{cache}"
    );
    let body = resp.into_body();
    assert_eq!(&body[..4], b"wOF2");

    assert_eq!(
        get(&app, "/brand/fonts/0000/missing.woff2").await.status(),
        404
    );
    assert_eq!(
        get(&app, "/brand/fonts/0000/../brand.toml").await.status(),
        404
    );
}
