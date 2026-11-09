//! UX4-16: "watched with" pairs near a page boundary. One full read pairs each event with the
//! newest unused own session of the item within two minutes, newest event first; a page that
//! starts below an event more than two minutes above the boundary must still see that event
//! take its session. Expected values are written out, not taken from the feed itself.

mod common;

use app::ports::Clock;
use app::{ActivityQuery, NewSession};
use chrono::Duration;
use domain::progress::Progress;
use domain::session::Participant;
use testkit::TestEnv;

#[tokio::test]
async fn a_session_taken_by_an_event_above_the_page_stays_taken() {
    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    let book = common::item(&env.app, "book", "Dune").await;
    let t0 = env.clock.now();
    env.clock.advance(Duration::seconds(10));
    let session = env
        .app
        .log_session(
            alice,
            NewSession::new(movie.id, vec![Participant::new(alice_user.self_companion)]),
        )
        .await
        .unwrap();
    env.clock.advance(Duration::seconds(300));
    let log = |item, p, at| env.app.log_progress(alice, item, p, Some(at), None);
    // newest first: E1 (130 s after t0, takes the session 120 s below it), a book page (the
    // page boundary, 5 s), E2 (-50 s, 60 s from the session but it is taken)
    let e1 = log(movie.id, Progress::Finished, t0 + Duration::seconds(130))
        .await
        .unwrap();
    let paged = Progress::Paged {
        page: Some(10),
        chapter: None,
        volume: None,
    };
    log(book.id, paged, t0 + Duration::seconds(5))
        .await
        .unwrap();
    let e2 = log(movie.id, Progress::Finished, t0 - Duration::seconds(50))
        .await
        .unwrap();
    let mut rows = Vec::new();
    let mut before = None;
    loop {
        let page = env
            .app
            .activity_feed(
                alice,
                ActivityQuery {
                    before,
                    limit: 1,
                    ..ActivityQuery::default()
                },
            )
            .await
            .unwrap();
        rows.extend(page.rows);
        match page.next {
            Some(n) => before = Some(n),
            None => break,
        }
    }
    let of = |id| rows.iter().find(|r| r.event_id == Some(id)).unwrap();
    assert_eq!(
        of(e1.id).session_id,
        Some(session.id),
        "E1 takes the session"
    );
    assert_eq!(of(e2.id).session_id, None, "the session is taken");
    assert_eq!(rows.len(), 3, "the session shows once, with E1");
}
