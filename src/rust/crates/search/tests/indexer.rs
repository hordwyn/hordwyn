//! Outbox -> index: writes through the Postgres repositories show up in search after the
//! indexer runs, per user, and re-processing is harmless.

use std::sync::Arc;

use app::NewCatalogItem;
use app::ports::{SearchIndex, SearchQuery};
use domain::catalog::ExternalId;
use domain::kind::KindId;
use domain::user::{Actor, Role};
use search::indexer::Indexer;
use search::pg::PgSearch;

fn q(index: &str, text: &str, viewer: Actor) -> SearchQuery {
    SearchQuery {
        index: index.into(),
        text: text.into(),
        viewer: viewer.user_id,
        filters: vec![],
        limit: 10,
    }
}

#[tokio::test]
async fn outbox_rows_become_documents_per_user() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::new();
    let store = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos = app::Repos {
        users: store.clone(),
        companions: store.clone(),
        link_requests: store.clone(),
        invites: store.clone(),
        catalog: store.clone(),
        library: store.clone(),
        progress: store.clone(),
        reviews: store.clone(),
        lists: store.clone(),
        sessions: store,
    };
    let app = t.app.clone();
    let anna = app
        .register_user("anna", "Anna", None, Role::Admin)
        .await
        .unwrap();
    let ben = app
        .register_user("ben", "Ben", None, Role::Member)
        .await
        .unwrap();
    let (a, b) = (
        Actor {
            user_id: anna.id,
            role: anna.role,
        },
        Actor {
            user_id: ben.id,
            role: ben.role,
        },
    );
    let item = app
        .upsert_catalog_item(NewCatalogItem {
            kind: KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: Some("bank robbery in Los Angeles".into()),
            external_ids: vec![ExternalId::new("tmdb", "949")],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: Some(5400.0),
        })
        .await
        .unwrap();
    app.add_to_library(a, item.id, None).await.unwrap();
    app.set_notes(a, item.id, Some("watched with dad".into()), None)
        .await
        .unwrap();
    app.create_companion(a, "Carla").await.unwrap();

    let index = Arc::new(PgSearch::new(pool.clone()));
    let ix = Indexer::new(app.clone(), pool.clone(), index.clone());
    assert!(ix.drain().await.unwrap() > 0);
    assert_eq!(ix.drain().await.unwrap(), 0, "outbox is empty afterwards");

    let hits = index.search(q("catalog", "robbery", b)).await.unwrap();
    assert_eq!(hits.len(), 1, "catalog is shared");
    assert_eq!(
        hits[0].fields["popularity"], 5400.0,
        "the document carries the source's popularity (OT1B-3)"
    );
    let lib_a = index.search(q("library", "dad", a)).await.unwrap();
    assert_eq!(lib_a.len(), 1);
    assert_eq!(lib_a[0].fields["title"], "Heat");
    assert!(
        index
            .search(q("library", "dad", b))
            .await
            .unwrap()
            .is_empty(),
        "ben never sees anna's notes"
    );
    assert_eq!(
        index.search(q("people", "carla", a)).await.unwrap().len(),
        1
    );
    assert!(
        index
            .search(q("people", "carla", b))
            .await
            .unwrap()
            .is_empty(),
        "companions are private"
    );

    // full reindex: queue everything again, documents stay the same
    assert!(Indexer::enqueue_all(&pool).await.unwrap() >= 4);
    ix.drain().await.unwrap();
    assert_eq!(index.search(q("library", "dad", a)).await.unwrap().len(), 1);
}
