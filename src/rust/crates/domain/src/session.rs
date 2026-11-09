//! Sessions with participants: board game plays, gaming sessions, "watched with" (D-9, D-9a).
//! Participants always reference a companion, never a user.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::ids::{CompanionId, GameGroupId, ItemId, SessionId, UserId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Participant {
    pub companion_id: CompanionId,
    pub score: Option<f64>,
    pub winner: bool,
    pub rank: Option<u32>,
    pub team: Option<String>,
    pub faction: Option<String>,
    pub starting_player: bool,
    pub first_play: bool,
}

impl Participant {
    #[must_use]
    pub fn new(companion_id: CompanionId) -> Self {
        Self {
            companion_id,
            score: None,
            winner: false,
            rank: None,
            team: None,
            faction: None,
            starting_player: false,
            first_play: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoopOutcome {
    Won,
    Lost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    /// User whose log this session belongs to.
    pub owner: UserId,
    pub item_id: ItemId,
    pub started_at: Timestamp,
    pub duration_minutes: Option<u32>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub participants: Vec<Participant>,
    pub coop_outcome: Option<CoopOutcome>,
    pub incomplete: bool,
    /// The game group this play was logged as (design.md 6.7); the group may be removed
    /// since, the session keeps it.
    #[serde(default)]
    pub game_group_id: Option<GameGroupId>,
}

impl Session {
    /// # Errors
    /// If a companion appears twice or a score is not finite.
    pub fn validate(&self) -> Result<(), DomainError> {
        let mut seen = std::collections::HashSet::new();
        for p in &self.participants {
            if !seen.insert(p.companion_id) {
                return Err(DomainError::invalid(
                    "participants",
                    "a companion can only participate once",
                ));
            }
            if p.score.is_some_and(|s| !s.is_finite()) {
                return Err(DomainError::invalid(
                    "score",
                    "score must be a finite number",
                ));
            }
        }
        Ok(())
    }
}
