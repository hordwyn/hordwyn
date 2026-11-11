//! Ports owned by the application core (D-4). Adapters live in their own crates; in-memory
//! fakes for every port live in `testkit`.
//!
//! All port inputs and outputs are plain data so adapters can be swapped (Postgres, SQLite
//! for the offline client, WASM connectors later).

use async_trait::async_trait;
use domain::Timestamp;
use domain::catalog::{CatalogItem, ExternalId};
use domain::companion::{Companion, Invite, LinkRequest};
use domain::game_group::GameGroup;
use domain::goal::Goal;
use domain::hlc::Hlc;
use domain::ids::{
    CompanionId, EntryId, EventId, GameGroupId, GoalId, InviteId, ItemId, LinkRequestId, ListId,
    ReviewId, SessionId, UserId,
};
use domain::kind::{KindDescriptor, KindId, Status};
use domain::library::LibraryEntry;
use domain::list::List;
use domain::progress::ProgressEvent;
use domain::review::Review;
use domain::session::Session;
use domain::user::User;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RepoError {
    /// A uniqueness constraint was violated (e.g. username taken).
    #[error("duplicate: {0}")]
    Duplicate(String),
    #[error("backend: {0}")]
    Backend(String),
}

pub type RepoResult<T> = Result<T, RepoError>;

/// The [`RepoError::Backend`] text of [`CatalogRepo::insert_new`] when an external id was claimed
/// by a concurrent insert and released again before its owner could be read (the caller
/// retries only this, UX4-18).
pub const CLAIM_RACE: &str = "external id claimed and released concurrently";

// ---------------------------------------------------------------- repositories

#[async_trait]
pub trait UserRepo: Send + Sync {
    /// Insert a user together with their self-companion (one transaction).
    async fn insert_with_self(&self, user: &User, self_companion: &Companion) -> RepoResult<()>;
    async fn get(&self, id: UserId) -> RepoResult<Option<User>>;
    async fn by_username(&self, username: &str) -> RepoResult<Option<User>>;
    /// Number of accounts (the first one becomes admin).
    async fn count(&self) -> RepoResult<u64>;
    /// Every account, oldest first (admin users page).
    async fn list(&self) -> RepoResult<Vec<User>>;
    async fn update(&self, user: &User) -> RepoResult<()>;
    /// Delete the user and everything they own. Companions of other users are untouched.
    async fn delete(&self, id: UserId) -> RepoResult<()>;
    /// The user's yearly goals (private: only ever read for their owner), any order.
    async fn goals(&self, user: UserId) -> RepoResult<Vec<Goal>>;
    async fn goal(&self, id: GoalId) -> RepoResult<Option<Goal>>;
    /// Insert or replace by id; another goal of the same user, year, group and measure is
    /// [`RepoError::Duplicate`].
    async fn upsert_goal(&self, goal: &Goal) -> RepoResult<()>;
    async fn delete_goal(&self, id: GoalId) -> RepoResult<()>;
}

#[async_trait]
pub trait CompanionRepo: Send + Sync {
    async fn insert(&self, c: &Companion) -> RepoResult<()>;
    async fn get(&self, id: CompanionId) -> RepoResult<Option<Companion>>;
    /// The companions of these ids that exist, in any order.
    async fn get_many(&self, ids: &[CompanionId]) -> RepoResult<Vec<Companion>>;
    async fn update(&self, c: &Companion) -> RepoResult<()>;
    /// Update several companions atomically (merge).
    async fn update_many(&self, cs: &[Companion]) -> RepoResult<()>;
    async fn delete(&self, id: CompanionId) -> RepoResult<()>;
    /// Active and merged companions of `owner`.
    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<Companion>>;
    /// Companions of any owner linked to `user`.
    async fn linked_to(&self, user: UserId) -> RepoResult<Vec<Companion>>;
    /// Game groups of `owner`, removed ones included, any order.
    async fn game_groups(&self, owner: UserId) -> RepoResult<Vec<GameGroup>>;
    async fn game_group(&self, id: GameGroupId) -> RepoResult<Option<GameGroup>>;
    /// Insert or replace a group with its members (one transaction).
    async fn upsert_game_group(&self, g: &GameGroup) -> RepoResult<()>;
}

#[async_trait]
pub trait LinkRequestRepo: Send + Sync {
    async fn insert(&self, r: &LinkRequest) -> RepoResult<()>;
    async fn get(&self, id: LinkRequestId) -> RepoResult<Option<LinkRequest>>;
    async fn update(&self, r: &LinkRequest) -> RepoResult<()>;
    async fn pending_for(&self, user: UserId) -> RepoResult<Vec<LinkRequest>>;
    /// Pending requests `user` sent, oldest first.
    async fn pending_from(&self, user: UserId) -> RepoResult<Vec<LinkRequest>>;
}

#[async_trait]
pub trait InviteRepo: Send + Sync {
    async fn insert(&self, i: &Invite) -> RepoResult<()>;
    async fn get(&self, id: InviteId) -> RepoResult<Option<Invite>>;
    async fn by_token_hash(&self, hash: &str) -> RepoResult<Option<Invite>>;
    async fn update(&self, i: &Invite) -> RepoResult<()>;
}

#[async_trait]
pub trait CatalogRepo: Send + Sync {
    /// Insert or replace by id.
    async fn upsert(&self, item: &CatalogItem) -> RepoResult<()>;
    /// Insert a new item and claim all its external ids atomically (DEC-445). When another
    /// item already holds one of them (also one a concurrent insert committed meanwhile),
    /// nothing is written and that item's id is returned.
    /// `obtained_at` stamps when the data was obtained (catalog retention, DEC-453).
    async fn insert_new(
        &self,
        item: &CatalogItem,
        obtained_at: Timestamp,
    ) -> RepoResult<Option<ItemId>>;
    /// Items holding an external id in one of `namespaces` whose connector data was obtained
    /// before `before` (created then, or last refreshed then: a change of `fetched_at`
    /// restamps it), oldest first, with that time.
    async fn provider_data_before(
        &self,
        namespaces: &[String],
        before: Timestamp,
        limit: u32,
    ) -> RepoResult<Vec<(ItemId, Timestamp)>>;
    /// Drop everything connectors supplied (title, year, description, original title,
    /// attributes, cover, provenance, episodes, fetch times; the external ids stay), make the
    /// item a stub, clear its stamp and let it be fetched again from `at` on (UX4-9). Returns the
    /// cover asset it referenced.
    async fn purge_provider_data(&self, id: ItemId, at: Timestamp) -> RepoResult<Option<String>>;
    /// Delete the item when no library entry, list, session, progress event or review refers
    /// to it (catalog retention, UX4-9); checked in the same statement as the delete.
    async fn delete_unreferenced(&self, id: ItemId) -> RepoResult<Removal>;
    /// Purged items (see [`CatalogRepo::purge_provider_data`]) with an id in one of `namespaces`
    /// that may be fetched again at `now`, longest waiting first.
    async fn expired_provider_data(
        &self,
        namespaces: &[String],
        now: Timestamp,
        limit: u32,
    ) -> RepoResult<Vec<ItemId>>;
    /// A purged item may be fetched again from `next` on (a refetch was just queued).
    async fn touch_expired(&self, id: ItemId, next: Timestamp) -> RepoResult<()>;
    async fn get(&self, id: ItemId) -> RepoResult<Option<CatalogItem>>;
    async fn by_external_id(
        &self,
        kind: &KindId,
        id: &ExternalId,
    ) -> RepoResult<Option<CatalogItem>>;
    /// The items with these ids that exist, in any order.
    async fn get_many(&self, ids: &[ItemId]) -> RepoResult<Vec<CatalogItem>>;
    /// The seasons fetched for an item with their episodes, by season number (WP4-7).
    async fn episode_seasons(
        &self,
        item: ItemId,
    ) -> RepoResult<Vec<domain::catalog::EpisodeSeason>>;
    /// Replace what is stored for these seasons of an item, in one transaction.
    async fn put_episode_seasons(
        &self,
        item: ItemId,
        seasons: &[domain::catalog::EpisodeSeason],
    ) -> RepoResult<()>;
    /// Who a change of the item matters to (DEC-490): the users with it in their library,
    /// history, sessions (the owner, and linked participants when the owner shares sessions),
    /// reviews or lists, and those who opened its page and are still watching (until after
    /// `now`). `Everyone` when it is on a list visible to the whole instance.
    async fn audience(&self, id: ItemId, now: Timestamp) -> RepoResult<ItemAudience>;
    /// `user` opened the item page: changes of the item reach them until `until`. Drops the
    /// user's watches that ended before `now`.
    async fn watch(
        &self,
        id: ItemId,
        user: UserId,
        until: Timestamp,
        now: Timestamp,
    ) -> RepoResult<()>;
}

#[async_trait]
pub trait LibraryRepo: Send + Sync {
    async fn get(&self, user: UserId, item: ItemId) -> RepoResult<Option<LibraryEntry>>;
    async fn get_by_id(&self, id: EntryId) -> RepoResult<Option<LibraryEntry>>;
    /// Which of these entry ids are in use (by anyone). The default reads them one by one;
    /// stores answer in one read (UX4-12).
    async fn existing_entry_ids(&self, ids: &[EntryId]) -> RepoResult<Vec<EntryId>> {
        let mut out = Vec::new();
        for id in ids {
            if self.get_by_id(*id).await?.is_some() {
                out.push(*id);
            }
        }
        Ok(out)
    }
    /// The user's entries of these items (those in the library), in any order.
    async fn entries_for(&self, user: UserId, items: &[ItemId]) -> RepoResult<Vec<LibraryEntry>>;
    /// Insert or replace by `(user, item)`.
    async fn upsert(&self, entry: &LibraryEntry) -> RepoResult<()>;
    async fn by_user(&self, user: UserId) -> RepoResult<Vec<LibraryEntry>>;
    /// Insert or replace several entries by `(user, item)` and, where given, replace their
    /// tags (normalised, distinct by key), all in one transaction (bulk edits and their undo).
    async fn write_many(&self, writes: &[(LibraryEntry, Option<Vec<String>>)]) -> RepoResult<()>;
    /// Delete the user's entries with these ids and their tags in one transaction. Progress
    /// events, sessions, reviews and list memberships stay ("delete keeps history"). Returns
    /// the entries deleted.
    async fn remove_many(&self, user: UserId, ids: &[EntryId]) -> RepoResult<u64>;
    /// One page of the user's library, most recently added first (`added_at`, then id), the
    /// total count, and whether more follow. `after` is the `(added_at, id)` of the previous
    /// page's last row (keyset paging, DEC-416).
    async fn page(
        &self,
        user: UserId,
        after: Option<(Timestamp, EntryId)>,
        limit: u32,
    ) -> RepoResult<EntryPage>;
    /// Is the item in anyone's library (pins its cover, D-8)?
    async fn any_for_item(&self, item: ItemId) -> RepoResult<bool>;
    /// One page of the user's entries matching `filter`, in its order and strictly after
    /// `filter.after`, the number of matching entries (all pages), and the cursor of the
    /// page's last row when more follow.
    async fn query(&self, user: UserId, filter: &LibraryFilter) -> RepoResult<HitPage>;
    /// Number of the user's entries per `(kind, status)`.
    async fn counts(&self, user: UserId) -> RepoResult<Vec<(KindId, Status, u64)>>;
    /// Tags of the given entries, `(entry, tag)` pairs sorted by tag key.
    async fn tags_of(&self, entries: &[EntryId]) -> RepoResult<Vec<(EntryId, String)>>;
    /// Replace the tags of an entry (already normalised and distinct by key).
    async fn set_tags(&self, entry: &LibraryEntry, tags: &[String]) -> RepoResult<()>;
    /// Every tag of the user with the number of entries carrying it, sorted by key.
    async fn tag_counts(&self, user: UserId) -> RepoResult<Vec<(String, u64)>>;
    /// Rename a tag (by key) on all of the user's entries; entries that already carry
    /// `to` keep one. Returns the number of entries changed.
    async fn rename_tag(&self, user: UserId, from_key: &str, to: &str) -> RepoResult<u64>;
    /// Planned entries of `kinds` that continue a series: among the user's completed entries
    /// of `kinds` with the same series (see [`crate::series_of`]) the highest position is
    /// `best`, and the planned item's position is above `best` and at most `best + 1`. Most
    /// recently added first, at most `limit` item ids.
    async fn next_in_series(
        &self,
        user: UserId,
        kinds: &[KindId],
        limit: u32,
    ) -> RepoResult<Vec<ItemId>>;
}

/// The plays logged as one game group ([`SessionRepo::group_plays`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupPlays {
    pub group: domain::ids::GameGroupId,
    pub plays: u32,
    pub last_at: Timestamp,
    pub last_item: ItemId,
}

/// The plays of one item in an owner's log, with the results of the person the entries
/// `mine` stand for ([`SessionRepo::play_totals`]): `decided` plays have a result for that
/// person ([`Session::result_for`]), `wins` of them are theirs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemPlayTotals {
    pub item_id: ItemId,
    pub plays: u32,
    pub minutes: u64,
    pub decided: u32,
    pub wins: u32,
}

/// The rule of [`SessionRepo::play_totals`] over sessions already read, sorted by item id:
/// what the in-memory store answers and what the Postgres statement is tested against.
#[must_use]
pub fn fold_play_totals(
    sessions: &[Session],
    mine: &[CompanionId],
    item: Option<ItemId>,
) -> Vec<ItemPlayTotals> {
    let mut per: std::collections::BTreeMap<uuid::Uuid, ItemPlayTotals> =
        std::collections::BTreeMap::new();
    for s in sessions
        .iter()
        .filter(|s| item.is_none_or(|i| s.item_id == i))
    {
        let t = per.entry(s.item_id.0).or_insert(ItemPlayTotals {
            item_id: s.item_id,
            plays: 0,
            minutes: 0,
            decided: 0,
            wins: 0,
        });
        t.plays += 1;
        t.minutes += u64::from(s.duration_minutes.unwrap_or(0));
        if let Some(won) = s.result_for(mine) {
            t.decided += 1;
            t.wins += u32::from(won);
        }
    }
    per.into_values().collect()
}

/// Who hears about a change of a catalog item ([`CatalogRepo::audience`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemAudience {
    /// Every signed-in user (the item is on a list visible to the instance).
    Everyone,
    /// These users; none: the change is not published.
    Users(Vec<UserId>),
}

/// What [`CatalogRepo::delete_unreferenced`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Removal {
    /// Deleted; the caller deletes the stored cover.
    Deleted { cover_asset: Option<String> },
    /// Still referenced (or already gone): kept.
    Kept,
}

/// Order of a library query (design.md 3.5).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LibrarySort {
    /// Latest progress event, else when added (default, newest first).
    #[default]
    LastActivity,
    Title,
    Added,
    Year,
    Rating,
    /// Logged sessions (board games).
    Plays,
}

impl LibrarySort {
    /// The natural direction: titles A to Z, everything else highest or newest first.
    #[must_use]
    pub const fn descending_by_default(self) -> bool {
        !matches!(self, Self::Title)
    }
}

/// What a library query selects. Empty lists select everything.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryFilter {
    pub kinds: Vec<KindId>,
    pub statuses: Vec<Status>,
    /// Tag keys ([`domain::tag::tag_key`]); an entry must carry all of them.
    pub tags: Vec<String>,
    /// `(attribute, value)`: the item attribute equals the value or, for a list, contains
    /// it (compared as text). All must match.
    pub facets: Vec<(String, String)>,
    /// Only these items (the result of a text search), when set.
    pub items: Option<Vec<ItemId>>,
    pub sort: LibrarySort,
    pub descending: bool,
    /// Rows strictly after this one in the order (keyset paging, DEC-416); its sort and
    /// direction are the filter's.
    pub after: Option<LibraryCursor>,
    pub limit: u32,
}

/// The value a row is ordered by under one sort: titles as lower-case text, last activity
/// and added as times, year, rating and plays as numbers (missing ones last in both
/// directions).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortValue {
    Text(String),
    Time(Timestamp),
    Int(Option<i64>),
}

/// Where a page ends: the sort, direction and key of its last row and the row's entry id
/// (the tie-break).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LibraryCursor {
    pub sort: LibrarySort,
    pub descending: bool,
    pub key: SortValue,
    pub id: EntryId,
}

/// A page of library entries, most recently added first.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryPage {
    pub entries: Vec<LibraryEntry>,
    /// Entries in the whole library.
    pub total: u64,
    /// More entries follow the last one.
    pub more: bool,
}

/// A page of a library query.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HitPage {
    pub hits: Vec<LibraryHit>,
    /// Matching entries (all pages).
    pub total: u64,
    /// The cursor after the last hit, when more follow.
    pub next: Option<LibraryCursor>,
}

/// A library entry as a query returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryHit {
    pub entry: LibraryEntry,
    /// Latest effective progress event, or when it was added.
    pub last_activity: Timestamp,
}

/// A window of a user's history, newest first: events strictly before `before` (by
/// `(occurred_at, id)`), at or after `since`, of the given kinds (empty: all).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoryWindow {
    pub before: Option<(Timestamp, uuid::Uuid)>,
    pub since: Option<Timestamp>,
    pub kinds: Vec<KindId>,
    pub limit: u32,
}

/// Rows of a user's history in several time spans at once (the person filter of the activity
/// feed, DEC-494): each span is `[from, to)` by time, the spans do not overlap; items of
/// `kinds` (empty: all); newest first by `(time, id)`, at most `limit` in all.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistorySpans {
    pub spans: Vec<(Timestamp, Timestamp)>,
    pub kinds: Vec<KindId>,
    pub limit: u32,
}

impl HistorySpans {
    /// The window of one span (`before` the span's end with the smallest id: strictly
    /// before `to`).
    #[must_use]
    pub fn window(&self, (from, to): (Timestamp, Timestamp)) -> HistoryWindow {
        HistoryWindow {
            before: Some((to, uuid::Uuid::nil())),
            since: Some(from),
            kinds: self.kinds.clone(),
            limit: self.limit,
        }
    }
}

/// Newest first by `key`, at most `limit` (the default span reads).
fn newest<T>(mut rows: Vec<T>, limit: u32, key: impl Fn(&T) -> (Timestamp, uuid::Uuid)) -> Vec<T> {
    rows.sort_by_key(|r| std::cmp::Reverse(key(r)));
    rows.truncate(limit as usize);
    rows
}

#[async_trait]
pub trait ProgressRepo: Send + Sync {
    /// Append an event. If an event with the same `(user, client_op_id)` exists, nothing is
    /// written and the existing event is returned.
    async fn append(&self, e: &ProgressEvent) -> RepoResult<ProgressEvent>;
    async fn for_item(&self, user: UserId, item: ItemId) -> RepoResult<Vec<ProgressEvent>>;
    async fn get(&self, id: EventId) -> RepoResult<Option<ProgressEvent>>;
    /// Every event of the user (undo events included), oldest first.
    async fn by_user(&self, user: UserId) -> RepoResult<Vec<ProgressEvent>>;
    /// Every event of the user for these items (undo events included), oldest first.
    async fn for_items(&self, user: UserId, items: &[ItemId]) -> RepoResult<Vec<ProgressEvent>>;
    /// Effective events (no undo events, none that were undone) in `window`, newest first.
    async fn recent(&self, user: UserId, window: &HistoryWindow) -> RepoResult<Vec<ProgressEvent>>;
    /// Effective events in the spans `w`, as [`Self::recent`]. The default reads one window
    /// per span; stores answer in one read.
    async fn recent_spans(&self, user: UserId, w: &HistorySpans) -> RepoResult<Vec<ProgressEvent>> {
        let mut out = Vec::new();
        for &span in &w.spans {
            out.extend(self.recent(user, &w.window(span)).await?);
        }
        Ok(newest(out, w.limit, |e| (e.occurred_at, e.id.0)))
    }
    /// The user's statistics per kind and month in `zone` (an IANA name, DEC-239), by the
    /// rules of [`crate::month_amounts`] (sessions counted with the user's own sessions'
    /// durations).
    async fn month_amounts(&self, user: UserId, zone: &str)
    -> RepoResult<Vec<crate::MonthAmounts>>;
    /// The amounts of the user's effective events in `[from, to)` per kind, by the rules of
    /// [`crate::window_amounts`] (forward moves measured against the item's earlier events).
    async fn window_amounts(
        &self,
        user: UserId,
        from: Timestamp,
        to: Timestamp,
    ) -> RepoResult<Vec<crate::KindAmounts>>;
}

#[async_trait]
pub trait ReviewRepo: Send + Sync {
    /// Insert or replace by `(user, item)`.
    async fn upsert(&self, r: &Review) -> RepoResult<()>;
    async fn get(&self, user: UserId, item: ItemId) -> RepoResult<Option<Review>>;
    async fn delete(&self, id: ReviewId) -> RepoResult<()>;
    async fn for_item(&self, item: ItemId) -> RepoResult<Vec<Review>>;
}

#[async_trait]
pub trait ListRepo: Send + Sync {
    async fn insert(&self, l: &List) -> RepoResult<()>;
    async fn get(&self, id: ListId) -> RepoResult<Option<List>>;
    async fn update(&self, l: &List) -> RepoResult<()>;
    async fn delete(&self, id: ListId) -> RepoResult<()>;
    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<List>>;
    /// The list shared with visibility `link` and this token.
    async fn by_share_token(&self, token: &str) -> RepoResult<Option<List>>;
    /// Every list with visibility `instance`, of any owner.
    async fn instance_visible(&self) -> RepoResult<Vec<List>>;
}

#[async_trait]
pub trait SessionRepo: Send + Sync {
    async fn insert(&self, s: &Session) -> RepoResult<()>;
    async fn get(&self, id: SessionId) -> RepoResult<Option<Session>>;
    /// The sessions with these ids, in the order of `ids`; an id without a session is left
    /// out. The default reads them one by one; stores answer in one read (OT1A-3).
    async fn get_many(&self, ids: &[SessionId]) -> RepoResult<Vec<Session>> {
        let mut out = Vec::with_capacity(ids.len());
        for &id in ids {
            out.extend(self.get(id).await?);
        }
        Ok(out)
    }
    async fn update(&self, s: &Session) -> RepoResult<()>;
    async fn delete(&self, id: SessionId) -> RepoResult<()>;
    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<Session>>;
    /// Sessions in which any of `companions` participates.
    async fn with_participants(&self, companions: &[CompanionId]) -> RepoResult<Vec<Session>>;
    /// The owner's sessions in the window `w` (DEC-235): newest first by `(started_at, id)`,
    /// strictly before `w.before`, started at or after `w.since`, of an item of `w.kinds`
    /// (empty: any), at most `w.limit`.
    async fn by_owner_window(&self, owner: UserId, w: &HistoryWindow) -> RepoResult<Vec<Session>>;
    /// Per game group of the owner's sessions: the number of plays and the newest play
    /// (UX4-17). The default reads every session; stores answer with one grouped read.
    async fn group_plays(&self, owner: UserId) -> RepoResult<Vec<GroupPlays>> {
        let mut per: std::collections::HashMap<domain::ids::GameGroupId, GroupPlays> =
            std::collections::HashMap::new();
        for s in self.by_owner(owner).await? {
            let Some(group) = s.game_group_id else {
                continue;
            };
            let g = per.entry(group).or_insert(GroupPlays {
                group,
                plays: 0,
                last_at: s.started_at,
                last_item: s.item_id,
            });
            g.plays += 1;
            if s.started_at > g.last_at {
                g.last_at = s.started_at;
                g.last_item = s.item_id;
            }
        }
        Ok(per.into_values().collect())
    }
    /// Per item of the owner's sessions (`item`: only that one): plays, minutes and the
    /// results of the person the entries `mine` stand for, sorted by item id (ot2f). The
    /// default reads every session; stores answer with one grouped read.
    async fn play_totals(
        &self,
        owner: UserId,
        mine: &[CompanionId],
        item: Option<ItemId>,
    ) -> RepoResult<Vec<ItemPlayTotals>> {
        Ok(fold_play_totals(&self.by_owner(owner).await?, mine, item))
    }
    /// Sessions in which any of `companions` participates, in the window `w` as
    /// [`Self::by_owner_window`].
    async fn with_participants_window(
        &self,
        companions: &[CompanionId],
        w: &HistoryWindow,
    ) -> RepoResult<Vec<Session>>;
    /// The owner's sessions started in the spans `w`, as [`Self::by_owner_window`]. The
    /// default reads one window per span; stores answer in one read.
    async fn by_owner_spans(&self, owner: UserId, w: &HistorySpans) -> RepoResult<Vec<Session>> {
        let mut out = Vec::new();
        for &span in &w.spans {
            out.extend(self.by_owner_window(owner, &w.window(span)).await?);
        }
        Ok(newest(out, w.limit, |s| (s.started_at, s.id.0)))
    }
    /// Sessions with any of `companions` started in the spans `w`, as
    /// [`Self::with_participants_window`]. The default reads one window per span.
    async fn with_participants_spans(
        &self,
        companions: &[CompanionId],
        w: &HistorySpans,
    ) -> RepoResult<Vec<Session>> {
        let mut out = Vec::new();
        for &span in &w.spans {
            out.extend(
                self.with_participants_window(companions, &w.window(span))
                    .await?,
            );
        }
        Ok(newest(out, w.limit, |s| (s.started_at, s.id.0)))
    }
}

// ---------------------------------------------------------------- kinds

/// Registered media kinds (implemented by `kinds`).
pub trait KindCatalog: Send + Sync {
    fn get(&self, id: &KindId) -> Option<KindDescriptor>;
    fn all(&self) -> Vec<KindDescriptor>;
    /// Validate and normalise an attributes object against the kind's schema.
    ///
    /// # Errors
    /// A human-readable reason if the attributes are invalid or the kind is unknown.
    fn validate_attributes(&self, kind: &KindId, attributes: &Value) -> Result<Value, String>;
}

// ---------------------------------------------------------------- infrastructure

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
    /// Next hybrid logical clock stamp of this node.
    fn hlc(&self) -> Hlc;
    /// Merge a stamp received from a client (sync), so later local stamps sort after it.
    fn observe(&self, _remote: Hlc) {}
}

pub trait IdGen: Send + Sync {
    /// A new `UUIDv7`.
    fn new_id(&self) -> uuid::Uuid;
    /// A new random, URL-safe secret token (invites, share links).
    fn new_token(&self) -> String;
    /// Hash of a token for storage and lookup.
    fn hash_token(&self, token: &str) -> String;
}

/// Reference to a changed entity, used by realtime updates and the outbox.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EntityRef {
    pub entity: String,
    pub id: uuid::Uuid,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppEvent {
    /// `entity.updated`, `job.progress`, `import.progress`, ...
    pub topic: String,
    pub entity: Option<EntityRef>,
    /// Users that may receive the event; empty = instance-wide.
    pub audience: Vec<UserId>,
    pub payload: Value,
}

impl AppEvent {
    #[must_use]
    pub fn entity_updated(entity: &str, id: uuid::Uuid, audience: Vec<UserId>) -> Self {
        Self {
            topic: "entity.updated".into(),
            entity: Some(EntityRef {
                entity: entity.into(),
                id,
            }),
            audience,
            payload: Value::Null,
        }
    }
}

#[async_trait]
pub trait EventBus: Send + Sync {
    /// Best effort: delivery failures are logged by the adapter, never returned.
    async fn publish(&self, event: AppEvent);
}

/// Who receives a notification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "users", rename_all = "snake_case")]
pub enum Audience {
    Users(Vec<UserId>),
    /// Every admin of the instance.
    Admins,
}

/// What a notification is about: used by subscription filters.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subject {
    pub kind: Option<String>,
    pub item: Option<domain::ids::ItemId>,
    pub list: Option<domain::ids::ListId>,
    pub companion: Option<domain::ids::CompanionId>,
}

/// Something worth telling people about (D-17): an event of the catalogue plus the values
/// its templates use. Subscriptions, rendering, channels and delivery belong to the adapter.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Notification {
    /// Catalogue id, e.g. `social.link_request`.
    pub event: String,
    pub audience: Audience,
    /// The same event with the same key reaches a user once (dedupe), e.g. `episode:<id>:1x02`.
    pub dedupe_key: String,
    /// Template arguments.
    pub args: serde_json::Map<String, Value>,
    pub subject: Subject,
    /// App path to open, e.g. `/history`.
    pub link: Option<String>,
}

impl Notification {
    #[must_use]
    pub fn new(event: &str, audience: Audience, dedupe_key: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            audience,
            dedupe_key: dedupe_key.into(),
            args: serde_json::Map::new(),
            subject: Subject::default(),
            link: None,
        }
    }

    #[must_use]
    pub fn arg(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.args.insert(name.into(), value.into());
        self
    }

    #[must_use]
    pub fn link(mut self, path: impl Into<String>) -> Self {
        self.link = Some(path.into());
        self
    }

    #[must_use]
    pub fn subject(mut self, subject: Subject) -> Self {
        self.subject = subject;
        self
    }
}

#[async_trait]
pub trait Notifications: Send + Sync {
    /// Best effort like [`EventBus::publish`]: failures are logged by the adapter.
    async fn emit(&self, n: Notification);
}

/// Job-queue lanes in priority order (D-6.4).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    Interactive,
    UserImport,
    BackgroundRefresh,
    Prefetch,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NewJob {
    pub lane: Lane,
    /// Handler name, e.g. `catalog.refresh`.
    pub kind: String,
    pub payload: Value,
    /// Jobs with the same dedupe key are not enqueued twice while one is pending.
    pub dedupe_key: Option<String>,
    /// Higher runs first within a lane.
    pub priority: i16,
    pub run_at: Option<Timestamp>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{0}")]
pub struct PortError(pub String);

#[async_trait]
pub trait JobQueue: Send + Sync {
    /// Returns the job id, or the id of the pending job with the same dedupe key.
    async fn enqueue(&self, job: NewJob) -> Result<uuid::Uuid, PortError>;
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchDoc {
    pub id: String,
    /// Owner for per-user indexes; `None` for shared catalog documents.
    pub owner: Option<UserId>,
    pub fields: Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchQuery {
    pub index: String,
    pub text: String,
    /// The user searching; adapters always filter by this server-side (D-13).
    pub viewer: UserId,
    pub filters: Vec<(String, String)>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SearchHit {
    pub id: String,
    pub fields: Value,
}

#[async_trait]
pub trait SearchIndex: Send + Sync {
    async fn upsert(&self, index: &str, docs: Vec<SearchDoc>) -> Result<(), PortError>;
    async fn delete(&self, index: &str, ids: Vec<String>) -> Result<(), PortError>;
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchHit>, PortError>;
}

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), PortError>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PortError>;
    async fn delete(&self, keys: &[String]) -> Result<(), PortError>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HttpRequest {
    pub connector: String,
    pub op: String,
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub lane: Lane,
    /// A 401/403 answer says nothing about the instance credential: the request used a
    /// member's own credential or an unsaved candidate key (UX2-S2, UX2-S5). The governed
    /// client then keeps it out of the shared per-connector rejection state.
    #[serde(default)]
    pub skip_auth_record: bool,
    /// The member an interactive request is made for (their search), for the per-user fair
    /// share of the interactive lane in hosted mode (design.md 4.1). `None`: background work.
    #[serde(default)]
    pub user: Option<UserId>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Why an outgoing request was not answered by the upstream server.
#[derive(Debug, Error, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HttpError {
    /// Host not on the connector's allowlist, or unknown connector.
    #[error("blocked: {0}")]
    Blocked(String),
    /// The connector's circuit breaker is open; no request was sent.
    #[error("circuit open")]
    CircuitOpen,
    /// Upstream asked us to slow down for longer than we are willing to wait.
    #[error("rate limited, retry after {retry_after_ms} ms")]
    RateLimited { retry_after_ms: u64 },
    #[error("network: {0}")]
    Network(String),
}

/// Outgoing HTTP. The production adapter is the governed client in `catalog-core`
/// (allowlist, rate limit, circuit breaker, cache); connectors only ever see this port.
/// Non-2xx answers are returned as `Ok` responses.
#[async_trait]
pub trait HttpFetch: Send + Sync {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError>;
}

// ---------------------------------------------------------------- sync

/// One row of a user's change feed (D-12).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChangeRef {
    pub seq: i64,
    pub entity: String,
    pub id: uuid::Uuid,
    /// `upsert` or `delete`.
    pub op: String,
}

/// Per-user change sequence for sync clients ("changes since cursor").
#[async_trait]
pub trait ChangeFeed: Send + Sync {
    async fn changes_since(
        &self,
        user: UserId,
        cursor: i64,
        limit: i64,
    ) -> RepoResult<Vec<ChangeRef>>;
}

// ---------------------------------------------------------------- secrets at rest

/// Where an encrypted value belongs: bound into the ciphertext as associated data, so a
/// value copied to another row, column or purpose does not decrypt (addendum-1 2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretSlot<'a> {
    /// One purpose per secret field (`connector_credential`, `channel_secret`, ...).
    pub purpose: &'a str,
    /// Primary key of the row.
    pub row: &'a str,
    pub field: &'a str,
}

/// The stored value cannot be decrypted: its key version is not loaded, or the key or the
/// bytes are wrong (indistinguishable for an AEAD). Such rows are `locked`, never deleted.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("secret cannot be decrypted (key version {version:?})")]
pub struct SecretLocked {
    /// Key version named in the stored bytes, if they are long enough to carry one.
    pub version: Option<u16>,
}

/// Envelope encryption of secrets at rest (the `auth` key ring in production, a fixed key
/// in tests). Callers zero plaintext buffers after use where they can.
pub trait SecretCipher: Send + Sync {
    /// Encrypt with the active key version.
    fn seal(&self, slot: SecretSlot<'_>, plaintext: &[u8]) -> Vec<u8>;

    /// Decrypt a value written by [`SecretCipher::seal`] for the same slot.
    ///
    /// # Errors
    /// [`SecretLocked`] for an unknown key version, a wrong key or tampered bytes.
    fn open(&self, slot: SecretSlot<'_>, blob: &[u8]) -> Result<Vec<u8>, SecretLocked>;

    /// Re-wrap the value's data key with the active key version (KEK rotation); `None` when
    /// it already uses the active version.
    ///
    /// # Errors
    /// [`SecretLocked`] when the value cannot be decrypted.
    fn rewrap(&self, slot: SecretSlot<'_>, blob: &[u8]) -> Result<Option<Vec<u8>>, SecretLocked>;
}

/// One kind of stored secret (a purpose) as the rotation job and the status page see it.
/// Implemented by the store that owns the column.
#[async_trait]
pub trait SecretRows: Send + Sync {
    fn purpose(&self) -> &'static str;
    /// Encrypt values still stored in plain text (migration on start); returns the count.
    async fn seal_plaintext(&self, cipher: &dyn SecretCipher) -> RepoResult<u64>;
    /// Rows per key version.
    async fn version_counts(&self) -> RepoResult<Vec<(u16, u64)>>;
    /// Re-wrap up to `limit` rows that are not on the active version `active`, in row id
    /// order after `after` (keyset paging, so rows that stay locked never hide the rows
    /// behind them). Rows that cannot be decrypted are left as they are and counted as
    /// locked; values too short to carry a key version are skipped.
    async fn rewrap_batch(
        &self,
        cipher: &dyn SecretCipher,
        active: u16,
        after: Option<uuid::Uuid>,
        limit: u32,
    ) -> RepoResult<RewrapBatch>;
}

/// One page of [`SecretRows::rewrap_batch`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RewrapBatch {
    pub rewrapped: u64,
    pub locked: u64,
    /// Id of the last row looked at, the `after` of the next page; `None` when the page was
    /// empty (done).
    pub last: Option<uuid::Uuid>,
}
