//! User lists and visibility (D-9 sharing).

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::ids::{ItemId, ListId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Visibility {
    Private,
    /// Visible to every user of this instance.
    Instance,
    /// Visible to anyone who has the link token.
    Link {
        token: String,
    },
}

/// What the share dialog offers (design.md 5.10): the visibility without the link token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityChoice {
    Private,
    Instance,
    Link,
}

impl VisibilityChoice {
    /// `private`, `instance` or `link`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Instance => "instance",
            Self::Link => "link",
        }
    }

    /// # Errors
    /// Anything but `private`, `instance` or `link`.
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s {
            "private" => Ok(Self::Private),
            "instance" => Ok(Self::Instance),
            "link" => Ok(Self::Link),
            _ => Err(DomainError::invalid(
                "visibility",
                "visibility must be private, instance or link",
            )),
        }
    }
}

impl Visibility {
    #[must_use]
    pub const fn choice(&self) -> VisibilityChoice {
        match self {
            Self::Private => VisibilityChoice::Private,
            Self::Instance => VisibilityChoice::Instance,
            Self::Link { .. } => VisibilityChoice::Link,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub item_id: ItemId,
    pub note: Option<String>,
    pub added_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct List {
    pub id: ListId,
    pub owner: UserId,
    pub name: String,
    pub description: Option<String>,
    pub visibility: Visibility,
    /// Ordered; position is the index.
    pub items: Vec<ListItem>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

pub const MAX_LIST_NAME_CHARS: usize = 200;

/// # Errors
/// If the name is blank or too long.
pub fn validate_list_name(name: &str) -> Result<(), DomainError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > MAX_LIST_NAME_CHARS {
        return Err(DomainError::invalid(
            "name",
            "list name must be 1..=200 characters",
        ));
    }
    Ok(())
}

impl List {
    /// Add an item at the end (no-op if already present). Returns whether it was added.
    pub fn add(&mut self, item: ListItem) -> bool {
        if self.items.iter().any(|i| i.item_id == item.item_id) {
            return false;
        }
        self.items.push(item);
        true
    }

    /// Remove an item. Returns whether it was present.
    pub fn remove(&mut self, item_id: ItemId) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.item_id != item_id);
        before != self.items.len()
    }

    /// Move an item to `to` (clamped to the end).
    ///
    /// # Errors
    /// If the item is not in the list.
    pub fn move_item(&mut self, item_id: ItemId, to: usize) -> Result<(), DomainError> {
        let from = self
            .items
            .iter()
            .position(|i| i.item_id == item_id)
            .ok_or_else(|| DomainError::invalid("item_id", "item is not in the list"))?;
        let it = self.items.remove(from);
        let to = to.min(self.items.len());
        self.items.insert(to, it);
        Ok(())
    }
}
