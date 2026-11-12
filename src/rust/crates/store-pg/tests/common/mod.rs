#![allow(dead_code)]

use std::sync::Arc;

use app::{App, NewCatalogItem, Repos};
use domain::catalog::{CatalogItem, ExternalId};
use domain::kind::KindId;
use domain::user::{Actor, Role, User};
use store_pg::PgStore;
use testkit::{FixedClock, MemJobs, RecordingEvents, SeqIds, StaticKinds};

pub struct PgEnv {
    pub app: App,
    pub store: PgStore,
}

pub async fn env() -> Option<PgEnv> {
    let pool = store_pg::testing::fresh_db().await?;
    let store = PgStore::new(pool);
    let s = Arc::new(store.clone());
    let app = App {
        repos: Repos {
            users: s.clone(),
            companions: s.clone(),
            link_requests: s.clone(),
            invites: s.clone(),
            catalog: s.clone(),
            library: s.clone(),
            progress: s.clone(),
            reviews: s.clone(),
            lists: s.clone(),
            sessions: s,
        },
        kinds: Arc::new(StaticKinds::default()),
        clock: Arc::new(FixedClock::default()),
        ids: Arc::new(SeqIds::default()),
        events: Arc::new(RecordingEvents::default()),
        jobs: Arc::new(MemJobs::default()),
        notifications: Arc::new(testkit::RecordingNotifications::default()),
    };
    Some(PgEnv { app, store })
}

pub async fn user(app: &App, name: &str) -> (User, Actor) {
    let u = app
        .register_user(name, &name.to_uppercase(), None, Role::Member)
        .await
        .unwrap();
    let a = Actor {
        user_id: u.id,
        role: u.role,
    };
    (u, a)
}

pub async fn item(app: &App, kind: &str, title: &str) -> CatalogItem {
    app.upsert_catalog_item(NewCatalogItem {
        kind: KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2001),
        description: None,
        external_ids: vec![ExternalId::new("test", title)],
        attributes: serde_json::json!({"genres": ["x"]}),
        cover_url: None,
        popularity: None,
    })
    .await
    .unwrap()
}

pub async fn count(store: &PgStore, sql: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql.to_string()))
        .fetch_one(store.pool())
        .await
        .unwrap()
}
