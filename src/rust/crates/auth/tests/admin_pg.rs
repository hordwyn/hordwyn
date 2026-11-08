//! The Postgres admin store: settings, audit log with filters and retention, encrypted
//! data-source credentials (addendum-1 2.2 and 3).

// One scenario on purpose: the steps share one database.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::sync::Arc;

use app::ports::SecretRows;
use auth::admin::{ActorKind, AdminStore, AuditEntry, AuditFilter, LicenceRecord, StoredSettings};
use auth::admin_pg::{PgAdminStore, has_encrypted_rows};
use chrono::{Duration, Utc};

fn entry(
    action: &str,
    at: chrono::DateTime<Utc>,
    actor: Option<domain::ids::UserId>,
) -> AuditEntry {
    AuditEntry {
        id: uuid::Uuid::now_v7(),
        at,
        actor_id: actor,
        actor_kind: if actor.is_some() {
            ActorKind::User
        } else {
            ActorKind::System
        },
        actor_name: actor.map(|_| "anna".into()),
        action: action.into(),
        target_type: Some("user".into()),
        target_id: Some("x".into()),
        detail: serde_json::json!({"from": "member", "to": "admin"}),
        client_addr: None,
        request_id: None,
    }
}

#[tokio::test]
async fn settings_audit_and_encrypted_credentials_on_postgres() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::new();
    let store = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos.users = store.clone();
    t.app.repos.companions = store;
    let user = t
        .app
        .register_user("anna", "Anna", None, domain::user::Role::Admin)
        .await
        .unwrap();
    let ring = Arc::new(auth::secrets::KeyRing::generate().0);
    let s = PgAdminStore::new(pool.clone(), ring.clone());
    assert!(!has_encrypted_rows(&pool).await.unwrap());

    assert_eq!(s.settings().await.unwrap(), StoredSettings::default());
    let mut set = StoredSettings {
        hosting: Some("hosted".into()),
        session_days: Some(7),
        storage_max_total_bytes: Some(5 << 30),
        updated_by: Some(user.id),
        ..StoredSettings::default()
    };
    set.updated_at = Some(Utc::now());
    s.put_settings(&set, &[]).await.unwrap();
    let back = s.settings().await.unwrap();
    assert_eq!(
        (back.hosting, back.session_days),
        (Some("hosted".into()), Some(7))
    );
    assert_eq!(back.storage_max_total_bytes, Some(5 << 30));

    let now = Utc::now();
    s.audit(&entry(
        "role.changed",
        now - Duration::days(400),
        Some(user.id),
    ))
    .await
    .unwrap();
    s.audit(&entry(
        "user.disabled",
        now - Duration::days(1),
        Some(user.id),
    ))
    .await
    .unwrap();
    s.audit(&entry("secrets.rewrapped", now, None))
        .await
        .unwrap();
    let all = s
        .audit_query(&AuditFilter {
            limit: 10,
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].action, "secrets.rewrapped", "newest first");
    let users = s
        .audit_query(&AuditFilter {
            action: Some("user.".into()),
            limit: 10,
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(users.len(), 1);
    let by = s
        .audit_query(&AuditFilter {
            actor: Some(user.id),
            limit: 10,
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(by.len(), 2);
    assert_eq!(s.audit_purge(now - Duration::days(365)).await.unwrap(), 1);

    let mut fields = BTreeMap::new();
    fields.insert("api_key".to_string(), "tmdb-secret-value".to_string());
    let c = s
        .put_credential("tmdb", None, &fields, Some(user.id), now, None)
        .await
        .unwrap();
    let raw: (Vec<u8>,) = sqlx::query_as("SELECT secret FROM connector_credentials WHERE id = $1")
        .bind(c.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !raw.0.windows(6).any(|w| w == b"secret"),
        "no plain text in the column"
    );
    assert!(has_encrypted_rows(&pool).await.unwrap());
    let back = s.credential("tmdb", None).await.unwrap().unwrap();
    assert_eq!(
        back.fields.as_ref().unwrap()["api_key"],
        "tmdb-secret-value"
    );
    // replace keeps the row id
    let again = s
        .put_credential("tmdb", None, &fields, Some(user.id), now, None)
        .await
        .unwrap();
    assert_eq!(again.id, c.id);
    s.record_test(c.id, now, "ok").await.unwrap();
    // per-user credential next to the instance one
    let mut pin = BTreeMap::new();
    pin.insert("pin".to_string(), "1234".to_string());
    s.put_credential("tvdb", Some(user.id), &pin, Some(user.id), now, None)
        .await
        .unwrap();
    assert_eq!(s.user_credentials(user.id).await.unwrap().len(), 1);
    assert_eq!(s.instance_credentials().await.unwrap().len(), 1);
    assert_eq!(s.version_counts().await.unwrap(), vec![(1, 2)]);
    // another key: locked, not deleted
    let other = PgAdminStore::new(pool.clone(), Arc::new(auth::secrets::KeyRing::generate().0));
    assert!(
        other
            .credential("tmdb", None)
            .await
            .unwrap()
            .unwrap()
            .fields
            .is_err()
    );
    assert!(
        s.delete_credential("tvdb", Some(user.id), None)
            .await
            .unwrap()
    );
    assert!(
        !s.delete_credential("tvdb", Some(user.id), None)
            .await
            .unwrap()
    );

    s.put_licence(
        &LicenceRecord {
            connector: "tmdb".into(),
            agreement_on_file: true,
            recorded_by: Some(user.id),
            recorded_at: now,
        },
        None,
    )
    .await
    .unwrap();
    assert!(s.licences().await.unwrap()[0].agreement_on_file);
    s.record_key_versions(&[2, 1], now).await.unwrap();
    s.record_key_versions(&[2], now + Duration::days(1))
        .await
        .unwrap();
    let kv = s.key_versions().await.unwrap();
    assert_eq!(kv.len(), 2);
    assert!(
        kv.iter().all(|(_, t)| (*t - now).num_seconds().abs() < 1),
        "first seen kept"
    );
    s.ping().await.unwrap();
}

/// addendum-1 3.4: the audit row is part of the change's transaction (a row that cannot be
/// written, here a repeated id, takes the change with it); the SMTP password is encrypted
/// (purpose `smtp_password`); the source switches are stored.
#[tokio::test]
async fn audited_writes_mail_password_and_source_switches_on_postgres() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let ring = Arc::new(auth::secrets::KeyRing::generate().0);
    let s = PgAdminStore::new(pool.clone(), ring.clone());
    let now = Utc::now();
    let first = entry("setting.changed", now, None);
    s.audit(&first).await.unwrap();

    let next = StoredSettings {
        hosting: Some("hosted".into()),
        ..StoredSettings::default()
    };
    assert!(
        s.put_settings(&next, std::slice::from_ref(&first))
            .await
            .is_err()
    );
    assert_eq!(s.settings().await.unwrap().hosting, None, "rolled back");
    let mut fields = BTreeMap::new();
    fields.insert("api_key".to_string(), "k".to_string());
    assert!(
        s.put_credential("tmdb", None, &fields, None, now, Some(&first))
            .await
            .is_err()
    );
    assert_eq!(s.credential("tmdb", None).await.unwrap(), None);
    assert!(
        s.set_source_enabled("tmdb", false, None, now, Some(&first))
            .await
            .is_err()
    );
    assert_eq!(s.source_switches().await.unwrap().len(), 0);

    let row = entry("source.disabled", now, None);
    s.set_source_enabled("tmdb", false, None, now, Some(&row))
        .await
        .unwrap();
    assert_eq!(
        s.source_switches().await.unwrap(),
        vec![("tmdb".to_string(), false)]
    );
    let all = s
        .audit_query(&AuditFilter {
            limit: 10,
            ..AuditFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(all.len(), 2, "{all:?}");

    // mail: settings without the password, the password encrypted on its own
    let mut st = s.settings().await.unwrap();
    st.mail.host = Some("smtp.example.org".into());
    st.mail.port = Some(465);
    st.sign_in_mode = Some("page".into());
    s.put_settings(&st, &[]).await.unwrap();
    assert!(!has_encrypted_rows(&pool).await.unwrap());
    s.put_mail_password(Some("smtp-pw"), None).await.unwrap();
    assert!(has_encrypted_rows(&pool).await.unwrap());
    let back = s.settings().await.unwrap();
    assert_eq!(
        (
            back.mail.port,
            back.mail.password_set,
            back.sign_in_mode.as_deref()
        ),
        (Some(465), true, Some("page"))
    );
    // saving the settings again leaves the password alone
    s.put_settings(&back, &[]).await.unwrap();
    assert_eq!(
        s.mail_password().await.unwrap().unwrap().unwrap(),
        "smtp-pw"
    );
    let raw: Vec<u8> = sqlx::query_scalar("SELECT mail_password FROM instance_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!String::from_utf8_lossy(&raw).contains("smtp-pw"));
    let other = PgAdminStore::new(pool.clone(), Arc::new(auth::secrets::KeyRing::generate().0));
    assert!(
        other.mail_password().await.unwrap().unwrap().is_err(),
        "locked"
    );
    let rows = auth::admin_pg::PgMailSecret::new(pool.clone());
    assert_eq!(rows.version_counts().await.unwrap(), vec![(1, 1)]);
    s.record_mail_test(now, "ok").await.unwrap();
    assert_eq!(
        s.settings().await.unwrap().mail.last_test_result.as_deref(),
        Some("ok")
    );
    s.put_mail_password(None, None).await.unwrap();
    assert_eq!(s.mail_password().await.unwrap(), None);
}
