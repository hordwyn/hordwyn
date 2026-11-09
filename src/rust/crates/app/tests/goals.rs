//! WP4-6 opt-in yearly goals (design.md 9, R-04) and "this year so far vs the same period last
//! year" (design.md 6.1, 6.6): a goal's progress is the year's sum of its measure; goals are
//! private; the same period runs from January 1 to the same local day.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{AppError, NewGoal};
use chrono::{TimeZone, Utc};
use domain::goal::GoalMetric;
use domain::kind::KindGroup;
use domain::progress::Progress;
use testkit::TestEnv;

fn utc(y: i32, m: u32, d: u32, h: u32, min: u32) -> domain::Timestamp {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0).unwrap()
}

fn goal(year: i32, group: Option<KindGroup>, metric: GoalMetric, target: u32) -> NewGoal {
    NewGoal {
        year,
        group,
        metric,
        target,
    }
}

#[tokio::test]
async fn a_goal_counts_the_years_sum_of_its_measure_and_is_private() {
    let env = TestEnv::new();
    env.clock.set(utc(2026, 6, 1, 12, 0));
    let (_, alice) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    // alice and bob are linked people: goals still stay with their owner
    let c = env.app.create_companion(alice, "Bob").await.unwrap();
    let req = env
        .app
        .request_link(alice, c.id, bob_user.id)
        .await
        .unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();

    for (title, at) in [
        ("Dune", utc(2026, 2, 1, 10, 0)),
        ("Emma", utc(2026, 3, 1, 10, 0)),
        ("Ulysses", utc(2026, 5, 30, 10, 0)),
        ("Old", utc(2025, 12, 31, 10, 0)),
    ] {
        let book = common::item(&env.app, "movie", title).await;
        env.app
            .log_progress(alice, book.id, Progress::Finished, Some(at), None)
            .await
            .unwrap();
    }
    let book = common::item(&env.app, "book", "Heat").await;
    env.app
        .log_progress(
            alice,
            book.id,
            Progress::Paged {
                page: Some(120),
                chapter: None,
                volume: None,
            },
            Some(utc(2026, 4, 1, 10, 0)),
            None,
        )
        .await
        .unwrap();

    let read = env
        .app
        .set_goal(
            alice,
            goal(2026, Some(KindGroup::Watch), GoalMetric::Finished, 12),
        )
        .await
        .unwrap();
    assert_eq!((read.done, read.goal.target), (3, 12), "only 2026's films");
    let all = env
        .app
        .set_goal(alice, goal(2026, None, GoalMetric::Finished, 50))
        .await
        .unwrap();
    assert_eq!(all.done, 3, "every group, only 2026");
    let pages = env
        .app
        .set_goal(
            alice,
            goal(2026, Some(KindGroup::Read), GoalMetric::Pages, 500),
        )
        .await
        .unwrap();
    assert_eq!(pages.done, 120, "the book, not the films");
    let last_year = env
        .app
        .set_goal(
            alice,
            goal(2025, Some(KindGroup::Watch), GoalMetric::Finished, 5),
        )
        .await
        .unwrap();
    assert_eq!(last_year.done, 1);
    // one goal per year, group and measure
    assert!(matches!(
        env.app
            .set_goal(
                alice,
                goal(2026, Some(KindGroup::Watch), GoalMetric::Finished, 20)
            )
            .await,
        Err(AppError::Conflict(_))
    ));
    assert!(
        env.app
            .set_goal(alice, goal(2026, None, GoalMetric::Plays, 0))
            .await
            .is_err(),
        "a target of 0 is no goal"
    );

    let listed = env.app.goals(alice).await.unwrap();
    assert_eq!(listed.len(), 4);
    assert_eq!(listed[0].goal.year, 2026, "newest year first");

    // edit and delete
    let edited = env
        .app
        .update_goal(
            alice,
            read.goal.id,
            goal(2026, Some(KindGroup::Watch), GoalMetric::Finished, 6),
        )
        .await
        .unwrap();
    assert_eq!((edited.done, edited.goal.target), (3, 6));
    env.app.delete_goal(alice, last_year.goal.id).await.unwrap();
    assert_eq!(env.app.goals(alice).await.unwrap().len(), 3);

    // private: a linked person sees none of them and cannot change or delete them
    assert_eq!(env.app.goals(bob).await.unwrap().len(), 0);
    assert!(
        env.app
            .update_goal(
                bob,
                read.goal.id,
                goal(2026, Some(KindGroup::Watch), GoalMetric::Finished, 1)
            )
            .await
            .is_err()
    );
    assert!(env.app.delete_goal(bob, all.goal.id).await.is_err());
    let still = env.app.goals(alice).await.unwrap();
    assert!(
        still
            .iter()
            .any(|g| g.goal.id == read.goal.id && g.goal.target == 6)
    );
    assert!(still.iter().any(|g| g.goal.id == all.goal.id));
}

#[tokio::test]
async fn the_same_period_ends_on_the_same_local_day_and_feb_29_maps_to_feb_28() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    // Auckland is UTC+13 in summer: local days start at 11:00 UTC the day before
    let zone = app::time_zone::parse_zone("Pacific/Auckland").unwrap();
    env.app.set_time_zone(me, Some(zone)).await.unwrap();
    // today: 2028-02-29 10:00 local (a leap day)
    env.clock.set(utc(2028, 2, 28, 21, 0));
    let cases = [
        // this year so far
        ("a", utc(2027, 12, 31, 11, 10)), // 2028-01-01 00:10 local
        ("b", utc(2028, 2, 28, 20, 0)),   // 2028-02-29 09:00 local
        // last year's same period: 2027-01-01 .. 2027-02-28 (no Feb 29 in 2027)
        ("c", utc(2026, 12, 31, 11, 10)), // 2027-01-01 00:10 local
        ("d", utc(2027, 2, 28, 10, 0)),   // 2027-02-28 23:00 local
        ("e", utc(2027, 2, 28, 11, 30)),  // 2027-03-01 00:30 local: after the period
        ("f", utc(2027, 1, 15, 10, 0)),   // inside, a full month
        ("g", utc(2026, 12, 31, 10, 0)),  // 2026-12-31 23:00 local: before
    ];
    for (n, at) in cases {
        let item = common::item(&env.app, "movie", n).await;
        env.app
            .log_progress(me, item.id, Progress::Finished, Some(at), None)
            .await
            .unwrap();
    }
    let c = env.app.year_compare(me).await.unwrap();
    assert_eq!(c.year, 2028);
    assert_eq!(c.through.to_string(), "2028-02-29");
    assert_eq!(c.last_through.to_string(), "2027-02-28");
    assert_eq!(c.overall.this.finished, 2);
    assert_eq!(c.overall.last.finished, 3, "c, d and f");
    let watch = c
        .groups
        .iter()
        .find(|(g, _)| *g == KindGroup::Watch)
        .map(|(_, p)| *p)
        .unwrap();
    assert_eq!((watch.this.finished, watch.last.finished), (2, 3));
}

#[tokio::test]
async fn on_january_1_both_periods_are_that_one_day() {
    let env = TestEnv::new();
    let (user, me) = common::user(&env, "alice").await;
    let zone = app::time_zone::parse_zone("Pacific/Auckland").unwrap();
    env.app.set_time_zone(me, Some(zone)).await.unwrap();
    // 2027-01-01 08:00 local
    env.clock.set(utc(2026, 12, 31, 19, 0));
    let game = common::item(&env.app, "board_game", "Azul").await;
    for at in [
        utc(2026, 12, 31, 12, 0), // 2027-01-01 01:00 local: this year
        utc(2026, 12, 31, 10, 0), // 2026-12-31 23:00 local: last year, after its Jan 1
        utc(2025, 12, 31, 12, 0), // 2026-01-01 01:00 local: last year's Jan 1
        utc(2026, 1, 1, 12, 0),   // 2026-01-02 01:00 local: after last year's period
    ] {
        let mut s = app::NewSession::new(
            game.id,
            vec![domain::session::Participant::new(user.self_companion)],
        );
        s.started_at = Some(at);
        env.app.log_session(me, s).await.unwrap();
    }
    let c = env.app.year_compare(me).await.unwrap();
    assert_eq!(c.through.to_string(), "2027-01-01");
    assert_eq!(c.last_through.to_string(), "2026-01-01");
    assert_eq!((c.overall.this.plays, c.overall.last.plays), (1, 1));
}
