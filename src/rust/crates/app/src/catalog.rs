use domain::DomainError;
use domain::catalog::{CatalogItem, EntityState, ExternalId};
use domain::ids::ItemId;
use domain::kind::{KindDescriptor, KindId};

use crate::error::OptionExt;
use crate::ports::AppEvent;
use crate::{App, AppResult};

/// Catalog data as delivered by a connector or importer.
#[derive(Debug, Clone, PartialEq)]
pub struct NewCatalogItem {
    pub kind: KindId,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub external_ids: Vec<ExternalId>,
    pub attributes: serde_json::Value,
}

impl App {
    /// # Errors
    /// Unknown kind.
    pub fn kind(&self, id: &KindId) -> AppResult<KindDescriptor> {
        self.kinds
            .get(id)
            .ok_or_else(|| DomainError::invalid("kind", format!("unknown kind `{id}`")).into())
    }

    /// Insert a catalog item, or update the existing one that shares an external id.
    /// Attributes are validated against the kind's schema.
    ///
    /// # Errors
    /// Unknown kind, invalid attributes or a blank title.
    pub async fn upsert_catalog_item(&self, new: NewCatalogItem) -> AppResult<CatalogItem> {
        self.kind(&new.kind)?;
        if new.title.trim().is_empty() {
            return Err(DomainError::invalid("title", "title must not be empty").into());
        }
        let attributes = self
            .kinds
            .validate_attributes(&new.kind, &new.attributes)
            .map_err(|r| DomainError::invalid("attributes", r))?;
        let mut existing = None;
        for ext in &new.external_ids {
            if let Some(found) = self.repos.catalog.by_external_id(&new.kind, ext).await? {
                existing = Some(found);
                break;
            }
        }
        let item = match existing {
            Some(mut item) => {
                item.title = new.title;
                item.original_title = new.original_title.or(item.original_title);
                item.year = new.year.or(item.year);
                item.description = new.description.or(item.description);
                for ext in new.external_ids {
                    if !item.external_ids.contains(&ext) {
                        item.external_ids.push(ext);
                    }
                }
                item.attributes = attributes;
                item
            }
            None => CatalogItem {
                id: ItemId(self.uuid()),
                kind: new.kind,
                title: new.title,
                original_title: new.original_title,
                year: new.year,
                description: new.description,
                cover_asset: None,
                cover_url: None,
                external_ids: new.external_ids,
                attributes,
                provenance: std::collections::BTreeMap::new(),
                state: EntityState::Partial,
                fetched_at: None,
                fresh_until: None,
            },
        };
        self.repos.catalog.upsert(&item).await?;
        self.publish(AppEvent::entity_updated("catalog_item", item.id.0, vec![]))
            .await;
        Ok(item)
    }

    /// # Errors
    /// Unknown item.
    pub async fn catalog_item(&self, id: ItemId) -> AppResult<CatalogItem> {
        self.repos.catalog.get(id).await?.found("catalog item")
    }
}

/// Job kind that refreshes one catalog item from its connectors.
pub const REFRESH_JOB: &str = "catalog.refresh";

impl App {
    /// Item detail for a page: returns what is stored right now and, if the item is a stub,
    /// partial or stale, enqueues a background refresh (deduplicated per item). Never waits
    /// on a provider (D-6.1); the refresh publishes `entity.updated` when it is done.
    ///
    /// # Errors
    /// Unknown item. A failing queue is logged by the adapter and does not fail the read.
    pub async fn item_detail(&self, id: ItemId) -> AppResult<CatalogItem> {
        let item = self.catalog_item(id).await?;
        if item.needs_refresh(self.now()) {
            // A full queue must not break reads; the next view enqueues again.
            let _ = self
                .jobs
                .enqueue(crate::ports::NewJob {
                    lane: crate::ports::Lane::Interactive,
                    kind: REFRESH_JOB.into(),
                    payload: serde_json::json!({ "item_id": id }),
                    dedupe_key: Some(format!("{REFRESH_JOB}:{id}")),
                    priority: 0,
                    run_at: None,
                })
                .await;
        }
        Ok(item)
    }
}

/// Result of refreshing an item from its connectors (already merged).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogRefresh {
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub attributes: serde_json::Value,
    pub external_ids: Vec<ExternalId>,
    pub cover_url: Option<String>,
    pub provenance: std::collections::BTreeMap<String, String>,
    /// Seconds until the item is considered stale again.
    pub fresh_for_secs: i64,
}

impl App {
    /// Store refreshed data, mark the item `full` and publish `entity.updated`. Attributes
    /// that fail validation are dropped (old ones kept) rather than failing the refresh.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn apply_catalog_refresh(
        &self,
        id: ItemId,
        refresh: CatalogRefresh,
    ) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        let now = self.now();
        match self
            .kinds
            .validate_attributes(&item.kind, &refresh.attributes)
        {
            Ok(attrs) => item.attributes = attrs,
            Err(reason) => {
                // Provider data that does not fit the schema must not break the page.
                item.provenance.insert("attributes.invalid".into(), reason);
            }
        }
        if !refresh.title.trim().is_empty() {
            item.title = refresh.title;
        }
        item.original_title = refresh.original_title.or(item.original_title);
        item.year = refresh.year.or(item.year);
        item.description = refresh.description.or(item.description);
        item.cover_url = refresh.cover_url.or(item.cover_url);
        for ext in refresh.external_ids {
            if !item.external_ids.contains(&ext) {
                item.external_ids.push(ext);
            }
        }
        item.provenance.extend(refresh.provenance);
        item.state = EntityState::Full;
        item.fetched_at = Some(now);
        item.fresh_until = Some(now + chrono::Duration::seconds(refresh.fresh_for_secs));
        self.repos.catalog.upsert(&item).await?;
        self.publish(AppEvent::entity_updated("catalog_item", item.id.0, vec![]))
            .await;
        Ok(item)
    }

    /// Record a failed refresh: stubs become `failed`; anything else stays as it is and is
    /// retried after `retry_in_secs`.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn mark_catalog_refresh_failed(
        &self,
        id: ItemId,
        retry_in_secs: i64,
    ) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        if item.state == EntityState::Stub {
            item.state = EntityState::Failed;
        }
        item.fresh_until = Some(self.now() + chrono::Duration::seconds(retry_in_secs));
        self.repos.catalog.upsert(&item).await?;
        self.publish(AppEvent::entity_updated("catalog_item", item.id.0, vec![]))
            .await;
        Ok(item)
    }
}
