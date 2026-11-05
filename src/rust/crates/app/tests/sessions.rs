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
