//! REST routes of v1-manual: an item entered by hand and its details. Handlers only call
//! `crate::manual`.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use contracts::{CatalogItemDto, ItemDetailsDto, NewItemDto};
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::error::ApiResult;
use crate::manual;
use crate::{ApiState, Authed};

pub(crate) fn routes() -> OpenApiRouter<ApiState> {
    OpenApiRouter::new()
        .routes(routes!(create_item))
        .routes(routes!(items_titled))
        .routes(routes!(edit_item))
        .routes(routes!(reset_item))
}

/// Add an item no source knows, of any enabled kind. It has no external ids, is never
/// refreshed, and its title, year and cover address are marked as set by a person. The
/// cover address is loaded by the browsers that show the item, never by the server; on a
/// hosted instance only an admin may set one. The item is not in anybody's library yet
/// (`POST /library`). 409 when items of the kind with this title exist, unless the body
/// says `allow_duplicate`.
#[utoipa::path(post, path = "/items", tag = "catalog", request_body = NewItemDto,
    responses((status = 201, body = CatalogItemDto), (status = 403, body = contracts::ErrorDto),
        (status = 409, body = contracts::ErrorDto),
        (status = 422, body = contracts::ErrorDto)))]
async fn create_item(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<NewItemDto>,
) -> ApiResult<(StatusCode, Json<CatalogItemDto>)> {
    Ok((
        StatusCode::CREATED,
        Json(manual::create_item(&s, a, b).await?),
    ))
}

#[derive(Deserialize, IntoParams)]
struct TitledParams {
    /// Kind id.
    kind: String,
    /// The title to look for; case and surrounding blanks do not matter.
    title: String,
}

/// The items of a kind that already have this title (at most five), to be shown before a
/// second one is entered by hand.
#[utoipa::path(get, path = "/items/titled", tag = "catalog", params(TitledParams),
    responses((status = 200, body = Vec<CatalogItemDto>)))]
async fn items_titled(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Query(p): Query<TitledParams>,
) -> ApiResult<Json<Vec<CatalogItemDto>>> {
    Ok(Json(manual::items_titled(&s, a, &p.kind, &p.title).await?))
}

/// Correct title, year and cover address of an item no source of this instance answers for
/// (entered by hand, a title-only stub of an import). Send the three fields together; a
/// field whose value changes is marked as set by a person. Every member on a home
/// instance, admins only on a hosted one. An item a source answers for takes title and year
/// (kept by every refresh, #16); 409 when such a request changes its cover address.
#[utoipa::path(patch, path = "/items/{id}", tag = "catalog", request_body = ItemDetailsDto,
    params(("id" = String, Path, description = "catalog item id")),
    responses((status = 200, body = CatalogItemDto), (status = 403, body = contracts::ErrorDto),
        (status = 404, body = contracts::ErrorDto), (status = 409, body = contracts::ErrorDto),
        (status = 422, body = contracts::ErrorDto)))]
async fn edit_item(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
    Json(b): Json<ItemDetailsDto>,
) -> ApiResult<Json<CatalogItemDto>> {
    Ok(Json(manual::edit_item(&s, a, &id, b).await?))
}

/// Give the title and year a person corrected on an item a source answers for back to the
/// source (#16): their mark as set by a person is removed and a refresh is queued, so the
/// source's values come back within moments. Every member on a home instance, admins only
/// on a hosted one; 409 when no source answers for the item.
#[utoipa::path(delete, path = "/items/{id}/corrections", tag = "catalog",
    params(("id" = String, Path, description = "catalog item id")),
    responses((status = 200, body = CatalogItemDto), (status = 403, body = contracts::ErrorDto),
        (status = 404, body = contracts::ErrorDto), (status = 409, body = contracts::ErrorDto)))]
async fn reset_item(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(id): Path<String>,
) -> ApiResult<Json<CatalogItemDto>> {
    Ok(Json(manual::reset_item(&s, a, &id).await?))
}
