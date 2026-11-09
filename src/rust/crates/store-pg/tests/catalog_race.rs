//! DEC-445: concurrent imports and lookups of one external id must end with exactly one
//! catalog item that carries the id (a lost race used to leave a second item without it).

mod common;

use app::NewCatalogItem;
use domain::catalog::ExternalId;
use domain::kind::KindId;

const RUNS: usize = 16;

fn show(title: &str) -> NewCatalogItem {
    NewCatalogItem {
        kind: KindId::new("show").unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2011),
        description: None,
        external_ids: vec![ExternalId::new("tmdb", "1399")],
        attributes: serde_json::json!({}),
        cover_url: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_imports_of_one_external_id_create_one_item() {
    let Some(env) = common::env().await else {
        return;
    };
    let mut set = tokio::task::JoinSet::new();
    for n in 0..RUNS {
        let app = env.app.clone();
        set.spawn(async move {
            // the three entry points that may create an item: imports and webhooks, search
            // results, connector data
            match n % 3 {
                0 => {
                    app.find_or_create_catalog_item(show("Game of Thrones"))
                        .await
                }
                1 => app.ensure_catalog_item(show("Game of Thrones")).await,
                _ => app.upsert_catalog_item(show("Game of Thrones")).await,
            }
        });
    }
    let mut ids = Vec::new();
    while let Some(r) = set.join_next().await {
        ids.push(r.unwrap().unwrap().id);
    }
    ids.sort_by_key(|i| i.0);
    ids.dedup();
    assert_eq!(ids.len(), 1, "every caller gets the same item");
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM catalog_items").await,
        1
    );
    let owner = env
        .app
        .repos
        .catalog
        .by_external_id(
            &KindId::new("show").unwrap(),
            &ExternalId::new("tmdb", "1399"),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(owner.id, ids[0]);
}

/// A loser of the race must not attach data to the winner when it came from users (G2-2):
/// `find_or_create_catalog_item` returns the existing item unchanged.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lost_race_from_user_data_leaves_the_item_unchanged() {
    let Some(env) = common::env().await else {
        return;
    };
    let mut set = tokio::task::JoinSet::new();
    for n in 0..RUNS {
        let app = env.app.clone();
        set.spawn(async move {
            let mut new = show(&format!("Title {n}"));
            new.external_ids
                .push(ExternalId::new("imdb", format!("tt{n}")));
            app.find_or_create_catalog_item(new).await
        });
    }
    let mut items = Vec::new();
    while let Some(r) = set.join_next().await {
        items.push(r.unwrap().unwrap());
    }
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM catalog_items").await,
        1
    );
    // the winner keeps its own two ids; nobody else's imdb id was attached
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM external_ids").await,
        2
    );
    let stored = env.app.catalog_item(items[0].id).await.unwrap();
    for i in &items {
        assert_eq!(i.id, stored.id);
    }
}
