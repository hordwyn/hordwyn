//! The rewatch run of a show (model A, DEC-537, DEC-587): starting it sets In progress,
//! its last episode or stopping it sets Completed; the next step and "x of y" count within
//! the run; the header's count counts a run once.

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

mod common;

use app::{AppError, Bucket, NextStep};
use domain::ids::ItemId;
use domain::kind::Status;
use domain::progress::{Progress, ProgressEvent};
use domain::user::Actor;
use testkit::TestEnv;

fn ep(episode: u32) -> Progress {
    Progress::Episode { season: 1, episode }
}

/// A show with one season of `episodes` episodes, every one watched on its own day, and the
/// entry set to Completed by hand.
async fn completed_show(env: &TestEnv, me: Actor, episodes: u32) -> ItemId {
    let mut item = common::item(&env.app, "show", "Harbour Watch").await;
    item.attributes = serde_json::json!({ "seasons": [{"number": 1, "episode_count": episodes}] });
    env.store.state().catalog.insert(item.id, item.clone());
    for e in 1..=episodes {
        env.clock.advance(chrono::Duration::days(1));
        log(env, me, item.id, ep(e)).await;
    }
    env.app
        .set_status(me, item.id, Status::completed(), None)
        .await
        .unwrap();
    env.clock.advance(chrono::Duration::days(30));
    item.id
}

async fn log(env: &TestEnv, me: Actor, item: ItemId, p: Progress) -> ProgressEvent {
    env.clock.advance(chrono::Duration::minutes(1));
    env.app.log_progress(me, item, p, None, None).await.unwrap()
}

async fn status(env: &TestEnv, me: Actor, item: ItemId) -> String {
    let e = env.app.entry(me, item).await.unwrap().unwrap();
    e.status.value.as_str().to_string()
}

async fn times(env: &TestEnv, me: Actor, item: ItemId) -> u32 {
    let entry = env.app.entry(me, item).await.unwrap().unwrap();
    let it = env.app.catalog_item(item).await.unwrap();
    let kind = env.app.kind(&it.kind).unwrap();
    let history = env.app.history(me, item).await.unwrap();
    app::times(
        entry.status.value.as_str() == Status::COMPLETED,
        &kind,
        &it,
        &history,
    )
}

#[tokio::test]
async fn a_run_sets_in_progress_and_counts_within_itself() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 3).await;
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.next_step, NextStep::CaughtUp);
    assert_eq!(row.rewatch, None);

    let start = log(&env, me, show, Progress::Rewatch).await;
    let change = start.status_change.clone().unwrap();
    assert_eq!(
        (change.from.as_str(), change.to.as_str()),
        ("completed", "in_progress")
    );
    assert_eq!(status(&env, me, show).await, "in_progress");
    let row = env.app.library_row(me, show).await.unwrap();
    let run = row.rewatch.clone().unwrap();
    assert_eq!((run.done, run.total, run.running), (0, Some(3), true));
    assert_eq!(run.start, start.id);
    assert_eq!(
        row.next_step,
        NextStep::Episode {
            season: 1,
            episode: 1
        }
    );
    assert_eq!(row.fraction(), Some(0.0));
    // the summary still knows every view; the start is not a view
    assert_eq!(
        (row.summary.episodes.len(), row.summary.episode_views),
        (3, 3)
    );
    assert_eq!(times(&env, me, show).await, 1, "watched once before");

    log(&env, me, show, ep(1)).await;
    log(&env, me, show, ep(2)).await;
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch.as_ref().map(|r| r.done), Some(2));
    assert_eq!(
        row.next_step,
        NextStep::Episode {
            season: 1,
            episode: 3
        }
    );
    assert_eq!(status(&env, me, show).await, "in_progress");
    assert_eq!(times(&env, me, show).await, 1);

    // the last episode of the run gives Completed back
    let last = log(&env, me, show, ep(3)).await;
    assert_eq!(
        last.status_change.as_ref().map(|c| c.to.as_str()),
        Some("completed")
    );
    assert_eq!(status(&env, me, show).await, "completed");
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch, None, "a finished run is history");
    assert_eq!(row.next_step, NextStep::CaughtUp);
    assert_eq!(times(&env, me, show).await, 2);

    // undoing that episode reopens the run
    env.app.undo_progress(me, last.id).await.unwrap();
    assert_eq!(status(&env, me, show).await, "in_progress");
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(
        row.rewatch.as_ref().map(|r| (r.done, r.running)),
        Some((2, true))
    );
}

#[tokio::test]
async fn a_stopped_run_leaves_completed_and_keeps_the_views() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 3).await;
    log(&env, me, show, Progress::Rewatch).await;
    log(&env, me, show, ep(1)).await;
    log(&env, me, show, ep(2)).await;
    // stopping is the status write
    env.app
        .set_status(me, show, Status::completed(), None)
        .await
        .unwrap();
    assert_eq!(status(&env, me, show).await, "completed");
    let row = env.app.library_row(me, show).await.unwrap();
    let run = row.rewatch.clone().unwrap();
    assert_eq!((run.done, run.total, run.running), (2, Some(3), false));
    assert_eq!(
        row.summary.episode_views, 5,
        "the views stay in the history"
    );
    assert_eq!(row.next_step, NextStep::CaughtUp);
    assert_eq!(
        times(&env, me, show).await,
        1,
        "a stopped run is not a pass"
    );

    // start again: the new run counts from zero, the old views stay
    log(&env, me, show, Progress::Rewatch).await;
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(
        row.rewatch.as_ref().map(|r| (r.done, r.running)),
        Some((0, true))
    );
    assert_eq!(row.summary.episode_views, 5);
    // a run without an episode that is stopped has nothing to say
    env.app
        .set_status(me, show, Status::completed(), None)
        .await
        .unwrap();
    assert_eq!(env.app.library_row(me, show).await.unwrap().rewatch, None);
}

/// The count of ot2a ("Watched 3 times") counts a run once, however its episodes were
/// logged, and names the day its pass was done (review OT2A-7).
#[tokio::test]
async fn the_count_counts_a_run_once() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 2).await;
    let item = env.app.catalog_item(show).await.unwrap();
    let kind = env.app.kind(&item.kind).unwrap();
    let first_pass = env.app.history(me, show).await.unwrap()[1].occurred_at;
    // one favourite episode again, outside a run: no pass, and the day stays the first pass's
    log(&env, me, show, ep(1)).await;
    assert_eq!(times(&env, me, show).await, 1);
    let history = env.app.history(me, show).await.unwrap();
    assert_eq!(app::pass_at(&kind, &item, &history), Some(first_pass));

    log(&env, me, show, Progress::Rewatch).await;
    log(&env, me, show, ep(1)).await;
    log(&env, me, show, ep(1)).await;
    let last = log(&env, me, show, ep(2)).await;
    assert_eq!(status(&env, me, show).await, "completed");
    assert_eq!(times(&env, me, show).await, 2, "one run is one more time");
    let history = env.app.history(me, show).await.unwrap();
    assert_eq!(app::pass_at(&kind, &item, &history), Some(last.occurred_at));

    // a show completed by hand with half its episodes logged, then a whole run: the run is
    // the second time (the views alone would say once)
    let mut other = common::item(&env.app, "show", "Tideline").await;
    other.attributes = serde_json::json!({ "seasons": [{"number": 1, "episode_count": 2}] });
    env.store.state().catalog.insert(other.id, other.clone());
    log(&env, me, other.id, ep(1)).await;
    env.app
        .set_status(me, other.id, Status::completed(), None)
        .await
        .unwrap();
    assert_eq!(times(&env, me, other.id).await, 1);
    let history = env.app.history(me, other.id).await.unwrap();
    assert_eq!(
        app::pass_at(&kind, &other, &history),
        None,
        "no pass has a day"
    );
    log(&env, me, other.id, Progress::Rewatch).await;
    assert_eq!(
        times(&env, me, other.id).await,
        1,
        "once before, while it runs"
    );
    log(&env, me, other.id, ep(1)).await;
    let end = log(&env, me, other.id, ep(2)).await;
    assert_eq!(status(&env, me, other.id).await, "completed");
    assert_eq!(times(&env, me, other.id).await, 2);
    let history = env.app.history(me, other.id).await.unwrap();
    assert_eq!(app::pass_at(&kind, &other, &history), Some(end.occurred_at));
}

#[tokio::test]
async fn what_is_not_a_run() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    // a movie refuses the start and never leaves Completed
    let movie = common::item(&env.app, "movie", "Heat").await;
    log(&env, me, movie.id, Progress::Finished).await;
    let refused = env
        .app
        .log_progress(me, movie.id, Progress::Rewatch, None, None)
        .await;
    assert!(refused.is_err());
    let again = log(&env, me, movie.id, Progress::Finished).await;
    assert_eq!(again.status_change, None);
    assert_eq!(status(&env, me, movie.id).await, "completed");

    // a start on a show that is In progress starts nothing: it is refused and leaves no
    // row in the history (review OT2E-12)
    let show = completed_show(&env, me, 2).await;
    env.app
        .set_status(me, show, Status::in_progress(), None)
        .await
        .unwrap();
    let start = env
        .app
        .log_progress(me, show, Progress::Rewatch, None, None)
        .await;
    assert!(matches!(start, Err(AppError::Conflict(_))), "{start:?}");
    assert_eq!(env.app.progress(me, show).await.unwrap().len(), 2);
    // the dry run that "watched with" asks first says the same, also without an entry
    let other = common::item(&env.app, "show", "Unshelved").await;
    for item in [show, other.id] {
        let asked = env
            .app
            .check_progress(me, item, &Progress::Rewatch, None, None)
            .await;
        assert!(matches!(asked, Err(AppError::Conflict(_))), "{asked:?}");
    }
    assert!(env.app.entry(me, other.id).await.unwrap().is_none());
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch, None);
    assert_eq!(row.next_step, NextStep::CaughtUp);
    // and its episodes complete nothing
    log(&env, me, show, ep(1)).await;
    log(&env, me, show, ep(2)).await;
    assert_eq!(status(&env, me, show).await, "in_progress");

    // views dated before the start (an import, a backfill) are not the run's
    let show = {
        let mut item = common::item(&env.app, "show", "Saltmarsh").await;
        item.attributes = serde_json::json!({ "seasons": [{"number": 1, "episode_count": 2}] });
        env.store.state().catalog.insert(item.id, item.clone());
        log(&env, me, item.id, ep(1)).await;
        log(&env, me, item.id, ep(2)).await;
        env.app
            .set_status(me, item.id, Status::completed(), None)
            .await
            .unwrap();
        item.id
    };
    let before = env.clock.now_for_tests() - chrono::Duration::days(400);
    log(&env, me, show, Progress::Rewatch).await;
    for e in 1..=2 {
        env.app
            .import_progress(me, show, ep(e), Some(before), None)
            .await
            .unwrap();
    }
    assert_eq!(status(&env, me, show).await, "in_progress");
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch.as_ref().map(|r| r.done), Some(0));

    // the skipped episode of a run whose last one is watched is the next step
    log(&env, me, show, ep(2)).await;
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(
        row.next_step,
        NextStep::Episode {
            season: 1,
            episode: 1
        }
    );
}

/// The Undo of "Log N again" takes a whole run back: its episodes newest first, then the
/// start. The last episode's undo reopens the run (In progress), the start's undo gives
/// Completed back although the status was written again in between (by that first undo).
#[tokio::test]
async fn undoing_a_whole_run_gives_completed_back() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 2).await;
    let start = log(&env, me, show, Progress::Rewatch).await;
    let first = log(&env, me, show, ep(1)).await;
    let last = log(&env, me, show, ep(2)).await;
    assert_eq!(status(&env, me, show).await, "completed");
    for event in [&last, &first] {
        env.clock.advance(chrono::Duration::seconds(1));
        env.app.undo_progress(me, event.id).await.unwrap();
    }
    assert_eq!(status(&env, me, show).await, "in_progress");
    env.clock.advance(chrono::Duration::seconds(1));
    env.app.undo_progress(me, start.id).await.unwrap();
    assert_eq!(status(&env, me, show).await, "completed");
    assert_eq!(env.app.library_row(me, show).await.unwrap().rewatch, None);
    assert_eq!(times(&env, me, show).await, 1);

    // a status chosen by hand in between stays (DEC-208): start, paused and back to In
    // progress by hand, then the start is undone
    let start = log(&env, me, show, Progress::Rewatch).await;
    for s in [Status::new(Status::PAUSED), Status::in_progress()] {
        env.clock.advance(chrono::Duration::seconds(1));
        env.app.set_status(me, show, s, None).await.unwrap();
    }
    env.app.undo_progress(me, start.id).await.unwrap();
    assert_eq!(status(&env, me, show).await, "in_progress");
}

async fn completions(env: &TestEnv, me: Actor) -> u64 {
    let months = env.app.stats_series(me, None, Bucket::Month).await.unwrap();
    months.iter().map(|p| p.amounts.completions).sum()
}

/// Stopping writes a marker (review OT2E-2, DEC-590): the run ends there. It does not come
/// back with a later In progress, later views are not its, and it is no completion.
#[tokio::test]
async fn a_stop_marker_ends_the_run_for_good() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 4).await;
    log(&env, me, show, Progress::Rewatch).await;
    log(&env, me, show, ep(1)).await;
    log(&env, me, show, ep(2)).await;
    let stop = log(&env, me, show, Progress::RewatchStop).await;
    assert_eq!(
        stop.status_change
            .as_ref()
            .map(|c| (c.from.as_str(), c.to.as_str())),
        Some(("in_progress", "completed"))
    );
    assert_eq!(status(&env, me, show).await, "completed");
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(
        row.rewatch.as_ref().map(|r| (r.done, r.running)),
        Some((2, false))
    );
    assert_eq!(times(&env, me, show).await, 1);
    assert_eq!(completions(&env, me).await, 0, "a stop is no completion");

    // "Again" on a favourite episode next week: an extra view, not the run's
    env.clock.advance(chrono::Duration::days(7));
    log(&env, me, show, ep(3)).await;
    assert_eq!(status(&env, me, show).await, "completed");
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch.as_ref().map(|r| r.done), Some(2));
    assert_eq!(times(&env, me, show).await, 1, "a stopped run is no pass");

    // a new season months later, In progress by hand: no run comes back
    let mut item = env.app.catalog_item(show).await.unwrap();
    item.attributes = serde_json::json!({ "seasons": [
        {"number": 1, "episode_count": 4}, {"number": 2, "episode_count": 2}] });
    env.store.state().catalog.insert(item.id, item.clone());
    env.clock.advance(chrono::Duration::days(90));
    env.app
        .set_status(me, show, Status::in_progress(), None)
        .await
        .unwrap();
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch, None);
    assert_eq!(
        row.next_step,
        NextStep::Episode {
            season: 2,
            episode: 1
        }
    );
    let first = log(
        &env,
        me,
        show,
        Progress::Episode {
            season: 2,
            episode: 1,
        },
    )
    .await;
    assert_eq!(first.status_change, None);

    // nothing is open: a stop is refused and leaves no row
    let stored = env.app.progress(me, show).await.unwrap().len();
    let again = env
        .app
        .log_progress(me, show, Progress::RewatchStop, None, None)
        .await;
    assert!(matches!(again, Err(AppError::Conflict(_))), "{again:?}");
    assert_eq!(env.app.progress(me, show).await.unwrap().len(), stored);
}

/// The stop can be undone like the start: the run runs again.
#[tokio::test]
async fn undoing_a_stop_reopens_the_run() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 3).await;
    log(&env, me, show, Progress::Rewatch).await;
    log(&env, me, show, ep(1)).await;
    let stop = log(&env, me, show, Progress::RewatchStop).await;
    env.clock.advance(chrono::Duration::seconds(1));
    env.app.undo_progress(me, stop.id).await.unwrap();
    assert_eq!(status(&env, me, show).await, "in_progress");
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(
        row.rewatch.as_ref().map(|r| (r.done, r.running)),
        Some((1, true))
    );
    // a status write without the marker (the status select, an older client) is as before:
    // stopped while Completed, running again with In progress
    for (to, running) in [(Status::completed(), false), (Status::in_progress(), true)] {
        env.clock.advance(chrono::Duration::seconds(1));
        env.app.set_status(me, show, to, None).await.unwrap();
        let row = env.app.library_row(me, show).await.unwrap();
        assert_eq!(row.rewatch.as_ref().map(|r| r.running), Some(running));
    }
}

/// Review OT2E-1: while a run is running a view dated before its start would be logged and
/// not counted. The server refuses it; a view without a date never sorts before the start,
/// also when the start carries a client clock that runs ahead.
#[tokio::test]
async fn a_view_dated_before_the_start_of_a_running_run_is_refused() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 3).await;
    let yesterday = env.clock.now_for_tests() - chrono::Duration::days(1);
    // a start from a queue whose clock is two minutes ahead
    let ahead = env.clock.now_for_tests() + chrono::Duration::minutes(2);
    let start = env
        .app
        .log_progress(me, show, Progress::Rewatch, Some(ahead), None)
        .await
        .unwrap();
    let stored = env.app.progress(me, show).await.unwrap().len();
    let refused = env
        .app
        .log_progress(me, show, ep(1), Some(yesterday), None)
        .await;
    assert!(
        matches!(&refused, Err(AppError::Domain(e)) if e.to_string().contains("occurred_at")),
        "{refused:?}"
    );
    assert_eq!(env.app.progress(me, show).await.unwrap().len(), stored);
    // logged now, by the server's clock (before the start's): it is the run's
    let first = log(&env, me, show, ep(1)).await;
    assert!(first.occurred_at > start.occurred_at);
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch.as_ref().map(|r| r.done), Some(1));
    // a special is no part of a run: its date is free
    let special = Progress::Episode {
        season: 0,
        episode: 1,
    };
    env.app
        .log_progress(me, show, special, Some(yesterday), None)
        .await
        .unwrap();
    // a replay of a log that is stored answers that log, whatever the run says now
    let op = Some(uuid::Uuid::from_u128(7));
    let once = env
        .app
        .log_progress(me, show, ep(2), None, op)
        .await
        .unwrap();
    let twice = env
        .app
        .log_progress(me, show, ep(2), Some(yesterday), op)
        .await
        .unwrap();
    assert_eq!(once.id, twice.id);
    // once the run is stopped, a view of yesterday is a plain view again
    log(&env, me, show, Progress::RewatchStop).await;
    env.app
        .log_progress(me, show, ep(3), Some(yesterday), None)
        .await
        .unwrap();
}

/// Review OT2E-12: a start is dated by the server unless a queue sends its own time, and a
/// start dated before the newest view (it would turn the first run's views into a run) is
/// refused. A replay of a stored start answers that start.
#[tokio::test]
async fn a_start_cannot_be_dated_before_the_newest_view() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 2).await;
    let long_ago = env.clock.now_for_tests() - chrono::Duration::days(400);
    let refused = env
        .app
        .log_progress(me, show, Progress::Rewatch, Some(long_ago), None)
        .await;
    assert!(matches!(refused, Err(AppError::Domain(_))), "{refused:?}");
    assert_eq!(status(&env, me, show).await, "completed");
    assert_eq!(times(&env, me, show).await, 1);
    assert_eq!(env.app.progress(me, show).await.unwrap().len(), 2);

    let op = Some(uuid::Uuid::from_u128(9));
    let start = env
        .app
        .log_progress(me, show, Progress::Rewatch, None, op)
        .await
        .unwrap();
    // the entry is In progress now: a new start would be refused, the replay is not
    let replay = env
        .app
        .log_progress(me, show, Progress::Rewatch, None, op)
        .await
        .unwrap();
    assert_eq!(replay.id, start.id);
    assert_eq!(env.app.progress(me, show).await.unwrap().len(), 3);
}

/// The limit DEC-589 names: a run that covers every known episode without an event that
/// finished it (its last episode was logged while the entry was paused) counts as over.
#[tokio::test]
async fn a_run_that_covers_every_episode_is_over_without_a_finishing_event() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let show = completed_show(&env, me, 2).await;
    log(&env, me, show, Progress::Rewatch).await;
    log(&env, me, show, ep(1)).await;
    env.app
        .set_status(me, show, Status::new(Status::PAUSED), None)
        .await
        .unwrap();
    let last = log(&env, me, show, ep(2)).await;
    assert_eq!(last.status_change, None);
    env.clock.advance(chrono::Duration::seconds(1));
    env.app
        .set_status(me, show, Status::in_progress(), None)
        .await
        .unwrap();
    let row = env.app.library_row(me, show).await.unwrap();
    assert_eq!(row.rewatch, None, "not shown as running");
    assert_eq!(row.next_step, NextStep::CaughtUp);
    assert_eq!(times(&env, me, show).await, 2, "it counts as a pass");
    // one more view finishes nothing a second time
    let more = log(&env, me, show, ep(1)).await;
    assert_eq!(more.status_change, None);
}
