//! Game groups (design.md 6.7, DESIGN.md 18 "game groups (recurring game nights)"): a named
//! set of the owner's people who play together. Members reference companions, so merging a
//! member follows the merge when the group is read; a removed group keeps its sessions.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::companion::validate_companion_name;
use crate::ids::{CompanionId, GameGroupId, UserId};

/// The most members a group may have.
pub const MAX_GROUP_MEMBERS: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameGroup {
    pub id: GameGroupId,
    pub owner: UserId,
    pub name: String,
    /// In the order the owner chose; the owner's own entry is never a member (a play adds
    /// the owner anyway).
    pub members: Vec<CompanionId>,
    /// Set when the owner removed the group: hidden, its sessions keep pointing at it.
    pub removed_at: Option<Timestamp>,
    pub created_at: Timestamp,
}

impl GameGroup {
    /// # Errors
    /// A blank or too long name (the rules of a person's name), no members, too many, or a
    /// member twice.
    pub fn validate(&self) -> Result<(), DomainError> {
        validate_companion_name(&self.name)?;
        if self.members.is_empty() || self.members.len() > MAX_GROUP_MEMBERS {
            return Err(DomainError::invalid(
                "members",
                "a group needs 1..=30 members",
            ));
        }
        let mut seen = std::collections::HashSet::new();
        if !self.members.iter().all(|m| seen.insert(*m)) {
            return Err(DomainError::invalid(
                "members",
                "a person can only be a member once",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.removed_at.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_group_needs_a_name_and_distinct_members() {
        let a = CompanionId(uuid::Uuid::from_u128(1));
        let mut g = GameGroup {
            id: GameGroupId(uuid::Uuid::nil()),
            owner: UserId(uuid::Uuid::nil()),
            name: "Thursday".into(),
            members: vec![a],
            removed_at: None,
            created_at: chrono::DateTime::UNIX_EPOCH,
        };
        assert_eq!(g.validate(), Ok(()));
        g.members.push(a);
        assert!(g.validate().is_err());
        g.members.clear();
        assert!(g.validate().is_err());
        g.members.push(a);
        g.name = "  ".into();
        assert!(g.validate().is_err());
    }
}
