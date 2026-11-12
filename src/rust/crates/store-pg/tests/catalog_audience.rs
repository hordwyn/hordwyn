//! Who hears about a change of a catalog item, on Postgres (migration 0031, DEC-490).

mod common;

use app::CatalogRefresh;
use app::ports::ItemAudience;
use domain::ids::{ItemId, UserId};
use domain::list::{Visibility, VisibilityChoice};
use domain::session::Participant;

fn refresh(title: &str) -> CatalogRefresh {
    CatalogRefresh {
        title: title.into(),
        original_title: None,
        year: Some(2001),
        description: None,
        attributes: serde_json::json!({"genres": ["x"]}),
        external_ids: vec![],
        cover_url: None,
        provenance: std::collections::BTreeMap::new(),
        fresh_for_secs: 60,
        popularity: None,
    }
}

async fn audience(env: &common::PgEnv, id: ItemId, now: domain::Timestamp) -> ItemAudience {
    use app::ports::CatalogRepo;
    match env.store.audience(id, now).await.unwrap() {
        ItemAudience::Users(mut u) => {
            u.sort_by_key(|x| x.0);
            ItemAudience::Users(u)
        }
        e @ ItemAudience::Everyone => e,
    }
}

fn users(mut u: Vec<UserId>) -> ItemAudience {
    u.sort_by_key(|x| x.0);
    ItemAudience::Users(u)
}

#[tokio::test]
async fn a_catalog_change_reaches_the_users_it_matters_to_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let now = chrono::Utc::now();
    let (alice_user, alice) = common::user(app, "alice").await;
    let (bob_user, bob) = common::user(app, "bob").await;
    let (carol_user, carol) = common::user(app, "carol").await;
    let (dave_user, dave) = common::user(app, "dave").await;
    let item = common::item(app, "board_game", "Azul").await;
    assert_eq!(audience(&env, item.id, now).await, users(vec![]));

    // Alice has it; Bob does not
    app.add_to_library(alice, item.id, None).await.unwrap();
    app.apply_catalog_refresh(item.id, refresh("Azul"))
        .await
        .unwrap();
    assert_eq!(
        audience(&env, item.id, now).await,
        users(vec![alice_user.id])
    );

    // Carol's list, Bob's play with a companion linked to Dave (while Bob shares sessions)
    let list = app
        .create_list(carol, "Games", None, Visibility::Private)
        .await
        .unwrap();
    app.add_to_list(carol, list.id, item.id, None)
        .await
        .unwrap();
    let c = app.create_companion(bob, "D.").await.unwrap();
    let req = app.request_link(bob, c.id, dave_user.id).await.unwrap();
    app.accept_link(dave, req.id).await.unwrap();
    app.log_session(
        bob,
        app::NewSession::new(item.id, vec![Participant::new(c.id)]),
    )
    .await
    .unwrap();
    let everyone_involved = vec![alice_user.id, bob_user.id, carol_user.id, dave_user.id];
    assert_eq!(
        audience(&env, item.id, now).await,
        users(everyone_involved.clone())
    );
    app.set_privacy(bob, false).await.unwrap();
    assert_eq!(
        audience(&env, item.id, now).await,
        users(everyone_involved[..3].to_vec())
    );

    // a list the instance can see
    app.set_list_visibility(carol, list.id, VisibilityChoice::Instance, false)
        .await
        .unwrap();
    assert_eq!(audience(&env, item.id, now).await, ItemAudience::Everyone);
}

#[tokio::test]
async fn an_open_item_page_watches_its_item_for_a_while_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (erin_user, erin) = common::user(app, "erin").await;
    let stub = common::item(app, "movie", "Stub").await;
    let other = common::item(app, "movie", "Other").await;
    // FixedClock starts at 2026-01-01
    let start = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .to_utc();
    app.watch_item(erin, stub.id).await.unwrap();
    app.watch_item(erin, stub.id).await.unwrap();
    assert_eq!(
        audience(&env, stub.id, start).await,
        users(vec![erin_user.id])
    );
    assert_eq!(audience(&env, other.id, start).await, users(vec![]));
    let after = start + chrono::Duration::seconds(app::ITEM_WATCH_SECS + 1);
    assert_eq!(audience(&env, stub.id, after).await, users(vec![]));
    // an ended watch is dropped with the next one of the same user; the item can go
    {
        use app::ports::CatalogRepo;
        env.store
            .watch(
                other.id,
                erin_user.id,
                after + chrono::Duration::hours(1),
                after,
            )
            .await
            .unwrap();
    }
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM catalog_watchers").await,
        1
    );
    assert_eq!(
        app.delete_unreferenced_item(other.id).await.unwrap(),
        app::ports::Removal::Deleted { cover_asset: None }
    );
}
