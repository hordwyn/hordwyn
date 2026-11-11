//! What a rewatch run asks of a log (DEC-587, DEC-590, DEC-591): which marker is refused,
//! and when a log happened.
//!
//! A run is read from the history in the order of `occurred_at`, so a date decides what is
//! the run's. The rules keep a log from falling out of a run without the person seeing it:
//!
//! - a start that starts nothing (the entry is not Completed) and a stop without an open run
//!   are refused, they leave no row;
//! - a start dated before the newest view is refused (the views after it would be its run);
//! - while a run is running, an episode dated before its start is refused (it would be
//!   logged and not counted); an import is the exception, its views are old on purpose;
//! - a log without a date never sorts before what it follows (a start that came from a
//!   queue whose clock runs ahead).

use domain::DomainError;
use domain::Timestamp;
use domain::catalog::CatalogItem;
use domain::kind::Status;
use domain::progress::{Progress, ProgressEvent, effective};
use domain::rewatch::Run;

use crate::{AppError, AppResult};

/// Who logs: a person now (the API, the offline queue), or an import replaying a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Live,
    Import,
}

fn too_early(reason: &str) -> AppError {
    DomainError::invalid("occurred_at", reason).into()
}

/// A log for an item that is not in the library: a marker is refused (there is no run
/// without an entry, and a marker never adds the item); any other log adds it.
///
/// # Errors
/// `Conflict` for a marker.
pub(crate) fn without_entry(progress: &Progress) -> AppResult<()> {
    if matches!(progress, Progress::Rewatch | Progress::RewatchStop) {
        return Err(AppError::Conflict(
            "a rewatch run needs a library entry".into(),
        ));
    }
    Ok(())
}

/// `sent`, or now, but after `floor` when the clock that dated `floor` runs ahead.
fn not_before(sent: Option<Timestamp>, now: Timestamp, floor: Option<Timestamp>) -> Timestamp {
    sent.unwrap_or_else(|| floor.map_or(now, |f| now.max(f + chrono::Duration::milliseconds(1))))
}

/// When the log of `progress` happened (`sent`: the client's date, `None`: now), on an entry
/// with `status` whose stored events are `history` (it may be empty when the entry is not In
/// progress and `progress` is no marker: no run is running then).
///
/// # Errors
/// `Conflict` for a marker that would start or stop nothing; `invalid` (`occurred_at`) for
/// a date the run cannot take.
pub(crate) fn occurred_at(
    item: &CatalogItem,
    status: &Status,
    history: &[ProgressEvent],
    progress: &Progress,
    sent: Option<Timestamp>,
    now: Timestamp,
    source: Source,
) -> AppResult<Timestamp> {
    let newest = || effective(history).iter().map(|e| e.occurred_at).max();
    let run = Run::newest(history);
    match progress {
        Progress::Rewatch => {
            if status.as_str() != Status::COMPLETED {
                return Err(AppError::Conflict(
                    "a rewatch run starts on a completed entry".into(),
                ));
            }
            let newest = newest();
            if sent.zip(newest).is_some_and(|(t, n)| t < n) {
                return Err(too_early(
                    "a rewatch run cannot start before the newest view",
                ));
            }
            Ok(not_before(sent, now, newest))
        }
        Progress::RewatchStop => {
            let Some(run) = run.filter(Run::is_open) else {
                return Err(AppError::Conflict("no rewatch run is open".into()));
            };
            if sent.is_some_and(|t| t < run.start.occurred_at) {
                return Err(too_early("lies before the start of the rewatch run"));
            }
            Ok(not_before(sent, now, newest()))
        }
        Progress::Episode { season, .. } if *season > 0 => {
            let Some(run) = run.filter(|r| crate::next_step::running(r, status, item)) else {
                return Ok(sent.unwrap_or(now));
            };
            let start = run.start.occurred_at;
            if source == Source::Live && sent.is_some_and(|t| t < start) {
                return Err(too_early("lies before the start of the rewatch run"));
            }
            Ok(not_before(sent, now, Some(start)))
        }
        _ => Ok(sent.unwrap_or(now)),
    }
}
