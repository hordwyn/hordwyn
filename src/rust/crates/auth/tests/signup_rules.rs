//! UX phase 0 "form discipline": every sign-up rule has its own error, the availability check
//! normalises like sign-up, only answers when sign-up is possible and is throttled.

use std::sync::Arc;

use auth::{
    Auth, AuthError, AuthSettings, MemAuthStore, SignUp, SignupMode, USERNAME_CHECKS_PER_MINUTE,
};
use contracts::auth::{UsernameAvailability, UsernameRule};

fn auth(t: &testkit::TestEnv, signup: SignupMode) -> Auth {
    Auth::new(
        t.app.clone(),
        Arc::new(MemAuthStore::default()),
        AuthSettings {
            signup,
            ..AuthSettings::default()
        },
    )
}

fn up(name: &str) -> SignUp {
    SignUp {
        username: name.into(),
        display_name: String::new(),
        email: None,
        password: "correct horse battery".into(),
        invite: None,
    }
}

#[tokio::test]
async fn each_sign_up_rule_has_its_own_error() {
    let t = testkit::TestEnv::new();
    let a = auth(&t, SignupMode::Open);
    let rule = |r: Result<_, AuthError>| match r {
        Err(AuthError::InvalidUsername(rule)) => Some(rule),
        _ => None,
    };
    assert_eq!(
        rule(a.sign_up(up("ab")).await),
        Some(UsernameRule::TooShort)
    );
    assert_eq!(
        rule(a.sign_up(up(&"a".repeat(33))).await),
        Some(UsernameRule::TooLong)
    );
    assert_eq!(
        rule(a.sign_up(up("anna k")).await),
        Some(UsernameRule::Space)
    );
    assert_eq!(
        rule(a.sign_up(up("me@example.org")).await),
        Some(UsernameRule::InvalidChars)
    );
    let mut long_name = up("anna");
    long_name.display_name = "x".repeat(101);
    assert!(matches!(
        a.sign_up(long_name).await,
        Err(AuthError::DisplayNameTooLong)
    ));
    let mut short = up("anna");
    short.password = "short".into();
    assert!(matches!(
        a.sign_up(short).await,
        Err(AuthError::PasswordTooShort)
    ));
    let mut long = up("anna");
    long.password = "p".repeat(1025);
    assert!(matches!(
        a.sign_up(long).await,
        Err(AuthError::PasswordTooLong)
    ));
    let mut blank = up("Anna");
    blank.display_name = "   ".into();
    let (u, _) = a.sign_up(blank).await.unwrap();
    assert_eq!(u.display_name, "anna", "a blank display name falls back");
    assert!(matches!(
        a.sign_up(up(" ANNA ")).await,
        Err(AuthError::UsernameTaken)
    ));
}

#[tokio::test]
async fn sign_in_does_not_say_which_part_was_wrong() {
    let t = testkit::TestEnv::new();
    let a = auth(&t, SignupMode::Open);
    a.sign_up(up("anna")).await.unwrap();
    for name in ["a b", "x", "nobody", "anna"] {
        assert!(
            matches!(
                a.sign_in(name, "wrong password!", None).await,
                Err(AuthError::InvalidCredentials)
            ),
            "{name}"
        );
    }
}

#[tokio::test]
async fn availability_normalises_and_suggests_free_names() {
    let t = testkit::TestEnv::new();
    let a = auth(&t, SignupMode::Open);
    assert_eq!(
        a.username_available(" Anna ", None).await.unwrap(),
        UsernameAvailability::Available {
            username: "anna".into()
        },
        "the first account is always possible"
    );
    a.sign_up(up("anna")).await.unwrap();
    a.sign_up(up("anna2")).await.unwrap();
    assert_eq!(
        a.username_available("ANNA", None).await.unwrap(),
        UsernameAvailability::Taken {
            username: "anna".into(),
            suggestions: vec!["anna3".into(), "anna4".into(), "anna5".into()],
        }
    );
    assert_eq!(
        a.username_available("anna k", None).await.unwrap(),
        UsernameAvailability::Invalid {
            username: "anna k".into(),
            rule: UsernameRule::Space,
        }
    );
    let max = "b".repeat(32);
    a.sign_up(up(&max)).await.unwrap();
    let UsernameAvailability::Taken { suggestions, .. } =
        a.username_available(&max, None).await.unwrap()
    else {
        panic!("taken")
    };
    assert_eq!(
        suggestions[0],
        format!("{}2", "b".repeat(31)),
        "stays within 32"
    );
}

#[tokio::test]
async fn availability_answers_only_when_sign_up_is_possible() {
    let t = testkit::TestEnv::new();
    let a = auth(&t, SignupMode::Invite);
    let (_, session) = a.sign_up(up("anna")).await.unwrap();
    assert!(matches!(
        a.username_available("ben", None).await,
        Err(AuthError::SignupClosed)
    ));
    assert!(matches!(
        a.username_available("ben", Some("nope")).await,
        Err(AuthError::InvalidInvite)
    ));
    // a bad invite must not reveal a taken name on sign-up either
    let mut bad = up("anna");
    bad.invite = Some("nope".into());
    assert!(matches!(
        a.sign_up(bad).await,
        Err(AuthError::InvalidInvite)
    ));
    let actor = a.session(&session.0).await.unwrap().unwrap();
    let c = a.app().create_companion(actor, "Ben").await.unwrap();
    let (_, invite) = a.app().promote_companion(actor, c.id, None).await.unwrap();
    assert!(matches!(
        a.username_available("anna", Some(&invite)).await,
        Ok(UsernameAvailability::Taken { .. })
    ));
    let mut taken = up("anna");
    taken.invite = Some(invite.clone());
    assert!(matches!(
        a.sign_up(taken).await,
        Err(AuthError::UsernameTaken)
    ));
    let mut ok = up("ben");
    ok.invite = Some(invite);
    a.sign_up(ok).await.unwrap();

    let closed = auth(&t, SignupMode::Closed);
    assert!(matches!(
        closed.username_available("carl", None).await,
        Err(AuthError::SignupClosed)
    ));
}

#[tokio::test]
async fn availability_checks_are_throttled_per_instance() {
    let t = testkit::TestEnv::new();
    let a = auth(&t, SignupMode::Open);
    for i in 0..USERNAME_CHECKS_PER_MINUTE {
        a.username_available(&format!("name{i}"), None)
            .await
            .unwrap();
    }
    assert!(matches!(
        a.username_available("other", None).await,
        Err(AuthError::TooManyAttempts { .. })
    ));
    // a format check needs no lookup and is not limited
    assert!(matches!(
        a.username_available("a b", None).await,
        Ok(UsernameAvailability::Invalid { .. })
    ));
    t.clock.advance(chrono::Duration::seconds(61));
    a.username_available("other", None).await.unwrap();
}

#[test]
fn shared_rules_agree_with_the_domain() {
    use contracts::auth::{DISPLAY_NAME_MAX_LEN, check_username};
    assert_eq!(
        DISPLAY_NAME_MAX_LEN,
        domain::companion::MAX_COMPANION_NAME_CHARS
    );
    for name in [
        "ab",
        "abc",
        "anna",
        "a.b-c_d",
        "anna k",
        "Anna",
        "me@x",
        "j\u{f6}rg",
        "0123456789",
    ] {
        assert_eq!(
            check_username(name).is_ok(),
            domain::user::validate_username(name).is_ok(),
            "{name}"
        );
    }
    for n in [31, 32, 33] {
        let name = "a".repeat(n);
        assert_eq!(
            check_username(&name).is_ok(),
            domain::user::validate_username(&name).is_ok()
        );
    }
}

/// UX0-9: a closed instance answers `signup_closed` without a lookup, so those calls do not
/// use up the instance's check budget.
#[tokio::test]
async fn closed_sign_up_does_not_count_towards_the_check_limit() {
    let t = testkit::TestEnv::new();
    let open = auth(&t, SignupMode::Open);
    open.sign_up(up("admin")).await.unwrap();
    let closed = auth(&t, SignupMode::Closed);
    for i in 0..=USERNAME_CHECKS_PER_MINUTE {
        assert!(
            matches!(
                closed.username_available(&format!("name{i}"), None).await,
                Err(AuthError::SignupClosed)
            ),
            "call {i}"
        );
    }
}

/// UX0-4: the lock-out carries the seconds left, for the client's "try again in N minutes".
#[test]
fn too_many_attempts_carries_the_seconds_left() {
    assert_eq!(
        AuthError::TooManyAttempts {
            retry_after_secs: 90
        }
        .retry_after_secs(),
        Some(90)
    );
    assert_eq!(AuthError::SignupClosed.retry_after_secs(), None);
}
