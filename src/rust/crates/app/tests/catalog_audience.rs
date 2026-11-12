//! A change of a catalog item reaches the users it matters to, not every signed-in user
//! (DEC-490): an import or refresh elsewhere must not refetch everybody's pages.

mod common;

use app::{CatalogRefresh, NewCatalogItem};
use domain::catalog::{CatalogItem, ExternalId};
use domain::ids::{ItemId, UserId};
use domain::kind::KindId;
use domain::list::{Visibility, VisibilityChoice};
use domain::session::Participant;
use testkit::TestEnv;

fn refresh(title: &str) -> CatalogRefresh {
    CatalogRefresh {
        title: title.into(),
        original_title: None,
        year: Some(2001),
        description: Some("Fresh.".into()),
        attributes: serde_json::json!({}),
        external_ids: vec![],
        cover_url: None,
        provenance: std::collections::BTreeMap::new(),
        fresh_for_secs: 60,
        popularity: None,
    }
}

/// The audiences of the `catalog_item` events for `id` published since the last clear
/// (sorted; an empty audience is the whole instance).
fn audiences(env: &TestEnv, id: ItemId) -> Vec<Vec<UserId>> {
    env.events
        .all()
        .into_iter()
        .filter(|e| {
            e.entity
                .as_ref()
                .is_some_and(|r| r.entity == "catalog_item" && r.id == id.0)
        })
        .map(|e| {
            let mut a = e.audience;
            a.sort_by_key(|u| u.0);
            a
        })
        .collect()
}

fn sorted(mut users: Vec<UserId>) -> Vec<UserId> {
    users.sort_by_key(|u| u.0);
    users
}

/// What an import does with a row: the shared item (found or created), then the entry.
async fn import(env: &TestEnv, actor: domain::user::Actor, title: &str) -> CatalogItem {
    let item = env
        .app
        .find_or_create_catalog_item(NewCatalogItem {
            kind: KindId::new("movie").unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(2001),
            description: None,
            external_ids: vec![ExternalId::new("test", title)],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    env.app.add_to_library(actor, item.id, None).await.unwrap();
    item
}

#[tokio::test]
async fn an_import_elsewhere_reaches_only_those_who_have_the_item() {
    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    env.events.clear();
    // Alice imports an item Bob does not have; the refresh after the import tells Alice only
    let item = import(&env, alice, "Arrival").await;
    env.app
        .apply_catalog_refresh(item.id, refresh("Arrival"))
        .await
        .unwrap();
    env.app
        .set_cover_asset(item.id, "cover-1".into())
        .await
        .unwrap();
    assert_eq!(
        audiences(&env, item.id),
        vec![vec![alice_user.id], vec![alice_user.id]]
    );
    // once Bob has it too, he hears about it
    env.app.add_to_library(bob, item.id, None).await.unwrap();
    env.events.clear();
    env.app
        .apply_catalog_refresh(item.id, refresh("Arrival"))
        .await
        .unwrap();
    assert_eq!(
        audiences(&env, item.id),
        vec![sorted(vec![alice_user.id, bob_user.id])]
    );
}

#[tokio::test]
async fn an_item_nobody_has_is_published_to_nobody() {
    let env = TestEnv::new();
    common::user(&env, "alice").await;
    env.events.clear();
    let item = common::item(&env.app, "movie", "Nobody's").await;
    env.app
        .apply_catalog_refresh(item.id, refresh("Nobody's"))
        .await
        .unwrap();
    env.app
        .mark_catalog_refresh_failed(item.id, 60)
        .await
        .unwrap();
    env.app.purge_provider_data(item.id).await.unwrap();
    assert_eq!(audiences(&env, item.id), Vec::<Vec<UserId>>::new());
}

#[tokio::test]
async fn lists_sessions_and_an_open_item_page_are_in_the_audience() {
    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let (carol_user, carol) = common::user(&env, "carol").await;
    let item = common::item(&env.app, "board_game", "Azul").await;

    // on Alice's private list: Alice
    let list = env
        .app
        .create_list(alice, "Games", None, Visibility::Private)
        .await
        .unwrap();
    env.app
        .add_to_list(alice, list.id, item.id, None)
        .await
        .unwrap();
    env.events.clear();
    env.app.set_cover_asset(item.id, "c".into()).await.unwrap();
    assert_eq!(audiences(&env, item.id), vec![vec![alice_user.id]]);

    // a play logged by Bob with a companion linked to Carol: Bob, and Carol while Bob shares
    // his sessions
    let c = env.app.create_companion(bob, "C.").await.unwrap();
    let req = env
        .app
        .request_link(bob, c.id, carol_user.id)
        .await
        .unwrap();
    env.app.accept_link(carol, req.id).await.unwrap();
    env.app
        .log_session(
            bob,
            app::NewSession::new(item.id, vec![Participant::new(c.id)]),
        )
        .await
        .unwrap();
    env.events.clear();
    env.app.set_cover_asset(item.id, "c".into()).await.unwrap();
    assert_eq!(
        audiences(&env, item.id),
        vec![sorted(vec![alice_user.id, bob_user.id, carol_user.id])]
    );
    env.app.set_privacy(bob, false).await.unwrap();
    env.events.clear();
    env.app.set_cover_asset(item.id, "c".into()).await.unwrap();
    assert_eq!(
        audiences(&env, item.id),
        vec![sorted(vec![alice_user.id, bob_user.id])]
    );

    // a list the whole instance can see: everybody
    env.app
        .set_list_visibility(alice, list.id, VisibilityChoice::Instance, false)
        .await
        .unwrap();
    env.events.clear();
    env.app.set_cover_asset(item.id, "c".into()).await.unwrap();
    assert_eq!(audiences(&env, item.id), vec![Vec::<UserId>::new()]);
}

#[tokio::test]
async fn an_open_item_page_hears_about_its_item_for_a_while() {
    let env = TestEnv::new();
    let (dave_user, dave) = common::user(&env, "dave").await;
    let item = common::item(&env.app, "movie", "Stub").await;
    env.app.watch_item(dave, item.id).await.unwrap();
    env.events.clear();
    env.app
        .apply_catalog_refresh(item.id, refresh("Stub"))
        .await
        .unwrap();
    assert_eq!(audiences(&env, item.id), vec![vec![dave_user.id]]);
    // after the watch ends: nobody
    let t0 = env.clock.now_for_tests();
    env.clock
        .set(t0 + chrono::Duration::seconds(app::ITEM_WATCH_SECS + 1));
    env.events.clear();
    env.app.set_cover_asset(item.id, "c".into()).await.unwrap();
    assert_eq!(audiences(&env, item.id), Vec::<Vec<UserId>>::new());
}

/// OT1B-3: a search result brings the source's popularity with it; a kept copy without one
/// gets it from the next search, a stored value is left to the refresh (no write per search).
#[tokio::test]
async fn a_search_result_fills_a_missing_popularity_and_keeps_a_stored_one() {
    let env = TestEnv::new();
    let hit = |popularity: Option<f64>| NewCatalogItem {
        kind: KindId::new("movie").unwrap(),
        title: "Harry Potter and the Philosopher's Stone".into(),
        original_title: None,
        year: Some(2001),
        description: None,
        external_ids: vec![ExternalId::new("tmdb", "671")],
        attributes: serde_json::json!({}),
        cover_url: None,
        popularity,
    };
    let kept = env.app.ensure_catalog_item(hit(None)).await.unwrap();
    assert_eq!(kept.popularity, None);
    let again = env
        .app
        .ensure_catalog_item(hit(Some(28000.0)))
        .await
        .unwrap();
    assert_eq!((again.id, again.popularity), (kept.id, Some(28000.0)));
    assert_eq!(
        env.app.catalog_item(kept.id).await.unwrap().popularity,
        Some(28000.0)
    );
    let third = env
        .app
        .ensure_catalog_item(hit(Some(28003.0)))
        .await
        .unwrap();
    assert_eq!(third.popularity, Some(28000.0), "left to the refresh");
    let fresh = env
        .app
        .ensure_catalog_item(NewCatalogItem {
            external_ids: vec![ExternalId::new("tmdb", "672")],
            ..hit(Some(21000.0))
        })
        .await
        .unwrap();
    assert_eq!(fresh.popularity, Some(21000.0), "a new item has it at once");
    let merged = env
        .app
        .upsert_catalog_item(hit(Some(28100.0)))
        .await
        .unwrap();
    assert_eq!(
        merged.popularity,
        Some(28100.0),
        "connector data replaces it"
    );
    let kept = env.app.upsert_catalog_item(hit(None)).await.unwrap();
    assert_eq!(
        kept.popularity,
        Some(28100.0),
        "an answer without one keeps it"
    );
}

/// A connector search result: inserted as a partial item nobody refers to yet.
async fn search_result(env: &TestEnv, kind: &str, title: &str) -> CatalogItem {
    env.app
        .ensure_catalog_item(NewCatalogItem {
            kind: KindId::new(kind).unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(2001),
            description: None,
            external_ids: vec![ExternalId::new("test", title)],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn a_detail_request_that_queues_a_refresh_reaches_the_requester() {
    let env = TestEnv::new();
    let (dave_user, dave) = common::user(&env, "dave").await;
    common::user(&env, "erin").await;
    env.events.clear();
    // a fresh search result is inserted without an event (nobody can have it yet)
    let stub = search_result(&env, "movie", "Slow Movie").await;
    assert_eq!(audiences(&env, stub.id), Vec::<Vec<UserId>>::new());
    // Dave asks for its details (`GET /items/{id}`): the refresh is queued and its result,
    // and what follows it (cover, episodes), reach Dave and nobody else
    env.app.item_detail_for(dave, stub.id).await.unwrap();
    assert_eq!(env.jobs.jobs().len(), 1);
    env.app
        .apply_catalog_refresh(stub.id, refresh("Slow Movie (refreshed)"))
        .await
        .unwrap();
    env.app
        .set_cover_asset(stub.id, "cover".into())
        .await
        .unwrap();
    assert_eq!(
        audiences(&env, stub.id),
        vec![vec![dave_user.id], vec![dave_user.id]]
    );
}

#[tokio::test]
async fn a_detail_request_of_a_fresh_item_writes_no_watch() {
    let env = TestEnv::new();
    let (_, dave) = common::user(&env, "dave").await;
    let item = search_result(&env, "movie", "Fresh").await;
    env.app
        .apply_catalog_refresh(item.id, refresh("Fresh"))
        .await
        .unwrap();
    // nothing to refresh: no job and no watch, so a later change is nobody's business
    env.app.item_detail_for(dave, item.id).await.unwrap();
    assert_eq!(env.jobs.jobs().len(), 0);
    env.events.clear();
    env.app.set_cover_asset(item.id, "c".into()).await.unwrap();
    assert_eq!(audiences(&env, item.id), Vec::<Vec<UserId>>::new());
}

#[tokio::test]
async fn a_search_result_added_to_a_list_or_matched_by_an_import_reaches_its_user() {
    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    common::user(&env, "carol").await;
    // the palette scoped to a list adds a search result (`POST /lists/{id}/items`)
    let listed = search_result(&env, "movie", "Listed").await;
    let list = env
        .app
        .create_list(alice, "Later", None, Visibility::Private)
        .await
        .unwrap();
    env.app
        .add_to_list(alice, list.id, listed.id, None)
        .await
        .unwrap();
    // an unmatched import row matched with a search result: the row's play goes into Bob's
    // history (what `match_row` replays)
    let matched = search_result(&env, "movie", "Matched").await;
    env.app
        .log_progress(
            bob,
            matched.id,
            domain::progress::Progress::Finished,
            None,
            None,
        )
        .await
        .unwrap();
    env.events.clear();
    for id in [listed.id, matched.id] {
        env.app
            .apply_catalog_refresh(id, refresh("Refreshed"))
            .await
            .unwrap();
    }
    assert_eq!(audiences(&env, listed.id), vec![vec![alice_user.id]]);
    assert_eq!(audiences(&env, matched.id), vec![vec![bob_user.id]]);
}

#[tokio::test]
async fn episodes_fetched_after_a_detail_request_reach_the_requester() {
    let env = TestEnv::new();
    let (dave_user, dave) = common::user(&env, "dave").await;
    let mut show = search_result(&env, "show", "Harbour Watch").await;
    show.attributes = serde_json::json!({ "seasons": [{"number": 1, "episode_count": 1}] });
    env.store.state().catalog.insert(show.id, show.clone());
    env.app.item_detail_for(dave, show.id).await.unwrap();
    // the refresh queues the episode job on the background lane; its answer comes later
    let key = env.app.seasons_to_fetch(show.id).await.unwrap().remove(0);
    env.events.clear();
    let episode = domain::catalog::CatalogEpisode {
        season: 1,
        number: 1,
        air_date: chrono::NaiveDate::from_ymd_opt(2025, 1, 1),
        runtime_minutes: Some(50),
    };
    env.app
        .store_episodes(
            show.id,
            &[app::SeasonFetch {
                key,
                connector: "tmdb".into(),
                episodes: vec![episode],
            }],
        )
        .await
        .unwrap();
    assert_eq!(audiences(&env, show.id), vec![vec![dave_user.id]]);
}
