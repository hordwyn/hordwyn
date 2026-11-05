//! A user's library entry for one catalog item: status, rating and notes are LWW fields.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::hlc::Lww;
use crate::ids::{EntryId, ItemId, UserId};
use crate::kind::{KindId, Status};

/// Rating on a 0..=100 scale; the UI maps it to stars or 1..10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Rating(u8);

impl Rating {
    pub const MAX: u8 = 100;

    /// # Errors
    /// If `v` is above [`Rating::MAX`].
    pub fn new(v: u8) -> Result<Self, DomainError> {
        if v <= Self::MAX {
            Ok(Self(v))
        } else {
            Err(DomainError::invalid(
                "rating",
                format!("{v} is above {}", Self::MAX),
            ))
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Rating {
    type Error = DomainError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}

impl From<Rating> for u8 {
    fn from(r: Rating) -> Self {
        r.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: EntryId,
    pub user_id: UserId,
    pub item_id: ItemId,
    pub kind: KindId,
    pub status: Lww<Status>,
    pub rating: Lww<Option<Rating>>,
    pub notes: Lww<Option<String>>,
    pub added_at: Timestamp,
}
