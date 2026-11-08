//! In-memory implementation of every repository port.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use app::ports::{
    CatalogRepo, CompanionRepo, HistoryWindow, InviteRepo, LibraryFilter, LibraryHit, LibraryRepo,
    LibrarySort, LinkRequestRepo, ListRepo, ProgressRepo, RepoError, RepoResult, ReviewRepo,
    SessionRepo, UserRepo,
};
use async_trait::async_trait;
use domain::catalog::{CatalogItem, ExternalId};
use domain::companion::{Companion, Invite, LinkRequest, RequestState};
use domain::ids::{
    CompanionId, EntryId, EventId, InviteId, ItemId, LinkRequestId, ListId, ReviewId, SessionId,
    UserId,
};
use domain::kind::{KindId, Status};
use domain::library::LibraryEntry;
use domain::list::{List, Visibility};
use domain::progress::ProgressEvent;
use domain::review::Review;
use domain::session::Session;
use domain::user::User;

#[derive(Default)]
pub struct State {
    pub users: HashMap<UserId, User>,
    pub companions: HashMap<CompanionId, Companion>,
    pub link_requests: HashMap<LinkRequestId, LinkRequest>,
    pub invites: HashMap<InviteId, Invite>,
    pub catalog: HashMap<ItemId, CatalogItem>,
    pub library: HashMap<EntryId, LibraryEntry>,
    pub progress: Vec<ProgressEvent>,
    pub reviews: HashMap<ReviewId, Review>,
    pub lists: HashMap<ListId, List>,
    pub sessions: HashMap<SessionId, Session>,
    pub tags: HashMap<EntryId, Vec<String>>,
}

#[derive(Default)]
pub struct MemStore {
    state: Mutex<State>,
}

impl MemStore {
    /// Number of catalog items (tests).
    #[must_use]
    pub fn catalog_len(&self) -> usize {
        self.state().catalog.len()
    }

    /// Direct access for assertions.
    ///
    /// # Panics
    /// If the lock is poisoned.
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("lock")
    }
}

#[async_trait]
impl UserRepo for MemStore {
    async fn insert_with_self(&self, user: &User, self_companion: &Companion) -> RepoResult<()> {
        let mut s = self.state();
        if s.users.values().any(|u| u.username == user.username) {
            return Err(RepoError::Duplicate("username is taken".into()));
        }
        s.users.insert(user.id, user.clone());
        s.companions
            .insert(self_companion.id, self_companion.clone());
        Ok(())
    }

    async fn get(&self, id: UserId) -> RepoResult<Option<User>> {
        Ok(self.state().users.get(&id).cloned())
    }

    async fn by_username(&self, username: &str) -> RepoResult<Option<User>> {
        Ok(self
            .state()
            .users
            .values()
            .find(|u| u.username == username)
            .cloned())
    }

    async fn count(&self) -> RepoResult<u64> {
        Ok(self.state().users.len() as u64)
    }

    async fn list(&self) -> RepoResult<Vec<User>> {
        let mut v: Vec<User> = self.state().users.values().cloned().collect();
        v.sort_by_key(|u| (u.created_at, u.id.0));
        Ok(v)
    }

    async fn update(&self, user: &User) -> RepoResult<()> {
        self.state().users.insert(user.id, user.clone());
        Ok(())
    }

    async fn delete(&self, id: UserId) -> RepoResult<()> {
        let mut s = self.state();
        s.users.remove(&id);
        s.companions.retain(|_, c| c.owner != id);
        s.library.retain(|_, e| e.user_id != id);
        s.progress.retain(|e| e.user_id != id);
        s.reviews.retain(|_, r| r.user_id != id);
        s.lists.retain(|_, l| l.owner != id);
        s.sessions.retain(|_, x| x.owner != id);
        s.link_requests.retain(|_, r| r.from != id && r.to != id);
        Ok(())
    }
}

#[async_trait]
impl CompanionRepo for MemStore {
    async fn insert(&self, c: &Companion) -> RepoResult<()> {
        self.state().companions.insert(c.id, c.clone());
        Ok(())
    }

    async fn get(&self, id: CompanionId) -> RepoResult<Option<Companion>> {
        Ok(self.state().companions.get(&id).cloned())
    }

    async fn update(&self, c: &Companion) -> RepoResult<()> {
        self.state().companions.insert(c.id, c.clone());
        Ok(())
    }

    async fn update_many(&self, cs: &[Companion]) -> RepoResult<()> {
        let mut s = self.state();
        for c in cs {
            s.companions.insert(c.id, c.clone());
        }
        Ok(())
    }

    async fn delete(&self, id: CompanionId) -> RepoResult<()> {
        self.state().companions.remove(&id);
        Ok(())
    }

    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<Companion>> {
        Ok(self
            .state()
            .companions
            .values()
            .filter(|c| c.owner == owner)
            .cloned()
            .collect())
    }

    async fn linked_to(&self, user: UserId) -> RepoResult<Vec<Companion>> {
        Ok(self
            .state()
            .companions
            .values()
            .filter(|c| c.linked_user == Some(user))
            .cloned()
            .collect())
    }
}

#[async_trait]
impl LinkRequestRepo for MemStore {
    async fn insert(&self, r: &LinkRequest) -> RepoResult<()> {
        self.state().link_requests.insert(r.id, r.clone());
        Ok(())
    }

    async fn get(&self, id: LinkRequestId) -> RepoResult<Option<LinkRequest>> {
        Ok(self.state().link_requests.get(&id).cloned())
    }

    async fn update(&self, r: &LinkRequest) -> RepoResult<()> {
        self.state().link_requests.insert(r.id, r.clone());
        Ok(())
    }

    async fn pending_for(&self, user: UserId) -> RepoResult<Vec<LinkRequest>> {
        Ok(self
            .state()
            .link_requests
            .values()
            .filter(|r| r.to == user && r.state == RequestState::Pending)
            .cloned()
            .collect())
    }

    async fn pending_from(&self, user: UserId) -> RepoResult<Vec<LinkRequest>> {
        let mut out: Vec<LinkRequest> = self
            .state()
            .link_requests
            .values()
            .filter(|r| r.from == user && r.state == RequestState::Pending)
            .cloned()
            .collect();
        out.sort_by_key(|r| (r.created_at, r.id.0));
        Ok(out)
    }
}

#[async_trait]
impl InviteRepo for MemStore {
    async fn insert(&self, i: &Invite) -> RepoResult<()> {
        self.state().invites.insert(i.id, i.clone());
        Ok(())
    }

    async fn get(&self, id: InviteId) -> RepoResult<Option<Invite>> {
        Ok(self.state().invites.get(&id).cloned())
    }

    async fn by_token_hash(&self, hash: &str) -> RepoResult<Option<Invite>> {
        Ok(self
            .state()
            .invites
            .values()
            .find(|i| i.token_hash == hash)
            .cloned())
    }

    async fn update(&self, i: &Invite) -> RepoResult<()> {
        self.state().invites.insert(i.id, i.clone());
        Ok(())
    }
}

#[async_trait]
impl CatalogRepo for MemStore {
    async fn upsert(&self, item: &CatalogItem) -> RepoResult<()> {
        self.state().catalog.insert(item.id, item.clone());
        Ok(())
    }

    async fn get(&self, id: ItemId) -> RepoResult<Option<CatalogItem>> {
        Ok(self.state().catalog.get(&id).cloned())
    }

    async fn by_external_id(
        &self,
        kind: &KindId,
        id: &ExternalId,
    ) -> RepoResult<Option<CatalogItem>> {
        Ok(self
            .state()
            .catalog
            .values()
            .find(|i| &i.kind == kind && i.external_ids.contains(id))
            .cloned())
    }

    async fn get_many(&self, ids: &[ItemId]) -> RepoResult<Vec<CatalogItem>> {
        let s = self.state();
        Ok(ids
            .iter()
            .filter_map(|i| s.catalog.get(i).cloned())
            .collect())
    }
}

#[async_trait]
impl LibraryRepo for MemStore {
    async fn get(&self, user: UserId, item: ItemId) -> RepoResult<Option<LibraryEntry>> {
        Ok(self
            .state()
            .library
            .values()
            .find(|e| e.user_id == user && e.item_id == item)
            .cloned())
    }

    async fn get_by_id(&self, id: EntryId) -> RepoResult<Option<LibraryEntry>> {
        Ok(self.state().library.get(&id).cloned())
    }

    async fn upsert(&self, entry: &LibraryEntry) -> RepoResult<()> {
        let mut s = self.state();
        s.library.retain(|_, e| {
            !(e.user_id == entry.user_id && e.item_id == entry.item_id) || e.id == entry.id
        });
        s.library.insert(entry.id, entry.clone());
        Ok(())
    }

    async fn any_for_item(&self, item: ItemId) -> RepoResult<bool> {
        Ok(self.state().library.values().any(|e| e.item_id == item))
    }

    async fn query(&self, user: UserId, f: &LibraryFilter) -> RepoResult<(Vec<LibraryHit>, u64)> {
        let s = self.state();
        let mut hits: Vec<(SortKey, LibraryHit)> = s
            .library
            .values()
            .filter(|e| e.user_id == user && s.hit_matches(e, f))
            .map(|e| {
                let last = s
                    .effective_for(user, e.item_id)
                    .iter()
                    .map(|x| x.occurred_at)
                    .max()
                    .map_or(e.added_at, |t| t.max(e.added_at));
                let hit = LibraryHit {
                    entry: e.clone(),
                    last_activity: last,
                };
                (sort_key(&s, &hit, f.sort), hit)
            })
            .collect();
        // values first in the chosen direction, missing values (no year, no rating) last
        hits.sort_by(|(a, _), (b, _)| {
            let missing = |k: &SortKey| k.0.is_none() && k.1.is_none();
            missing(a).cmp(&missing(b)).then_with(|| {
                let o = (&a.0, a.1, a.2).cmp(&(&b.0, b.1, b.2));
                if f.descending { o.reverse() } else { o }
            })
        });
        let total = hits.len() as u64;
        let page = hits
            .into_iter()
            .map(|(_, h)| h)
            .skip(usize::try_from(f.offset).unwrap_or(usize::MAX))
            .take(f.limit as usize)
            .collect();
        Ok((page, total))
    }

    async fn counts(&self, user: UserId) -> RepoResult<Vec<(KindId, Status, u64)>> {
        let mut m: HashMap<(KindId, Status), u64> = HashMap::new();
        for e in self.state().library.values().filter(|e| e.user_id == user) {
            *m.entry((e.kind.clone(), e.status.value.clone()))
                .or_default() += 1;
        }
        Ok(m.into_iter().map(|((k, s), n)| (k, s, n)).collect())
    }

    async fn tags_of(&self, entries: &[EntryId]) -> RepoResult<Vec<(EntryId, String)>> {
        let s = self.state();
        let mut out: Vec<(EntryId, String)> = entries
            .iter()
            .flat_map(|e| {
                s.tags
                    .get(e)
                    .into_iter()
                    .flatten()
                    .map(move |t| (*e, t.clone()))
            })
            .collect();
        out.sort_by_key(|(e, t)| (*e, domain::tag::tag_key(t)));
        Ok(out)
    }

    async fn set_tags(&self, entry: &LibraryEntry, tags: &[String]) -> RepoResult<()> {
        self.state().tags.insert(entry.id, tags.to_vec());
        Ok(())
    }

    async fn tag_counts(&self, user: UserId) -> RepoResult<Vec<(String, u64)>> {
        let s = self.state();
        let mut m: std::collections::BTreeMap<String, (String, u64)> =
            std::collections::BTreeMap::new();
        // the form on the earliest added entry names the tag
        let mut mine: Vec<&LibraryEntry> =
            s.library.values().filter(|e| e.user_id == user).collect();
        mine.sort_by_key(|e| (e.added_at, e.id));
        for e in mine {
            {
                for t in s.tags.get(&e.id).into_iter().flatten() {
                    m.entry(domain::tag::tag_key(t))
                        .or_insert_with(|| (t.clone(), 0))
                        .1 += 1;
                }
            }
        }
        Ok(m.into_values().filter(|(_, n)| *n > 0).collect())
    }

    async fn rename_tag(&self, user: UserId, from_key: &str, to: &str) -> RepoResult<u64> {
        let mut s = self.state();
        let mine: Vec<EntryId> = s
            .library
            .values()
            .filter(|e| e.user_id == user)
            .map(|e| e.id)
            .collect();
        let to_key = domain::tag::tag_key(to);
        let mut changed = 0;
        for id in mine {
            if let Some(tags) = s.tags.get_mut(&id)
                && tags.iter().any(|t| domain::tag::tag_key(t) == from_key)
            {
                tags.retain(|t| {
                    let k = domain::tag::tag_key(t);
                    k != from_key && k != to_key
                });
                tags.push(to.to_string());
                tags.sort_by_key(|t| domain::tag::tag_key(t));
                changed += 1;
            }
        }
        Ok(changed)
    }

    async fn next_in_series(
        &self,
        user: UserId,
        kinds: &[KindId],
        limit: u32,
    ) -> RepoResult<Vec<ItemId>> {
        let s = self.state();
        let series = |e: &LibraryEntry| {
            s.catalog
                .get(&e.item_id)
                .and_then(app::series_of)
                .filter(|_| e.user_id == user && kinds.contains(&e.kind))
        };
        let mut best: HashMap<String, f64> = HashMap::new();
        for e in s.library.values() {
            if e.status.value == Status::completed()
                && let Some((name, pos)) = series(e)
            {
                let b = best.entry(name).or_insert(pos);
                *b = b.max(pos);
            }
        }
        let mut hits: Vec<&LibraryEntry> = s
            .library
            .values()
            .filter(|e| e.status.value == Status::planned())
            .filter(|e| {
                series(e).is_some_and(|(name, pos)| {
                    best.get(&name).is_some_and(|b| *b < pos && pos <= *b + 1.0)
                })
            })
            .collect();
        hits.sort_by_key(|e| std::cmp::Reverse((e.added_at, e.id.0)));
        Ok(hits
            .into_iter()
            .take(limit as usize)
            .map(|e| e.item_id)
            .collect())
    }

    async fn by_user(&self, user: UserId) -> RepoResult<Vec<LibraryEntry>> {
        let mut v: Vec<_> = self
            .state()
            .library
            .values()
            .filter(|e| e.user_id == user)
            .cloned()
            .collect();
        v.sort_by_key(|e| e.added_at);
        Ok(v)
    }

    async fn page(
        &self,
        user: UserId,
        offset: u64,
        limit: u32,
    ) -> RepoResult<(Vec<LibraryEntry>, u64)> {
        let mut v = LibraryRepo::by_user(self, user).await?;
        v.sort_by_key(|e| std::cmp::Reverse((e.added_at, e.id.0)));
        let total = v.len() as u64;
        let page = v
            .into_iter()
            .skip(usize::try_from(offset).unwrap_or(usize::MAX))
            .take(limit as usize)
            .collect();
        Ok((page, total))
    }
}

#[async_trait]
impl ProgressRepo for MemStore {
    async fn append(&self, e: &ProgressEvent) -> RepoResult<ProgressEvent> {
        let mut s = self.state();
        if let Some(op) = e.client_op_id
            && let Some(existing) = s
                .progress
                .iter()
                .find(|x| x.user_id == e.user_id && x.client_op_id == Some(op))
        {
            return Ok(existing.clone());
        }
        s.progress.push(e.clone());
        Ok(e.clone())
    }

    async fn for_item(&self, user: UserId, item: ItemId) -> RepoResult<Vec<ProgressEvent>> {
        let mut v: Vec<_> = self
            .state()
            .progress
            .iter()
            .filter(|e| e.user_id == user && e.item_id == item)
            .cloned()
            .collect();
        v.sort_by_key(|e| e.occurred_at);
        Ok(v)
    }

    async fn get(&self, id: EventId) -> RepoResult<Option<ProgressEvent>> {
        Ok(self.state().progress.iter().find(|e| e.id == id).cloned())
    }

    async fn by_user(&self, user: UserId) -> RepoResult<Vec<ProgressEvent>> {
        let mut v: Vec<_> = self
            .state()
            .progress
            .iter()
            .filter(|e| e.user_id == user)
            .cloned()
            .collect();
        v.sort_by_key(|e| (e.occurred_at, e.id));
        Ok(v)
    }

    async fn month_amounts(&self, user: UserId, zone: &str) -> RepoResult<Vec<app::MonthAmounts>> {
        let zone = app::time_zone::parse_zone(zone).unwrap_or(app::time_zone::Tz::UTC);
        let events = ProgressRepo::by_user(self, user).await?;
        let s = self.state();
        let durations: HashMap<SessionId, u64> = s
            .sessions
            .values()
            .filter(|x| x.owner == user)
            .map(|x| (x.id, u64::from(x.duration_minutes.unwrap_or(0))))
            .collect();
        Ok(app::month_amounts(&s.catalog, &events, &durations, zone))
    }

    async fn for_items(&self, user: UserId, items: &[ItemId]) -> RepoResult<Vec<ProgressEvent>> {
        let mut v = ProgressRepo::by_user(self, user).await?;
        v.retain(|e| items.contains(&e.item_id));
        Ok(v)
    }

    async fn recent(&self, user: UserId, w: &HistoryWindow) -> RepoResult<Vec<ProgressEvent>> {
        let all = ProgressRepo::by_user(self, user).await?;
        let s = self.state();
        let mut v: Vec<ProgressEvent> = domain::progress::effective(&all)
            .into_iter()
            .filter(|e| w.before.is_none_or(|b| (e.occurred_at, e.id.0) < b))
            .filter(|e| w.since.is_none_or(|t| e.occurred_at >= t))
            .filter(|e| {
                w.kinds.is_empty()
                    || s.catalog
                        .get(&e.item_id)
                        .is_some_and(|i| w.kinds.contains(&i.kind))
            })
            .cloned()
            .collect();
        v.sort_by_key(|e| std::cmp::Reverse((e.occurred_at, e.id.0)));
        v.truncate(w.limit as usize);
        Ok(v)
    }
}

#[async_trait]
impl ReviewRepo for MemStore {
    async fn upsert(&self, r: &Review) -> RepoResult<()> {
        self.state().reviews.insert(r.id, r.clone());
        Ok(())
    }

    async fn get(&self, user: UserId, item: ItemId) -> RepoResult<Option<Review>> {
        Ok(self
            .state()
            .reviews
            .values()
            .find(|r| r.user_id == user && r.item_id == item)
            .cloned())
    }

    async fn delete(&self, id: ReviewId) -> RepoResult<()> {
        self.state().reviews.remove(&id);
        Ok(())
    }

    async fn for_item(&self, item: ItemId) -> RepoResult<Vec<Review>> {
        Ok(self
            .state()
            .reviews
            .values()
            .filter(|r| r.item_id == item)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl ListRepo for MemStore {
    async fn insert(&self, l: &List) -> RepoResult<()> {
        self.state().lists.insert(l.id, l.clone());
        Ok(())
    }

    async fn get(&self, id: ListId) -> RepoResult<Option<List>> {
        Ok(self.state().lists.get(&id).cloned())
    }

    async fn update(&self, l: &List) -> RepoResult<()> {
        self.state().lists.insert(l.id, l.clone());
        Ok(())
    }

    async fn delete(&self, id: ListId) -> RepoResult<()> {
        self.state().lists.remove(&id);
        Ok(())
    }

    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<List>> {
        Ok(self
            .state()
            .lists
            .values()
            .filter(|l| l.owner == owner)
            .cloned()
            .collect())
    }

    async fn by_share_token(&self, token: &str) -> RepoResult<Option<List>> {
        Ok(self
            .state()
            .lists
            .values()
            .find(|l| matches!(&l.visibility, Visibility::Link { token: t } if t == token))
            .cloned())
    }

    async fn instance_visible(&self) -> RepoResult<Vec<List>> {
        Ok(self
            .state()
            .lists
            .values()
            .filter(|l| l.visibility == Visibility::Instance)
            .cloned()
            .collect())
    }
}

#[async_trait]
impl SessionRepo for MemStore {
    async fn insert(&self, s: &Session) -> RepoResult<()> {
        self.state().sessions.insert(s.id, s.clone());
        Ok(())
    }

    async fn get(&self, id: SessionId) -> RepoResult<Option<Session>> {
        Ok(self.state().sessions.get(&id).cloned())
    }

    async fn update(&self, s: &Session) -> RepoResult<()> {
        self.state().sessions.insert(s.id, s.clone());
        Ok(())
    }

    async fn delete(&self, id: SessionId) -> RepoResult<()> {
        self.state().sessions.remove(&id);
        Ok(())
    }

    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<Session>> {
        Ok(self
            .state()
            .sessions
            .values()
            .filter(|s| s.owner == owner)
            .cloned()
            .collect())
    }

    async fn with_participants(&self, companions: &[CompanionId]) -> RepoResult<Vec<Session>> {
        Ok(self
            .state()
            .sessions
            .values()
            .filter(|s| {
                s.participants
                    .iter()
                    .any(|p| companions.contains(&p.companion_id))
            })
            .cloned()
            .collect())
    }
}

// ---------------------------------------------------------------- library queries

fn facet_matches(item: &CatalogItem, attr: &str, value: &str) -> bool {
    let text = |v: &serde_json::Value| match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    match item.attributes.get(attr) {
        Some(serde_json::Value::Array(a)) => a.iter().any(|v| text(v) == value),
        Some(v) => text(v) == value,
        None => false,
    }
}

impl State {
    fn effective_for(&self, user: UserId, item: ItemId) -> Vec<&ProgressEvent> {
        let all: Vec<ProgressEvent> = self
            .progress
            .iter()
            .filter(|e| e.user_id == user && e.item_id == item)
            .cloned()
            .collect();
        let live: std::collections::HashSet<_> = domain::progress::effective(&all)
            .into_iter()
            .map(|e| e.id)
            .collect();
        self.progress
            .iter()
            .filter(|e| live.contains(&e.id))
            .collect()
    }

    fn hit_matches(&self, e: &LibraryEntry, f: &LibraryFilter) -> bool {
        let tags: Vec<String> = self
            .tags
            .get(&e.id)
            .map(|t| t.iter().map(|x| domain::tag::tag_key(x)).collect())
            .unwrap_or_default();
        let item = self.catalog.get(&e.item_id);
        (f.kinds.is_empty() || f.kinds.contains(&e.kind))
            && (f.statuses.is_empty() || f.statuses.contains(&e.status.value))
            && f.tags.iter().all(|t| tags.contains(t))
            && f.facets
                .iter()
                .all(|(k, v)| item.is_some_and(|i| facet_matches(i, k, v)))
            && f.items.as_ref().is_none_or(|ids| ids.contains(&e.item_id))
    }
}

type SortKey = (Option<String>, Option<i64>, uuid::Uuid);

fn sort_key(s: &State, h: &LibraryHit, sort: LibrarySort) -> SortKey {
    let item = s.catalog.get(&h.entry.item_id);
    let num = match sort {
        LibrarySort::LastActivity => Some(h.last_activity.timestamp_micros()),
        LibrarySort::Added => Some(h.entry.added_at.timestamp_micros()),
        LibrarySort::Year => item.and_then(|i| i.year).map(i64::from),
        LibrarySort::Rating => h.entry.rating.value.map(|r| i64::from(r.value())),
        LibrarySort::Plays => Some(
            i64::try_from(
                s.effective_for(h.entry.user_id, h.entry.item_id)
                    .iter()
                    .filter(|e| matches!(e.progress, domain::progress::Progress::Session { .. }))
                    .count(),
            )
            .unwrap_or(i64::MAX),
        ),
        LibrarySort::Title => None,
    };
    let title = (sort == LibrarySort::Title)
        .then(|| item.map_or_else(String::new, |i| i.title.to_lowercase()));
    (title, num, h.entry.id.0)
}
