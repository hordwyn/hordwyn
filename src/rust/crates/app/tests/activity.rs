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

/// UX3-16 (b): the person filter (History, the person page) also finds the plays of the
/// people merged into that person, through a merge chain too.
#[tokio::test]
async fn the_person_filter_follows_merges() {
    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let bob = env.app.create_companion(alice, "Bob").await.unwrap();
    let bobby = env.app.create_companion(alice, "Bobby").await.unwrap();
    let rob = env.app.create_companion(alice, "Rob").await.unwrap();
    let game = common::item(&env.app, "board_game", "Azul").await;
    for c in [bob.id, bobby.id, rob.id] {
        env.app
            .log_session(
                alice,
                NewSession::new(
                    game.id,
                    vec![
                        Participant::new(alice_user.self_companion),
                        Participant::new(c),
                    ],
                ),
            )
            .await
            .unwrap();
        env.clock.advance(chrono::Duration::hours(1));
    }
    // Rob into Bobby, then Bobby into Bob: a chain
    env.app
        .merge_companions(alice, bobby.id, rob.id)
        .await
        .unwrap();
    env.app
        .merge_companions(alice, bob.id, bobby.id)
        .await
        .unwrap();
    let with = |p| ActivityQuery {
        person: Some(p),
        ..ActivityQuery::default()
    };
    let n = |q| async { env.app.activity_feed(alice, q).await.unwrap().rows.len() };
    assert_eq!(n(with(bob.id)).await, 3, "Bob, Bobby and Rob");
    assert_eq!(n(with(bobby.id)).await, 2, "Bobby and Rob");
}

/// Counts full session reads (WP4-1: the feed reads sessions in windows only).
struct CountingSessions {
    inner: std::sync::Arc<testkit::MemStore>,
    full_reads: std::sync::atomic::AtomicUsize,
    /// Reads of one session by id (OT1A-3: the item page reads its plays in one batch).
    single_reads: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl app::ports::SessionRepo for CountingSessions {
    async fn insert(&self, s: &domain::session::Session) -> app::ports::RepoResult<()> {
        self.inner.insert(s).await
    }
    async fn get(
        &self,
        id: domain::ids::SessionId,
    ) -> app::ports::RepoResult<Option<domain::session::Session>> {
        self.single_reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        app::ports::SessionRepo::get(&*self.inner, id).await
    }
    async fn get_many(
        &self,
        ids: &[domain::ids::SessionId],
    ) -> app::ports::RepoResult<Vec<domain::session::Session>> {
        app::ports::SessionRepo::get_many(&*self.inner, ids).await
    }
    async fn update(&self, s: &domain::session::Session) -> app::ports::RepoResult<()> {
        app::ports::SessionRepo::update(&*self.inner, s).await
    }
    async fn delete(&self, id: domain::ids::SessionId) -> app::ports::RepoResult<()> {
        app::ports::SessionRepo::delete(&*self.inner, id).await
    }
    async fn by_owner(
        &self,
        owner: domain::ids::UserId,
    ) -> app::ports::RepoResult<Vec<domain::session::Session>> {
        self.full_reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        app::ports::SessionRepo::by_owner(&*self.inner, owner).await
    }
    async fn with_participants(
        &self,
        companions: &[domain::ids::CompanionId],
    ) -> app::ports::RepoResult<Vec<domain::session::Session>> {
        self.full_reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.inner.with_participants(companions).await
    }
    async fn by_owner_window(
        &self,
        owner: domain::ids::UserId,
        w: &app::ports::HistoryWindow,
    ) -> app::ports::RepoResult<Vec<domain::session::Session>> {
        self.inner.by_owner_window(owner, w).await
    }
    async fn with_participants_window(
        &self,
        companions: &[domain::ids::CompanionId],
        w: &app::ports::HistoryWindow,
    ) -> app::ports::RepoResult<Vec<domain::session::Session>> {
        self.inner.with_participants_window(companions, w).await
    }
    async fn group_plays(
        &self,
        owner: domain::ids::UserId,
    ) -> app::ports::RepoResult<Vec<app::ports::GroupPlays>> {
        self.inner.group_plays(owner).await
    }
}

/// Pages through the feed with `limit` and answers every row, checking `next` at the end.
async fn all_pages(
    env: &TestEnv,
    me: domain::user::Actor,
    q: &ActivityQuery,
) -> Vec<app::ActivityRow> {
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
        assert!(rows.len() < 1000, "paging does not end");
    }
    rows
}

#[tokio::test]
async fn windowed_pages_return_the_full_reads_rows() {
    // WP4-1 (DEC-235): pages of any size over interleaved events, own plays, "watched with"
    // sessions and shared sessions give exactly the rows of one big read, in order, and the
    // feed never reads all sessions
    let mut env = TestEnv::new();
    let counting = std::sync::Arc::new(CountingSessions {
        inner: env.store.clone(),
        full_reads: std::sync::atomic::AtomicUsize::new(0),
        single_reads: std::sync::atomic::AtomicUsize::new(0),
    });
    env.app.repos.sessions = counting.clone();
    env.clock.set(
        chrono::DateTime::parse_from_rfc3339("2024-03-01T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc),
    );
    let (alice_user, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let anna = env.app.create_companion(alice, "Anna").await.unwrap();
    // bob's companion "Alice" stands for alice: bob's plays with it are shared with her
    let for_alice = env.app.create_companion(bob, "Alice").await.unwrap();
    let req = env
        .app
        .request_link(bob, for_alice.id, alice_user.id)
        .await
        .unwrap();
    env.app.accept_link(alice, req.id).await.unwrap();
    let movie = common::item(&env.app, "movie", "Heat").await;
    let show = common::item(&env.app, "show", "Dark").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let book = common::item(&env.app, "book", "Dune").await;
    let me = |extra: bool| {
        let mut p = vec![Participant::new(alice_user.self_companion)];
        if extra {
            p.push(Participant::new(anna.id));
        }
        p
    };
    for i in 0u32..40 {
        match i % 5 {
            0 => {
                // "watched with": a session and a Finished event at the same instant
                env.app
                    .log_session(alice, NewSession::new(movie.id, me(true)))
                    .await
                    .unwrap();
                env.app
                    .log_progress(alice, movie.id, Progress::Finished, None, None)
                    .await
                    .unwrap();
            }
            1 => {
                env.app
                    .log_play(alice, NewSession::new(game.id, me(i % 2 == 1)))
                    .await
                    .unwrap();
            }
            2 => {
                let ev = env
                    .app
                    .log_progress(
                        alice,
                        show.id,
                        Progress::Episode {
                            season: 1,
                            episode: i,
                        },
                        None,
                        None,
                    )
                    .await
                    .unwrap();
                if i % 3 == 0 {
                    env.app.undo_progress(alice, ev.id).await.unwrap();
                }
            }
            3 => {
                env.app
                    .log_session(
                        bob,
                        NewSession::new(
                            game.id,
                            vec![
                                Participant::new(bob_user.self_companion),
                                Participant::new(for_alice.id),
                            ],
                        ),
                    )
                    .await
                    .unwrap();
            }
            _ => {
                env.app
                    .log_progress(
                        alice,
                        book.id,
                        Progress::Paged {
                            page: Some(i * 10),
                            chapter: None,
                            volume: None,
                        },
                        None,
                        None,
                    )
                    .await
                    .unwrap();
            }
        }
        env.clock.advance(chrono::Duration::days(19));
    }
    let queries = [
        ActivityQuery::default(),
        ActivityQuery {
            year: Some(2024),
            ..ActivityQuery::default()
        },
        ActivityQuery {
            year: Some(2025),
            ..ActivityQuery::default()
        },
        ActivityQuery {
            person: Some(anna.id),
            ..ActivityQuery::default()
        },
        ActivityQuery {
            shared_only: true,
            ..ActivityQuery::default()
        },
    ];
    for q in queries {
        let full = env
            .app
            .activity_feed(
                alice,
                ActivityQuery {
                    limit: 100,
                    ..q.clone()
                },
            )
            .await
            .unwrap();
        assert_eq!(full.next, None, "{q:?}");
        assert!(full.rows.len() > 3, "{q:?}: {}", full.rows.len());
        if let Some(y) = q.year {
            use chrono::Datelike;
            assert!(full.rows.iter().all(|r| r.at.year() == y));
        }
        for limit in 1..=7 {
            let paged = all_pages(&env, alice, &ActivityQuery { limit, ..q.clone() }).await;
            assert_eq!(paged, full.rows, "{q:?} in pages of {limit}");
        }
    }
    assert_eq!(
        counting
            .full_reads
            .load(std::sync::atomic::Ordering::Relaxed),
        0,
        "the feed reads sessions in windows"
    );
}

#[tokio::test]
async fn game_groups_count_their_plays_without_reading_every_session() {
    // UX4-17: the People page reads the owner's sessions once (for the people), not again
    // for the groups
    let mut env = TestEnv::new();
    let counting = std::sync::Arc::new(CountingSessions {
        inner: env.store.clone(),
        full_reads: std::sync::atomic::AtomicUsize::new(0),
        single_reads: std::sync::atomic::AtomicUsize::new(0),
    });
    env.app.repos.sessions = counting.clone();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let anna = env.app.create_companion(alice, "Anna").await.unwrap();
    let game = common::item(&env.app, "board_game", "Azul").await;
    let late = common::item(&env.app, "board_game", "Carcassonne").await;
    let group = env
        .app
        .create_game_group(
            alice,
            app::GameGroupInput {
                name: "Thursday".into(),
                members: vec![anna.id],
            },
        )
        .await
        .unwrap();
    for item in [game.id, game.id, late.id] {
        let mut play = NewSession::new(
            item,
            vec![
                Participant::new(alice_user.self_companion),
                Participant::new(anna.id),
            ],
        );
        play.game_group = Some(group.id);
        env.app.log_play(alice, play).await.unwrap();
        env.clock.advance(chrono::Duration::hours(1));
    }
    let before = counting
        .full_reads
        .load(std::sync::atomic::Ordering::Relaxed);
    let groups = env.app.game_groups(alice).await.unwrap();
    assert_eq!(
        counting
            .full_reads
            .load(std::sync::atomic::Ordering::Relaxed),
        before,
        "no full session read"
    );
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].plays, 3);
    assert_eq!(groups[0].last.as_ref().unwrap().item_id, late.id);
}

/// OT-11: a play is stored with scores, the winner, the duration and the co-op outcome; the
/// feed row and the item's plays carry them (no row said more than "Session logged").
#[tokio::test]
async fn a_play_row_carries_scores_the_winner_the_duration_and_the_outcome() {
    use domain::session::CoopOutcome;

    let env = TestEnv::new();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let ben = env.app.create_companion(alice, "Ben").await.unwrap();
    let game = common::item(&env.app, "board_game", "Azul").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    env.app
        .log_progress(alice, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    env.clock.advance(chrono::Duration::hours(1));
    let mut me = Participant::new(alice_user.self_companion);
    me.score = Some(31.0);
    let mut other = Participant::new(ben.id);
    other.score = Some(42.5);
    other.winner = true;
    let mut play = NewSession::new(game.id, vec![me, other]);
    play.duration_minutes = Some(45);
    let scored = env.app.log_session(alice, play).await.unwrap();
    env.clock.advance(chrono::Duration::hours(1));
    let mut coop = NewSession::new(
        game.id,
        vec![
            Participant::new(alice_user.self_companion),
            Participant::new(ben.id),
        ],
    );
    coop.coop_outcome = Some(CoopOutcome::Won);
    let together = env.app.log_session(alice, coop).await.unwrap();

    let page = env
        .app
        .activity_feed(alice, ActivityQuery::default())
        .await
        .unwrap();
    assert_eq!(page.rows.len(), 3);
    let (won, play, watched) = (&page.rows[0], &page.rows[1], &page.rows[2]);
    assert_eq!(
        won.play,
        Some(app::ActivityPlay {
            duration_minutes: None,
            coop: Some(CoopOutcome::Won),
        })
    );
    assert_eq!(
        play.play,
        Some(app::ActivityPlay {
            duration_minutes: Some(45),
            coop: None,
        })
    );
    let people: Vec<_> = play
        .participants
        .iter()
        .map(|p| (p.name.as_str(), p.is_me, p.score, p.winner))
        .collect();
    assert_eq!(
        people,
        vec![
            ("ALICE", true, Some(31.0), false),
            ("Ben", false, Some(42.5), true)
        ]
    );
    assert_eq!(watched.play, None, "an event without a session is no play");

    // the item page: the plays behind the item's session events, by session
    let history = env.app.history(alice, game.id).await.unwrap();
    let plays = env.app.item_plays(alice, &history).await.unwrap();
    assert_eq!(
        plays.iter().map(|p| p.session_id).collect::<Vec<_>>(),
        vec![together.id, scored.id],
        "newest first"
    );
    let of = |id| plays.iter().find(|p| p.session_id == id).unwrap();
    assert_eq!(of(scored.id).play.duration_minutes, Some(45));
    assert_eq!(of(scored.id).participants[1].score, Some(42.5));
    assert!(of(scored.id).participants[1].winner);
    assert!(of(scored.id).participants[0].is_me);
    assert_eq!(of(together.id).play.coop, Some(CoopOutcome::Won));
    // nothing to read for an item without plays
    let history = env.app.history(alice, movie.id).await.unwrap();
    assert_eq!(env.app.item_plays(alice, &history).await.unwrap().len(), 0);
    // another user's events never show a play of alice
    let (_, bob) = common::user(&env, "bob").await;
    let history = env.app.history(alice, game.id).await.unwrap();
    assert_eq!(env.app.item_plays(bob, &history).await.unwrap().len(), 0);
}

/// OT1A-3, OT1A-10: the item page reads the plays of an item in one batch, newest first and
/// at most `MAX_ITEM_PLAYS` of them.
#[tokio::test]
async fn the_plays_of_an_item_are_read_in_one_batch_newest_first_and_capped() {
    let mut env = TestEnv::new();
    let counting = std::sync::Arc::new(CountingSessions {
        inner: env.store.clone(),
        full_reads: std::sync::atomic::AtomicUsize::new(0),
        single_reads: std::sync::atomic::AtomicUsize::new(0),
    });
    env.app.repos.sessions = counting.clone();
    let (alice_user, alice) = common::user(&env, "alice").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let mut logged = Vec::new();
    for _ in 0..=app::MAX_ITEM_PLAYS {
        env.clock.advance(chrono::Duration::minutes(1));
        let play = NewSession::new(game.id, vec![Participant::new(alice_user.self_companion)]);
        logged.push(env.app.log_session(alice, play).await.unwrap().id);
    }
    let history = env.app.history(alice, game.id).await.unwrap();
    let reads = || {
        (
            counting
                .single_reads
                .load(std::sync::atomic::Ordering::Relaxed),
            counting
                .full_reads
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    };
    let before = reads();
    let plays = env.app.item_plays(alice, &history).await.unwrap();
    assert_eq!(reads(), before, "no read per play and no full read");
    // the newest 50 of the 51, newest first: the oldest play is the one left out
    let expected: Vec<_> = logged
        .iter()
        .rev()
        .take(app::MAX_ITEM_PLAYS)
        .copied()
        .collect();
    assert_eq!(expected.len(), 50);
    assert_eq!(
        plays.iter().map(|p| p.session_id).collect::<Vec<_>>(),
        expected
    );
}

#[tokio::test]
async fn the_year_filter_follows_the_persons_time_zone() {
    use chrono::TimeZone;
    let at = |y, mo, d, h, mi| chrono::Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let zone = app::time_zone::parse_zone("Europe/Berlin").unwrap();
    env.app.set_time_zone(me, Some(zone)).await.unwrap();
    env.clock.set(at(2026, 3, 1, 12, 0));
    let cases = [
        ("eve", at(2025, 12, 31, 22, 30)), // 2025-12-31 23:30 in Berlin
        ("new year", at(2025, 12, 31, 23, 30)), // 2026-01-01 00:30 in Berlin
        ("first", at(2024, 12, 31, 23, 0)), // 2025-01-01 00:00 in Berlin, the first instant
        ("before", at(2024, 12, 31, 22, 59)), // 2024-12-31 23:59 in Berlin
    ];
    for (title, when) in cases {
        let item = common::item(&env.app, "movie", title).await;
        env.app
            .log_progress(me, item.id, Progress::Finished, Some(when), None)
            .await
            .unwrap();
    }
    let titles = |year: i32| {
        let env = &env;
        async move {
            let q = ActivityQuery {
                year: Some(year),
                limit: 50,
                ..ActivityQuery::default()
            };
            let page = env.app.activity_feed(me, q).await.unwrap();
            page.rows.into_iter().map(|r| r.title).collect::<Vec<_>>()
        }
    };
    assert_eq!(titles(2024).await, ["before"]);
    assert_eq!(titles(2025).await, ["eve", "first"]);
    assert_eq!(titles(2026).await, ["new year"]);
    // without a zone the year is the UTC year, as before
    env.app.set_time_zone(me, None).await.unwrap();
    assert_eq!(titles(2025).await, ["new year", "eve"]);
    assert_eq!(titles(2024).await, ["first", "before"]);
    // a year no calendar holds lists nothing; it does not drop the filter (review POL-9)
    assert_eq!(titles(i32::MAX).await.len(), 0);
    assert_eq!(titles(300_000).await.len(), 0);
    assert_eq!(titles(i32::MIN).await.len(), 0);
}

/// Review POL-10: the boundary lies on the other side of the UTC midnight west of Greenwich,
/// and the southern zones are on daylight time on 1 January.
#[tokio::test]
async fn the_year_filter_west_of_utc_and_in_southern_daylight_time() {
    use chrono::TimeZone;
    let at = |y, mo, d, h, mi| chrono::Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    env.clock.set(at(2026, 3, 1, 12, 0));
    let cases = [
        // America/Los_Angeles, UTC-8 in winter: the year starts at 08:00 UTC
        ("la last", at(2026, 1, 1, 7, 59)), // 2025-12-31 23:59
        ("la first", at(2026, 1, 1, 8, 0)), // 2026-01-01 00:00
        // Australia/Sydney, UTC+11 in its summer: the year starts at 13:00 UTC the day before
        ("sydney last", at(2025, 12, 31, 12, 59)), // 2025-12-31 23:59
        ("sydney first", at(2025, 12, 31, 13, 0)), // 2026-01-01 00:00
        // America/Santiago, UTC-3 in its summer: the year starts at 03:00 UTC
        ("santiago last", at(2026, 1, 1, 2, 59)), // 2025-12-31 23:59
        ("santiago first", at(2026, 1, 1, 3, 0)), // 2026-01-01 00:00
    ];
    for (title, when) in cases {
        let item = common::item(&env.app, "movie", title).await;
        env.app
            .log_progress(me, item.id, Progress::Finished, Some(when), None)
            .await
            .unwrap();
    }
    for (zone, city) in [
        ("America/Los_Angeles", "la"),
        ("Australia/Sydney", "sydney"),
        ("America/Santiago", "santiago"),
    ] {
        let tz = app::time_zone::parse_zone(zone).unwrap();
        env.app.set_time_zone(me, Some(tz)).await.unwrap();
        for (year, edge) in [(2025, "last"), (2026, "first")] {
            let q = ActivityQuery {
                year: Some(year),
                limit: 50,
                ..ActivityQuery::default()
            };
            let page = env.app.activity_feed(me, q).await.unwrap();
            let titles: Vec<String> = page.rows.into_iter().map(|r| r.title).collect();
            let own = format!("{city} {edge}");
            assert!(titles.contains(&own), "{zone} {year}: {titles:?}");
            let other = format!("{city} {}", if edge == "last" { "first" } else { "last" });
            assert!(!titles.contains(&other), "{zone} {year}: {titles:?}");
        }
    }
}
