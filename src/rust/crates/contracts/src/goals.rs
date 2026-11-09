//! WP4-6: opt-in yearly goals, "this year so far vs the same period last year", the people
//! seen most often (design.md 6.1, 6.6, 9; research R-04), and game groups (design.md 6.7).

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::{AmountsDto, StatItemDto};

/// A private yearly goal and how much of it is done ("7 of 12 in 2026").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct GoalDto {
    pub id: String,
    pub year: i32,
    /// A group id, or `all`.
    pub group: String,
    /// `completions`, `finished`, `episodes`, `pages`, `plays` or `minutes`.
    pub metric: String,
    pub target: u32,
    pub done: u64,
}

/// Body of `POST /stats/goals` and `PUT /stats/goals/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NewGoalDto {
    /// 2000..=2100.
    pub year: i32,
    /// A group id, or `all`.
    pub group: String,
    pub metric: String,
    /// 1..=1000000.
    pub target: u32,
}

/// One group's numbers this year so far and over the same days last year.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CompareDto {
    /// A group id, or `all`.
    pub group: String,
    pub this: AmountsDto,
    pub last: AmountsDto,
}

/// "This year so far vs the same period last year": January 1 to today in the caller's
/// zone, and January 1 to the same day last year (February 28 for February 29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct YearCompareDto {
    pub year: i32,
    /// `2026-03-07`: the last day counted this year.
    pub through: String,
    /// The last day counted last year.
    pub last_through: String,
    /// `all` first, then Watch, Read, Listen, Play (and Other with activity).
    pub groups: Vec<CompareDto>,
}

/// A person seen often ("Most often with").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct OftenWithDto {
    pub id: String,
    pub name: String,
    /// Sessions together (plays and "watched with").
    pub together: u32,
    pub plays: u32,
}

/// `GET /stats/compare`: the comparison and up to five people by sessions together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct StatsCompareDto {
    pub compare: YearCompareDto,
    pub often_with: Vec<OftenWithDto>,
}

/// A member of a game group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct GroupMemberDto {
    pub id: String,
    pub name: String,
}

/// A game group card (design.md 6.7): members as they stand now (merges followed), plays
/// logged as the group, the latest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct GameGroupDto {
    pub id: String,
    pub name: String,
    pub members: Vec<GroupMemberDto>,
    pub plays: u32,
    pub last: Option<StatItemDto>,
}

/// Body of `POST /game-groups` and `PUT /game-groups/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct GameGroupInputDto {
    pub name: String,
    /// Companion ids of the caller's people (not their own entry), 1..=30.
    pub members: Vec<String>,
}
