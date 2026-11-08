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
    owners: HashMap<domain::ids::UserId, String>,
}

fn year_bounds(year: i32) -> Option<(Timestamp, Timestamp)> {
    use chrono::TimeZone;
    let start = chrono::Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).single()?;
    let end = chrono::Utc
        .with_ymd_and_hms(year + 1, 1, 1, 0, 0, 0)
        .single()?;
    Some((start, end))
}

impl App {
    /// One page of the activity feed.
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
        let mut before = match (q.before, bounds) {
            (Some(b), Some((_, end))) => Some(b.min((end, uuid::Uuid::nil()))),
            (Some(b), None) => Some(b),
            (None, Some((_, end))) => Some((end, uuid::Uuid::nil())),
            (None, None) => None,
        };
        let since = bounds.map(|(s, _)| s);
        let me = self.user(actor.user_id).await?;
        let own_sessions = if q.shared_only {
            vec![]
        } else {
            self.my_sessions(actor).await?
        };
        let shared = self.shared_sessions(actor).await?;
        let mut ctx = self
            .feed_context(actor, &own_sessions, &shared, me.self_companion)
            .await?;
        let people = match q.person {
            Some(p) => Some(self.person_ids(actor, p).await?),
            None => None,
        };
        let mut rows: Vec<ActivityRow> = Vec::new();
        let mut done = false;
        let fetch = u32::try_from(limit * 2 + 1).unwrap_or(u32::MAX);
        while rows.len() <= limit && !done {
            let events = if q.shared_only {
                vec![]
            } else {
                self.repos
                    .progress
                    .recent(
                        actor.user_id,
                        &HistoryWindow {
                            before,
                            since,
                            kinds: kinds.clone(),
                            limit: fetch,
                        },
                    )
                    .await?
            };
            done = events.len() < fetch as usize;
            let low = if done {
                since.map(|s| (s, uuid::Uuid::nil()))
            } else {
                events.last().map(|e| (e.occurred_at, e.id.0))
            };
            let in_window = |s: &Session| {
                let k = (s.started_at, s.id.0);
                before.is_none_or(|b| k < b)
                    && low.is_none_or(|l| k >= l)
                    && since.is_none_or(|t| s.started_at >= t)
            };
            let own: Vec<&Session> = own_sessions.iter().filter(|s| in_window(s)).collect();
            let theirs: Vec<&Session> = shared.iter().filter(|s| in_window(s)).collect();
            self.load_items(&mut ctx, &events, &own, &theirs).await?;
            let batch = self.batch_rows(&ctx, &events, &own, &theirs);
            rows.extend(
                batch
                    .into_iter()
                    .filter(|r| keep(r, &q, &kinds, people.as_ref())),
            );
            before = low;
            if low.is_none() {
                done = true;
            }
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

    async fn feed_context(
        &self,
        actor: Actor,
        own: &[Session],
        shared: &[Session],
        self_companion: CompanionId,
    ) -> AppResult<Ctx> {
        let mut mine = HashSet::from([self_companion]);
        for c in self.repos.companions.linked_to(actor.user_id).await? {
            mine.insert(c.id);
        }
        let mut names = HashMap::new();
        let mut owners = HashMap::new();
        for s in own.iter().chain(shared) {
            for p in &s.participants {
                if let std::collections::hash_map::Entry::Vacant(v) = names.entry(p.companion_id)
                    && let Some(c) = self.repos.companions.get(p.companion_id).await?
                {
                    if c.merged_into.is_some_and(|m| mine.contains(&m)) {
                        mine.insert(c.id);
                    }
                    v.insert(c.name);
                }
            }
            if s.owner != actor.user_id && !owners.contains_key(&s.owner) {
                let name = self
                    .user_opt(s.owner)
                    .await?
                    .map(|u| u.display_name)
                    .unwrap_or_default();
                owners.insert(s.owner, name);
            }
        }
        Ok(Ctx {
            items: HashMap::new(),
            names,
            mine,
            owners,
        })
    }

    async fn load_items(
        &self,
        ctx: &mut Ctx,
        events: &[ProgressEvent],
        own: &[&Session],
        theirs: &[&Session],
    ) -> AppResult<()> {
        let missing: Vec<ItemId> = events
            .iter()
            .map(|e| e.item_id)
            .chain(own.iter().chain(theirs).map(|s| s.item_id))
            .filter(|i| !ctx.items.contains_key(i))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        for i in self.repos.catalog.get_many(&missing).await? {
            ctx.items.insert(i.id, i);
        }
        Ok(())
    }

    fn batch_rows(
        &self,
        ctx: &Ctx,
        events: &[ProgressEvent],
        own: &[&Session],
        theirs: &[&Session],
    ) -> Vec<ActivityRow> {
        let now = self.now();
        let by_id: HashMap<SessionId, &Session> = own.iter().map(|s| (s.id, *s)).collect();
        let mut used: HashSet<SessionId> = HashSet::new();
        let mut rows = Vec::new();
        for e in events {
            let Some(item) = ctx.items.get(&e.item_id) else {
                continue;
            };
            let session = match e.progress {
                Progress::Session { session_id } => by_id.get(&session_id).copied(),
                _ => own.iter().copied().find(|s| {
                    s.item_id == e.item_id
                        && !used.contains(&s.id)
                        && (s.started_at - e.occurred_at).num_seconds().abs() <= PAIR_SECS
                        && !matches!(e.progress, Progress::Session { .. })
                }),
            };
            if let Some(s) = session {
                used.insert(s.id);
            }
            let row_type = if matches!(e.progress, Progress::Session { .. }) {
                ActivityType::Session
            } else {
                ActivityType::Progress
            };
            let mut row = self.row(ctx, item, (e.occurred_at, e.id.0), row_type, session);
            row.progress = (row_type == ActivityType::Progress).then(|| e.progress.clone());
            row.event_id = Some(e.id);
            row.undoable = now - e.recorded_at <= chrono::Duration::hours(UNDO_WINDOW_HOURS);
            rows.push(row);
        }
        for s in own.iter().filter(|s| !used.contains(&s.id)) {
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
                rows.push(self.row(
                    ctx,
                    item,
                    (s.started_at, s.id.0),
                    ActivityType::Session,
                    Some(s),
                ));
            }
        }
        for s in theirs {
            let Some(item) = ctx.items.get(&s.item_id) else {
                continue;
            };
            let mut row = self.row(
                ctx,
                item,
                (s.started_at, s.id.0),
                ActivityType::Session,
                Some(s),
            );
            row.shared = true;
            row.owner_name = ctx.owners.get(&s.owner).cloned();
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
