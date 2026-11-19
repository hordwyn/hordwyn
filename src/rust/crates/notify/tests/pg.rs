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
    let store = Arc::new(PgNotifyStore::new(
        pool,
        Arc::new(testkit::fakes::FakeCipher::default()),
    ));
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
    // the digest is claimed in one statement, once (OT-37, #6); a dead run's claim is taken
    // over once it is stale
    let now = t.clock.now();
    let minutes = chrono::Duration::minutes;
    let early = due - chrono::Duration::seconds(1);
    let claim = |until, at| store.claim_digest(ch.id, until, at, at - minutes(15));
    assert_eq!(claim(early, now).await.unwrap().len(), 0);
    let claimed = claim(due, now).await.unwrap();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].state, DeliveryState::Sending);
    assert_eq!(claimed[0].sent_at, Some(now));
    assert_eq!(claim(due, now).await.unwrap().len(), 0, "claimed once");
    assert_eq!(
        claim(due, now + minutes(16)).await.unwrap().len(),
        1,
        "stale"
    );
    let id = claimed[0].id;
    let later = now + minutes(16);
    assert!(
        store
            .claim_delivery(id, later, later - minutes(15))
            .await
            .unwrap()
            .is_none(),
        "held by the digest run"
    );
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

/// addendum-1 2.2 (DEC-140 reversed): channel secrets are stored encrypted, the JSON config
/// only keeps the redacted form; rows from before are migrated; a missing key pauses the
/// channel instead of sending placeholders.
#[tokio::test]
async fn channel_secrets_are_encrypted_at_rest() {
    use app::ports::SecretRows;
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::new();
    let pg = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos.users = pg.clone();
    t.app.repos.companions = pg;
    let user = t
        .app
        .register_user("root", "Root", None, Role::Admin)
        .await
        .unwrap();
    let cipher = Arc::new(testkit::fakes::FakeCipher::default());
    let store = PgNotifyStore::new(pool.clone(), cipher.clone());
    let ch = Channel {
        id: uuid::Uuid::now_v7(),
        user: user.id,
        name: "Push".into(),
        config: ChannelConfig::Gotify {
            server: "https://push.example.org".into(),
            token: "gotify-token-123".into(),
        },
        enabled: true,
        created_at: t.clock.now(),
    };
    store.put_channel(&ch).await.unwrap();
    let (config, secret): (serde_json::Value, Option<Vec<u8>>) =
        sqlx::query_as("SELECT config, secret FROM notify_channels WHERE id = $1")
            .bind(ch.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!config.to_string().contains("gotify-token-123"), "{config}");
    assert!(secret.is_some());
    assert_eq!(store.channel(ch.id).await.unwrap().unwrap(), ch);

    // a row written before the migration: secret only in the JSON
    let old = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO notify_channels (id, user_id, name, config, enabled, created_at)
         VALUES ($1, $2, 'Old', $3, true, now())",
    )
    .bind(old)
    .bind(user.id.0)
    .bind(
        serde_json::json!({"kind": "webhook", "url": "https://h.example.org", "secret": "whsec_x"}),
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(store.seal_plaintext(cipher.as_ref()).await.unwrap(), 1);
    assert_eq!(store.seal_plaintext(cipher.as_ref()).await.unwrap(), 0);
    let (config,): (serde_json::Value,) =
        sqlx::query_as("SELECT config FROM notify_channels WHERE id = $1")
            .bind(old)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!config.to_string().contains("whsec_x"));
    let back = store.channel(old).await.unwrap().unwrap();
    assert!(
        matches!(back.config, ChannelConfig::Webhook { ref secret, .. } if secret == "whsec_x")
    );
    assert_eq!(store.version_counts().await.unwrap(), vec![(1, 2)]);

    // key v2 only: locked -> paused, redacted, never the placeholder as a secret
    let v2 = Arc::new(testkit::fakes::FakeCipher {
        version: 2,
        loaded: vec![2],
    });
    let locked = PgNotifyStore::new(pool.clone(), v2.clone());
    let c = locked.channel(ch.id).await.unwrap().unwrap();
    assert!(!c.enabled);
    assert!(
        matches!(c.config, ChannelConfig::Gotify { ref token, .. } if token != "gotify-token-123")
    );
    // rotation with both versions loaded re-wraps everything
    let both = testkit::fakes::FakeCipher {
        version: 2,
        loaded: vec![2, 1],
    };
    let b = store.rewrap_batch(&both, 2, None, 100).await.unwrap();
    assert_eq!((b.rewrapped, b.locked), (2, 0));
    assert_eq!(store.version_counts().await.unwrap(), vec![(2, 2)]);
}
