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
    /// Provider cover URL (redirect fallback, registered as an asset by the refresh job).
    pub cover_url: Option<String>,
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
                item.cover_url = new.cover_url.or(item.cover_url);
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
                cover_url: new.cover_url,
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

/// Size limits for catalog data from users (G2-10).
pub const MAX_TITLE_CHARS: usize = 500;
pub const MAX_EXTERNAL_IDS: usize = 10;

fn check_untrusted(new: &NewCatalogItem) -> AppResult<()> {
    let bad =
        |m: &str| -> AppResult<()> { Err(DomainError::invalid("item", m.to_string()).into()) };
    if new.title.chars().count() > MAX_TITLE_CHARS {
        return bad("title is longer than 500 characters");
    }
    if new.external_ids.len() > MAX_EXTERNAL_IDS {
        return bad("more than 10 external ids");
    }
    let ns_ok = |ns: &str| {
        (1..=32).contains(&ns.len())
            && ns
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    if new
        .external_ids
        .iter()
        .any(|e| !ns_ok(&e.namespace) || e.value.is_empty() || e.value.chars().count() > 200)
    {
        return bad(
            "external ids need a namespace of a-z, 0-9, _ (up to 32) and a value up to 200 characters",
        );
    }
    Ok(())
}

impl App {
    /// Like [`App::upsert_catalog_item`], but an item that already exists (same external id)
    /// is returned unchanged except for new external ids and a missing cover URL. Search
    /// results use this so a sparse result never overwrites a refreshed item.
    ///
    /// # Errors
    /// Unknown kind, invalid attributes or a blank title (for new items).
    pub async fn ensure_catalog_item(&self, new: NewCatalogItem) -> AppResult<CatalogItem> {
        for ext in &new.external_ids {
            if let Some(mut item) = self.repos.catalog.by_external_id(&new.kind, ext).await? {
                let mut changed = false;
                for e in &new.external_ids {
                    if !item.external_ids.contains(e) {
                        item.external_ids.push(e.clone());
                        changed = true;
                    }
                }
                if item.cover_url.is_none() && new.cover_url.is_some() {
                    item.cover_url = new.cover_url;
                    changed = true;
                }
                if changed {
                    self.repos.catalog.upsert(&item).await?;
                }
                return Ok(item);
            }
        }
        self.upsert_catalog_item(new).await
    }

    /// For data from users (imports, webhooks), not from connectors: an existing item (same
    /// external id) is returned unchanged, so nobody can attach ids or covers to a shared
    /// item that a refresh would then follow (G2-2). New items are size-checked.
    ///
    /// # Errors
    /// Unknown kind, a blank or too long title, too many or too long ids.
    pub async fn find_or_create_catalog_item(&self, new: NewCatalogItem) -> AppResult<CatalogItem> {
        check_untrusted(&new)?;
        for ext in &new.external_ids {
            if let Some(item) = self.repos.catalog.by_external_id(&new.kind, ext).await? {
                return Ok(item);
            }
        }
        self.upsert_catalog_item(new).await
    }

    /// Attach the processed cover asset to an item.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn set_cover_asset(&self, id: ItemId, asset: String) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        item.cover_asset = Some(asset);
        self.repos.catalog.upsert(&item).await?;
        self.publish(AppEvent::entity_updated("catalog_item", item.id.0, vec![]))
            .await;
        Ok(item)
    }

    /// # Errors
    /// Storage errors.
    pub async fn item_in_any_library(&self, id: ItemId) -> AppResult<bool> {
        Ok(self.repos.library.any_for_item(id).await?)
    }
}
