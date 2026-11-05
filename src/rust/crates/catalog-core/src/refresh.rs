//! Background refresh of one catalog item (the job behind stale-while-revalidate, D-6.1).

use app::ports::Lane;
use app::{App, AppError, CatalogRefresh};
use domain::ids::ItemId;

use crate::connector::Ctx;
use crate::merge::{FieldPriority, merge};
use crate::model::{Capability, DetailsRequest};
use crate::registry::Registry;

/// Seconds an item stays fresh after a successful refresh.
pub const FRESH_FOR_SECS: i64 = 7 * 24 * 3600;
/// Seconds before a failed refresh is retried.
pub const RETRY_FAILED_SECS: i64 = 3600;

/// Fetch details from every connector that knows the item (by external id in its
/// namespace), merge, store, publish `entity.updated`.
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
    let ctx = Ctx { http, lane };
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
            language: None,
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
            app.apply_catalog_refresh(
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
                },
            )
            .await?;
        }
        None => {
            app.mark_catalog_refresh_failed(id, RETRY_FAILED_SECS)
                .await?;
        }
    }
    Ok(())
}
