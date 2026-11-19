//! v1-books: what a person set on a catalog item survives a refresh (v1-plan.md 7, B3), the
//! page total of a book can be corrected (3.1.3), and failed stubs get another refresh
//! (3.1.4, B1).

mod common;

use app::ports::{Clock, Lane};
use app::{CatalogRefresh, NewCatalogItem, REFRESH_JOB, USER_PROVENANCE};
use chrono::Duration;
use domain::catalog::{CatalogItem, EntityState, ExternalId};
use domain::kind::KindId;
use serde_json::json;
use testkit::TestEnv;

fn from_source(pages: u32) -> CatalogRefresh {
    CatalogRefresh {
        title: "Dune".into(),
        original_title: None,
        year: Some(1965),
        description: Some("Sand.".into()),
        attributes: json!({"authors": ["Frank Herbert"], "page_count": pages}),
        external_ids: vec![],
        cover_url: Some("https://covers.example/1-L.jpg".into()),
        provenance: [
            ("title", "openlibrary"),
            ("attributes.authors", "openlibrary"),
            ("attributes.page_count", "openlibrary"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
        fresh_for_secs: 3600,
        popularity: None,
    }
}

async fn stub(env: &TestEnv, kind: &str, title: &str, ids: &[(&str, &str)]) -> CatalogItem {
    env.app
        .find_or_create_catalog_item(NewCatalogItem {
            kind: KindId::new(kind).unwrap(),
            title: title.into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: ids.iter().map(|(n, v)| ExternalId::new(*n, *v)).collect(),
            attributes: json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap()
}

async fn load(env: &TestEnv, id: domain::ids::ItemId) -> CatalogItem {
    env.app.catalog_item(id).await.unwrap()
}

#[tokio::test]
async fn a_refresh_replaces_a_page_count_from_the_source() {
    let env = TestEnv::new();
    let book = stub(&env, "book", "Dune", &[("openlibrary", "OL893415W")]).await;
    let first = env
        .app
        .apply_catalog_refresh(book.id, from_source(608))
        .await
        .unwrap();
    assert_eq!(first.attributes["page_count"], 608);
    assert_eq!(first.provenance["attributes.page_count"], "openlibrary");
    let second = env
        .app
        .apply_catalog_refresh(book.id, from_source(612))
        .await
        .unwrap();
    assert_eq!(second.attributes["page_count"], 612);
    // an attribute the source no longer gives is gone, as before
    let mut bare = from_source(1);
    bare.attributes = json!({"authors": ["Frank Herbert"]});
    bare.provenance.remove("attributes.page_count");
    let third = env.app.apply_catalog_refresh(book.id, bare).await.unwrap();
    assert_eq!(third.attributes.get("page_count"), None);
}

#[tokio::test]
async fn a_refresh_keeps_a_page_count_a_person_set() {
    let env = TestEnv::new();
    let book = stub(&env, "book", "Dune", &[("openlibrary", "OL893415W")]).await;
    env.app
        .apply_catalog_refresh(book.id, from_source(608))
        .await
        .unwrap();
    env.events.clear();
    let set = env.app.set_item_page_count(book.id, 577).await.unwrap();
    assert_eq!(set.attributes["page_count"], 577);
    assert_eq!(set.provenance["attributes.page_count"], USER_PROVENANCE);
    assert_eq!(set.attributes["authors"], json!(["Frank Herbert"]));
    assert_eq!(set.state, EntityState::Full, "nothing else changes");
    assert_eq!(
        env.app.catalog_item(book.id).await.unwrap().attributes["page_count"],
        577,
        "stored"
    );

    let after = env
        .app
        .apply_catalog_refresh(book.id, from_source(612))
        .await
        .unwrap();
    assert_eq!(
        after.attributes["page_count"], 577,
        "the person's value stays"
    );
    assert_eq!(after.provenance["attributes.page_count"], USER_PROVENANCE);
    assert_eq!(after.provenance["attributes.authors"], "openlibrary");
    // also when the source gives no page count at all
    let mut bare = from_source(1);
    bare.attributes = json!({"authors": ["F. Herbert"]});
    bare.provenance.remove("attributes.page_count");
    let after = env.app.apply_catalog_refresh(book.id, bare).await.unwrap();
    assert_eq!(after.attributes["page_count"], 577);
    assert_eq!(after.attributes["authors"], json!(["F. Herbert"]));
}

/// The rule is by field, for every field (v1-manual sets title, year and cover the same way).
#[tokio::test]
async fn a_refresh_keeps_every_field_a_person_set() {
    let env = TestEnv::new();
    let book = stub(&env, "book", "Dune (my copy)", &[("openlibrary", "OL1W")]).await;
    let mut item = env.app.catalog_item(book.id).await.unwrap();
    item.year = Some(1984);
    item.cover_url = Some("https://example.org/mine.jpg".into());
    for f in ["title", "year", "cover_url"] {
        item.provenance.insert(f.into(), USER_PROVENANCE.into());
    }
    env.app.repos.catalog.upsert(&item).await.unwrap();
    let after = env
        .app
        .apply_catalog_refresh(book.id, from_source(608))
        .await
        .unwrap();
    assert_eq!(after.title, "Dune (my copy)");
    assert_eq!(after.year, Some(1984));
    assert_eq!(
        after.cover_url.as_deref(),
        Some("https://example.org/mine.jpg")
    );
    assert_eq!(after.provenance["title"], USER_PROVENANCE);
    assert_eq!(
        after.description.as_deref(),
        Some("Sand."),
        "the rest follows the source"
    );
    assert_eq!(after.attributes["page_count"], 608);
}

#[tokio::test]
async fn a_page_count_is_a_positive_number_on_a_paged_kind() {
    let env = TestEnv::new();
    let book = stub(&env, "book", "Dune", &[("openlibrary", "OL893415W")]).await;
    let movie = stub(&env, "movie", "Dune", &[("tmdb", "438631")]).await;
    assert!(env.app.set_item_page_count(book.id, 0).await.is_err());
    assert!(env.app.set_item_page_count(book.id, 100_001).await.is_err());
    assert!(env.app.set_item_page_count(movie.id, 300).await.is_err());
    let unchanged = env.app.catalog_item(book.id).await.unwrap();
    assert_eq!(unchanged.attributes.get("page_count"), None);
    // a title-only stub takes it too, and stays a stub that is still to be fetched
    let set = env.app.set_item_page_count(book.id, 300).await.unwrap();
    assert_eq!(set.attributes["page_count"], 300);
    assert_eq!(set.state, book.state);
}

async fn fail(env: &TestEnv, id: domain::ids::ItemId) {
    let mut item = load(env, id).await;
    item.state = EntityState::Failed;
    item.fresh_until = Some(env.clock.now() + Duration::hours(1));
    env.app.repos.catalog.upsert(&item).await.unwrap();
}

#[tokio::test]
async fn refresh_failed_requeues_only_items_a_registered_source_knows() {
    let env = TestEnv::new();
    let (_, alice) = common::user(&env, "alice").await;
    // a stub whose refresh failed: never refreshed again on its own
    let known = stub(&env, "book", "Known", &[("openlibrary", "OL1W")]).await;
    let other = stub(&env, "book", "Other source", &[("google_books", "abc")]).await;
    let film = stub(&env, "movie", "Film", &[("tmdb", "11")]).await;
    for i in [&known, &other, &film] {
        fail(&env, i.id).await;
    }
    // an import stub: `partial`, never fetched, in a library; its last try failed too
    let imported = stub(&env, "book", "Imported", &[("openlibrary", "OL3W")]).await;
    env.app
        .add_to_library(alice, imported.id, None)
        .await
        .unwrap();
    let waiting = env
        .app
        .mark_catalog_refresh_failed(imported.id, 3600)
        .await
        .unwrap();
    assert_eq!(waiting.state, EntityState::Partial);
    assert!(!waiting.needs_refresh(env.clock.now()));
    // a search result nobody added, and a book that has its data
    let searched = stub(&env, "book", "Searched", &[("openlibrary", "OL4W")]).await;
    let fine = stub(&env, "book", "Fine", &[("openlibrary", "OL2W")]).await;
    env.app.add_to_library(alice, fine.id, None).await.unwrap();
    env.app
        .apply_catalog_refresh(fine.id, from_source(100))
        .await
        .unwrap();
    env.jobs.drain();
    let sources = ["openlibrary".to_string(), "tmdb".to_string()];
    let ids = |items: &[CatalogItem]| {
        let mut ids: Vec<_> = items.iter().map(|i| i.id).collect();
        ids.sort_by_key(|i| i.0);
        ids
    };
    let sorted = |mut ids: Vec<domain::ids::ItemId>| {
        ids.sort_by_key(|i| i.0);
        ids
    };

    // only books: the film waits
    let book = KindId::new("book").unwrap();
    let again = env.app.requeue_failed(Some(&book), &sources).await.unwrap();
    assert_eq!(ids(&again), sorted(vec![known.id, imported.id]));
    let k = load(&env, known.id).await;
    assert_eq!(k.state, EntityState::Stub);
    assert!(k.needs_refresh(env.clock.now()), "due at once");
    let i = load(&env, imported.id).await;
    assert_eq!(i.state, EntityState::Partial);
    assert!(i.needs_refresh(env.clock.now()), "due at once");
    assert_eq!(
        load(&env, other.id).await.state,
        EntityState::Failed,
        "no source knows it"
    );
    assert_eq!(
        load(&env, film.id).await.state,
        EntityState::Failed,
        "another kind"
    );
    assert_eq!(
        load(&env, fine.id).await.state,
        EntityState::Full,
        "has its data"
    );
    assert_eq!(
        load(&env, searched.id).await.fresh_until,
        searched.fresh_until
    );
    let jobs = env.jobs.drain();
    assert_eq!(jobs.len(), 2);
    for j in &jobs {
        assert_eq!(j.kind, REFRESH_JOB);
        assert_eq!(j.lane, Lane::BackgroundRefresh);
    }
    let mut asked: Vec<String> = jobs
        .iter()
        .map(|j| j.payload["item_id"].to_string())
        .collect();
    asked.sort();
    let mut want = vec![json!(known.id).to_string(), json!(imported.id).to_string()];
    want.sort();
    assert_eq!(asked, want);

    // every kind: the film too (a stub that is still unfetched is asked again)
    let again = env.app.requeue_failed(None, &sources).await.unwrap();
    assert!(ids(&again).contains(&film.id));
    assert_eq!(load(&env, film.id).await.state, EntityState::Stub);
    assert!(!ids(&again).contains(&other.id));
}

/// The batch of `cli refresh-failed` stores what a search row holds; the description follows
/// with the normal refresh when the page is opened.
#[tokio::test]
async fn a_partial_fill_leaves_the_item_due_for_its_full_refresh() {
    let env = TestEnv::new();
    let book = stub(&env, "book", "dune", &[("openlibrary", "OL893415W")]).await;
    fail(&env, book.id).await;
    env.app.set_item_page_count(book.id, 577).await.unwrap();
    let mut row = from_source(608);
    row.description = None;
    let filled = env.app.apply_catalog_partial(book.id, row).await.unwrap();
    assert_eq!(filled.state, EntityState::Partial);
    assert_eq!(filled.title, "Dune");
    assert_eq!(filled.attributes["authors"], json!(["Frank Herbert"]));
    assert_eq!(
        filled.attributes["page_count"], 577,
        "a person's value stays here too"
    );
    assert_eq!(
        filled.cover_url.as_deref(),
        Some("https://covers.example/1-L.jpg")
    );
    assert!(filled.needs_refresh(env.clock.now()));
    assert!(filled.needs_refresh(env.clock.now() + Duration::days(30)));

    let without = env
        .app
        .items_without_id_from(&KindId::new("book").unwrap(), "google_books")
        .await
        .unwrap();
    assert_eq!(without.iter().map(|i| i.id).collect::<Vec<_>>(), [book.id]);
    assert_eq!(
        env.app
            .items_without_id_from(&KindId::new("book").unwrap(), "openlibrary")
            .await
            .unwrap()
            .len(),
        0
    );
}

/// Review BOOK-5: a paged kind whose attributes hold no `page_count` (manga counts
/// chapters) takes no page total, and says so in the words of the rule, not of the
/// attribute check.
#[tokio::test]
async fn a_paged_kind_without_a_page_count_takes_no_total() {
    let env = TestEnv::with_kinds(std::sync::Arc::new(kinds::Registry::with_builtins()));
    let manga = stub(&env, "manga", "Akira", &[("anilist", "30664")]).await;
    let err = env
        .app
        .set_item_page_count(manga.id, 180)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("this kind has no pages"), "{err}");
    let unchanged = env.app.catalog_item(manga.id).await.unwrap();
    assert_eq!(unchanged.attributes.get("page_count"), None);
    assert_eq!(unchanged.provenance.get("attributes.page_count"), None);
    // book and comic have the attribute
    for kind in ["book", "comic"] {
        let item = stub(&env, kind, "Paged", &[("test", kind)]).await;
        let set = env.app.set_item_page_count(item.id, 180).await.unwrap();
        assert_eq!(set.attributes["page_count"], 180, "{kind}");
    }
}

/// `cli refresh-source tvdb` (#3): every stored item with an id of the source is listed;
/// `--apply` makes each due and queues its refresh. Items of other sources stay.
#[tokio::test]
async fn requeue_source_queues_every_item_with_an_id_of_that_source() {
    let env = TestEnv::new();
    let ids = |items: &[CatalogItem]| items.iter().map(|i| i.id).collect::<Vec<_>>();
    let new = |title: &str, ns: &str| NewCatalogItem {
        kind: KindId::new("show").unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2011),
        description: None,
        external_ids: vec![ExternalId::new(ns, title)],
        attributes: json!({}),
        cover_url: None,
        popularity: None,
    };
    let show = env
        .app
        .upsert_catalog_item(new("Unforgettable", "tvdb"))
        .await
        .unwrap();
    let other = env
        .app
        .upsert_catalog_item(new("Dark", "tmdb"))
        .await
        .unwrap();
    // a search result nobody added is left out (staging of 1.0.1-rc.1 listed 2055 of them)
    let searched = env
        .app
        .upsert_catalog_item(new("Arnau", "tvdb"))
        .await
        .unwrap();
    let (_, me) = common::user(&env, "robin").await;
    for i in [show.id, other.id] {
        env.app.add_to_library(me, i, None).await.unwrap();
    }
    env.jobs.drain();
    let dry = env.app.requeue_source("tvdb", false).await.unwrap();
    assert_eq!(ids(&dry), vec![show.id]);
    assert!(env.jobs.drain().is_empty(), "a dry run queues nothing");
    let done = env.app.requeue_source("tvdb", true).await.unwrap();
    assert_eq!(ids(&done), vec![show.id]);
    let jobs = env.jobs.drain();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, REFRESH_JOB);
    assert_eq!(jobs[0].payload["item_id"], json!(show.id));
    assert!(
        load(&env, show.id).await.needs_refresh(env.clock.now()),
        "due at once"
    );
    assert_eq!(
        load(&env, other.id).await.fresh_until,
        other.fresh_until,
        "another source"
    );
    assert_eq!(
        load(&env, searched.id).await.fresh_until,
        searched.fresh_until,
        "in nobody's library"
    );
}
