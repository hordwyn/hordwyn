mod common;

use app::{AppError, NewSession};
use domain::progress::Progress;
use domain::session::Participant;
use testkit::TestEnv;

#[tokio::test]
async fn session_on_sessions_kind_appends_progress_event() {
    let env = TestEnv::new();
    let (u, me) = common::user(&env, "alice").await;
    let game = common::item(&env.app, "board_game", "Brass").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    let mut p_me = Participant::new(u.self_companion);
    p_me.score = Some(152.0);
    p_me.winner = true;
    let mut p_anna = Participant::new(anna.id);
    p_anna.score = Some(140.0);
    let s = env
        .app
        .log_session(me, NewSession::new(game.id, vec![p_me, p_anna]))
        .await
        .unwrap();
    let events = env.app.progress(me, game.id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].progress, Progress::Session { session_id: s.id });
    assert!(env.app.entry(me, game.id).await.unwrap().is_some());
}

#[tokio::test]
async fn watched_with_reuses_sessions_for_other_kinds() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Coco").await;
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    env.app
        .log_session(
            me,
            NewSession::new(movie.id, vec![Participant::new(anna.id)]),
        )
        .await
        .unwrap();
    assert_eq!(env.app.progress(me, movie.id).await.unwrap().len(), 0);
}

#[tokio::test]
async fn participants_must_be_own_companions_and_unique() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let game = common::item(&env.app, "board_game", "Root").await;
    let bobs = env.app.create_companion(bob, "Zed").await.unwrap();
    assert!(matches!(
        env.app
            .log_session(
                me,
                NewSession::new(game.id, vec![Participant::new(bobs.id)])
            )
            .await,
        Err(AppError::Forbidden)
    ));
    let anna = env.app.create_companion(me, "Anna").await.unwrap();
    assert!(
        env.app
            .log_session(
                me,
                NewSession::new(
                    game.id,
                    vec![Participant::new(anna.id), Participant::new(anna.id)]
                )
            )
            .await
            .is_err()
    );
}

/// Alice logs a play with her companion "B." who is linked to Bob.
async fn shared_play(
    env: &TestEnv,
    kind: &str,
) -> (
    domain::user::Actor,
    domain::user::Actor,
    domain::session::Session,
) {
    let (_, alice) = common::user(env, "alice").await;
    let (bob_user, bob) = common::user(env, "bob").await;
    let item = common::item(&env.app, kind, "Azul").await;
    let c = env.app.create_companion(alice, "B.").await.unwrap();
    let req = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();
    let mut p = Participant::new(c.id);
    p.score = Some(61.0);
    p.winner = true;
    let mut new = NewSession::new(item.id, vec![p]);
    new.notes = Some("alice's private note".into());
    let s = env.app.log_session(alice, new).await.unwrap();
    (alice, bob, s)
}

#[tokio::test]
async fn claiming_a_shared_session_copies_it_once() {
    let env = TestEnv::new();
    let (alice, bob, s) = shared_play(&env, "board_game").await;
    let copy = env.app.claim_session(bob, s.id).await.unwrap();
    assert_ne!(copy.id, s.id);
    assert_eq!(copy.owner, bob.user_id);
    assert_eq!(copy.started_at, s.started_at);
    assert_eq!(copy.notes, None, "notes stay with the owner");
    let me = env.app.user(bob.user_id).await.unwrap();
    assert_eq!(copy.participants.len(), 1);
    assert_eq!(copy.participants[0].companion_id, me.self_companion);
    assert_eq!(copy.participants[0].score, Some(61.0));
    assert!(copy.participants[0].winner);
    let events = env.app.progress(bob, s.item_id).await.unwrap();
    assert_eq!(events.len(), 1);

    // claiming again is a no-op
    let again = env.app.claim_session(bob, s.id).await.unwrap();
    assert_eq!(again.id, copy.id);
    assert_eq!(env.app.progress(bob, s.item_id).await.unwrap().len(), 1);
    // the owner's history is untouched, and the owner cannot claim their own
    assert_eq!(env.app.session(alice, s.id).await.unwrap(), s);
    assert!(matches!(
        env.app.claim_session(alice, s.id).await,
        Err(AppError::Conflict(_))
    ));
    // the copy is not shared back as "with me" to anyone
    assert_eq!(env.app.shared_sessions(alice).await.unwrap().len(), 0);
}

#[tokio::test]
async fn claiming_a_watched_with_session_logs_a_finish() {
    let env = TestEnv::new();
    let (_, bob, s) = shared_play(&env, "movie").await;
    env.app.claim_session(bob, s.id).await.unwrap();
    let events = env.app.progress(bob, s.item_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].progress, Progress::Finished);
    assert_eq!(events[0].occurred_at, s.started_at);
}

#[tokio::test]
async fn strangers_cannot_claim() {
    let env = TestEnv::new();
    let (_, _, s) = shared_play(&env, "board_game").await;
    let (_, eve) = common::user(&env, "eve").await;
    assert!(matches!(
        env.app.claim_session(eve, s.id).await,
        Err(AppError::Forbidden)
    ));
}

#[tokio::test]
async fn owners_can_stop_sharing_sessions() {
    let env = TestEnv::new();
    let (alice, bob, s) = shared_play(&env, "board_game").await;
    assert_eq!(env.app.shared_sessions(bob).await.unwrap().len(), 1);
    let u = env.app.set_privacy(alice, false).await.unwrap();
    assert!(!u.share_sessions);
    assert_eq!(env.app.shared_sessions(bob).await.unwrap().len(), 0);
    assert!(matches!(
        env.app.session(bob, s.id).await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        env.app.claim_session(bob, s.id).await,
        Err(AppError::Forbidden)
    ));
    env.app.set_privacy(alice, true).await.unwrap();
    assert_eq!(env.app.shared_sessions(bob).await.unwrap().len(), 1);
}
