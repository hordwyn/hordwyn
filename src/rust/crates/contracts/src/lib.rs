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
    /// Privacy: linked participants see the sessions this user logs.
    #[serde(default)]
    pub share_sessions: bool,
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

/// Everything the item page shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ItemPageDto {
    pub item: CatalogItemDto,
    /// The viewer's library entry, if the item is in their library.
    pub entry: Option<LibraryEntryDto>,
    pub progress: Vec<ProgressEventDto>,
    /// Attribution of the connectors that supplied data for this item.
    pub sources: Vec<AttributionDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ListDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// `private`, `instance` or `link`.
    pub visibility: String,
    pub item_ids: Vec<String>,
}

// ---------------------------------------------------------------- sharing (D-9)

/// A link-shared list as anyone with the link sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SharedListDto {
    pub name: String,
    pub description: Option<String>,
    pub owner_name: String,
    pub items: Vec<CatalogItemDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ShareLinkDto {
    /// Path of the shared page; sharing again replaces (revokes) the previous link.
    pub path: String,
    pub token: String,
}

/// "This companion is you": a request to link a companion of another user to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LinkRequestDto {
    pub id: String,
    pub companion_id: String,
    pub companion_name: String,
    /// Display name of the user who asks.
    pub from_name: String,
}

/// Promotion of a companion to an account: the single-use invite link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct InviteDto {
    pub id: String,
    /// Shown once; only its hash is stored.
    pub token: String,
    /// Sign-up page with the token filled in.
    pub path: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ParticipantDto {
    pub companion_id: String,
    pub name: String,
    /// The participant stands for the caller.
    pub is_me: bool,
    pub score: Option<f64>,
    pub winner: bool,
}

/// A logged session (play, "watched with").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LoggedSessionDto {
    pub id: String,
    pub item_id: String,
    pub item_title: String,
    pub owner_name: String,
    /// Logged by the caller (otherwise shared with them, read-only).
    pub mine: bool,
    pub started_at: String,
    pub participants: Vec<ParticipantDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PrivacyDto {
    /// Linked participants see the sessions you log.
    pub share_sessions: bool,
}

// ---------------------------------------------------------------- sync (D-12)

/// A mutation recorded by an offline client. Mutable fields carry the client's HLC stamp
/// (`{wall_ms:016x}-{counter:08x}-{node:08x}`); the server keeps the newest stamp per field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncCommandDto {
    Add {
        item_id: String,
    },
    SetStatus {
        item_id: String,
        status: String,
        hlc: String,
    },
    Rate {
        item_id: String,
        rating: Option<u8>,
        hlc: String,
    },
    SetNotes {
        item_id: String,
        notes: Option<String>,
        hlc: String,
    },
    /// Append-only; idempotent by `op_id`.
    LogProgress {
        item_id: String,
        #[cfg_attr(feature = "openapi", schema(value_type = Object))]
        progress: serde_json::Value,
        occurred_at: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SyncCommandEnvelope {
    /// Client-generated id (UUID); replaying a command with the same id changes nothing.
    pub op_id: String,
    pub command: SyncCommandDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SyncBatchDto {
    pub commands: Vec<SyncCommandEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SyncResultDto {
    pub op_id: String,
    pub ok: bool,
    pub error: Option<ErrorDto>,
}

/// A library entry with the stamps a client needs to merge it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SyncEntryDto {
    pub id: String,
    pub item_id: String,
    pub kind: String,
    pub status: String,
    pub status_hlc: String,
    pub rating: Option<u8>,
    pub rating_hlc: String,
    pub notes: Option<String>,
    pub notes_hlc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SyncChangeDto {
    pub seq: i64,
    pub entity: String,
    pub id: String,
    /// `upsert` or `delete`.
    pub op: String,
    /// Current state for library entries (absent after a delete).
    pub entry: Option<SyncEntryDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SyncChangesDto {
    /// Pass back as `cursor` to continue.
    pub cursor: i64,
    pub changes: Vec<SyncChangeDto>,
    /// More changes are waiting.
    pub more: bool,
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

// ---------------------------------------------------------------- notifications (D-17)

/// An event of the notification catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct EventTypeDto {
    /// Stable id, e.g. `release.episode`; subscriptions also accept `release.*` and `*`.
    pub id: String,
    /// `user` or `admin`.
    pub audience: String,
    /// Template arguments.
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct InboxItemDto {
    pub id: String,
    pub event: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
    pub created_at: String,
    pub read: bool,
}

/// A notification channel. `config` is tagged by `kind` (`email`, `ntfy`, `gotify`,
/// `discord`, `matrix`, `apprise`, `webhook`, `web_push`); secrets in it are replaced by
/// `********` in answers and kept when sent back unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ChannelDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub config: serde_json::Value,
    /// Only in the answer that created a webhook without a secret: the generated signing
    /// secret (Standard Webhooks `whsec_...`). It is not shown again.
    pub secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NewChannelDto {
    pub name: String,
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub config: serde_json::Value,
    #[serde(default = "yes")]
    pub enabled: bool,
}

const fn yes() -> bool {
    true
}

fn default_targets() -> serde_json::Value {
    serde_json::json!({ "type": "all" })
}

fn default_mode() -> serde_json::Value {
    serde_json::json!({ "mode": "immediate" })
}

fn utc() -> String {
    "UTC".into()
}

/// A subscription: event pattern + filter + channels + delivery mode + quiet hours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SubscriptionDto {
    /// Omit to create.
    #[serde(default)]
    pub id: Option<String>,
    /// Event id, `group.*` or `*`.
    pub event: String,
    /// `{"kinds": [...], "lists": [...], "items": [...], "companions": [...]}`; empty = any.
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub filter: serde_json::Value,
    /// `{"type": "all"}` or `{"type": "only", "ids": [channel ids]}`.
    #[serde(default = "default_targets")]
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub targets: serde_json::Value,
    /// `{"mode": "immediate"}` or `{"mode": "digest", "cron": "0 8 * * *"}`.
    #[serde(default = "default_mode")]
    #[cfg_attr(feature = "openapi", schema(value_type = Object))]
    pub mode: serde_json::Value,
    /// `{"start": "22:00:00", "end": "07:00:00"}` in `timezone`.
    #[serde(default)]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub quiet_hours: Option<serde_json::Value>,
    #[serde(default = "utc")]
    pub timezone: String,
    #[serde(default)]
    pub rate_limit_per_hour: Option<u32>,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Instance defaults only: users cannot override it.
    #[serde(default)]
    pub locked: bool,
    /// Set by the server: an admin's instance default.
    #[serde(default)]
    pub instance_default: bool,
}

/// One entry of the delivery log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct DeliveryDto {
    pub id: String,
    pub channel_id: String,
    pub event: String,
    pub title: String,
    /// `queued`, `digest`, `sent`, `failed` or `suppressed`.
    pub state: String,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub due_at: String,
    pub created_at: String,
    pub sent_at: Option<String>,
}

/// Admin override of a notification text (Fluent message id, e.g. `release-episode-title`
/// or `release-episode-email-body`); `text: null` removes the override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct TemplateOverrideDto {
    pub locale: String,
    pub key: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct WebPushKeyDto {
    /// VAPID public key for `pushManager.subscribe`; `null` when web push is not set up.
    pub public_key: Option<String>,
}

/// Everything the notifications page shows, in one answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NotificationPageDto {
    pub inbox: Vec<InboxItemDto>,
    pub channels: Vec<ChannelDto>,
    pub subscriptions: Vec<SubscriptionDto>,
    pub events: Vec<EventTypeDto>,
    pub deliveries: Vec<DeliveryDto>,
}
