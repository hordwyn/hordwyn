//! The person filter of the activity feed, read from the person's sessions (DEC-494).
//!
//! Every row a person takes part in hangs on a session with them: a play or a standalone
//! session at its own start, a "watched with" event within [`PAIR_SECS`] of it (whole
//! seconds, so under `PAIR_SECS + 1`), a play's event at the session's start (both written
//! together, `log_play` and `claim_session`), a shared row at its start. So the feed reads
//! the person's sessions newest first ("anchors"), turns each into a region of
//! `REACH_SECS` around it, and builds the rows of every region exactly as the window walk
//! builds the rows of a window (`window_rows`, with the same pairing reads at its ends), from
//! one read per source for all regions of a round. Time between regions holds no row with the
//! person, so it is skipped. A round whose reads come back full (a dense region) hands over
//! to the window walk at the point reached.

use std::collections::HashSet;

use domain::Timestamp;
use domain::ids::CompanionId;
use domain::kind::KindId;
use domain::progress::ProgressEvent;
use domain::session::Session;
use domain::user::Actor;

use super::{
    ActivityQuery, ActivityRow, Ctx, EDGE_LIMIT, Key, MAX_WINDOW, PAIR_SECS, Pairing, Window,
    event_key, keep, session_key,
};
use crate::ports::{HistorySpans, HistoryWindow};
use crate::{App, AppResult};

/// Seconds around a session of the person in which a row with them can lie.
const REACH_SECS: i64 = PAIR_SECS + 1;

/// Rows a span read may return per anchor of the round before the round counts as dense.
const ROWS_PER_ANCHOR: u32 = 8;

/// One feed query with the person filter.
pub(super) struct Feed<'a> {
    pub actor: Actor,
    pub q: &'a ActivityQuery,
    pub kinds: &'a [KindId],
    /// The person and everyone merged into them.
    pub people: &'a HashSet<CompanionId>,
    pub representing: &'a [CompanionId],
    /// `before` is where the page starts; `since` and `end` are the query's bounds.
    pub base: Window,
}

/// What the person walk leaves for the window walk.
pub(super) struct PersonWalk {
    pub rows: Vec<ActivityRow>,
    /// Everything at or above this key is done.
    pub before: Option<Key>,
    /// No row is left below `before`.
    pub done: bool,
}

/// The regions `[lo, hi)` around `anchors` (newest first), below `before` and at or after
/// `since`, merged where they meet; newest first as `(hi, lo)`.
fn regions(anchors: &[Session], before: Option<Key>, since: Option<Timestamp>) -> Vec<(Key, Key)> {
    let reach = chrono::Duration::seconds(REACH_SECS);
    let mut out: Vec<(Key, Key)> = Vec::new();
    for s in anchors {
        let top = (s.started_at + reach, uuid::Uuid::nil());
        let hi = before.map_or(top, |b| b.min(top));
        let low = (s.started_at - reach, uuid::Uuid::nil());
        let lo = since.map_or(low, |t| low.max((t, uuid::Uuid::nil())));
        if lo >= hi {
            continue;
        }
        match out.last_mut() {
            Some(last) if hi >= last.1 => last.1 = last.1.min(lo),
            _ => out.push((hi, lo)),
        }
    }
    out
}

/// The time spans that hold every row of the regions and of their pairing reads
/// ([`Window::top`] three spans up, [`Window::bottom`] one down), merged; newest first.
fn read_spans(regions: &[(Key, Key)], base: Window) -> Vec<(Timestamp, Timestamp)> {
    let mut out: Vec<(Timestamp, Timestamp)> = Vec::new();
    for &(hi, lo) in regions {
        let (from, to) = (base.bottom(lo.0), base.top(hi.0, 3).0);
        match out.last_mut() {
            Some(last) if to >= last.0 => last.0 = last.0.min(from),
            _ => out.push((from, to)),
        }
    }
    out
}

/// [`App::pairing`] from rows read for all regions at once (both newest first, holding every
/// row of the window's pairing reads).
fn pairing_in(
    window: Window,
    own: Vec<Session>,
    events: &[ProgressEvent],
    sessions: &[Session],
) -> Pairing {
    let near_events = |top: Key, since: Timestamp| {
        events
            .iter()
            .filter(move |e| event_key(e) < top && e.occurred_at >= since)
            .take(EDGE_LIMIT as usize)
    };
    let near_sessions = |top: Key, since: Timestamp| {
        sessions
            .iter()
            .filter(move |s| session_key(s) < top && s.started_at >= since)
            .take(EDGE_LIMIT as usize)
    };
    let mut out = own;
    let mut above = vec![];
    let mut below = vec![];
    if let Some(b) = window.before {
        above = near_events(window.top(b.0, 2), b.0)
            .filter(|e| event_key(e) >= b)
            .cloned()
            .collect();
        out.extend(
            near_sessions(window.top(b.0, 3), b.0)
                .filter(|s| session_key(s) >= b)
                .cloned(),
        );
    }
    if let Some(l) = window.low {
        below = near_events(l, window.bottom(l.0)).cloned().collect();
        out.extend(near_sessions(l, window.bottom(l.0)).cloned());
    }
    out.sort_by_key(|s| std::cmp::Reverse(session_key(s)));
    out.dedup_by_key(|s| s.id);
    Pairing {
        sessions: out,
        above,
        below,
    }
}

/// One region's inputs to [`App::batch_rows`].
struct Region {
    window: Window,
    events: Vec<ProgressEvent>,
    pairing: Pairing,
    theirs: Vec<Session>,
}

impl App {
    /// The person-filtered rows from `feed.base.before` down, in rounds of anchors (doubling
    /// like the windows), until the page is full, the anchors run out, or a round is dense.
    pub(super) async fn person_walk(
        &self,
        ctx: &mut Ctx,
        feed: &Feed<'_>,
        limit: usize,
    ) -> AppResult<PersonWalk> {
        let reach = chrono::Duration::seconds(REACH_SECS);
        let ids: Vec<CompanionId> = feed.people.iter().copied().collect();
        let mut before = feed.base.before;
        let mut anchors_before = before.map(|b| (b.0 + reach, uuid::Uuid::nil()));
        let mut fetch = u32::try_from(limit + 1).unwrap_or(u32::MAX);
        let mut rows = Vec::new();
        loop {
            let w = HistoryWindow {
                before: anchors_before,
                since: feed.base.since.map(|s| s - reach),
                kinds: feed.kinds.to_vec(),
                limit: fetch,
            };
            let anchors = self
                .repos
                .sessions
                .with_participants_window(&ids, &w)
                .await?;
            let regions = regions(&anchors, before, feed.base.since);
            if !regions.is_empty() {
                let foreign = anchors.iter().any(|s| s.owner != feed.actor.user_id);
                let cap = fetch.saturating_mul(ROWS_PER_ANCHOR);
                let Some(batch) = self.person_round(ctx, feed, &regions, foreign, cap).await?
                else {
                    return Ok(PersonWalk {
                        rows,
                        before,
                        done: false,
                    });
                };
                rows.extend(
                    batch
                        .into_iter()
                        .filter(|r| keep(r, feed.q, feed.kinds, Some(feed.people))),
                );
                before = regions.last().map(|r| r.1);
            }
            let last = anchors.len() < fetch as usize;
            if last || rows.len() > limit {
                return Ok(PersonWalk {
                    rows,
                    before,
                    done: last,
                });
            }
            anchors_before = anchors.last().map(session_key);
            fetch = fetch.saturating_mul(2).min(MAX_WINDOW);
        }
    }

    /// Every row of `regions` (unfiltered), from one read per source; `None` when a read
    /// returned `cap` rows (it may have been cut).
    async fn person_round(
        &self,
        ctx: &mut Ctx,
        feed: &Feed<'_>,
        regions: &[(Key, Key)],
        foreign: bool,
        cap: u32,
    ) -> AppResult<Option<Vec<ActivityRow>>> {
        let user = feed.actor.user_id;
        let spans = HistorySpans {
            spans: read_spans(regions, feed.base),
            kinds: feed.kinds.to_vec(),
            limit: cap,
        };
        let (events, own) = if feed.q.shared_only {
            (vec![], vec![])
        } else {
            (
                self.repos.progress.recent_spans(user, &spans).await?,
                self.repos.sessions.by_owner_spans(user, &spans).await?,
            )
        };
        // a shared row with the person is an anchor of another owner
        let theirs = if foreign && !feed.representing.is_empty() {
            self.repos
                .sessions
                .with_participants_spans(feed.representing, &spans)
                .await?
        } else {
            vec![]
        };
        let full = |n: usize| n >= cap as usize;
        if full(events.len()) || full(own.len()) || full(theirs.len()) {
            return Ok(None);
        }
        let mut parts = Vec::with_capacity(regions.len());
        for &(hi, lo) in regions {
            let window = Window {
                before: Some(hi),
                low: Some(lo),
                ..feed.base
            };
            let mut shared = Vec::new();
            for s in &theirs {
                if s.owner != user
                    && window.contains(session_key(s))
                    && self.owner_shares(ctx, s.owner).await?
                {
                    shared.push(s.clone());
                }
            }
            parts.push(region(window, &events, &own, shared));
        }
        let sessions: Vec<&Session> = parts
            .iter()
            .flat_map(|p| p.pairing.sessions.iter().chain(&p.theirs))
            .collect();
        let region_events: Vec<ProgressEvent> =
            parts.iter().flat_map(|p| p.events.clone()).collect();
        self.load_names(ctx, &sessions).await?;
        self.load_items(ctx, &region_events, &sessions).await?;
        Ok(Some(
            parts
                .iter()
                .flat_map(|p| self.batch_rows(ctx, &p.events, &p.pairing, p.window, &p.theirs))
                .collect(),
        ))
    }
}

/// The inputs of one region, as `window_rows` prepares a window's.
fn region(
    window: Window,
    events: &[ProgressEvent],
    own: &[Session],
    theirs: Vec<Session>,
) -> Region {
    let mine: Vec<ProgressEvent> = events
        .iter()
        .filter(|e| window.contains(event_key(e)))
        .cloned()
        .collect();
    let sessions: Vec<Session> = own
        .iter()
        .filter(|s| window.contains(session_key(s)))
        .cloned()
        .collect();
    let pairing = if mine.is_empty() && sessions.is_empty() {
        Pairing {
            sessions,
            above: vec![],
            below: vec![],
        }
    } else {
        pairing_in(window, sessions, events, own)
    };
    Region {
        window,
        events: mine,
        pairing,
        theirs,
    }
}
