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
    /// The kind id (`movie`): the icon and the shape of the row's leading box.
    #[serde(default)]
    pub kind: String,
    /// The address of a small cover (DEC-640: the stored variant, else the provider's
    /// address) as the item page had it, or a later read that named the item
    /// ([`refreshed`]); the row shows the icon when it no longer loads.
    #[serde(default)]
    pub cover: Option<String>,
}

/// `item` first, without an older copy of it, at most [`MAX_RECENT`].
#[must_use]
pub fn pushed(mut list: Vec<RecentItem>, item: RecentItem) -> Vec<RecentItem> {
    list.retain(|r| r.id != item.id);
    list.insert(0, item);
    list.truncate(MAX_RECENT);
    list
}

/// The list with the covers a read just returned (`fresh`: item id and the address by the one
/// rule, DEC-640) for the rows that are in it; `None` when no row changes. A row keeps the
/// address of the day its item page was opened, often the provider's because the cover was
/// not stored yet (review POL-3): every later answer that names the item corrects it. A
/// stored cover is never given up for a provider's address or for none: an answer may know
/// less than the item page did, and only the item page writes whatever it has.
#[must_use]
pub fn refreshed(
    list: &[RecentItem],
    fresh: &[(String, Option<String>)],
) -> Option<Vec<RecentItem>> {
    let mut changed = false;
    let new = list
        .iter()
        .map(|row| {
            let now = fresh.iter().find(|(id, _)| *id == row.id).map(|(_, c)| c);
            match now {
                Some(cover)
                    if *cover != row.cover
                        && (cover.as_deref().is_some_and(is_stored)
                            || !row.cover.as_deref().is_some_and(is_stored)) =>
                {
                    changed = true;
                    RecentItem {
                        cover: cover.clone(),
                        ..row.clone()
                    }
                }
                _ => row.clone(),
            }
        })
        .collect();
    changed.then_some(new)
}

/// An address this instance serves (a stored variant, `/assets/...`), not a provider's.
fn is_stored(url: &str) -> bool {
    url.starts_with('/') && !url.starts_with("//")
}

/// Write the covers of a read into `username`'s list ([`refreshed`]); the new list when a
/// row changed.
#[must_use]
pub fn refresh(username: &str, fresh: &[(String, Option<String>)]) -> Option<Vec<RecentItem>> {
    #[cfg(feature = "hydrate")]
    {
        let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten())?;
        let list = refreshed(&load(username), fresh)?;
        let _ = storage.set_item(&storage_key(username), &serde_json::to_string(&list).ok()?);
        Some(list)
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (username, fresh);
        None
    }
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
            kind: String::new(),
            cover: None,
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
        assert_eq!((old.kind.as_str(), old.cover), ("", None));
    }

    /// Review POL-3: a row remembered with the provider's address takes the stored cover as
    /// soon as a read names it.
    #[test]
    fn a_fresh_read_replaces_the_remembered_cover() {
        let row = |id: &str, cover: Option<&str>| RecentItem {
            cover: cover.map(Into::into),
            ..r(id)
        };
        let fresh = |id: &str, cover: Option<&str>| (id.to_string(), cover.map(String::from));
        let list = vec![
            row("a", Some("https://img.example/a.jpg")),
            row("b", Some("/assets/b/thumb.webp")),
            row("c", None),
        ];
        // items that are not in the list, and a cover that is the same: nothing to write
        assert_eq!(
            refreshed(&list, &[fresh("x", Some("/assets/x.webp"))]),
            None
        );
        assert_eq!(
            refreshed(&list, &[fresh("b", Some("/assets/b/thumb.webp"))]),
            None
        );
        assert_eq!(refreshed(&list, &[]), None);
        let new = refreshed(
            &list,
            &[
                fresh("c", Some("https://img.example/c.jpg")),
                fresh("x", None),
                fresh("a", Some("/assets/a/thumb.webp")),
            ],
        )
        .expect("two rows change");
        assert_eq!(
            new.iter()
                .map(|x| (x.id.as_str(), x.cover.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("a", Some("/assets/a/thumb.webp")),
                ("b", Some("/assets/b/thumb.webp")),
                ("c", Some("https://img.example/c.jpg")),
            ]
        );
        // everything else of a row stays
        assert_eq!(new[0].label, list[0].label);
        // a stored cover is not given up for what an answer says: no cover, or the
        // provider's address (the item page writes those)
        assert_eq!(refreshed(&list, &[fresh("b", None)]), None);
        assert_eq!(
            refreshed(&list, &[fresh("b", Some("https://img.example/b.jpg"))]),
            None
        );
        assert_eq!(
            refreshed(&list, &[fresh("b", Some("//img.example/b.jpg"))]),
            None
        );
        // a provider's address that changed, or went away, is taken
        let moved = refreshed(&list, &[fresh("a", Some("https://img.example/a2.jpg"))]);
        assert_eq!(
            moved.expect("one row changes")[0].cover.as_deref(),
            Some("https://img.example/a2.jpg")
        );
        let gone = refreshed(&list, &[fresh("a", None)]).expect("one row changes");
        assert_eq!(gone[0].cover, None);
    }
}
