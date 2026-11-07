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
        skip_auth_record: false,
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

/// design.md 4.4: the source status is computed from the governed client's counters.
#[tokio::test]
async fn counters_record_calls_rate_limits_auth_failures_and_the_circuit() {
    let server = MockServer::start().await;
    Mock::given(path("/ok"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    Mock::given(path("/limited"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .mount(&server)
        .await;
    Mock::given(path("/denied"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    Mock::given(path("/broken"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let mut p = policy(&server);
    p.max_retries = 0;
    p.cache_ttl_secs = 0;
    let stats = Arc::new(catalog_core::SourceStats::default());
    let mut h = GovernedHttp::new("test-agent/1", None);
    h.add_connector("fake", p);
    let http = h.with_stats(stats.clone());
    http.fetch(get(&server, "/ok")).await.unwrap();
    let _ = http.fetch(get(&server, "/limited")).await;
    let r = http.fetch(get(&server, "/denied")).await.unwrap();
    assert_eq!(r.status, 401);
    let c = stats.counters("fake");
    assert_eq!((c.calls_24h, c.rate_limited_24h), (3, 1));
    assert_eq!(c.last_auth_status, Some(401));
    assert!(c.last_ok_at.is_some());
    assert!(stats.rejected("fake"));
    stats.key_changed("fake");
    assert!(!stats.rejected("fake"), "a new key clears the rejection");
    for _ in 0..3 {
        let _ = http.fetch(get(&server, "/broken")).await;
    }
    let c = stats.counters("fake");
    assert!(c.circuit_open_until.is_some(), "{c:?}");
    assert_eq!(stats.counters("other").calls_24h, 0);
}

/// UX2-S2 / UX2-S5: a rejection of a member's own credential or of an unsaved candidate key
/// says nothing about the instance key: it is not recorded in the shared counters.
#[tokio::test]
async fn rejections_of_user_scoped_requests_are_not_recorded() {
    let server = MockServer::start().await;
    Mock::given(path("/denied"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let mut p = policy(&server);
    p.cache_ttl_secs = 0;
    let stats = Arc::new(catalog_core::SourceStats::default());
    let mut h = GovernedHttp::new("test-agent/1", None);
    h.add_connector("fake", p);
    let http = h.with_stats(stats.clone());
    let mut req = get(&server, "/denied");
    req.skip_auth_record = true;
    assert_eq!(http.fetch(req).await.unwrap().status, 401);
    assert!(!stats.rejected("fake"), "{:?}", stats.counters("fake"));
    assert_eq!(stats.counters("fake").calls_24h, 1, "the call still counts");
    assert_eq!(
        http.fetch(get(&server, "/denied")).await.unwrap().status,
        401
    );
    assert!(stats.rejected("fake"), "an instance request is recorded");
}

/// UX2-S1: a network error never carries the request URL (query strings may hold keys).
#[tokio::test]
async fn network_errors_do_not_carry_the_url() {
    // a port nothing listens on
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let mut p = ConnectorPolicy {
        hosts: vec![addr.to_string()],
        ..ConnectorPolicy::oidc("127.0.0.1", Some(addr.port()))
    };
    p.max_retries = 0;
    let mut h = GovernedHttp::new("test-agent/1", None);
    h.add_connector("fake", p);
    let err = h
        .fetch(HttpRequest {
            connector: "fake".into(),
            op: "test".into(),
            method: "GET".into(),
            url: format!("http://{addr}/token?client_secret=s3cr3t-value"),
            headers: vec![],
            body: None,
            lane: Lane::Interactive,
            skip_auth_record: false,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Network(_)), "{err:?}");
    let shown = err.to_string();
    assert!(!shown.contains("s3cr3t-value"), "{shown}");
    assert!(!shown.contains("/token"), "{shown}");
}
