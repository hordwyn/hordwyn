//! DTOs shared between server and clients (web app, Tauri, scripts). Plain serde data,
//! wasm-safe, no domain internals. With feature `openapi` they also describe themselves for
//! the REST API's OpenAPI document.
#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct UserDto {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    /// `admin` or `member`.
    pub role: String,
    pub self_companion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ExternalIdDto {
    pub namespace: String,
    pub value: String,
}

/// Placeholder data for an image that is not loaded yet (D-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ImageDto {
    /// URL to use in `src` (our `/assets/...` path or the provider URL as a fallback).
    pub url: String,
    /// `srcset`-ready variants, empty when only the fallback exists.
    pub variants: Vec<ImageVariantDto>,
    /// Base64 thumbhash for an instant blurred placeholder.
    pub thumbhash: Option<String>,
    /// `#rrggbb`.
    pub dominant_color: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ImageVariantDto {
    pub url: String,
    pub width: u32,
    pub mime: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CatalogItemDto {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub cover: Option<ImageDto>,
    pub external_ids: Vec<ExternalIdDto>,
    /// Kind-specific attributes, validated against the kind's JSON Schema.
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub attributes: serde_json::Value,
    /// `stub`, `partial`, `full` or `failed` (D-6.1). Not `full` means a refresh may follow
    /// as an `entity.updated` event.
    pub state: String,
    /// Connector that supplied each field.
    pub provenance: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LibraryEntryDto {
    pub id: String,
    pub item_id: String,
    pub kind: String,
    pub status: String,
    /// 0..=100.
    pub rating: Option<u8>,
    pub notes: Option<String>,
    pub added_at: String,
}

/// A library row with its catalog item, for grids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LibraryItemDto {
    pub entry: LibraryEntryDto,
    pub item: CatalogItemDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ProgressEventDto {
    pub id: String,
    pub item_id: String,
    pub occurred_at: String,
    /// Progress as a tagged object, see `NewProgressDto`.
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub progress: serde_json::Value,
}

/// Body of `POST /items/{id}/progress`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NewProgressDto {
    /// Progress in the domain's tagged form, e.g. `{"type": "finished"}` or
    /// `{"type": "episode", "season": 1, "episode": 3}`.
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub progress: serde_json::Value,
    /// RFC 3339; defaults to now.
    pub occurred_at: Option<String>,
    /// Idempotency key (UUID).
    pub client_op_id: Option<String>,
    /// Companion ids this was done with ("watched with"); logged as a session.
    #[serde(default)]
    pub with: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CompanionDto {
    pub id: String,
    pub name: String,
    pub notes: Option<String>,
    pub is_self: bool,
    pub linked_user: Option<String>,
    pub merged_into: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SearchResponseDto {
    /// Correlates the `search.results` events that stream connector results.
    pub search_id: String,
    /// Local results, answered without waiting for any provider.
    pub items: Vec<CatalogItemDto>,
    /// Connectors queried in the background.
    pub pending: Vec<String>,
}

/// Payload of a `search.results` event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SearchResultsEventDto {
    pub search_id: String,
    pub connector: String,
    pub items: Vec<CatalogItemDto>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SignUpDto {
    pub username: String,
    pub display_name: String,
    pub password: String,
    pub email: Option<String>,
    pub invite: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SignInDto {
    pub username: String,
    pub password: String,
    pub totp: Option<String>,
}

/// Session token for non-browser clients (browsers get an `HttpOnly` cookie as well).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SessionDto {
    pub token: String,
    pub user: UserDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ErrorDto {
    /// Stable machine-readable code (`not_found`, `forbidden`, `totp_required`, ...).
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AttributionDto {
    pub connector: String,
    pub name: String,
    pub text: String,
    pub url: String,
    pub licence: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtos_round_trip_as_json() {
        let e = ErrorDto {
            code: "not_found".into(),
            message: "item not found".into(),
        };
        let back: ErrorDto = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }
}
