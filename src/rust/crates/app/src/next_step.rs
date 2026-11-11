//! The computed next step of a library entry (design.md 3.4): what the primary button on a
//! card, on Home and on the item page does.

use domain::Timestamp;
use domain::catalog::CatalogItem;
use domain::ids::EventId;
use domain::kind::{KindDescriptor, ProgressModel, Status};
use domain::library::LibraryEntry;
use domain::progress::{Progress, ProgressEvent, ProgressSummary, effective};
use domain::rewatch::{Phase, Run, runs};
use serde_json::Value;

/// What logging progress means next for an entry. Labels are the UI's business (Fluent keys
/// `next-step-*`); this is data only.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum NextStep {
    /// `Single`: mark watched / read / played; `again` once a finish is logged. A
    /// completion set by hand is not one: the first logged finish is that same pass (it gives
    /// it a day), so every step that says "again" raises [`times`] (OT2A-1).
    Finish { again: bool },
    /// `Episodic`: the next unwatched episode.
    Episode { season: u32, episode: u32 },
    /// `Episodic`: every known episode is watched ("up to date").
    CaughtUp,
    /// `Paged`: update the page (or chapter); current position and totals when known.
    Page {
        page: Option<u32>,
        chapter: Option<u32>,
        pages: Option<u32>,
        chapters: Option<u32>,
    },
    /// `Timed`: update the position; current and total seconds when known.
    Position {
        seconds: Option<u64>,
        total: Option<u64>,
    },
    /// `Sessions`: log a play or session; `plays` logged so far.
    Session { plays: u32 },
}

fn attr_u64(item: &CatalogItem, name: &str) -> Option<u64> {
    item.attributes.get(name).and_then(Value::as_u64)
}

fn attr_u32(item: &CatalogItem, name: &str) -> Option<u32> {
    attr_u64(item, name).and_then(|v| u32::try_from(v).ok())
}

/// Known regular seasons `(number, episode count)`, sorted, specials (season 0) left out.
/// Kinds without a season list but with an `episode_count` have one season of that size.
fn seasons(item: &CatalogItem) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = item
        .attributes
        .get("seasons")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    let n = u32::try_from(s.get("number")?.as_u64()?).ok()?;
                    let c = u32::try_from(s.get("episode_count")?.as_u64()?).ok()?;
                    Some((n, c))
                })
                .filter(|(n, c)| *n > 0 && *c > 0)
                .collect()
        })
        .unwrap_or_default();
    out.sort_unstable();
    if out.is_empty()
        && let Some(c) = attr_u32(item, "episode_count").filter(|c| *c > 0)
    {
        out.push((1, c));
    }
    out
}

fn next_episode(item: &CatalogItem, summary: &ProgressSummary) -> NextStep {
    let known = seasons(item);
    let last = summary.episodes.iter().rev().find(|(s, _)| *s > 0).copied();
    let Some((season, episode)) = last else {
        let first = known.first().map_or(1, |(n, _)| *n);
        return NextStep::Episode {
            season: first,
            episode: 1,
        };
    };
    match known.iter().find(|(n, _)| *n == season) {
        Some((_, count)) if episode >= *count => match known.iter().find(|(n, _)| *n > season) {
            Some((next, _)) => NextStep::Episode {
                season: *next,
                episode: 1,
            },
            None => NextStep::CaughtUp,
        },
        _ => NextStep::Episode {
            season,
            episode: episode + 1,
        },
    }
}

/// The next step of an entry (an item of `kind`) given its progress history (undo events
/// included; they are applied here). The history decides it, not the status (OT2A-1), with
/// one exception: while a rewatch run is running (the entry is In progress, DEC-587) the
/// step counts within the run.
#[must_use]
pub fn next_step(
    entry: &LibraryEntry,
    kind: &KindDescriptor,
    item: &CatalogItem,
    progress: &[ProgressEvent],
) -> NextStep {
    let summary = ProgressSummary::of(progress);
    step_in(&entry.status.value, kind, item, progress, &summary)
}

/// [`next_step`] for a caller that holds the summary already.
pub(crate) fn step_in(
    status: &Status,
    kind: &KindDescriptor,
    item: &CatalogItem,
    progress: &[ProgressEvent],
    summary: &ProgressSummary,
) -> NextStep {
    if kind.progress_model == ProgressModel::Episodic
        && let Some(run) = Run::newest(progress)
        && running(&run, status, item)
    {
        return next_in_run(item, &run);
    }
    step_for(kind, item, summary)
}

/// Every known regular episode `(season, episode)`, in order.
pub(crate) fn known_episodes(item: &CatalogItem) -> Vec<(u32, u32)> {
    seasons(item)
        .into_iter()
        .flat_map(|(season, count)| (1..=count).map(move |episode| (season, episode)))
        .collect()
}

/// Is the run over: its last episode gave Completed back, or (two logs at once, a status
/// set by hand in between) it covers every known episode without an event saying so.
fn over(run: &Run<'_>, item: &CatalogItem) -> bool {
    run.finished.is_some() || run.covers(&known_episodes(item))
}

pub(crate) fn running(run: &Run<'_>, status: &Status, item: &CatalogItem) -> bool {
    run.phase(status) == Some(Phase::Running) && !over(run, item)
}

/// The next episode of a running run: the one after the highest it has, as in a first run;
/// when that was the last known one, the first episode the run skipped.
fn next_in_run(item: &CatalogItem, run: &Run<'_>) -> NextStep {
    let within = ProgressSummary {
        episodes: run.episodes.clone(),
        ..ProgressSummary::default()
    };
    match next_episode(item, &within) {
        NextStep::CaughtUp => known_episodes(item)
            .into_iter()
            .find(|ep| !run.episodes.contains(ep))
            .map_or(NextStep::CaughtUp, |(season, episode)| NextStep::Episode {
                season,
                episode,
            }),
        step => step,
    }
}

/// The rewatch run of an entry as the pages show it (DEC-537 model A, DEC-587): running
/// ("Rewatch: 5 of 24 episodes") or stopped with something watched ("rewatch stopped after 5
/// of 24 episodes"). A finished run is not shown; [`times`] counts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewatch {
    /// The event that started the run; the history after it is the run's.
    pub start: EventId,
    /// Distinct regular episodes watched in the run.
    pub done: u32,
    /// Regular episodes the item knows.
    pub total: Option<u32>,
    /// The entry is In progress; `false`: stopped (Completed again).
    pub running: bool,
}

/// The run to show for an entry with `status`; `None` for a kind without episodes, without
/// a run, for a finished run, a stopped one without an episode, a stopped one once the
/// entry is no longer Completed (a stop marker ended it for good, DEC-590), and while the
/// entry is neither In progress nor Completed.
#[must_use]
pub fn rewatch(
    status: &Status,
    kind: &KindDescriptor,
    item: &CatalogItem,
    progress: &[ProgressEvent],
) -> Option<Rewatch> {
    if kind.progress_model != ProgressModel::Episodic {
        return None;
    }
    let run = Run::newest(progress)?;
    if over(&run, item) {
        return None;
    }
    let running = match run.phase(status)? {
        Phase::Running => true,
        Phase::Stopped if !run.episodes.is_empty() && status.as_str() == Status::COMPLETED => false,
        Phase::Stopped | Phase::Finished => return None,
    };
    Some(Rewatch {
        start: run.start.id,
        done: u32::try_from(run.episodes.len()).unwrap_or(u32::MAX),
        total: episode_total(item),
        running,
    })
}

/// Does logging `event` complete the rewatch run of an entry that is In progress (`history`
/// is what is stored, without `event`)? Then the event gives Completed back.
#[must_use]
pub(crate) fn completes_run(
    item: &CatalogItem,
    history: &[ProgressEvent],
    event: &ProgressEvent,
) -> bool {
    if Run::newest(history).is_none_or(|run| over(&run, item)) {
        return false;
    }
    let mut with: Vec<ProgressEvent> = history.to_vec();
    with.push(event.clone());
    Run::newest(&with).is_some_and(|run| over(&run, item))
}

/// The step of an item that is not in the viewer's library (OT-4): what its first log would
/// be, given any history that outlived a removed entry.
#[must_use]
pub fn first_step(
    kind: &KindDescriptor,
    item: &CatalogItem,
    progress: &[ProgressEvent],
) -> NextStep {
    step_for(kind, item, &ProgressSummary::of(progress))
}

/// How many times the person went through the item (OT-7, "Watched 3 times"; never "once
/// plus N rewatches"): finished events for the single, paged and timed models, plays for
/// sessions, and for an episodic item the whole passes over its known regular episodes (the
/// least often watched one decides; none while an episode is open or none is known). A
/// `completed` entry counts once at least: a status set by hand is the person's word, and
/// the first finish logged after it is that same pass, not a second one (OT2A-1).
#[must_use]
pub fn times(
    completed: bool,
    kind: &KindDescriptor,
    item: &CatalogItem,
    progress: &[ProgressEvent],
) -> u32 {
    let counted = match kind.progress_model {
        ProgressModel::Sessions => return ProgressSummary::of(progress).sessions,
        ProgressModel::Episodic => passes(item, progress).max(run_passes(item, progress)),
        ProgressModel::Single | ProgressModel::Paged | ProgressModel::Timed => {
            ProgressSummary::of(progress).finished
        }
    };
    counted.max(u32::from(completed))
}

/// What the rewatch runs prove (DEC-587): a run starts on a Completed entry, so the item was
/// gone through once before the first one, and every run that is over is one more time. The
/// views alone do not say that for a show completed by hand with part of it logged.
fn run_passes(item: &CatalogItem, progress: &[ProgressEvent]) -> u32 {
    let all = runs(progress);
    if all.is_empty() {
        return 0;
    }
    let done = all.iter().filter(|r| over(r, item)).count();
    u32::try_from(done).unwrap_or(u32::MAX).saturating_add(1)
}

/// The day of the pass that [`times`] names (review OT2A-7: not the newest view): the last
/// finish for the single, paged and timed models, the last play for sessions; for an
/// episodic item the day its last whole pass was done (the day the last of its episodes got
/// that view), or the day its last rewatch run finished when that is later. `None` when no
/// pass has a day (a completion set by hand).
#[must_use]
pub fn pass_at(
    kind: &KindDescriptor,
    item: &CatalogItem,
    progress: &[ProgressEvent],
) -> Option<Timestamp> {
    let live = effective(progress);
    let last = |wanted: fn(&Progress) -> bool| {
        live.iter()
            .filter(|e| wanted(&e.progress))
            .map(|e| e.occurred_at)
            .max()
    };
    match kind.progress_model {
        ProgressModel::Sessions => last(|p| matches!(p, Progress::Session { .. })),
        ProgressModel::Single | ProgressModel::Paged | ProgressModel::Timed => {
            last(|p| matches!(p, Progress::Finished))
        }
        ProgressModel::Episodic => {
            let run_end = runs(progress)
                .iter()
                .filter_map(|r| r.finished.map(|e| e.occurred_at))
                .max();
            whole_pass_at(item, &live).max(run_end)
        }
    }
}

/// Views of each episode, by when they happened.
fn episode_views(
    live: &[&ProgressEvent],
) -> std::collections::BTreeMap<(u32, u32), Vec<Timestamp>> {
    let mut views = std::collections::BTreeMap::<(u32, u32), Vec<Timestamp>>::new();
    for e in live {
        if let Progress::Episode { season, episode } = e.progress {
            views
                .entry((season, episode))
                .or_default()
                .push(e.occurred_at);
        }
    }
    for at in views.values_mut() {
        at.sort_unstable();
    }
    views
}

/// When the last whole pass over the known regular episodes was done.
fn whole_pass_at(item: &CatalogItem, live: &[&ProgressEvent]) -> Option<Timestamp> {
    let views = episode_views(live);
    let known = known_episodes(item);
    let passes = known
        .iter()
        .map(|ep| views.get(ep).map_or(0, Vec::len))
        .min()
        .filter(|n| *n > 0)?;
    known
        .iter()
        .filter_map(|ep| views.get(ep)?.get(passes - 1).copied())
        .max()
}

/// Whole passes over the known regular episodes: how often the least watched one was watched.
fn passes(item: &CatalogItem, progress: &[ProgressEvent]) -> u32 {
    let views = episode_views(&effective(progress));
    let least = known_episodes(item)
        .iter()
        .map(|ep| views.get(ep).map_or(0, Vec::len))
        .min()
        .unwrap_or(0);
    u32::try_from(least).unwrap_or(u32::MAX)
}

pub(crate) fn step_for(
    kind: &KindDescriptor,
    item: &CatalogItem,
    summary: &ProgressSummary,
) -> NextStep {
    match kind.progress_model {
        ProgressModel::Single => NextStep::Finish {
            again: summary.finished > 0,
        },
        ProgressModel::Episodic => next_episode(item, summary),
        ProgressModel::Paged => NextStep::Page {
            page: summary.page,
            chapter: summary.chapter,
            pages: attr_u32(item, "page_count"),
            chapters: attr_u32(item, "chapter_count"),
        },
        ProgressModel::Timed => NextStep::Position {
            seconds: summary.position,
            total: attr_u64(item, "duration_seconds"),
        },
        ProgressModel::Sessions => NextStep::Session {
            plays: summary.sessions,
        },
    }
}

/// Regular episodes of an episodic item, when its data knows them.
pub(crate) fn episode_total(item: &CatalogItem) -> Option<u32> {
    let s = seasons(item);
    (!s.is_empty()).then(|| s.iter().map(|(_, c)| c).sum())
}

/// Share done, 0..=1, for a progress bar; `None` when it cannot be derived.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub(crate) fn fraction(
    step: &NextStep,
    summary: &ProgressSummary,
    item: &CatalogItem,
) -> Option<f32> {
    let ratio = |done: u64, total: u64| (total > 0).then(|| (done as f32 / total as f32).min(1.0));
    match step {
        NextStep::Finish { .. } => Some(if summary.finished > 0 { 1.0 } else { 0.0 }),
        NextStep::Episode { .. } | NextStep::CaughtUp => {
            let watched = summary.episodes.iter().filter(|(s, _)| *s > 0).count() as u64;
            ratio(watched, u64::from(episode_total(item)?))
        }
        NextStep::Page { page, pages, .. } => ratio(u64::from((*page)?), u64::from((*pages)?)),
        NextStep::Position { seconds, total } => ratio((*seconds)?, (*total)?),
        NextStep::Session { .. } => None,
    }
}
