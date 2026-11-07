//! Append-only progress events (D-12). They never conflict: sync only appends.
//!
//! An event is never changed or deleted. Undo appends a compensating [`Progress::Undo`]
//! event that names the event it takes back; [`effective`] is the history without both.

use std::collections::{BTreeSet, HashSet};

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::hlc::Hlc;
use crate::ids::{EventId, ItemId, SessionId, UserId};
use crate::kind::{ProgressModel, Status};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Progress {
    /// `Single`: watched / finished once more.
    Finished,
    /// `Episodic`: an episode was watched / heard.
    Episode { season: u32, episode: u32 },
    /// `Paged`: position reached (at least one field set).
    Paged {
        page: Option<u32>,
        chapter: Option<u32>,
        volume: Option<u32>,
    },
    /// `Timed`: position in seconds.
    Position { seconds: u64 },
    /// `Sessions`: a session was logged.
    Session { session_id: SessionId },
    /// Compensating event: takes back `event_id` (any model). Written only by the undo
    /// use-case, never accepted from clients as progress.
    Undo { event_id: EventId },
}

impl Progress {
    /// The progress model this progress belongs to; `None` for [`Progress::Undo`].
    #[must_use]
    pub const fn model(&self) -> Option<ProgressModel> {
        match self {
            Self::Finished => Some(ProgressModel::Single),
            Self::Episode { .. } => Some(ProgressModel::Episodic),
            Self::Paged { .. } => Some(ProgressModel::Paged),
            Self::Position { .. } => Some(ProgressModel::Timed),
            Self::Session { .. } => Some(ProgressModel::Sessions),
            Self::Undo { .. } => None,
        }
    }

    /// Check that this progress fits `model` and is well-formed.
    ///
    /// # Errors
    /// On a model mismatch, an empty `Paged` position or an `Undo`.
    pub fn validate_for(&self, model: ProgressModel) -> Result<(), DomainError> {
        let Some(own) = self.model() else {
            return Err(DomainError::invalid(
                "progress",
                "undo is not progress; use the undo operation",
            ));
        };
        if own != model {
            return Err(DomainError::ProgressModelMismatch {
                model: model.as_str().into(),
                got: own.as_str().into(),
            });
        }
        if let Self::Paged {
            page: None,
            chapter: None,
            volume: None,
        } = self
        {
            return Err(DomainError::invalid(
                "progress",
                "paged progress needs a page, chapter or volume",
            ));
        }
        Ok(())
    }
}

/// The status move an event caused (Planned -> In progress, -> Completed), with the stamp
/// it was written with. Undo restores `from` only while the entry still carries exactly
/// this write, so a later status chosen by hand is never overwritten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusChange {
    pub from: Status,
    pub to: Status,
    pub stamp: Hlc,
}

/// One immutable progress event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub id: EventId,
    pub user_id: UserId,
    pub item_id: ItemId,
    /// When it happened (user supplied, may be in the past).
    pub occurred_at: Timestamp,
    pub progress: Progress,
    /// Idempotency key from the client, if any (D-12).
    pub client_op_id: Option<uuid::Uuid>,
    /// When the server stored it (undo is only offered for recent events).
    #[serde(default)]
    pub recorded_at: Timestamp,
    /// The status move this event caused, if any.
    #[serde(default)]
    pub status_change: Option<StatusChange>,
}

/// The history without undo events and the events they take back, in input order.
#[must_use]
pub fn effective(events: &[ProgressEvent]) -> Vec<&ProgressEvent> {
    let undone: HashSet<EventId> = events
        .iter()
        .filter_map(|e| match e.progress {
            Progress::Undo { event_id } => Some(event_id),
            _ => None,
        })
        .collect();
    events
        .iter()
        .filter(|e| !matches!(e.progress, Progress::Undo { .. }) && !undone.contains(&e.id))
        .collect()
}

/// Where a user stands with one item, derived from its effective history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressSummary {
    /// Effective events.
    pub events: u32,
    /// `Finished` events (watched, read, played through; rewatches count).
    pub finished: u32,
    /// Distinct episodes watched, `(season, episode)`.
    pub episodes: BTreeSet<(u32, u32)>,
    /// Episode events including rewatches.
    pub episode_views: u32,
    /// Latest paged position (by `occurred_at`).
    pub page: Option<u32>,
    pub chapter: Option<u32>,
    pub volume: Option<u32>,
    /// Pages read: the sum of forward page moves.
    pub pages_read: u64,
    /// Latest timed position in seconds.
    pub position: Option<u64>,
    /// Seconds listened: the sum of forward position moves.
    pub seconds_listened: u64,
    /// Logged sessions.
    pub sessions: u32,
    pub first_at: Option<Timestamp>,
    pub last_at: Option<Timestamp>,
}

impl ProgressSummary {
    /// Summarise a history (undo is applied here, the input may contain it).
    #[must_use]
    pub fn of(events: &[ProgressEvent]) -> Self {
        let mut live = effective(events);
        live.sort_by_key(|e| (e.occurred_at, e.id));
        let mut s = Self::default();
        for e in live {
            s.add(e);
        }
        s
    }

    fn add(&mut self, e: &ProgressEvent) {
        self.events += 1;
        self.first_at = Some(
            self.first_at
                .map_or(e.occurred_at, |t| t.min(e.occurred_at)),
        );
        self.last_at = Some(self.last_at.map_or(e.occurred_at, |t| t.max(e.occurred_at)));
        match e.progress {
            Progress::Finished => self.finished += 1,
            Progress::Episode { season, episode } => {
                self.episode_views += 1;
                self.episodes.insert((season, episode));
            }
            Progress::Paged {
                page,
                chapter,
                volume,
            } => {
                if let Some(p) = page {
                    let before = self.page.unwrap_or(0);
                    self.pages_read += u64::from(p.saturating_sub(before));
                    self.page = Some(p);
                }
                self.chapter = chapter.or(self.chapter);
                self.volume = volume.or(self.volume);
            }
            Progress::Position { seconds } => {
                let before = self.position.unwrap_or(0);
                self.seconds_listened += seconds.saturating_sub(before);
                self.position = Some(seconds);
            }
            Progress::Session { .. } => self.sessions += 1,
            Progress::Undo { .. } => {}
        }
    }

    /// The highest episode watched (specials, season 0, sort first).
    #[must_use]
    pub fn last_episode(&self) -> Option<(u32, u32)> {
        self.episodes.iter().next_back().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn progress_must_match_model() {
        assert!(
            Progress::Finished
                .validate_for(ProgressModel::Single)
                .is_ok()
        );
        assert!(
            Progress::Episode {
                season: 1,
                episode: 2
            }
            .validate_for(ProgressModel::Single)
            .is_err()
        );
        assert!(
            Progress::Position { seconds: 5 }
                .validate_for(ProgressModel::Timed)
                .is_ok()
        );
        let undo = Progress::Undo {
            event_id: EventId(uuid::Uuid::nil()),
        };
        assert!(undo.validate_for(ProgressModel::Single).is_err());
    }

    #[test]
    fn empty_paged_progress_is_rejected() {
        let p = Progress::Paged {
            page: None,
            chapter: None,
            volume: None,
        };
        assert!(p.validate_for(ProgressModel::Paged).is_err());
    }

    fn ev(n: u128, day: u32, progress: Progress) -> ProgressEvent {
        ProgressEvent {
            id: EventId(uuid::Uuid::from_u128(n)),
            user_id: UserId(uuid::Uuid::nil()),
            item_id: ItemId(uuid::Uuid::nil()),
            occurred_at: Utc.with_ymd_and_hms(2026, 1, day, 0, 0, 0).unwrap(),
            progress,
            client_op_id: None,
            recorded_at: Utc.with_ymd_and_hms(2026, 1, day, 0, 0, 0).unwrap(),
            status_change: None,
        }
    }

    fn page(p: u32) -> Progress {
        Progress::Paged {
            page: Some(p),
            chapter: None,
            volume: None,
        }
    }

    #[test]
    fn undo_removes_the_event_and_itself_from_the_effective_history() {
        let events = vec![
            ev(1, 1, page(10)),
            ev(2, 2, page(40)),
            ev(
                3,
                3,
                Progress::Undo {
                    event_id: EventId(uuid::Uuid::from_u128(2)),
                },
            ),
        ];
        let ids: Vec<u128> = effective(&events)
            .iter()
            .map(|e| e.id.0.as_u128())
            .collect();
        assert_eq!(ids, vec![1]);
        let s = ProgressSummary::of(&events);
        assert_eq!((s.events, s.page, s.pages_read), (1, Some(10), 10));
    }

    #[test]
    fn summary_counts_episodes_pages_positions_and_sessions() {
        let ep = |s, e| Progress::Episode {
            season: s,
            episode: e,
        };
        let events = vec![
            ev(1, 1, ep(1, 1)),
            ev(2, 2, ep(1, 2)),
            ev(3, 3, ep(1, 1)),
            ev(4, 4, ep(0, 1)),
        ];
        let s = ProgressSummary::of(&events);
        assert_eq!((s.episodes.len(), s.episode_views), (3, 4));
        assert_eq!(s.last_episode(), Some((1, 2)));
        // pages: a re-read from page 5 counts the forward moves only
        let s = ProgressSummary::of(&[ev(1, 1, page(100)), ev(2, 2, page(5)), ev(3, 3, page(25))]);
        assert_eq!((s.page, s.pages_read), (Some(25), 120));
        let s = ProgressSummary::of(&[
            ev(1, 1, Progress::Position { seconds: 600 }),
            ev(2, 2, Progress::Position { seconds: 900 }),
        ]);
        assert_eq!((s.position, s.seconds_listened), (Some(900), 900));
        let sid = SessionId(uuid::Uuid::nil());
        let s = ProgressSummary::of(&[
            ev(1, 1, Progress::Session { session_id: sid }),
            ev(2, 5, Progress::Session { session_id: sid }),
        ]);
        assert_eq!(s.sessions, 2);
        assert_eq!(
            s.last_at,
            Some(Utc.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap())
        );
    }
}
