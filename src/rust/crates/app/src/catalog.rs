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
                external_ids: new.external_ids,
                attributes,
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
