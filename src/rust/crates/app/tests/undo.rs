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

/// Review OT2E-5: since DEC-588 an undo also reverts a status move whose write a later undo
/// restored. Outside a rewatch run that changes nothing: no kind has two events on one entry
/// that both move the status (`finished` is refused for a book and a show, and the single
/// model has no In progress move), so there the rule is still "exactly that write". Inside
/// runs the chain can be longer than one run: pinned here over two.
#[tokio::test]
async fn an_undo_chain_needs_two_moving_events_and_only_runs_have_them() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let page = |p| Progress::Paged {
        page: Some(p),
        chapter: None,
        volume: None,
    };
    let status = async |item| {
        let e = env.app.entry(me, item).await.unwrap().unwrap();
        e.status.value.as_str().to_string()
    };
    let book = common::item(&env.app, "book", "Dune").await;
    env.app.add_to_library(me, book.id, None).await.unwrap();
    let first = env
        .app
        .log_progress(me, book.id, page(10), None, None)
        .await
        .unwrap();
    assert!(first.status_change.is_some());
    let finished = env
        .app
        .log_progress(me, book.id, Progress::Finished, None, None)
        .await;
    assert!(finished.is_err(), "a book has no finished event");
    let second = env
        .app
        .log_progress(me, book.id, page(20), None, None)
        .await
        .unwrap();
    assert_eq!(second.status_change, None);
    env.app.undo_progress(me, second.id).await.unwrap();
    assert_eq!(status(book.id).await, "in_progress");
    env.app.undo_progress(me, first.id).await.unwrap();
    assert_eq!(status(book.id).await, "planned");

    // two rewatch runs of a one-episode show, taken back newest first
    let mut show = common::item(&env.app, "show", "Dark").await;
    show.attributes = serde_json::json!({ "seasons": [{"number": 1, "episode_count": 1}] });
    env.store.state().catalog.insert(show.id, show.clone());
    let ep = Progress::Episode {
        season: 1,
        episode: 1,
    };
    let log = async |p: Progress| {
        env.clock.advance(chrono::Duration::minutes(1));
        env.app
            .log_progress(me, show.id, p, None, None)
            .await
            .unwrap()
    };
    log(ep.clone()).await;
    env.app
        .set_status(me, show.id, Status::completed(), None)
        .await
        .unwrap();
    let start_1 = log(Progress::Rewatch).await;
    let end_1 = log(ep.clone()).await;
    assert_eq!(status(show.id).await, "completed");
    let start_2 = log(Progress::Rewatch).await;
    assert_eq!(status(show.id).await, "in_progress");
    for (event, after) in [
        (start_2, "completed"),
        (end_1, "in_progress"),
        (start_1, "completed"),
    ] {
        env.clock.advance(chrono::Duration::seconds(1));
        env.app.undo_progress(me, event.id).await.unwrap();
        assert_eq!(status(show.id).await, after);
    }
}
