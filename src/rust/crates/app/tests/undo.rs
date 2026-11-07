//! `undo_progress`: a compensating event, the derived state returns to before.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{AppError, NextStep};
use domain::kind::Status;
use domain::progress::Progress;
use testkit::TestEnv;

#[tokio::test]
async fn undo_appends_a_compensating_event_and_restores_status_and_next_step() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = common::item(&env.app, "show", "Dark").await;
    env.app.add_to_library(me, show.id, None).await.unwrap();
    let ep = |e| Progress::Episode {
        season: 1,
        episode: e,
    };
    env.app
        .log_progress(me, show.id, ep(1), None, None)
        .await
        .unwrap();
    let before = env.app.library_row(me, show.id).await.unwrap();
    let second = env
        .app
        .log_progress(me, show.id, ep(2), None, None)
        .await
        .unwrap();
    let undo = env.app.undo_progress(me, second.id).await.unwrap();
    assert_eq!(
        undo.progress,
        Progress::Undo {
            event_id: second.id
        }
    );
    // append-only: both events are still stored
    assert_eq!(env.app.progress(me, show.id).await.unwrap().len(), 3);
    let after = env.app.library_row(me, show.id).await.unwrap();
    assert_eq!(after.next_step, before.next_step);
    assert_eq!(
        after.next_step,
        NextStep::Episode {
            season: 1,
            episode: 2
        }
    );
    assert_eq!(after.summary, before.summary);
    // the effective history no longer has it
    let history = env.app.history(me, show.id).await.unwrap();
    assert_eq!(history.len(), 1);
    // undo twice is a no-op that returns the same compensating event
    let again = env.app.undo_progress(me, second.id).await.unwrap();
    assert_eq!(again.id, undo.id);
    assert_eq!(env.app.progress(me, show.id).await.unwrap().len(), 3);
}

#[tokio::test]
async fn undo_restores_the_status_the_event_changed_but_not_a_later_manual_one() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    env.app.add_to_library(me, movie.id, None).await.unwrap();
    let e = env
        .app
        .log_progress(me, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    assert_eq!(
        env.app
            .entry(me, movie.id)
            .await
            .unwrap()
            .unwrap()
            .status
            .value,
        Status::completed()
    );
    env.app.undo_progress(me, e.id).await.unwrap();
    assert_eq!(
        env.app
            .entry(me, movie.id)
            .await
            .unwrap()
            .unwrap()
            .status
            .value,
        Status::planned()
    );

    let book = common::item(&env.app, "book", "Dune").await;
    let page = Progress::Paged {
        page: Some(30),
        chapter: None,
        volume: None,
    };
    let e = env
        .app
        .log_progress(me, book.id, page, None, None)
        .await
        .unwrap();
    env.app
        .set_status(me, book.id, Status::new("paused"), None)
        .await
        .unwrap();
    env.app.undo_progress(me, e.id).await.unwrap();
    assert_eq!(
        env.app
            .entry(me, book.id)
            .await
            .unwrap()
            .unwrap()
            .status
            .value,
        Status::new("paused"),
        "a status chosen by hand after the event stays"
    );
}

#[tokio::test]
async fn only_own_recent_events_can_be_undone_and_undo_removes_a_logged_play() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, bob) = common::user(&env, "bob").await;
    let movie = common::item(&env.app, "movie", "Alien").await;
    let e = env
        .app
        .log_progress(me, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    assert!(matches!(
        env.app.undo_progress(bob, e.id).await,
        Err(AppError::NotFound(_))
    ));
    env.clock.advance(chrono::Duration::hours(25));
    assert!(matches!(
        env.app.undo_progress(me, e.id).await,
        Err(AppError::Conflict(_))
    ));

    let game = common::item(&env.app, "board_game", "Azul").await;
    let me_user = env.app.user(me.user_id).await.unwrap();
    let session = env
        .app
        .log_session(
            me,
            app::NewSession::new(
                game.id,
                vec![domain::session::Participant::new(me_user.self_companion)],
            ),
        )
        .await
        .unwrap();
    let play = env.app.history(me, game.id).await.unwrap().pop().unwrap();
    env.app.undo_progress(me, play.id).await.unwrap();
    assert!(
        env.app.session(me, session.id).await.is_err(),
        "the play is gone"
    );
    let undo_of_undo = env.app.progress(me, game.id).await.unwrap().pop().unwrap();
    assert!(
        env.app.undo_progress(me, undo_of_undo.id).await.is_err(),
        "an undo cannot be undone"
    );
}

#[tokio::test]
async fn a_replayed_play_is_one_session_and_stays_gone_after_undo() {
    // UX1-3
    let env = TestEnv::new();
    let (user, me) = common::user(&env, "alice").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let mut play = app::NewSession::new(
        game.id,
        vec![domain::session::Participant::new(user.self_companion)],
    );
    play.client_op_id = Some(uuid::Uuid::from_u128(7));
    let (s1, e1) = env.app.log_play(me, play.clone()).await.unwrap();
    let (s2, e2) = env.app.log_play(me, play.clone()).await.unwrap();
    assert_eq!((s1.id, e1.id), (s2.id, e2.id));
    assert_eq!(env.app.my_sessions(me).await.unwrap().len(), 1);
    env.app.undo_progress(me, e1.id).await.unwrap();
    assert_eq!(env.app.my_sessions(me).await.unwrap().len(), 0);
    let (_, e3) = env.app.log_play(me, play).await.unwrap();
    assert_eq!(e3.id, e1.id, "the replay answers the undone event");
    assert_eq!(
        env.app.my_sessions(me).await.unwrap().len(),
        0,
        "and stores nothing"
    );
}
