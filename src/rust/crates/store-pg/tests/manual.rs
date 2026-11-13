//! v1-manual on Postgres: the purge of connector data keeps what a person set (review
//! BOOK-10 of v1-books), a purged item with a kept title is still refetched, and the lookup
//! of items by title (migration 0037).

mod common;

use app::{CatalogRefresh, ItemDetails, NewCatalogItem, USER_PROVENANCE};
use chrono::Duration;
use domain::catalog::{CatalogItem, EntityState, ExternalId};
use domain::kind::KindId;

fn kind(id: &str) -> KindId {
    KindId::new(id).unwrap()
}

fn details(title: &str, year: Option<i32>, cover: Option<&str>) -> ItemDetails {
    ItemDetails {
        title: title.into(),
        year,
        cover_url: cover.map(Into::into),
    }
}

fn never(_: &CatalogItem) -> bool {
    false
}

fn refresh() -> CatalogRefresh {
    CatalogRefresh {
        title: "Source title".into(),
        original_title: Some("Quelltitel".into()),
        year: Some(1995),
        description: Some("From the source.".into()),
        attributes: serde_json::json!({"runtime": 100, "page_count": 10}),
        external_ids: vec![],
        cover_url: Some("https://img.example/source.jpg".into()),
        provenance: [
            ("title", "tmdb"),
            ("year", "tmdb"),
            ("cover_url", "tmdb"),
            ("attributes.runtime", "tmdb"),
        ]
        .into_iter()
        .map(|(f, s)| (f.to_string(), s.to_string()))
        .collect(),
        fresh_for_secs: 60,
        popularity: Some(10.0),
    }
}

#[tokio::test]
async fn the_purge_keeps_what_a_person_set_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let ns = vec!["tmdb".to_string()];
    let item = corrected_tmdb_item(app).await;

    app.purge_provider_data(item.id).await.unwrap();
    let purged = app.catalog_item(item.id).await.unwrap();
    assert_eq!(purged.title, "My title");
    assert_eq!(
        purged.cover_url.as_deref(),
        Some("https://img.example/mine.jpg")
    );
    assert_eq!(purged.attributes, serde_json::json!({"page_count": 321}));
    assert_eq!(purged.year, None);
    assert_eq!(purged.description, None);
    assert_eq!(purged.original_title, None);
    assert_eq!(purged.cover_asset, None);
    assert_eq!(purged.popularity, None);
    assert_eq!(purged.state, EntityState::Stub);
    assert_eq!(purged.fetched_at, None);
    assert_eq!(
        purged
            .provenance
            .iter()
            .map(|(f, s)| (f.as_str(), s.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("attributes.page_count", USER_PROVENANCE),
            ("cover_url", USER_PROVENANCE),
            ("title", USER_PROVENANCE),
        ]
    );
    assert_eq!(purged.external_ids, vec![ExternalId::new("tmdb", "42")]);

    // FixedClock stands still: the item is due at once, and stays due through writes that
    // are no fetch (a failed attempt, another correction)
    let soon = chrono::Utc::now() + Duration::days(1);
    assert_eq!(
        app.expired_provider_data(&ns, soon, 10).await.unwrap(),
        vec![item.id]
    );
    app.mark_catalog_refresh_failed(item.id, 60).await.unwrap();
    app.edit_item_details(item.id, details("My title 2", None, None), true, &never)
        .await
        .unwrap();
    assert_eq!(
        app.expired_provider_data(&ns, soon, 10).await.unwrap(),
        vec![item.id]
    );
    // the refetch ends the wait and leaves the person's fields alone
    let back = app.apply_catalog_refresh(item.id, refresh()).await.unwrap();
    assert_eq!(back.title, "My title 2");
    assert_eq!(back.cover_url, None, "removed by the person");
    assert_eq!(back.attributes["page_count"], 321);
    assert_eq!(back.attributes["runtime"], 100);
    assert_eq!(back.description.as_deref(), Some("From the source."));
    assert_eq!(
        app.expired_provider_data(&ns, soon, 10).await.unwrap(),
        vec![]
    );
}

#[tokio::test]
async fn a_purge_without_fields_of_a_person_empties_the_maps_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let item = common::item(app, "movie", "Plain").await;
    app.apply_catalog_refresh(item.id, refresh()).await.unwrap();
    app.purge_provider_data(item.id).await.unwrap();
    let purged = app.catalog_item(item.id).await.unwrap();
    assert!(purged.title_unavailable());
    assert_eq!(purged.attributes, serde_json::json!({}));
    assert_eq!(purged.provenance.len(), 0);
    assert_eq!(purged.cover_url, None);
}

#[tokio::test]
async fn a_hand_entered_item_is_stored_and_found_by_its_title_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let first = app
        .create_manual_item(
            kind("board_game"),
            details("Catan", Some(1995), Some("https://img.example/c.jpg")),
            true,
        )
        .await
        .unwrap();
    assert_eq!(app.catalog_item(first.id).await.unwrap(), first);
    app.create_manual_item(kind("book"), details("Catan", None, None), true)
        .await
        .unwrap();
    app.create_manual_item(
        kind("board_game"),
        details("Catan Junior", None, None),
        true,
    )
    .await
    .unwrap();
    for typed in ["catan", "  CATAN ", "Catan"] {
        let found = app.items_titled(&kind("board_game"), typed).await.unwrap();
        assert_eq!(
            found.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![first.id],
            "{typed}"
        );
        assert_eq!(
            found[0], first,
            "the whole item, ids and provenance included"
        );
    }
    // the index of migration 0037 holds the compared expression
    assert_eq!(
        common::count(
            &env.store,
            "SELECT count(*) FROM pg_indexes WHERE indexname = 'catalog_items_kind_title'
               AND indexdef LIKE '%lower(btrim(title))%'"
        )
        .await,
        1
    );
    // the outbox carries the new item to the search index
    assert_eq!(
        common::count(
            &env.store,
            &format!(
                "SELECT count(*) FROM outbox WHERE entity = 'catalog_item' AND entity_id = '{}'",
                first.id.0
            )
        )
        .await,
        1
    );
}

/// The library documents of the search index carry the item's title, original title and
/// year (the Library's text filter reads them): a corrected value must reach them, so the
/// write of an item where one of the three changed queues its library entries for the index
/// as well (LC-36; the year: review MAN-6).
#[tokio::test]
async fn a_changed_title_reindexes_the_library_entries_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (_, me) = common::user(app, "alice").await;
    let item = app
        .create_manual_item(kind("board_game"), details("Marsh", None, None), true)
        .await
        .unwrap();
    let entry = app.add_to_library(me, item.id, None).await.unwrap();
    let queued = || async {
        common::count(
            &env.store,
            &format!(
                "SELECT count(*) FROM outbox WHERE entity = 'library_entry' AND entity_id = '{}'",
                entry.id.0
            ),
        )
        .await
    };
    let before = queued().await;
    // a write that leaves title, original title and year alone queues nothing for the
    // entries (the picture is not in their documents)
    let picture = Some("https://img.example/marsh.jpg");
    app.edit_item_details(item.id, details("Marsh", None, picture), true, &never)
        .await
        .unwrap();
    assert_eq!(queued().await, before);
    // a corrected year alone
    app.edit_item_details(item.id, details("Marsh", Some(2019), picture), true, &never)
        .await
        .unwrap();
    assert_eq!(queued().await, before + 1, "the year reaches the entry");
    let before = queued().await;
    app.edit_item_details(
        item.id,
        details("Marsh, second printing", Some(2019), None),
        true,
        &never,
    )
    .await
    .unwrap();
    assert_eq!(queued().await, before + 1, "the entry is indexed again");
}

/// A film of `tmdb`, refreshed, with a stored cover; a person then corrected its title and
/// cover address and set one attribute (as the page total of v1-books).
async fn corrected_tmdb_item(app: &app::App) -> CatalogItem {
    let item = app
        .upsert_catalog_item(NewCatalogItem {
            kind: kind("movie"),
            title: "Source title".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![ExternalId::new("tmdb", "42")],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    app.apply_catalog_refresh(item.id, refresh()).await.unwrap();
    app.set_cover_asset(item.id, "asset-1".into())
        .await
        .unwrap();
    app.edit_item_details(
        item.id,
        details("My title", Some(1995), Some("https://img.example/mine.jpg")),
        true,
        &never,
    )
    .await
    .unwrap();
    // one attribute of the person (as the page total of v1-books)
    let mut stored = app.catalog_item(item.id).await.unwrap();
    stored.attributes = serde_json::json!({"runtime": 100, "page_count": 321});
    stored
        .provenance
        .insert("attributes.page_count".into(), USER_PROVENANCE.into());
    app.repos.catalog.upsert(&stored).await.unwrap();
    app.catalog_item(item.id).await.unwrap()
}
