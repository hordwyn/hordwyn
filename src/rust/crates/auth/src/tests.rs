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
            ..AuthSettings::default()
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

// ---------------------------------------------------------------- UX phase 2 (DEC-191, OD-08)

async fn totp_on(
    auth: &Auth,
    clock: &FixedClock,
    token: &SessionToken,
) -> (totp_rs::Totp, Vec<String>) {
    let actor = auth.session(&token.0).await.unwrap().unwrap();
    auth.totp_begin(actor).await.unwrap();
    let secret = auth
        .store()
        .credentials(actor.user_id)
        .await
        .unwrap()
        .unwrap()
        .totp_secret
        .unwrap();
    let totp = auth.totp(secret, "anna").unwrap();
    let now = u64::try_from(clock.now().timestamp()).unwrap();
    let codes = auth
        .totp_confirm(actor, &totp.generate(now).to_string())
        .await
        .unwrap();
    (totp, codes)
}

#[tokio::test]
async fn signing_in_is_a_fresh_proof_that_slides_for_15_minutes_up_to_an_hour() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (_, token) = auth.sign_up(su("anna")).await.unwrap();
    assert!(auth.require_fresh(&token.0).await.is_ok(), "sign-up counts");
    for _ in 0..4 {
        clock.advance(Duration::minutes(14));
        assert!(auth.require_fresh(&token.0).await.is_ok(), "slides");
    }
    clock.advance(Duration::minutes(5));
    assert!(
        matches!(
            auth.require_fresh(&token.0).await,
            Err(AuthError::ReauthRequired)
        ),
        "capped at one hour after the proof"
    );
    let t2 = auth
        .sign_in("anna", "correct horse battery", None)
        .await
        .unwrap();
    clock.advance(Duration::minutes(16));
    assert!(matches!(
        auth.require_fresh(&t2.0).await,
        Err(AuthError::ReauthRequired)
    ));
    let status = auth.reauth_status(&t2.0).await.unwrap();
    assert!(!status.fresh && status.has_password && !status.has_totp);
}

#[tokio::test]
async fn reauth_checks_the_password_then_totp_and_regenerates_the_session() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (_, token) = auth.sign_up(su("anna")).await.unwrap();
    let (totp, _) = totp_on(&auth, &clock, &token).await;
    clock.advance(Duration::minutes(30));
    assert!(matches!(
        auth.reauth(&token.0, "wrong password!", None).await,
        Err(AuthError::InvalidCredentials)
    ));
    assert!(matches!(
        auth.reauth(&token.0, "correct horse battery", None).await,
        Err(AuthError::TotpRequired)
    ));
    let now = u64::try_from(clock.now().timestamp()).unwrap();
    let fresh = auth
        .reauth(
            &token.0,
            "correct horse battery",
            Some(&totp.generate(now).to_string()),
        )
        .await
        .unwrap();
    assert_ne!(fresh, token);
    assert!(
        auth.session(&token.0).await.unwrap().is_none(),
        "old id is gone"
    );
    assert!(auth.require_fresh(&fresh.0).await.is_ok());
    let s = auth
        .store()
        .session(&token_hash(&fresh.0))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(s.reauth_method.as_deref(), Some("password+totp"));
}

/// A six-digit code that none of the steps around `now` accepts.
fn a_wrong_code(totp: &totp_rs::Totp, clock: &FixedClock) -> String {
    let now = u64::try_from(clock.now().timestamp()).unwrap();
    let right: Vec<String> = [now - 60, now - 30, now, now + 30, now + 60]
        .iter()
        .map(|t| totp.generate(*t).to_string())
        .collect();
    (0..1_000_000u32)
        .map(|c| format!("{c:06}"))
        .find(|c| !right.contains(c))
        .unwrap()
}

/// SEC2-1: someone who has the password guesses the one-time code from many addresses.
/// The count of wrong codes belongs to the account alone, so the wait grows whatever the
/// address; a right code clears it.
#[tokio::test]
async fn wrong_codes_are_bounded_per_account_whatever_the_address() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (_, token) = auth.sign_up(su("anna")).await.unwrap();
    let (totp, codes) = totp_on(&auth, &clock, &token).await;
    let free = auth.settings().throttle.free_failures;
    let wrong = a_wrong_code(&totp, &clock);
    let mut outcomes = Vec::new();
    for n in 0..40u8 {
        let from = format!("198.51.{n}.7");
        let code = if n % 2 == 0 {
            wrong.as_str()
        } else {
            "aaaaa-bbbbb"
        };
        let r = client::scope(
            Some(&from),
            auth.sign_in("anna", "correct horse battery", Some(code)),
        )
        .await;
        outcomes.push(match r {
            Err(AuthError::InvalidTotp) => 0,
            Err(AuthError::TooManyAttempts { retry_after_secs }) => retry_after_secs,
            other => panic!("{other:?}"),
        });
    }
    let checked = outcomes.iter().filter(|w| **w == 0).count();
    assert_eq!(
        checked,
        usize::try_from(free).unwrap() + 1,
        "wrong codes checked from 40 addresses: {outcomes:?}"
    );
    assert!(outcomes[40 - 1] >= 1, "the account waits: {outcomes:?}");
    // the wait grows with the next failure, from yet another address
    clock.advance(Duration::seconds(outcomes[40 - 1]));
    let wrong = a_wrong_code(&totp, &clock);
    for (from, expect_wait) in [("192.0.2.1", 0), ("192.0.2.2", 2)] {
        let r = client::scope(
            Some(from),
            auth.sign_in("anna", "correct horse battery", Some(&wrong)),
        )
        .await;
        match (r, expect_wait) {
            (Err(AuthError::InvalidTotp), 0) => {}
            (Err(AuthError::TooManyAttempts { retry_after_secs }), w) if w > 0 => {
                assert_eq!(retry_after_secs, w);
            }
            (other, _) => panic!("{other:?}"),
        }
    }
    // after the wait the right code works and clears the count
    clock.advance(Duration::seconds(2));
    auth.sign_in("anna", "correct horse battery", Some(&codes[0]))
        .await
        .unwrap();
    let wrong = a_wrong_code(&totp, &clock);
    for n in 0..free {
        let r = client::scope(
            Some(&format!("192.0.2.{}", 10 + n)),
            auth.sign_in("anna", "correct horse battery", Some(&wrong)),
        )
        .await;
        assert!(matches!(r, Err(AuthError::InvalidTotp)), "{r:?}");
    }
}

/// SEC2-1: wrong passwords never reach the code count, so strangers cannot make the owner
/// wait at the second step.
#[tokio::test]
async fn wrong_passwords_do_not_touch_the_code_count() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (_, token) = auth.sign_up(su("anna")).await.unwrap();
    let (totp, _) = totp_on(&auth, &clock, &token).await;
    clock.advance(Duration::seconds(30));
    for n in 0..40u8 {
        let r = client::scope(
            Some(&format!("198.51.{n}.7")),
            auth.sign_in("anna", "wrong password!", Some("123456")),
        )
        .await;
        assert!(matches!(r, Err(AuthError::InvalidCredentials)), "{r:?}");
    }
    let now = u64::try_from(clock.now().timestamp()).unwrap();
    client::scope(
        Some("203.0.113.7"),
        auth.sign_in(
            "anna",
            "correct horse battery",
            Some(&totp.generate(now).to_string()),
        ),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn recovery_codes_are_shown_once_hashed_and_work_once() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (user, token) = auth.sign_up(su("anna")).await.unwrap();
    let (_, codes) = totp_on(&auth, &clock, &token).await;
    assert_eq!(codes.len(), RECOVERY_CODES);
    assert!(
        codes
            .iter()
            .all(|c| c.len() == 11 && c.as_bytes()[5] == b'-')
    );
    let stored = auth.store().recovery_codes(user.id).await.unwrap();
    assert!(stored.iter().all(|c| c.code_hash.starts_with("$argon2id$")));
    assert!(
        stored
            .iter()
            .all(|c| !codes.iter().any(|p| c.code_hash.contains(p)))
    );
    let code = codes[3].to_uppercase();
    assert!(
        auth.sign_in("anna", "correct horse battery", Some(&code))
            .await
            .is_ok()
    );
    assert!(matches!(
        auth.sign_in("anna", "correct horse battery", Some(&code))
            .await,
        Err(AuthError::InvalidTotp)
    ));
}

#[tokio::test]
async fn disabled_accounts_cannot_sign_in_and_lose_their_sessions() {
    let (auth, _, _) = env(SignupMode::Open);
    let (admin, _) = auth.sign_up(su("anna")).await.unwrap();
    let (ben, ben_token) = auth.sign_up(su("ben")).await.unwrap();
    auth.set_disabled(ben.id, Some(admin.id), true, None)
        .await
        .unwrap();
    assert!(auth.session(&ben_token.0).await.unwrap().is_none());
    assert!(matches!(
        auth.sign_in("ben", "correct horse battery", None).await,
        Err(AuthError::AccountDisabled)
    ));
    auth.set_disabled(ben.id, Some(admin.id), false, None)
        .await
        .unwrap();
    assert!(
        auth.sign_in("ben", "correct horse battery", None)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_locked_authenticator_says_so() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (user, token) = auth.sign_up(su("anna")).await.unwrap();
    totp_on(&auth, &clock, &token).await;
    let mut c = auth.store().credentials(user.id).await.unwrap().unwrap();
    c.totp_locked = true;
    c.totp_secret = None;
    auth.store().put_credentials(&c).await.unwrap();
    assert!(matches!(
        auth.sign_in("anna", "correct horse battery", Some("123456"))
            .await,
        Err(AuthError::TotpLocked)
    ));
}

#[tokio::test]
async fn admin_signup_invites_work_once_and_can_be_revoked() {
    let (auth, clock, _) = env(SignupMode::Invite);
    let (admin, _) = auth.sign_up(su("anna")).await.unwrap();
    let (inv, token) = auth
        .create_signup_invite(admin.id, Some("ben@example.org".into()), None, None)
        .await
        .unwrap();
    assert_eq!(auth.signup_invites().await.unwrap().len(), 1);
    let mut ben = su("ben");
    ben.invite = Some(token.clone());
    let (u, _) = auth.sign_up(ben).await.unwrap();
    assert_eq!(u.role, Role::Member);
    let mut carl = su("carl");
    carl.invite = Some(token);
    assert!(matches!(
        auth.sign_up(carl).await,
        Err(AuthError::InvalidInvite)
    ));
    let (inv2, token2) = auth
        .create_signup_invite(admin.id, None, None, None)
        .await
        .unwrap();
    assert_ne!(inv.id, inv2.id);
    auth.revoke_signup_invite(inv2.id, None).await.unwrap();
    let mut dora = su("dora");
    dora.invite = Some(token2.clone());
    assert!(matches!(
        auth.sign_up(dora).await,
        Err(AuthError::InvalidInvite)
    ));
    let (_, token3) = auth
        .create_signup_invite(admin.id, None, None, None)
        .await
        .unwrap();
    clock.advance(Duration::days(15));
    let mut eve = su("eve");
    eve.invite = Some(token3);
    assert!(matches!(
        auth.sign_up(eve).await,
        Err(AuthError::InvalidInvite)
    ));
}

#[tokio::test]
async fn the_oidc_reauth_hook_needs_a_recent_auth_time_of_the_same_identity() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (user, token) = auth.sign_up(su("anna")).await.unwrap();
    auth.store()
        .link_identity(&OidcIdentity::new(
            "https://idp",
            "s1",
            user.id,
            clock.now(),
        ))
        .await
        .unwrap();
    clock.advance(Duration::hours(2));
    let old = clock.now() - Duration::minutes(10);
    assert!(matches!(
        auth.confirm_oidc_reauth(&token.0, "https://idp", "s1", "idp", old)
            .await,
        Err(AuthError::ReauthRequired)
    ));
    assert!(matches!(
        auth.confirm_oidc_reauth(&token.0, "https://idp", "other", "idp", clock.now())
            .await,
        Err(AuthError::ReauthRequired)
    ));
    let fresh = auth
        .confirm_oidc_reauth(&token.0, "https://idp", "s1", "idp", clock.now())
        .await
        .unwrap();
    assert!(auth.require_fresh(&fresh.0).await.is_ok());
}

/// UX2-S4 / UX2-11: two sign-ups racing on one single-use admin invite: exactly one wins.
#[tokio::test]
async fn a_single_use_invite_admits_one_of_two_racing_sign_ups() {
    let (auth, _, _) = env(SignupMode::Invite);
    let (admin, _) = auth.sign_up(su("anna")).await.unwrap();
    let (inv, token) = auth
        .create_signup_invite(admin.id, None, None, None)
        .await
        .unwrap();
    let mut ben = su("ben");
    ben.invite = Some(token.clone());
    let mut carl = su("carl");
    carl.invite = Some(token);
    let (b, c) = tokio::join!(auth.sign_up(ben), auth.sign_up(carl));
    let won: Vec<_> = [&b, &c].into_iter().filter(|r| r.is_ok()).collect();
    assert_eq!(won.len(), 1, "{b:?} / {c:?}");
    assert!(
        [&b, &c]
            .into_iter()
            .any(|r| matches!(r, Err(AuthError::InvalidInvite)))
    );
    let used = auth
        .signup_invites()
        .await
        .unwrap()
        .into_iter()
        .find(|i| i.id == inv.id)
        .unwrap();
    let winner = won[0].as_ref().unwrap().0.id;
    assert_eq!(used.used_by, Some(winner));
}

// A refused sign-up (the name is taken) does not use up the invite.
#[tokio::test]
async fn a_failed_sign_up_does_not_use_up_the_invite() {
    let (auth, _, _) = env(SignupMode::Invite);
    let (admin, _) = auth.sign_up(su("anna")).await.unwrap();
    let (_, token) = auth
        .create_signup_invite(admin.id, None, None, None)
        .await
        .unwrap();
    let mut anna_again = su("anna");
    anna_again.invite = Some(token.clone());
    assert!(matches!(
        auth.sign_up(anna_again).await,
        Err(AuthError::UsernameTaken)
    ));
    let mut ben = su("ben");
    ben.invite = Some(token);
    auth.sign_up(ben).await.unwrap();
}

/// UX2-S6: guessing the current password through the password change is slowed like
/// sign-in, and shares its count (SEC-5: a growing wait, no lock).
#[tokio::test]
async fn changing_the_password_is_throttled_like_sign_in() {
    let (auth, clock, _) = env(SignupMode::Open);
    let (_, t1) = auth.sign_up(su("anna")).await.unwrap();
    let actor = auth.session(&t1.0).await.unwrap().unwrap();
    for _ in 0..=auth.settings().throttle.free_failures {
        assert!(matches!(
            auth.change_password(actor, "wrong password!", "another long secret")
                .await,
            Err(AuthError::InvalidCredentials)
        ));
    }
    assert!(matches!(
        auth.change_password(actor, "correct horse battery", "another long secret")
            .await,
        Err(AuthError::TooManyAttempts { .. })
    ));
    assert!(matches!(
        auth.sign_in("anna", "correct horse battery", None).await,
        Err(AuthError::TooManyAttempts { .. })
    ));
    clock.advance(Duration::seconds(1));
    auth.change_password(actor, "correct horse battery", "another long secret")
        .await
        .unwrap();
}

/// UX2-S9: a stored credential never prints its decrypted fields.
#[test]
fn stored_credentials_redact_their_fields_in_debug() {
    let c = crate::admin::StoredCredential {
        id: uuid::Uuid::nil(),
        connector: "tvdb".into(),
        user: None,
        fields: Ok([("pin".to_string(), "5678-secret".to_string())].into()),
        created_by: None,
        created_at: chrono::Utc::now(),
        last_tested_at: None,
        last_result: None,
    };
    let shown = format!("{c:?}");
    assert!(!shown.contains("5678-secret"), "{shown}");
    assert!(shown.contains("pin"), "field names stay visible: {shown}");
}
