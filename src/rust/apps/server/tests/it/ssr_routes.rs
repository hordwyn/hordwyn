//! The hang seen under full CI load (DEC-384): building the page router renders the app once
//! to list its routes, and while it does, `leptos_router` sets a process-wide flag that makes
//! every resource created anywhere in the process wait forever. A page rendered by another
//! router at that moment never finished. Routers built later must not do that again.

use crate::support;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

fn services() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return false;
    }
    true
}

async fn server() -> Server {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap()
}

fn options() -> leptos::prelude::LeptosOptions {
    leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pages_render_while_other_routers_are_built() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = srv.app_router(&options());
    // an account exists, so signed-out pages render instead of sending to the wizard
    let body = r#"{"username":"anna","display_name":"Anna","password":"correct horse battery"}"#;
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/sign-up")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    assert_eq!(support::send(&app, req).await.status(), 201, "sign-up");
    // routers are built again and again on another thread, as parallel tests do
    let stop = Arc::new(AtomicBool::new(false));
    let builder = {
        let (srv, stop) = (srv.clone(), stop.clone());
        tokio::task::spawn_blocking(move || {
            let mut built = 0u32;
            while !stop.load(Ordering::Relaxed) {
                drop(srv.app_router(&options()));
                built += 1;
            }
            built
        })
    };
    // the signed-out shell creates the app's resources on every page
    let mut hung = Vec::new();
    for i in 0..150 {
        let uri = if i % 2 == 0 { "/sign-in" } else { "/sign-up" };
        let req = axum::http::Request::builder()
            .uri(uri)
            .header("accept-language", "en")
            .body(axum::body::Body::empty())
            .unwrap();
        let page = tokio::time::timeout(Duration::from_secs(10), support::send(&app, req)).await;
        match page {
            Ok(resp) => assert_eq!(resp.status(), 200, "{uri}"),
            Err(_) => hung.push(format!("{i}: {uri}")),
        }
        if hung.len() >= 3 {
            break;
        }
    }
    stop.store(true, Ordering::Relaxed);
    let built = builder.await.unwrap();
    assert_eq!(
        hung,
        Vec::<String>::new(),
        "pages that never finished ({built} routers built meanwhile)"
    );
}
