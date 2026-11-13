//! Items entered and corrected by hand (v1-manual, v1-plan.md 4a): the operations behind
//! `POST /items`, `PATCH /items/{id}`, `GET /items/titled` and the web app's dialog.

use app::{AppError, ItemDetails};
use catalog_core::refresh::source_known;
use contracts::{CatalogItemDto, ItemDetailsDto, NewItemDto};
use domain::DomainError;
use domain::catalog::CatalogItem;
use domain::ids::ItemId;
use domain::kind::KindId;
use domain::user::Actor;

use crate::error::ApiResult;
use crate::ops::{may_edit_items, parse_id};
use crate::{ApiError, ApiState, dto};

/// A refused value names its field (`title`, `year`, `cover_url`, `kind`) in the answer.
fn with_field(e: AppError) -> ApiError {
    let field = match &e {
        AppError::Domain(DomainError::Invalid { field, .. }) => Some(*field),
        _ => None,
    };
    let e = ApiError::from(e);
    match field {
        Some(f) => e.with_field(f),
        None => e,
    }
}

/// A person adds an item no source knows. Every signed-in member may; a cover address
/// (which the browsers of other members load) only where the member may change shared
/// items: everybody on a home instance, admins on a hosted one.
///
/// # Errors
/// `invalid` with the field for a kind that is not enabled, a blank or too long title, a
/// year out of range or a cover address that is not a plain `https` one; `forbidden` for a
/// member's cover address on a hosted instance; `conflict` on the field `title`, naming the
/// ids, when items of the kind with this title exist and `allow_duplicate` is not set.
pub async fn create_item(s: &ApiState, a: Actor, new: NewItemDto) -> ApiResult<CatalogItemDto> {
    let kind = KindId::new(new.kind.trim()).map_err(|e| {
        with_field(AppError::Domain(DomainError::invalid(
            "kind",
            e.to_string(),
        )))
    })?;
    let item = s
        .app
        .add_manual_item(
            kind,
            ItemDetails {
                title: new.title,
                year: new.year,
                cover_url: new.cover_url,
            },
            may_edit_items(s, a),
            new.allow_duplicate,
        )
        .await
        .map_err(|e| match e {
            // the only conflict here: items of that title exist (the dialog lists them)
            AppError::Conflict(_) => ApiError::from(e).with_field("title"),
            e => with_field(e),
        })?;
    Ok(dto::item(s, &item).await)
}

/// "Edit details": title, year and cover address of an item no source of this instance
/// answers for.
///
/// # Errors
/// `forbidden` for a member of a hosted instance; `not_found`; `conflict` when a source
/// answers for the item; `invalid` with the field as for [`create_item`].
pub async fn edit_item(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    details: ItemDetailsDto,
) -> ApiResult<CatalogItemDto> {
    let id = parse_id(item_id, ItemId)?;
    if !may_edit_items(s, a) {
        return Err(AppError::Forbidden.into());
    }
    let sourced = |i: &CatalogItem| source_known(&s.registry, i);
    let item = s
        .app
        .edit_item_details(
            id,
            ItemDetails {
                title: details.title,
                year: details.year,
                cover_url: details.cover_url,
            },
            // who may edit the shared item may also choose its picture address
            true,
            &sourced,
        )
        .await
        .map_err(with_field)?;
    Ok(dto::item(s, &item).await)
}

/// The items of `kind` with this title (case and surrounding blanks apart), at most five:
/// what the dialog lists as "Already here" before a second one is entered.
///
/// # Errors
/// Storage errors. An unknown kind or a blank title has no items.
pub async fn items_titled(
    s: &ApiState,
    _a: Actor,
    kind: &str,
    title: &str,
) -> ApiResult<Vec<CatalogItemDto>> {
    let Ok(kind) = KindId::new(kind.trim()) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for item in s.app.items_titled(&kind, title).await? {
        out.push(dto::item(s, &item).await);
    }
    Ok(out)
}
