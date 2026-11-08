//! The imports page (design.md 6.11, R-35): an import as `GET /imports` and
//! `GET /imports/{id}` send it (the JSON of `importers::ImportRecord`; every field defaults,
//! so the page reads older records too).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One import: state, counts, preview, errors and the rows waiting for a manual match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportDto {
    pub id: String,
    /// `generic-jsonl`, `generic-csv`, `legacy-json`, `trakt-json`.
    pub format: String,
    pub dry_run: bool,
    /// `queued`, `running`, `done`, `failed`, `cancelled`.
    pub state: String,
    /// Rows handled so far.
    pub cursor: u64,
    /// Rows in the file (0 until it is read).
    pub total: u64,
    pub imported: u64,
    pub matched: u64,
    pub errors: Vec<ImportErrorDto>,
    pub unmatched: Vec<UnmatchedDto>,
    pub preview: Option<PreviewDto>,
    /// The import whose file this one runs again.
    pub source_import: Option<String>,
    /// RFC 3339.
    pub created_at: String,
    pub updated_at: String,
}

impl ImportDto {
    /// Queued or running: the runner is still at it.
    #[must_use]
    pub fn active(&self) -> bool {
        matches!(self.state.as_str(), "queued" | "running")
    }
}

/// Why a row was not imported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportErrorDto {
    pub line: u64,
    pub source_id: Option<String>,
    /// The importer's reason, in English (not translated).
    pub message: String,
}

/// A row without a catalog match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnmatchedDto {
    pub line: u64,
    pub row: UnmatchedRowDto,
}

/// The parts of an unmatched row the page shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnmatchedRowDto {
    pub source_id: String,
    pub kind: String,
    pub title: String,
    pub year: Option<i32>,
}

/// What a file holds before anything is written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PreviewDto {
    pub kinds: BTreeMap<String, u64>,
    pub events: u64,
    pub samples: Vec<SampleDto>,
    pub mapping: Vec<MappingDto>,
    pub lost: Vec<LostDto>,
    pub lists: Vec<String>,
    pub unmatched: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SampleDto {
    pub title: String,
    pub kind: String,
    pub status: Option<String>,
    pub events: u64,
    pub lists: Vec<String>,
    pub unmatched: bool,
}

/// A source field (as named in the file) and the target code it fills.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MappingDto {
    pub from: String,
    pub to: String,
}

/// Something the format does not keep, by code, and how often.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LostDto {
    pub code: String,
    pub count: u64,
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_record_json() {
        let v = serde_json::json!({
            "id": "0190", "user_id": "u", "format": "legacy-json", "source_key": "k",
            "dry_run": true, "state": "done", "cursor": 21, "imported": 21, "matched": 0,
            "errors": [{"line": 3, "source_id": null, "message": "bad"}],
            "created_at": "2026-11-08T10:00:00Z", "updated_at": "2026-11-08T10:00:01Z",
            "total": 21,
            "unmatched": [{"line": 4, "row": {"source_id": "s", "kind": "movie",
                "title": "T", "external_ids": {}, "history": []}}],
            "preview": {"kinds": {"movie": 8}, "events": 82, "samples": [], "mapping": [],
                "lost": [{"code": "watched_on", "count": 15}], "lists": ["A"], "unmatched": 0},
            "source_import": null
        });
        let d: super::ImportDto = serde_json::from_value(v).unwrap();
        assert_eq!(d.total, 21);
        assert_eq!(d.unmatched[0].row.title, "T");
        assert_eq!(d.preview.unwrap().lost[0].count, 15);
        assert!(!super::ImportDto::default().active());
    }
}
