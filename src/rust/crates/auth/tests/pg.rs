//! The Postgres auth store behaves like the in-memory one.

use std::sync::Arc;

use auth::pg::PgAuthStore;
use auth::{Auth, AuthSettings, SignUp, SignupMode};

#[tokio::test]
async fn sign_up_sign_in_and_sessions_on_postgres() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::new();
    let store = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos.users = store.clone();
    t.app.repos.companions = store;
    let pg = Arc::new(PgAuthStore::new(
        pool,
        Arc::new(auth::secrets::KeyRing::generate().0),
    ));
    let auth = Auth::new(
        t.app.clone(),
        pg.clone(),
        AuthSettings {
            signup: SignupMode::Open,
            ..AuthSettings::default()
        },
    );
    let (user, token) = auth
        .sign_up(SignUp {
            username: "anna".into(),
            display_name: "Anna".into(),
            email: None,
            password: "correct horse battery".into(),
            invite: None,
        })
        .await
        .unwrap();
    assert_eq!(
        auth.session(&token.0).await.unwrap().unwrap().user_id,
        user.id
    );
    let t2 = auth
        .sign_in("anna", "correct horse battery", None)
        .await
        .unwrap();
    auth.sign_out(&token.0).await.unwrap();
    assert!(auth.session(&token.0).await.unwrap().is_none());
    assert!(auth.session(&t2.0).await.unwrap().is_some());
    let actor = auth.session(&t2.0).await.unwrap().unwrap();
    auth.totp_begin(actor).await.unwrap();
    let creds = auth::AuthStore::credentials(pg.as_ref(), user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(creds.totp_secret.is_some() && !creds.totp_enabled);
    auth::AuthStore::link_identity(
        pg.as_ref(),
        &auth::OidcIdentity {
            issuer: "https://id.example.org".into(),
            subject: "abc".into(),
            user_id: user.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        auth::AuthStore::identity(pg.as_ref(), "https://id.example.org", "abc")
            .await
            .unwrap()
            .unwrap()
            .user_id,
        user.id
    );
    let now = app::ports::Clock::now(t.app.clock.as_ref());
    assert_eq!(pg.purge_expired(now).await.unwrap(), 0);
    assert_eq!(
        pg.purge_expired(now + chrono::Duration::days(31))
            .await
            .unwrap(),
        1,
        "the remaining session expires after 30 days"
    );
}

fn ring(lines: &str) -> auth::secrets::KeyRing {
    auth::secrets::KeyRing::parse(lines).unwrap()
}

fn key(v: u16, b: u8) -> String {
    use base64::Engine;
    format!(
        "v{v}:{}",
        base64::engine::general_purpose::STANDARD.encode([b; 32])
    )
}

/// addendum-1 2.2: TOTP seeds are encrypted at rest, plain rows from before are migrated
/// on start, a KEK rotation re-wraps them, and a missing key version makes them `locked`.
#[tokio::test]
async fn totp_seeds_are_encrypted_migrated_rotated_and_locked_without_the_key() {
    use app::ports::SecretRows;
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::new();
    let store = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos.users = store.clone();
    t.app.repos.companions = store;
    let v1 = Arc::new(ring(&key(1, 1)));
    let pg = PgAuthStore::new(pool.clone(), v1.clone());
    let user = t
        .app
        .register_user("anna", "Anna", None, domain::user::Role::Admin)
        .await
        .unwrap();
    // a row from before the migration: plain seed only
    sqlx::query(
        "INSERT INTO credentials (user_id, totp_secret, totp_enabled) VALUES ($1, $2, true)",
    )
    .bind(user.id.0)
    .bind(b"0123456789abcdefghij".to_vec())
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(pg.seal_plaintext(v1.as_ref()).await.unwrap(), 1);
    assert_eq!(
        pg.seal_plaintext(v1.as_ref()).await.unwrap(),
        0,
        "idempotent"
    );
    let (plain, enc): (Option<Vec<u8>>, Option<Vec<u8>>) =
        sqlx::query_as("SELECT totp_secret, totp_secret_enc FROM credentials WHERE user_id = $1")
            .bind(user.id.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(plain, None);
    let enc = enc.unwrap();
    assert!(
        !enc.windows(10).any(|w| w == b"0123456789"),
        "no plain text"
    );
    let c = auth::AuthStore::credentials(&pg, user.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(c.totp_secret.as_deref(), Some(&b"0123456789abcdefghij"[..]));
    assert_eq!(pg.version_counts().await.unwrap(), vec![(1, 1)]);

    // rotation: v2 active, v1 still loaded
    let both = ring(&format!("{}\n{}", key(2, 2), key(1, 1)));
    let pg2 = PgAuthStore::new(pool.clone(), Arc::new(both));
    let cipher = ring(&format!("{}\n{}", key(2, 2), key(1, 1)));
    let b = pg2.rewrap_batch(&cipher, 2, None, 100).await.unwrap();
    assert_eq!((b.rewrapped, b.locked), (1, 0));
    let b = pg2.rewrap_batch(&cipher, 2, b.last, 100).await.unwrap();
    assert_eq!((b.rewrapped, b.locked, b.last), (0, 0, None));
    assert_eq!(pg2.version_counts().await.unwrap(), vec![(2, 1)]);

    // only v1 loaded now: the seed is locked, sign-in says so, the value is kept
    let old = PgAuthStore::new(pool.clone(), Arc::new(ring(&key(1, 1))));
    let c = auth::AuthStore::credentials(&old, user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(c.totp_locked && c.totp_secret.is_none());
    assert_eq!(old.locked_totp_count().await.unwrap(), 1);
    auth::AuthStore::put_credentials(&old, &c).await.unwrap();
    assert_eq!(pg2.version_counts().await.unwrap(), vec![(2, 1)], "kept");
}

/// UX2-10 / UX2-S7: locked rows ahead in id order do not stop the re-wrap of the rows behind
/// them (keyset paging), each locked row is counted once, and a value too short to carry a
/// key version is skipped instead of failing the whole select.
#[tokio::test]
async fn rewrap_pages_past_locked_rows_and_skips_short_values() {
    use app::ports::SecretRows;
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let mut t = testkit::TestEnv::new();
    let store = Arc::new(store_pg::PgStore::new(pool.clone()));
    t.app.repos.users = store.clone();
    t.app.repos.companions = store;
    let mut ids = Vec::new();
    for name in ["anna", "ben", "carl", "dora"] {
        let u = t
            .app
            .register_user(name, name, None, domain::user::Role::Member)
            .await
            .unwrap();
        ids.push(u.id.0);
    }
    ids.sort();
    let plain = |id: uuid::Uuid| {
        sqlx::query(
            "INSERT INTO credentials (user_id, totp_secret, totp_enabled) VALUES ($1, $2, true)",
        )
        .bind(id)
        .bind(b"0123456789abcdefghij".to_vec())
        .execute(&pool)
    };
    // the first row in id order is sealed with a key that is gone later: locked
    plain(ids[0]).await.unwrap();
    let lost = ring(&key(9, 9));
    PgAuthStore::new(pool.clone(), Arc::new(ring(&key(9, 9))))
        .seal_plaintext(&lost)
        .await
        .unwrap();
    for id in &ids[1..3] {
        plain(*id).await.unwrap();
    }
    let v1 = ring(&key(1, 1));
    PgAuthStore::new(pool.clone(), Arc::new(ring(&key(1, 1))))
        .seal_plaintext(&v1)
        .await
        .unwrap();
    // a damaged value: too short for the version bytes
    sqlx::query("INSERT INTO credentials (user_id, totp_secret_enc) VALUES ($1, $2)")
        .bind(ids[3])
        .bind(vec![1u8])
        .execute(&pool)
        .await
        .unwrap();

    let cipher = ring(&format!("{}\n{}", key(2, 2), key(1, 1)));
    let pg = PgAuthStore::new(
        pool.clone(),
        Arc::new(ring(&format!("{}\n{}", key(2, 2), key(1, 1)))),
    );
    let (mut rewrapped, mut locked, mut after) = (0, 0, None);
    for _ in 0..10 {
        let b = pg.rewrap_batch(&cipher, 2, after, 1).await.unwrap();
        rewrapped += b.rewrapped;
        locked += b.locked;
        after = b.last;
        if after.is_none() {
            break;
        }
    }
    assert_eq!((rewrapped, locked), (2, 1));
    assert_eq!(
        pg.version_counts().await.unwrap(),
        vec![(2, 2), (9, 1)],
        "the locked row is kept as it was"
    );
}

/// UX2-S4: an admin invite is claimed with one conditional update; a second claim, a
/// revoked or an expired invite gets nothing.
#[tokio::test]
async fn signup_invites_are_claimed_once() {
    use auth::InviteStore;
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let pg = PgAuthStore::new(pool, Arc::new(auth::secrets::KeyRing::generate().0));
    let now = chrono::Utc::now();
    let invite = |id: u128, expires: chrono::DateTime<chrono::Utc>| auth::SignupInvite {
        id: uuid::Uuid::from_u128(id),
        token_hash: format!("hash-{id}"),
        email: None,
        note: None,
        created_by: None,
        created_at: now,
        expires_at: expires,
        used_by: None,
        used_at: None,
        revoked_at: None,
    };
    let open = invite(1, now + chrono::Duration::days(1));
    let expired = invite(2, now - chrono::Duration::minutes(1));
    let mut revoked = invite(3, now + chrono::Duration::days(1));
    for i in [&open, &expired, &revoked] {
        pg.insert_signup_invite(i).await.unwrap();
    }
    revoked.revoked_at = Some(now);
    pg.update_signup_invite(&revoked).await.unwrap();
    assert!(pg.claim_signup_invite(open.id, now).await.unwrap());
    assert!(
        !pg.claim_signup_invite(open.id, now).await.unwrap(),
        "single use"
    );
    assert!(!pg.claim_signup_invite(expired.id, now).await.unwrap());
    assert!(!pg.claim_signup_invite(revoked.id, now).await.unwrap());
    // a failed account creation gives the claim back
    pg.finish_signup_invite(open.id, None).await.unwrap();
    assert!(pg.claim_signup_invite(open.id, now).await.unwrap());
}
