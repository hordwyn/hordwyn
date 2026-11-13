//! Background refresh of one catalog item (the job behind stale-while-revalidate, D-6.1).

use app::ports::Lane;
use app::{App, AppError, CatalogRefresh};
use domain::ids::ItemId;

use crate::connector::Ctx;
use crate::merge::{FieldPriority, merge};
use crate::model::{Capability, DetailsRequest, EpisodesRequest};
use crate::registry::Registry;

/// Seconds an item stays fresh after a successful refresh.
pub const FRESH_FOR_SECS: i64 = 7 * 24 * 3600;
/// Seconds before a failed refresh is retried.
pub const RETRY_FAILED_SECS: i64 = 3600;

/// Does a source of this instance answer for the item: a registered connector that gives
/// details for the item's kind and in whose namespace the item holds an external id. The
/// connector's key and switch do not matter (a source without its key still owns its
/// items; their refresh fails and is retried as before). Without such a source no refresh
/// is queued or recorded for the item (B2) and its details may be edited by hand
/// (v1-manual, "Edit details").
#[must_use]
pub fn source_known(registry: &Registry, item: &domain::catalog::CatalogItem) -> bool {
    registry.infos().iter().any(|info| {
        info.capabilities.contains(&Capability::Details)
            && info.kinds.iter().any(|k| k == item.kind.as_str())
            && item.external_ids.iter().any(|e| e.namespace == info.id)
    })
}

/// Fetch details from every connector that knows the item (by external id in its
/// namespace), merge, store, publish `entity.updated`. An item no source answers for
/// ([`source_known`]) is left untouched.
///
/// # Errors
/// Unknown item or storage errors. Connector failures are recorded on the item instead.
pub async fn refresh_item(
    app: &App,
    registry: &Registry,
    http: std::sync::Arc<dyn app::ports::HttpFetch>,
    lane: Lane,
    id: ItemId,
) -> Result<(), AppError> {
    let item = app.catalog_item(id).await?;
    if !source_known(registry, &item) {
        // nobody to ask (entered by hand, a title-only stub of an import, ids of sources this
        // instance does not have): the item stays as it is. It used to be marked failed
        // and retried every hour (v1-plan.md 7, B2)
        return Ok(());
    }
    let ctx = Ctx::new(http, lane);
    let mut snapshots = Vec::new();
    let connectors = registry.for_kind(item.kind.as_str(), Capability::Details);
    for c in &connectors {
        let info = c.info();
        let Some(ext) = item.external_ids.iter().find(|e| e.namespace == info.id) else {
            continue;
        };
        let Some(details) = c.as_details() else {
            continue;
        };
        let req = DetailsRequest {
            kind: item.kind.to_string(),
            id: ext.value.clone(),
            // the item is shared by every member: the instance language (WD-2, LB-35)
            language: registry.language().map(String::from),
        };
        match details.details(&ctx, &req).await {
            Ok(s) => snapshots.push(s),
            Err(e) => {
                tracing::warn!(connector = %info.id, item = %id, error = %e, "details failed");
            }
        }
    }
    let priority = FieldPriority {
        default: connectors.iter().map(|c| c.info().id.clone()).collect(),
        ..FieldPriority::default()
    };
    match merge(&snapshots, &priority) {
        Some(m) => {
            let item = app
                .apply_catalog_refresh(
                    id,
                    CatalogRefresh {
                        title: m.title,
                        original_title: m.original_title,
                        year: m.year,
                        description: m.description,
                        attributes: m.attributes,
                        external_ids: m.external_ids,
                        cover_url: m.cover_url,
                        provenance: m.provenance,
                        fresh_for_secs: FRESH_FOR_SECS,
                        popularity: m.popularity,
                    },
                )
                .await?;
            // per-episode dates follow on the background lane (WP4-7)
            if episode_source(registry, &item).is_some() {
                app.queue_episode_refresh(id).await?;
            }
        }
        None => {
            app.mark_catalog_refresh_failed(id, RETRY_FAILED_SECS)
                .await?;
        }
    }
    Ok(())
}

/// The connector that answers for an item's episodes: the one that supplied its season
/// list (provenance `attributes.seasons`), else the first in priority order, as long as it
/// knows the item (an external id in its namespace) and has [`Capability::Episodes`].
fn episode_source(
    registry: &Registry,
    item: &domain::catalog::CatalogItem,
) -> Option<(std::sync::Arc<dyn crate::Connector>, String)> {
    let usable: Vec<_> = registry
        .for_kind(item.kind.as_str(), Capability::Episodes)
        .into_iter()
        .filter_map(|c| {
            let ext = item
                .external_ids
                .iter()
                .find(|e| e.namespace == c.info().id)?
                .value
                .clone();
            Some((c, ext))
        })
        .collect();
    let seasons_from = item.provenance.get("attributes.seasons");
    let pick = usable
        .iter()
        .position(|(c, _)| Some(&c.info().id) == seasons_from)
        .unwrap_or(0);
    usable.into_iter().nth(pick)
}

/// `catalog.episodes`: ask the item's episode source once per season that needs it
/// ([`App::seasons_to_fetch`]: new, changed, or with episodes still to air) and store the
/// answers. A season that fails is logged and left as it was. Returns the number of seasons
/// whose stored episodes changed.
///
/// # Errors
/// Unknown item or storage errors.
pub async fn refresh_episodes(
    app: &App,
    registry: &Registry,
    http: std::sync::Arc<dyn app::ports::HttpFetch>,
    lane: Lane,
    id: ItemId,
) -> Result<usize, AppError> {
    let item = app.catalog_item(id).await?;
    let Some((connector, ext)) = episode_source(registry, &item) else {
        return Ok(0);
    };
    let Some(source) = connector.as_episodes() else {
        return Ok(0);
    };
    let ctx = Ctx::new(http, lane);
    let mut fetched = Vec::new();
    for key in app.seasons_to_fetch(id).await? {
        let req = EpisodesRequest {
            kind: item.kind.to_string(),
            id: ext.clone(),
            season: key.number,
        };
        match source.season_episodes(&ctx, &req).await {
            Ok(episodes) => fetched.push(app::SeasonFetch {
                key,
                connector: connector.info().id.clone(),
                episodes,
            }),
            Err(e) => {
                tracing::warn!(connector = %connector.info().id, item = %id, season = req.season, error = %e, "season episodes failed");
            }
        }
    }
    app.store_episodes(id, &fetched).await
}
