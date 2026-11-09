//! DTOs of UX phase 1: kinds and groups, library counts and queries, rows with the next
//! step, tags, the activity feed, statistics and Home. Numbers and timestamps are plain
//! (locale-neutral); labels are Fluent keys the UI translates.

use serde::{Deserialize, Serialize};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

use crate::{CatalogItemDto, LibraryEntryDto};

/// A media kind as the UI needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct KindDto {
    pub id: String,
    /// `watch`, `read`, `listen`, `play` or `other`.
    pub group: String,
    /// Fluent key of the kind's name, `kind-<id>`.
    pub label_key: String,
    /// `single`, `episodic`, `paged`, `timed` or `sessions`.
    pub progress_model: String,
    /// Allowed statuses, core first.
    pub statuses: Vec<String>,
    /// Attribute names usable as library filters (`facets` of a library query).
    pub facets: Vec<String>,
}

/// An activity group with its kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct KindGroupDto {
    pub id: String,
    /// Fluent key, `kind-group-<id>`.
    pub label_key: String,
    pub kinds: Vec<String>,
}

/// Every enabled kind and the groups in display order (Watch, Read, Listen, Play, then
/// Other when a kind is in it).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct KindsDto {
    pub groups: Vec<KindGroupDto>,
    pub kinds: Vec<KindDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CountDto {
    /// Group id, kind id or status.
    pub key: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct CountCellDto {
    pub kind: String,
    pub group: String,
    pub status: String,
    pub count: u64,
}

/// Counts for the group switcher, kind chips and status tabs of the signed-in user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LibraryCountsDto {
    pub total: u64,
    /// Watch, Read, Listen, Play always (0 included); `other` only when not empty.
    pub groups: Vec<CountDto>,
    /// Kinds with entries.
    pub kinds: Vec<CountDto>,
    /// Statuses with entries, core statuses first.
    pub statuses: Vec<CountDto>,
    /// Non-empty `(kind, status)` cells: sum them for counts under a selection.
    pub cells: Vec<CountCellDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct FacetFilterDto {
    pub attribute: String,
    pub value: String,
}

/// A library query, the Library page's URL state. Every field is optional.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(default)]
pub struct LibraryQueryDto {
    /// Group id (`watch`, ...).
    pub group: Option<String>,
    pub kinds: Vec<String>,
    pub statuses: Vec<String>,
    /// Tags as shown (case does not matter); an entry must carry all.
    pub tags: Vec<String>,
    pub facets: Vec<FacetFilterDto>,
    /// Text filter (title, original title, note, review), answered by the search index; at
    /// most the best 100 matches are considered.
    pub q: Option<String>,
    /// `last_activity` (default), `title`, `added`, `year`, `rating`, `plays`.
    pub sort: Option<String>,
    /// Default: titles A to Z, everything else newest or highest first.
    pub descending: Option<bool>,
    /// Continue after the previous page: its `next` (opaque; keyset paging, DEC-416). A
    /// cursor made for another sort or direction is refused.
    pub after: Option<String>,
    /// 1 to 500; 0 or absent: 48.
    pub limit: u32,
}

/// What the primary button does next (design.md 3.4).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NextStepDto {
    /// `finish`, `episode`, `caught_up`, `page`, `position` or `session`.
    pub action: String,
    /// Fluent key of the button label: `next-step-watched`, `next-step-watched-again`,
    /// `next-step-episode` (`$season`, `$episode`), `next-step-caught-up`,
    /// `next-step-update-page`, `next-step-update-position`, `next-step-log-play`,
    /// `next-step-log-session`.
    pub label_key: String,
    /// Finished before (Single): "Watched again".
    pub again: bool,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    /// Current page and chapter; totals when the item knows them.
    pub page: Option<u32>,
    pub chapter: Option<u32>,
    pub pages: Option<u32>,
    pub chapters: Option<u32>,
    /// Current and total position in seconds.
    pub seconds: Option<u64>,
    pub total_seconds: Option<u64>,
    /// Sessions logged so far.
    pub plays: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct EpisodeRefDto {
    pub season: u32,
    pub episode: u32,
}

/// Where the user stands with an item, from its effective history.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ProgressSummaryDto {
    pub events: u32,
    /// Finished events, rewatches included.
    pub finished: u32,
    /// Distinct episodes watched and episode events (with rewatches).
    pub episodes_watched: u32,
    pub episode_views: u32,
    /// Total episodes when the item knows them.
    pub episodes_total: Option<u32>,
    /// Highest episode watched.
    pub last_episode: Option<EpisodeRefDto>,
    pub page: Option<u32>,
    pub chapter: Option<u32>,
    pub volume: Option<u32>,
    pub pages_read: u64,
    pub position_seconds: Option<u64>,
    pub seconds_listened: u64,
    pub sessions: u32,
    /// RFC 3339.
    pub first_at: Option<String>,
    pub last_at: Option<String>,
    /// 0..=1 for a progress bar, when it can be derived.
    pub fraction: Option<f32>,
}

/// A library card or table row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LibraryRowDto {
    pub entry: LibraryEntryDto,
    pub item: CatalogItemDto,
    pub group: String,
    /// RFC 3339: latest progress event, else when added.
    pub last_activity: String,
    pub tags: Vec<String>,
    pub summary: ProgressSummaryDto,
    pub next_step: NextStepDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct LibraryRowsDto {
    pub rows: Vec<LibraryRowDto>,
    /// Entries matching the query (all pages).
    pub total: u64,
    /// Pass as `after` for the next page; absent on the last page.
    pub next: Option<String>,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct TagDto {
    pub tag: String,
    /// Entries carrying it.
    pub entries: u64,
}

/// Body of `POST /library/{item_id}/tags`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct TagBodyDto {
    pub tag: String,
}

/// Body of `PUT /tags/{tag}` (rename; onto an existing tag merges them).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct RenameTagDto {
    pub name: String,
}

/// Body of `PUT /library/{item_id}/note`: the private note, `null` clears it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct NoteDto {
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ActivityPersonDto {
    pub companion_id: String,
    pub name: String,
    /// Stands for the viewer.
    pub is_me: bool,
}

/// One activity row (a sentence in the UI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ActivityRowDto {
    /// RFC 3339; group rows by the viewer's local day from it.
    pub at: String,
    /// The day of `at` in the viewer's time zone (`2026-02-01`, DEC-239), computed on the
    /// server so the browser needs no time zone database.
    #[serde(default)]
    pub local_day: String,
    /// The time of `at` in the viewer's time zone (`00:30`).
    #[serde(default)]
    pub local_time: String,
    /// `progress` or `session`.
    pub row_type: String,
    pub item_id: String,
    pub title: String,
    pub kind: String,
    pub group: String,
    /// The progress (`{"type": "episode", "season": 2, "episode": 5}`, ...) of a
    /// `progress` row.
    #[cfg_attr(feature = "openapi", schema(value_type = Option<Object>))]
    pub progress: Option<serde_json::Value>,
    /// Own rows: the event, for undo.
    pub event_id: Option<String>,
    pub session_id: Option<String>,
    pub participants: Vec<ActivityPersonDto>,
    /// Shared with the viewer by a linked user ("Copy to my log").
    pub shared: bool,
    pub owner_name: Option<String>,
    pub undoable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ActivityPageDto {
    pub rows: Vec<ActivityRowDto>,
    /// Pass as `before` for the next page; `null` at the end.
    pub next: Option<String>,
}

/// Filters of the activity feed (query parameters of `GET /activity`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(default)]
pub struct ActivityQueryDto {
    /// `next` of the previous page.
    pub before: Option<String>,
    /// 1 to 100, default 20.
    pub limit: Option<u32>,
    pub group: Option<String>,
    pub kind: Option<String>,
    /// Companion id.
    pub person: Option<String>,
    /// `progress` or `session`; absent: both.
    pub row_type: Option<String>,
    /// Only sessions shared with you.
    pub shared: bool,
    /// Calendar year (UTC).
    pub year: Option<i32>,
}

/// Activity amounts; `minutes` only where derivable (runtimes, positions, session lengths).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct AmountsDto {
    pub events: u64,
    /// Events that completed an entry.
    pub completions: u64,
    pub finished: u64,
    pub episodes: u64,
    pub pages: u64,
    pub plays: u64,
    pub minutes: u64,
}

/// All-time totals of one group (or `all`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct TotalsDto {
    /// Group id, or `all` for the overall totals.
    pub group: String,
    pub entries: u64,
    pub planned: u64,
    pub in_progress: u64,
    pub completed: u64,
    pub paused: u64,
    pub dropped: u64,
    pub amounts: AmountsDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct StatsTotalsDto {
    pub overall: TotalsDto,
    /// Watch, Read, Listen, Play; Other when it has entries.
    pub groups: Vec<TotalsDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct SeriesPointDto {
    /// `2026-03` (month) or `2026` (year), UTC.
    pub bucket: String,
    pub amounts: AmountsDto,
}

/// Activity over time for the Stats page; buckets without activity are left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct StatsSeriesDto {
    /// Group id, or `all`.
    pub group: String,
    /// `month` or `year`.
    pub bucket: String,
    pub points: Vec<SeriesPointDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct UpNextDto {
    pub row: LibraryRowDto,
    /// `next_in_series` or `planned`.
    pub reason: String,
}

/// Everything the signed-in Home page shows, in one answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct HomeDto {
    /// Entries in the library; 0 means the empty state.
    pub library_total: u64,
    /// Up to 6 in-progress entries, most recent activity first.
    pub continue_rows: Vec<LibraryRowDto>,
    /// Up to 6 planned entries.
    pub up_next: Vec<UpNextDto>,
    /// The first 5 activity rows; `next` set when there are more (link to History).
    pub activity: ActivityPageDto,
    pub stats: StatsTotalsDto,
}

/// One player of a logged play.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct PlayerDto {
    pub companion_id: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub winner: bool,
}

/// Body of `POST /items/{id}/plays`: a play or gaming session of a kind with the sessions
/// progress model (design.md 5.4, the play sheet).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(default)]
pub struct NewPlayDto {
    /// RFC 3339; defaults to now.
    pub started_at: Option<String>,
    pub duration_minutes: Option<u32>,
    pub location: Option<String>,
    pub notes: Option<String>,
    /// Players (companion ids, your own companion included); empty: only you.
    pub players: Vec<PlayerDto>,
    /// Co-op games: `won` or `lost`; absent for competitive plays.
    pub coop: Option<String>,
    /// Idempotency key (UUID) of the play's progress event.
    pub client_op_id: Option<String>,
    /// Logged as this game group of the caller.
    pub game_group_id: Option<String>,
}

/// Body of `POST /library/bulk` (design.md 3.5 bulk bar, WP4-3): one operation on up to 500
/// items of the caller's library.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(default)]
pub struct BulkRequestDto {
    /// Catalog item ids; at most 500 (more: 400). The same id twice counts once.
    pub item_ids: Vec<String>,
    /// `set_status` (needs `status`), `add_tag` / `remove_tag` (need `tag`), `add_to_list`
    /// (needs `list_id`, a list of the caller), or `remove` (from the library; history stays).
    pub op: String,
    pub status: Option<String>,
    pub tag: Option<String>,
    pub list_id: Option<String>,
}

/// The outcome for one id of a bulk request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct BulkResultDto {
    pub item_id: String,
    /// Changed, or already as asked.
    pub ok: bool,
    /// When not ok: `not_in_library`, `status_not_allowed` or `too_many_tags`.
    pub error: Option<String>,
}

/// Answer of `POST /library/bulk` and `DELETE /library/{item_id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct BulkResponseDto {
    /// One per distinct id, in request order.
    pub results: Vec<BulkResultDto>,
    /// Ids that are ok.
    pub done: u32,
    /// Opaque and signed, for the caller only, valid one hour; `POST /library/bulk/undo` with
    /// it puts back what this change did (a change made since stays).
    pub undo: String,
}

/// Body of `POST /library/bulk/undo`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct BulkUndoDto {
    pub undo: String,
}
