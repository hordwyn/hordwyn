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
    /// What a merge moved from this companion into the kept one, so it can be undone.
    #[serde(default)]
    pub merge: Option<MergeRecord>,
    /// Removed from People while sessions still name it ("delete keeps history"): hidden and
    /// no longer usable, kept for the history.
    #[serde(default)]
    pub removed_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

/// The record of a merge, kept on the merged-away companion (DEC-017): what moved to the
/// kept companion. Undoing the merge moves it back if the kept one still has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeRecord {
    pub at: Timestamp,
    pub moved_link: Option<UserId>,
    pub moved_notes: Option<String>,
    pub moved_avatar: Option<String>,
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
    /// Neither merged away nor removed: shown on People and usable in new sessions.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.merged_into.is_none() && self.removed_at.is_none()
    }

    /// # Errors
    /// If this companion was merged away or removed.
    pub fn ensure_active(&self) -> Result<(), DomainError> {
        if self.merged_into.is_some() {
            Err(DomainError::NotAllowed(
                "companion was merged into another one".into(),
            ))
        } else if self.removed_at.is_some() {
            Err(DomainError::NotAllowed("companion was removed".into()))
        } else {
            Ok(())
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
pub fn merge(
    keep: &mut Companion,
    remove: &mut Companion,
    at: Timestamp,
) -> Result<(), DomainError> {
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
    if let (Some(a), Some(b)) = (keep.linked_user, remove.linked_user)
        && a != b
    {
        return Err(DomainError::NotAllowed(
            "both companions are linked to different users".into(),
        ));
    }
    let mut record = MergeRecord {
        at,
        moved_link: None,
        moved_notes: None,
        moved_avatar: None,
    };
    if keep.linked_user.is_none() && remove.linked_user.is_some() {
        keep.linked_user = remove.linked_user;
        record.moved_link = remove.linked_user;
    }
    if keep.notes.is_none() && remove.notes.is_some() {
        keep.notes = remove.notes.take();
        record.moved_notes.clone_from(&keep.notes);
    }
    if keep.avatar_asset.is_none() && remove.avatar_asset.is_some() {
        keep.avatar_asset.clone_from(&remove.avatar_asset);
        record.moved_avatar.clone_from(&remove.avatar_asset);
    }
    remove.linked_user = None;
    remove.merged_into = Some(keep.id);
    remove.merge = Some(record);
    Ok(())
}

/// Undo [`merge`] from the record on `remove`: it is active again, and the link, notes and
/// avatar that moved go back if `keep` still has them (DEC-017).
///
/// # Errors
/// If `remove` was not merged into `keep`, or `keep` is no longer active.
pub fn unmerge(keep: &mut Companion, remove: &mut Companion) -> Result<(), DomainError> {
    if remove.merged_into != Some(keep.id) {
        return Err(DomainError::NotAllowed(
            "the companion is not merged into this one".into(),
        ));
    }
    keep.ensure_active()?;
    let record = remove.merge.take();
    remove.merged_into = None;
    let Some(r) = record else {
        return Ok(());
    };
    if r.moved_link.is_some() && keep.linked_user == r.moved_link {
        remove.linked_user = keep.linked_user.take();
    }
    if r.moved_notes.is_some() && keep.notes == r.moved_notes {
        remove.notes = keep.notes.take();
    }
    if r.moved_avatar.is_some() && keep.avatar_asset == r.moved_avatar {
        remove.avatar_asset = keep.avatar_asset.take();
    }
    Ok(())
}

/// The key under which two names count as the same person for merge suggestions: lower
/// case, letters and digits only ("Lee." and " lee" match, "Lee K." does not).
#[must_use]
pub fn name_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
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
            merge: None,
            removed_at: None,
            created_at: DateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn merge_marks_removed_and_moves_link() {
        let (mut keep, mut remove) = (c(1, 9), c(2, 9));
        remove.linked_user = Some(UserId(Uuid::from_u128(5)));
        remove.notes = Some("likes coop".into());
        merge(&mut keep, &mut remove, DateTime::UNIX_EPOCH).unwrap();
        unmerge(&mut keep, &mut remove).unwrap();
        assert_eq!(remove.linked_user, Some(UserId(Uuid::from_u128(5))));
        assert_eq!(remove.notes.as_deref(), Some("likes coop"));
        assert!(keep.linked_user.is_none() && remove.merged_into.is_none());
        merge(&mut keep, &mut remove, DateTime::UNIX_EPOCH).unwrap();
        assert_eq!(remove.merged_into, Some(keep.id));
        assert_eq!(keep.linked_user, Some(UserId(Uuid::from_u128(5))));
        assert_eq!(keep.notes.as_deref(), Some("likes coop"));
        assert!(remove.linked_user.is_none());
    }

    #[test]
    fn merge_rules() {
        let (mut a, mut b) = (c(1, 9), c(2, 8));
        assert!(
            merge(&mut a, &mut b, DateTime::UNIX_EPOCH).is_err(),
            "different owners"
        );
        let (mut a, mut b) = (c(1, 9), c(2, 9));
        b.is_self = true;
        assert!(
            merge(&mut a, &mut b, DateTime::UNIX_EPOCH).is_err(),
            "self cannot be removed"
        );
        let (mut a, mut b) = (c(1, 9), c(2, 9));
        a.linked_user = Some(UserId(Uuid::from_u128(3)));
        b.linked_user = Some(UserId(Uuid::from_u128(4)));
        assert!(
            merge(&mut a, &mut b, DateTime::UNIX_EPOCH).is_err(),
            "conflicting links"
        );
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
