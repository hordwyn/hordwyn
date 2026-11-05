//! Catalog items: one generic shape for every kind, kind-specific data in `attributes` (D-5.1).

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::ids::ItemId;
use crate::kind::KindId;

/// Link to a record in another system, e.g. `("tmdb", "603")`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExternalId {
    pub namespace: String,
    pub value: String,
}

impl ExternalId {
    pub fn new(namespace: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            value: value.into(),
        }
    }
}

/// How complete our copy of the provider data is (D-6 stale-while-revalidate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityState {
    Stub,
    Partial,
    Full,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogItem {
    pub id: ItemId,
    pub kind: KindId,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    /// blake3 hash of the cover asset, if one is stored.
    pub cover_asset: Option<String>,
    /// Provider URL of the cover, used until (or instead of) a stored asset (D-7 redirect).
    pub cover_url: Option<String>,
    pub external_ids: Vec<ExternalId>,
    /// Kind-specific data, validated against the kind's attributes schema on write.
    pub attributes: serde_json::Value,
    /// Which connector supplied each field (`title`, `attributes.genres`, ...), D-6.7.
    pub provenance: std::collections::BTreeMap<String, String>,
    pub state: EntityState,
    pub fetched_at: Option<Timestamp>,
    pub fresh_until: Option<Timestamp>,
}

impl CatalogItem {
    /// True if the item should be refreshed in the background at `now`.
    #[must_use]
    pub fn needs_refresh(&self, now: Timestamp) -> bool {
        match self.state {
            EntityState::Stub | EntityState::Partial => true,
            EntityState::Failed => false,
            EntityState::Full => self.fresh_until.is_none_or(|t| t <= now),
        }
    }
}
