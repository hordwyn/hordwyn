//! The computed next step of a library entry (design.md 3.4): what the primary button on a
//! card, on Home and on the item page does.

use domain::catalog::CatalogItem;
use domain::kind::{KindDescriptor, ProgressModel};
use domain::library::LibraryEntry;
use domain::progress::{ProgressEvent, ProgressSummary};
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
/// included; they are applied here). The entry's status does not decide it (OT2A-1): the
/// history does.
#[must_use]
pub fn next_step(
    _entry: &LibraryEntry,
    kind: &KindDescriptor,
    item: &CatalogItem,
    progress: &[ProgressEvent],
) -> NextStep {
    step_for(kind, item, &ProgressSummary::of(progress))
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
        ProgressModel::Episodic => passes(item, progress),
        ProgressModel::Single | ProgressModel::Paged | ProgressModel::Timed => {
            ProgressSummary::of(progress).finished
        }
    };
    counted.max(u32::from(completed))
}

/// Whole passes over the known regular episodes: how often the least watched one was watched.
fn passes(item: &CatalogItem, progress: &[ProgressEvent]) -> u32 {
    let mut views = std::collections::BTreeMap::<(u32, u32), u32>::new();
    for e in domain::progress::effective(progress) {
        if let domain::progress::Progress::Episode { season, episode } = e.progress {
            *views.entry((season, episode)).or_default() += 1;
        }
    }
    seasons(item)
        .into_iter()
        .flat_map(|(season, count)| (1..=count).map(move |episode| (season, episode)))
        .map(|ep| views.get(&ep).copied().unwrap_or(0))
        .min()
        .unwrap_or(0)
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
