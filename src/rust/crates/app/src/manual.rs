//! Items a person enters or corrects by hand (v1-manual, v1-plan.md 4a): no source, the
//! typed fields marked as theirs ([`USER_PROVENANCE`]) so every writer of the shared item
//! keeps them.

use chrono::Datelike;
use domain::DomainError;
use domain::catalog::{CatalogItem, EntityState};
use domain::ids::ItemId;
use domain::kind::KindId;

use crate::catalog::{MAX_TITLE_CHARS, USER_PROVENANCE};
use crate::{App, AppError, AppResult};

/// Longest cover address a person may type.
pub const MAX_COVER_URL_CHARS: usize = 2000;
/// How many years ahead of the current one a year may be (announced titles).
pub const MAX_YEARS_AHEAD: i32 = 5;
/// How many items of the same title [`App::items_titled`] names.
pub const MAX_TITLED: u32 = 5;

/// What a person types for an item: the fields "Add by hand" and "Edit details" hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemDetails {
    pub title: String,
    pub year: Option<i32>,
    /// Address of a picture every browser that shows the item loads by itself; the server
    /// never fetches it (v1-plan.md 7, B5). Blank is none.
    pub cover_url: Option<String>,
}

/// The cover address as stored: trimmed, blank is none.
///
/// # Errors
/// Not an `https` address with a host, longer than [`MAX_COVER_URL_CHARS`], with user
/// info, or with a character an address does not hold unescaped.
pub fn checked_cover_url(typed: Option<&str>) -> AppResult<Option<String>> {
    let Some(url) = typed.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };
    let bad = |m: &str| -> AppResult<Option<String>> {
        Err(DomainError::invalid("cover_url", m.to_string()).into())
    };
    if url.chars().count() > MAX_COVER_URL_CHARS {
        return bad("the address is longer than 2000 characters");
    }
    let rest = match url.get(..8) {
        Some(scheme) if scheme.eq_ignore_ascii_case("https://") => &url[8..],
        _ => return bad("the address must start with https://"),
    };
    if url
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || "\"'<>\\`{}|^".contains(c))
    {
        return bad("the address holds a character that must be escaped");
    }
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if host.is_empty() {
        return bad("the address has no host");
    }
    if host.contains('@') {
        return bad("the address must not hold a user name or password");
    }
    Ok(Some(url.to_string()))
}

impl App {
    /// The checks "Add by hand", "Edit details" and the import of an item without ids share;
    /// returns the details as stored.
    ///
    /// # Errors
    /// A blank or too long title, a year out of range, a cover address that is not a plain
    /// `https` one (each `Invalid` with its field).
    pub fn checked_details(&self, details: ItemDetails) -> AppResult<ItemDetails> {
        let ItemDetails {
            title,
            year,
            cover_url,
        } = details;
        let title = title.trim();
        if title.is_empty() {
            return Err(DomainError::invalid("title", "title must not be empty").into());
        }
        if title.chars().count() > MAX_TITLE_CHARS {
            return Err(
                DomainError::invalid("title", "title is longer than 500 characters").into(),
            );
        }
        let latest = self.now().year() + MAX_YEARS_AHEAD;
        if year.is_some_and(|y| !(1..=latest).contains(&y)) {
            return Err(DomainError::invalid("year", format!("a year from 1 to {latest}")).into());
        }
        Ok(ItemDetails {
            title: title.to_string(),
            year,
            cover_url: checked_cover_url(cover_url.as_deref())?,
        })
    }

    /// A person adds an item no source knows (any enabled kind). It has no external ids, is
    /// complete as it is (`full`: nothing more will come) and its title, year and cover
    /// address are marked as the person's. `may_set_cover` is the caller's rule for an
    /// address other members' browsers load (a hosted instance: admins only).
    ///
    /// # Errors
    /// A kind that is not enabled; a blank or too long title; a year outside 1 to the
    /// current year plus [`MAX_YEARS_AHEAD`]; a cover address that is not a plain `https`
    /// one; `Forbidden` for a cover address when `may_set_cover` is false.
    pub async fn create_manual_item(
        &self,
        kind: KindId,
        details: ItemDetails,
        may_set_cover: bool,
    ) -> AppResult<CatalogItem> {
        if self.kinds.get(&kind).is_none() {
            return Err(DomainError::invalid("kind", format!("unknown kind `{kind}`")).into());
        }
        let details = self.checked_details(details)?;
        if details.cover_url.is_some() && !may_set_cover {
            return Err(AppError::Forbidden);
        }
        let item = CatalogItem {
            id: ItemId(self.uuid()),
            kind,
            title: details.title,
            original_title: None,
            year: details.year,
            description: None,
            cover_asset: None,
            cover_url: details.cover_url,
            external_ids: Vec::new(),
            attributes: serde_json::json!({}),
            provenance: ["title", "year", "cover_url"]
                .into_iter()
                .map(|field| (field.to_string(), USER_PROVENANCE.to_string()))
                .collect(),
            state: EntityState::Full,
            fetched_at: None,
            fresh_until: None,
            popularity: None,
        };
        // without external ids nobody else can hold one of them: the insert always writes
        self.repos.catalog.insert_new(&item, self.now()).await?;
        Ok(item)
    }

    /// "Add by hand" as a person asks for it: [`App::create_manual_item`], but refused with
    /// `Conflict` (naming the ids) when items of the kind with this title (case and
    /// surrounding blanks apart) exist, unless `allow_duplicate`: a second item of a title
    /// is a choice, not an accident (review MAN-2; nobody can delete an item). The field
    /// checks come first.
    ///
    /// # Errors
    /// As [`App::create_manual_item`]; `Conflict` for a title that is there.
    pub async fn add_manual_item(
        &self,
        kind: KindId,
        details: ItemDetails,
        may_set_cover: bool,
        allow_duplicate: bool,
    ) -> AppResult<CatalogItem> {
        if !allow_duplicate {
            if self.kinds.get(&kind).is_none() {
                return Err(DomainError::invalid("kind", format!("unknown kind `{kind}`")).into());
            }
            let checked = self.checked_details(details.clone())?;
            let there = self.items_titled(&kind, &checked.title).await?;
            if !there.is_empty() {
                let ids: Vec<String> = there.iter().map(|i| i.id.0.to_string()).collect();
                return Err(AppError::Conflict(format!(
                    "items of this kind with this title exist: {}",
                    ids.join(", ")
                )));
            }
        }
        self.create_manual_item(kind, details, may_set_cover).await
    }

    /// "Edit details": a person corrects title, year and cover address of an item no source
    /// answers for (`sourced` says whether one does: the app does not know the sources).
    /// Each field that changes becomes the person's; a field left as it is keeps its
    /// provenance. A new cover address drops the stored picture (it showed the old one);
    /// no address removes a cover the person set and leaves any other cover alone. The
    /// item is `full` afterwards.
    ///
    /// # Errors
    /// Unknown item; `Conflict` when a source answers for it; the checks of
    /// [`App::create_manual_item`]; `Forbidden` for another cover address when
    /// `may_set_cover` is false (keeping or removing the stored one is allowed).
    pub async fn edit_item_details(
        &self,
        id: ItemId,
        details: ItemDetails,
        may_set_cover: bool,
        sourced: &(dyn Fn(&CatalogItem) -> bool + Sync),
    ) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        if sourced(&item) {
            return Err(AppError::Conflict(
                "a source answers for this item; its details follow the source".into(),
            ));
        }
        let mut details = self.checked_details(details)?;
        // no address given: a picture that is the person's is removed; one that is not (a
        // source's address, a stored picture) is not theirs to lose over a corrected title
        let cover_is_theirs =
            item.provenance.get("cover_url").map(String::as_str) == Some(USER_PROVENANCE);
        if details.cover_url.is_none() && !cover_is_theirs {
            details.cover_url.clone_from(&item.cover_url);
        }
        let mut mine = |field: &str| {
            item.provenance
                .insert(field.to_string(), USER_PROVENANCE.to_string());
        };
        if details.title != item.title {
            mine("title");
        }
        if details.year != item.year {
            mine("year");
        }
        if details.cover_url != item.cover_url {
            if details.cover_url.is_some() && !may_set_cover {
                return Err(AppError::Forbidden);
            }
            mine("cover_url");
            item.cover_asset = None;
        }
        item.title = details.title;
        item.year = details.year;
        item.cover_url = details.cover_url;
        item.state = EntityState::Full;
        self.repos.catalog.upsert(&item).await?;
        self.publish_item(item.id).await;
        Ok(item)
    }

    /// The items of `kind` whose title is `title` but for case and surrounding blanks, at
    /// most [`MAX_TITLED`], by id: listed before a second one is entered by hand.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn items_titled(&self, kind: &KindId, title: &str) -> AppResult<Vec<CatalogItem>> {
        let title = title.trim();
        if title.is_empty() {
            return Ok(Vec::new());
        }
        Ok(self.repos.catalog.titled(kind, title, MAX_TITLED).await?)
    }
}
