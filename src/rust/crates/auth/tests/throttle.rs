//! SEC-5 (DEC-535): failed sign-ins slow the pair of account and client address down, a
//! client address has a budget across all names, and nobody can shut an account by guessing
//! from somewhere else.

use std::sync::Arc;

use auth::{Auth, AuthError, AuthSettings, MemAuthStore, SignUp, SignupMode, client};
use chrono::Duration;
use testkit::fakes::FixedClock;

const PASSWORD: &str = "correct horse battery";
const HOME: &str = "203.0.113.7";
const ELSEWHERE: &str = "198.51.100.20";

async fn env() -> (Auth, Arc<FixedClock>, testkit::TestEnv) {
    let t = testkit::TestEnv::new();
    let a = Auth::new(
        t.app.clone(),
        Arc::new(MemAuthStore::default()),
        AuthSettings {
            signup: SignupMode::Open,
            ..AuthSettings::default()
        },
    );
    for name in ["admin", "anna"] {
        a.sign_up(SignUp {
            username: name.into(),
            display_name: name.into(),
            email: None,
            password: PASSWORD.into(),
            invite: None,
        })
        .await
        .unwrap();
    }
    (a, t.clock.clone(), t)
}

async fn wrong(a: &Auth, from: &str, name: &str) -> AuthError {
    client::scope(Some(from), a.sign_in(name, "wrong password!", None))
        .await
        .unwrap_err()
}

fn wait_of(e: &AuthError) -> i64 {
    match e {
        AuthError::TooManyAttempts { retry_after_secs } => *retry_after_secs,
        e => panic!("expected a wait, got {e:?}"),
    }
}

#[tokio::test]
async fn the_delay_grows_with_every_further_failure() {
    let (a, clock, _t) = env().await;
    let s = a.settings().throttle;
    assert_eq!((s.free_failures, s.delay_base_secs), (3, 1), "defaults");
    // the free failures cost nothing
    for _ in 0..s.free_failures {
        assert!(matches!(
            wrong(&a, HOME, "anna").await,
            AuthError::InvalidCredentials
        ));
    }
    let mut waits = Vec::new();
    for _ in 0..5 {
        assert!(matches!(
            wrong(&a, HOME, "anna").await,
            AuthError::InvalidCredentials
        ));
        // now the pair waits, whatever the password
        let wait = wait_of(&wrong(&a, HOME, "anna").await);
        waits.push(wait);
        clock.advance(Duration::seconds(wait));
    }
    assert_eq!(waits, [1, 2, 4, 8, 16]);
}

#[tokio::test]
async fn the_delay_stops_growing_at_its_maximum() {
    let (a, clock, _t) = env().await;
    a.update_settings(|s| {
        s.throttle.delay_max_secs = 5;
        s.throttle.address_failures = 1000;
    });
    let mut last = 0;
    for _ in 0..24 {
        match wrong(&a, HOME, "anna").await {
            AuthError::InvalidCredentials => {}
            e => {
                last = wait_of(&e);
                clock.advance(Duration::seconds(last));
            }
        }
    }
    assert_eq!(last, 5);
}

#[tokio::test]
async fn the_right_password_waits_too_and_signs_in_after_the_delay() {
    let (a, clock, _t) = env().await;
    for _ in 0..4 {
        wrong(&a, HOME, "anna").await;
    }
    let refused = client::scope(Some(HOME), a.sign_in("ANNA", PASSWORD, None))
        .await
        .unwrap_err();
    assert_eq!(wait_of(&refused), 1);
    clock.advance(Duration::seconds(1));
    client::scope(Some(HOME), a.sign_in("anna", PASSWORD, None))
        .await
        .unwrap();
    // a success starts the pair again: the next failures are free
    for _ in 0..3 {
        assert!(matches!(
            wrong(&a, HOME, "anna").await,
            AuthError::InvalidCredentials
        ));
    }
}

#[tokio::test]
async fn an_address_over_its_budget_is_refused_for_every_name() {
    let (a, clock, _t) = env().await;
    let budget = a.settings().throttle.address_failures;
    assert_eq!(budget, 30, "default");
    // spraying: a new name every time, so no pair ever waits
    for n in 0..budget {
        assert!(matches!(
            wrong(&a, ELSEWHERE, &format!("name{n}")).await,
            AuthError::InvalidCredentials
        ));
    }
    let e = wrong(&a, ELSEWHERE, "another").await;
    assert!(wait_of(&e) > 60, "until the window ends: {e:?}");
    let e = client::scope(Some(ELSEWHERE), a.sign_in("anna", PASSWORD, None))
        .await
        .unwrap_err();
    assert!(matches!(e, AuthError::TooManyAttempts { .. }), "{e:?}");
    // other addresses are not affected
    client::scope(Some(HOME), a.sign_in("anna", PASSWORD, None))
        .await
        .unwrap();
    // and the window ends
    clock.advance(Duration::minutes(15));
    client::scope(Some(ELSEWHERE), a.sign_in("anna", PASSWORD, None))
        .await
        .unwrap();
}

#[tokio::test]
async fn the_admin_name_cannot_be_locked_by_guesses_from_another_address() {
    let (a, clock, _t) = env().await;
    // someone guesses at the admin name until their address is out of budget
    let mut failures = 0;
    for _ in 0..60 {
        match wrong(&a, ELSEWHERE, "admin").await {
            AuthError::InvalidCredentials => failures += 1,
            AuthError::TooManyAttempts { retry_after_secs } => {
                clock.advance(Duration::seconds(retry_after_secs.min(4)));
            }
            e => panic!("{e:?}"),
        }
    }
    assert!(failures >= 5, "{failures} wrong guesses were checked");
    // the owner signs in from their own address at once
    client::scope(Some(HOME), a.sign_in("admin", PASSWORD, None))
        .await
        .unwrap();
}

#[tokio::test]
async fn addresses_of_one_ipv6_network_share_a_budget() {
    let (a, _clock, _t) = env().await;
    a.update_settings(|s| s.throttle.address_failures = 4);
    for host in 1..=4 {
        let from = format!("2001:db8:1:2::{host}");
        assert!(matches!(
            wrong(&a, &from, &format!("name{host}")).await,
            AuthError::InvalidCredentials
        ));
    }
    let e = wrong(&a, "2001:db8:1:2:ffff::9", "anna").await;
    assert!(matches!(e, AuthError::TooManyAttempts { .. }), "{e:?}");
    // the next /64 is someone else
    assert!(matches!(
        wrong(&a, "2001:db8:1:3::1", "anna").await,
        AuthError::InvalidCredentials
    ));
}

#[tokio::test]
async fn a_pair_starts_again_after_a_quiet_window() {
    let (a, clock, _t) = env().await;
    for _ in 0..4 {
        wrong(&a, HOME, "anna").await;
    }
    clock.advance(Duration::minutes(16));
    for _ in 0..3 {
        assert!(matches!(
            wrong(&a, HOME, "anna").await,
            AuthError::InvalidCredentials
        ));
    }
}

#[tokio::test]
async fn without_a_client_address_the_account_alone_is_slowed() {
    let (a, clock, _t) = env().await;
    // the command line and in-process callers have no address
    for _ in 0..4 {
        assert!(matches!(
            a.sign_in("anna", "wrong password!", None).await,
            Err(AuthError::InvalidCredentials)
        ));
    }
    let e = a.sign_in("anna", PASSWORD, None).await.unwrap_err();
    assert_eq!(wait_of(&e), 1);
    // a request with an address has its own count
    client::scope(Some(HOME), a.sign_in("anna", PASSWORD, None))
        .await
        .unwrap();
    clock.advance(Duration::seconds(1));
    a.sign_in("anna", PASSWORD, None).await.unwrap();
}

#[tokio::test]
async fn re_auth_and_password_change_share_the_count_of_the_pair() {
    let (a, _clock, _t) = env().await;
    let token = client::scope(Some(HOME), a.sign_in("anna", PASSWORD, None))
        .await
        .unwrap();
    let actor = a.session(&token.0).await.unwrap().unwrap();
    for _ in 0..2 {
        let e = client::scope(Some(HOME), a.reauth(&token.0, "wrong password!", None))
            .await
            .unwrap_err();
        assert!(matches!(e, AuthError::InvalidCredentials), "{e:?}");
    }
    for _ in 0..2 {
        let e = client::scope(
            Some(HOME),
            a.change_password(actor, "wrong password!", "a new long password"),
        )
        .await
        .unwrap_err();
        assert!(matches!(e, AuthError::InvalidCredentials), "{e:?}");
    }
    let e = client::scope(Some(HOME), a.reauth(&token.0, PASSWORD, None))
        .await
        .unwrap_err();
    assert_eq!(wait_of(&e), 1);
}

#[tokio::test]
async fn unknown_reset_links_use_up_the_budget_of_the_address() {
    let (a, _clock, _t) = env().await;
    a.update_settings(|s| s.throttle.address_failures = 3);
    for n in 0..3 {
        let r = client::scope(
            Some(ELSEWHERE),
            a.reset_password(&format!("pwr_guess{n}"), "a new long password"),
        )
        .await
        .unwrap();
        assert_eq!(r, None);
    }
    let e = client::scope(
        Some(ELSEWHERE),
        a.reset_password("pwr_guess", "a new long password"),
    )
    .await
    .unwrap_err();
    assert!(matches!(e, AuthError::TooManyAttempts { .. }), "{e:?}");
    // sign-in from there is refused as well: one budget per address
    let e = wrong(&a, ELSEWHERE, "anna").await;
    assert!(matches!(e, AuthError::TooManyAttempts { .. }), "{e:?}");
}

#[tokio::test]
async fn wrong_codes_while_enrolling_an_authenticator_are_slowed() {
    let (a, _clock, _t) = env().await;
    let token = client::scope(Some(HOME), a.sign_in("anna", PASSWORD, None))
        .await
        .unwrap();
    let actor = a.session(&token.0).await.unwrap().unwrap();
    a.totp_begin(actor).await.unwrap();
    for _ in 0..4 {
        let e = client::scope(Some(HOME), a.totp_confirm(actor, "000000"))
            .await
            .unwrap_err();
        assert!(matches!(e, AuthError::InvalidTotp), "{e:?}");
    }
    let e = client::scope(Some(HOME), a.totp_confirm(actor, "000000"))
        .await
        .unwrap_err();
    assert_eq!(wait_of(&e), 1);
}

#[tokio::test]
async fn usernames_are_normalized_and_bad_ones_named() {
    let t = testkit::TestEnv::new();
    let a = Auth::new(
        t.app.clone(),
        Arc::new(MemAuthStore::default()),
        AuthSettings {
            signup: SignupMode::Open,
            ..AuthSettings::default()
        },
    );
    let up = |name: &str| SignUp {
        username: name.into(),
        display_name: String::new(),
        email: None,
        password: PASSWORD.into(),
        invite: None,
    };
    let (u, _) = a.sign_up(up(" Robin ")).await.unwrap();
    assert_eq!(u.username, "robin");
    assert_eq!(u.display_name, "robin", "empty display name falls back");
    a.sign_in("ROBIN", PASSWORD, None).await.unwrap();
    assert!(matches!(
        a.sign_up(up("me@example.org")).await,
        Err(AuthError::InvalidUsername(
            contracts::auth::UsernameRule::InvalidChars
        ))
    ));
}
