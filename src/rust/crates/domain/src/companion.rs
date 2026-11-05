//! Companions: people a user does things with (D-9). History always references companions,
//! so promotion, demotion and merging never rewrite sessions.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::ids::{CompanionId, InviteId, LinkRequestId, UserId};

/// Name and avatar of a linked user, kept when the link ends (demotion).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSnapshot {
    pub display_name: String,
    pub avatar_asset: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Companion {
    pub id: CompanionId,
    pub owner: UserId,
    pub name: String,
    pub avatar_asset: Option<String>,
    pub notes: Option<String>,
    pub linked_user: Option<UserId>,
    /// Snapshot of the formerly linked user, set on demotion.
    pub snapshot: Option<UserSnapshot>,
    /// The owner's own self-companion.
    pub is_self: bool,
    /// Set when this companion was merged into another one; it is then read-only and every
    /// reference resolves to the target.
    pub merged_into: Option<CompanionId>,
    pub created_at: Timestamp,
}

pub const MAX_COMPANION_NAME_CHARS: usize = 100;

/// # Errors
/// If the name is blank or too long.
pub fn validate_companion_name(name: &str) -> Result<(), DomainError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > MAX_COMPANION_NAME_CHARS {
        return Err(DomainError::invalid(
            "name",
            "name must be 1..=100 characters",
        ));
    }
    Ok(())
}

impl Companion {
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.merged_into.is_none()
    }

    /// # Errors
    /// If this companion was merged away.
    pub fn ensure_active(&self) -> Result<(), DomainError> {
        if self.is_active() {
            Ok(())
        } else {
            Err(DomainError::NotAllowed(
                "companion was merged into another one".into(),
            ))
        }
    }

    /// End the link to a user, keeping a snapshot of their name and avatar (demotion).
    pub fn demote(&mut self, snapshot: UserSnapshot) {
        if self.linked_user.take().is_some() {
            self.snapshot = Some(snapshot);
        }
    }
}

/// Merge `remove` into `keep` (both owned by the same user). Nothing that references
/// `remove` is rewritten; readers resolve `merged_into`.
///
/// # Errors
/// If owners differ, either is already merged, `remove` is the self-companion, or both are
/// linked to different users.
pub fn merge(keep: &mut Companion, remove: &mut Companion) -> Result<(), DomainError> {
    if keep.id == remove.id {
        return Err(DomainError::NotAllowed(
            "cannot merge a companion into itself".into(),
        ));
    }
    if keep.owner != remove.owner {
        return Err(DomainError::NotAllowed(
            "companions belong to different users".into(),
        ));
    }
    keep.ensure_active()?;
    remove.ensure_active()?;
    if remove.is_self {
        return Err(DomainError::NotAllowed(
            "the self-companion can only be the merge target".into(),
        ));
    }
    match (keep.linked_user, remove.linked_user) {
        (Some(a), Some(b)) if a != b => {
            return Err(DomainError::NotAllowed(
                "both companions are linked to different users".into(),
            ));
        }
        (None, Some(b)) => keep.linked_user = Some(b),
        _ => {}
    }
    if keep.notes.is_none() {
        keep.notes = remove.notes.take();
    }
    if keep.avatar_asset.is_none() {
        keep.avatar_asset.clone_from(&remove.avatar_asset);
    }
    remove.linked_user = None;
    remove.merged_into = Some(keep.id);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestState {
    Pending,
    Accepted,
    Declined,
    Cancelled,
}

/// Request from a companion's owner to link it to an existing user; needs their acceptance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRequest {
    pub id: LinkRequestId,
    pub companion_id: CompanionId,
    pub from: UserId,
    pub to: UserId,
    pub state: RequestState,
    pub created_at: Timestamp,
}

/// Invitation to create an account from a companion (promotion). Only the hash of the token
/// is stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invite {
    pub id: InviteId,
    pub companion_id: CompanionId,
    pub invited_by: UserId,
    pub email: Option<String>,
    pub token_hash: String,
    pub state: RequestState,
    pub expires_at: Timestamp,
    pub created_at: Timestamp,
}

/// How long an invite stays valid.
pub const INVITE_TTL_DAYS: i64 = 14;

/// Expiry of an invite created at `now`.
#[must_use]
pub fn invite_expiry(now: crate::Timestamp) -> crate::Timestamp {
    now + chrono::Duration::days(INVITE_TTL_DAYS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use uuid::Uuid;

    fn c(n: u128, owner: u128) -> Companion {
        Companion {
            id: CompanionId(Uuid::from_u128(n)),
            owner: UserId(Uuid::from_u128(owner)),
            name: format!("c{n}"),
            avatar_asset: None,
            notes: None,
            linked_user: None,
            snapshot: None,
            is_self: false,
            merged_into: None,
            created_at: DateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn merge_marks_removed_and_moves_link() {
        let (mut keep, mut remove) = (c(1, 9), c(2, 9));
        remove.linked_user = Some(UserId(Uuid::from_u128(5)));
        remove.notes = Some("likes coop".into());
        merge(&mut keep, &mut remove).unwrap();
        assert_eq!(remove.merged_into, Some(keep.id));
        assert_eq!(keep.linked_user, Some(UserId(Uuid::from_u128(5))));
        assert_eq!(keep.notes.as_deref(), Some("likes coop"));
        assert!(remove.linked_user.is_none());
    }

    #[test]
    fn merge_rules() {
        let (mut a, mut b) = (c(1, 9), c(2, 8));
        assert!(merge(&mut a, &mut b).is_err(), "different owners");
        let (mut a, mut b) = (c(1, 9), c(2, 9));
        b.is_self = true;
        assert!(merge(&mut a, &mut b).is_err(), "self cannot be removed");
        let (mut a, mut b) = (c(1, 9), c(2, 9));
        a.linked_user = Some(UserId(Uuid::from_u128(3)));
        b.linked_user = Some(UserId(Uuid::from_u128(4)));
        assert!(merge(&mut a, &mut b).is_err(), "conflicting links");
    }

    #[test]
    fn demote_keeps_snapshot() {
        let mut a = c(1, 9);
        a.linked_user = Some(UserId(Uuid::from_u128(3)));
        a.demote(UserSnapshot {
            display_name: "Anna".into(),
            avatar_asset: None,
        });
        assert!(a.linked_user.is_none());
        assert_eq!(a.snapshot.unwrap().display_name, "Anna");
    }
}
