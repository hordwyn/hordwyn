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

/// The source whose client fetches and stores the cover of `item`: the connector that gave
/// the address, else the one of the item's first external id. `None` means the server never
/// fetches it: an address a person typed (provenance `user`; every browser loads it by
/// itself, v1-plan.md 7, B5) and an address nobody answers for (no source: the fetch ran
/// under an empty connector name and failed).
fn cover_source(item: &domain::catalog::CatalogItem) -> Option<String> {
    let source = match item.provenance.get("cover_url") {
        Some(by) if by == app::USER_PROVENANCE => return None,
        Some(by) => Some(by.clone()),
        None => item.external_ids.first().map(|e| e.namespace.clone()),
    };
    source.filter(|s| !s.is_empty())
}

/// `catalog.refresh`: details from every connector, merge, store; then register the cover
/// as an asset (pinned if the item is in a library), unless it has no [`cover_source`].
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
            && let Some(connector) = cover_source(&item)
        {
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

/// `catalog.episodes`: per-episode release dates of one item, one call per season that needs
/// it, on the background lane it was queued on (WP4-7).
struct Episodes {
    app: App,
    registry: Registry,
    http: Arc<dyn HttpFetch>,
}

#[async_trait]
impl Handler for Episodes {
    async fn run(&self, job: &Job) -> Result<(), String> {
        let id = ItemId(uuid_field(job, "item_id")?);
        catalog_core::refresh::refresh_episodes(
            &self.app,
            &self.registry,
            self.http.clone(),
            job.lane,
            id,
        )
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
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

/// `admin.audit_retention`: delete audit rows older than 365 days (OD-13) and expired
/// password reset links (UX3-12).
struct AuditRetention(api::admin::AdminApi, auth::Auth);

#[async_trait]
impl Handler for AuditRetention {
    async fn run(&self, _job: &Job) -> Result<(), String> {
        let n = api::admin::purge_audit(self.0.store.as_ref(), chrono::Utc::now())
            .await
            .map_err(|e| e.to_string())?;
        if n > 0 {
            tracing::info!(count = n, "deleted audit rows past the retention period");
        }
        let n = self
            .1
            .purge_password_resets()
            .await
            .map_err(|e| e.to_string())?;
        if n > 0 {
            tracing::info!(count = n, "deleted expired password reset links");
        }
        Ok(())
    }
}

/// `secrets.rewrap`: move values on an old key version to the active one (nightly while
/// any is left; counts only, never values).
struct Rewrap(api::admin::AdminApi);

#[async_trait]
impl Handler for Rewrap {
    async fn run(&self, _job: &Job) -> Result<(), String> {
        let r = api::admin::rewrap_all(&self.0)
            .await
            .map_err(|e| e.to_string())?;
        if r.rewrapped > 0 || r.locked > 0 {
            tracing::info!(
                rewrapped = r.rewrapped,
                locked = r.locked,
                "re-wrapped stored secrets"
            );
            let entry = auth::admin::AuditEntry {
                id: uuid::Uuid::now_v7(),
                at: chrono::Utc::now(),
                actor_id: None,
                actor_kind: auth::admin::ActorKind::System,
                actor_name: None,
                action: "secrets.rewrapped".into(),
                target_type: None,
                target_id: None,
                detail: serde_json::json!({
                    "rewrapped": r.rewrapped, "locked": r.locked, "key_version": self.0.keys.active()
                }),
                client_addr: None,
                request_id: None,
            };
            self.0
                .store
                .audit(&entry)
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// `admin.probes`: the Overview health rows, every minute on the background lane (DEC-285).
struct Probes(api::ApiState);

#[async_trait]
impl Handler for Probes {
    async fn run(&self, _job: &Job) -> Result<(), String> {
        api::admin::run_probes(&self.0)
            .await
            .map(|_| ())
            .map_err(|e| e.message)
    }
}

/// `usage.funnel_daily`: one `metrics` event per flow of yesterday, then the 180-day
/// retention of the funnel totals (WP4-5).
struct UsageDaily(api::ApiState);

#[async_trait]
impl Handler for UsageDaily {
    async fn run(&self, _job: &Job) -> Result<(), String> {
        api::usage::daily(&self.0, chrono::Utc::now().date_naive())
            .await
            .map(|_| ())
            .map_err(|e| e.message)
    }
}

/// `catalog.retention`: catalog data of sources whose terms limit caching (TMDB 1.C) is
/// refreshed, or purged with its stored cover when no refresh is possible; stale stored
/// provider responses older than the same age go too (DEC-453).
struct Retention {
    app: App,
    registry: Registry,
    assets: Arc<assets::AssetService>,
    cache: catalog_core::cache::HttpCache,
    max_age_days: u32,
}

#[async_trait]
impl Handler for Retention {
    async fn run(&self, _job: &Job) -> Result<(), String> {
        let report = catalog_core::retention::sweep(&self.app, &self.registry, self.max_age_days)
            .await
            .map_err(|e| e.to_string())?;
        for id in &report.refreshed {
            // the stored image is as old as the data: fetch it again with it
            let item = self
                .app
                .catalog_item(*id)
                .await
                .map_err(|e| e.to_string())?;
            if let Some(asset) = item
                .cover_asset
                .as_deref()
                .and_then(|a| uuid::Uuid::parse_str(a).ok())
            {
                self.assets
                    .refetch(asset)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        for p in report.purged.iter().chain(&report.deleted) {
            if let Some(asset) = p
                .cover_asset
                .as_deref()
                .and_then(|a| uuid::Uuid::parse_str(a).ok())
            {
                self.assets.forget(asset).await.map_err(|e| e.to_string())?;
            }
        }
        let before = self.app.clock.now() - chrono::Duration::days(i64::from(self.max_age_days));
        let responses = self
            .cache
            .purge_stale_before(before)
            .await
            .map_err(|e| e.to_string())?;
        let changed = report.refreshed.len()
            + report.purged.len()
            + report.deleted.len()
            + report.refetched.len();
        if changed > 0 || responses > 0 {
            tracing::info!(
                refreshed = report.refreshed.len(),
                purged = report.purged.len(),
                deleted = report.deleted.len(),
                refetched = report.refetched.len(),
                responses,
                "catalog retention"
            );
        }
        Ok(())
    }
}

/// Cron entries of the admin jobs (registered when a worker starts).
pub async fn schedule_admin_jobs(s: &Server) {
    let now = chrono::Utc::now();
    for (name, cron, kind) in [
        (
            "admin.audit_retention",
            "17 3 * * *",
            crate::admin_wiring::AUDIT_RETENTION_JOB,
        ),
        (
            "secrets.rewrap",
            "37 3 * * *",
            crate::admin_wiring::REWRAP_JOB,
        ),
        ("admin.probes", "* * * * *", crate::admin_wiring::PROBES_JOB),
        (
            "usage.funnel_daily",
            "47 3 * * *",
            crate::admin_wiring::USAGE_DAILY_JOB,
        ),
        (
            "catalog.retention",
            "41 * * * *",
            catalog_core::retention::RETENTION_JOB,
        ),
    ] {
        if let Err(e) = s
            .queue
            .schedule(
                name,
                cron,
                Lane::BackgroundRefresh,
                kind,
                serde_json::json!({}),
                now,
            )
            .await
        {
            tracing::warn!(error = %e, name, "scheduling a cron job failed");
        }
    }
}

pub fn worker(s: &Server) -> Worker {
    let cfg = WorkerConfig {
        concurrency: usize::try_from(s.config.jobs.concurrency).unwrap_or(4),
        shutdown_grace: std::time::Duration::from_secs(s.config.jobs.shutdown_grace_secs),
        ..WorkerConfig::default()
    };
    let mut w = Worker::new(s.queue.clone(), cfg);
    if let Some(a) = &s.api.admin {
        w = w
            .handle(
                crate::admin_wiring::AUDIT_RETENTION_JOB,
                Arc::new(AuditRetention(a.clone(), s.api.auth.clone())),
            )
            .handle(crate::admin_wiring::REWRAP_JOB, Arc::new(Rewrap(a.clone())))
            .handle(
                crate::admin_wiring::PROBES_JOB,
                Arc::new(Probes(s.api.clone())),
            )
            .handle(
                crate::admin_wiring::USAGE_DAILY_JOB,
                Arc::new(UsageDaily(s.api.clone())),
            );
    }
    w.handle(
        app::REFRESH_JOB,
        Arc::new(Refresh {
            app: s.app.clone(),
            registry: s.registry.clone(),
            http: s.http.clone(),
            assets: s.assets.clone(),
        }),
    )
    .handle(
        app::EPISODES_JOB,
        Arc::new(Episodes {
            app: s.app.clone(),
            registry: s.registry.clone(),
            http: s.http.clone(),
        }),
    )
    .handle(
        catalog_core::retention::RETENTION_JOB,
        Arc::new(Retention {
            app: s.app.clone(),
            registry: s.registry.clone(),
            assets: s.assets.clone(),
            cache: catalog_core::cache::HttpCache {
                index: Arc::new(catalog_core::pg::PgCacheIndex::new(s.pool.clone())),
                blobs: s.budget.clone(),
            },
            max_age_days: s.config.jobs.catalog_max_age_days,
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

#[cfg(test)]
mod tests {
    use domain::catalog::{CatalogItem, EntityState, ExternalId};

    use super::cover_source;

    fn item(ids: &[&str], cover_by: Option<&str>) -> CatalogItem {
        CatalogItem {
            id: domain::ids::ItemId(uuid::Uuid::nil()),
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "x".into(),
            original_title: None,
            year: None,
            description: None,
            cover_asset: None,
            cover_url: Some("https://img.example/a.jpg".into()),
            external_ids: ids.iter().map(|ns| ExternalId::new(*ns, "1")).collect(),
            attributes: serde_json::json!({}),
            provenance: cover_by
                .map(|by| ("cover_url".to_string(), by.to_string()))
                .into_iter()
                .collect(),
            state: EntityState::Full,
            fetched_at: None,
            fresh_until: None,
            popularity: None,
        }
    }

    #[test]
    fn a_cover_a_person_typed_has_no_source_to_fetch_it() {
        // B5: also when the item holds ids of a source (a corrected cover)
        assert_eq!(cover_source(&item(&[], Some("user"))), None);
        assert_eq!(cover_source(&item(&["tmdb"], Some("user"))), None);
        // nobody answers for the address: no fetch under an empty name
        assert_eq!(cover_source(&item(&[], None)), None);
        assert_eq!(cover_source(&item(&[], Some(""))), None);
        // a source's cover is fetched by that source's client
        assert_eq!(
            cover_source(&item(&["tmdb"], Some("tvdb"))).as_deref(),
            Some("tvdb")
        );
        assert_eq!(
            cover_source(&item(&["igdb"], None)).as_deref(),
            Some("igdb")
        );
    }
}
