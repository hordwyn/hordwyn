//! sec0 (SEC-2): requests to server functions and pages are refused above the JSON body
//! limit before anything of them is read; the import upload keeps its own, larger limit.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use api::JSON_BODY_LIMIT;
use axum::body::Body;
use leptos::server_fn::ServerFn;
use server::{Overrides, Server};

use crate::support;

const FORM: &str = "application/x-www-form-urlencoded";
/// The size of one piece of a streamed test body.
const PIECE: usize = 64 << 10;

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn app() -> Option<axum::Router> {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return None;
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
    Some(srv.app_router(&options))
}

/// A form body of `len` bytes that arrives in pieces, and how many bytes of it were asked
/// for so far.
fn pieces(len: usize) -> (Body, Arc<AtomicUsize>) {
    let read = Arc::new(AtomicUsize::new(0));
    let counter = read.clone();
    let stream = futures::stream::unfold(0_usize, move |sent| {
        let counter = counter.clone();
        async move {
            if sent == len {
                return None;
            }
            let n = PIECE.min(len - sent);
            let mut piece = vec![b'a'; n];
            if sent == 0 {
                let head = b"username=";
                piece[..head.len().min(n)].copy_from_slice(&head[..head.len().min(n)]);
            }
            counter.fetch_add(n, Ordering::SeqCst);
            Some((Ok::<_, std::io::Error>(piece), sent + n))
        }
    });
    (Body::from_stream(stream), read)
}

/// Post `len` bytes to `uri`; with `declared` the request says its length up front.
async fn post(app: &axum::Router, uri: &str, len: usize, declared: bool) -> (u16, usize, String) {
    let (body, read) = pieces(len);
    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", FORM)
        .header("accept", "application/json");
    if declared {
        req = req.header("content-length", len);
    }
    let resp = support::send(app, req.body(body).unwrap()).await;
    let status = resp.status().as_u16();
    let text = String::from_utf8_lossy(&resp.into_body()).into_owned();
    (status, read.load(Ordering::SeqCst), text)
}

#[tokio::test]
async fn a_server_function_body_over_the_limit_is_refused_unread() {
    let Some(app) = app().await else { return };
    let path = web::api::SignIn::PATH;
    assert!(path.starts_with("/_fn/"), "{path}");

    let (status, read, text) = post(&app, path, JSON_BODY_LIMIT + 1, true).await;
    assert_eq!(status, 413, "declared length over the limit: {text}");
    assert_eq!(read, 0, "refused by its length, before the body is read");

    // exactly the limit passes: the function reads all of it and answers for itself
    let (status, read, text) = post(&app, path, JSON_BODY_LIMIT, true).await;
    assert_ne!(status, 413, "{text}");
    assert_eq!(read, JSON_BODY_LIMIT, "status {status}: {text}");
}

#[tokio::test]
async fn a_body_without_a_declared_length_is_read_up_to_the_limit_only() {
    let Some(app) = app().await else { return };
    let len = 3 * JSON_BODY_LIMIT;
    let (status, read, text) = post(&app, web::api::SignIn::PATH, len, false).await;
    assert_eq!(status, 413, "{text}");
    assert!(
        read <= JSON_BODY_LIMIT + PIECE,
        "reading stops at the limit: {read} of {len} bytes were read"
    );
    assert!(!text.contains("aaaa"), "nothing of the body comes back");

    // at the limit it passes
    let (status, read, text) = post(&app, web::api::SignIn::PATH, JSON_BODY_LIMIT, false).await;
    assert_ne!(status, 413, "{text}");
    assert_eq!(read, JSON_BODY_LIMIT);
}

#[tokio::test]
async fn pages_and_unknown_paths_have_the_limit_too() {
    let Some(app) = app().await else { return };
    for uri in ["/", "/sign-in", "/no/such/page", "/pkg/app.js"] {
        let (status, read, text) = post(&app, uri, JSON_BODY_LIMIT + 1, true).await;
        assert_eq!(status, 413, "{uri}: {text}");
        assert_eq!(read, 0, "{uri}");
        let (status, read, _) = post(&app, uri, 3 * JSON_BODY_LIMIT, false).await;
        assert_eq!(status, 413, "{uri} without a declared length");
        assert!(read <= JSON_BODY_LIMIT + PIECE, "{uri}: {read} bytes read");
    }
}

/// The guard for the other direction: an import file above the JSON limit is still taken
/// (REST, `IMPORT_BODY_LIMIT`).
#[tokio::test]
async fn an_import_file_above_the_json_limit_is_still_accepted() {
    let Some(app) = app().await else { return };
    let sign_up = serde_json::json!({"username": "anna", "display_name": "Anna",
        "password": "correct horse battery"});
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/sign-up")
        .header("content-type", "application/json")
        .body(Body::from(sign_up.to_string()))
        .unwrap();
    let resp = support::send(&app, req).await;
    assert_eq!(resp.status(), 201, "sign-up");
    let cookie = resp.headers()["set-cookie"].to_str().unwrap();
    let cookie = cookie.split(';').next().unwrap().to_string();

    let mut file = vec![b' '; JSON_BODY_LIMIT + (1 << 20)];
    file[0] = b'[';
    *file.last_mut().unwrap() = b']';
    assert!(file.len() < api::IMPORT_BODY_LIMIT);
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/imports?format=trakt-json&dry_run=true")
        .header("cookie", cookie)
        .body(Body::from(file))
        .unwrap();
    let resp = support::send(&app, req).await;
    assert_eq!(
        resp.status(),
        202,
        "{}",
        String::from_utf8_lossy(resp.body())
    );
}

/// Review of sec0 (SEC0-6): a body that breaks off is answered in the shape of every other
/// error of the API, not with a bare status.
#[tokio::test]
async fn a_body_that_breaks_off_is_answered_as_an_error_document() {
    let Some(app) = app().await else { return };
    let broken = futures::stream::iter([
        Ok(b"username=".to_vec()),
        Err(std::io::Error::other("connection reset")),
    ]);
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(web::api::SignIn::PATH)
        .header("content-type", FORM)
        .body(Body::from_stream(broken))
        .unwrap();
    let resp = support::send(&app, req).await;
    assert_eq!(resp.status(), 400);
    let text = String::from_utf8_lossy(resp.body()).into_owned();
    let json: serde_json::Value = serde_json::from_slice(resp.body()).expect(&text);
    assert_eq!(json["code"], "bad_request", "{text}");
    assert!(!text.contains("connection reset"), "{text}");
}
