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
    /// Id of the cover asset (assets crate), once the image has been registered.
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
            // a failed refresh sets `fresh_until` to its retry time; until then, do not retry
            // (otherwise every page view queues a refresh that fails again, DEC-114)
            EntityState::Stub | EntityState::Partial | EntityState::Full => {
                self.fresh_until.is_none_or(|t| t <= now)
            }
            EntityState::Failed => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn item(state: EntityState, fresh_until: Option<Timestamp>) -> CatalogItem {
        CatalogItem {
            id: crate::ids::ItemId(uuid::Uuid::nil()),
            kind: crate::kind::KindId::new("movie").unwrap(),
            title: "x".into(),
            original_title: None,
            year: None,
            description: None,
            cover_asset: None,
            cover_url: None,
            external_ids: vec![],
            attributes: serde_json::Value::Null,
            provenance: std::collections::BTreeMap::new(),
            state,
            fetched_at: None,
            fresh_until,
        }
    }

    #[test]
    fn refresh_waits_for_the_retry_time_after_a_failure() {
        let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let later = Some(now + Duration::hours(1));
        assert!(item(EntityState::Partial, None).needs_refresh(now));
        assert!(
            !item(EntityState::Partial, later).needs_refresh(now),
            "retry pending"
        );
        assert!(item(EntityState::Partial, later).needs_refresh(now + Duration::hours(2)));
        assert!(item(EntityState::Stub, None).needs_refresh(now));
        assert!(!item(EntityState::Full, later).needs_refresh(now));
        assert!(!item(EntityState::Failed, None).needs_refresh(now));
    }
}
