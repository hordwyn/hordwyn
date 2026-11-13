//! The completion no event made (LB-13, LB-14, LB-15): where it lies, that it stays there
//! whatever writes the status again, and that it is "undated" without a logged step.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::ports::ProgressRepo;
use app::{Bucket, BulkOp, NewGoal};
use chrono::{TimeZone, Utc};
use domain::goal::GoalMetric;
use domain::ids::ItemId;
use domain::kind::{KindGroup, Status};
use domain::progress::Progress;
use domain::user::Actor;
use testkit::TestEnv;

fn utc(y: i32, m: u32, d: u32) -> domain::Timestamp {
    Utc.with_ymd_and_hms(y, m, d, 20, 0, 0).unwrap()
}

async fn page(env: &TestEnv, me: Actor, item: ItemId, page: u32, at: domain::Timestamp) {
    let p = Progress::Paged {
        page: Some(page),
        chapter: None,
        volume: None,
    };
    env.app
        .log_progress(me, item, p, Some(at), None)
        .await
        .unwrap();
}

async fn complete(env: &TestEnv, me: Actor, item: ItemId) {
    env.app
        .set_status(me, item, Status::completed(), None)
        .await
        .unwrap();
}

/// The months that hold a completion, with their number.
async fn months(env: &TestEnv, me: Actor) -> Vec<(String, u64)> {
    env.app
        .stats_series(me, None, Bucket::Month)
        .await
        .unwrap()
        .into_iter()
        .filter(|p| p.amounts.completions > 0)
        .map(|p| (p.bucket, p.amounts.completions))
        .collect()
}

async fn total(env: &TestEnv, me: Actor) -> u64 {
    let t = env.app.stats_totals(me).await.unwrap();
    t.overall.amounts.completions
}

/// A book read in spring 2025 and set to Completed in June 2026.
async fn read_book(env: &TestEnv, me: Actor) -> ItemId {
    env.clock.set(utc(2026, 6, 1));
    let book = common::item(&env.app, "book", "Dune").await;
    page(env, me, book.id, 100, utc(2025, 1, 10)).await;
    page(env, me, book.id, 300, utc(2025, 3, 2)).await;
    complete(env, me, book.id).await;
    book.id
}

/// BOOK-1 (3): choosing Completed again must not move the completion.
#[tokio::test]
async fn completed_written_again_keeps_its_day() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let book = read_book(&env, me).await;
    let first = months(&env, me).await;
    assert_eq!(first.len(), 1);
    env.clock.advance(chrono::Duration::days(40));
    complete(&env, me, book).await;
    assert_eq!(months(&env, me).await, first);
    assert_eq!(total(&env, me).await, 1);
}

/// BOOK-1 (2): a bulk "Completed" over entries that already are Completed.
#[tokio::test]
async fn a_bulk_completed_over_completed_entries_keeps_their_day() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let book = read_book(&env, me).await;
    let other = common::item(&env.app, "book", "Emma").await;
    page(&env, me, other.id, 50, utc(2026, 5, 20)).await;
    let first = months(&env, me).await;
    env.clock.advance(chrono::Duration::days(40));
    let out = env
        .app
        .bulk(
            me,
            &[book, other.id],
            BulkOp::SetStatus(Status::completed()),
        )
        .await
        .unwrap();
    // the one that was Completed stays where it was, the other joins on its newest step
    assert_eq!(
        months(&env, me).await,
        [first[0].clone(), ("2026-05".to_string(), 1)]
    );
    // the undo takes back what the change did, and only that
    env.app.undo_bulk(me, &out.undo).await.unwrap();
    assert_eq!(months(&env, me).await, first);
}

/// BOOK-1 (4): a removed entry that Undo brings back is finished on the day it was.
#[tokio::test]
async fn a_removed_and_restored_entry_keeps_its_completion() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let book = read_book(&env, me).await;
    let first = months(&env, me).await;
    env.clock.advance(chrono::Duration::days(40));
    let out = env.app.bulk(me, &[book], BulkOp::Remove).await.unwrap();
    assert_eq!(months(&env, me).await, [], "no entry, no completion");
    env.app.undo_bulk(me, &out.undo).await.unwrap();
    assert_eq!(months(&env, me).await, first);
}

/// LB-15 (BOOK-2): the day is the newest logged step, not the day the status was written.
#[tokio::test]
async fn a_completion_by_hand_lies_on_the_newest_logged_step() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let book = read_book(&env, me).await;
    assert_eq!(months(&env, me).await, [("2025-03".to_string(), 1)]);
    let books = |from, to| {
        let store = env.store.clone();
        async move {
            ProgressRepo::window_amounts(&*store, me.user_id, from, to)
                .await
                .unwrap()
                .into_iter()
                .find(|k| k.kind.as_str() == "book")
                .map_or(0, |k| k.amounts.completions)
        }
    };
    assert_eq!(books(utc(2025, 3, 1), utc(2025, 4, 1)).await, 1);
    assert_eq!(books(utc(2026, 5, 1), utc(2026, 7, 1)).await, 0);

    // a step logged after the completion does not move it
    page(&env, me, book, 310, utc(2026, 5, 30)).await;
    assert_eq!(months(&env, me).await, [("2025-03".to_string(), 1)]);

    // moved away by hand it is gone; Completed again is a new completion, on the newest step
    env.app
        .set_status(me, book, Status::in_progress(), None)
        .await
        .unwrap();
    assert_eq!(total(&env, me).await, 0);
    complete(&env, me, book).await;
    assert_eq!(months(&env, me).await, [("2026-05".to_string(), 1)]);
}

/// LB-15 (BOOK-2): Completed without any logged step counts in the all-time totals and in
/// no month, year, year comparison or goal. A later Completed write gives it a day when a
/// step exists by then (an import writes the status before and after the history).
#[tokio::test]
async fn a_completed_entry_without_a_logged_step_is_undated() {
    let env = TestEnv::new();
    env.clock.set(utc(2026, 6, 1));
    let (_, me) = common::user(&env, "alice").await;
    let book = common::item(&env.app, "book", "Emma").await;
    let show = common::item(&env.app, "show", "Dark").await;
    env.app
        .add_to_library(me, book.id, Some(Status::completed()))
        .await
        .unwrap();
    complete(&env, me, show.id).await;

    let t = env.app.stats_totals(me).await.unwrap();
    assert_eq!(t.overall.amounts.completions, 2);
    let read = t.groups.iter().find(|g| g.group == KindGroup::Read);
    assert_eq!(read.unwrap().totals.amounts.completions, 1);
    assert_eq!(t.overall.amounts.events, 0);
    assert_eq!(t.undated_completions, 2, "the page says so in a footnote");
    for bucket in [Bucket::Month, Bucket::Year] {
        let series = env.app.stats_series(me, None, bucket).await.unwrap();
        assert_eq!(series, [], "no month and no year holds it");
    }
    let goal = env
        .app
        .set_goal(
            me,
            NewGoal {
                year: 2026,
                group: None,
                metric: GoalMetric::Completions,
                target: 12,
            },
        )
        .await
        .unwrap();
    assert_eq!(goal.done, 0);
    let compare = env.app.year_compare(me).await.unwrap();
    assert_eq!(compare.overall.this.completions, 0);
    assert_eq!(compare.overall.last.completions, 0);
    let all =
        ProgressRepo::window_amounts(&*env.store, me.user_id, utc(1990, 1, 1), utc(2090, 1, 1));
    assert_eq!(all.await.unwrap(), []);

    // Completed written again once a step exists: now it has a day, and keeps it
    page(&env, me, book.id, 80, utc(2024, 2, 3)).await;
    assert_eq!(months(&env, me).await, [], "a step alone dates nothing");
    complete(&env, me, book.id).await;
    assert_eq!(months(&env, me).await, [("2024-02".to_string(), 1)]);
    page(&env, me, book.id, 90, utc(2024, 4, 4)).await;
    complete(&env, me, book.id).await;
    assert_eq!(months(&env, me).await, [("2024-02".to_string(), 1)]);
    assert_eq!(total(&env, me).await, 2);
}

/// An entry an event completed is not counted a second time when Completed is written
/// again by hand (an import after the film was marked watched).
#[tokio::test]
async fn a_completion_by_an_event_is_not_doubled_by_a_status_write() {
    let env = TestEnv::new();
    env.clock.set(utc(2026, 6, 1));
    let (_, me) = common::user(&env, "alice").await;
    let film = common::item(&env.app, "movie", "Heat").await;
    env.app
        .log_progress(me, film.id, Progress::Finished, Some(utc(2026, 1, 5)), None)
        .await
        .unwrap();
    env.clock.advance(chrono::Duration::days(3));
    complete(&env, me, film.id).await;
    assert_eq!(months(&env, me).await, [("2026-01".to_string(), 1)]);
    assert_eq!(total(&env, me).await, 1);
}
