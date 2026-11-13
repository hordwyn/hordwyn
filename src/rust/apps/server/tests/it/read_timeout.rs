//! sec2 (DEC-653): a request that stalls is given up. The head of a request has a time
//! limit at the listener; a body may not pause longer than `server.body_idle_timeout_secs`
//! and may not take longer than `server.body_read_timeout_secs` in all (the import upload:
//! `server.import_read_timeout_secs`); and a body is only collected for server
//! functions, so a stalled body to a page pins nothing.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::body::Body;
use leptos::server_fn::ServerFn;
use server::{Overrides, Server};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::support;

const FORM: &str = "application/x-www-form-urlencoded";

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
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n\
         [server]\nbody_idle_timeout_secs = 1\nbody_read_timeout_secs = 2\nimport_read_timeout_secs = 5\n",
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

/// A body of `pieces`, one every `gap`, that never ends when `stall` is set; and how many
/// of its bytes were asked for.
fn slow(pieces: Vec<Vec<u8>>, gap: Duration, stall: bool) -> (Body, Arc<AtomicUsize>) {
    let read = Arc::new(AtomicUsize::new(0));
    let counter = read.clone();
    let stream = futures::stream::unfold(pieces.into_iter(), move |mut rest| {
        let counter = counter.clone();
        async move {
            tokio::time::sleep(gap).await;
            match rest.next() {
                Some(piece) => {
                    counter.fetch_add(piece.len(), Ordering::SeqCst);
                    Some((Ok::<_, std::io::Error>(piece), rest))
                }
                None if stall => std::future::pending().await,
                None => None,
            }
        }
    });
    (Body::from_stream(stream), read)
}

async fn post(app: &axum::Router, uri: &str, kind: &str, body: Body) -> (u16, String, Duration) {
    let started = Instant::now();
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", kind)
        .header("accept", "application/json")
        .body(body)
        .unwrap();
    let resp = support::send(app, req).await;
    let status = resp.status().as_u16();
    let text = String::from_utf8_lossy(&resp.into_body()).into_owned();
    (status, text, started.elapsed())
}

fn timed_out(status: u16, text: &str) {
    assert_eq!(status, 408, "{text}");
    let json: serde_json::Value = serde_json::from_str(text).expect(text);
    assert_eq!(json["code"], "request_timeout", "{text}");
}

#[tokio::test]
async fn a_body_that_stalls_is_given_up() {
    let Some(app) = app().await else { return };
    for (uri, kind, start) in [
        (web::api::SignIn::PATH, FORM, &b"username="[..]),
        (
            "/api/v1/auth/sign-in",
            "application/json",
            &b"{\"username\":"[..],
        ),
    ] {
        let (body, _) = slow(vec![start.to_vec()], Duration::ZERO, true);
        let (status, text, took) = post(&app, uri, kind, body).await;
        timed_out(status, &text);
        assert!(took < Duration::from_secs(5), "{uri}: after {took:?}");
    }
}

#[tokio::test]
async fn a_body_that_trickles_is_given_up_after_the_time_for_a_whole_body() {
    let Some(app) = app().await else { return };
    for (uri, kind) in [
        (web::api::SignIn::PATH, FORM),
        ("/api/v1/auth/sign-in", "application/json"),
    ] {
        // a byte every 300 ms never pauses for the idle limit of 1 s
        let pieces = vec![b" ".to_vec(); 60];
        let (body, read) = slow(pieces, Duration::from_millis(300), true);
        let (status, text, took) = post(&app, uri, kind, body).await;
        timed_out(status, &text);
        assert!(
            took >= Duration::from_millis(1900) && took < Duration::from_secs(6),
            "{uri}: after {took:?}"
        );
        assert!(read.load(Ordering::SeqCst) < 20, "{uri}: reading stopped");
    }
}

/// The import upload may take longer than the limit for other bodies (64 MiB over a slow
/// line), up to its own limit.
#[tokio::test]
async fn the_import_upload_only_has_the_idle_limit() {
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

    let upload = |pieces: Vec<Vec<u8>>, gap, stall| {
        let (body, _) = slow(pieces, gap, stall);
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/imports?format=trakt-json&dry_run=true")
            .header("cookie", &cookie)
            .body(body)
            .unwrap()
    };
    // 3.2 s in all, longer than the 2 s for other bodies
    let mut pieces = vec![b" ".to_vec(); 8];
    pieces[0] = b"[".to_vec();
    pieces[7] = b"]".to_vec();
    let resp = support::send(&app, upload(pieces, Duration::from_millis(400), false)).await;
    assert_eq!(
        resp.status(),
        202,
        "{}",
        String::from_utf8_lossy(resp.body())
    );
    // but it may not stall
    let resp = support::send(&app, upload(vec![b"[".to_vec()], Duration::ZERO, true)).await;
    timed_out(
        resp.status().as_u16(),
        &String::from_utf8_lossy(resp.body()),
    );
    // nor trickle for ever (review SEC2-7): a byte every 300 ms for 12 s ends at the 5 s of
    // `import_read_timeout_secs`
    let started = Instant::now();
    let resp = support::send(
        &app,
        upload(vec![b" ".to_vec(); 40], Duration::from_millis(300), false),
    )
    .await;
    timed_out(
        resp.status().as_u16(),
        &String::from_utf8_lossy(resp.body()),
    );
    let took = started.elapsed();
    assert!(
        took >= Duration::from_millis(4500) && took < Duration::from_secs(9),
        "after {took:?}"
    );
}

/// Review SEC2-7: an upload without a session is refused before anything of its body is
/// read, so a stranger cannot hold an upload open.
#[tokio::test]
async fn an_import_upload_without_a_session_reads_nothing() {
    let Some(app) = app().await else { return };
    let (body, read) = slow(vec![b"[".to_vec()], Duration::ZERO, true);
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/imports?format=trakt-json&dry_run=true")
        .body(body)
        .unwrap();
    let started = Instant::now();
    let resp = support::send(&app, req).await;
    assert_eq!(
        resp.status(),
        401,
        "{}",
        String::from_utf8_lossy(resp.body())
    );
    assert_eq!(
        read.load(Ordering::SeqCst),
        0,
        "nothing of the body was read"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
}

/// Only server functions have their body collected: a page, a file or a path without a
/// route never reads it, so a body that stalls there holds no memory and no task.
#[tokio::test]
async fn a_body_to_a_page_is_not_read() {
    let Some(app) = app().await else { return };
    for uri in ["/", "/sign-in", "/no/such/page", "/pkg/app.js"] {
        let pieces = vec![vec![b'a'; 64 << 10]; 4];
        let (body, read) = slow(pieces, Duration::ZERO, true);
        let (status, text, took) = post(&app, uri, FORM, body).await;
        assert_ne!(status, 408, "{uri}: nothing waited for the body: {text}");
        assert_eq!(read.load(Ordering::SeqCst), 0, "{uri}: status {status}");
        assert!(took < Duration::from_secs(1), "{uri}: after {took:?}");
    }
}

/// The listener closes a connection whose request head does not arrive in time.
#[tokio::test]
async fn a_request_head_that_stalls_closes_the_connection() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stop = tokio_util::sync::CancellationToken::new();
    let app =
        axum::Router::new().route(
            "/peer",
            axum::routing::get(
                |axum::extract::ConnectInfo(peer): axum::extract::ConnectInfo<
                    std::net::SocketAddr,
                >| async move { peer.ip().to_string() },
            ),
        );
    let served = tokio::spawn(server::serve::serve(
        listener,
        app,
        Duration::from_secs(1),
        stop.clone().cancelled_owned(),
    ));

    // a whole request is answered, and the handler sees the peer address
    let mut ok = tokio::net::TcpStream::connect(addr).await.unwrap();
    ok.write_all(b"GET /peer HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut answer = String::new();
    ok.read_to_string(&mut answer).await.unwrap();
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.ends_with("127.0.0.1"), "{answer}");

    // half a head or nothing at all: the connection is closed after the limit (SEC2-2: the
    // version sniffing of an HTTP/1-or-2 listener had no timer)
    for (what, sent) in [
        ("half a head", &b"GET /peer HTTP/1.1\r\nHost: te"[..]),
        ("nothing", &b""[..]),
    ] {
        let took = closed_after(addr, sent).await.expect(what);
        assert!(
            took >= Duration::from_millis(900) && took < Duration::from_secs(5),
            "{what}: closed after {took:?}"
        );
    }
    // the HTTP/2 preface, part or whole, is not served (HTTP/1 only; HTTP/2 had no head
    // limit) and does not hold the connection: closed at once or after the limit
    for (what, sent) in [
        ("part of the HTTP/2 preface", &b"PRI * HTTP/2.0\r\n"[..]),
        (
            "the HTTP/2 preface",
            &b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"[..],
        ),
    ] {
        let took = closed_after(addr, sent).await.expect(what);
        assert!(
            took < Duration::from_secs(5),
            "{what}: closed after {took:?}"
        );
    }

    stop.cancel();
    served.await.unwrap();
}

/// Connect to `addr`, send `sent`, and wait (at most 10 s, so a missing limit fails the test
/// instead of hanging it) until the server closes the connection; how long that took.
async fn closed_after(addr: std::net::SocketAddr, sent: &[u8]) -> Option<Duration> {
    let started = Instant::now();
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream.write_all(sent).await.unwrap();
    let mut rest = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut rest))
        .await
        .ok()
        .map(|_| started.elapsed())
}
