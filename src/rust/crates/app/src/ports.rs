//! Ports owned by the application core (D-4). Adapters live in their own crates; in-memory
//! fakes for every port live in `testkit`.
//!
//! All port inputs and outputs are plain data so adapters can be swapped (Postgres, SQLite
//! for the offline client, WASM connectors later).

use async_trait::async_trait;
use domain::Timestamp;
use domain::catalog::{CatalogItem, ExternalId};
use domain::companion::{Companion, Invite, LinkRequest};
use domain::hlc::Hlc;
use domain::ids::{
    CompanionId, EntryId, EventId, InviteId, ItemId, LinkRequestId, ListId, ReviewId, SessionId,
    UserId,
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
}

#[async_trait]
pub trait CompanionRepo: Send + Sync {
    async fn insert(&self, c: &Companion) -> RepoResult<()>;
    async fn get(&self, id: CompanionId) -> RepoResult<Option<Companion>>;
    async fn update(&self, c: &Companion) -> RepoResult<()>;
    /// Update several companions atomically (merge).
    async fn update_many(&self, cs: &[Companion]) -> RepoResult<()>;
    async fn delete(&self, id: CompanionId) -> RepoResult<()>;
    /// Active and merged companions of `owner`.
    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<Companion>>;
    /// Companions of any owner linked to `user`.
    async fn linked_to(&self, user: UserId) -> RepoResult<Vec<Companion>>;
}

#[async_trait]
pub trait LinkRequestRepo: Send + Sync {
    async fn insert(&self, r: &LinkRequest) -> RepoResult<()>;
    async fn get(&self, id: LinkRequestId) -> RepoResult<Option<LinkRequest>>;
    async fn update(&self, r: &LinkRequest) -> RepoResult<()>;
    async fn pending_for(&self, user: UserId) -> RepoResult<Vec<LinkRequest>>;
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
    async fn get(&self, id: ItemId) -> RepoResult<Option<CatalogItem>>;
    async fn by_external_id(
        &self,
        kind: &KindId,
        id: &ExternalId,
    ) -> RepoResult<Option<CatalogItem>>;
    /// The items with these ids that exist, in any order.
    async fn get_many(&self, ids: &[ItemId]) -> RepoResult<Vec<CatalogItem>>;
}

#[async_trait]
pub trait LibraryRepo: Send + Sync {
    async fn get(&self, user: UserId, item: ItemId) -> RepoResult<Option<LibraryEntry>>;
    async fn get_by_id(&self, id: EntryId) -> RepoResult<Option<LibraryEntry>>;
    /// Insert or replace by `(user, item)`.
    async fn upsert(&self, entry: &LibraryEntry) -> RepoResult<()>;
    async fn by_user(&self, user: UserId) -> RepoResult<Vec<LibraryEntry>>;
    /// One page of the user's library, most recently added first, and the total count.
    async fn page(
        &self,
        user: UserId,
        offset: u64,
        limit: u32,
    ) -> RepoResult<(Vec<LibraryEntry>, u64)>;
    /// Is the item in anyone's library (pins its cover, D-8)?
    async fn any_for_item(&self, item: ItemId) -> RepoResult<bool>;
    /// One page of the user's entries matching `filter`, in its order, and the number of
    /// matching entries.
    async fn query(
        &self,
        user: UserId,
        filter: &LibraryFilter,
    ) -> RepoResult<(Vec<LibraryHit>, u64)>;
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
    pub offset: u64,
    pub limit: u32,
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
    /// The user's statistics per kind and UTC month, by the rules of
    /// [`crate::month_amounts`] (sessions counted with the user's own sessions' durations).
    async fn month_amounts(&self, user: UserId) -> RepoResult<Vec<crate::MonthAmounts>>;
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
}

#[async_trait]
pub trait SessionRepo: Send + Sync {
    async fn insert(&self, s: &Session) -> RepoResult<()>;
    async fn get(&self, id: SessionId) -> RepoResult<Option<Session>>;
    async fn update(&self, s: &Session) -> RepoResult<()>;
    async fn delete(&self, id: SessionId) -> RepoResult<()>;
    async fn by_owner(&self, owner: UserId) -> RepoResult<Vec<Session>>;
    /// Sessions in which any of `companions` participates.
    async fn with_participants(&self, companions: &[CompanionId]) -> RepoResult<Vec<Session>>;
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
