//! REST routes of WP4-6: goals and the year comparison (Stats page), game groups (People).
//! Handlers only call `crate::goals`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use contracts::{GameGroupDto, GameGroupInputDto, GoalDto, NewGoalDto, StatsCompareDto};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiResult;
use crate::goals;
use crate::{ApiState, Authed};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(list_goals, create_goal))
        .routes(routes!(update_goal, delete_goal))
        .routes(routes!(stats_compare))
        .routes(routes!(list_groups, create_group))
        .routes(routes!(update_group, delete_group))
}

/// The caller's yearly goals with their progress (private: never anyone else's).
#[utoipa::path(get, path = "/stats/goals", tag = "stats",
    responses((status = 200, body = Vec<GoalDto>)))]
async fn list_goals(State(s): State<ApiState>, Authed(a): Authed) -> ApiResult<Json<Vec<GoalDto>>> {
    Ok(Json(goals::goals(&s, a).await?))
}

/// Set a goal for a year, a group (or `all`) and a measure; one per such slot.
#[utoipa::path(post, path = "/stats/goals", tag = "stats", request_body = NewGoalDto,
    responses((status = 200, body = GoalDto), (status = 409, body = contracts::ErrorDto),
        (status = 422, body = contracts::ErrorDto)))]
async fn create_goal(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NewGoalDto>,
) -> ApiResult<Json<GoalDto>> {
    Ok(Json(goals::create_goal(&s, a, &b).await?))
}

#[utoipa::path(put, path = "/stats/goals/{id}", tag = "stats", request_body = NewGoalDto,
    params(("id" = String, Path)),
    responses((status = 200, body = GoalDto), (status = 404, body = contracts::ErrorDto)))]
async fn update_goal(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<NewGoalDto>,
) -> ApiResult<Json<GoalDto>> {
    Ok(Json(goals::update_goal(&s, a, &id, &b).await?))
}

#[utoipa::path(delete, path = "/stats/goals/{id}", tag = "stats",
    params(("id" = String, Path)), responses((status = 204)))]
async fn delete_goal(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    goals::delete_goal(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// This year so far against the same period last year, and the people seen most often.
#[utoipa::path(get, path = "/stats/compare", tag = "stats",
    responses((status = 200, body = StatsCompareDto)))]
async fn stats_compare(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<StatsCompareDto>> {
    Ok(Json(goals::stats_compare(&s, a).await?))
}

#[utoipa::path(get, path = "/game-groups", tag = "companions",
    responses((status = 200, body = Vec<GameGroupDto>)))]
async fn list_groups(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<Vec<GameGroupDto>>> {
    Ok(Json(goals::game_groups(&s, a).await?))
}

#[utoipa::path(post, path = "/game-groups", tag = "companions", request_body = GameGroupInputDto,
    responses((status = 200, body = GameGroupDto), (status = 403, body = contracts::ErrorDto),
        (status = 422, body = contracts::ErrorDto)))]
async fn create_group(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<GameGroupInputDto>,
) -> ApiResult<Json<GameGroupDto>> {
    Ok(Json(goals::create_game_group(&s, a, &b).await?))
}

/// Rename a group and replace its members.
#[utoipa::path(put, path = "/game-groups/{id}", tag = "companions", request_body = GameGroupInputDto,
    params(("id" = String, Path)),
    responses((status = 200, body = GameGroupDto), (status = 404, body = contracts::ErrorDto)))]
async fn update_group(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<GameGroupInputDto>,
) -> ApiResult<Json<GameGroupDto>> {
    Ok(Json(goals::update_game_group(&s, a, &id, &b).await?))
}

/// Remove a group: hidden from People and the play sheet, its plays keep it.
#[utoipa::path(delete, path = "/game-groups/{id}", tag = "companions",
    params(("id" = String, Path)), responses((status = 204)))]
async fn delete_group(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    goals::remove_game_group(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
