//! Statistics (design.md 6.1, owner ruling DEC-171): all-time totals per group for Home and
//! time series per group for the Stats page. Plain numbers; the UI formats them.

use std::collections::{BTreeMap, HashMap};
use std::hash::BuildHasher;

use domain::Timestamp;
use domain::catalog::CatalogItem;
use domain::ids::{ItemId, SessionId};
use domain::kind::{KindGroup, KindId, Status};
use domain::progress::{Progress, ProgressEvent, effective};
use domain::user::Actor;
use serde_json::Value;

use crate::{App, AppResult};

/// Activity amounts over a period. `minutes` counts only what can be derived: runtimes of
/// watched movies and episodes (from item attributes), positions moved forward in timed
/// kinds, and session durations.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Amounts {
    /// Effective progress events.
    pub events: u64,
    /// Events that moved an entry to Completed.
    pub completions: u64,
    /// `Finished` events (including rewatches).
    pub finished: u64,
    /// Episodes watched (including rewatches).
    pub episodes: u64,
    /// Pages read (forward moves).
    pub pages: u64,
    /// Logged sessions.
    pub plays: u64,
    pub minutes: u64,
}

impl Amounts {
    pub(crate) fn add(&mut self, o: &Self) {
        self.events += o.events;
        self.completions += o.completions;
        self.finished += o.finished;
        self.episodes += o.episodes;
        self.pages += o.pages;
        self.plays += o.plays;
        self.minutes += o.minutes;
    }
}

/// All-time totals: entries by status plus activity amounts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub entries: u64,
    pub planned: u64,
    pub in_progress: u64,
    pub completed: u64,
    pub paused: u64,
    pub dropped: u64,
    pub amounts: Amounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupTotals {
    pub group: KindGroup,
    pub totals: Totals,
}

/// All-time activity of one kind (OT-17, DEC-542: the time tiles are per kind).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindTotals {
    pub kind: KindId,
    pub amounts: Amounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsTotals {
    pub overall: Totals,
    /// Watch, Read, Listen, Play always; Other only when it has entries.
    pub groups: Vec<GroupTotals>,
    /// The kinds with activity, sorted by kind. Their amounts add up to those of their
    /// group and to `overall` (minutes too: seconds become whole minutes per kind).
    pub kinds: Vec<KindTotals>,
}

/// Time-series bucket size (calendar of the user's zone, `bucket_of`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bucket {
    Month,
    Year,
}

/// One bucket of a series: `2026-03` (month) or `2026` (year).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesPoint {
    pub bucket: String,
    pub amounts: Amounts,
}

/// The amounts of one kind in one month of the user's zone, with seconds not yet turned into minutes. The
/// repositories answer these (Postgres sums them in SQL), so the totals and series cost one
/// row per kind and month, not one per event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthAmounts {
    pub kind: KindId,
    /// `2026-03`.
    pub month: String,
    pub amounts: Amounts,
    pub seconds: u64,
}

fn attr(item: &CatalogItem, name: &str) -> u64 {
    item.attributes
        .get(name)
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

/// What one item's effective events add, per event: `(when, amounts, seconds)`.
fn contributions<T: BuildHasher>(
    item: &CatalogItem,
    events: &[&ProgressEvent],
    durations: &HashMap<SessionId, u64, T>,
) -> Vec<(Timestamp, Amounts, u64)> {
    let mut page = 0u64;
    let mut position = 0u64;
    let mut out = Vec::with_capacity(events.len());
    for e in events {
        let mut a = Amounts {
            events: 1,
            completions: u64::from(
                e.status_change
                    .as_ref()
                    .is_some_and(|c| c.to.as_str() == Status::COMPLETED),
            ),
            ..Amounts::default()
        };
        let mut seconds = 0;
        match e.progress {
            Progress::Finished => {
                a.finished = 1;
                seconds = attr(item, "runtime_minutes") * 60;
            }
            Progress::Episode { .. } => {
                a.episodes = 1;
                seconds = attr(item, "episode_runtime_minutes") * 60;
            }
            Progress::Paged { page: Some(p), .. } => {
                a.pages = u64::from(p).saturating_sub(page);
                page = u64::from(p);
            }
            Progress::Position { seconds: s } => {
                seconds = s.saturating_sub(position);
                position = s;
            }
            Progress::Session { session_id } => {
                a.plays = 1;
                seconds = durations.get(&session_id).copied().unwrap_or(0) * 60;
            }
            Progress::Paged { page: None, .. } | Progress::Undo { .. } => {}
        }
        out.push((e.occurred_at, a, seconds));
    }
    out
}

/// The statistics rules over one user's events (all of them, undo events included): each
/// effective event counts once; `Finished` and episodes add the item's `runtime_minutes` /
/// `episode_runtime_minutes` (whole numbers only), pages and positions count forward moves
/// per item in `(occurred_at, id)` order, sessions add their duration (from `durations`,
/// the user's own sessions). Events of items missing from `items` are left out. Sorted by
/// kind and month.
#[must_use]
pub fn month_amounts<S: BuildHasher, T: BuildHasher>(
    items: &HashMap<ItemId, CatalogItem, S>,
    events: &[ProgressEvent],
    durations: &HashMap<SessionId, u64, T>,
    zone: crate::time_zone::Tz,
) -> Vec<MonthAmounts> {
    let mut by_item: BTreeMap<ItemId, Vec<ProgressEvent>> = BTreeMap::new();
    for e in events {
        by_item.entry(e.item_id).or_default().push(e.clone());
    }
    let mut out: BTreeMap<(KindId, String), (Amounts, u64)> = BTreeMap::new();
    for (id, events) in &by_item {
        let Some(item) = items.get(id) else { continue };
        let mut live = effective(events);
        live.sort_by_key(|e| (e.occurred_at, e.id));
        for (at, a, secs) in contributions(item, &live, durations) {
            let (sum, s) = out
                .entry((item.kind.clone(), bucket_of(at, zone, Bucket::Month)))
                .or_default();
            sum.add(&a);
            *s += secs;
        }
    }
    out.into_iter()
        .map(|((kind, month), (amounts, seconds))| MonthAmounts {
            kind,
            month,
            amounts,
            seconds,
        })
        .collect()
}

/// The amounts of one kind over a period, with seconds not yet turned into minutes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindAmounts {
    pub kind: KindId,
    pub amounts: Amounts,
    pub seconds: u64,
}

/// The rules of [`month_amounts`] for the effective events in `[from, to)` only, summed per
/// kind (sorted by kind): pages and positions still count forward moves against the item's
/// earlier events, so `events` must hold every event of the items concerned.
#[must_use]
pub fn window_amounts<S: BuildHasher, T: BuildHasher>(
    items: &HashMap<ItemId, CatalogItem, S>,
    events: &[ProgressEvent],
    durations: &HashMap<SessionId, u64, T>,
    from: Timestamp,
    to: Timestamp,
) -> Vec<KindAmounts> {
    let mut by_item: BTreeMap<ItemId, Vec<ProgressEvent>> = BTreeMap::new();
    for e in events {
        by_item.entry(e.item_id).or_default().push(e.clone());
    }
    let mut out: BTreeMap<KindId, (Amounts, u64)> = BTreeMap::new();
    for (id, events) in &by_item {
        let Some(item) = items.get(id) else { continue };
        let mut live = effective(events);
        live.sort_by_key(|e| (e.occurred_at, e.id));
        for (at, a, secs) in contributions(item, &live, durations) {
            if at < from || at >= to {
                continue;
            }
            let (sum, s) = out.entry(item.kind.clone()).or_default();
            sum.add(&a);
            *s += secs;
        }
    }
    out.into_iter()
        .map(|(kind, (amounts, seconds))| KindAmounts {
            kind,
            amounts,
            seconds,
        })
        .collect()
}

fn bucket_of(t: Timestamp, zone: crate::time_zone::Tz, b: Bucket) -> String {
    let local = t.with_timezone(&zone);
    match b {
        Bucket::Month => local.format("%Y-%m").to_string(),
        Bucket::Year => local.format("%Y").to_string(),
    }
}

impl App {
    /// All-time totals overall and per group (Home).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn stats_totals(&self, actor: Actor) -> AppResult<StatsTotals> {
        let mut per: BTreeMap<KindGroup, Totals> = KindGroup::ALL[..4]
            .iter()
            .map(|g| (*g, Totals::default()))
            .collect();
        for (kind, status, n) in self.repos.library.counts(actor.user_id).await? {
            let t = per.entry(self.group_of(&kind)).or_default();
            t.entries += n;
            match status.as_str() {
                Status::PLANNED => t.planned += n,
                Status::IN_PROGRESS => t.in_progress += n,
                Status::COMPLETED => t.completed += n,
                Status::PAUSED => t.paused += n,
                Status::DROPPED => t.dropped += n,
                _ => {}
            }
        }
        let zone = self.user_zone(actor.user_id).await?;
        let mut per_kind: BTreeMap<KindId, (Amounts, u64)> = BTreeMap::new();
        for m in self
            .repos
            .progress
            .month_amounts(actor.user_id, zone.name())
            .await?
        {
            let (a, secs) = per_kind.entry(m.kind).or_default();
            a.add(&m.amounts);
            *secs += m.seconds;
        }
        // seconds become whole minutes per kind, and a group is the sum of its kinds, so
        // the tiles (per kind) add up to the group and overall figures (DEC-572)
        let mut kinds = Vec::with_capacity(per_kind.len());
        for (kind, (mut amounts, secs)) in per_kind {
            amounts.minutes = secs / 60;
            let group = self.group_of(&kind);
            let t = per.entry(group).or_default();
            if group == KindGroup::Other && t.entries == 0 {
                continue;
            }
            t.amounts.add(&amounts);
            kinds.push(KindTotals { kind, amounts });
        }
        let mut overall = Totals::default();
        let mut groups = Vec::new();
        for (group, t) in per {
            if group == KindGroup::Other && t.entries == 0 {
                continue;
            }
            overall.entries += t.entries;
            overall.planned += t.planned;
            overall.in_progress += t.in_progress;
            overall.completed += t.completed;
            overall.paused += t.paused;
            overall.dropped += t.dropped;
            overall.amounts.add(&t.amounts);
            groups.push(GroupTotals { group, totals: t });
        }
        Ok(StatsTotals {
            overall,
            groups,
            kinds,
        })
    }

    /// Activity per month or year, oldest first, for one group or all; buckets without
    /// activity are left out.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn stats_series(
        &self,
        actor: Actor,
        group: Option<KindGroup>,
        bucket: Bucket,
    ) -> AppResult<Vec<SeriesPoint>> {
        let mut per: BTreeMap<String, (Amounts, u64)> = BTreeMap::new();
        let zone = self.user_zone(actor.user_id).await?;
        for m in self
            .repos
            .progress
            .month_amounts(actor.user_id, zone.name())
            .await?
        {
            if group.is_some_and(|g| g != self.group_of(&m.kind)) {
                continue;
            }
            let key = match bucket {
                Bucket::Month => m.month,
                Bucket::Year => m.month.split('-').next().unwrap_or_default().to_string(),
            };
            let (a, secs) = per.entry(key).or_default();
            a.add(&m.amounts);
            *secs += m.seconds;
        }
        Ok(per
            .into_iter()
            .map(|(bucket, (mut amounts, secs))| {
                amounts.minutes = secs / 60;
                SeriesPoint { bucket, amounts }
            })
            .collect())
    }
}
