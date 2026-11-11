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
    /// Played together without winning or losing (OT-18): many co-op games have no such
    /// end. Stored and sent as `none`.
    #[serde(rename = "none")]
    NoResult,
}

impl CoopOutcome {
    /// The stored and sent word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Won => "won",
            Self::Lost => "lost",
            Self::NoResult => "none",
        }
    }

    /// The outcome of a stored or sent word.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        [Self::Won, Self::Lost, Self::NoResult]
            .into_iter()
            .find(|o| o.as_str() == word)
    }
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
    /// Whether the play has a result: a co-op game won or lost, or a competitive play with
    /// a winner. A co-op play without a result (OT-18) and a play without a winner have
    /// none, so they count in no win rate.
    #[must_use]
    pub fn decided(&self) -> bool {
        match self.coop_outcome {
            Some(CoopOutcome::Won | CoopOutcome::Lost) => true,
            Some(CoopOutcome::NoResult) => false,
            None => self.participants.iter().any(|p| p.winner),
        }
    }

    /// The result for the person the entries `mine` stand for: `Some(true)` a win,
    /// `Some(false)` a play with a result that was not theirs, `None` when the play has no
    /// result or they did not take part. A co-op win is a win for everyone at the table.
    #[must_use]
    pub fn result_for(&self, mine: &[CompanionId]) -> Option<bool> {
        let mut own = self
            .participants
            .iter()
            .filter(|p| mine.contains(&p.companion_id))
            .peekable();
        own.peek()?;
        if !self.decided() {
            return None;
        }
        Some(match self.coop_outcome {
            Some(outcome) => outcome == CoopOutcome::Won,
            None => own.any(|p| p.winner),
        })
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> CompanionId {
        CompanionId(uuid::Uuid::from_u128(n))
    }

    fn play(parts: &[(u128, bool)], coop: Option<CoopOutcome>) -> Session {
        Session {
            id: SessionId(uuid::Uuid::from_u128(99)),
            owner: UserId(uuid::Uuid::from_u128(98)),
            item_id: ItemId(uuid::Uuid::from_u128(97)),
            started_at: Timestamp::default(),
            duration_minutes: None,
            location: None,
            notes: None,
            participants: parts
                .iter()
                .map(|(c, winner)| Participant {
                    winner: *winner,
                    ..Participant::new(id(*c))
                })
                .collect(),
            coop_outcome: coop,
            incomplete: false,
            game_group_id: None,
        }
    }

    /// OT-18: a co-op play can end without a result; it is stored as its own outcome and
    /// the two old ones keep their words.
    #[test]
    fn a_coop_outcome_has_three_stored_words() {
        for (outcome, word) in [
            (CoopOutcome::Won, "won"),
            (CoopOutcome::Lost, "lost"),
            (CoopOutcome::NoResult, "none"),
        ] {
            assert_eq!(outcome.as_str(), word);
            assert_eq!(CoopOutcome::parse(word), Some(outcome));
            assert_eq!(
                serde_json::to_value(outcome).unwrap(),
                serde_json::json!(word)
            );
        }
        assert_eq!(CoopOutcome::parse("draw"), None);
    }

    /// The win rate rule (ot2f): a play has a result when a co-op game was won or lost, or
    /// a competitive play names a winner; a co-op play without a result and a play without
    /// a winner have none.
    #[test]
    fn a_play_has_a_result_only_when_one_was_logged() {
        assert!(play(&[(1, false), (2, true)], None).decided());
        assert!(!play(&[(1, false), (2, false)], None).decided());
        assert!(play(&[(1, false)], Some(CoopOutcome::Won)).decided());
        assert!(play(&[(1, false)], Some(CoopOutcome::Lost)).decided());
        assert!(!play(&[(1, false)], Some(CoopOutcome::NoResult)).decided());
        // a winner mark left on a co-op play without a result does not decide it
        assert!(!play(&[(1, true)], Some(CoopOutcome::NoResult)).decided());
    }

    /// A person's result: only of plays they took part in; a co-op win is everyone's, a
    /// co-op loss nobody's; one of several entries standing for the person is enough.
    #[test]
    fn a_result_counts_for_the_people_who_took_part() {
        let mine = [id(1), id(7)];
        let competitive = play(&[(1, false), (2, true)], None);
        assert_eq!(competitive.result_for(&mine), Some(false));
        assert_eq!(competitive.result_for(&[id(2)]), Some(true));
        assert_eq!(competitive.result_for(&[id(3)]), None, "did not take part");
        assert_eq!(
            play(&[(7, true), (2, false)], None).result_for(&mine),
            Some(true)
        );
        let won = play(&[(1, false), (2, false)], Some(CoopOutcome::Won));
        assert_eq!(won.result_for(&mine), Some(true));
        assert_eq!(won.result_for(&[id(3)]), None, "did not take part");
        let lost = play(&[(1, true)], Some(CoopOutcome::Lost));
        assert_eq!(lost.result_for(&mine), Some(false));
        let open = play(&[(1, false)], Some(CoopOutcome::NoResult));
        assert_eq!(open.result_for(&mine), None);
        assert_eq!(
            play(&[(1, false), (2, false)], None).result_for(&mine),
            None,
            "no winner named"
        );
    }
}
