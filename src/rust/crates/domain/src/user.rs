//! User accounts and roles.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::ids::{CompanionId, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Member,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: Role,
    /// The companion that represents this user in their own address book.
    pub self_companion: CompanionId,
    /// Privacy (D-9): sessions this user logged are visible to linked participants.
    #[serde(default = "default_true")]
    pub share_sessions: bool,
    /// Language for pages, notifications and emails (`en`, `de`); `None` follows the browser
    /// and then the brand default (DEC-201).
    #[serde(default)]
    pub locale: Option<String>,
    /// IANA time zone (`Europe/Berlin`) for days, "today" and times (DEC-239); `None` follows
    /// the brand default, then UTC.
    #[serde(default)]
    pub time_zone: Option<String>,
    pub created_at: Timestamp,
}

const fn default_true() -> bool {
    true
}

/// The authenticated caller of a use-case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actor {
    pub user_id: UserId,
    pub role: Role,
}

impl Actor {
    #[must_use]
    pub fn is_admin(&self) -> bool {
        self.role == Role::Admin
    }
}

/// # Errors
/// Usernames are 3..=32 characters of `[a-z0-9_.-]`.
pub fn validate_username(name: &str) -> Result<(), DomainError> {
    let ok = (3..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c));
    if ok {
        Ok(())
    } else {
        Err(DomainError::invalid(
            "username",
            "use 3 to 32 characters: a-z, 0-9, _ . -",
        ))
    }
}
