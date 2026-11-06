//! One imported library entry, the common shape every format parses into. It is also the
//! item record of our own export format (`generic`), versioned and published as JSON Schema.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Current version of the generic format.
pub const GENERIC_VERSION: u32 = 1;
/// `format` value of the generic format's header line.
pub const GENERIC_FORMAT: &str = "library-export";

/// First line of a generic JSON Lines file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Header {
    /// Always `library-export`.
    pub format: String,
    /// Format version, currently 1.
    pub version: u32,
}

/// A progress event of an entry (watched, read, played...).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    /// RFC 3339 timestamp or `YYYY-MM-DD`.
    pub date: String,
    /// Progress in the tagged form of the API, e.g. `{"type": "finished"}`; default finished.
    #[serde(default)]
    #[schemars(with = "Option<serde_json::Value>")]
    pub progress: Option<serde_json::Value>,
}

/// One library entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportRow {
    /// Stable id of the row in the source; re-importing the same source id updates instead
    /// of duplicating.
    pub source_id: String,
    /// Media kind id (`movie`, `book`, ...).
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub year: Option<i32>,
    /// Provider ids, e.g. `{"tmdb": "550", "imdb": "tt0137523"}`; used to match the catalog.
    #[serde(default)]
    pub external_ids: std::collections::BTreeMap<String, String>,
    /// Status id of the kind (`planned`, `in_progress`, `completed`, ...).
    #[serde(default)]
    pub status: Option<String>,
    /// 0..=100.
    #[serde(default)]
    pub rating: Option<u8>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

/// Why a row was not imported (per-row report).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowError {
    /// 1-based line or record number in the source file.
    pub line: u64,
    pub source_id: Option<String>,
    pub message: String,
}

/// JSON Schema of an [`ImportRow`] (published in the docs).
#[must_use]
pub fn row_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ImportRow)).unwrap_or_default()
}
