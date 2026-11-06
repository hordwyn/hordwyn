//! M07 brand swap: the same binary renders a second brand from another brand directory, with
//! no code changes (D-10).

use server::{Overrides, Server};
use tower::ServiceExt;

async fn home(brand_dir: &str, accept_language: &str) -> (String, String) {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
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
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/")
                .header("accept-language", accept_language)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let html = String::from_utf8(
        axum::body::to_bytes(resp.into_body(), 1 << 22)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/sign-up")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(
                    r#"{"username":"anna","display_name":"Anna","password":"correct horse battery"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = resp.headers()["set-cookie"].to_str().unwrap().to_string();
    (html, cookie)
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
