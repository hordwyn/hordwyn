//! The activity feed (Home "Recent activity", the History page): the user's own effective
//! progress events and plays, plus sessions linked users shared with them, newest first.

use std::collections::{HashMap, HashSet};

use domain::Timestamp;
use domain::catalog::CatalogItem;
use domain::ids::{CompanionId, EventId, ItemId, SessionId};
use domain::kind::{KindGroup, KindId, ProgressModel};
use domain::progress::{Progress, ProgressEvent};
use domain::session::Session;
use domain::user::Actor;

use crate::ports::HistoryWindow;
use crate::{App, AppResult, UNDO_WINDOW_HOURS};

mod person;
use person::Feed;

/// A "watched with" session belongs to the progress event logged within this many seconds.
const PAIR_SECS: i64 = 120;
/// Largest page.
pub const MAX_ACTIVITY_PAGE: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    /// A progress event (watched, page, position, finished), maybe with people.
    Progress,
    /// A logged session (play, gaming session), own or shared.
    Session,
}

/// Filters and paging of the feed. Rows are strictly before `before` (the `next` of the
/// previous page).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActivityQuery {
    pub before: Option<(Timestamp, uuid::Uuid)>,
    /// 1 to [`MAX_ACTIVITY_PAGE`]; 0 means 20.
    pub limit: u32,
    pub group: Option<KindGroup>,
    pub kind: Option<KindId>,
    /// Only rows this companion of the actor (or, for shared rows, the actor) took part in.
    pub person: Option<CompanionId>,
    /// Only these row types (empty: all).
    pub types: Vec<ActivityType>,
    /// Only sessions shared with the actor.
    pub shared_only: bool,
    /// Only this calendar year (UTC).
    pub year: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityPerson {
    pub companion_id: CompanionId,
    pub name: String,
    /// Stands for the viewer.
    pub is_me: bool,
}

/// One row of the feed: a sentence for the UI ("Watched S2 E5 of Dark with Anna").
#[derive(Debug, Clone, PartialEq)]
pub struct ActivityRow {
    /// Paging key; pass the last row's as `before`.
    pub key: (Timestamp, uuid::Uuid),
    pub at: Timestamp,
    pub row_type: ActivityType,
    pub item_id: ItemId,
    pub title: String,
    pub kind: KindId,
    pub group: KindGroup,
    /// The progress of a `Progress` row.
    pub progress: Option<Progress>,
    /// The event (own rows), for undo and edit.
    pub event_id: Option<EventId>,
    pub session_id: Option<SessionId>,
    pub participants: Vec<ActivityPerson>,
    /// A session another user shared with the viewer (read-only, "Copy to my log").
    pub shared: bool,
    /// Display name of the owner of a shared row.
    pub owner_name: Option<String>,
    /// The viewer may still undo it.
    pub undoable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityPage {
    pub rows: Vec<ActivityRow>,
    /// `before` of the next page; `None` at the end.
    pub next: Option<(Timestamp, uuid::Uuid)>,
}

struct Ctx {
    items: HashMap<ItemId, CatalogItem>,
    names: HashMap<CompanionId, String>,
    mine: HashSet<CompanionId>,
    /// Owners of shared rows: their display name, `None` when they do not share sessions.
    owners: HashMap<domain::ids::UserId, Option<String>>,
}

type Key = (Timestamp, uuid::Uuid);

fn session_key(s: &Session) -> Key {
    (s.started_at, s.id.0)
}

fn event_key(e: &ProgressEvent) -> Key {
    (e.occurred_at, e.id.0)
}

/// Rows of one window in the three sources, as read (each newest first, at most `limit`).
struct Streams {
    events: Vec<ProgressEvent>,
    own: Vec<Session>,
    theirs: Vec<Session>,
}

impl Streams {
    /// The lower end of the window: the newest last key among the sources that filled their
    /// page (older rows of the others come with the next window); `None` when none did.
    fn low(&self, limit: u32) -> Option<Key> {
        let full = |n: usize| n == limit as usize;
        let lasts = [
            self.events
                .last()
                .filter(|_| full(self.events.len()))
                .map(event_key),
            self.own
                .last()
                .filter(|_| full(self.own.len()))
                .map(session_key),
            self.theirs
                .last()
                .filter(|_| full(self.theirs.len()))
                .map(session_key),
        ];
        lasts.into_iter().flatten().max()
    }
}

/// The window `[low, before)` being turned into rows, with the query's own bounds (the
/// year) that no read may cross.
#[derive(Clone, Copy)]
struct Window {
    before: Option<Key>,
    low: Option<Key>,
    since: Option<Timestamp>,
    end: Option<Key>,
}

impl Window {
    fn contains(&self, k: Key) -> bool {
        self.before.is_none_or(|b| k < b) && self.low.is_none_or(|l| k >= l)
    }

    /// The upper end of the pairing read `reach` pair spans above `t`, never past the
    /// query's own bounds (a year), as one big read would not be.
    fn top(&self, t: Timestamp, reach: i32) -> Key {
        let k = (
            t + chrono::Duration::seconds(PAIR_SECS) * reach,
            uuid::Uuid::nil(),
        );
        self.end.map_or(k, |e| e.min(k))
    }

    /// The lower end of the pairing read below `t`.
    fn bottom(&self, t: Timestamp) -> Timestamp {
        let pair = chrono::Duration::seconds(PAIR_SECS);
        self.since.map_or(t - pair, |s| s.max(t - pair))
    }
}

/// The own sessions of the window plus those within [`PAIR_SECS`] of its ends, and the
/// events just above and below it: a "watched with" pair split by a window end is matched
/// as in one big read.
struct Pairing {
    sessions: Vec<Session>,
    above: Vec<ProgressEvent>,
    below: Vec<ProgressEvent>,
}

/// Largest window per source (rows), reached by doubling from `2 * limit + 1`.
const MAX_WINDOW: u32 = 2_000;

/// Rows read near a window end (pairs are logged together, so a handful at most).
const EDGE_LIMIT: u32 = 64;

fn year_bounds(year: i32) -> Option<(Timestamp, Timestamp)> {
    use chrono::TimeZone;
    let start = chrono::Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).single()?;
    let end = chrono::Utc
        .with_ymd_and_hms(year + 1, 1, 1, 0, 0, 0)
        .single()?;
    Some((start, end))
}

impl App {
    /// One page of the activity feed. Events, own sessions and shared sessions are read in
    /// windows of `2 * limit + 1` rows each, doubling while the page is not full (DEC-235,
    /// WP4-1), never all of them. With the person filter the person's sessions drive the
    /// reads (`person::person_walk`, DEC-494); the windows take over only after a dense round.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn activity_feed(&self, actor: Actor, q: ActivityQuery) -> AppResult<ActivityPage> {
        let limit = if q.limit == 0 {
            20
        } else {
            q.limit.min(MAX_ACTIVITY_PAGE)
        } as usize;
        let kinds: Option<Vec<KindId>> = match (&q.group, &q.kind) {
            (None, None) => Some(vec![]),
            _ => self.selected_kinds(q.group, &q.kind.iter().cloned().collect::<Vec<_>>()),
        };
        let Some(kinds) = kinds else {
            return Ok(ActivityPage {
                rows: vec![],
                next: None,
            });
        };
        let bounds = q.year.and_then(year_bounds);
        let end = bounds.map(|(_, e)| (e, uuid::Uuid::nil()));
        let mut before = match (q.before, end) {
            (Some(b), Some(e)) => Some(b.min(e)),
            (b, e) => b.or(e),
        };
        let since = bounds.map(|(s, _)| s);
        let me = self.user(actor.user_id).await?;
        let people = match q.person {
            Some(p) => Some(self.person_ids(actor, p).await?),
            None => None,
        };
        let representing: Vec<CompanionId> = self
            .companion_ids_representing(actor)
            .await?
            .into_iter()
            .collect();
        let mut ctx = self.feed_context(actor, me.self_companion).await?;
        let mut rows: Vec<ActivityRow> = Vec::new();
        let mut done = false;
        if let Some(p) = &people {
            let feed = Feed {
                actor,
                q: &q,
                kinds: &kinds,
                people: p,
                representing: &representing,
                base: Window {
                    before,
                    low: None,
                    since,
                    end,
                },
            };
            let walk = self.person_walk(&mut ctx, &feed, limit).await?;
            (rows, before, done) = (walk.rows, walk.before, walk.done);
        }
        let mut fetch = u32::try_from(limit * 2 + 1).unwrap_or(u32::MAX);
        while rows.len() <= limit && !done {
            let w = HistoryWindow {
                before,
                since,
                kinds: kinds.clone(),
                limit: fetch,
            };
            let streams = self
                .feed_streams(actor, q.shared_only, &w, &representing)
                .await?;
            let low = streams.low(fetch);
            done = low.is_none();
            let window = Window {
                before,
                low: low.or(since.map(|s| (s, uuid::Uuid::nil()))),
                since,
                end,
            };
            let batch = self
                .window_rows(actor, &mut ctx, streams, window, &kinds)
                .await?;
            rows.extend(
                batch
                    .into_iter()
                    .filter(|r| keep(r, &q, &kinds, people.as_ref())),
            );
            before = window.low;
            if before.is_none() {
                done = true;
            }
            // a filter that drops most rows (a person, a type) needs fewer, larger windows
            fetch = fetch.saturating_mul(2).min(MAX_WINDOW);
        }
        rows.sort_by_key(|r| std::cmp::Reverse(r.key));
        let more = rows.len() > limit || !done;
        rows.truncate(limit);
        let next = if more {
            rows.last().map(|r| r.key).or(before)
        } else {
            None
        };
        Ok(ActivityPage { rows, next })
    }

    async fn feed_streams(
        &self,
        actor: Actor,
        shared_only: bool,
        w: &HistoryWindow,
        representing: &[CompanionId],
    ) -> AppResult<Streams> {
        let (events, own) = if shared_only {
            (vec![], vec![])
        } else {
            (
                self.repos.progress.recent(actor.user_id, w).await?,
                self.repos
                    .sessions
                    .by_owner_window(actor.user_id, w)
                    .await?,
            )
        };
        let theirs = if representing.is_empty() {
            vec![]
        } else {
            self.repos
                .sessions
                .with_participants_window(representing, w)
                .await?
        };
        Ok(Streams {
            events,
            own,
            theirs,
        })
    }

    /// The rows of one window: its events (paired with their sessions), its own sessions
    /// that no event took, and the shared sessions of owners who share.
    async fn window_rows(
        &self,
        actor: Actor,
        ctx: &mut Ctx,
        streams: Streams,
        window: Window,
        kinds: &[KindId],
    ) -> AppResult<Vec<ActivityRow>> {
        let events: Vec<ProgressEvent> = streams
            .events
            .into_iter()
            .filter(|e| window.contains(event_key(e)))
            .collect();
        let own: Vec<Session> = streams
            .own
            .into_iter()
            .filter(|s| window.contains(session_key(s)))
            .collect();
        let mut theirs = Vec::new();
        for s in streams.theirs {
            if s.owner != actor.user_id
                && window.contains(session_key(&s))
                && self.owner_shares(ctx, s.owner).await?
            {
                theirs.push(s);
            }
        }
        let pairing = if events.is_empty() && own.is_empty() {
            Pairing {
                sessions: own,
                above: vec![],
                below: vec![],
            }
        } else {
            self.pairing(actor, window, kinds, own).await?
        };
        let all_sessions: Vec<&Session> = pairing.sessions.iter().chain(&theirs).collect();
        self.load_names(ctx, &all_sessions).await?;
        self.load_items(ctx, &events, &all_sessions).await?;
        Ok(self.batch_rows(ctx, &events, &pairing, window, &theirs))
    }

    /// Display name of a shared row's owner; `false` when they do not share sessions.
    async fn owner_shares(&self, ctx: &mut Ctx, owner: domain::ids::UserId) -> AppResult<bool> {
        if let Some(name) = ctx.owners.get(&owner) {
            return Ok(name.is_some());
        }
        let u = self.user_opt(owner).await?;
        let name = u.filter(|u| u.share_sessions).map(|u| u.display_name);
        let shares = name.is_some();
        ctx.owners.insert(owner, name);
        Ok(shares)
    }

    async fn pairing(
        &self,
        actor: Actor,
        window: Window,
        kinds: &[KindId],
        own: Vec<Session>,
    ) -> AppResult<Pairing> {
        let near = |before: Option<Key>, since: Option<Timestamp>| HistoryWindow {
            before,
            since,
            kinds: kinds.to_vec(),
            limit: EDGE_LIMIT,
        };
        let top = |t: Timestamp, reach: i32| Some(window.top(t, reach));
        let bottom = |t: Timestamp| Some(window.bottom(t));
        let user = actor.user_id;
        let mut sessions = own;
        let mut above = vec![];
        let mut below = vec![];
        if let Some(b) = window.before {
            // UX4-16: an event up to two pair spans above the page can take a session within one
            // span of it, choosing the newest within its own span (so sessions up to three
            // spans are read). Exact unless a chain of three or more events and sessions of one
            // item, each within two minutes of the next, crosses the boundary.
            let w = near(top(b.0, 2), Some(b.0));
            above = self.repos.progress.recent(user, &w).await?;
            above.retain(|e| event_key(e) >= b);
            let w = near(top(b.0, 3), Some(b.0));
            let mut s = self.repos.sessions.by_owner_window(user, &w).await?;
            s.retain(|s| session_key(s) >= b);
            sessions.extend(s);
        }
        if let Some(l) = window.low {
            let w = near(Some(l), bottom(l.0));
            below = self.repos.progress.recent(user, &w).await?;
            sessions.extend(self.repos.sessions.by_owner_window(user, &w).await?);
        }
        sessions.sort_by_key(|s| std::cmp::Reverse(session_key(s)));
        sessions.dedup_by_key(|s| s.id);
        Ok(Pairing {
            sessions,
            above,
            below,
        })
    }

    /// `person` and every companion of the actor whose merge chain ends in them (UX3-16):
    /// plays logged with a duplicate before a merge still count for the person.
    async fn person_ids(
        &self,
        actor: Actor,
        person: CompanionId,
    ) -> AppResult<HashSet<CompanionId>> {
        let all = self.repos.companions.by_owner(actor.user_id).await?;
        let into: HashMap<CompanionId, CompanionId> = all
            .iter()
            .filter_map(|c| c.merged_into.map(|m| (c.id, m)))
            .collect();
        let mut out = HashSet::from([person]);
        for c in &all {
            let mut at = c.id;
            // a chain is at most as long as the list; a cycle never reaches `person`
            for _ in 0..all.len() {
                match into.get(&at) {
                    Some(&next) if next == person => {
                        out.insert(c.id);
                        break;
                    }
                    Some(&next) => at = next,
                    None => break,
                }
            }
        }
        Ok(out)
    }

    async fn feed_context(&self, actor: Actor, self_companion: CompanionId) -> AppResult<Ctx> {
        let mut mine = HashSet::from([self_companion]);
        for c in self.repos.companions.linked_to(actor.user_id).await? {
            mine.insert(c.id);
        }
        Ok(Ctx {
            items: HashMap::new(),
            names: HashMap::new(),
            mine,
            owners: HashMap::new(),
        })
    }

    /// Names of the participants not seen yet, in one read.
    async fn load_names(&self, ctx: &mut Ctx, sessions: &[&Session]) -> AppResult<()> {
        let missing: Vec<CompanionId> = sessions
            .iter()
            .flat_map(|s| s.participants.iter().map(|p| p.companion_id))
            .filter(|c| !ctx.names.contains_key(c))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        for c in self.repos.companions.get_many(&missing).await? {
            if c.merged_into.is_some_and(|m| ctx.mine.contains(&m)) {
                ctx.mine.insert(c.id);
            }
            ctx.names.insert(c.id, c.name);
        }
        Ok(())
    }

    async fn load_items(
        &self,
        ctx: &mut Ctx,
        events: &[ProgressEvent],
        sessions: &[&Session],
    ) -> AppResult<()> {
        let missing: Vec<ItemId> = events
            .iter()
            .map(|e| e.item_id)
            .chain(sessions.iter().map(|s| s.item_id))
            .filter(|i| !ctx.items.contains_key(i))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        for i in self.repos.catalog.get_many(&missing).await? {
            ctx.items.insert(i.id, i);
        }
        Ok(())
    }

    /// The session an event goes with: its own for a play, else the newest own session of
    /// the same item within [`PAIR_SECS`] that no newer event took ("watched with").
    fn pair_of<'a>(
        e: &ProgressEvent,
        sessions: &'a [Session],
        used: &HashSet<SessionId>,
    ) -> Option<&'a Session> {
        match e.progress {
            Progress::Session { session_id } => sessions.iter().find(|s| s.id == session_id),
            _ => sessions.iter().find(|s| {
                s.item_id == e.item_id
                    && !used.contains(&s.id)
                    && (s.started_at - e.occurred_at).num_seconds().abs() <= PAIR_SECS
            }),
        }
    }

    fn batch_rows(
        &self,
        ctx: &Ctx,
        events: &[ProgressEvent],
        pairing: &Pairing,
        window: Window,
        theirs: &[Session],
    ) -> Vec<ActivityRow> {
        let now = self.now();
        let mut used: HashSet<SessionId> = HashSet::new();
        // newer events outside the window took their sessions first
        for e in &pairing.above {
            if let Some(s) = Self::pair_of(e, &pairing.sessions, &used) {
                used.insert(s.id);
            }
        }
        let mut rows = Vec::new();
        for e in events {
            let session = Self::pair_of(e, &pairing.sessions, &used);
            if let Some(s) = session {
                used.insert(s.id);
            }
            let Some(item) = ctx.items.get(&e.item_id) else {
                continue;
            };
            let row_type = if matches!(e.progress, Progress::Session { .. }) {
                ActivityType::Session
            } else {
                ActivityType::Progress
            };
            let mut row = self.row(ctx, item, event_key(e), row_type, session);
            row.progress = (row_type == ActivityType::Progress).then(|| e.progress.clone());
            row.event_id = Some(e.id);
            row.undoable = now - e.recorded_at <= chrono::Duration::hours(UNDO_WINDOW_HOURS);
            rows.push(row);
        }
        for e in &pairing.below {
            if let Some(s) = Self::pair_of(e, &pairing.sessions, &used) {
                used.insert(s.id);
            }
        }
        for s in pairing
            .sessions
            .iter()
            .filter(|s| window.contains(session_key(s)) && !used.contains(&s.id))
        {
            // plays of session kinds appear through their event; "watched with" sessions
            // without a matching event (claimed copies) stand alone
            let Some(item) = ctx.items.get(&s.item_id) else {
                continue;
            };
            let sessions_kind = self
                .kinds
                .get(&item.kind)
                .is_some_and(|k| k.progress_model == ProgressModel::Sessions);
            if !sessions_kind {
                rows.push(self.row(ctx, item, session_key(s), ActivityType::Session, Some(s)));
            }
        }
        for s in theirs {
            let Some(item) = ctx.items.get(&s.item_id) else {
                continue;
            };
            let mut row = self.row(ctx, item, session_key(s), ActivityType::Session, Some(s));
            row.shared = true;
            row.owner_name = ctx.owners.get(&s.owner).cloned().flatten();
            rows.push(row);
        }
        rows
    }

    fn row(
        &self,
        ctx: &Ctx,
        item: &CatalogItem,
        key: (Timestamp, uuid::Uuid),
        row_type: ActivityType,
        session: Option<&Session>,
    ) -> ActivityRow {
        let participants = session
            .map(|s| {
                s.participants
                    .iter()
                    .map(|p| ActivityPerson {
                        companion_id: p.companion_id,
                        name: ctx.names.get(&p.companion_id).cloned().unwrap_or_default(),
                        is_me: ctx.mine.contains(&p.companion_id),
                    })
                    .collect()
            })
            .unwrap_or_default();
        ActivityRow {
            key,
            at: key.0,
            row_type,
            item_id: item.id,
            title: item.title.clone(),
            kind: item.kind.clone(),
            group: self.group_of(&item.kind),
            progress: None,
            event_id: None,
            session_id: session.map(|s| s.id),
            participants,
            shared: false,
            owner_name: None,
            undoable: false,
        }
    }
}

/// `people`: the filtered person and everyone merged into them ([`App::person_ids`]).
fn keep(
    r: &ActivityRow,
    q: &ActivityQuery,
    kinds: &[KindId],
    people: Option<&HashSet<CompanionId>>,
) -> bool {
    (kinds.is_empty() || kinds.contains(&r.kind))
        && (q.types.is_empty() || q.types.contains(&r.row_type))
        && (!q.shared_only || r.shared)
        && people.is_none_or(|p| r.participants.iter().any(|x| p.contains(&x.companion_id)))
}
