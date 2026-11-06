use std::sync::Arc;

use chrono::Duration;
use testkit::TestEnv;
use testkit::fakes::FixedClock;

use super::*;

fn env(signup: SignupMode) -> (Auth, Arc<FixedClock>, TestEnv) {
    let t = TestEnv::new();
    let auth = Auth::new(
        t.app.clone(),
        Arc::new(MemAuthStore::default()),
        AuthSettings {
            signup,
            session_days: 30,
            totp_issuer: "Test".into(),
        },
    );
    (auth, t.clock.clone(), t)
}

fn su(name: &str) -> SignUp {
    SignUp {
        username: name.into(),
        display_name: name.into(),
        email: None,
        password: "correct horse battery".into(),
        invite: None,
    }
}

#[tokio::test]
async fn first_account_is_admin_even_when_signup_is_closed() {
    let (auth, _, _) = env(SignupMode::Closed);
    let (user, token) = auth.sign_up(su("anna")).await.unwrap();
    assert_eq!(user.role, Role::Admin);
    let actor = auth.session(&token.0).await.unwrap().unwrap();
    assert!(actor.is_admin());
    assert!(matches!(
        auth.sign_up(su("ben")).await,
        Err(AuthError::SignupClosed)
    ));
}

#[tokio::test]
async fn invite_mode_requires_a_valid_invite() {
    let (auth, _, _) = env(SignupMode::Invite);
    auth.sign_up(su("anna")).await.unwrap();
    assert!(matches!(
        auth.sign_up(su("ben")).await,
        Err(AuthError::SignupClosed)
    ));
    let mut bad = su("ben");
    bad.invite = Some("nope".into());
    assert!(matches!(
        auth.sign_up(bad).await,
        Err(AuthError::InvalidInvite)
    ));
}

#[tokio::test]
async fn promoted_companion_signs_up_with_the_invite() {
    let (auth, _, _) = env(SignupMode::Invite);
    let (anna, token) = auth.sign_up(su("anna")).await.unwrap();
    let actor = auth.session(&token.0).await.unwrap().unwrap();
    let c = auth.app().create_companion(actor, "Ben").await.unwrap();
    let (_, invite) = auth
        .app()
        .promote_companion(actor, c.id, None)
        .await
        .unwrap();
    let mut s = su("ben");
    s.invite = Some(invite);
    let (ben, _) = auth.sign_up(s).await.unwrap();
    assert_eq!(ben.role, Role::Member);
    let linked = auth.app().companion(actor, c.id).await.unwrap();
    assert_eq!(linked.linked_user, Some(ben.id));
    assert_ne!(anna.id, ben.id);
}

#[tokio::test]
async fn sign_in_checks_the_password_and_rejects_unknown_users_alike() {
    let (auth, _, _) = env(SignupMode::Open);
    auth.sign_up(su("anna")).await.unwrap();
    assert!(
        auth.sign_in("anna", "correct horse battery", None)
            .await
            .is_ok()
    );
    assert!(matches!(
        auth.sign_in("anna", "wrong password!!", None).await,
        Err(AuthError::InvalidCredentials)
    ));
    assert!(matches!(
        auth.sign_in("nobody", "correct horse battery", None).await,
        Err(AuthError::InvalidCredentials)
    ));
}

#[tokio::test]
async fn weak_passwords_are_refused() {
    let (auth, _, _) = env(SignupMode::Open);
    let mut s = su("anna");
    s.password = "short".into();
    assert!(matches!(
        auth.sign_up(s).await,
        Err(AuthError::PasswordTooShort)
    ));
}

#[tokio::test]
async fn sessions_expire_slide_and_end_on_sign_out() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (_, token) = auth.sign_up(su("anna")).await.unwrap();
    clock.advance(Duration::days(20));
    assert!(auth.session(&token.0).await.unwrap().is_some(), "slides");
    clock.advance(Duration::days(20));
    assert!(
        auth.session(&token.0).await.unwrap().is_some(),
        "20 days after the last use"
    );
    clock.advance(Duration::days(31));
    assert!(auth.session(&token.0).await.unwrap().is_none(), "expired");
    let t2 = auth
        .sign_in("anna", "correct horse battery", None)
        .await
        .unwrap();
    auth.sign_out(&t2.0).await.unwrap();
    assert!(auth.session(&t2.0).await.unwrap().is_none());
}

#[tokio::test]
async fn only_token_hashes_are_stored() {
    let (auth, _, _) = env(SignupMode::Open);
    let (_, token) = auth.sign_up(su("anna")).await.unwrap();
    assert!(auth.store().session(&token.0).await.unwrap().is_none());
    assert!(
        auth.store()
            .session(&token_hash(&token.0))
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn totp_enrolment_and_replay_protection() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (user, token) = auth.sign_up(su("anna")).await.unwrap();
    let actor = auth.session(&token.0).await.unwrap().unwrap();
    let enrol = auth.totp_begin(actor).await.unwrap();
    assert!(enrol.otpauth_url.starts_with("otpauth://totp/"));
    let secret = auth
        .store()
        .credentials(user.id)
        .await
        .unwrap()
        .unwrap()
        .totp_secret
        .unwrap();
    let totp = auth.totp(secret, "anna").unwrap();
    let now = || u64::try_from(clock.now().timestamp()).unwrap();
    auth.totp_confirm(actor, &totp.generate(now()).to_string())
        .await
        .unwrap();
    assert!(matches!(
        auth.sign_in("anna", "correct horse battery", None).await,
        Err(AuthError::TotpRequired)
    ));
    // the code used for confirmation cannot be replayed
    assert!(matches!(
        auth.sign_in(
            "anna",
            "correct horse battery",
            Some(&totp.generate(now()).to_string())
        )
        .await,
        Err(AuthError::InvalidTotp)
    ));
    clock.advance(Duration::seconds(30));
    assert!(
        auth.sign_in(
            "anna",
            "correct horse battery",
            Some(&totp.generate(now()).to_string())
        )
        .await
        .is_ok()
    );
}

#[tokio::test]
async fn changing_the_password_ends_other_sessions() {
    let (auth, _, _) = env(SignupMode::Open);
    let (_, t1) = auth.sign_up(su("anna")).await.unwrap();
    let actor = auth.session(&t1.0).await.unwrap().unwrap();
    let t2 = auth
        .change_password(actor, "correct horse battery", "another long secret")
        .await
        .unwrap();
    assert!(auth.session(&t1.0).await.unwrap().is_none());
    assert!(auth.session(&t2.0).await.unwrap().is_some());
    assert!(
        auth.sign_in("anna", "another long secret", None)
            .await
            .is_ok()
    );
}

#[test]
fn dummy_hash_parses() {
    assert!(PasswordHash::new(DUMMY_HASH).is_ok());
}
