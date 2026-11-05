//! Written reviews.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::ids::{ItemId, ReviewId, UserId};
use crate::list::Visibility;

pub const MAX_REVIEW_CHARS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Review {
    pub id: ReviewId,
    pub user_id: UserId,
    pub item_id: ItemId,
    pub body: String,
    pub contains_spoilers: bool,
    pub visibility: Visibility,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

/// # Errors
/// If the body is blank or longer than [`MAX_REVIEW_CHARS`].
pub fn validate_review_body(body: &str) -> Result<(), DomainError> {
    if body.trim().is_empty() {
        return Err(DomainError::invalid("body", "review must not be empty"));
    }
    if body.chars().count() > MAX_REVIEW_CHARS {
        return Err(DomainError::invalid("body", "review is too long"));
    }
    Ok(())
}
