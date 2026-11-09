//! Catalog retention (DEC-453): data from a source whose terms limit how long it may be cached
//! (TMDB terms 1.C: not "for longer than 6 months") is refreshed before it gets that old, or
//! purged when no refresh is possible. An item nobody opens is otherwise never refreshed.
//! Strict reading (UX4-9, owner may reverse): a purge drops every field TMDB supplied, title
//! and year included, keeping only the external id and the person's own data; an item nothing
//! refers to is deleted instead of refreshed; a purged item is fetched again once TMDB answers.
//!
//! The job only queues refreshes (`catalog.refresh` on the `background_refresh` lane, deduped
//! per item), so every provider call still goes through the governed client and its rate
//! limit; at most [`SWEEP_BATCH`] items are looked at per run.

use app::ports::{Lane, NewJob, Removal};
use app::{App, AppError};
use chrono::Duration;
use domain::catalog::CatalogItem;
use domain::ids::ItemId;

use crate::model::Capability;
use crate::registry::Registry;

/// Job kind of the sweep.
pub const RETENTION_JOB: &str = "catalog.retention";
/// Connector ids (external id namespaces) whose terms limit how long their data may be cached.
/// TMDB: "Cache, for longer than 6 months, any information obtained through or from TMDB"
/// (api-terms-of-use 1.C); the other connector docs in `docs/connectors/` state no limit.
pub const CACHE_LIMITED: &[&str] = &["tmdb"];
/// Data older than this is purged even when a refresh could be queued: the refreshes queued
/// since the configured age did not succeed, and six months must never pass.
pub const HARD_LIMIT_DAYS: i64 = 175;
/// Highest configurable age (days) below [`HARD_LIMIT_DAYS`], leaving room for refreshes.
pub const MAX_AGE_LIMIT_DAYS: u32 = 170;
/// Items looked at per run (oldest first; the server runs the job hourly).
pub const SWEEP_BATCH: u32 = 500;
/// An expired item is fetched again at most this often while its source keeps failing.
pub const REFETCH_EVERY_HOURS: i64 = 24;

/// An item whose connector data was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Purged {
    pub id: ItemId,
    /// The cover asset it referenced (the caller deletes the stored image).
    pub cover_asset: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SweepReport {
    /// Items whose refresh was queued.
    pub refreshed: Vec<ItemId>,
    /// Items whose connector data was dropped (still referenced by someone's data).
    pub purged: Vec<Purged>,
    /// Items nothing referred to, deleted at the limit instead of refreshed (UX4-9).
    pub deleted: Vec<Purged>,
    /// Expired items whose refetch was queued now that their source can answer again.
    pub refetched: Vec<ItemId>,
}

/// Whether a usable connector can refresh every cache-limited id the item holds.
fn refreshable(registry: &Registry, item: &CatalogItem) -> bool {
    let usable = registry.for_kind(item.kind.as_str(), Capability::Details);
    item.external_ids
        .iter()
        .filter(|e| CACHE_LIMITED.contains(&e.namespace.as_str()))
        .all(|e| usable.iter().any(|c| c.info().id == e.namespace))
}

async fn queue_refresh(app: &App, id: ItemId) -> Result<(), AppError> {
    app.jobs
        .enqueue(NewJob {
            lane: Lane::BackgroundRefresh,
            kind: app::REFRESH_JOB.into(),
            payload: serde_json::json!({ "item_id": id }),
            dedupe_key: Some(format!("{}:{id}", app::REFRESH_JOB)),
            priority: 0,
            run_at: None,
        })
        .await
        .map(|_| ())
        .map_err(|e| AppError::Repo(app::ports::RepoError::Backend(e.to_string())))
}

/// One run. Items whose cache-limited data is older than `max_age_days`: deleted when nothing
/// refers to them (no library entry, list, session, progress event or review); else a refresh
/// is queued when a usable connector can answer and the data is younger than
/// [`HARD_LIMIT_DAYS`]; else every connector-supplied field is purged, title and year
/// included (`App::purge_provider_data`). Purged items are fetched again, at most once per
/// [`REFETCH_EVERY_HOURS`], once a usable connector can answer (UX4-9).
///
/// # Errors
/// Storage errors. A failing queue is reported as an error so the job retries.
pub async fn sweep(
    app: &App,
    registry: &Registry,
    max_age_days: u32,
) -> Result<SweepReport, AppError> {
    let now = app.clock.now();
    let max_age = Duration::days(i64::from(max_age_days.min(MAX_AGE_LIMIT_DAYS)));
    let hard = now - Duration::days(HARD_LIMIT_DAYS);
    let namespaces: Vec<String> = CACHE_LIMITED.iter().map(|n| (*n).into()).collect();
    let mut report = SweepReport::default();
    for (id, obtained) in app
        .stale_provider_data(&namespaces, now - max_age, SWEEP_BATCH)
        .await?
    {
        if let Removal::Deleted { cover_asset } = app.delete_unreferenced_item(id).await? {
            report.deleted.push(Purged { id, cover_asset });
            continue;
        }
        let item = app.catalog_item(id).await?;
        if obtained > hard && refreshable(registry, &item) {
            queue_refresh(app, id).await?;
            report.refreshed.push(id);
        } else {
            let cover_asset = app.purge_provider_data(id).await?;
            report.purged.push(Purged { id, cover_asset });
        }
    }
    for id in app
        .expired_provider_data(&namespaces, now, SWEEP_BATCH)
        .await?
    {
        let item = app.catalog_item(id).await?;
        if refreshable(registry, &item) {
            queue_refresh(app, id).await?;
            app.touch_expired(id, now + Duration::hours(REFETCH_EVERY_HOURS))
                .await?;
            report.refetched.push(id);
        }
    }
    Ok(report)
}
