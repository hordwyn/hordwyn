//! The governed HTTP client against a real local HTTP server.

use std::sync::Arc;
use std::time::{Duration, Instant};

use app::ports::{HttpError, HttpFetch, HttpRequest, Lane};
use catalog_core::cache::{HttpCache, MemCacheIndex};
use catalog_core::{ConnectorPolicy, GovernedHttp, RateLimit};
use testkit::MemBlobs;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn policy(server: &MockServer) -> ConnectorPolicy {
    ConnectorPolicy {
        hosts: vec![server.address().to_string()],
        rate_limit: RateLimit {
            per_second: 1000,
            burst: 1000,
            max_concurrency: 16,
        },
        cache_ttl_secs: 3600,
        breaker_threshold: 3,
        breaker_cooldown: Duration::from_secs(60),
        max_retry_wait: Duration::from_secs(5),
        max_retries: 2,
    }
}

fn client(server: &MockServer, p: ConnectorPolicy) -> Arc<GovernedHttp> {
    let cache = HttpCache {
        index: Arc::new(MemCacheIndex::default()),
        blobs: Arc::new(MemBlobs::default()),
    };
    let mut h = GovernedHttp::new("test-agent/1", Some(cache));
    h.add_connector("fake", p);
    let _ = server;
    Arc::new(h)
}

fn get(server: &MockServer, p: &str) -> HttpRequest {
    HttpRequest {
        connector: "fake".into(),
        op: "test".into(),
        method: "GET".into(),
        url: format!("{}{p}", server.uri()),
        headers: vec![],
        body: None,
        lane: Lane::Interactive,
    }
}

#[tokio::test]
async fn hundred_concurrent_identical_requests_make_one_upstream_call() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/item/1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("{}")
                .set_delay(Duration::from_millis(200)),
        )
        .expect(1)
        .mount(&server)
        .await;
    let http = client(&server, policy(&server));
    let mut tasks = Vec::new();
    for _ in 0..100 {
        let (http, req) = (http.clone(), get(&server, "/item/1"));
        tasks.push(tokio::spawn(async move { http.fetch(req).await }));
    }
    for t in tasks {
        assert_eq!(t.await.unwrap().unwrap().status, 200);
    }
    assert_eq!(http.upstream_calls(), 1);
    // and the next one is a cache hit
    http.fetch(get(&server, "/item/1")).await.unwrap();
    assert_eq!(http.upstream_calls(), 1);
}

#[tokio::test]
async fn retry_after_on_429_is_honoured() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/limited"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/limited"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;
    let http = client(&server, policy(&server));
    let started = Instant::now();
    let resp = http.fetch(get(&server, "/limited")).await.unwrap();
    assert_eq!(resp.status, 200);
    assert!(
        started.elapsed() >= Duration::from_secs(1),
        "waited {:?}",
        started.elapsed()
    );
    let times: Vec<_> = server.received_requests().await.unwrap();
    assert_eq!(times.len(), 2);
}

#[tokio::test]
async fn long_retry_after_fails_fast_and_pauses_the_connector() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "120"))
        .mount(&server)
        .await;
    let http = client(&server, policy(&server));
    let started = Instant::now();
    let err = http.fetch(get(&server, "/a")).await.unwrap_err();
    assert!(matches!(err, HttpError::RateLimited { retry_after_ms } if retry_after_ms >= 100_000));
    let err = http.fetch(get(&server, "/b")).await.unwrap_err();
    assert!(matches!(err, HttpError::RateLimited { .. }));
    assert!(started.elapsed() < Duration::from_secs(2));
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "paused: no second request"
    );
}

#[tokio::test]
async fn open_circuit_fails_fast() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let mut p = policy(&server);
    p.max_retries = 0;
    let http = client(&server, p);
    for i in 0..3 {
        let r = http.fetch(get(&server, &format!("/x{i}"))).await.unwrap();
        assert_eq!(r.status, 500);
    }
    assert!(http.circuit_open("fake"));
    let started = Instant::now();
    let err = http.fetch(get(&server, "/x9")).await.unwrap_err();
    assert_eq!(err, HttpError::CircuitOpen);
    assert!(started.elapsed() < Duration::from_millis(50));
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn non_allowlisted_host_is_blocked_before_any_request() {
    let allowed = MockServer::start().await;
    let other = MockServer::start().await;
    let http = client(&allowed, policy(&allowed));
    let err = http.fetch(get(&other, "/steal")).await.unwrap_err();
    assert!(matches!(err, HttpError::Blocked(_)), "{err:?}");
    let mut unknown = get(&allowed, "/x");
    unknown.connector = "nobody".into();
    assert!(matches!(
        http.fetch(unknown).await,
        Err(HttpError::Blocked(_))
    ));
    assert_eq!(other.received_requests().await.unwrap().len(), 0);
    assert_eq!(allowed.received_requests().await.unwrap().len(), 0);
    assert_eq!(http.upstream_calls(), 0);
}

#[tokio::test]
async fn stale_entry_is_revalidated_with_etag() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/doc"))
        .and(header("If-None-Match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304).insert_header("Cache-Control", "max-age=60"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/doc"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("ETag", "\"v1\"")
                .insert_header("Cache-Control", "max-age=1")
                .set_body_string("payload"),
        )
        .mount(&server)
        .await;
    let http = client(&server, policy(&server));
    assert_eq!(
        http.fetch(get(&server, "/doc")).await.unwrap().body,
        b"payload"
    );
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let again = http.fetch(get(&server, "/doc")).await.unwrap();
    assert_eq!(
        (again.status, again.body.as_slice()),
        (200, b"payload".as_slice())
    );
    assert_eq!(http.upstream_calls(), 2);
    http.fetch(get(&server, "/doc")).await.unwrap();
    assert_eq!(http.upstream_calls(), 2, "fresh again after 304");
}

#[tokio::test]
async fn transient_5xx_is_retried() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(502))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let http = client(&server, policy(&server));
    assert_eq!(
        http.fetch(get(&server, "/flaky")).await.unwrap().status,
        200
    );
    assert!(!http.circuit_open("fake"));
}
