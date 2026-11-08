//! REST routes of the People and Lists pages (UX phase 3, WP3-6). Handlers only call
//! `crate::people`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use contracts::{
    AddedDto, CompanionDto, ListDto, ListPageDto, ListPatchDto, ListsOverviewDto, PeoplePageDto,
    PersonPageDto, PersonPatchDto, PositionDto, VisibilityDto,
};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiResult;
use crate::people;
use crate::{ApiState, Authed};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(people_page))
        .routes(routes!(person_page))
        .routes(routes!(update_person, delete_person))
        .routes(routes!(unmerge))
        .routes(routes!(lists_overview))
        .routes(routes!(list_page, patch_list, delete_list))
        .routes(routes!(remove_item))
        .routes(routes!(move_item))
        .routes(routes!(set_visibility))
        .routes(routes!(add_list_to_library))
        .routes(routes!(add_shared_to_library))
}

/// Requests for the caller, the people with their stats, the duplicate suggestions.
#[utoipa::path(get, path = "/people", tag = "companions",
    responses((status = 200, body = PeoplePageDto)))]
async fn people_page(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<PeoplePageDto>> {
    Ok(Json(people::people_page(&s, a).await?))
}

#[utoipa::path(get, path = "/people/{id}", tag = "companions", params(("id" = String, Path)),
    responses((status = 200, body = PersonPageDto), (status = 404, body = contracts::ErrorDto)))]
async fn person_page(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<PersonPageDto>> {
    Ok(Json(people::person_page(&s, a, &id).await?))
}

#[utoipa::path(patch, path = "/companions/{id}", tag = "companions", request_body = PersonPatchDto,
    params(("id" = String, Path)), responses((status = 200, body = CompanionDto)))]
async fn update_person(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<PersonPatchDto>,
) -> ApiResult<Json<CompanionDto>> {
    Ok(Json(people::update_person(&s, a, &id, b).await?))
}

/// Delete a person; one named in the history is removed from People and the history kept.
#[utoipa::path(delete, path = "/companions/{id}", tag = "companions",
    params(("id" = String, Path)), responses((status = 204)))]
async fn delete_person(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    people::delete_person(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Undo a merge from its record; the path names the entry that was merged away.
#[utoipa::path(post, path = "/companions/{id}/unmerge", tag = "sharing",
    params(("id" = String, Path)),
    responses((status = 200, body = CompanionDto), (status = 409, body = contracts::ErrorDto)))]
async fn unmerge(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<CompanionDto>> {
    Ok(Json(people::unmerge(&s, a, &id).await?))
}

/// Mine, shared with me (instance-visible lists of linked people), on this instance.
#[utoipa::path(get, path = "/lists/overview", tag = "lists",
    responses((status = 200, body = ListsOverviewDto)))]
async fn lists_overview(
    State(s): State<ApiState>,
    Authed(a): Authed,
) -> ApiResult<Json<ListsOverviewDto>> {
    Ok(Json(people::lists_overview(&s, a).await?))
}

#[utoipa::path(get, path = "/lists/{id}", tag = "lists", params(("id" = String, Path)),
    responses((status = 200, body = ListPageDto), (status = 403, body = contracts::ErrorDto)))]
async fn list_page(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<ListPageDto>> {
    Ok(Json(people::list_page(&s, a, &id).await?))
}

#[utoipa::path(patch, path = "/lists/{id}", tag = "lists", request_body = ListPatchDto,
    params(("id" = String, Path)), responses((status = 200, body = ListDto)))]
async fn patch_list(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<ListPatchDto>,
) -> ApiResult<Json<ListDto>> {
    Ok(Json(people::patch_list(&s, a, &id, b).await?))
}

#[utoipa::path(delete, path = "/lists/{id}", tag = "lists", params(("id" = String, Path)),
    responses((status = 204)))]
async fn delete_list(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    people::delete_list(&s, a, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/lists/{id}/items/{item_id}", tag = "lists",
    params(("id" = String, Path), ("item_id" = String, Path)),
    responses((status = 200, body = ListDto)))]
async fn remove_item(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path((id, item)): Path<(String, String)>,
) -> ApiResult<Json<ListDto>> {
    Ok(Json(people::remove_item(&s, a, &id, &item).await?))
}

/// Move an item to a new index (the list page's "Move up" / "Move down").
#[utoipa::path(put, path = "/lists/{id}/items/{item_id}/position", tag = "lists",
    request_body = PositionDto, params(("id" = String, Path), ("item_id" = String, Path)),
    responses((status = 200, body = ListDto)))]
async fn move_item(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path((id, item)): Path<(String, String)>,
    Json(b): Json<PositionDto>,
) -> ApiResult<Json<ListDto>> {
    Ok(Json(people::move_item(&s, a, &id, &item, b.to).await?))
}

/// Private, everyone on this instance, or anyone with the link; `new_link` replaces the link
/// and any other choice revokes it.
#[utoipa::path(put, path = "/lists/{id}/visibility", tag = "sharing", request_body = VisibilityDto,
    params(("id" = String, Path)), responses((status = 200, body = ListDto)))]
async fn set_visibility(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<VisibilityDto>,
) -> ApiResult<Json<ListDto>> {
    Ok(Json(people::set_visibility(&s, a, &id, b).await?))
}

/// "Add all to my library".
#[utoipa::path(post, path = "/lists/{id}/library", tag = "lists", params(("id" = String, Path)),
    responses((status = 200, body = AddedDto)))]
async fn add_list_to_library(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<AddedDto>> {
    Ok(Json(people::add_list_to_library(&s, a, &id).await?))
}

/// "Add all to my library" from the public page of a link-shared list.
#[utoipa::path(post, path = "/shared/lists/{token}/library", tag = "sharing",
    params(("token" = String, Path)),
    responses((status = 200, body = AddedDto), (status = 404, body = contracts::ErrorDto)))]
async fn add_shared_to_library(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(token): Path<String>,
) -> ApiResult<Json<AddedDto>> {
    Ok(Json(people::add_shared_to_library(&s, a, &token).await?))
}
