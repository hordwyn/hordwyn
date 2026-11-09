//! DEC-494: the person filter of the activity feed reads from the person's sessions. In pages
//! of any size it returns exactly the rows of the unfiltered feed whose people include the
//! person or anyone merged into them: plays, "watched with" events (also seconds away from
//! their session), standalone sessions, shared sessions (of an owner who shares or not), with
//! the year, type and kind filters, and around a burst of events that makes a round dense.

// one scenario, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use std::collections::HashSet;

use app::ports::Clock;
use app::{ActivityQuery, ActivityRow, ActivityType, NewSession};
use chrono::Duration;
use domain::ids::CompanionId;
use domain::kind::KindId;
use domain::progress::Progress;
use domain::session::Participant;
use domain::user::Actor;
use testkit::TestEnv;

async fn all_pages(env: &TestEnv, me: Actor, q: &ActivityQuery) -> Vec<ActivityRow> {
    let mut rows = Vec::new();
    let mut before = None;
    loop {
        let page = env
            .app
            .activity_feed(
                me,
                ActivityQuery {
                    before,
                    ..q.clone()
                },
            )
            .await
            .unwrap();
        assert!(page.rows.len() <= q.limit as usize);
        rows.extend(page.rows);
        match page.next {
            Some(n) => before = Some(n),
            None => break,
        }
        assert!(rows.len() < 2000, "paging does not end");
    }
    rows
}

#[tokio::test]
async fn person_pages_equal_the_full_read_filtered_by_person() {
    let env = TestEnv::new();
    env.clock.set(
        chrono::DateTime::parse_from_rfc3339("2024-06-01T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    let (alice_user, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let (carol_user, carol) = common::user(&env, "carol").await;
    let anna = env.app.create_companion(alice, "Anna").await.unwrap();
    let annie = env.app.create_companion(alice, "Annie").await.unwrap();
    let ben = env.app.create_companion(alice, "Ben").await.unwrap();
    // bob's "Alice" and carol's "Alice" stand for alice; carol does not share sessions
    let mut stand_ins = Vec::new();
    for (owner, owner_actor) in [(&bob_user, bob), (&carol_user, carol)] {
        let c = env
            .app
            .create_companion(owner_actor, "Alice")
            .await
            .unwrap();
        let req = env
            .app
            .request_link(owner_actor, c.id, alice_user.id)
            .await
            .unwrap();
        env.app.accept_link(alice, req.id).await.unwrap();
        stand_ins.push((owner.self_companion, owner_actor, c.id));
    }
    let movie = common::item(&env.app, "movie", "Heat").await;
    let show = common::item(&env.app, "show", "Dark").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let book = common::item(&env.app, "book", "Dune").await;
    let me_with = |c: Option<CompanionId>| {
        let mut p = vec![Participant::new(alice_user.self_companion)];
        p.extend(c.map(Participant::new));
        p
    };
    let people = [Some(anna.id), Some(annie.id), Some(ben.id), None];
    for i in 0u32..48 {
        let who = people[(i as usize / 2) % people.len()];
        let now = env.clock.now();
        match i % 6 {
            0 => {
                // "watched with": the event 100 s after its session
                let mut s = NewSession::new(movie.id, me_with(who));
                s.started_at = Some(now - Duration::seconds(100));
                env.app.log_session(alice, s).await.unwrap();
                env.app
                    .log_progress(alice, movie.id, Progress::Finished, None, None)
                    .await
                    .unwrap();
            }
            1 => {
                let (_, ev) = env
                    .app
                    .log_play(alice, NewSession::new(game.id, me_with(who)))
                    .await
                    .unwrap();
                if i % 4 == 3 {
                    // an undone play leaves a session without a row
                    env.app.undo_progress(alice, ev.id).await.unwrap();
                }
            }
            2 => {
                // a session no event takes (a standalone row)
                env.app
                    .log_session(alice, NewSession::new(show.id, me_with(who)))
                    .await
                    .unwrap();
            }
            3 => {
                let (self_c, owner, stand_in) = stand_ins[(i as usize / 6) % 2];
                env.app
                    .log_session(
                        owner,
                        NewSession::new(
                            game.id,
                            vec![Participant::new(self_c), Participant::new(stand_in)],
                        ),
                    )
                    .await
                    .unwrap();
            }
            4 if i == 22 => {
                // a burst: 70 episodes within a minute around a play with Anna
                for n in 0..70u32 {
                    let at = now + Duration::seconds(i64::from(n) - 35);
                    let ep = Progress::Episode {
                        season: 2,
                        episode: n + 1,
                    };
                    env.app
                        .log_progress(alice, show.id, ep, Some(at), None)
                        .await
                        .unwrap();
                }
                env.app
                    .log_play(alice, NewSession::new(game.id, me_with(Some(anna.id))))
                    .await
                    .unwrap();
            }
            _ => {
                let paged = Progress::Paged {
                    page: Some(i * 10),
                    chapter: None,
                    volume: None,
                };
                env.app
                    .log_progress(alice, book.id, paged, None, None)
                    .await
                    .unwrap();
            }
        }
        let gap = if i % 3 == 0 {
            Duration::hours(7)
        } else {
            Duration::days(19)
        };
        env.clock.advance(gap);
    }
    env.app.set_privacy(carol, false).await.unwrap();
    // Annie into Anna after the plays: plays with Annie count for Anna
    env.app
        .merge_companions(alice, anna.id, annie.id)
        .await
        .unwrap();
    let anna_set = HashSet::from([anna.id, annie.id]);
    let bob_alice = stand_ins[0].2;
    let carol_alice = stand_ins[1].2;
    let movies = Some(KindId::new("movie").unwrap());
    let cases: Vec<(ActivityQuery, HashSet<CompanionId>, bool)> = vec![
        (ActivityQuery::default(), anna_set.clone(), true),
        (ActivityQuery::default(), HashSet::from([annie.id]), true),
        (ActivityQuery::default(), HashSet::from([ben.id]), true),
        (ActivityQuery::default(), HashSet::from([bob_alice]), true),
        (
            ActivityQuery::default(),
            HashSet::from([carol_alice]),
            false,
        ),
        (
            ActivityQuery {
                year: Some(2025),
                ..ActivityQuery::default()
            },
            anna_set.clone(),
            true,
        ),
        (
            ActivityQuery {
                types: vec![ActivityType::Session],
                ..ActivityQuery::default()
            },
            anna_set.clone(),
            true,
        ),
        (
            ActivityQuery {
                kind: movies,
                ..ActivityQuery::default()
            },
            anna_set.clone(),
            true,
        ),
        (
            ActivityQuery {
                shared_only: true,
                ..ActivityQuery::default()
            },
            HashSet::from([bob_alice]),
            true,
        ),
    ];
    for (q, set, some) in cases {
        let person = if set.contains(&anna.id) {
            anna.id
        } else {
            *set.iter().next().unwrap()
        };
        let full = all_pages(
            &env,
            alice,
            &ActivityQuery {
                limit: 100,
                ..q.clone()
            },
        )
        .await;
        let expected: Vec<ActivityRow> = full
            .into_iter()
            .filter(|r| r.participants.iter().any(|p| set.contains(&p.companion_id)))
            .collect();
        assert_eq!(
            !expected.is_empty(),
            some,
            "{q:?} {person:?}: {}",
            expected.len()
        );
        for limit in [1, 2, 3, 5, 7, 20, 100] {
            let paged = all_pages(
                &env,
                alice,
                &ActivityQuery {
                    limit,
                    person: Some(person),
                    ..q.clone()
                },
            )
            .await;
            assert_eq!(paged, expected, "{q:?} with {person:?} in pages of {limit}");
        }
    }
}
