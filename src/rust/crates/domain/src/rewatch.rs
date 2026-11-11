//! The rewatch run of an episodic item (model A: the status says where the person is now,
//! the history counts every time; DEC-537, DEC-587).
//!
//! A run is not stored: it is read from the effective history. It starts with a
//! [`Progress::Rewatch`] event that moved the entry from Completed to In progress (the move
//! is on the event, [`StatusChange`]); a start logged on any other status carries no move
//! and starts nothing. The regular episodes logged after the start, up to the next start,
//! are the run's. The episode that makes the run cover every known episode carries the move
//! back to Completed: the run is finished. A [`Progress::RewatchStop`] marker ends the run
//! unfinished (DEC-590): it carries In progress -> Completed, and nothing logged after it is
//! the run's. A status set to Completed by hand (the status select, an older client) leaves
//! no event: the run then reads as stopped while the entry is Completed and runs again when
//! the entry is set back to In progress.

use std::collections::BTreeSet;

use crate::kind::Status;
use crate::progress::{Progress, ProgressEvent, StatusChange, effective};

/// One rewatch run, read from the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run<'a> {
    /// The start: a [`Progress::Rewatch`] event carrying Completed -> In progress.
    pub start: &'a ProgressEvent,
    /// Distinct regular episodes (season above 0) logged in the run.
    pub episodes: BTreeSet<(u32, u32)>,
    /// The episode that finished the run (it carries the move to Completed).
    pub finished: Option<&'a ProgressEvent>,
    /// The stop marker that ended the run unfinished ([`Progress::RewatchStop`], DEC-590).
    pub stopped: Option<&'a ProgressEvent>,
}

/// Where a run stands, given the entry's status now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Unfinished and the entry is In progress.
    Running,
    /// Unfinished: a stop marker ended it, or the entry is Completed again without one.
    Stopped,
    /// Its last episode moved the entry back to Completed.
    Finished,
}

fn moves(change: Option<&StatusChange>, from: &str, to: &str) -> bool {
    change.is_some_and(|c| c.from.as_str() == from && c.to.as_str() == to)
}

/// Does this event start a run?
#[must_use]
pub fn starts_run(e: &ProgressEvent) -> bool {
    matches!(e.progress, Progress::Rewatch)
        && moves(
            e.status_change.as_ref(),
            Status::COMPLETED,
            Status::IN_PROGRESS,
        )
}

/// Every run of a history (undo events included or not), oldest first. The history order is
/// `(occurred_at, id)`: views dated before a start (an import, a backfill) are not the
/// run's, whenever they were recorded.
#[must_use]
pub fn runs(events: &[ProgressEvent]) -> Vec<Run<'_>> {
    let mut live = effective(events);
    live.sort_by_key(|e| (e.occurred_at, e.id));
    let mut out: Vec<Run<'_>> = Vec::new();
    for e in live {
        if starts_run(e) {
            out.push(Run {
                start: e,
                episodes: BTreeSet::new(),
                finished: None,
                stopped: None,
            });
            continue;
        }
        let Some(run) = out.last_mut().filter(|r| r.is_open()) else {
            continue;
        };
        if matches!(e.progress, Progress::RewatchStop) {
            run.stopped = Some(e);
            continue;
        }
        let Progress::Episode { season, episode } = &e.progress else {
            continue;
        };
        if *season == 0 {
            continue;
        }
        run.episodes.insert((*season, *episode));
        if e.status_change
            .as_ref()
            .is_some_and(|c| c.to.as_str() == Status::COMPLETED)
        {
            run.finished = Some(e);
        }
    }
    out
}

impl Run<'_> {
    /// The run of a history that is the newest one.
    #[must_use]
    pub fn newest(events: &[ProgressEvent]) -> Option<Run<'_>> {
        runs(events).pop()
    }

    /// Neither finished nor stopped by a marker: episodes can still join it.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.finished.is_none() && self.stopped.is_none()
    }

    /// Where the run stands for an entry with `status`. A finished run and one a marker
    /// stopped stay what they are whatever the status says; a run without either is running
    /// on an In progress entry, stopped on a Completed one, and `None` on any other status
    /// (a paused or dropped entry shows no run).
    #[must_use]
    pub fn phase(&self, status: &Status) -> Option<Phase> {
        if self.finished.is_some() {
            return Some(Phase::Finished);
        }
        if self.stopped.is_some() {
            return Some(Phase::Stopped);
        }
        match status.as_str() {
            Status::IN_PROGRESS => Some(Phase::Running),
            Status::COMPLETED => Some(Phase::Stopped),
            _ => None,
        }
    }

    /// Does the run cover every one of the `known` regular episodes? Never for an item that
    /// knows none.
    #[must_use]
    pub fn covers(&self, known: &[(u32, u32)]) -> bool {
        !known.is_empty() && known.iter().all(|ep| self.episodes.contains(ep))
    }
}

/// The status a logged event moves the entry to (`None`: it stays). `finished` completes;
/// a rewatch start reopens a Completed entry; the event that closes a running run
/// (`closes_run`: the episode that completes it, or a stop marker while a run is open) gives
/// Completed back; anything else moves Planned to In progress.
#[must_use]
pub fn status_after(status: &Status, progress: &Progress, closes_run: bool) -> Option<Status> {
    let now = status.as_str();
    match progress {
        Progress::Finished => Some(Status::completed()),
        Progress::Rewatch => (now == Status::COMPLETED).then(Status::in_progress),
        Progress::Undo { .. } => None,
        Progress::RewatchStop => (closes_run && now == Status::IN_PROGRESS).then(Status::completed),
        Progress::Episode { .. } if closes_run && now == Status::IN_PROGRESS => {
            Some(Status::completed())
        }
        _ => (now == Status::PLANNED).then(Status::in_progress),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::hlc::Hlc;
    use crate::ids::{EventId, ItemId, UserId};
    use crate::kind::ProgressModel;

    /// A person's entry and history, with the rules of the log applied as the app does.
    struct Show {
        status: Status,
        events: Vec<ProgressEvent>,
        known: Vec<(u32, u32)>,
    }

    impl Show {
        fn completed(episodes: u32) -> Self {
            let mut s = Self {
                status: Status::planned(),
                events: Vec::new(),
                known: (1..=episodes).map(|e| (1, e)).collect(),
            };
            for e in 1..=episodes {
                s.log(Progress::Episode {
                    season: 1,
                    episode: e,
                });
            }
            s.status = Status::completed();
            s
        }

        fn log(&mut self, progress: Progress) -> EventId {
            let n = u128::try_from(self.events.len()).unwrap() + 1;
            let at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
                + chrono::Duration::hours(i64::try_from(n).unwrap());
            let mut event = ProgressEvent {
                id: EventId(uuid::Uuid::from_u128(n)),
                user_id: UserId(uuid::Uuid::nil()),
                item_id: ItemId(uuid::Uuid::nil()),
                occurred_at: at,
                progress,
                client_op_id: None,
                recorded_at: at,
                status_change: None,
            };
            let open = Run::newest(&self.events).is_some_and(|r| r.is_open());
            self.events.push(event.clone());
            let closes = match event.progress {
                Progress::RewatchStop => open,
                _ => Run::newest(&self.events).is_some_and(|r| {
                    r.phase(&self.status) == Some(Phase::Running) && r.covers(&self.known)
                }),
            };
            self.events.pop();
            if let Some(to) =
                status_after(&self.status, &event.progress, closes).filter(|to| *to != self.status)
            {
                event.status_change = Some(StatusChange {
                    from: self.status.clone(),
                    to: to.clone(),
                    stamp: Hlc::new(u64::try_from(n).unwrap(), 0, 0),
                    from_stamp: None,
                });
                self.status = to;
            }
            let id = event.id;
            self.events.push(event);
            id
        }

        fn ep(&mut self, episode: u32) -> EventId {
            self.log(Progress::Episode { season: 1, episode })
        }

        /// Appends the undo event only. The status side of an undo is the app's
        /// (`App::undo_progress`, tested in `app/tests/rewatch.rs`); a test here sets the
        /// status the app would restore.
        fn undo(&mut self, id: EventId) {
            let n = u128::try_from(self.events.len()).unwrap() + 1;
            self.events.push(ProgressEvent {
                id: EventId(uuid::Uuid::from_u128(n)),
                progress: Progress::Undo { event_id: id },
                status_change: None,
                ..self.events[0].clone()
            });
        }

        fn phase(&self) -> Option<Phase> {
            Run::newest(&self.events).and_then(|r| r.phase(&self.status))
        }

        fn done(&self) -> usize {
            Run::newest(&self.events).map_or(0, |r| r.episodes.len())
        }
    }

    #[test]
    fn a_rewatch_start_is_progress_of_the_episodic_model_only() {
        assert!(
            Progress::Rewatch
                .validate_for(ProgressModel::Episodic)
                .is_ok()
        );
        for other in [
            ProgressModel::Single,
            ProgressModel::Paged,
            ProgressModel::Timed,
            ProgressModel::Sessions,
        ] {
            assert!(Progress::Rewatch.validate_for(other).is_err(), "{other:?}");
        }
        let json = serde_json::to_value(Progress::Rewatch).unwrap();
        assert_eq!(json, serde_json::json!({"type": "rewatch"}));
    }

    /// Start, mark, finish: In progress while it runs, Completed again with its last episode.
    #[test]
    fn a_run_starts_runs_and_finishes() {
        let mut show = Show::completed(3);
        assert_eq!(show.phase(), None);
        show.log(Progress::Rewatch);
        assert_eq!(show.status, Status::in_progress());
        assert_eq!((show.phase(), show.done()), (Some(Phase::Running), 0));
        show.ep(1);
        show.ep(2);
        // the same episode twice is one episode of the run
        show.ep(2);
        assert_eq!(show.status, Status::in_progress());
        assert_eq!((show.phase(), show.done()), (Some(Phase::Running), 2));
        show.ep(3);
        assert_eq!(show.status, Status::completed());
        assert_eq!((show.phase(), show.done()), (Some(Phase::Finished), 3));
        // one more view afterwards is not the run's and moves nothing
        show.ep(1);
        assert_eq!(show.status, Status::completed());
        assert_eq!(show.phase(), Some(Phase::Finished));
        assert_eq!(runs(&show.events).len(), 1);
    }

    /// Stop, then start again: a stopped run keeps its views, the next run counts from zero.
    #[test]
    fn a_run_stops_and_another_one_starts() {
        let mut show = Show::completed(3);
        show.log(Progress::Rewatch);
        show.ep(1);
        show.ep(2);
        // stopping is the status write, no event
        show.status = Status::completed();
        assert_eq!((show.phase(), show.done()), (Some(Phase::Stopped), 2));
        assert_eq!(show.events.len(), 6);
        show.log(Progress::Rewatch);
        assert_eq!(show.status, Status::in_progress());
        assert_eq!((show.phase(), show.done()), (Some(Phase::Running), 0));
        let all = runs(&show.events);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].episodes.len(), 2);
        assert_eq!(all[0].finished, None);
        // paused in the middle: the run does not show, and comes back with In progress
        show.ep(1);
        show.status = Status::new(Status::PAUSED);
        assert_eq!(show.phase(), None);
        show.ep(2);
        show.ep(3);
        assert_eq!(
            show.status,
            Status::new(Status::PAUSED),
            "no move while paused"
        );
        show.status = Status::in_progress();
        assert_eq!(show.phase(), Some(Phase::Running));
    }

    /// A start on an entry that is not Completed starts nothing; specials and views dated
    /// before the start are not the run's.
    #[test]
    fn only_a_start_on_a_completed_entry_is_a_run() {
        let mut show = Show::completed(2);
        show.status = Status::in_progress();
        show.log(Progress::Rewatch);
        assert_eq!(show.status, Status::in_progress());
        assert_eq!(runs(&show.events).len(), 0);
        // planned: a start is not what begins a first run
        let mut planned = Show {
            status: Status::planned(),
            events: Vec::new(),
            known: vec![(1, 1)],
        };
        planned.log(Progress::Rewatch);
        assert_eq!(planned.status, Status::planned());

        let mut show = Show::completed(2);
        show.log(Progress::Rewatch);
        show.log(Progress::Episode {
            season: 0,
            episode: 1,
        });
        assert_eq!(show.done(), 0);
        // an imported view of years ago, recorded now
        let mut old = show.events[0].clone();
        old.id = EventId(uuid::Uuid::from_u128(99));
        old.occurred_at = Utc.with_ymd_and_hms(2020, 5, 1, 0, 0, 0).unwrap();
        old.progress = Progress::Episode {
            season: 1,
            episode: 2,
        };
        show.events.push(old);
        assert_eq!(show.done(), 0);
        assert_eq!(show.phase(), Some(Phase::Running));
    }

    /// What the run derivation reads from a history with undo events: an undone start is no
    /// run, an undone finishing episode or stop marker leaves the run open. (That the entry
    /// gets its status back is the app's undo: `app/tests/rewatch.rs`.)
    #[test]
    fn undo_takes_a_start_a_finish_and_a_stop_back() {
        let mut show = Show::completed(2);
        let start = show.log(Progress::Rewatch);
        show.undo(start);
        assert_eq!(runs(&show.events).len(), 0);
        show.status = Status::completed();

        show.log(Progress::Rewatch);
        show.ep(1);
        let last = show.ep(2);
        assert_eq!(show.phase(), Some(Phase::Finished));
        show.undo(last);
        let all = runs(&show.events);
        assert_eq!(
            (all.len(), all[0].finished, all[0].episodes.len()),
            (1, None, 1)
        );
        show.status = Status::in_progress();
        assert_eq!((show.phase(), show.done()), (Some(Phase::Running), 1));

        let stop = show.log(Progress::RewatchStop);
        assert_eq!(show.phase(), Some(Phase::Stopped));
        show.undo(stop);
        assert_eq!(runs(&show.events)[0].stopped, None);
        show.status = Status::in_progress();
        assert_eq!((show.phase(), show.done()), (Some(Phase::Running), 1));
    }

    /// A stop is a marker (DEC-590): the run ends there. It does not come back when the
    /// status is set to In progress by hand later, and views logged after it are not its.
    #[test]
    fn a_stopped_run_does_not_come_back_with_in_progress() {
        let mut show = Show::completed(3);
        show.log(Progress::Rewatch);
        show.ep(1);
        show.ep(2);
        show.log(Progress::RewatchStop);
        assert_eq!(show.status, Status::completed());
        assert_eq!((show.phase(), show.done()), (Some(Phase::Stopped), 2));
        let stop = show.events.last().unwrap();
        assert_eq!(
            stop.status_change
                .as_ref()
                .map(|c| (c.from.as_str(), c.to.as_str())),
            Some(("in_progress", "completed"))
        );
        // a new season months later, In progress by hand: the old run stays stopped
        show.known.push((2, 1));
        show.status = Status::in_progress();
        assert_eq!((show.phase(), show.done()), (Some(Phase::Stopped), 2));
        // a second stop stops nothing and moves nothing
        show.log(Progress::RewatchStop);
        assert_eq!(show.events.last().unwrap().status_change, None);
        assert_eq!(show.status, Status::in_progress());
    }

    #[test]
    fn views_after_a_stop_do_not_join_the_run() {
        let mut show = Show::completed(3);
        show.log(Progress::Rewatch);
        show.ep(1);
        show.log(Progress::RewatchStop);
        // "Again" on the other episodes, on the Completed entry and on one set In progress
        show.ep(2);
        show.status = Status::in_progress();
        show.ep(3);
        assert_eq!(show.status, Status::in_progress(), "no finishing move");
        let all = runs(&show.events);
        assert_eq!(all.len(), 1);
        assert_eq!(
            all[0].episodes.iter().copied().collect::<Vec<_>>(),
            [(1, 1)]
        );
        assert_eq!(all[0].finished, None);
        assert!(!all[0].covers(&show.known), "a stopped run is never a pass");
    }

    /// Two stopped runs in a row stay two, also when the same events are logged again in
    /// their order on a fresh entry (what an import of an export does).
    #[test]
    fn two_stopped_runs_stay_apart() {
        let story = [
            Progress::Rewatch,
            Progress::Episode {
                season: 1,
                episode: 1,
            },
            Progress::RewatchStop,
            Progress::Rewatch,
            Progress::Episode {
                season: 1,
                episode: 2,
            },
            Progress::Episode {
                season: 1,
                episode: 3,
            },
            Progress::RewatchStop,
        ];
        for _replay in 0..2 {
            let mut show = Show::completed(3);
            for p in &story {
                show.log(p.clone());
            }
            assert_eq!(show.status, Status::completed());
            let all = runs(&show.events);
            let episodes: Vec<Vec<(u32, u32)>> = all
                .iter()
                .map(|r| r.episodes.iter().copied().collect())
                .collect();
            assert_eq!(episodes, [vec![(1, 1)], vec![(1, 2), (1, 3)]]);
            assert_eq!(all.iter().filter(|r| r.stopped.is_some()).count(), 2);
        }
    }

    /// The state DEC-589 lists as a limit: a run covers every known episode and no event
    /// finished it (its last episodes were logged while the entry was paused). The run says
    /// so with `covers`; the app treats it as over.
    #[test]
    fn a_run_can_cover_every_episode_without_a_finishing_event() {
        let mut show = Show::completed(2);
        show.log(Progress::Rewatch);
        show.ep(1);
        show.status = Status::new(Status::PAUSED);
        show.ep(2);
        show.status = Status::in_progress();
        let run = Run::newest(&show.events).unwrap();
        assert_eq!((run.finished, run.stopped), (None, None));
        assert!(run.covers(&show.known));
        assert!(!run.covers(&[(1, 1), (1, 2), (2, 1)]));
        assert_eq!(run.phase(&show.status), Some(Phase::Running));
    }

    /// A movie rewatch never leaves Completed, and the first-run rules stay.
    #[test]
    fn the_other_moves_are_as_before() {
        let completed = Status::completed();
        assert_eq!(
            status_after(&completed, &Progress::Finished, false),
            Some(Status::completed())
        );
        assert_eq!(
            status_after(&Status::planned(), &Progress::Finished, false),
            Some(Status::completed())
        );
        let ep = Progress::Episode {
            season: 1,
            episode: 1,
        };
        assert_eq!(
            status_after(&Status::planned(), &ep, false),
            Some(Status::in_progress())
        );
        assert_eq!(status_after(&completed, &ep, false), None);
        assert_eq!(status_after(&Status::in_progress(), &ep, false), None);
        assert_eq!(status_after(&Status::new(Status::PAUSED), &ep, true), None);
        assert_eq!(
            status_after(&Status::in_progress(), &ep, true),
            Some(Status::completed())
        );
        // an item that knows no episodes is never covered
        let mut show = Show::completed(1);
        show.known.clear();
        show.log(Progress::Rewatch);
        show.ep(1);
        assert_eq!(show.phase(), Some(Phase::Running));
    }
}
