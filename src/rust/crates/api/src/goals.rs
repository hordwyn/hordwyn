//! WP4-6, shared by the REST handlers and the web app's server functions: private yearly
//! goals, "this year so far vs the same period last year" with the people seen most often,
//! and game groups.

use std::collections::HashMap;

use app::time_zone::local_day;
use app::{ComparePair, GameGroupInput, GameGroupView, NewGoal};
use contracts::{
    CompareDto, GameGroupDto, GameGroupInputDto, GoalDto, GroupMemberDto, NewGoalDto, OftenWithDto,
    StatItemDto, StatsCompareDto, YearCompareDto,
};
use domain::goal::GoalMetric;
use domain::ids::{CompanionId, GameGroupId, GoalId};
use domain::kind::KindGroup;
use domain::user::Actor;

use crate::browse::{amounts, parse_group};
use crate::error::ApiResult;
use crate::ops::parse_id;
use crate::{ApiError, ApiState};

/// People shown under "Most often with".
pub const OFTEN_WITH: usize = 5;

fn goal_dto(g: &app::GoalProgress) -> GoalDto {
    GoalDto {
        id: g.goal.id.0.to_string(),
        year: g.goal.year,
        group: g.goal.group.map_or("all", KindGroup::as_str).into(),
        metric: g.goal.metric.as_str().into(),
        target: g.goal.target,
        done: g.done,
    }
}

fn new_goal(b: &NewGoalDto) -> ApiResult<NewGoal> {
    Ok(NewGoal {
        year: b.year,
        group: match b.group.as_str() {
            "all" => None,
            g => Some(parse_group(g)?),
        },
        metric: GoalMetric::parse(&b.metric).map_err(app::AppError::from)?,
        target: b.target,
    })
}

/// # Errors
/// Storage errors.
pub async fn goals(s: &ApiState, a: Actor) -> ApiResult<Vec<GoalDto>> {
    Ok(s.app.goals(a).await?.iter().map(goal_dto).collect())
}

/// # Errors
/// Unknown group or measure, bad year or target, a goal for the same slot (409).
pub async fn create_goal(s: &ApiState, a: Actor, b: &NewGoalDto) -> ApiResult<GoalDto> {
    Ok(goal_dto(&s.app.set_goal(a, new_goal(b)?).await?))
}

/// # Errors
/// As [`create_goal`]; not the caller's goal (404).
pub async fn update_goal(s: &ApiState, a: Actor, id: &str, b: &NewGoalDto) -> ApiResult<GoalDto> {
    let id = parse_id(id, GoalId)?;
    Ok(goal_dto(&s.app.update_goal(a, id, new_goal(b)?).await?))
}

/// # Errors
/// Not the caller's goal (404).
pub async fn delete_goal(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    Ok(s.app.delete_goal(a, parse_id(id, GoalId)?).await?)
}

fn compare_dto(group: &str, p: &ComparePair) -> CompareDto {
    CompareDto {
        group: group.into(),
        this: amounts(&p.this),
        last: amounts(&p.last),
    }
}

/// The year so far against the same days last year, and the people with the most sessions
/// together (then plays, then name).
///
/// # Errors
/// Storage errors.
pub async fn stats_compare(s: &ApiState, a: Actor) -> ApiResult<StatsCompareDto> {
    let c = s.app.year_compare(a).await?;
    let mut groups = vec![compare_dto("all", &c.overall)];
    groups.extend(c.groups.iter().map(|(g, p)| compare_dto(g.as_str(), p)));
    let names: HashMap<CompanionId, String> = s
        .app
        .companions(a)
        .await?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    let mut people: Vec<OftenWithDto> = s
        .app
        .companion_stats(a)
        .await?
        .into_iter()
        .filter(|st| st.together > 0)
        .filter_map(|st| {
            Some(OftenWithDto {
                name: names.get(&st.companion_id)?.clone(),
                id: st.companion_id.0.to_string(),
                together: st.together,
                plays: st.plays,
            })
        })
        .collect();
    people.sort_by(|x, y| {
        (y.together, y.plays)
            .cmp(&(x.together, x.plays))
            .then_with(|| x.name.to_lowercase().cmp(&y.name.to_lowercase()))
    });
    people.truncate(OFTEN_WITH);
    Ok(StatsCompareDto {
        compare: YearCompareDto {
            year: c.year,
            through: c.through.to_string(),
            last_through: c.last_through.to_string(),
            groups,
        },
        often_with: people,
        play: crate::browse::play_totals(s.app.play_totals(a).await?),
    })
}

fn group_dto(
    g: &GameGroupView,
    names: &HashMap<CompanionId, String>,
    zone: app::time_zone::Tz,
) -> GameGroupDto {
    GameGroupDto {
        id: g.group.id.0.to_string(),
        name: g.group.name.clone(),
        members: g
            .group
            .members
            .iter()
            .map(|m| GroupMemberDto {
                id: m.0.to_string(),
                name: names.get(m).cloned().unwrap_or_default(),
            })
            .collect(),
        plays: g.plays,
        last: g.last.as_ref().map(|l| StatItemDto {
            item_id: l.item_id.0.to_string(),
            title: l.title.clone(),
            at: l.at.to_rfc3339(),
            local_day: local_day(l.at, zone),
        }),
    }
}

/// The caller's game groups by name.
///
/// # Errors
/// Storage errors.
pub async fn game_groups(s: &ApiState, a: Actor) -> ApiResult<Vec<GameGroupDto>> {
    let groups = s.app.game_groups(a).await?;
    if groups.is_empty() {
        return Ok(Vec::new());
    }
    let names: HashMap<CompanionId, String> = s
        .app
        .companions(a)
        .await?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    let zone = s.app.user_zone(a.user_id).await?;
    Ok(groups.iter().map(|g| group_dto(g, &names, zone)).collect())
}

fn group_input(b: &GameGroupInputDto) -> ApiResult<GameGroupInput> {
    Ok(GameGroupInput {
        name: b.name.clone(),
        members: b
            .members
            .iter()
            .map(|m| parse_id(m, CompanionId))
            .collect::<ApiResult<_>>()?,
    })
}

/// The group as the People tab shows it after a change.
async fn one_group(s: &ApiState, a: Actor, id: GameGroupId) -> ApiResult<GameGroupDto> {
    game_groups(s, a)
        .await?
        .into_iter()
        .find(|g| g.id == id.0.to_string())
        .ok_or_else(|| ApiError::from(app::AppError::NotFound("game group")))
}

/// # Errors
/// Bad name or members; people that are not the caller's (403).
pub async fn create_game_group(
    s: &ApiState,
    a: Actor,
    b: &GameGroupInputDto,
) -> ApiResult<GameGroupDto> {
    let g = s.app.create_game_group(a, group_input(b)?).await?;
    one_group(s, a, g.id).await
}

/// # Errors
/// As [`create_game_group`]; not the caller's group (404) or removed (409).
pub async fn update_game_group(
    s: &ApiState,
    a: Actor,
    id: &str,
    b: &GameGroupInputDto,
) -> ApiResult<GameGroupDto> {
    let id = parse_id(id, GameGroupId)?;
    s.app.update_game_group(a, id, group_input(b)?).await?;
    one_group(s, a, id).await
}

/// Remove a group; its plays keep it.
///
/// # Errors
/// Not the caller's group (404) or removed already (409).
pub async fn remove_game_group(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    Ok(s.app
        .remove_game_group(a, parse_id(id, GameGroupId)?)
        .await?)
}
