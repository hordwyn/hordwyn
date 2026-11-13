//! Review SEC2-9: the outgoing clients ignore proxy settings from the environment. Through a
//! proxy the target name would be resolved by the proxy, out of reach of the private-address
//! check. Its own test binary: it changes the process environment before any client exists.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use app::ports::{HttpFetch, HttpRequest, Lane};
use catalog_core::{ConnectorPolicy, GovernedHttp, PlainHttp, PrivateRule, RateLimit};
use domain::net::PrivateAllow;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn get(connector: &str, url: String) -> HttpRequest {
    HttpRequest {
        connector: connector.into(),
        op: "test".into(),
        method: "GET".into(),
        url,
        headers: vec![],
        body: None,
        lane: Lane::Interactive,
        skip_auth_record: false,
        user: None,
    }
}

#[tokio::test]
async fn proxy_variables_in_the_environment_are_not_used() {
    // a "proxy" that counts the connections it gets and answers nothing
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let through_proxy = Arc::new(AtomicUsize::new(0));
    tokio::spawn({
        let n = through_proxy.clone();
        async move {
            while let Ok((socket, _)) = proxy.accept().await {
                n.fetch_add(1, Ordering::SeqCst);
                drop(socket);
            }
        }
    });
    for var in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
    ] {
        // SAFETY: the only test of this binary, before any client or other thread reads the
        // environment
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var(var, &proxy_url);
        }
    }
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("NO_PROXY");
        std::env::remove_var("no_proxy");
    }

    let server = MockServer::start().await;
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;
    let url = format!("{}/x", server.uri());

    let plain = PlainHttp::new("test", PrivateAllow::fixed(true));
    let r = plain.fetch(get("notify", url.clone())).await;
    assert_eq!(r.map(|r| r.status).ok(), Some(204), "plain client");

    let mut governed = GovernedHttp::new("test", None);
    governed.add_connector(
        "fake",
        ConnectorPolicy {
            hosts: vec![server.address().to_string()],
            rate_limit: RateLimit {
                per_second: 1000,
                burst: 1000,
                max_concurrency: 4,
            },
            cache_ttl_secs: 0,
            breaker_threshold: 3,
            breaker_cooldown: Duration::from_secs(60),
            max_retry_wait: Duration::from_secs(1),
            max_retries: 0,
            private: PrivateRule::Allowed,
        },
    );
    let r = governed.fetch(get("fake", url)).await;
    assert_eq!(r.map(|r| r.status).ok(), Some(204), "governed client");
    assert_eq!(
        through_proxy.load(Ordering::SeqCst),
        0,
        "nothing went to the proxy"
    );
}
