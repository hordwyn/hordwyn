//! v1-manual: an item entered by hand, "Edit details", and the fields a person set surviving
//! every writer of the shared item (v1-plan.md 4a; review BOOK-10 of v1-books).

mod common;

use app::{
    AppError, CatalogRefresh, ItemDetails, MAX_COVER_URL_CHARS, NewCatalogItem, USER_PROVENANCE,
};
use chrono::Datelike;
use domain::catalog::{CatalogItem, EntityState, ExternalId};
use domain::kind::KindId;
use testkit::TestEnv;

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

fn always(_: &CatalogItem) -> bool {
    true
}

fn invalid_field(e: &AppError) -> Option<&'static str> {
    match e {
        AppError::Domain(domain::DomainError::Invalid { field, .. }) => Some(field),
        _ => None,
    }
}

fn refresh(title: &str) -> CatalogRefresh {
    CatalogRefresh {
        title: title.into(),
        original_title: None,
        year: Some(1995),
        description: Some("From the source.".into()),
        attributes: serde_json::json!({}),
        external_ids: vec![],
        cover_url: Some("https://img.example/source.jpg".into()),
        provenance: [("title", "src"), ("year", "src"), ("cover_url", "src")]
            .into_iter()
            .map(|(f, s)| (f.to_string(), s.to_string()))
            .collect(),
        fresh_for_secs: 60,
        popularity: None,
    }
}

#[tokio::test]
async fn a_member_creates_an_item_by_hand() {
    let env = TestEnv::new();
    let item = env
        .app
        .create_manual_item(
            kind("board_game"),
            details(
                "  Zine of the Marsh ",
                Some(2019),
                Some("https://img.example/box.jpg"),
            ),
            true,
        )
        .await
        .unwrap();
    assert_eq!(item.title, "Zine of the Marsh", "trimmed");
    assert_eq!(item.year, Some(2019));
    assert_eq!(
        item.cover_url.as_deref(),
        Some("https://img.example/box.jpg")
    );
    assert_eq!(item.state, EntityState::Full);
    assert_eq!(item.external_ids.len(), 0);
    assert_eq!(item.attributes, serde_json::json!({}));
    assert_eq!(item.cover_asset, None);
    for field in ["title", "year", "cover_url"] {
        assert_eq!(item.provenance[field], USER_PROVENANCE, "{field}");
    }
    assert_eq!(env.app.catalog_item(item.id).await.unwrap(), item, "stored");
}

#[tokio::test]
async fn a_blank_or_too_long_title_is_refused() {
    let env = TestEnv::new();
    for title in [String::new(), "   ".to_string(), "x".repeat(501)] {
        let err = env
            .app
            .create_manual_item(kind("book"), details(&title, None, None), true)
            .await
            .unwrap_err();
        assert_eq!(invalid_field(&err), Some("title"), "{err:?}");
    }
    env.app
        .create_manual_item(kind("book"), details(&"x".repeat(500), None, None), true)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_year_is_between_one_and_five_years_from_now() {
    let env = TestEnv::new();
    let this_year = env.clock.now_for_tests().year();
    for year in [0, -3, this_year + 6] {
        let err = env
            .app
            .create_manual_item(kind("book"), details("A", Some(year), None), true)
            .await
            .unwrap_err();
        assert_eq!(invalid_field(&err), Some("year"), "{year}: {err:?}");
    }
    for year in [1, this_year + 5] {
        env.app
            .create_manual_item(kind("book"), details("A", Some(year), None), true)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn a_cover_address_must_be_https() {
    let env = TestEnv::new();
    let long = format!("https://img.example/{}", "a".repeat(MAX_COVER_URL_CHARS));
    for cover in [
        "http://img.example/a.jpg",
        "ftp://img.example/a.jpg",
        "//img.example/a.jpg",
        "img.example/a.jpg",
        "https://",
        "https:///a.jpg",
        "https://user:secret@img.example/a.jpg",
        "https://user@img.example/a.jpg",
        "https://img.example/a b.jpg",
        "https://img.example/a\".jpg",
        "https://img.example/a\n.jpg",
        "javascript:alert(1)",
        "data:image/png;base64,AAAA",
        long.as_str(),
    ] {
        let err = env
            .app
            .create_manual_item(kind("book"), details("A", None, Some(cover)), true)
            .await
            .unwrap_err();
        assert_eq!(invalid_field(&err), Some("cover_url"), "{cover}: {err:?}");
    }
    // an at sign after the host is part of the path
    let ok = env
        .app
        .create_manual_item(
            kind("book"),
            details("A", None, Some(" HTTPS://img.example/a@2x.jpg?w=1 ")),
            true,
        )
        .await
        .unwrap();
    assert_eq!(
        ok.cover_url.as_deref(),
        Some("HTTPS://img.example/a@2x.jpg?w=1"),
        "trimmed"
    );
    // an empty field is no cover
    let none = env
        .app
        .create_manual_item(kind("book"), details("B", None, Some("  ")), true)
        .await
        .unwrap();
    assert_eq!(none.cover_url, None);
}

#[tokio::test]
async fn a_disabled_kind_is_refused() {
    let env = TestEnv::new();
    // the kind catalog holds the enabled kinds only
    let err = env
        .app
        .create_manual_item(kind("comic"), details("A", None, None), true)
        .await
        .unwrap_err();
    assert_eq!(invalid_field(&err), Some("kind"), "{err:?}");
}

#[tokio::test]
async fn hosted_mode_refuses_a_members_cover_address() {
    let env = TestEnv::new();
    // the caller says whether the actor may choose an address other browsers load
    let err = env
        .app
        .create_manual_item(
            kind("book"),
            details("A", None, Some("https://img.example/a.jpg")),
            false,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "{err:?}");
    // without an address the member may create
    env.app
        .create_manual_item(kind("book"), details("A", None, None), false)
        .await
        .unwrap();
}

#[tokio::test]
async fn an_item_without_ids_is_not_queued_for_a_refresh() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let item = env
        .app
        .create_manual_item(kind("board_game"), details("Marsh", None, None), true)
        .await
        .unwrap();
    env.app.item_detail(item.id).await.unwrap();
    env.app.item_detail_for(me, item.id).await.unwrap();
    assert_eq!(env.jobs.jobs().len(), 0, "nobody could answer");

    // a title-only stub of an import (no ids either) is not queued, whatever its state
    let stub = env
        .app
        .find_or_create_catalog_item(NewCatalogItem {
            kind: kind("book"),
            title: "Stub".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    env.app.item_detail(stub.id).await.unwrap();
    assert_eq!(env.jobs.jobs().len(), 0);
}

#[tokio::test]
async fn an_item_only_unknown_sources_name_is_not_queued() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    // an id in a namespace no registered source answers for (a kept import stub)
    let item = common::item(&env.app, "book", "Kept").await;
    env.app
        .item_detail_where(item.id, Some(me), &never)
        .await
        .unwrap();
    assert_eq!(env.jobs.jobs().len(), 0);
    env.app
        .item_detail_where(item.id, Some(me), &always)
        .await
        .unwrap();
    assert_eq!(env.jobs.jobs().len(), 1, "a source that knows it is asked");
}

#[tokio::test]
async fn a_refresh_keeps_title_year_and_cover_a_person_set() {
    let env = TestEnv::new();
    let item = env
        .app
        .create_manual_item(
            kind("movie"),
            details("My title", Some(2001), Some("https://img.example/mine.jpg")),
            true,
        )
        .await
        .unwrap();
    // the item later gets a source id and is refreshed from it
    let mut r = refresh("Source title");
    r.external_ids = vec![ExternalId::new("src", "7")];
    let after = env.app.apply_catalog_refresh(item.id, r).await.unwrap();
    assert_eq!(after.title, "My title");
    assert_eq!(after.year, Some(2001));
    assert_eq!(
        after.cover_url.as_deref(),
        Some("https://img.example/mine.jpg")
    );
    for field in ["title", "year", "cover_url"] {
        assert_eq!(after.provenance[field], USER_PROVENANCE, "{field}");
    }
    // what the person did not set comes from the source
    assert_eq!(after.description.as_deref(), Some("From the source."));
    assert_eq!(after.external_ids, vec![ExternalId::new("src", "7")]);
}

#[tokio::test]
async fn a_refresh_keeps_a_year_and_a_cover_a_person_removed() {
    let env = TestEnv::new();
    let item = env
        .app
        .create_manual_item(kind("movie"), details("Mine", None, None), true)
        .await
        .unwrap();
    let after = env
        .app
        .apply_catalog_refresh(item.id, refresh("Source"))
        .await
        .unwrap();
    assert_eq!(after.year, None);
    assert_eq!(after.cover_url, None);
}

fn source_item(title: &str, cover: Option<&str>) -> NewCatalogItem {
    NewCatalogItem {
        kind: kind("movie"),
        title: title.into(),
        original_title: None,
        year: Some(1999),
        description: None,
        external_ids: vec![ExternalId::new("src", "42")],
        attributes: serde_json::json!({"runtime": 100}),
        cover_url: cover.map(Into::into),
        popularity: None,
    }
}

/// An item of a source whose title and cover a person then corrected.
async fn corrected(env: &TestEnv) -> CatalogItem {
    let item = env
        .app
        .upsert_catalog_item(source_item(
            "Source title",
            Some("https://img.example/s.jpg"),
        ))
        .await
        .unwrap();
    // they put their own picture there, then took it away again
    env.app
        .edit_item_details(
            item.id,
            details("My title", Some(1999), Some("https://img.example/mine.jpg")),
            true,
            &never,
        )
        .await
        .unwrap();
    env.app
        .edit_item_details(item.id, details("My title", Some(1999), None), true, &never)
        .await
        .unwrap()
}

/// The dialog shows only an address the person typed. Saved with the field empty, a
/// picture that is not theirs (a source's address, a stored picture) stays as it is: a
/// corrected title must not take the cover away.
#[tokio::test]
async fn an_empty_address_leaves_a_cover_that_is_not_the_persons() {
    let env = TestEnv::new();
    let item = env
        .app
        .upsert_catalog_item(source_item(
            "Source title",
            Some("https://img.example/s.jpg"),
        ))
        .await
        .unwrap();
    env.app
        .set_cover_asset(item.id, "asset-3".into())
        .await
        .unwrap();
    let edited = env
        .app
        .edit_item_details(item.id, details("My title", Some(1999), None), true, &never)
        .await
        .unwrap();
    assert_eq!(edited.title, "My title");
    assert_eq!(
        edited.cover_url.as_deref(),
        Some("https://img.example/s.jpg")
    );
    assert_eq!(edited.cover_asset.as_deref(), Some("asset-3"));
    assert!(!edited.provenance.contains_key("cover_url"));
    // their own address replaces it, and an empty field then removes theirs
    let own = env
        .app
        .edit_item_details(
            item.id,
            details("My title", Some(1999), Some("https://img.example/mine.jpg")),
            true,
            &never,
        )
        .await
        .unwrap();
    assert_eq!(own.cover_asset, None);
    assert_eq!(own.provenance["cover_url"], USER_PROVENANCE);
    let removed = env
        .app
        .edit_item_details(item.id, details("My title", Some(1999), None), true, &never)
        .await
        .unwrap();
    assert_eq!(removed.cover_url, None);
    assert_eq!(removed.provenance["cover_url"], USER_PROVENANCE);
}

#[tokio::test]
async fn the_merge_writer_keeps_what_a_person_set() {
    let env = TestEnv::new();
    let mine = corrected(&env).await;
    assert_eq!(mine.provenance["title"], USER_PROVENANCE);
    assert_eq!(mine.provenance["cover_url"], USER_PROVENANCE);
    assert!(
        !mine.provenance.contains_key("year"),
        "unchanged: not theirs"
    );
    let mut again = source_item("Source title 2", Some("https://img.example/s2.jpg"));
    again.year = Some(2000);
    let after = env.app.upsert_catalog_item(again).await.unwrap();
    assert_eq!(after.id, mine.id);
    assert_eq!(after.title, "My title");
    assert_eq!(after.cover_url, None, "the person removed the cover");
    assert_eq!(
        after.year,
        Some(2000),
        "the source's field follows the source"
    );
    assert_eq!(after.provenance["title"], USER_PROVENANCE);
}

#[tokio::test]
async fn a_search_result_does_not_fill_a_cover_a_person_removed() {
    let env = TestEnv::new();
    let mine = corrected(&env).await;
    let after = env
        .app
        .ensure_catalog_item(source_item(
            "Source title",
            Some("https://img.example/s3.jpg"),
        ))
        .await
        .unwrap();
    assert_eq!(after.id, mine.id);
    assert_eq!(after.cover_url, None);
}

/// The purged items of the source `src` that may be fetched again a day from now.
async fn due_for_refetch(env: &TestEnv) -> Vec<domain::ids::ItemId> {
    env.app
        .expired_provider_data(
            &["src".to_string()],
            env.clock.now_for_tests() + chrono::Duration::days(1),
            10,
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn the_retention_purge_keeps_what_a_person_set() {
    let env = TestEnv::new();
    let item = env
        .app
        .upsert_catalog_item(source_item(
            "Source title",
            Some("https://img.example/s.jpg"),
        ))
        .await
        .unwrap();
    env.app
        .apply_catalog_refresh(
            item.id,
            CatalogRefresh {
                attributes: serde_json::json!({"runtime": 100, "page_count": 10}),
                ..refresh("Source title")
            },
        )
        .await
        .unwrap();
    // a person corrects the title and the cover, and (as v1-books does) one attribute
    env.app
        .edit_item_details(
            item.id,
            details("My title", Some(1995), Some("https://img.example/mine.jpg")),
            true,
            &never,
        )
        .await
        .unwrap();
    let mut stored = env.app.catalog_item(item.id).await.unwrap();
    stored.attributes = serde_json::json!({"runtime": 100, "page_count": 321});
    stored
        .provenance
        .insert("attributes.page_count".into(), USER_PROVENANCE.into());
    stored
        .provenance
        .insert("attributes.runtime".into(), "src".into());
    env.app.repos.catalog.upsert(&stored).await.unwrap();

    env.app.purge_provider_data(item.id).await.unwrap();
    let purged = env.app.catalog_item(item.id).await.unwrap();
    assert_eq!(purged.title, "My title");
    assert_eq!(
        purged.cover_url.as_deref(),
        Some("https://img.example/mine.jpg")
    );
    assert_eq!(purged.attributes, serde_json::json!({"page_count": 321}));
    assert_eq!(purged.year, None, "the source's year goes");
    assert_eq!(purged.description, None);
    assert_eq!(purged.state, EntityState::Stub);
    let kept: Vec<(&str, &str)> = purged
        .provenance
        .iter()
        .map(|(f, s)| (f.as_str(), s.as_str()))
        .collect();
    assert_eq!(
        kept,
        vec![
            ("attributes.page_count", USER_PROVENANCE),
            ("cover_url", USER_PROVENANCE),
            ("title", USER_PROVENANCE),
        ],
        "the map still says whose the kept fields are"
    );
    // the item is still due for its refetch although it has a title
    let due = due_for_refetch(&env).await;
    assert_eq!(due, vec![item.id]);
    // and a failed attempt leaves it due (a write of a titled item used to end the expiry)
    env.app
        .mark_catalog_refresh_failed(item.id, 60)
        .await
        .unwrap();
    let due = due_for_refetch(&env).await;
    assert_eq!(due, vec![item.id]);
    // the refetch brings the source's data back around the person's fields
    let back = env
        .app
        .apply_catalog_refresh(item.id, refresh("Source title"))
        .await
        .unwrap();
    assert_eq!(back.title, "My title");
    assert_eq!(back.year, Some(1995));
    assert_eq!(
        back.cover_url.as_deref(),
        Some("https://img.example/mine.jpg")
    );
    assert_eq!(back.attributes["page_count"], 321);
    let due = due_for_refetch(&env).await;
    assert_eq!(due.len(), 0, "fetched again: no longer expired");
}

#[tokio::test]
async fn edit_details_marks_the_changed_fields_as_the_persons() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let stub = common::item(&env.app, "book", "Kept stub").await;
    env.app.add_to_library(me, stub.id, None).await.unwrap();
    env.app
        .set_cover_asset(stub.id, "asset-9".into())
        .await
        .unwrap();
    let before = env.events.topics().len();
    let edited = env
        .app
        .edit_item_details(
            stub.id,
            details(
                " Kept, corrected ",
                Some(1999),
                Some("https://img.example/c.jpg"),
            ),
            true,
            &never,
        )
        .await
        .unwrap();
    assert_eq!(edited.title, "Kept, corrected");
    assert_eq!(edited.provenance["title"], USER_PROVENANCE);
    assert_eq!(edited.provenance["cover_url"], USER_PROVENANCE);
    assert!(!edited.provenance.contains_key("year"), "1999 was there");
    assert_eq!(
        edited.cover_asset, None,
        "the stored picture was the old one"
    );
    assert_eq!(edited.state, EntityState::Full, "nothing more will come");
    assert_eq!(edited.external_ids, stub.external_ids);
    assert!(
        env.events.topics().len() > before,
        "the open pages hear of it"
    );
    assert_eq!(env.app.catalog_item(stub.id).await.unwrap(), edited);

    // removing the year is a choice too
    let cleared = env
        .app
        .edit_item_details(
            stub.id,
            details("Kept, corrected", None, Some("https://img.example/c.jpg")),
            true,
            &never,
        )
        .await
        .unwrap();
    assert_eq!(cleared.year, None);
    assert_eq!(cleared.provenance["year"], USER_PROVENANCE);
}

#[tokio::test]
async fn edit_details_checks_what_create_checks() {
    let env = TestEnv::new();
    let item = env
        .app
        .create_manual_item(
            kind("book"),
            details("A", None, Some("https://img.example/a.jpg")),
            true,
        )
        .await
        .unwrap();
    let id = item.id;
    let err = env
        .app
        .edit_item_details(id, details(" ", None, None), true, &never)
        .await
        .unwrap_err();
    assert_eq!(invalid_field(&err), Some("title"));
    let err = env
        .app
        .edit_item_details(id, details("A", Some(0), None), true, &never)
        .await
        .unwrap_err();
    assert_eq!(invalid_field(&err), Some("year"));
    let err = env
        .app
        .edit_item_details(
            id,
            details("A", None, Some("http://img.example/a.jpg")),
            true,
            &never,
        )
        .await
        .unwrap_err();
    assert_eq!(invalid_field(&err), Some("cover_url"));
    // who may not choose an address may keep or remove the one that is there, not set one
    let err = env
        .app
        .edit_item_details(
            id,
            details("A", None, Some("https://img.example/other.jpg")),
            false,
            &never,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "{err:?}");
    env.app
        .edit_item_details(
            id,
            details("B", None, Some("https://img.example/a.jpg")),
            false,
            &never,
        )
        .await
        .unwrap();
    let unknown = domain::ids::ItemId(uuid::Uuid::from_u128(999));
    let err = env
        .app
        .edit_item_details(unknown, details("A", None, None), true, &never)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound(_)), "{err:?}");
}

#[tokio::test]
async fn edit_details_is_refused_for_an_item_a_source_answers_for() {
    let env = TestEnv::new();
    let item = common::item(&env.app, "movie", "Sourced").await;
    let err = env
        .app
        .edit_item_details(item.id, details("Mine", None, None), true, &always)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
    assert_eq!(
        env.app.catalog_item(item.id).await.unwrap().title,
        "Sourced"
    );
}

#[tokio::test]
async fn items_with_the_same_title_are_found_before_a_second_entry() {
    let env = TestEnv::new();
    let first = env
        .app
        .create_manual_item(kind("board_game"), details("Catan", Some(1995), None), true)
        .await
        .unwrap();
    env.app
        .create_manual_item(kind("book"), details("Catan", None, None), true)
        .await
        .unwrap();
    env.app
        .create_manual_item(
            kind("board_game"),
            details("Catan Junior", None, None),
            true,
        )
        .await
        .unwrap();
    for typed in ["catan", "  CATAN  ", "Catan"] {
        let found = env
            .app
            .items_titled(&kind("board_game"), typed)
            .await
            .unwrap();
        assert_eq!(
            found.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![first.id],
            "{typed}"
        );
    }
    let none = env
        .app
        .items_titled(&kind("board_game"), " ")
        .await
        .unwrap();
    assert_eq!(none.len(), 0);
    // a second one with the same title is a choice, not an error
    let second = env
        .app
        .create_manual_item(kind("board_game"), details("Catan", None, None), true)
        .await
        .unwrap();
    assert_ne!(second.id, first.id);
    assert_eq!(
        env.app
            .items_titled(&kind("board_game"), "Catan")
            .await
            .unwrap()
            .len(),
        2
    );
}
