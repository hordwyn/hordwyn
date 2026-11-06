//! Job handlers run by the worker role.

use std::sync::Arc;

use app::App;
use app::ports::{HttpFetch, Lane};
use async_trait::async_trait;
use catalog_core::Registry;
use domain::ids::ItemId;
use jobs::{Handler, Job, Worker, WorkerConfig};

use crate::Server;

/// Lane of catalog refreshes queued by page views (interactive) is kept; refreshes the
/// worker schedules itself run in this lane.
pub const REFRESH_LANE: Lane = Lane::BackgroundRefresh;

fn uuid_field(job: &Job, field: &str) -> Result<uuid::Uuid, String> {
    job.payload
        .get(field)
        .and_then(|v| v.as_str())
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .ok_or_else(|| format!("payload field `{field}` missing or not a UUID"))
}

/// `catalog.refresh`: details from every connector, merge, store; then register the cover
/// as an asset (pinned if the item is in a library).
struct Refresh {
    app: App,
    registry: Registry,
    http: Arc<dyn HttpFetch>,
    assets: Arc<assets::AssetService>,
}

#[async_trait]
impl Handler for Refresh {
    async fn run(&self, job: &Job) -> Result<(), String> {
        let id = ItemId(uuid_field(job, "item_id")?);
        catalog_core::refresh::refresh_item(
            &self.app,
            &self.registry,
            self.http.clone(),
            job.lane,
            id,
        )
        .await
        .map_err(|e| e.to_string())?;
        let item = self.app.catalog_item(id).await.map_err(|e| e.to_string())?;
        if item.cover_asset.is_none()
            && let Some(url) = &item.cover_url
        {
            let connector = item
                .provenance
                .get("cover_url")
                .cloned()
                .or_else(|| item.external_ids.first().map(|e| e.namespace.clone()))
                .unwrap_or_default();
            let in_library = self
                .app
                .item_in_any_library(id)
                .await
                .map_err(|e| e.to_string())?;
            let priority = if in_library {
                assets::FetchPriority::Library
            } else {
                assets::FetchPriority::Catalog
            };
            let asset = self
                .assets
                .register(assets::AssetKind::Cover, url, &connector, priority)
                .await
                .map_err(|e| e.to_string())?;
            if in_library {
                self.assets.pin(asset.id).await.map_err(|e| e.to_string())?;
            }
            self.app
                .set_cover_asset(id, asset.id.to_string())
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

struct FetchAsset(Arc<assets::AssetService>);

#[async_trait]
impl Handler for FetchAsset {
    async fn run(&self, job: &Job) -> Result<(), String> {
        let id = uuid_field(job, "asset_id")?;
        self.0
            .fetch(id)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

struct Import(Arc<importers::Importer>);

#[async_trait]
impl Handler for Import {
    async fn run(&self, job: &Job) -> Result<(), String> {
        let id = uuid_field(job, "import_id")?;
        self.0.run(id).await.map(|_| ()).map_err(|e| e.to_string())
    }
}

struct Evict(Arc<assets::StorageBudget>);

#[async_trait]
impl Handler for Evict {
    async fn run(&self, _job: &Job) -> Result<(), String> {
        self.0.evict().await.map(|_| ()).map_err(|e| e.to_string())
    }
}

struct Deliver(Arc<notify::Deliverer>);

#[async_trait]
impl Handler for Deliver {
    async fn run(&self, job: &Job) -> Result<(), String> {
        self.0.deliver(uuid_field(job, "delivery_id")?).await
    }
}

struct Digest(Arc<notify::Deliverer>);

#[async_trait]
impl Handler for Digest {
    async fn run(&self, job: &Job) -> Result<(), String> {
        let until = serde_json::from_value(job.payload["until"].clone())
            .map_err(|e| format!("job payload `until`: {e}"))?;
        self.0.digest(uuid_field(job, "channel_id")?, until).await
    }
}

pub fn worker(s: &Server) -> Worker {
    let cfg = WorkerConfig {
        concurrency: usize::try_from(s.config.jobs.concurrency).unwrap_or(4),
        shutdown_grace: std::time::Duration::from_secs(s.config.jobs.shutdown_grace_secs),
        ..WorkerConfig::default()
    };
    Worker::new(s.queue.clone(), cfg)
        .handle(
            app::REFRESH_JOB,
            Arc::new(Refresh {
                app: s.app.clone(),
                registry: s.registry.clone(),
                http: s.http.clone(),
                assets: s.assets.clone(),
            }),
        )
        .handle(assets::FETCH_JOB, Arc::new(FetchAsset(s.assets.clone())))
        .handle(assets::EVICT_JOB, Arc::new(Evict(s.budget.clone())))
        .handle(
            importers::IMPORT_JOB,
            Arc::new(Import(s.api.importer.clone().expect("importer is wired"))),
        )
        .handle(notify::DELIVER_JOB, Arc::new(Deliver(s.deliverer.clone())))
        .handle(notify::DIGEST_JOB, Arc::new(Digest(s.deliverer.clone())))
}
