//! `SearchIndex` adapters (D-13): Meilisearch (primary) and Postgres FTS + `pg_trgm`
//! (fallback), plus the outbox indexer that keeps them in sync with Postgres.
//!
//! Multi-tenant safety lives here, not in callers: queries on per-user indexes (`library`,
//! `people`) are always filtered by the viewer's id; filter keys must be on an allowlist and
//! values are escaped, so a query can never widen its own scope.
#![deny(unsafe_code)]

pub mod indexer;
pub mod meili;
pub mod pg;

use app::ports::{PortError, SearchQuery};

/// Index names (without the deployment prefix).
pub const CATALOG: &str = "catalog";
pub const LIBRARY: &str = "library";
pub const PEOPLE: &str = "people";
pub const INDEXES: [&str; 3] = [CATALOG, LIBRARY, PEOPLE];

/// Is the index per user (owner-filtered)?
#[must_use]
pub fn is_private(index: &str) -> bool {
    index != CATALOG
}

/// Attributes that may be used as filters, per index.
#[must_use]
pub fn filterable(index: &str) -> &'static [&'static str] {
    match index {
        CATALOG => &["kind", "year"],
        LIBRARY => &["kind", "status", "year", "rating", "item_id"],
        PEOPLE => &["source"],
        _ => &[],
    }
}

/// Searchable text attributes, most important first.
#[must_use]
pub fn searchable(index: &str) -> &'static [&'static str] {
    match index {
        CATALOG => &["title", "original_title", "description"],
        LIBRARY => &["title", "original_title", "notes", "review"],
        PEOPLE => &["name", "notes"],
        _ => &[],
    }
}

/// Reject unknown indexes and filter keys before any adapter builds a query.
///
/// # Errors
/// Unknown index or a filter key not on the allowlist.
pub fn check(q: &SearchQuery) -> Result<(), PortError> {
    if !INDEXES.contains(&q.index.as_str()) {
        return Err(PortError(format!("unknown index `{}`", q.index)));
    }
    let allowed = filterable(&q.index);
    for (k, _) in &q.filters {
        if !allowed.contains(&k.as_str()) {
            return Err(PortError(format!(
                "attribute `{k}` is not filterable in `{}`",
                q.index
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::UserId;

    fn q(index: &str, filters: Vec<(&str, &str)>) -> SearchQuery {
        SearchQuery {
            index: index.into(),
            text: "x".into(),
            viewer: UserId(uuid::Uuid::now_v7()),
            filters: filters
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            limit: 10,
        }
    }

    #[test]
    fn only_allowlisted_filters_pass() {
        assert!(check(&q("library", vec![("status", "completed")])).is_ok());
        assert!(check(&q("library", vec![("owner", "someone-else")])).is_err());
        assert!(check(&q("library", vec![("kind = 'x' OR 1", "1")])).is_err());
        assert!(check(&q("secrets", vec![])).is_err());
    }
}
