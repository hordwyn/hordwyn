#![cfg(feature = "pg")]
//! The Postgres notification store: dedupe constraints, admins, digest queue, overrides.

use std::sync::Arc;

use app::ports::{Audience, Clock, Notification, Notifications};
use domain::user::Role;
use notify::model::{
    Channel, ChannelConfig, DeliveryMode, DeliveryState, Filter, Subscription, Targets,
};
use notify::pg::PgNotifyStore;
use notify::{Engine, NotifyStore};

#[tokio::test]
// One scenario on purpose: the steps share one database.
#[allow(clippy::too_many_lines)]
async fn engine_on_postgres_dedupes_and_queues_digests() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::new();
    let pg = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos.users = pg.clone();
    t.app.repos.companions = pg;
    let admin = t
        .app
        .register_user("root", "Root", None, Role::Admin)
        .await
        .unwrap();
    let store = Arc::new(PgNotifyStore::new(pool));
    assert_eq!(store.admins().await.unwrap(), vec![admin.id]);
    let engine = Engine::new(
        store.clone(),
        t.jobs.clone(),
        t.events.clone(),
        t.clock.clone(),
        t.ids.clone(),
        "Shelf",
        "de",
        notify::template::Overrides::new(),
    );
    let ch = Channel {
        id: uuid::Uuid::now_v7(),
        user: admin.id,
        name: "Mail".into(),
        config: ChannelConfig::Email {
            to: "root@example.org".into(),
        },
        enabled: true,
        created_at: t.clock.now(),
    };
    store.put_channel(&ch).await.unwrap();
    store
        .put_subscription(&Subscription {
            id: uuid::Uuid::now_v7(),
            user: None,
            event: "admin.*".into(),
            filter: Filter::default(),
            targets: Targets::All,
            mode: DeliveryMode::Digest {
                cron: "0 8 * * 1".into(),
            },
            quiet_hours: None,
            timezone: "UTC".into(),
            rate_limit_per_hour: None,
            enabled: true,
            locked: true,
        })
        .await
        .unwrap();
    assert_eq!(store.defaults().await.unwrap().len(), 1);
    let n = Notification::new("admin.signup", Audience::Admins, "user:x")
        .arg("username", "carol")
        .arg("name", "Carol");
    engine.emit(n.clone()).await;
    engine.emit(n).await;
    let inbox = store.inbox(admin.id, 10).await.unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].title, "Neues Konto: carol");
    assert_eq!(store.count_unread(admin.id).await.unwrap(), 1);
    let log = store.deliveries(admin.id, 10).await.unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].state, DeliveryState::Digest);
    // 2026-01-01 is a Thursday: the digest is due Monday 2026-01-05 08:00 UTC
    let due: chrono::DateTime<chrono::Utc> = "2026-01-05T08:00:00Z".parse().unwrap();
    assert_eq!(log[0].due_at, due);
    assert_eq!(
        store
            .due_digest(ch.id, due - chrono::Duration::seconds(1))
            .await
            .unwrap()
            .len(),
        0
    );
    assert_eq!(store.due_digest(ch.id, due).await.unwrap().len(), 1);
    assert_eq!(
        store
            .count_since(ch.id, "admin.signup", t.clock.now())
            .await
            .unwrap(),
        1
    );
    store
        .mark_read(admin.id, inbox[0].id, t.clock.now())
        .await
        .unwrap();
    assert!(
        store.inbox(admin.id, 10).await.unwrap()[0]
            .read_at
            .is_some()
    );
    assert_eq!(store.count_unread(admin.id).await.unwrap(), 0);

    store
        .set_template_override("de", "admin-signup-title", Some("Hallo { $username }"))
        .await
        .unwrap();
    engine.reload_templates().await.unwrap();
    engine
        .emit(Notification::new("admin.signup", Audience::Admins, "user:y").arg("username", "dan"))
        .await;
    assert_eq!(
        store.inbox(admin.id, 1).await.unwrap()[0].title,
        "Hallo dan"
    );
    store
        .set_template_override("de", "admin-signup-title", None)
        .await
        .unwrap();
    assert!(!store.template_overrides().await.unwrap().contains_key("de"));
    // deleting the channel removes its deliveries
    store.delete_channel(ch.id).await.unwrap();
    assert_eq!(store.deliveries(admin.id, 10).await.unwrap().len(), 0);
}
