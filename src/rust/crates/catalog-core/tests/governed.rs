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
        contact: catalog_core::ContactUse::None,
        shared_pace: false,
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
        user: None,
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
            user: None,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Network(_)), "{err:?}");
    let shown = err.to_string();
    assert!(!shown.contains("s3cr3t-value"), "{shown}");
    assert!(!shown.contains("/token"), "{shown}");
}

/// The allowlist of a connector can change at run time (OIDC providers added in the admin
/// area, phase 3 WP3-2): the handle replaces the hosts for every later request.
#[tokio::test]
async fn host_allowlist_changes_at_run_time() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    let mut p = policy(&server);
    p.hosts.clear();
    let http = client(&server, p);
    let hosts = http.host_list("fake").expect("a handle for the connector");
    assert!(http.host_list("other").is_none());
    let r = http
        .fetch(get(&server, "/.well-known/openid-configuration"))
        .await;
    assert!(matches!(r, Err(HttpError::Blocked(_))), "{r:?}");
    hosts.set(vec![server.address().to_string()]);
    let r = http
        .fetch(get(&server, "/.well-known/openid-configuration"))
        .await
        .unwrap();
    assert_eq!(r.status, 200);
    assert_eq!(hosts.get(), vec![server.address().to_string()]);
    hosts.set(Vec::new());
    let r = http
        .fetch(get(&server, "/.well-known/openid-configuration"))
        .await;
    assert!(matches!(r, Err(HttpError::Blocked(_))), "{r:?}");
}

fn contact_client(
    server: &MockServer,
    wants: catalog_core::ContactUse,
    contact: Option<&str>,
) -> GovernedHttp {
    let mut p = policy(server);
    p.cache_ttl_secs = 0;
    p.contact = wants;
    let shared = catalog_core::OperatorContact::default();
    shared.set(contact).unwrap();
    let mut h = GovernedHttp::new("tracker/1.2 (+https://support.example)", None);
    h.add_connector("fake", p);
    h.with_contact("tracker/1.2", shared)
}

/// v1-books 3.1.1: Open Library and Wikidata ask clients to name a contact.
#[tokio::test]
async fn a_connector_that_wants_the_contact_sends_it_in_the_user_agent() {
    use catalog_core::ContactUse;
    for wants in [ContactUse::Preferred, ContactUse::Required] {
        let server = MockServer::start().await;
        Mock::given(path("/x"))
            .and(header("user-agent", "tracker/1.2 (ops@example.org)"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;
        let http = contact_client(&server, wants, Some("ops@example.org"));
        assert_eq!(http.fetch(get(&server, "/x")).await.unwrap().status, 200);
    }
    // a preferred contact that is not set: the plain header, and the source is still asked
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .and(header(
            "user-agent",
            "tracker/1.2 (+https://support.example)",
        ))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let http = contact_client(&server, ContactUse::Preferred, None);
    assert_eq!(http.fetch(get(&server, "/x")).await.unwrap().status, 200);
}

#[tokio::test]
async fn other_connectors_never_send_the_contact() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .and(header(
            "user-agent",
            "tracker/1.2 (+https://support.example)",
        ))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let http = contact_client(
        &server,
        catalog_core::ContactUse::None,
        Some("ops@example.org"),
    );
    assert_eq!(http.fetch(get(&server, "/x")).await.unwrap().status, 200);
    let sent = &server.received_requests().await.unwrap()[0];
    let all = format!("{:?}", sent.headers);
    assert!(!all.contains("ops@example.org"), "{all}");
}

#[tokio::test]
async fn a_required_contact_that_is_missing_blocks_the_call_without_a_request() {
    let server = MockServer::start().await;
    Mock::given(path("/x"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let http = contact_client(&server, catalog_core::ContactUse::Required, None);
    let err = http.fetch(get(&server, "/x")).await.unwrap_err();
    assert!(matches!(err, HttpError::Blocked(_)), "{err:?}");
    assert_eq!(http.upstream_calls(), 0);
}

/// The turns a client took from the shared pace.
#[derive(Default)]
struct Turns(std::sync::Mutex<Vec<(String, Duration)>>);

#[async_trait::async_trait]
impl catalog_core::SharedPace for Turns {
    async fn turn(&self, source: &str, every: Duration) {
        self.0.lock().unwrap().push((source.to_string(), every));
    }
}

/// Review BOOK-8: a source that limits per address takes a turn from the pace all processes
/// share before every request it sends, at the interval of its rate limit; a source with a
/// key keeps to this client's own token bucket.
#[tokio::test]
async fn a_source_without_a_key_takes_its_turn_from_the_shared_pace() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(3)
        .mount(&server)
        .await;
    let mut shared = policy(&server);
    shared.shared_pace = true;
    shared.rate_limit.per_second = 2;
    let turns = Arc::new(Turns::default());
    let mut h = GovernedHttp::new("test-agent/1", None);
    h.add_connector("fake", shared);
    h.add_connector("keyed", policy(&server));
    let h = h.with_shared_pace(turns.clone());
    for p in ["/a", "/b"] {
        assert_eq!(h.fetch(get(&server, p)).await.unwrap().status, 200);
    }
    let mut keyed = get(&server, "/c");
    keyed.connector = "keyed".into();
    assert_eq!(h.fetch(keyed).await.unwrap().status, 200);
    let half = Duration::from_millis(500);
    assert_eq!(
        *turns.0.lock().unwrap(),
        [("fake".to_string(), half), ("fake".to_string(), half)]
    );
}

/// WD-4: a pause a connector asks for (an error object in an HTTP 200 answer) holds the next
/// request to that source: a short one is waited for, a long one fails fast and sends nothing.
#[tokio::test]
async fn a_pause_asked_by_a_connector_holds_the_next_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    let http = client(&server, policy(&server));
    http.pause("fake", Duration::from_millis(400));
    let t = Instant::now();
    http.fetch(get(&server, "/short")).await.unwrap();
    assert!(
        t.elapsed() >= Duration::from_millis(350),
        "{:?}",
        t.elapsed()
    );
    http.pause("fake", Duration::from_secs(60));
    let r = http.fetch(get(&server, "/long")).await;
    assert!(
        matches!(r, Err(HttpError::RateLimited { retry_after_ms }) if retry_after_ms > 50_000),
        "{r:?}"
    );
    assert_eq!(http.upstream_calls(), 1);
}

/// WD-6 (API:Etiquette): the client asks for a compressed answer.
#[tokio::test]
async fn requests_ask_for_a_compressed_answer() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/gz"))
        .and(wiremock::matchers::header_regex("accept-encoding", "gzip"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;
    let http = client(&server, policy(&server));
    let r = http.fetch(get(&server, "/gz")).await.unwrap();
    assert_eq!(r.status, 200, "no Accept-Encoding: gzip");
}
