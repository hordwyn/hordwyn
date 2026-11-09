//! Opt-in yearly goals (design.md 9, research R-04): private to their owner, one per year,
//! group (or all groups) and measure. Progress is never judged against the calendar: no
//! "behind" or "ahead", only how many of how many.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::ids::{GoalId, UserId};
use crate::kind::KindGroup;

/// What a goal counts (the statistics' amounts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalMetric {
    /// Entries completed.
    Completions,
    /// `Finished` events (rewatches and rereads included).
    Finished,
    Episodes,
    Pages,
    Plays,
    Minutes,
}

impl GoalMetric {
    pub const ALL: [Self; 6] = [
        Self::Completions,
        Self::Finished,
        Self::Episodes,
        Self::Pages,
        Self::Plays,
        Self::Minutes,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Completions => "completions",
            Self::Finished => "finished",
            Self::Episodes => "episodes",
            Self::Pages => "pages",
            Self::Plays => "plays",
            Self::Minutes => "minutes",
        }
    }

    /// # Errors
    /// An unknown measure.
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        Self::ALL
            .into_iter()
            .find(|m| m.as_str() == s)
            .ok_or_else(|| {
                DomainError::invalid(
                    "metric",
                    "metric must be completions, finished, episodes, pages, plays or minutes",
                )
            })
    }
}

/// The years a goal may name.
pub const GOAL_YEARS: std::ops::RangeInclusive<i32> = 2000..=2100;
/// The largest target.
pub const MAX_GOAL_TARGET: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub id: GoalId,
    pub user_id: UserId,
    pub year: i32,
    /// `None`: every group.
    pub group: Option<KindGroup>,
    pub metric: GoalMetric,
    pub target: u32,
    pub created_at: Timestamp,
}

impl Goal {
    /// # Errors
    /// A year outside [`GOAL_YEARS`] or a target outside `1..=MAX_GOAL_TARGET`.
    pub fn validate(&self) -> Result<(), DomainError> {
        if !GOAL_YEARS.contains(&self.year) {
            return Err(DomainError::invalid("year", "year must be 2000..=2100"));
        }
        if self.target == 0 || self.target > MAX_GOAL_TARGET {
            return Err(DomainError::invalid("target", "target must be 1..=1000000"));
        }
        Ok(())
    }

    /// Same year, group and measure: the one goal allowed for them.
    #[must_use]
    pub fn same_slot(&self, other: &Self) -> bool {
        self.user_id == other.user_id
            && self.year == other.year
            && self.group == other.group
            && self.metric == other.metric
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_round_trip_and_bounds_hold() {
        for m in GoalMetric::ALL {
            assert_eq!(GoalMetric::parse(m.as_str()), Ok(m));
        }
        assert!(GoalMetric::parse("books").is_err());
        let mut g = Goal {
            id: GoalId(uuid::Uuid::nil()),
            user_id: UserId(uuid::Uuid::nil()),
            year: 2026,
            group: Some(KindGroup::Read),
            metric: GoalMetric::Finished,
            target: 12,
            created_at: chrono::DateTime::UNIX_EPOCH,
        };
        assert_eq!(g.validate(), Ok(()));
        g.target = 0;
        assert!(g.validate().is_err());
        g.target = 1;
        g.year = 1999;
        assert!(g.validate().is_err());
    }
}
