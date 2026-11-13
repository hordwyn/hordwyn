#![cfg(feature = "generic")]
//! M12 acceptance for the generic format: importing twice creates no duplicates, a crash in
//! the middle resumes and finishes, dry runs write nothing, bad rows are reported.

use std::fmt::Write as _;
use std::sync::Arc;

use app::ports::JobQueue;
use domain::user::{Actor, Role};
use importers::{ImportState, Importer, MemImportStore};
use testkit::TestEnv;
use testkit::fakes::MemBlobs;

const FILE: &str = r#"{"format":"library-export","version":1}
{"source_id":"m1","kind":"movie","title":"Heat","year":1995,"external_ids":{"tmdb":"949"},"status":"completed","rating":90,"notes":"with dad","history":[{"date":"2024-05-01"},{"date":"2025-01-02T20:00:00Z"}]}
{"source_id":"b1","kind":"book","title":"Dune","year":1965,"status":"in_progress"}
{"source_id":"x1","kind":"no_such_kind","title":"Nope"}
{"source_id":"m2","kind":"movie","title":"Ronin","rating":250}
"#;

struct Env {
    t: TestEnv,
    importer: Importer,
    actor: Actor,
}

async fn env() -> Env {
    let kinds = Arc::new(kinds::Registry::with_builtins());
    let t = TestEnv::with_kinds(kinds);
    let user = t
        .app
        .register_user("anna", "Anna", None, Role::Member)
        .await
        .unwrap();
    let importer = Importer::new(
        t.app.clone(),
        Arc::new(MemImportStore::default()),
        Arc::new(MemBlobs::default()),
        t.jobs.clone() as Arc<dyn JobQueue>,
    );
    Env {
        actor: Actor {
            user_id: user.id,
            role: user.role,
        },
        t,
        importer,
    }
}

async fn import(e: &Env, dry_run: bool) -> importers::ImportRecord {
    let r = e
        .importer
        .start(e.actor, "generic-jsonl", FILE.as_bytes().to_vec(), dry_run)
        .await
        .unwrap();
    e.importer.run(r.id).await.unwrap()
}

async fn counts(e: &Env) -> (usize, usize, usize) {
    let lib = e.t.app.library(e.actor).await.unwrap();
    let mut events = 0;
    for entry in &lib {
        events +=
            e.t.app
                .progress(e.actor, entry.item_id)
                .await
                .unwrap()
                .len();
    }
    (lib.len(), events, e.t.store.catalog_len())
}

#[tokio::test]
async fn imports_rows_and_reports_errors() {
    let e = env().await;
    let r = import(&e, false).await;
    assert_eq!(r.state, ImportState::Done);
    assert_eq!(r.imported, 2);
    assert_eq!(r.errors.len(), 2, "{:?}", r.errors);
    assert_eq!(r.errors[0].source_id.as_deref(), Some("x1"));
    let lib = e.t.app.library(e.actor).await.unwrap();
    let heat = lib.iter().find(|x| x.kind.as_str() == "movie").unwrap();
    assert_eq!(heat.status.value.as_str(), "completed");
    assert_eq!(
        heat.rating.value.map(domain::library::Rating::value),
        Some(90)
    );
    assert_eq!(heat.notes.value.as_deref(), Some("with dad"));
    assert_eq!(
        e.t.app.progress(e.actor, heat.item_id).await.unwrap().len(),
        2
    );
    // details are fetched later through the queue, never inline
    let jobs = e.t.jobs.jobs();
    assert!(
        jobs.iter()
            .any(|j| j.kind == app::REFRESH_JOB && j.lane == app::ports::Lane::UserImport)
    );
    assert!(jobs.iter().any(|j| j.kind == importers::IMPORT_JOB));
    let progress = e.t.events.topics();
    assert!(progress.iter().any(|t| t == "import.progress"));
    let n = e.t.notifications.all().pop().unwrap();
    assert_eq!(n.event, "import.finished");
    assert_eq!(n.args["imported"], 2);
    assert_eq!(n.args["errors"], 2);
}

#[tokio::test]
async fn importing_twice_creates_no_duplicates() {
    let e = env().await;
    import(&e, false).await;
    let first = counts(&e).await;
    import(&e, false).await;
    assert_eq!(counts(&e).await, first);
    assert_eq!(first.0, 2);
}

/// BOOK-1 (1), LB-15: a title the file says is Completed is finished on the day of its
/// newest history row, whenever and however often the file is imported; one without any
/// history is undated (in the totals, in no month).
#[tokio::test]
async fn importing_twice_keeps_every_completion_in_its_month() {
    const DONE: &str = r#"{"format":"library-export","version":1}
{"source_id":"m1","kind":"movie","title":"Heat","year":1995,"status":"completed","history":[{"date":"2024-05-01"},{"date":"2025-01-02T20:00:00Z"}]}
{"source_id":"b1","kind":"book","title":"Dune","year":1965,"status":"completed"}
"#;
    async fn run(e: &Env) -> (Vec<(String, u64)>, u64) {
        let file = DONE.as_bytes().to_vec();
        let r = e.importer.start(e.actor, "generic-jsonl", file, false);
        e.importer.run(r.await.unwrap().id).await.unwrap();
        let series = e.t.app.stats_series(e.actor, None, app::Bucket::Month);
        let done = series.await.unwrap().into_iter();
        let months = done
            .filter(|p| p.amounts.completions > 0)
            .map(|p| (p.bucket, p.amounts.completions))
            .collect();
        let totals = e.t.app.stats_totals(e.actor).await.unwrap();
        (months, totals.overall.amounts.completions)
    }
    let e = env().await;
    let first = run(&e).await;
    assert_eq!(first, (vec![("2025-01".to_string(), 1)], 2));
    e.t.clock.advance(chrono::Duration::days(40));
    assert_eq!(run(&e).await, first);
}

#[tokio::test]
async fn a_crash_mid_import_resumes_and_finishes() {
    let e = env().await;
    let r = e
        .importer
        .start(e.actor, "generic-jsonl", FILE.as_bytes().to_vec(), false)
        .await
        .unwrap();
    // the "worker" dies after one row
    let partial = e.importer.run_rows(r.id, Some(1)).await.unwrap();
    assert_eq!(partial.state, ImportState::Running);
    assert_eq!(partial.cursor, 1);
    // the job is claimed again and finishes
    let done = e.importer.run(r.id).await.unwrap();
    assert_eq!(done.state, ImportState::Done);
    assert_eq!(done.cursor, 4);
    assert_eq!(done.imported, 2);
    let after = counts(&e).await;
    // and a full second import changes nothing
    import(&e, false).await;
    assert_eq!(counts(&e).await, after);
}

#[tokio::test]
async fn dry_run_writes_nothing() {
    let e = env().await;
    let r = import(&e, true).await;
    assert_eq!(r.state, ImportState::Done);
    assert!(r.dry_run);
    assert_eq!(r.errors.len(), 2);
    assert_eq!(counts(&e).await, (0, 0, 0));
}

#[tokio::test]
async fn unknown_formats_are_refused() {
    let e = env().await;
    assert!(
        e.importer
            .start(e.actor, "zip-of-cats", vec![], false)
            .await
            .is_err()
    );
}

/// A store whose cancel request arrives while the import runs (after its first check).
#[derive(Default)]
struct CancelsMidway {
    inner: MemImportStore,
    checks: std::sync::atomic::AtomicU32,
}

#[async_trait::async_trait]
impl importers::ImportStore for CancelsMidway {
    async fn insert(&self, r: &importers::ImportRecord) -> Result<(), app::ports::PortError> {
        self.inner.insert(r).await
    }
    async fn get(
        &self,
        id: uuid::Uuid,
    ) -> Result<Option<importers::ImportRecord>, app::ports::PortError> {
        self.inner.get(id).await
    }
    async fn save(&self, r: &importers::ImportRecord) -> Result<(), app::ports::PortError> {
        self.inner.save(r).await
    }
    async fn list(
        &self,
        user: uuid::Uuid,
        limit: u32,
    ) -> Result<Vec<importers::ImportRecord>, app::ports::PortError> {
        self.inner.list(user, limit).await
    }
    async fn request_cancel(&self, id: uuid::Uuid) -> Result<(), app::ports::PortError> {
        self.inner.request_cancel(id).await
    }
    async fn cancel_requested(&self, _id: uuid::Uuid) -> Result<bool, app::ports::PortError> {
        let n = self
            .checks
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(n >= 1)
    }
    async fn item_for(
        &self,
        user: uuid::Uuid,
        format: &str,
        source_id: &str,
    ) -> Result<Option<uuid::Uuid>, app::ports::PortError> {
        self.inner.item_for(user, format, source_id).await
    }
    async fn set_item_for(
        &self,
        user: uuid::Uuid,
        format: &str,
        source_id: &str,
        item: uuid::Uuid,
    ) -> Result<(), app::ports::PortError> {
        self.inner.set_item_for(user, format, source_id, item).await
    }
}

#[tokio::test]
async fn a_cancel_request_stops_at_the_next_checkpoint() {
    let e = env().await;
    let importer = Importer::new(
        e.t.app.clone(),
        Arc::new(CancelsMidway::default()),
        Arc::new(MemBlobs::default()),
        e.t.jobs.clone() as Arc<dyn JobQueue>,
    );
    let mut file = String::from("{\"format\":\"library-export\",\"version\":1}\n");
    for i in 0..60 {
        let _ = writeln!(
            file,
            "{{\"source_id\":\"r{i}\",\"kind\":\"movie\",\"title\":\"Film {i}\"}}"
        );
    }
    let r = importer
        .start(e.actor, "generic-jsonl", file.into_bytes(), false)
        .await
        .unwrap();
    let r = importer.run(r.id).await.unwrap();
    assert_eq!(r.state, ImportState::Cancelled);
    assert_eq!(
        (r.cursor, r.imported, r.total),
        (importers::CHECKPOINT, 25, 60)
    );
    assert_eq!(e.t.app.library(e.actor).await.unwrap().len(), 25);
    // a stopped import stays stopped
    assert_eq!(
        importer.run(r.id).await.unwrap().state,
        ImportState::Cancelled
    );
}

/// UX3-15: the uploaded file is deleted when a real run ends (done or cancelled); a dry run
/// keeps it for "Run for real", whose end then deletes it; a rerun after that answers
/// `import_file_gone`.
#[tokio::test]
async fn uploaded_files_are_deleted_when_the_real_run_ends() {
    let kinds = Arc::new(kinds::Registry::with_builtins());
    let t = TestEnv::with_kinds(kinds);
    let user = t
        .app
        .register_user("anna", "Anna", None, Role::Member)
        .await
        .unwrap();
    let actor = Actor {
        user_id: user.id,
        role: user.role,
    };
    let blobs = Arc::new(MemBlobs::default());
    let importer = Importer::new(
        t.app.clone(),
        Arc::new(MemImportStore::default()),
        blobs.clone(),
        t.jobs.clone() as Arc<dyn JobQueue>,
    );
    let dry = importer
        .start(actor, "generic-jsonl", FILE.as_bytes().to_vec(), true)
        .await
        .unwrap();
    importer.run(dry.id).await.unwrap();
    assert_eq!(blobs.total_bytes(), FILE.len(), "a dry run keeps its file");
    let real = importer.rerun(actor, dry.id).await.unwrap();
    importer.run(real.id).await.unwrap();
    assert_eq!(blobs.total_bytes(), 0, "the real run deleted it");
    let err = importer.rerun(actor, real.id).await.unwrap_err();
    assert!(
        matches!(
            err,
            importers::ImportError::Conflict {
                code: "import_file_gone",
                ..
            }
        ),
        "{err:?}"
    );
    // a cancelled real run deletes its file too
    let r = importer
        .start(actor, "generic-jsonl", FILE.as_bytes().to_vec(), false)
        .await
        .unwrap();
    importer.cancel(actor, r.id).await.unwrap();
    let r = importer.run(r.id).await.unwrap();
    assert_eq!(r.state, ImportState::Cancelled);
    assert_eq!(blobs.total_bytes(), 0);
}
