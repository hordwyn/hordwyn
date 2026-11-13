//! Review BOOK-11 and BOOK-9 on Postgres: the two statements behind `cli refresh-failed` and
//! `cli match-books` (which items they name, in which order), and the log of applied
//! matches.

mod common;

use app::NewCatalogItem;
use app::ports::CatalogRepo;
use domain::catalog::{CatalogItem, EntityState, ExternalId};
use domain::ids::ItemId;
use domain::kind::KindId;

async fn stub(app: &app::App, kind: &str, title: &str, id: (&str, &str)) -> CatalogItem {
    app.find_or_create_catalog_item(NewCatalogItem {
        kind: KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: None,
        description: None,
        external_ids: vec![ExternalId::new(id.0, id.1)],
        attributes: serde_json::json!({}),
        cover_url: None,
        popularity: None,
    })
    .await
    .unwrap()
}

fn sorted(mut ids: Vec<ItemId>) -> Vec<ItemId> {
    ids.sort_by_key(|i| i.0);
    ids
}

/// `unfetched_items`: an item in state `failed`, and an item that was never fetched and is
/// in somebody's library; not a never fetched search row nobody added, and not an item that
/// was fetched. The kind filter narrows it.
#[tokio::test]
async fn unfetched_items_are_failed_ones_and_never_fetched_ones_in_a_library() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "alice").await;
    let book = KindId::new("book").unwrap();
    let failed = stub(app, "book", "Failed", ("openlibrary", "OL1W")).await;
    let imported = stub(app, "book", "Imported", ("openlibrary", "OL2W")).await;
    let search_row = stub(app, "book", "Nobody added it", ("openlibrary", "OL3W")).await;
    let film = stub(app, "movie", "Failed film", ("tmdb", "11")).await;
    let fetched = stub(app, "book", "Fetched", ("openlibrary", "OL4W")).await;
    let answer = app::CatalogRefresh {
        title: "Fetched".into(),
        original_title: None,
        year: None,
        description: None,
        attributes: serde_json::json!({}),
        external_ids: vec![],
        cover_url: None,
        provenance: std::collections::BTreeMap::new(),
        fresh_for_secs: 60,
        popularity: None,
    };
    app.apply_catalog_partial(fetched.id, answer).await.unwrap();
    for i in [&failed, &film] {
        let mut item = app.catalog_item(i.id).await.unwrap();
        item.state = EntityState::Failed;
        CatalogRepo::upsert(&env.store, &item).await.unwrap();
    }
    for i in [&imported, &fetched] {
        app.add_to_library(me, i.id, None).await.unwrap();
    }
    assert!(
        app.catalog_item(imported.id)
            .await
            .unwrap()
            .fetched_at
            .is_none()
    );
    assert!(
        app.catalog_item(fetched.id)
            .await
            .unwrap()
            .fetched_at
            .is_some()
    );

    let all = CatalogRepo::unfetched_items(&env.store, None)
        .await
        .unwrap();
    assert_eq!(all, sorted(vec![failed.id, imported.id, film.id]));
    assert!(!all.contains(&search_row.id));
    let books = CatalogRepo::unfetched_items(&env.store, Some(&book)).await;
    assert_eq!(books.unwrap(), sorted(vec![failed.id, imported.id]));
    // the entry is removed: the import stub is a row nobody holds again
    app.remove_from_library(me, imported.id).await.unwrap();
    let books = CatalogRepo::unfetched_items(&env.store, Some(&book)).await;
    assert_eq!(books.unwrap(), [failed.id]);
}

/// `items_without_namespace`: the items of one kind that hold no id of the namespace,
/// whatever other ids they hold, by title.
#[tokio::test]
async fn items_without_a_namespace_are_named_by_title() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let book = KindId::new("book").unwrap();
    let zeta = stub(app, "book", "Zeta", ("google_books", "g1")).await;
    let alpha = stub(app, "book", "Alpha", ("google_books", "g2")).await;
    let held = stub(app, "book", "Held", ("openlibrary", "OL1W")).await;
    let film = stub(app, "movie", "A film", ("tmdb", "11")).await;
    let none = CatalogRepo::items_without_namespace(&env.store, &book, "openlibrary").await;
    let none = none.unwrap();
    assert_eq!(none, [alpha.id, zeta.id]);
    assert!(!none.contains(&held.id) && !none.contains(&film.id));
    let other = CatalogRepo::items_without_namespace(&env.store, &book, "google_books").await;
    assert_eq!(other.unwrap(), [held.id]);
}

/// The log of `match-books --apply`: each line holds the item as it was, oldest first.
#[tokio::test]
async fn applied_matches_are_logged_with_the_item_as_it_was() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let mut dune = stub(app, "book", "Dune.", ("google_books", "g1")).await;
    dune.year = Some(1965);
    let emma = stub(app, "book", "Emma", ("google_books", "g2")).await;
    assert_eq!(env.store.catalog_matches().await.unwrap(), []);
    for (item, id) in [(&dune, "OL893415W"), (&emma, "OL66W")] {
        env.store
            .log_catalog_match(item, "openlibrary", id)
            .await
            .unwrap();
    }
    let log = env.store.catalog_matches().await.unwrap();
    assert_eq!(log.len(), 2);
    assert_eq!(
        (
            log[0].item_id,
            log[0].external_id.as_str(),
            log[0].source.as_str()
        ),
        (dune.id, "OL893415W", "openlibrary")
    );
    assert_eq!(
        (log[0].title_before.as_str(), log[0].year_before),
        ("Dune.", Some(1965))
    );
    assert_eq!(
        (log[1].title_before.as_str(), log[1].year_before),
        ("Emma", None)
    );
    // the whole item can be read back and written again
    let before: CatalogItem = serde_json::from_value(log[0].before.clone()).unwrap();
    assert_eq!(before, dune);
}
