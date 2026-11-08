//! The legacy importer (KICKOFF Q4, rule 6), derived only from the owner's export: the
//! anonymised fixture at `crates/testkit/fixtures/import/legacy/export.json` (21 items, every
//! kind, source and history state). Exact counts per kind, the episodes and their dates, the
//! statuses from the built-in collections, lists from the other collections, what is lost,
//! and that a second run creates nothing.

use std::collections::BTreeMap;
use std::sync::Arc;

use app::ports::JobQueue;
use domain::user::{Actor, Role};
use importers::{ImportState, Importer, MemImportStore};
use testkit::TestEnv;
use testkit::fakes::MemBlobs;

fn fixture() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../testkit/fixtures/import/legacy/export.json");
    std::fs::read(path).expect("legacy fixture")
}

struct Env {
    t: TestEnv,
    importer: Importer,
    actor: Actor,
}

async fn env() -> Env {
    let t = TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let user = t
        .app
        .register_user("owner", "Owner", None, Role::Member)
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

async fn import(e: &Env, bytes: Vec<u8>, dry_run: bool) -> importers::ImportRecord {
    let r = e
        .importer
        .start(e.actor, importers::legacy::FORMAT, bytes, dry_run)
        .await
        .unwrap();
    e.importer.run(r.id).await.unwrap()
}

/// (library entries, progress events, lists, list items, catalog items)
async fn counts(e: &Env) -> (usize, usize, usize, usize, usize) {
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
    let lists = e.t.app.lists(e.actor).await.unwrap();
    let items = lists.iter().map(|l| l.items.len()).sum();
    (
        lib.len(),
        events,
        lists.len(),
        items,
        e.t.store.catalog_len(),
    )
}

fn lost(r: &importers::ImportRecord) -> BTreeMap<String, u64> {
    r.preview
        .as_ref()
        .expect("preview")
        .lost
        .iter()
        .map(|l| (l.code.clone(), l.count))
        .collect()
}

#[test]
fn the_fixture_parses_into_one_row_per_item() {
    let parsed = importers::legacy::parse(&fixture());
    assert_eq!(parsed.rows.len(), 21);
    let rows: Vec<_> = parsed.rows.into_iter().map(Result::unwrap).collect();
    let mut kinds = BTreeMap::new();
    for r in &rows {
        *kinds.entry(r.kind.as_str()).or_insert(0) += 1;
    }
    assert_eq!(
        kinds,
        BTreeMap::from([
            ("anime", 3),
            ("book", 4),
            ("movie", 8),
            ("show", 3),
            ("video_game", 3)
        ])
    );
    // source + identifier -> external id and the stable source id
    let castle = rows.iter().find(|r| r.title == "Castle").unwrap();
    assert_eq!(castle.source_id, "show:tmdb:1419");
    assert_eq!(castle.external_ids["tmdb"], "1419");
    assert_eq!(castle.status.as_deref(), Some("in_progress"));
    assert_eq!(castle.history.len(), 12);
    assert_eq!(castle.history[0].date, "2026-02-20T00:29:05Z");
    assert_eq!(
        castle.history[0].progress,
        Some(serde_json::json!({"type": "episode", "season": 5, "episode": 7}))
    );
    assert_eq!(castle.lists, vec!["Monitoring".to_string()]);
    let sao = rows.iter().find(|r| r.title == "Sword Art Online").unwrap();
    assert_eq!(sao.external_ids["myanimelist"], "11757");
    assert_eq!(sao.status.as_deref(), Some("completed"));
    // undated views: logged on the day it entered Completed (an estimate, reported)
    assert_eq!(sao.history.len(), 25);
    assert_eq!(sao.history[0].date, "2025-06-21T09:42:08.913533Z");
    let anime = rows
        .iter()
        .find(|r| r.external_ids.get("myanimelist").map(String::as_str) == Some("53516"))
        .unwrap();
    assert_eq!(anime.history.len(), 12);
    assert_eq!(
        anime.history[0].progress,
        Some(serde_json::json!({"type": "episode", "season": 1, "episode": 12}))
    );
    assert_eq!(anime.history[0].date, "2025-07-01T00:00:00Z");
    let books: Vec<_> = rows.iter().filter(|r| r.kind == "book").collect();
    assert!(books.iter().all(|b| b.history.is_empty()));
    assert_eq!(
        books
            .iter()
            .filter(|b| b.external_ids.contains_key("google_books"))
            .count(),
        2
    );
    let dropped = rows
        .iter()
        .find(|r| r.external_ids.get("google_books").map(String::as_str) == Some("rWvQswEACAAJ"))
        .unwrap();
    assert_eq!(dropped.status.as_deref(), Some("dropped"));
    let held = rows
        .iter()
        .find(|r| r.external_ids.get("igdb").map(String::as_str) == Some("386"))
        .unwrap();
    assert_eq!(held.status.as_deref(), Some("paused"));
    // a rewatch in progress wins over the earlier completion
    let hp = rows
        .iter()
        .find(|r| r.external_ids.get("tmdb").map(String::as_str) == Some("671"))
        .unwrap();
    assert_eq!(hp.status.as_deref(), Some("in_progress"));
    // the sections that do not map are named, with their size
    let lost: BTreeMap<_, _> = parsed
        .lost
        .iter()
        .map(|l| (l.code.as_str(), l.count))
        .collect();
    assert_eq!(lost.get("estimated_dates"), Some(&31));
    assert_eq!(lost.get("undated_history"), None);
    assert_eq!(lost.get("partial_progress"), Some(&9));
    assert_eq!(lost.get("finish_dates"), Some(&1));
    assert_eq!(lost.get("watched_on"), Some(&15));
    assert_eq!(lost.get("workouts"), None, "empty sections are not listed");
}

#[tokio::test]
async fn dry_run_previews_and_writes_nothing() {
    let e = env().await;
    let r = import(&e, fixture(), true).await;
    assert_eq!(r.state, ImportState::Done);
    assert_eq!((r.total, r.errors.len()), (21, 0), "{:?}", r.errors);
    let p = r.preview.as_ref().expect("preview");
    assert_eq!(
        p.kinds,
        BTreeMap::from([
            ("anime".to_string(), 3),
            ("book".to_string(), 4),
            ("movie".to_string(), 8),
            ("show".to_string(), 3),
            ("video_game".to_string(), 3)
        ])
    );
    assert_eq!(p.samples.len(), 5);
    assert_eq!(p.events, 82);
    assert_eq!(
        p.lists,
        vec![
            "Fast and Furious".to_string(),
            "Monitoring".into(),
            "Owned".into(),
            "WatchWithPartner".into()
        ]
    );
    assert!(p.mapping.iter().any(|m| m.from == "lot"));
    assert_eq!(lost(&r).get("estimated_dates"), Some(&31));
    assert_eq!(counts(&e).await, (0, 0, 0, 0, 0));
}

/// UX3-14: matching a row while the import still runs is refused (the runner's own copy of
/// the record would overwrite the match at its next checkpoint); after the run it works.
#[tokio::test]
async fn matching_while_the_import_runs_is_refused() {
    let e = env().await;
    let item = |lot: &str, source: &str, id: &str, title: &str| {
        serde_json::json!({"lot": lot, "source": source, "identifier": id, "source_id": title,
            "collections": [], "reviews": [], "seen_history": []})
    };
    let doc = serde_json::json!({
        "metadata": [
            item("movie", "custom", "x1", "Home video"),
            item("movie", "tmdb", "949", "Heat"),
            item("movie", "tmdb", "680", "Pulp Fiction"),
        ],
        "collections": [], "people": [], "workouts": [], "exercises": [],
        "measurements": [], "metadata_groups": [], "workout_templates": []
    });
    let r = e
        .importer
        .start(
            e.actor,
            importers::legacy::FORMAT,
            doc.to_string().into_bytes(),
            false,
        )
        .await
        .unwrap();
    let partial = e.importer.run_rows(r.id, Some(2)).await.unwrap();
    assert_eq!(partial.state, ImportState::Running);
    assert_eq!(partial.unmatched.len(), 1);
    let film =
        e.t.app
            .find_or_create_catalog_item(app::NewCatalogItem {
                kind: domain::kind::KindId::new("movie").unwrap(),
                title: "Home video".into(),
                original_title: None,
                year: None,
                description: None,
                external_ids: vec![],
                attributes: serde_json::json!({}),
                cover_url: None,
            })
            .await
            .unwrap();
    let err = e
        .importer
        .match_row(e.actor, r.id, "movie:custom:x1", film.id)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            importers::ImportError::Conflict {
                code: "import_running",
                ..
            }
        ),
        "{err:?}"
    );
    assert_eq!(e.importer.run(r.id).await.unwrap().state, ImportState::Done);
    let matched = e
        .importer
        .match_row(e.actor, r.id, "movie:custom:x1", film.id)
        .await
        .unwrap();
    assert_eq!(matched.unmatched.len(), 0);
}

#[tokio::test]
async fn the_fixture_imports_with_episodes_dates_statuses_and_lists() {
    let e = env().await;
    let r = import(&e, fixture(), false).await;
    assert_eq!(r.state, ImportState::Done);
    assert_eq!(r.imported, 21, "{:?}", r.errors);
    assert_eq!(r.unmatched.len(), 0);
    // 73 episodes (49 anime, 24 show) and 9 movie views (6 of them on estimated dates)
    assert_eq!(counts(&e).await, (21, 82, 4, 16, 21));
    let lib = e.t.app.library(e.actor).await.unwrap();
    let mut statuses = BTreeMap::new();
    for entry in &lib {
        *statuses
            .entry(entry.status.value.as_str().to_string())
            .or_insert(0) += 1;
    }
    assert_eq!(
        statuses,
        BTreeMap::from([
            ("completed".to_string(), 10),
            ("dropped".into(), 1),
            ("in_progress".into(), 9),
            ("paused".into(), 1)
        ])
    );
    let lists = e.t.app.lists(e.actor).await.unwrap();
    let sizes: BTreeMap<_, _> = lists
        .iter()
        .map(|l| (l.name.as_str(), l.items.len()))
        .collect();
    assert_eq!(
        sizes,
        BTreeMap::from([
            ("Fast and Furious", 3),
            ("Monitoring", 9),
            ("Owned", 3),
            ("WatchWithPartner", 1)
        ])
    );
    // Castle: 12 episodes with their own dates
    let mut shows = Vec::new();
    for entry in lib.iter().filter(|x| x.kind.as_str() == "show") {
        shows.push(e.t.app.progress(e.actor, entry.item_id).await.unwrap());
    }
    let castle = shows
        .iter()
        .find(|h| {
            h.iter().any(|ev| {
                ev.progress
                    == domain::progress::Progress::Episode {
                        season: 5,
                        episode: 7,
                    }
            })
        })
        .expect("Castle S5E7");
    assert_eq!(castle.len(), 12);
    let s5e7 = castle
        .iter()
        .find(|ev| {
            ev.progress
                == domain::progress::Progress::Episode {
                    season: 5,
                    episode: 7,
                }
        })
        .unwrap();
    assert_eq!(s5e7.occurred_at.to_rfc3339(), "2026-02-20T00:29:05+00:00");
}

#[tokio::test]
async fn a_second_run_creates_nothing() {
    let e = env().await;
    import(&e, fixture(), false).await;
    let first = counts(&e).await;
    let again = import(&e, fixture(), false).await;
    assert_eq!(again.imported, 21);
    assert_eq!(counts(&e).await, first);
}

/// Every logged event as `item|time|progress`, sorted.
async fn logged(e: &Env) -> Vec<String> {
    let mut out = Vec::new();
    for entry in e.t.app.library(e.actor).await.unwrap() {
        for ev in e.t.app.progress(e.actor, entry.item_id).await.unwrap() {
            out.push(format!(
                "{:?}|{}|{:?}",
                entry.item_id, ev.occurred_at, ev.progress
            ));
        }
    }
    out.sort();
    out
}

/// UX3-13: a newer export with one more view at the top of a show's history (newest first)
/// adds that view only; the op ids of the earlier views do not depend on their position.
#[tokio::test]
async fn a_newer_export_adds_only_the_new_view() {
    let e = env().await;
    import(&e, fixture(), false).await;
    let first = counts(&e).await;
    let before = logged(&e).await;
    let mut doc: serde_json::Value = serde_json::from_slice(&fixture()).unwrap();
    let show = doc["metadata"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|m| {
            m["lot"] == "show"
                && m["seen_history"]
                    .as_array()
                    .is_some_and(|h| h.iter().filter(|s| s["state"] == "completed").count() > 1)
        })
        .expect("a show with several views");
    let history = show["seen_history"].as_array_mut().unwrap();
    let mut newest = history
        .iter()
        .find(|s| s["state"] == "completed" && s["ended_on"].is_string())
        .unwrap()
        .clone();
    newest["ended_on"] = serde_json::json!("2026-10-01T20:00:00Z");
    history.insert(0, newest);
    let shifted = serde_json::to_vec(&doc).unwrap();
    import(&e, shifted.clone(), false).await;
    let (lib, events, lists, items, catalog) = first;
    assert_eq!(
        counts(&e).await,
        (lib, events + 1, lists, items, catalog),
        "one new view, no duplicates"
    );
    // with position-based ids the count also grows by one (the new view takes the old
    // first id, the oldest view gets a new one), so compare what was logged
    let mut after = logged(&e).await;
    for k in &before {
        let at = after
            .iter()
            .position(|x| x == k)
            .unwrap_or_else(|| panic!("lost {k}"));
        after.remove(at);
    }
    assert_eq!(after.len(), 1, "{after:?}");
    assert!(after[0].contains("2026-10-01 20:00:00 UTC"), "{after:?}");
    import(&e, shifted, false).await;
    assert_eq!(
        counts(&e).await.1,
        events + 1,
        "and the newer file again adds nothing"
    );
}

#[tokio::test]
async fn items_without_a_usable_id_are_unmatched() {
    let e = env().await;
    let doc = serde_json::json!({
        "metadata": [{
            "lot": "movie", "source": "custom", "identifier": "x1",
            "source_id": "Home video", "collections": [], "reviews": [],
            "seen_history": [{"state": "completed", "progress": "100",
                "ended_on": "2024-01-01T00:00:00Z", "providers_consumed_on": []}]
        }],
        "collections": [], "people": [], "workouts": [], "exercises": [],
        "measurements": [], "metadata_groups": [], "workout_templates": []
    });
    let r = import(&e, doc.to_string().into_bytes(), false).await;
    assert_eq!((r.imported, r.unmatched.len()), (0, 1));
    assert_eq!(r.unmatched[0].row.title, "Home video");
    assert_eq!(counts(&e).await, (0, 0, 0, 0, 0));
}

#[test]
fn a_file_that_is_not_the_export_is_one_error() {
    let parsed = importers::legacy::parse(b"[1, 2]");
    assert_eq!(parsed.rows.len(), 1);
    assert!(parsed.rows[0].is_err());
}

/// The owner's full export (git-ignored `owner-inbox/legacy/export.json`); prints counts only.
#[tokio::test]
#[ignore = "needs the owner's full export in owner-inbox/legacy/export.json"]
async fn the_full_owner_export_imports() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../owner-inbox/legacy/export.json");
    let bytes = std::fs::read(&path).expect("owner export");
    let e = env().await;
    let r = import(&e, bytes.clone(), false).await;
    assert_eq!(r.state, ImportState::Done);
    assert_eq!(r.errors.len(), 0, "{} row errors", r.errors.len());
    let p = r.preview.clone().unwrap();
    eprintln!(
        "rows {} imported {} unmatched {} kinds {:?} events {} lists {} lost {:?}",
        r.total,
        r.imported,
        r.unmatched.len(),
        p.kinds,
        p.events,
        p.lists.len(),
        lost(&r)
    );
    let first = counts(&e).await;
    import(&e, bytes, false).await;
    assert_eq!(counts(&e).await, first);
}
