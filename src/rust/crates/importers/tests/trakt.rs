#![cfg(feature = "trakt-import")]
//! The Trakt file importer (docs/connectors/trakt.md, DEC-297) on the synthetic fixtures in
//! `fixtures/trakt/` (the documented `/sync/history`, `/sync/ratings`, `/sync/watchlist`
//! shapes): history events per movie and per show episode, ratings 1..10 as 0..100, the
//! watchlist as planned with its note, what is lost, rows without a TMDB/IMDb/TVDB id
//! unmatched, and no duplicates on a second run. No network calls.

use std::collections::BTreeMap;
use std::sync::Arc;

use app::ports::JobQueue;
use domain::library::Rating;
use domain::user::{Actor, Role};
use importers::{ImportState, Importer, MemImportStore};
use testkit::TestEnv;
use testkit::fakes::MemBlobs;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/trakt")
        .join(name);
    std::fs::read(path).expect("trakt fixture")
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

async fn import(e: &Env, name: &str) -> importers::ImportRecord {
    let r = e
        .importer
        .start(e.actor, importers::trakt::FORMAT, fixture(name), false)
        .await
        .unwrap();
    e.importer.run(r.id).await.unwrap()
}

fn lost(r: &importers::ImportRecord) -> BTreeMap<String, u64> {
    r.preview
        .as_ref()
        .unwrap()
        .lost
        .iter()
        .map(|l| (l.code.clone(), l.count))
        .collect()
}

async fn events(e: &Env) -> usize {
    let mut n = 0;
    for entry in e.t.app.library(e.actor).await.unwrap() {
        n +=
            e.t.app
                .progress(e.actor, entry.item_id)
                .await
                .unwrap()
                .len();
    }
    n
}

#[test]
fn history_groups_events_by_movie_and_show() {
    let parsed = importers::trakt::parse(&fixture("history.json"));
    let rows: Vec<_> = parsed.rows.into_iter().map(Result::unwrap).collect();
    assert_eq!(rows.len(), 3);
    let knight = rows.iter().find(|r| r.title == "The Dark Knight").unwrap();
    assert_eq!(knight.source_id, "movie:4");
    assert_eq!(knight.kind, "movie");
    assert_eq!(knight.year, Some(2008));
    assert_eq!(knight.external_ids["tmdb"], "155");
    assert_eq!(knight.external_ids["imdb"], "tt0468569");
    assert_eq!(knight.status.as_deref(), Some("completed"));
    assert_eq!(knight.history.len(), 2);
    assert_eq!(knight.history[0].event_id.as_deref(), Some("1982346"));
    let parks = rows.iter().find(|r| r.kind == "show").unwrap();
    assert_eq!(parks.source_id, "show:4");
    assert_eq!(parks.external_ids["tvdb"], "84912");
    assert_eq!(parks.status.as_deref(), Some("in_progress"));
    assert_eq!(
        parks.history[0].progress,
        Some(serde_json::json!({"type": "episode", "season": 2, "episode": 1}))
    );
    assert_eq!(parks.history[0].date, "2014-02-27T09:28:53.000Z");
    let home = rows.iter().find(|r| r.title == "A Home Movie").unwrap();
    assert_eq!(home.external_ids.len(), 0, "trakt ids are not catalog ids");
    let lost: BTreeMap<_, _> = parsed
        .lost
        .iter()
        .map(|l| (l.code.as_str(), l.count))
        .collect();
    assert_eq!(lost.get("action"), Some(&5));
}

#[tokio::test]
async fn the_three_files_import_and_merge() {
    let e = env().await;
    let h = import(&e, "history.json").await;
    assert_eq!(h.state, ImportState::Done);
    assert_eq!((h.imported, h.unmatched.len()), (2, 1), "{:?}", h.errors);
    assert_eq!(h.unmatched[0].row.title, "A Home Movie");
    assert_eq!(events(&e).await, 4);
    let r = import(&e, "ratings.json").await;
    assert_eq!(r.imported, 2, "{:?}", r.errors);
    assert_eq!(lost(&r).get("season_episode_ratings"), Some(&2));
    assert_eq!(lost(&r).get("rated_at"), Some(&2));
    let w = import(&e, "watchlist.json").await;
    assert_eq!(w.imported, 2);
    assert_eq!(lost(&w).get("listed_at"), Some(&2));

    let lib = e.t.app.library(e.actor).await.unwrap();
    assert_eq!(lib.len(), 4, "Dark Knight, Parks, TRON, Walking Dead");
    let mut by_title = BTreeMap::new();
    for entry in &lib {
        let item = e.t.app.catalog_item(entry.item_id).await.unwrap();
        by_title.insert(item.title, entry.clone());
    }
    let tron = &by_title["TRON: Legacy"];
    assert_eq!(tron.rating.value.map(Rating::value), Some(100));
    assert_eq!(tron.status.value.as_str(), "planned");
    assert_eq!(
        tron.notes.value.as_deref(),
        Some("Need to catch up before TRON 3 is out.")
    );
    let parks = &by_title["Parks and Recreation"];
    assert_eq!(parks.rating.value.map(Rating::value), Some(70));
    assert_eq!(parks.status.value.as_str(), "in_progress");
    assert_eq!(
        by_title["The Dark Knight"].status.value.as_str(),
        "completed"
    );
    assert_eq!(
        by_title["The Walking Dead"].status.value.as_str(),
        "planned"
    );

    // the history again: no new events, the same entries
    import(&e, "history.json").await;
    assert_eq!(events(&e).await, 4);
    assert_eq!(e.t.app.library(e.actor).await.unwrap().len(), 4);
}

#[test]
fn other_files_are_one_error() {
    let parsed = importers::trakt::parse(br#"{"movies": []}"#);
    assert_eq!(parsed.rows.len(), 1);
    assert!(parsed.rows[0].is_err());
}
