//! Plain data exchanged with connectors (WIT-compatible: records, enums, strings, lists).

use app::ports::HttpError;
use domain::catalog::ExternalId;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Search,
    Details,
    People,
    Groups,
    Trending,
    Translations,
    Images,
    /// Per-episode release dates of one season (WP4-7).
    Episodes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimit {
    pub per_second: u32,
    pub burst: u32,
    pub max_concurrency: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Auth {
    None,
    /// Secret read from the named environment variable / config key.
    ApiKey {
        env: String,
    },
    Bearer {
        env: String,
    },
}

/// Shown on detail pages and on the "Data sources" page (D-5.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribution {
    pub text: String,
    pub url: String,
    pub licence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorInfo {
    pub id: String,
    pub name: String,
    /// Kind ids served, e.g. `movie`.
    pub kinds: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub rate_limit: RateLimit,
    pub auth: Auth,
    pub attribution: Attribution,
    /// `host` or `host:port` values the connector may contact.
    pub hosts: Vec<String>,
    /// Default HTTP cache TTL for responses without `Cache-Control: max-age`.
    pub cache_ttl_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub kind: String,
    pub query: String,
    pub language: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailsRequest {
    pub kind: String,
    /// An id in this connector's namespace.
    pub id: String,
    pub language: Option<String>,
}

/// The episodes of one season of an item (WP4-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodesRequest {
    pub kind: String,
    /// An id in this connector's namespace.
    pub id: String,
    pub season: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageKind {
    Cover,
    Backdrop,
    Person,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    pub kind: ImageKind,
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// What a connector knows about one item. `complete` is false for search results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub connector: String,
    pub kind: String,
    pub external_ids: Vec<ExternalId>,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    /// Kind attributes (validated by the kind registry before storage).
    pub attributes: serde_json::Value,
    pub images: Vec<ImageRef>,
    pub complete: bool,
    /// How well known the item is at this source, when its answer says so (see
    /// `domain::catalog::CatalogItem::popularity`).
    #[serde(default)]
    pub popularity: Option<f64>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectorError {
    #[error(transparent)]
    Http(#[from] HttpError),
    #[error("upstream answered {status}")]
    Status { status: u16 },
    /// The provider rejected the requesting member's own credential (their `TheTVDB` PIN);
    /// the instance key is not affected (UX2-S2).
    #[error("the provider rejected your own credential for this source (HTTP {status})")]
    UserCredentialRejected { status: u16 },
    #[error("not found")]
    NotFound,
    #[error("missing credentials: {0}")]
    MissingCredentials(String),
    #[error("cannot parse upstream response: {0}")]
    Parse(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
}
