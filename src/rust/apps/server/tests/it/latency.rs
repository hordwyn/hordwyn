//! M05 acceptance: with a connector that takes 10 s, item detail and search answer in under
//! 200 ms, and `entity.updated` arrives over SSE once the background refresh is done.

// One end-to-end scenario on purpose: the steps share a running server and an SSE stream.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use app::NewCatalogItem;
use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, Ctx, Details,
    DetailsRequest, RateLimit, Registry, Search, SearchRequest, Snapshot,
};
use domain::catalog::ExternalId;
use domain::kind::KindId;
use futures::StreamExt;
use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

const SLOW: Duration = Duration::from_secs(10);

struct SlowConnector(ConnectorInfo);

impl SlowConnector {
    fn new() -> Self {
        Self(ConnectorInfo {
            id: "slow".into(),
            name: "Slow".into(),
            kinds: vec!["movie".into()],
            capabilities: vec![Capability::Search, Capability::Details],
            rate_limit: RateLimit {
                per_second: 10,
                burst: 10,
                max_concurrency: 4,
            },
            auth: Auth::None,
            attribution: Attribution {
                text: "Slow test data".into(),
                url: "https://example.invalid".into(),
                licence: "test".into(),
            },
            hosts: vec![],
            cache_ttl_secs: 0,
            contact: catalog_core::ContactUse::None,
        })
    }

    fn snapshot(title: &str) -> Snapshot {
        Snapshot {
            connector: "slow".into(),
            kind: "movie".into(),
            external_ids: vec![ExternalId::new("slow", "1")],
            title: title.into(),
            original_title: None,
            year: Some(2001),
            description: Some("refreshed".into()),
            attributes: serde_json::json!({}),
            images: vec![],
            complete: true,
            popularity: None,
        }
    }
}

#[async_trait]
impl Search for SlowConnector {
    async fn search(&self, _: &Ctx, _: &SearchRequest) -> Result<Vec<Snapshot>, ConnectorError> {
        tokio::time::sleep(SLOW).await;
        Ok(vec![Self::snapshot("Slow Movie")])
    }
}

#[async_trait]
impl Details for SlowConnector {
    async fn details(&self, _: &Ctx, _: &DetailsRequest) -> Result<Snapshot, ConnectorError> {
        tokio::time::sleep(SLOW).await;
        Ok(Self::snapshot("Slow Movie (refreshed)"))
    }
}

impl Connector for SlowConnector {
    fn info(&self) -> &ConnectorInfo {
        &self.0
    }
    fn as_search(&self) -> Option<&dyn Search> {
        Some(self)
    }
    fn as_details(&self) -> Option<&dyn Details> {
        Some(self)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pages_never_wait_on_providers() {
    let Some(db_url) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let blobs = std::env::temp_dir().join(format!("blobs-{}", uuid::Uuid::now_v7().simple()));
    let toml = format!(
        "[database]\nurl = \"{db_url}\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n\
         [secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\nfs_root = \"{}\"\n\
         [brand]\ndir = \"{}/../../../../config/brand/default\"\n",
        blobs.display(),
        env!("CARGO_MANIFEST_DIR"),
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let mut registry = Registry::default();
    registry.register(Arc::new(SlowConnector::new()));
    let srv = Server::build(
        cfg,
        config::ServerRole::All,
        Overrides {
            registry: Some(registry),
            http: Some(Arc::new(testkit::fakes::ReplayHttp::default())),
            search: None,
        },
    )
    .await
    .unwrap();
    let item = srv
        .app
        .upsert_catalog_item(NewCatalogItem {
            kind: KindId::new("movie").unwrap(),
            title: "Slow Movie".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![ExternalId::new("slow", "1")],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();

    let stop = CancellationToken::new();
    let worker = {
        let srv = srv.clone();
        let stop = stop.clone();
        tokio::spawn(async move { srv.run_worker(stop).await })
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let router = srv.base_router();
    let web = {
        let stop = stop.clone();
        let bus = srv.bus.clone();
        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    stop.cancelled().await;
                    bus.shutdown();
                })
                .await
                .unwrap();
        })
    };

    let http = reqwest::Client::new();
    let signup: serde_json::Value = http
        .post(format!("{base}/api/v1/auth/sign-up"))
        .json(&serde_json::json!({
            "username": "anna", "display_name": "Anna", "password": "correct horse battery"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = signup["token"].as_str().unwrap().to_string();

    let mut sse = http
        .get(format!("{base}/api/v1/events"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .bytes_stream();

    let t = Instant::now();
    let r = http
        .get(format!("{base}/api/v1/items/{}", item.id.0))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let detail_ms = t.elapsed().as_millis();
    assert_eq!(r.status(), 200);
    let body: serde_json::Value = r.json().await.unwrap();
    assert_eq!(body["title"], "Slow Movie");
    assert_ne!(body["state"], "full");

    let t = Instant::now();
    let r = http
        .get(format!("{base}/api/v1/search?q=slow&kind=movie"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let search_ms = t.elapsed().as_millis();
    assert_eq!(r.status(), 200);
    let search: serde_json::Value = r.json().await.unwrap();
    assert_eq!(search["pending"], serde_json::json!(["slow"]));
    println!("item detail {detail_ms} ms, search {search_ms} ms");
    assert!(detail_ms < 200, "item detail took {detail_ms} ms");
    assert!(search_ms < 200, "search took {search_ms} ms");

    // SSE: wait for the refresh (about 10 s) and the streamed search results.
    let started = Instant::now();
    let mut buf = String::new();
    let mut updated = None;
    let mut results = None;
    let item_id = item.id.0.to_string();
    while (updated.is_none() || results.is_none()) && started.elapsed() < Duration::from_secs(40) {
        let Ok(Some(Ok(chunk))) = tokio::time::timeout(Duration::from_secs(40), sse.next()).await
        else {
            break;
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = buf.find("\n\n") {
            let frame = buf[..end].to_string();
            buf.drain(..end + 2);
            if frame.contains("event: entity.updated") && frame.contains(&item_id) {
                updated.get_or_insert(started.elapsed());
            }
            if frame.contains("event: search.results") {
                results.get_or_insert(started.elapsed());
            }
        }
    }
    let updated = updated.expect("entity.updated never arrived");
    let results = results.expect("search.results never arrived");
    println!("entity.updated after {updated:?}, search.results after {results:?}");
    assert!(
        updated >= SLOW.saturating_sub(Duration::from_secs(1)),
        "arrived before the slow refresh finished"
    );
    let r: serde_json::Value = http
        .get(format!("{base}/api/v1/items/{item_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r["title"], "Slow Movie (refreshed)");
    assert_eq!(r["state"], "full");

    stop.cancel();
    let _ = web.await;
    let _ = worker.await;
}
