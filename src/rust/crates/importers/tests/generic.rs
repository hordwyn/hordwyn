#![cfg(feature = "generic")]
//! M12 acceptance for the generic format: importing twice creates no duplicates, a crash in
//! the middle resumes and finishes, dry runs write nothing, bad rows are reported.

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
