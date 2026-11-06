#![cfg(feature = "generic")]
//! The import runner against Postgres repositories and the Postgres import store: two runs
//! of the same file leave the same rows.

use std::sync::Arc;

use app::ports::JobQueue;
use domain::user::Role;
use importers::pg::PgImportStore;
use importers::{ImportState, Importer};

#[tokio::test]
async fn importing_twice_on_postgres_creates_no_duplicates() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
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
    let user = t
        .app
        .register_user("anna", "Anna", None, Role::Member)
        .await
        .unwrap();
    let actor = domain::user::Actor {
        user_id: user.id,
        role: user.role,
    };
    let importer = Importer::new(
        t.app.clone(),
        Arc::new(PgImportStore::new(pool.clone())),
        Arc::new(testkit::fakes::MemBlobs::default()),
        t.jobs.clone() as Arc<dyn JobQueue>,
    );
    let file = "{\"format\":\"library-export\",\"version\":1}\n\
        {\"source_id\":\"1\",\"kind\":\"movie\",\"title\":\"Heat\",\"history\":[{\"date\":\"2024-05-01\"}]}\n\
        {\"source_id\":\"2\",\"kind\":\"book\",\"title\":\"Dune\",\"status\":\"completed\"}\n";
    let count = |sql: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, i64>(sql)
                .fetch_one(&pool)
                .await
                .unwrap()
        }
    };
    let mut seen = None;
    for _ in 0..2 {
        let r = importer
            .start(actor, "generic-jsonl", file.as_bytes().to_vec(), false)
            .await
            .unwrap();
        let r = importer.run(r.id).await.unwrap();
        assert_eq!((r.state, r.imported), (ImportState::Done, 2));
        assert_eq!(
            importer.status(actor, r.id).await.unwrap().state,
            ImportState::Done
        );
        let now = (
            count("SELECT count(*) FROM library_entries").await,
            count("SELECT count(*) FROM progress_events").await,
            count("SELECT count(*) FROM catalog_items").await,
        );
        assert_eq!(*seen.get_or_insert(now), now);
    }
    assert_eq!(seen, Some((2, 1, 2)));
}
