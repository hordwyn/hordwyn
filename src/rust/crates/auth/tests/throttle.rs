//! G2-4: repeated failed sign-ins for a name are throttled; the right password then waits too,
//! until the window has passed.

use std::sync::Arc;

use auth::{Auth, AuthError, AuthSettings, MAX_FAILED_SIGN_INS, MemAuthStore, SignUp, SignupMode};

#[tokio::test]
async fn failed_sign_ins_are_throttled_per_name() {
    let t = testkit::TestEnv::new();
    let a = Auth::new(
        t.app.clone(),
        Arc::new(MemAuthStore::default()),
        AuthSettings {
            signup: SignupMode::Open,
            ..AuthSettings::default()
        },
    );
    a.sign_up(SignUp {
        username: "anna".into(),
        display_name: "Anna".into(),
        email: None,
        password: "correct horse battery".into(),
        invite: None,
    })
    .await
    .unwrap();
    for _ in 0..MAX_FAILED_SIGN_INS {
        assert!(matches!(
            a.sign_in("anna", "wrong password!", None).await,
            Err(AuthError::InvalidCredentials)
        ));
    }
    // now even the right password is refused, and other names are not affected
    assert!(matches!(
        a.sign_in("ANNA", "correct horse battery", None).await,
        Err(AuthError::TooManyAttempts { .. })
    ));
    assert!(matches!(
        a.sign_in("bob", "whatever password", None).await,
        Err(AuthError::InvalidCredentials)
    ));
    t.clock.advance(chrono::Duration::minutes(16));
    a.sign_in("anna", "correct horse battery", None)
        .await
        .unwrap();
}
