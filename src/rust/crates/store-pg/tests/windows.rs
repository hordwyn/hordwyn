//! UX4-16: the keyset windows of the activity feed on Postgres. Walking the owner's sessions,
//! the sessions with given people and the events in pages of 1 to 3, over rows that share
//! their time, returns exactly the full reads (`by_owner`, `with_participants`, `by_user`).

mod common;

use app::NewSession;
use app::ports::{HistoryWindow, ProgressRepo, SessionRepo};
use domain::progress::Progress;
use domain::session::Participant;

fn key_s(s: &domain::session::Session) -> (domain::Timestamp, uuid::Uuid) {
    (s.started_at, s.id.0)
}

async fn walk<F, Fut, T>(
    limit: u32,
    read: F,
    key: fn(&T) -> (domain::Timestamp, uuid::Uuid),
) -> Vec<T>
where
    F: Fn(HistoryWindow) -> Fut,
    Fut: std::future::Future<Output = Vec<T>>,
{
    let mut out = Vec::new();
    let mut before = None;
    loop {
        let page = read(HistoryWindow {
            before,
            since: None,
            kinds: vec![],
            limit,
        })
        .await;
        let Some(last) = page.last() else {
            return out;
        };
        before = Some(key(last));
        out.extend(page);
        assert!(out.len() < 100, "paging does not end");
    }
}

#[tokio::test]
async fn windows_with_equal_times_walk_every_row_once_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let (alice_user, alice) = common::user(app, "alice").await;
    let anna = app.create_companion(alice, "Anna").await.unwrap();
    let game = common::item(app, "board_game", "Azul").await;
    let show = common::item(app, "show", "Dark").await;
    // the clock stands still: every row shares its time with others
    for n in 0..7u32 {
        let mut people = vec![Participant::new(alice_user.self_companion)];
        if n % 2 == 0 {
            people.push(Participant::new(anna.id));
        }
        app.log_play(alice, NewSession::new(game.id, people))
            .await
            .unwrap();
        app.log_progress(
            alice,
            show.id,
            Progress::Episode {
                season: 1,
                episode: n + 1,
            },
            None,
            None,
        )
        .await
        .unwrap();
    }
    let mut owner = SessionRepo::by_owner(&env.store, alice.user_id)
        .await
        .unwrap();
    owner.sort_by_key(|s| std::cmp::Reverse(key_s(s)));
    let mut with = SessionRepo::with_participants(&env.store, &[anna.id])
        .await
        .unwrap();
    with.sort_by_key(|s| std::cmp::Reverse(key_s(s)));
    let mut events: Vec<_> = ProgressRepo::by_user(&env.store, alice.user_id)
        .await
        .unwrap();
    events.sort_by_key(|e| std::cmp::Reverse((e.occurred_at, e.id.0)));
    assert_eq!((owner.len(), with.len(), events.len()), (7, 4, 14));
    for limit in 1..=3 {
        let store = &env.store;
        let paged = walk(
            limit,
            |w| async move { store.by_owner_window(alice.user_id, &w).await.unwrap() },
            key_s,
        )
        .await;
        assert_eq!(paged, owner, "owner, pages of {limit}");
        let paged = walk(
            limit,
            |w| async move {
                store
                    .with_participants_window(&[anna.id], &w)
                    .await
                    .unwrap()
            },
            key_s,
        )
        .await;
        assert_eq!(paged, with, "with anna, pages of {limit}");
        let paged = walk(
            limit,
            |w| async move { store.recent(alice.user_id, &w).await.unwrap() },
            |e: &domain::progress::ProgressEvent| (e.occurred_at, e.id.0),
        )
        .await;
        assert_eq!(paged, events, "events, pages of {limit}");
    }
}

/// DEC-494 data: 30 rows 90 s apart, Anna on every other play, a "watched with" pair (the
/// event 100 s after its session) every fifth, some undone plays, an episode 30 s after each.
struct Seeded {
    alice: domain::user::Actor,
    anna: domain::ids::CompanionId,
    game: domain::ids::ItemId,
    show: domain::ids::ItemId,
    t0: domain::Timestamp,
}

async fn seed_person(app: &app::App) -> Seeded {
    use chrono::Duration;
    let (alice_user, alice) = common::user(app, "alice").await;
    let anna = app.create_companion(alice, "Anna").await.unwrap();
    let game = common::item(app, "board_game", "Azul").await;
    let show = common::item(app, "show", "Dark").await;
    let movie = common::item(app, "movie", "Heat").await;
    let t0 = app.clock.now();
    for n in 0..30u32 {
        let at = t0 + Duration::seconds(90 * i64::from(n));
        let mut people = vec![Participant::new(alice_user.self_companion)];
        if n % 2 == 0 {
            people.push(Participant::new(anna.id));
        }
        let item = if n % 5 == 0 { movie.id } else { game.id };
        let mut s = NewSession::new(item, people);
        s.started_at = Some(at);
        if n % 5 == 0 {
            app.log_session(alice, s).await.unwrap();
            let later = Some(at + Duration::seconds(100));
            app.log_progress(alice, movie.id, Progress::Finished, later, None)
                .await
                .unwrap();
        } else {
            let (_, ev) = app.log_play(alice, s).await.unwrap();
            if n % 7 == 3 {
                app.undo_progress(alice, ev.id).await.unwrap();
            }
        }
        let ep = Progress::Episode {
            season: 1,
            episode: n + 1,
        };
        app.log_progress(alice, show.id, ep, Some(at + Duration::seconds(30)), None)
            .await
            .unwrap();
    }
    Seeded {
        alice,
        anna: anna.id,
        game: game.id,
        show: show.id,
        t0,
    }
}

/// DEC-494: the span reads of the person filter on Postgres return exactly the full reads'
/// rows in the spans (by time, `[from, to)`), of the kinds, newest first, cut at the limit.
#[tokio::test]
async fn span_reads_return_the_full_reads_rows_in_the_spans_on_postgres() {
    use app::ports::HistorySpans;
    use domain::kind::KindId;

    let Some(env) = common::env().await else {
        return;
    };
    let store = &env.store;
    let x = seed_person(&env.app).await;
    let kind_of = |i| {
        if i == x.game {
            "board_game"
        } else if i == x.show {
            "show"
        } else {
            "movie"
        }
    };
    let secs = |s: i64| x.t0 + chrono::Duration::seconds(s);
    let spans = vec![
        (secs(2000), secs(2600)),
        (secs(1500), secs(1600)),
        (secs(200), secs(700)),
    ];
    let inside = |t: domain::Timestamp| spans.iter().any(|&(a, b)| t >= a && t < b);
    let all = HistoryWindow {
        limit: 10_000,
        ..HistoryWindow::default()
    };
    let user = x.alice.user_id;
    let owner = store.by_owner_window(user, &all).await.unwrap();
    let with = store
        .with_participants_window(&[x.anna], &all)
        .await
        .unwrap();
    let events = store.recent(user, &all).await.unwrap();
    for kinds in [vec![], vec!["board_game".to_string()]] {
        for limit in [1000, 3] {
            let w = HistorySpans {
                spans: spans.clone(),
                kinds: kinds.iter().map(|k| KindId::new(k).unwrap()).collect(),
                limit,
            };
            let of_kind = |i| kinds.is_empty() || kinds.iter().any(|k| k == kind_of(i));
            let cut = |n: usize| n.min(limit as usize);
            let want: Vec<_> = owner
                .iter()
                .filter(|s| inside(s.started_at) && of_kind(s.item_id))
                .cloned()
                .collect();
            let got = store.by_owner_spans(user, &w).await.unwrap();
            assert_eq!(got, want[..cut(want.len())], "owner {kinds:?} {limit}");
            let want: Vec<_> = with
                .iter()
                .filter(|s| inside(s.started_at) && of_kind(s.item_id))
                .cloned()
                .collect();
            let got = store.with_participants_spans(&[x.anna], &w).await.unwrap();
            assert_eq!(got, want[..cut(want.len())], "with anna {kinds:?} {limit}");
            let want: Vec<_> = events
                .iter()
                .filter(|e| inside(e.occurred_at) && of_kind(e.item_id))
                .cloned()
                .collect();
            let got = store.recent_spans(user, &w).await.unwrap();
            assert_eq!(got, want[..cut(want.len())], "events {kinds:?} {limit}");
            assert!(want.len() > 3, "{kinds:?}: {}", want.len());
        }
    }
}

/// DEC-494: on Postgres the person-filtered feed in pages equals the unfiltered feed's rows
/// with the person.
#[tokio::test]
async fn the_person_feed_matches_the_full_read_on_postgres() {
    use app::ActivityQuery;

    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let x = seed_person(app).await;
    let full = app
        .activity_feed(
            x.alice,
            ActivityQuery {
                limit: 100,
                ..ActivityQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(full.next, None);
    let expected: Vec<_> = full
        .rows
        .into_iter()
        .filter(|r| r.participants.iter().any(|p| p.companion_id == x.anna))
        .collect();
    assert!(expected.len() > 10, "{}", expected.len());
    for limit in [1, 2, 3, 20] {
        let mut rows = Vec::new();
        let mut before = None;
        loop {
            let q = ActivityQuery {
                before,
                limit,
                person: Some(x.anna),
                ..ActivityQuery::default()
            };
            let page = app.activity_feed(x.alice, q).await.unwrap();
            rows.extend(page.rows);
            match page.next {
                Some(n) => before = Some(n),
                None => break,
            }
            assert!(rows.len() < 200, "paging does not end");
        }
        assert_eq!(rows, expected, "person pages of {limit}");
    }
}
