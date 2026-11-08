//! Importers (D-16): one module and cargo feature per format, a common streaming runner.
//!
//! Every import runs as a job in the `user_import` lane. It is
//! - **idempotent**: rows are keyed by (user, format, source id); library writes are upserts,
//!   ratings/status/notes are last-writer-wins, and progress events carry a client op id
//!   derived from the row, so running an import twice creates nothing new;
//! - **resumable**: the runner stores a cursor every few rows; after a crash the job is
//!   claimed again and continues from the cursor (re-processing a few rows is harmless);
//! - **non-blocking for catalog matching**: rows are matched by external ids; new items are
//!   created as stubs and refreshed by queued `catalog.refresh` jobs, never inline;
//! - **observable**: `import.progress` events over SSE, a preview (counts per kind, sample
//!   rows, what maps to what and what is lost, R-35), a per-row error report and a
//!   persistent list of unmatched rows that can be matched by hand;
//! - **cancellable**: a cancel request is checked at every checkpoint.
#![deny(unsafe_code)]

pub mod cell;
#[cfg(feature = "generic")]
pub mod generic;
pub mod legacy;
pub mod pg;
pub mod row;
mod runner;
#[cfg(feature = "trakt-import")]
pub mod trakt;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Mutex;

use app::AppError;
use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use row::{ImportRow, RowError};
pub use runner::Importer;

/// Job kind that runs an import.
pub const IMPORT_JOB: &str = "import.run";
/// Rows between cursor saves, progress events and cancel checks.
pub const CHECKPOINT: u64 = 25;
/// Sample rows in a preview.
pub const SAMPLES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportState {
    Queued,
    Running,
    Done,
    Failed,
    /// Stopped at a checkpoint on the user's request.
    Cancelled,
}

impl ImportState {
    /// Done, failed or cancelled: the runner will not touch it again.
    #[must_use]
    pub const fn is_final(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

/// Something of the source that is not kept, with how often it occurs (R-35).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lost {
    /// Stable code; the page names it (`imports-lost-<code>`).
    pub code: String,
    pub count: u64,
}

impl Lost {
    #[must_use]
    pub fn new(code: impl Into<String>, count: u64) -> Self {
        Self {
            code: code.into(),
            count,
        }
    }
}

/// One line of "what maps to what": a source field and the target it fills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    /// The source's own field names, as in the file (not translated).
    pub from: String,
    /// Target code (`kind`, `external_ids`, `title`, `history`, `status`, `rating`, `notes`,
    /// `lists`, `same`); the page names it (`imports-target-<code>`).
    pub to: String,
}

impl Mapping {
    #[must_use]
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

/// One sample row of a preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    pub title: String,
    pub kind: String,
    pub status: Option<String>,
    /// History entries of the row.
    pub events: u64,
    pub lists: Vec<String>,
    /// No usable external id: the row will be unmatched.
    pub unmatched: bool,
}

/// What a file holds, before anything is written (design.md 6.11, R-35).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preview {
    /// Rows per kind.
    pub kinds: BTreeMap<String, u64>,
    /// History entries in all rows.
    pub events: u64,
    /// Up to [`SAMPLES`] rows, one of each kind first.
    pub samples: Vec<Sample>,
    pub mapping: Vec<Mapping>,
    pub lost: Vec<Lost>,
    /// Lists the rows belong to (created when missing), sorted.
    pub lists: Vec<String>,
    /// Rows without a usable external id.
    pub unmatched: u64,
}

/// A row that found no catalog item (no usable external id). Kept with the import until it
/// is matched by hand, which replays it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unmatched {
    pub line: u64,
    pub row: ImportRow,
}

/// One import and its report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub format: String,
    /// Blob key of the uploaded file.
    pub source_key: String,
    pub dry_run: bool,
    pub state: ImportState,
    /// Rows already handled (resume point).
    pub cursor: u64,
    pub imported: u64,
    /// Rows matched to an existing catalog item by external id.
    pub matched: u64,
    pub errors: Vec<RowError>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Rows in the file (known once it is parsed).
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub unmatched: Vec<Unmatched>,
    #[serde(default)]
    pub preview: Option<Preview>,
    /// The import whose file this one runs again (a dry run's real run, a retry).
    #[serde(default)]
    pub source_import: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("unknown import format `{0}`")]
    UnknownFormat(String),
    #[error("unknown import {0}")]
    Unknown(Uuid),
    #[error("forbidden")]
    Forbidden,
    #[error(transparent)]
    App(#[from] AppError),
    #[error("import storage: {0}")]
    Port(#[from] PortError),
    #[error("the uploaded file is missing")]
    MissingFile,
    /// The import is in the wrong state for the request (`code` for the API).
    #[error("{message}")]
    Conflict { code: &'static str, message: String },
    /// No unmatched row with this source id.
    #[error("no unmatched row `{0}`")]
    UnknownRow(String),
}

#[async_trait]
pub trait ImportStore: Send + Sync {
    async fn insert(&self, r: &ImportRecord) -> Result<(), PortError>;
    async fn get(&self, id: Uuid) -> Result<Option<ImportRecord>, PortError>;
    async fn save(&self, r: &ImportRecord) -> Result<(), PortError>;
    /// The user's imports, newest first.
    async fn list(&self, user: Uuid, limit: u32) -> Result<Vec<ImportRecord>, PortError>;
    /// Ask the runner to stop at its next checkpoint (kept apart from the record, which
    /// the runner overwrites).
    async fn request_cancel(&self, id: Uuid) -> Result<(), PortError>;
    async fn cancel_requested(&self, id: Uuid) -> Result<bool, PortError>;
    async fn item_for(
        &self,
        user: Uuid,
        format: &str,
        source_id: &str,
    ) -> Result<Option<Uuid>, PortError>;
    async fn set_item_for(
        &self,
        user: Uuid,
        format: &str,
        source_id: &str,
        item: Uuid,
    ) -> Result<(), PortError>;
}

#[derive(Default)]
pub struct MemImportStore {
    records: Mutex<HashMap<Uuid, ImportRecord>>,
    rows: Mutex<HashMap<(Uuid, String, String), Uuid>>,
    cancels: Mutex<HashSet<Uuid>>,
}

#[async_trait]
impl ImportStore for MemImportStore {
    async fn insert(&self, r: &ImportRecord) -> Result<(), PortError> {
        self.records
            .lock()
            .expect("imports")
            .insert(r.id, r.clone());
        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<ImportRecord>, PortError> {
        Ok(self.records.lock().expect("imports").get(&id).cloned())
    }

    async fn save(&self, r: &ImportRecord) -> Result<(), PortError> {
        self.insert(r).await
    }

    async fn list(&self, user: Uuid, limit: u32) -> Result<Vec<ImportRecord>, PortError> {
        let mut out: Vec<ImportRecord> = self
            .records
            .lock()
            .expect("imports")
            .values()
            .filter(|r| r.user_id == user)
            .cloned()
            .collect();
        out.sort_by_key(|r| std::cmp::Reverse(r.id));
        out.truncate(limit as usize);
        Ok(out)
    }

    async fn request_cancel(&self, id: Uuid) -> Result<(), PortError> {
        self.cancels.lock().expect("cancels").insert(id);
        Ok(())
    }

    async fn cancel_requested(&self, id: Uuid) -> Result<bool, PortError> {
        Ok(self.cancels.lock().expect("cancels").contains(&id))
    }

    async fn item_for(
        &self,
        user: Uuid,
        format: &str,
        source_id: &str,
    ) -> Result<Option<Uuid>, PortError> {
        Ok(self
            .rows
            .lock()
            .expect("rows")
            .get(&(user, format.to_string(), source_id.to_string()))
            .copied())
    }

    async fn set_item_for(
        &self,
        user: Uuid,
        format: &str,
        source_id: &str,
        item: Uuid,
    ) -> Result<(), PortError> {
        self.rows
            .lock()
            .expect("rows")
            .insert((user, format.to_string(), source_id.to_string()), item);
        Ok(())
    }
}

/// Formats compiled into this build.
#[must_use]
pub fn formats() -> Vec<&'static str> {
    let mut f = vec![];
    #[cfg(feature = "generic")]
    f.extend(["generic-jsonl", "generic-csv"]);
    f.push(legacy::FORMAT);
    #[cfg(feature = "trakt-import")]
    f.push(trakt::FORMAT);
    f
}

/// Formats whose rows need an external id to be matched: rows without one are kept as
/// unmatched instead of becoming title-only stubs (our own generic format keeps the stub,
/// since items a user created have no provider id).
#[must_use]
pub fn needs_external_id(format: &str) -> bool {
    matches!(format, legacy::FORMAT | "trakt-json")
}

/// The rows of a file and what the format does not keep.
pub struct Parsed {
    pub rows: Vec<Result<ImportRow, RowError>>,
    pub lost: Vec<Lost>,
}

impl Parsed {
    /// A file that could not be read at all: one error.
    pub(crate) fn error(message: String) -> Self {
        Self {
            rows: vec![Err(RowError {
                line: 1,
                source_id: None,
                message,
            })],
            lost: Vec::new(),
        }
    }
}

/// Non-zero counts as [`Lost`] entries, by code.
pub(crate) fn lost_list(counts: BTreeMap<&'static str, u64>) -> Vec<Lost> {
    counts
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .map(|(code, n)| Lost::new(code, n))
        .collect()
}

fn parse(format: &str, bytes: &[u8]) -> Result<Parsed, ImportError> {
    let plain = |rows: Box<dyn Iterator<Item = Result<ImportRow, RowError>>>| Parsed {
        rows: rows.collect(),
        lost: Vec::new(),
    };
    match format {
        #[cfg(feature = "generic")]
        "generic-jsonl" => Ok(plain(generic::jsonl(std::io::Cursor::new(bytes.to_vec())))),
        #[cfg(feature = "generic")]
        "generic-csv" => Ok(plain(generic::csv(std::io::Cursor::new(bytes.to_vec())))),
        legacy::FORMAT => Ok(legacy::parse(bytes)),
        #[cfg(feature = "trakt-import")]
        trakt::FORMAT => Ok(trakt::parse(bytes)),
        other => {
            let _ = plain;
            Err(ImportError::UnknownFormat(other.to_string()))
        }
    }
}

/// What maps to what for `format`.
#[must_use]
pub fn mapping(format: &str) -> Vec<Mapping> {
    match format {
        legacy::FORMAT => legacy::mapping(),
        #[cfg(feature = "trakt-import")]
        trakt::FORMAT => trakt::mapping(),
        _ => vec![Mapping::new(
            "source_id, kind, title, year, external_ids, status, rating, notes, history, lists",
            "same",
        )],
    }
}

/// The preview of parsed rows (no database access).
#[must_use]
pub fn preview(format: &str, parsed: &Parsed) -> Preview {
    let mut p = Preview {
        mapping: mapping(format),
        lost: parsed.lost.clone(),
        ..Preview::default()
    };
    let mut lists = BTreeSet::new();
    let mut later = Vec::new();
    let strict = needs_external_id(format);
    for row in parsed.rows.iter().flatten() {
        let first_of_kind = !p.kinds.contains_key(&row.kind);
        *p.kinds.entry(row.kind.clone()).or_insert(0) += 1;
        p.events += row.history.len() as u64;
        lists.extend(row.lists.iter().cloned());
        let unmatched = strict && row.external_ids.is_empty();
        p.unmatched += u64::from(unmatched);
        let sample = Sample {
            title: row.title.clone(),
            kind: row.kind.clone(),
            status: row.status.clone(),
            events: row.history.len() as u64,
            lists: row.lists.clone(),
            unmatched,
        };
        if first_of_kind && p.samples.len() < SAMPLES {
            p.samples.push(sample);
        } else if later.len() < SAMPLES {
            later.push(sample);
        }
    }
    let room = SAMPLES.saturating_sub(p.samples.len());
    p.samples.extend(later.into_iter().take(room));
    p.lists = lists.into_iter().collect();
    p
}

/// Deterministic client op id for an event of a row: re-imports are no-ops (D-12). Keyed by
/// the source's event id when it has one, else by the event's position in the row.
fn op_id(user: Uuid, format: &str, source_id: &str, event: &row::HistoryEntry, i: usize) -> Uuid {
    let key = match &event.event_id {
        Some(e) => format!("import:{user}:{format}:{source_id}:event:{e}"),
        None => format!("import:{user}:{format}:{source_id}:{i}"),
    };
    let h = blake3::hash(key.as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&h.as_bytes()[..16]);
    uuid::Builder::from_random_bytes(b).into_uuid()
}

fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(12, 0, 0))
                .map(|d| d.and_utc())
        })
}

#[cfg(test)]
mod tests {
    use super::row::HistoryEntry;

    fn at(event_id: Option<&str>) -> HistoryEntry {
        HistoryEntry {
            date: "2024-01-01".into(),
            progress: None,
            event_id: event_id.map(String::from),
        }
    }

    #[test]
    fn op_ids_are_stable_per_row_and_event() {
        let u = uuid::Uuid::nil();
        let e = at(None);
        assert_eq!(
            super::op_id(u, "f", "1", &e, 0),
            super::op_id(u, "f", "1", &e, 0)
        );
        assert_ne!(
            super::op_id(u, "f", "1", &e, 0),
            super::op_id(u, "f", "1", &e, 1)
        );
        assert_ne!(
            super::op_id(u, "f", "1", &e, 0),
            super::op_id(u, "f", "2", &e, 0)
        );
        // with a source event id the position does not matter
        let k = at(Some("77"));
        assert_eq!(
            super::op_id(u, "f", "1", &k, 0),
            super::op_id(u, "f", "1", &k, 5)
        );
    }

    #[test]
    fn dates_parse_in_both_forms() {
        assert!(super::parse_date("2024-05-01").is_some());
        assert!(super::parse_date("2024-05-01T10:00:00Z").is_some());
        assert!(super::parse_date("yesterday").is_none());
    }

    #[test]
    fn previews_count_kinds_and_sample_each_kind_first() {
        let row = |id: &str, kind: &str| super::ImportRow {
            source_id: id.into(),
            kind: kind.into(),
            title: id.into(),
            year: None,
            external_ids: std::collections::BTreeMap::new(),
            status: None,
            rating: None,
            notes: None,
            history: vec![at(None)],
            lists: vec!["L".into()],
        };
        let parsed = super::Parsed {
            rows: ["a", "b", "c", "d", "e", "f"]
                .iter()
                .map(|id| Ok(row(id, "movie")))
                .chain([Ok(row("g", "book"))])
                .collect(),
            lost: vec![super::Lost::new("x", 2)],
        };
        let p = super::preview(super::legacy::FORMAT, &parsed);
        assert_eq!(p.kinds["movie"], 6);
        assert_eq!((p.events, p.unmatched), (7, 7));
        assert_eq!(p.samples.len(), 5);
        assert_eq!(p.samples[1].kind, "book", "one of each kind first");
        assert_eq!(p.lists, vec!["L".to_string()]);
        assert_eq!(p.lost, vec![super::Lost::new("x", 2)]);
    }
}
