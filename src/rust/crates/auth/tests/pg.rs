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
    let pg = Arc::new(PgAuthStore::new(pool));
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
