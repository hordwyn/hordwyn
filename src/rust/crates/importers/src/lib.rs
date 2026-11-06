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
//! - **observable**: `import.progress` events over SSE and a per-row error report.
#![deny(unsafe_code)]

#[cfg(feature = "generic")]
pub mod generic;
pub mod legacy;
pub mod pg;
pub mod row;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use app::ports::{AppEvent, BlobStore, JobQueue, Lane, NewJob, PortError};
use app::{App, AppError, NewCatalogItem};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::catalog::ExternalId;
use domain::ids::ItemId;
use domain::kind::{KindId, ProgressModel, Status};
use domain::library::Rating;
use domain::progress::Progress;
use domain::user::Actor;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use row::{ImportRow, RowError};

/// Job kind that runs an import.
pub const IMPORT_JOB: &str = "import.run";
/// Rows between cursor saves and progress events.
const CHECKPOINT: u64 = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportState {
    Queued,
    Running,
    Done,
    Failed,
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
}

#[async_trait]
pub trait ImportStore: Send + Sync {
    async fn insert(&self, r: &ImportRecord) -> Result<(), PortError>;
    async fn get(&self, id: Uuid) -> Result<Option<ImportRecord>, PortError>;
    async fn save(&self, r: &ImportRecord) -> Result<(), PortError>;
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
    f
}

type Rows = Box<dyn Iterator<Item = Result<ImportRow, RowError>>>;

fn parser(format: &str, bytes: Vec<u8>) -> Result<Rows, ImportError> {
    match format {
        #[cfg(feature = "generic")]
        "generic-jsonl" => Ok(generic::jsonl(std::io::Cursor::new(bytes))),
        #[cfg(feature = "generic")]
        "generic-csv" => Ok(generic::csv(std::io::Cursor::new(bytes))),
        legacy::FORMAT => Ok(legacy::parse(bytes)),
        other => Err(ImportError::UnknownFormat(other.to_string())),
    }
}

/// Deterministic client op id for event `i` of a row: re-imports are no-ops (D-12).
fn op_id(user: Uuid, format: &str, source_id: &str, i: usize) -> Uuid {
    let h = blake3::hash(format!("import:{user}:{format}:{source_id}:{i}").as_bytes());
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

pub struct Importer {
    app: App,
    store: Arc<dyn ImportStore>,
    blobs: Arc<dyn BlobStore>,
    jobs: Arc<dyn JobQueue>,
}

impl Importer {
    #[must_use]
    pub fn new(
        app: App,
        store: Arc<dyn ImportStore>,
        blobs: Arc<dyn BlobStore>,
        jobs: Arc<dyn JobQueue>,
    ) -> Self {
        Self {
            app,
            store,
            blobs,
            jobs,
        }
    }

    /// Store the uploaded file (counted against the user's upload quota) and queue the import.
    ///
    /// # Errors
    /// Unknown format, quota or storage errors.
    pub async fn start(
        &self,
        actor: Actor,
        format: &str,
        bytes: Vec<u8>,
        dry_run: bool,
    ) -> Result<ImportRecord, ImportError> {
        if !formats().contains(&format) {
            return Err(ImportError::UnknownFormat(format.to_string()));
        }
        let id = Uuid::now_v7();
        let user = actor.user_id.0;
        let key = format!("up/{user}/imports/{id}");
        self.blobs.put(&key, bytes).await?;
        let now = self.app.clock.now();
        let r = ImportRecord {
            id,
            user_id: user,
            format: format.to_string(),
            source_key: key,
            dry_run,
            state: ImportState::Queued,
            cursor: 0,
            imported: 0,
            matched: 0,
            errors: vec![],
            created_at: now,
            updated_at: now,
        };
        self.store.insert(&r).await?;
        self.jobs
            .enqueue(NewJob {
                lane: Lane::UserImport,
                kind: IMPORT_JOB.into(),
                payload: serde_json::json!({ "import_id": id }),
                dedupe_key: Some(format!("{IMPORT_JOB}:{id}")),
                priority: 0,
                run_at: None,
            })
            .await?;
        Ok(r)
    }

    /// # Errors
    /// Unknown import or not the owner.
    pub async fn status(&self, actor: Actor, id: Uuid) -> Result<ImportRecord, ImportError> {
        let r = self.store.get(id).await?.ok_or(ImportError::Unknown(id))?;
        // import reports show a user's library: owner only, admins included (privacy)
        if r.user_id != actor.user_id.0 {
            return Err(ImportError::Forbidden);
        }
        Ok(r)
    }

    /// The job: run the import to the end.
    ///
    /// # Errors
    /// Storage errors; the job is then retried and resumes at the cursor.
    pub async fn run(&self, id: Uuid) -> Result<ImportRecord, ImportError> {
        self.run_rows(id, None).await
    }

    /// Run at most `limit` more rows, then return as if the process had stopped (tests use
    /// this to simulate a crash mid-import).
    ///
    /// # Errors
    /// See [`Importer::run`].
    pub async fn run_rows(
        &self,
        id: Uuid,
        limit: Option<u64>,
    ) -> Result<ImportRecord, ImportError> {
        let mut r = self.store.get(id).await?.ok_or(ImportError::Unknown(id))?;
        if matches!(r.state, ImportState::Done | ImportState::Failed) {
            return Ok(r);
        }
        let user = self.app.user(domain::ids::UserId(r.user_id)).await?;
        let actor = Actor {
            user_id: user.id,
            role: user.role,
        };
        let Some(bytes) = self.blobs.get(&r.source_key).await? else {
            r.state = ImportState::Failed;
            r.updated_at = self.app.clock.now();
            self.store.save(&r).await?;
            self.notify(&r, "import.failed", ImportError::MissingFile.to_string())
                .await;
            return Err(ImportError::MissingFile);
        };
        let format = r.format.clone();
        // parsing is CPU work: do it off the async workers, in one go per run
        let rows: Vec<Result<ImportRow, RowError>> =
            tokio::task::spawn_blocking(move || parser(&format, bytes).map(Iterator::collect))
                .await
                .map_err(|e| PortError(e.to_string()))??;
        r.state = ImportState::Running;
        let total = rows.len() as u64;
        let skip = usize::try_from(r.cursor).unwrap_or(usize::MAX);
        for (done_now, row) in rows.into_iter().skip(skip).enumerate() {
            if limit.is_some_and(|l| done_now as u64 >= l) {
                self.store.save(&r).await?;
                return Ok(r);
            }
            match row {
                Ok(row) => match self.apply(actor, &r, &row).await {
                    Ok(matched) => {
                        r.imported += 1;
                        r.matched += u64::from(matched);
                    }
                    Err(e) => r.errors.push(RowError {
                        line: r.cursor + 1,
                        source_id: Some(row.source_id.clone()),
                        message: e.to_string(),
                    }),
                },
                Err(e) => r.errors.push(e),
            }
            r.cursor += 1;
            if r.cursor % CHECKPOINT == 0 {
                r.updated_at = self.app.clock.now();
                self.store.save(&r).await?;
                self.progress(&r, total).await;
            }
        }
        r.state = ImportState::Done;
        r.updated_at = self.app.clock.now();
        self.store.save(&r).await?;
        self.progress(&r, total).await;
        if !r.dry_run {
            self.notify(&r, "import.finished", String::new()).await;
        }
        Ok(r)
    }

    async fn notify(&self, r: &ImportRecord, event: &str, error: String) {
        self.app
            .notifications
            .emit(
                app::ports::Notification::new(
                    event,
                    app::ports::Audience::Users(vec![domain::ids::UserId(r.user_id)]),
                    format!("import:{}", r.id),
                )
                .arg("imported", r.imported)
                .arg("errors", r.errors.len())
                .arg("error", error),
            )
            .await;
    }

    async fn progress(&self, r: &ImportRecord, total: u64) {
        self.app
            .events
            .publish(AppEvent {
                topic: "import.progress".into(),
                entity: None,
                audience: vec![domain::ids::UserId(r.user_id)],
                payload: serde_json::json!({
                    "import_id": r.id,
                    "state": r.state,
                    "done": r.cursor,
                    "total": total,
                    "imported": r.imported,
                    "errors": r.errors.len(),
                    "dry_run": r.dry_run,
                }),
            })
            .await;
    }

    /// The catalog item for a row: the one an earlier import mapped, one with a matching
    /// external id, or a new stub (refreshed later in the import lane). `None` in a dry run
    /// when nothing matches.
    async fn resolve(
        &self,
        r: &ImportRecord,
        row: &ImportRow,
        kind: &KindId,
        external_ids: &[ExternalId],
    ) -> Result<Option<(ItemId, bool)>, ImportError> {
        let user = r.user_id;
        if let Some(item) = self.store.item_for(user, &r.format, &row.source_id).await? {
            return Ok(Some((ItemId(item), true)));
        }
        let mut existing = None;
        for e in external_ids {
            if let Some(i) = self
                .app
                .repos
                .catalog
                .by_external_id(kind, e)
                .await
                .map_err(AppError::from)?
            {
                existing = Some(i.id);
                break;
            }
        }
        let (id, matched) = match existing {
            Some(id) => (id, true),
            None if r.dry_run => return Ok(None),
            None => {
                let item = self
                    .app
                    .find_or_create_catalog_item(NewCatalogItem {
                        kind: kind.clone(),
                        title: row.title.clone(),
                        original_title: None,
                        year: row.year,
                        description: None,
                        external_ids: external_ids.to_vec(),
                        attributes: serde_json::json!({}),
                        cover_url: None,
                    })
                    .await?;
                // details come later, from the connectors, in the import lane
                if !external_ids.is_empty() {
                    let _ = self
                        .jobs
                        .enqueue(NewJob {
                            lane: Lane::UserImport,
                            kind: app::REFRESH_JOB.into(),
                            payload: serde_json::json!({ "item_id": item.id }),
                            dedupe_key: Some(format!("{}:{}", app::REFRESH_JOB, item.id)),
                            priority: 0,
                            run_at: None,
                        })
                        .await;
                }
                (item.id, false)
            }
        };
        if !r.dry_run {
            self.store
                .set_item_for(user, &r.format, &row.source_id, id.0)
                .await?;
        }
        Ok(Some((id, matched)))
    }

    /// Import one row; returns whether it matched an existing catalog item.
    async fn apply(
        &self,
        actor: Actor,
        r: &ImportRecord,
        row: &ImportRow,
    ) -> Result<bool, ImportError> {
        let kind = KindId::new(row.kind.clone()).map_err(AppError::from)?;
        let descriptor = self.app.kind(&kind)?;
        let status = row.status.clone().map(Status::new);
        if let Some(s) = &status {
            descriptor.check_status(s).map_err(AppError::from)?;
        }
        let rating = row
            .rating
            .map(Rating::new)
            .transpose()
            .map_err(AppError::from)?;
        let external_ids: Vec<ExternalId> = row
            .external_ids
            .iter()
            .map(|(ns, v)| ExternalId::new(ns.clone(), v.clone()))
            .collect();
        let user = actor.user_id.0;
        let Some((item_id, matched)) = self.resolve(r, row, &kind, &external_ids).await? else {
            // dry run, no catalog match: nothing more to check
            return Ok(false);
        };
        if r.dry_run {
            return Ok(matched);
        }
        self.app
            .add_to_library(actor, item_id, status.clone())
            .await?;
        if let Some(s) = status {
            self.app.set_status(actor, item_id, s, None).await?;
        }
        if rating.is_some() {
            self.app.rate(actor, item_id, rating, None).await?;
        }
        if row.notes.is_some() {
            self.app
                .set_notes(actor, item_id, row.notes.clone(), None)
                .await?;
        }
        for (i, h) in row.history.iter().enumerate() {
            let at = parse_date(&h.date);
            let progress = match &h.progress {
                Some(p) => serde_json::from_value::<Progress>(p.clone())
                    .map_err(|e| ImportError::Port(PortError(format!("progress: {e}"))))?,
                None if descriptor.progress_model == ProgressModel::Single => Progress::Finished,
                // "finished" without details only fits single-step kinds; the status covers it
                None => continue,
            };
            self.app
                .log_progress(
                    actor,
                    item_id,
                    progress,
                    at,
                    Some(op_id(user, &r.format, &row.source_id, i)),
                )
                .await?;
        }
        Ok(matched)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn op_ids_are_stable_per_row_and_event() {
        let u = uuid::Uuid::nil();
        assert_eq!(super::op_id(u, "f", "1", 0), super::op_id(u, "f", "1", 0));
        assert_ne!(super::op_id(u, "f", "1", 0), super::op_id(u, "f", "1", 1));
        assert_ne!(super::op_id(u, "f", "1", 0), super::op_id(u, "f", "2", 0));
    }

    #[test]
    fn dates_parse_in_both_forms() {
        assert!(super::parse_date("2024-05-01").is_some());
        assert!(super::parse_date("2024-05-01T10:00:00Z").is_some());
        assert!(super::parse_date("yesterday").is_none());
    }
}
