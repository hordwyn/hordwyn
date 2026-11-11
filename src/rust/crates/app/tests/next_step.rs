//! `app::next_step` per progress model (design.md 3.4).

// one scenario per test, many assertions
#![allow(clippy::too_many_lines)]

use app::{NextStep, next_step};
use chrono::{TimeZone, Utc};
use domain::catalog::{CatalogItem, EntityState};
use domain::hlc::{Hlc, Lww};
use domain::ids::{EntryId, EventId, ItemId, SessionId, UserId};
use domain::kind::{KindDescriptor, KindGroup, KindId, ProgressModel, Status};
use domain::library::LibraryEntry;
use domain::progress::{Progress, ProgressEvent};
use serde_json::json;

fn kind(id: &str, model: ProgressModel) -> KindDescriptor {
    KindDescriptor {
        id: KindId::new(id).unwrap(),
        group: KindGroup::Watch,
        progress_model: model,
        extra_statuses: vec![],
        attributes_schema: json!({}),
        facets: vec![],
        relations: vec![],
        default_connectors: vec![],
    }
}

fn item(kind: &str, attributes: serde_json::Value) -> CatalogItem {
    CatalogItem {
        id: ItemId(uuid::Uuid::nil()),
        kind: KindId::new(kind).unwrap(),
        title: "x".into(),
        original_title: None,
        year: None,
        description: None,
        cover_asset: None,
        cover_url: None,
        external_ids: vec![],
        attributes,
        provenance: std::collections::BTreeMap::new(),
        state: EntityState::Full,
        fetched_at: None,
        fresh_until: None,
    }
}

fn entry(kind: &str, status: &str) -> LibraryEntry {
    LibraryEntry {
        id: EntryId(uuid::Uuid::nil()),
        user_id: UserId(uuid::Uuid::nil()),
        item_id: ItemId(uuid::Uuid::nil()),
        kind: KindId::new(kind).unwrap(),
        status: Lww::new(Status::new(status), Hlc::default()),
        rating: Lww::new(None, Hlc::default()),
        notes: Lww::new(None, Hlc::default()),
        added_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    }
}

fn ev(n: u128, progress: Progress) -> ProgressEvent {
    let t = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
        + chrono::Duration::hours(i64::try_from(n).unwrap());
    ProgressEvent {
        id: EventId(uuid::Uuid::from_u128(n)),
        user_id: UserId(uuid::Uuid::nil()),
        item_id: ItemId(uuid::Uuid::nil()),
        occurred_at: t,
        progress,
        client_op_id: None,
        recorded_at: t,
        status_change: None,
    }
}

fn ep(n: u128, season: u32, episode: u32) -> ProgressEvent {
    ev(n, Progress::Episode { season, episode })
}

#[test]
fn single_is_watched_then_watched_again() {
    let k = kind("movie", ProgressModel::Single);
    let i = item("movie", json!({}));
    assert_eq!(
        next_step(&entry("movie", "planned"), &k, &i, &[]),
        NextStep::Finish { again: false }
    );
    assert_eq!(
        next_step(
            &entry("movie", "completed"),
            &k,
            &i,
            &[ev(1, Progress::Finished)]
        ),
        NextStep::Finish { again: true }
    );
}

#[test]
fn episodic_is_the_next_unwatched_episode_across_seasons() {
    let k = kind("show", ProgressModel::Episodic);
    let i = item(
        "show",
        json!({"seasons": [{"number": 0, "episode_count": 3}, {"number": 1, "episode_count": 2},
                           {"number": 2, "episode_count": 8}]}),
    );
    let e = entry("show", "in_progress");
    let first = NextStep::Episode {
        season: 1,
        episode: 1,
    };
    assert_eq!(next_step(&e, &k, &i, &[]), first);
    // a special does not move the next episode
    assert_eq!(next_step(&e, &k, &i, &[ep(1, 0, 2)]), first);
    assert_eq!(
        next_step(&e, &k, &i, &[ep(1, 1, 1), ep(2, 1, 2)]),
        NextStep::Episode {
            season: 2,
            episode: 1
        }
    );
    // rewatching an old episode does not move it back
    assert_eq!(
        next_step(&e, &k, &i, &[ep(1, 2, 4), ep(2, 1, 1)]),
        NextStep::Episode {
            season: 2,
            episode: 5
        }
    );
    assert_eq!(next_step(&e, &k, &i, &[ep(1, 2, 8)]), NextStep::CaughtUp);
    // an undone episode is not watched
    let undo = ev(
        3,
        Progress::Undo {
            event_id: EventId(uuid::Uuid::from_u128(2)),
        },
    );
    assert_eq!(
        next_step(&e, &k, &i, &[ep(1, 1, 1), ep(2, 1, 2), undo]),
        NextStep::Episode {
            season: 1,
            episode: 2
        }
    );
}

#[test]
fn episodic_without_season_data_counts_on_or_uses_the_episode_count() {
    let k = kind("anime", ProgressModel::Episodic);
    let e = entry("anime", "in_progress");
    let unknown = item("anime", json!({}));
    assert_eq!(
        next_step(&e, &k, &unknown, &[ep(1, 1, 12)]),
        NextStep::Episode {
            season: 1,
            episode: 13
        }
    );
    let twelve = item("anime", json!({"episode_count": 12}));
    assert_eq!(
        next_step(&e, &k, &twelve, &[ep(1, 1, 12)]),
        NextStep::CaughtUp
    );
}

#[test]
fn paged_timed_and_sessions_report_where_you_are() {
    let book = kind("book", ProgressModel::Paged);
    let i = item("book", json!({"page_count": 320}));
    let page = |n, p| {
        ev(
            n,
            Progress::Paged {
                page: Some(p),
                chapter: None,
                volume: None,
            },
        )
    };
    assert_eq!(
        next_step(
            &entry("book", "in_progress"),
            &book,
            &i,
            &[page(1, 10), page(2, 212)]
        ),
        NextStep::Page {
            page: Some(212),
            chapter: None,
            pages: Some(320),
            chapters: None
        }
    );
    let audio = kind("audiobook", ProgressModel::Timed);
    let i = item("audiobook", json!({"duration_seconds": 36000}));
    assert_eq!(
        next_step(
            &entry("audiobook", "planned"),
            &audio,
            &i,
            &[ev(1, Progress::Position { seconds: 900 })]
        ),
        NextStep::Position {
            seconds: Some(900),
            total: Some(36000)
        }
    );
    let game = kind("board_game", ProgressModel::Sessions);
    let sid = SessionId(uuid::Uuid::nil());
    assert_eq!(
        next_step(
            &entry("board_game", "in_progress"),
            &game,
            &item("board_game", json!({})),
            &[
                ev(1, Progress::Session { session_id: sid }),
                ev(2, Progress::Session { session_id: sid })
            ]
        ),
        NextStep::Session { plays: 2 }
    );
}

/// OT-4: an item that is not in the library still has a first step (the item page offers
/// "Mark as watched" next to "Want to watch").
#[test]
fn an_item_without_an_entry_has_a_first_step() {
    let movie = kind("movie", ProgressModel::Single);
    let i = item("movie", json!({}));
    assert_eq!(
        app::first_step(&movie, &i, &[]),
        NextStep::Finish { again: false }
    );
    // removed from the library, the history stays: the next watch is a rewatch
    assert_eq!(
        app::first_step(&movie, &i, &[ev(1, Progress::Finished)]),
        NextStep::Finish { again: true }
    );
    let show = kind("show", ProgressModel::Episodic);
    let s = item(
        "show",
        json!({"seasons": [{"number": 1, "episode_count": 2}]}),
    );
    assert_eq!(
        app::first_step(&show, &s, &[]),
        NextStep::Episode {
            season: 1,
            episode: 1
        }
    );
    assert_eq!(
        app::first_step(&kind("board_game", ProgressModel::Sessions), &i, &[]),
        NextStep::Session { plays: 0 }
    );
}

/// OT-7: "Watched 3 times" counts whole passes, never "once plus N rewatches".
#[test]
fn times_counts_whole_passes_per_model() {
    let movie = kind("movie", ProgressModel::Single);
    let i = item("movie", json!({}));
    assert_eq!(app::times(false, &movie, &i, &[]), 0);
    let three = [
        ev(1, Progress::Finished),
        ev(2, Progress::Finished),
        ev(3, Progress::Finished),
    ];
    assert_eq!(app::times(true, &movie, &i, &three), 3);
    // an undone watch does not count
    let undone = [
        ev(1, Progress::Finished),
        ev(2, Progress::Finished),
        ev(
            3,
            Progress::Undo {
                event_id: EventId(uuid::Uuid::from_u128(2)),
            },
        ),
    ];
    assert_eq!(app::times(true, &movie, &i, &undone), 1);
    // completed by hand, nothing logged: once
    assert_eq!(app::times(true, &movie, &i, &[]), 1);

    // a show: every known regular episode watched n times is n passes; specials do not count
    let show = kind("show", ProgressModel::Episodic);
    let s = item(
        "show",
        json!({"seasons": [{"number": 0, "episode_count": 1}, {"number": 1, "episode_count": 2}]}),
    );
    assert_eq!(app::times(false, &show, &s, &[ep(1, 1, 1)]), 0);
    assert_eq!(app::times(false, &show, &s, &[ep(1, 1, 1), ep(2, 1, 2)]), 1);
    let again = [ep(1, 1, 1), ep(2, 1, 2), ep(3, 1, 1), ep(4, 0, 1)];
    assert_eq!(app::times(false, &show, &s, &again), 1);
    let twice = [ep(1, 1, 1), ep(2, 1, 2), ep(3, 1, 1), ep(4, 1, 2)];
    assert_eq!(app::times(false, &show, &s, &twice), 2);
    // completed by hand with episodes open (or none known): once
    assert_eq!(app::times(true, &show, &s, &[ep(1, 1, 1)]), 1);
    assert_eq!(
        app::times(false, &show, &item("show", json!({})), &[ep(1, 1, 1)]),
        0
    );

    // plays are counted as they are
    let game = kind("board_game", ProgressModel::Sessions);
    let play = |n| {
        ev(
            n,
            Progress::Session {
                session_id: SessionId(uuid::Uuid::from_u128(n)),
            },
        )
    };
    assert_eq!(app::times(false, &game, &i, &[play(1), play(2)]), 2);

    // a book finished by status only (pages do not complete it): once
    let book = kind("book", ProgressModel::Paged);
    assert_eq!(app::times(false, &book, &i, &[]), 0);
    assert_eq!(app::times(true, &book, &i, &[]), 1);
}

/// OT2A-1: the button, the dialog and the header count the same thing. A completion set by
/// hand (or imported without a date) and the first logged finish are one and the same pass:
/// the step is not "again" before a finish is logged, and the count does not stand still
/// after a step that said "again".
#[test]
fn a_completion_by_hand_and_the_first_finish_are_one_pass() {
    let movie = kind("movie", ProgressModel::Single);
    let i = item("movie", json!({}));
    let done = entry("movie", "completed");
    // completed by hand, nothing logged: counted once, and the log that follows is that watch
    assert_eq!(app::times(true, &movie, &i, &[]), 1);
    assert_eq!(
        next_step(&done, &movie, &i, &[]),
        NextStep::Finish { again: false }
    );
    // then one finish: still once (now with a day), and the next one is a rewatch
    let one = [ev(1, Progress::Finished)];
    assert_eq!(app::times(true, &movie, &i, &one), 1);
    assert_eq!(
        next_step(&done, &movie, &i, &one),
        NextStep::Finish { again: true }
    );
    // every step that says "again" raises the count
    let two = [ev(1, Progress::Finished), ev(2, Progress::Finished)];
    assert_eq!(app::times(true, &movie, &i, &two), 2);
    // the same without an entry (removed, the history stayed)
    assert_eq!(
        app::first_step(&movie, &i, &one),
        NextStep::Finish { again: true }
    );
}
