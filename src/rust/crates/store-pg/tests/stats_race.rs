//! UX4-1: a dirty mark of the statistics cache must survive a refresh that runs while the
//! write that set it is still uncommitted (the writer's trigger ran before the reader took
//! the dirty set, the writer committed after the recompute).

mod common;

use std::time::Duration;

use app::ports::ProgressRepo;
use domain::progress::Progress;

async fn cached(env: &common::PgEnv, user: domain::ids::UserId) -> Vec<app::MonthAmounts> {
    tokio::time::timeout(
        Duration::from_secs(20),
        ProgressRepo::month_amounts(&env.store, user, "UTC"),
    )
    .await
    .expect("a stats read must not wait for an open writer")
    .unwrap()
}

async fn full(env: &common::PgEnv, user: domain::ids::UserId) -> Vec<app::MonthAmounts> {
    env.store.month_amounts_full(user, "UTC").await.unwrap()
}

const INSERT_EVENT: &str = "INSERT INTO progress_events
        (id, user_id, item_id, occurred_at, progress, recorded_at)
     VALUES (gen_random_uuid(), $1, $2, now(), jsonb_build_object('type', 'finished'), now())";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mark_set_by_an_uncommitted_writer_survives_a_concurrent_refresh() {
    let Some(env) = common::env().await else {
        return;
    };
    let (_, me) = common::user(&env.app, "alice").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    env.app
        .log_progress(me, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    // the cache is built; a second log leaves a committed mark behind
    assert_eq!(cached(&env, me.user_id).await, full(&env, me.user_id).await);
    env.app
        .log_progress(me, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();

    // the writer's trigger finds the committed mark; the writer stays open ...
    let mut writer = env.store.pool().begin().await.unwrap();
    sqlx::query(INSERT_EVENT)
        .bind(me.user_id.0)
        .bind(movie.id.0)
        .execute(&mut *writer)
        .await
        .unwrap();
    // ... while a reader refreshes on another connection (it cannot see the event yet)
    let during = cached(&env, me.user_id).await;
    writer.commit().await.unwrap();

    let after = full(&env, me.user_id).await;
    assert_ne!(
        during, after,
        "the open write was not visible to the refresh"
    );
    assert_eq!(
        cached(&env, me.user_id).await,
        after,
        "the write committed after the refresh must still be counted"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn interleaved_writers_and_readers_end_with_the_full_statement() {
    let Some(env) = common::env().await else {
        return;
    };
    let (_, me) = common::user(&env.app, "alice").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    let show = common::item(&env.app, "show", "Dark").await;
    let user = me.user_id;
    let mut set = tokio::task::JoinSet::new();
    for (n, item) in [movie.id, show.id, movie.id].into_iter().enumerate() {
        let pool = env.store.pool().clone();
        set.spawn(async move {
            for round in 0..15u64 {
                let mut tx = pool.begin().await.unwrap();
                sqlx::query(INSERT_EVENT)
                    .bind(user.0)
                    .bind(item.0)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                // hold the write open across a reader's refresh now and then
                tokio::time::sleep(Duration::from_millis((round + n as u64) % 3 * 7)).await;
                tx.commit().await.unwrap();
            }
        });
    }
    for _ in 0..2 {
        let store = env.store.clone();
        set.spawn(async move {
            for _ in 0..25 {
                ProgressRepo::month_amounts(&store, user, "UTC")
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(3)).await;
            }
        });
    }
    while let Some(done) = set.join_next().await {
        done.unwrap();
    }
    let expected = full(&env, user).await;
    assert_eq!(expected.iter().map(|m| m.amounts.events).sum::<u64>(), 45);
    assert_eq!(cached(&env, user).await, expected);
}

/// UX4-20: the trigger paths the WP4-1 script did not walk: an UPDATE of an event, a session
/// moved to another item, an item whose kind changes. After each the cached read equals the
/// full statement.
#[tokio::test]
async fn updates_of_events_sessions_and_kinds_mark_the_cache() {
    let Some(env) = common::env().await else {
        return;
    };
    let (alice, me) = common::user(&env.app, "alice").await;
    let movie = common::item(&env.app, "movie", "Heat").await;
    let other = common::item(&env.app, "movie", "Ronin").await;
    let game = common::item(&env.app, "board_game", "Azul").await;
    let late = common::item(&env.app, "board_game", "Carcassonne").await;
    let ev = env
        .app
        .log_progress(me, movie.id, Progress::Finished, None, None)
        .await
        .unwrap();
    let mut play = app::NewSession::new(
        game.id,
        vec![domain::session::Participant::new(alice.self_companion)],
    );
    play.duration_minutes = Some(30);
    let (session, _) = env.app.log_play(me, play).await.unwrap();
    let same = |step: &'static str| {
        let env = &env;
        async move {
            assert_eq!(
                cached(env, me.user_id).await,
                full(env, me.user_id).await,
                "{step}"
            );
        }
    };
    same("start").await;
    sqlx::query("UPDATE progress_events SET item_id = $2, occurred_at = occurred_at - interval '40 days' WHERE id = $1")
        .bind(ev.id.0)
        .bind(other.id.0)
        .execute(env.store.pool())
        .await
        .unwrap();
    same("event updated").await;
    sqlx::query("UPDATE sessions SET item_id = $2 WHERE id = $1")
        .bind(session.id.0)
        .bind(late.id.0)
        .execute(env.store.pool())
        .await
        .unwrap();
    same("session moved").await;
    sqlx::query("UPDATE catalog_items SET kind = 'show' WHERE id = $1")
        .bind(other.id.0)
        .execute(env.store.pool())
        .await
        .unwrap();
    same("kind changed").await;
}
