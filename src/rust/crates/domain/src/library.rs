//! A user's library entry for one catalog item: status, rating and notes are LWW fields.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::hlc::{Hlc, Lww};
use crate::ids::{EntryId, ItemId, UserId};
use crate::kind::{KindId, Status};
use crate::progress::{Progress, ProgressEvent};

/// Rating on a 0..=100 scale; the UI maps it to stars or 1..10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Rating(u8);

impl Rating {
    pub const MAX: u8 = 100;

    /// # Errors
    /// If `v` is above [`Rating::MAX`].
    pub fn new(v: u8) -> Result<Self, DomainError> {
        if v <= Self::MAX {
            Ok(Self(v))
        } else {
            Err(DomainError::invalid(
                "rating",
                format!("{v} is above {}", Self::MAX),
            ))
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Rating {
    type Error = DomainError;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}

impl From<Rating> for u8 {
    fn from(r: Rating) -> Self {
        r.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: EntryId,
    pub user_id: UserId,
    pub item_id: ItemId,
    pub kind: KindId,
    pub status: Lww<Status>,
    pub rating: Lww<Option<Rating>>,
    pub notes: Lww<Option<String>>,
    pub added_at: Timestamp,
    /// The completion no event made, while the entry has one (LB-14). The status says
    /// where the entry stands; this says that, and when, it was finished by a status.
    #[serde(default)]
    pub completed: Option<HandCompletion>,
}

/// The completion of an entry that no event made: a status written by a person (the status
/// select, "Finished" on a book, a bulk change) or by an import. It lies on the newest step
/// that was logged by then; without one it has no day ("undated": it counts in the all-time
/// totals and in no month, year or goal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandCompletion {
    At(Timestamp),
    Undated,
}

impl HandCompletion {
    /// The completion a status write to Completed makes, given the entry's effective events
    /// (`live`): none when one of them completed the entry (that event is the completion;
    /// a stop marker completes nothing), else on the newest of them, else undated.
    #[must_use]
    pub fn from_history(live: &[&ProgressEvent]) -> Option<Self> {
        let completing = |e: &ProgressEvent| {
            !matches!(e.progress, Progress::RewatchStop)
                && e.status_change
                    .as_ref()
                    .is_some_and(|c| c.to.as_str() == Status::COMPLETED)
        };
        if live.iter().any(|e| completing(e)) {
            return None;
        }
        let newest = live.iter().map(|e| e.occurred_at).max();
        Some(newest.map_or(Self::Undated, Self::At))
    }

    /// The day, when it has one.
    #[must_use]
    pub const fn at(self) -> Option<Timestamp> {
        match self {
            Self::At(t) => Some(t),
            Self::Undated => None,
        }
    }
}

impl LibraryEntry {
    /// Does a status write to `to` need the entry's history ([`Self::set_status_by_hand`])?
    /// Only a write to Completed of an entry whose completion has no day yet.
    #[must_use]
    pub fn dates_completion(&self, to: &Status) -> bool {
        to.as_str() == Status::COMPLETED && !matches!(self.completed, Some(HandCompletion::At(_)))
    }

    /// Write a status a person or an import chose (last writer wins; `false`: an older
    /// stamp, nothing changed). The completion follows: anything but Completed takes it
    /// away; Completed keeps a day it has, whoever writes it again and however often, and
    /// else takes `dated` ([`HandCompletion::from_history`] of the entry's events: an
    /// undated one gets its day once a step exists, and an entry an event completed gets
    /// none). A status an event moves is written with `status.set` and leaves the
    /// completion alone: the start of a rewatch run keeps it, a stop gives Completed back
    /// with it (DEC-590).
    pub fn set_status_by_hand(
        &mut self,
        to: Status,
        stamp: Hlc,
        dated: Option<HandCompletion>,
    ) -> bool {
        let completes = to.as_str() == Status::COMPLETED;
        if !self.status.set(to, stamp) {
            return false;
        }
        self.completed = match (completes, self.completed, dated) {
            (false, ..) => None,
            (true, Some(HandCompletion::At(t)), _) => Some(HandCompletion::At(t)),
            (true, Some(HandCompletion::Undated), None) => Some(HandCompletion::Undated),
            (true, _, dated) => dated,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;
    use crate::ids::EventId;
    use crate::progress::StatusChange;

    fn at(day: u32) -> Timestamp {
        Utc.with_ymd_and_hms(2026, 1, day, 12, 0, 0).unwrap()
    }

    fn event(n: u128, day: u32, progress: Progress, to: Option<Status>) -> ProgressEvent {
        ProgressEvent {
            id: EventId(uuid::Uuid::from_u128(n)),
            user_id: UserId(uuid::Uuid::nil()),
            item_id: ItemId(uuid::Uuid::nil()),
            occurred_at: at(day),
            progress,
            client_op_id: None,
            recorded_at: at(day),
            status_change: to.map(|to| StatusChange {
                from: Status::in_progress(),
                to,
                stamp: Hlc::new(1, 0, 0),
                from_stamp: None,
            }),
        }
    }

    fn entry() -> LibraryEntry {
        LibraryEntry {
            id: EntryId(uuid::Uuid::nil()),
            user_id: UserId(uuid::Uuid::nil()),
            item_id: ItemId(uuid::Uuid::nil()),
            kind: KindId::new("book").unwrap(),
            status: Lww::new(Status::planned(), Hlc::new(0, 0, 0)),
            rating: Lww::new(None, Hlc::new(0, 0, 0)),
            notes: Lww::new(None, Hlc::new(0, 0, 0)),
            added_at: at(1),
            completed: None,
        }
    }

    #[test]
    fn a_completion_by_hand_lies_on_the_newest_step_or_nowhere() {
        let ep = |e| Progress::Episode {
            season: 1,
            episode: e,
        };
        assert_eq!(
            HandCompletion::from_history(&[]),
            Some(HandCompletion::Undated)
        );
        let (a, b) = (event(1, 9, ep(2), None), event(2, 4, ep(1), None));
        assert_eq!(
            HandCompletion::from_history(&[&a, &b]),
            Some(HandCompletion::At(at(9)))
        );
        // an event completed the entry: that is the completion, a status adds none
        let done = event(3, 10, Progress::Finished, Some(Status::completed()));
        assert_eq!(HandCompletion::from_history(&[&a, &done]), None);
        // a stop marker gives Completed back and completes nothing
        let stop = event(4, 11, Progress::RewatchStop, Some(Status::completed()));
        assert_eq!(
            HandCompletion::from_history(&[&a, &stop]),
            Some(HandCompletion::At(at(11)))
        );
    }

    #[test]
    fn a_status_by_hand_keeps_the_day_of_a_completion() {
        let day = Some(HandCompletion::At(at(3)));
        let later = Some(HandCompletion::At(at(20)));
        let mut e = entry();
        assert!(e.dates_completion(&Status::completed()));
        assert!(!e.dates_completion(&Status::in_progress()));
        assert!(e.set_status_by_hand(Status::completed(), Hlc::new(1, 0, 0), day));
        assert_eq!(e.completed, day);
        // written again: the day stays, and the history is not even asked for
        assert!(!e.dates_completion(&Status::completed()));
        assert!(e.set_status_by_hand(Status::completed(), Hlc::new(2, 0, 0), later));
        assert_eq!(e.completed, day);
        // an older write changes nothing
        assert!(!e.set_status_by_hand(Status::planned(), Hlc::new(1, 0, 0), None));
        assert_eq!(e.completed, day);
        // an event moves the status (the start of a rewatch run): the completion stays
        assert!(e.status.set(Status::in_progress(), Hlc::new(3, 0, 0)));
        assert!(e.set_status_by_hand(Status::completed(), Hlc::new(4, 0, 0), later));
        assert_eq!(e.completed, day);
        // moved away by hand it is gone, and Completed again is a new one
        assert!(e.set_status_by_hand(Status::new(Status::PAUSED), Hlc::new(5, 0, 0), None));
        assert_eq!(e.completed, None);
        assert!(e.set_status_by_hand(Status::completed(), Hlc::new(6, 0, 0), later));
        assert_eq!(e.completed, later);
    }

    #[test]
    fn an_undated_completion_gets_a_day_once_and_keeps_it() {
        let undated = Some(HandCompletion::Undated);
        let day = Some(HandCompletion::At(at(3)));
        let mut e = entry();
        e.set_status_by_hand(Status::completed(), Hlc::new(1, 0, 0), undated);
        assert_eq!(e.completed, undated);
        assert!(e.dates_completion(&Status::completed()));
        // still no step, or an event completed a later run meanwhile: it stays undated
        e.set_status_by_hand(Status::completed(), Hlc::new(2, 0, 0), undated);
        e.set_status_by_hand(Status::completed(), Hlc::new(3, 0, 0), None);
        assert_eq!(e.completed, undated);
        e.set_status_by_hand(Status::completed(), Hlc::new(4, 0, 0), day);
        assert_eq!(e.completed, day);
        // an entry an event completed gets no second completion from a status write
        let mut film = entry();
        film.status.set(Status::completed(), Hlc::new(1, 0, 0));
        film.set_status_by_hand(Status::completed(), Hlc::new(2, 0, 0), None);
        assert_eq!(film.completed, None);
    }
}
