//! Recently opened items for the empty search palette (design.md 5.3, 6.4; WP4-4): the last
//! [`MAX_RECENT`] item pages opened, newest first, kept in this browser's `localStorage` per
//! user and never sent to the server.

use serde::{Deserialize, Serialize};

/// How many recent items the palette shows (phase 4 plan: the last 8 opened).
pub const MAX_RECENT: usize = 8;

/// One opened item as the palette lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentItem {
    pub id: String,
    /// "Title (year)", as search results read.
    pub label: String,
    /// The kind logs plays (the "Log a play of ..." suggestion).
    #[serde(default)]
    pub plays: bool,
}

/// `item` first, without an older copy of it, at most [`MAX_RECENT`].
#[must_use]
pub fn pushed(mut list: Vec<RecentItem>, item: RecentItem) -> Vec<RecentItem> {
    list.retain(|r| r.id != item.id);
    list.insert(0, item);
    list.truncate(MAX_RECENT);
    list
}

/// The storage key of `username`'s list (one browser may serve several accounts).
#[must_use]
pub fn storage_key(username: &str) -> String {
    format!("app-recent-{username}")
}

/// The stored list (empty when missing or unreadable).
#[must_use]
pub fn load(username: &str) -> Vec<RecentItem> {
    #[cfg(feature = "hydrate")]
    {
        web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(&storage_key(username)).ok().flatten())
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = username;
        Vec::new()
    }
}

/// Forget `username`'s list (sign-out, UX4-22: a shared browser keeps no titles).
pub fn forget(username: &str) {
    #[cfg(feature = "hydrate")]
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.remove_item(&storage_key(username));
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = username;
}

/// Put `item` first in `username`'s list.
pub fn remember(username: &str, item: RecentItem) {
    #[cfg(feature = "hydrate")]
    if let Some(s) = web_sys::window().and_then(|w| w.local_storage().ok().flatten())
        && let Ok(v) = serde_json::to_string(&pushed(load(username), item))
    {
        let _ = s.set_item(&storage_key(username), &v);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (username, item);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: &str) -> RecentItem {
        RecentItem {
            id: id.into(),
            label: id.to_uppercase(),
            plays: false,
        }
    }

    #[test]
    fn newest_first_without_doubles_and_at_most_eight() {
        let mut list = Vec::new();
        for id in ["a", "b", "c"] {
            list = pushed(list, r(id));
        }
        assert_eq!(
            list.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["c", "b", "a"]
        );
        list = pushed(list, r("a"));
        assert_eq!(
            list.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["a", "c", "b"]
        );
        for i in 0..20 {
            list = pushed(list, r(&format!("x{i}")));
        }
        assert_eq!(list.len(), MAX_RECENT);
        assert_eq!(list[0].id, "x19");
        assert_eq!(storage_key("pia"), "app-recent-pia");
        // older stored rows without `plays` still read
        let old: RecentItem = serde_json::from_str(r#"{"id":"1","label":"One"}"#).unwrap();
        assert!(!old.plays);
    }
}
