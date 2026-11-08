//! The import runner: start (store the file, queue the job), run with checkpoints, cancel,
//! run a file again, and the manual match of an unmatched row.

use std::collections::HashMap;
use std::sync::Arc;

use app::ports::{AppEvent, BlobStore, JobQueue, Lane, NewJob, PortError};
use app::{App, AppError, NewCatalogItem};
use domain::catalog::ExternalId;
use domain::ids::{ItemId, ListId};
use domain::kind::{KindId, ProgressModel, Status};
use domain::library::Rating;
use domain::list::Visibility;
use domain::progress::Progress;
use domain::user::Actor;
use uuid::Uuid;

use crate::{
    CHECKPOINT, IMPORT_JOB, ImportError, ImportRecord, ImportRow, ImportState, ImportStore,
    RowError, Unmatched, formats, needs_external_id, op_id, parse, parse_date, preview,
};

/// What one row came to.
enum Applied {
    /// Imported (or, in a dry run, would be); whether it matched an existing catalog item.
    Imported(bool),
    /// No usable external id and no earlier match.
    Unmatched,
}

/// Where a row's catalog item comes from.
enum Resolved {
    Item(ItemId, bool),
    Unmatched,
    /// Dry run, nothing matches: it would be a new stub.
    New,
}

/// How many of the user's newest imports are checked for one that still reads a file (a
/// rerun is queued right after the run it repeats, so it is among the newest).
const FILE_SIBLINGS: u32 = 200;

/// Lists by name, created on first use within one run.
type ListCache = HashMap<String, ListId>;

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
        let key = format!("up/{}/imports/{id}", actor.user_id.0);
        self.blobs.put(&key, bytes).await?;
        self.queue(actor, id, format, key, dry_run, None).await
    }

    async fn queue(
        &self,
        actor: Actor,
        id: Uuid,
        format: &str,
        source_key: String,
        dry_run: bool,
        source_import: Option<Uuid>,
    ) -> Result<ImportRecord, ImportError> {
        let now = self.app.clock.now();
        let r = ImportRecord {
            id,
            user_id: actor.user_id.0,
            format: format.to_string(),
            source_key,
            dry_run,
            state: ImportState::Queued,
            cursor: 0,
            imported: 0,
            matched: 0,
            errors: vec![],
            created_at: now,
            updated_at: now,
            total: 0,
            unmatched: vec![],
            preview: None,
            source_import,
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

    /// Run the file of an earlier import for real: the real run after a dry run's preview,
    /// or a retry (rows already imported change nothing).
    ///
    /// # Errors
    /// Unknown import or not the owner, the file deleted after a real run
    /// (`import_file_gone`); storage errors.
    pub async fn rerun(&self, actor: Actor, id: Uuid) -> Result<ImportRecord, ImportError> {
        let r = self.status(actor, id).await?;
        if self.blobs.get(&r.source_key).await?.is_none() {
            return Err(ImportError::Conflict {
                code: "import_file_gone",
                message: "the file of this import was deleted after its run; upload it again"
                    .into(),
            });
        }
        let format = r.format.clone();
        self.queue(
            actor,
            Uuid::now_v7(),
            &format,
            r.source_key,
            false,
            Some(id),
        )
        .await
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

    /// The user's imports, newest first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn list(&self, actor: Actor, limit: u32) -> Result<Vec<ImportRecord>, ImportError> {
        Ok(self.store.list(actor.user_id.0, limit).await?)
    }

    /// Ask a queued or running import to stop at its next checkpoint.
    ///
    /// # Errors
    /// Unknown import, not the owner, or already finished (`import_finished`).
    pub async fn cancel(&self, actor: Actor, id: Uuid) -> Result<ImportRecord, ImportError> {
        let r = self.status(actor, id).await?;
        if r.state.is_final() {
            return Err(ImportError::Conflict {
                code: "import_finished",
                message: "the import has already finished".into(),
            });
        }
        self.store.request_cancel(id).await?;
        Ok(r)
    }

    /// Match an unmatched row of a finished real run with `item` and replay it.
    ///
    /// # Errors
    /// Unknown import or row, not the owner, a running import (`import_running`), a dry run
    /// (`import_dry_run`), an item of
    /// another kind (`import_kind_mismatch`), or the row's own errors.
    pub async fn match_row(
        &self,
        actor: Actor,
        id: Uuid,
        source_id: &str,
        item: ItemId,
    ) -> Result<ImportRecord, ImportError> {
        let mut r = self.status(actor, id).await?;
        // the running import saves its own copy of the record at every checkpoint, which
        // would undo the match (UX3-14)
        if !r.state.is_final() {
            return Err(ImportError::Conflict {
                code: "import_running",
                message: "the import is still running; match its rows when it has finished".into(),
            });
        }
        if r.dry_run {
            return Err(ImportError::Conflict {
                code: "import_dry_run",
                message: "a dry run imports nothing; run the import first".into(),
            });
        }
        let pos = r
            .unmatched
            .iter()
            .position(|u| u.row.source_id == source_id)
            .ok_or_else(|| ImportError::UnknownRow(source_id.to_string()))?;
        let catalog = self.app.catalog_item(item).await?;
        if catalog.kind.as_str() != r.unmatched[pos].row.kind {
            return Err(ImportError::Conflict {
                code: "import_kind_mismatch",
                message: "the item is of another kind than the row".into(),
            });
        }
        self.store
            .set_item_for(r.user_id, &r.format, source_id, item.0)
            .await?;
        let row = r.unmatched[pos].row.clone();
        self.apply(actor, &r, &row, &mut ListCache::new()).await?;
        r.unmatched.remove(pos);
        r.imported += 1;
        r.matched += 1;
        r.updated_at = self.app.clock.now();
        self.store.save(&r).await?;
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
        if r.state.is_final() {
            return Ok(r);
        }
        if self.store.cancel_requested(id).await? {
            return self.stop(r).await;
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
        let parsed = tokio::task::spawn_blocking(move || parse(&format, &bytes))
            .await
            .map_err(|e| PortError(e.to_string()))??;
        r.state = ImportState::Running;
        r.total = parsed.rows.len() as u64;
        if r.preview.is_none() {
            r.preview = Some(preview(&r.format, &parsed));
        }
        let skip = usize::try_from(r.cursor).unwrap_or(usize::MAX);
        let mut lists = ListCache::new();
        for (done_now, row) in parsed.rows.into_iter().skip(skip).enumerate() {
            if limit.is_some_and(|l| done_now as u64 >= l) {
                self.store.save(&r).await?;
                return Ok(r);
            }
            self.handle(actor, &mut r, row, &mut lists).await;
            r.cursor += 1;
            if r.cursor % CHECKPOINT == 0 {
                r.updated_at = self.app.clock.now();
                self.store.save(&r).await?;
                self.progress(&r).await;
                if self.store.cancel_requested(id).await? {
                    return self.stop(r).await;
                }
            }
        }
        r.state = ImportState::Done;
        r.updated_at = self.app.clock.now();
        self.store.save(&r).await?;
        self.progress(&r).await;
        self.release_file(&r).await;
        if !r.dry_run {
            self.notify(&r, "import.finished", String::new()).await;
        }
        Ok(r)
    }

    /// One row into the record's counts, errors or unmatched list.
    async fn handle(
        &self,
        actor: Actor,
        r: &mut ImportRecord,
        row: Result<ImportRow, RowError>,
        lists: &mut ListCache,
    ) {
        let row = match row {
            Ok(row) => row,
            Err(e) => {
                r.errors.push(e);
                return;
            }
        };
        match self.apply(actor, r, &row, lists).await {
            Ok(Applied::Imported(matched)) => {
                r.imported += 1;
                r.matched += u64::from(matched);
            }
            Ok(Applied::Unmatched) => {
                // a resumed run sees the rows since the last checkpoint again
                if !r.unmatched.iter().any(|u| u.row.source_id == row.source_id) {
                    r.unmatched.push(Unmatched {
                        line: r.cursor + 1,
                        row,
                    });
                }
            }
            Err(e) => r.errors.push(RowError {
                line: r.cursor + 1,
                source_id: Some(row.source_id.clone()),
                message: e.to_string(),
            }),
        }
    }

    /// Stop on a cancel request.
    async fn stop(&self, mut r: ImportRecord) -> Result<ImportRecord, ImportError> {
        r.state = ImportState::Cancelled;
        r.updated_at = self.app.clock.now();
        self.store.save(&r).await?;
        self.progress(&r).await;
        self.release_file(&r).await;
        Ok(r)
    }

    /// Delete the uploaded file once a run has ended (UX3-15): it counts against the user's
    /// upload quota. A finished dry run keeps it for "Run for real"; a file another queued or
    /// running import of the user still reads is kept. Best effort: a failure only logs.
    async fn release_file(&self, r: &ImportRecord) {
        if r.dry_run && r.state == ImportState::Done {
            return;
        }
        let in_use = match self.store.list(r.user_id, FILE_SIBLINGS).await {
            Ok(all) => all
                .iter()
                .any(|o| o.id != r.id && o.source_key == r.source_key && !o.state.is_final()),
            Err(e) => {
                tracing::warn!(error = %e, "import file kept: the imports could not be listed");
                true
            }
        };
        if !in_use && let Err(e) = self.blobs.delete(std::slice::from_ref(&r.source_key)).await {
            tracing::warn!(error = %e, "deleting an import file failed");
        }
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

    async fn progress(&self, r: &ImportRecord) {
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
                    "total": r.total,
                    "imported": r.imported,
                    "errors": r.errors.len(),
                    "unmatched": r.unmatched.len(),
                    "dry_run": r.dry_run,
                }),
            })
            .await;
    }

    /// The catalog item for a row: the one an earlier import (or a manual match) mapped,
    /// one with a matching external id, or a new stub (refreshed later in the import lane).
    async fn resolve(
        &self,
        r: &ImportRecord,
        row: &ImportRow,
        kind: &KindId,
        external_ids: &[ExternalId],
    ) -> Result<Resolved, ImportError> {
        let user = r.user_id;
        if let Some(item) = self.store.item_for(user, &r.format, &row.source_id).await? {
            return Ok(Resolved::Item(ItemId(item), true));
        }
        if external_ids.is_empty() && needs_external_id(&r.format) {
            return Ok(Resolved::Unmatched);
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
            None if r.dry_run => return Ok(Resolved::New),
            None => (self.stub(row, kind, external_ids).await?, false),
        };
        if !r.dry_run {
            self.store
                .set_item_for(user, &r.format, &row.source_id, id.0)
                .await?;
        }
        Ok(Resolved::Item(id, matched))
    }

    async fn stub(
        &self,
        row: &ImportRow,
        kind: &KindId,
        external_ids: &[ExternalId],
    ) -> Result<ItemId, ImportError> {
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
        Ok(item.id)
    }

    /// Import one row.
    async fn apply(
        &self,
        actor: Actor,
        r: &ImportRecord,
        row: &ImportRow,
        lists: &mut ListCache,
    ) -> Result<Applied, ImportError> {
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
        let (item_id, matched) = match self.resolve(r, row, &kind, &external_ids).await? {
            Resolved::Item(id, matched) => (id, matched),
            Resolved::Unmatched => return Ok(Applied::Unmatched),
            // dry run, no catalog match: nothing more to check
            Resolved::New => return Ok(Applied::Imported(false)),
        };
        if r.dry_run {
            return Ok(Applied::Imported(matched));
        }
        self.app
            .add_to_library(actor, item_id, status.clone())
            .await?;
        self.history(actor, r, row, item_id, descriptor.progress_model)
            .await?;
        // after the history: its events move the status (finished -> completed), the
        // file's own status has the last word
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
        for name in &row.lists {
            let list = self.list_id(actor, name, lists).await?;
            self.app.add_to_list(actor, list, item_id, None).await?;
        }
        Ok(Applied::Imported(matched))
    }

    async fn history(
        &self,
        actor: Actor,
        r: &ImportRecord,
        row: &ImportRow,
        item_id: ItemId,
        model: ProgressModel,
    ) -> Result<(), ImportError> {
        for (i, h) in row.history.iter().enumerate() {
            let at = parse_date(&h.date);
            let progress = match &h.progress {
                Some(p) => serde_json::from_value::<Progress>(p.clone())
                    .map_err(|e| ImportError::Port(PortError(format!("progress: {e}"))))?,
                None if model == ProgressModel::Single => Progress::Finished,
                // "finished" without details only fits single-step kinds; the status covers it
                None => continue,
            };
            self.app
                .log_progress(
                    actor,
                    item_id,
                    progress,
                    at,
                    Some(op_id(actor.user_id.0, &r.format, &row.source_id, h, i)),
                )
                .await?;
        }
        Ok(())
    }

    /// The user's list called `name`, created (private) when there is none.
    async fn list_id(
        &self,
        actor: Actor,
        name: &str,
        cache: &mut ListCache,
    ) -> Result<ListId, ImportError> {
        if let Some(id) = cache.get(name) {
            return Ok(*id);
        }
        let found = self
            .app
            .lists(actor)
            .await?
            .into_iter()
            .find(|l| l.name == name);
        let id = match found {
            Some(l) => l.id,
            None => {
                self.app
                    .create_list(actor, name, None, Visibility::Private)
                    .await?
                    .id
            }
        };
        cache.insert(name.to_string(), id);
        Ok(id)
    }
}
