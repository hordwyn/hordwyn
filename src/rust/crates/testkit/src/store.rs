//! In-memory implementation of every repository port.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use app::ports::{
    CatalogRepo, CompanionRepo, InviteRepo, LibraryRepo, LinkRequestRepo, ListRepo, ProgressRepo,
    RepoError, RepoResult, ReviewRepo, SessionRepo, UserRepo,
};
use async_trait::async_trait;
use domain::catalog::{CatalogItem, ExternalId};
use domain::companion::{Companion, Invite, LinkRequest, RequestState};
use domain::ids::{
    CompanionId, EntryId, InviteId, ItemId, LinkRequestId, ListId, ReviewId, SessionId, UserId,
};
use domain::kind::KindId;
use domain::library::LibraryEntry;
use domain::list::List;
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
}

#[derive(Default)]
pub struct MemStore {
    state: Mutex<State>,
}

impl MemStore {
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
