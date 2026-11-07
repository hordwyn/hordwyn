//! The activity feed (Home, History): own events and plays, shared sessions, filters, paging.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{ActivityQuery, ActivityType, NewSession};
use domain::kind::{KindGroup, KindId};
use domain::progress::Progress;
use domain::session::Participant;
use testkit::TestEnv;

#[tokio::test]
async fn feed_is_newest_first_paged_and_skips_undone_events() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = common::item(&env.app, "show", "Dark").await;
    let mut ids = Vec::new();
    for e in 1..=5 {
        let ev = env
            .app
            .log_progress(
                me,
                show.id,
                Progress::Episode {
                    season: 1,
                    episode: e,
                },
                None,
                None,
            )
            .await
            .unwrap();
        ids.push(ev.id);
        env.clock.advance(chrono::Duration::hours(1));
    }
    env.app.undo_progress(me, ids[4]).await.unwrap();

    let page = env
        .app
        .activity_feed(
            me,
            ActivityQuery {
                limit: 2,
                ..ActivityQuery::default()
            },
        )
        .await
        .unwrap();
    let got: Vec<_> = page.rows.iter().map(|r| r.event_id.unwrap()).collect();
    assert_eq!(got, vec![ids[3], ids[2]]);
    assert!(page.rows[0].undoable);
    assert_eq!(page.rows[0].title, "Dark");
    assert_eq!(page.rows[0].group, KindGroup::Watch);
    let next = env
        .app
        .activity_feed(
            me,
            ActivityQuery {
                limit: 2,
                before: page.next,
                ..ActivityQuery::default()
            },
        )
        .await
        .unwrap();
    let got: Vec<_> = next.rows.iter().map(|r| r.event_id.unwrap()).collect();
    assert_eq!(got, vec![ids[1], ids[0]]);
    assert_eq!(next.next, None, "the end");
}

#[tokio::test]
async fn plays_watched_with_and_shared_sessions_with_filters() {
    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    // alice's companion "Bob" is linked to bob
    let c = env.app.create_companion(alice, "Bob").await.unwrap();
    let req = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();

    let game = common::item(&env.app, "board_game", "Azul").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    let book = common::item(&env.app, "book", "Dune").await;
    env.app
        .log_session(
            alice,
            NewSession::new(
                game.id,
                vec![
                    Participant::new(alice_user.self_companion),
                    Participant::new(c.id),
                ],
            ),
        )
        .await
        .unwrap();
    env.clock.advance(chrono::Duration::hours(1));
    // "watched with": a session plus a Finished event at the same time, one row
    env.app
        .log_session(
            alice,
            NewSession::new(
                movie.id,
                vec![
                    Participant::new(alice_user.self_companion),
                    Participant::new(c.id),
                ],
            ),
        )
        .await
        .unwrap();
    env.app
        .log_progress(alice, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    env.clock.advance(chrono::Duration::hours(1));
    env.app
        .log_progress(
            alice,
            book.id,
            Progress::Paged {
                page: Some(40),
                chapter: None,
                volume: None,
            },
            None,
            None,
        )
        .await
        .unwrap();

    let mine = env
        .app
        .activity_feed(alice, ActivityQuery::default())
        .await
        .unwrap();
    let rows: Vec<_> = mine
        .rows
        .iter()
        .map(|r| (r.title.as_str(), r.row_type, r.participants.len()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Dune", ActivityType::Progress, 0),
            ("Heat", ActivityType::Progress, 2),
            ("Azul", ActivityType::Session, 2)
        ]
    );
    assert!(mine.rows.iter().all(|r| !r.shared));

    let with_bob = ActivityQuery {
        person: Some(c.id),
        ..ActivityQuery::default()
    };
    assert_eq!(
        env.app
            .activity_feed(alice, with_bob)
            .await
            .unwrap()
            .rows
            .len(),
        2
    );
    let play = ActivityQuery {
        group: Some(KindGroup::Play),
        ..ActivityQuery::default()
    };
    assert_eq!(
        env.app.activity_feed(alice, play).await.unwrap().rows.len(),
        1
    );
    let movies = ActivityQuery {
        kind: Some(KindId::new("movie").unwrap()),
        ..ActivityQuery::default()
    };
    assert_eq!(
        env.app
            .activity_feed(alice, movies)
            .await
            .unwrap()
            .rows
            .len(),
        1
    );

    // bob sees alice's sessions as shared rows (alice shares sessions by default)
    let theirs = env
        .app
        .activity_feed(
            bob,
            ActivityQuery {
                shared_only: true,
                ..ActivityQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(theirs.rows.len(), 2);
    assert!(theirs.rows.iter().all(|r| r.shared && !r.undoable));
    assert_eq!(
        theirs.rows[0].owner_name.as_deref(),
        Some(alice_user.display_name.as_str())
    );
    assert!(theirs.rows[0].participants.iter().any(|p| p.is_me));

    env.app.set_privacy(alice, false).await.unwrap();
    let hidden = env
        .app
        .activity_feed(bob, ActivityQuery::default())
        .await
        .unwrap();
    assert_eq!(
        hidden.rows.len(),
        0,
        "privacy: alice no longer shares sessions"
    );
}
