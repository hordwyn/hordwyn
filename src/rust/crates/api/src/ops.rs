//! Operations shared by the REST handlers and the web app's server functions (through the
//! server's `web::Backend` implementation): plain async functions over [`ApiState`] that call
//! `app` use-cases and return `contracts` DTOs.

use std::collections::{BTreeSet, HashSet};

use app::ports::{AppEvent, SearchQuery};
use contracts::{
    AttributionDto, CatalogItemDto, CompanionDto, ItemPageDto, LibraryEntryDto, LibraryItemDto,
    ListDto, NewProgressDto, ProgressEventDto, SearchResponseDto, SearchResultsEventDto,
};
use domain::ids::{CompanionId, ItemId, ListId};
use domain::kind::Status;
use domain::library::Rating;
use domain::list::{List, Visibility};
use domain::progress::Progress;
use domain::session::Participant;
use domain::user::Actor;

use crate::error::ApiResult;
use crate::{ApiError, ApiState, dto};

/// Topic of streamed connector search results.
pub const SEARCH_RESULTS: &str = "search.results";

/// Parse an id from a path or body.
///
/// # Errors
/// `bad_request` if it is not a UUID.
pub fn parse_id<T>(s: &str, wrap: impl FnOnce(uuid::Uuid) -> T) -> ApiResult<T> {
    uuid::Uuid::parse_str(s)
        .map(wrap)
        .map_err(|_| ApiError::bad_request(format!("invalid id `{s}`")))
}

/// The stored item; a refresh it needs is queued and its `entity.updated` reaches the
/// requester (DEC-492).
///
/// # Errors
/// Unknown item.
pub async fn item(s: &ApiState, a: Actor, id: &str) -> ApiResult<CatalogItemDto> {
    let item = s.app.item_detail_for(a, parse_id(id, ItemId)?).await?;
    Ok(dto::item(s, &item).await)
}

/// Item page: item (refresh queued if stale), the viewer's entry, effective history,
/// attribution, tags, progress summary and next step.
///
/// # Errors
/// Unknown item.
pub async fn item_page(s: &ApiState, a: Actor, id: &str) -> ApiResult<ItemPageDto> {
    let item_id = parse_id(id, ItemId)?;
    // the page hears about its item while it is open, also a stub not in the library (DEC-490);
    // before the refresh is queued, so its event cannot come first; best effort (an unknown
    // item fails below)
    let _ = s.app.watch_item(a, item_id).await;
    let item = s.app.item_detail(item_id).await?;
    let entry = s.app.entry(a, item_id).await?;
    let progress = s.app.history(a, item_id).await?;
    // the row from the entry and history already read (UX1-10)
    let state = match &entry {
        Some(e) => Some(crate::browse::item_state(
            s.app.entry_row(e.clone(), item.clone(), &progress).await?,
        )),
        None => None,
    };
    // an item outside the library has its first step too (OT-4), and the count of whole
    // passes is the page's alone (OT-7). A stored item of a kind this deployment switched
    // off has neither, and still has its page (as before the step was added)
    let kind = s.app.kind(&item.kind).ok();
    let completed = entry
        .as_ref()
        .is_some_and(|e| e.status.value.as_str() == domain::kind::Status::COMPLETED);
    let times = kind
        .as_ref()
        .map_or(0, |k| app::times(completed, k, &item, &progress));
    let times_at = kind
        .as_ref()
        .and_then(|k| app::pass_at(k, &item, &progress))
        .map(|t| t.to_rfc3339());
    let first = kind.as_ref().filter(|_| entry.is_none()).map(|k| {
        let step = app::first_step(k, &item, &progress);
        crate::browse::next_step(&step, k.group, &item.kind)
    });
    let namespaces: BTreeSet<String> = item
        .external_ids
        .iter()
        .map(|e| e.namespace.clone())
        .chain(item.provenance.values().cloned())
        .collect();
    let sources = sources(s)
        .into_iter()
        .filter(|a| namespaces.contains(&a.connector))
        .collect();
    let episodes = s
        .app
        .catalog_episodes(item_id)
        .await?
        .into_iter()
        .map(|e| contracts::EpisodeDateDto {
            season: e.season,
            episode: e.number,
            air_date: e.air_date.map(|d| d.format("%Y-%m-%d").to_string()),
        })
        .collect();
    // who played, the scores, the winner and the duration of the item's plays (OT-11)
    let plays = s
        .app
        .item_plays(a, &progress)
        .await?
        .iter()
        .map(crate::browse::item_play)
        .collect();
    Ok(ItemPageDto {
        item: dto::item(s, &item).await,
        entry: entry.as_ref().map(dto::entry),
        progress: progress.iter().map(dto::progress).collect(),
        sources,
        tags: state.as_ref().map(|x| x.2.clone()).unwrap_or_default(),
        summary: state.as_ref().map(|x| x.0.clone()),
        next_step: state.map(|x| x.1).or(first),
        times,
        times_at,
        episodes,
        plays,
    })
}

/// The kinds a search covers: one kind, the enabled kinds of one kind group (the palette's
/// scope segments, design.md 6.4), or every enabled kind.
fn search_kinds(s: &ApiState, kind: Option<String>, group: Option<&str>) -> ApiResult<Vec<String>> {
    if let Some(k) = kind {
        return Ok(vec![k]);
    }
    let Some(g) = group.filter(|g| !g.is_empty() && *g != "all") else {
        return Ok(s.default_kinds.clone());
    };
    let of_group: HashSet<String> = s
        .app
        .kinds
        .all()
        .iter()
        .filter(|d| d.group.as_str() == g)
        .map(|d| d.id.to_string())
        .collect();
    let kinds: Vec<String> = s
        .default_kinds
        .iter()
        .filter(|k| of_group.contains(*k))
        .cloned()
        .collect();
    if kinds.is_empty() {
        return Err(ApiError::bad_request("unknown kind group"));
    }
    Ok(kinds)
}

/// Local results now; connector results follow as `search.results` events. `group` limits
/// the search to one kind group (`all` or none: every enabled kind).
///
/// # Errors
/// Empty query, unknown kind group.
pub async fn search(
    s: &ApiState,
    a: Actor,
    q: &str,
    kind: Option<String>,
    group: Option<String>,
) -> ApiResult<SearchResponseDto> {
    let text = q.trim().to_string();
    if text.is_empty() {
        return Err(ApiError::bad_request("empty query"));
    }
    let kinds = search_kinds(s, kind, group.as_deref())?;
    let mut filters = Vec::new();
    if kinds.len() == 1 {
        filters.push(("kind".to_string(), kinds[0].clone()));
    }
    let hits = s
        .search
        .search(SearchQuery {
            index: "catalog".into(),
            text: text.clone(),
            viewer: a.user_id,
            filters,
            limit: 20,
        })
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "local search failed");
            vec![]
        });
    // the hits' items in one read, in the order of the hits (DEC-493)
    let ids: Vec<ItemId> = hits
        .iter()
        .filter_map(|h| uuid::Uuid::parse_str(&h.id).ok().map(ItemId))
        .collect();
    let mut found: std::collections::HashMap<ItemId, domain::catalog::CatalogItem> = s
        .app
        .repos
        .catalog
        .get_many(&ids)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "search hits unreadable");
            vec![]
        })
        .into_iter()
        .map(|i| (i.id, i))
        .collect();
    let mut items = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Some(item) = found.remove(id) {
            items.push(dto::item(s, &item).await);
        }
    }
    // the local index filters by one kind only; a group is filtered here
    if group
        .as_deref()
        .is_some_and(|g| !g.is_empty() && g != "all")
    {
        items.retain(|i| kinds.contains(&i.kind));
    }
    let search_id = uuid::Uuid::now_v7().to_string();
    let pending: Vec<String> = s
        .fanout
        .targets(&kinds)
        .into_iter()
        .map(|(c, _)| c)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let (skipped, groups_without_source) = crate::admin::search_reasons(s, a, &kinds);
    // nothing to ask: no credentials read, no task (DEC-493)
    if !pending.is_empty() {
        stream_connector_results(s, a, text, kinds, search_id.clone()).await;
    }
    Ok(SearchResponseDto {
        search_id,
        items,
        pending,
        skipped,
        groups_without_source,
    })
}

/// Ask the connectors in the background; each one's results go to the searching user as a
/// `search.results` event.
async fn stream_connector_results(
    s: &ApiState,
    a: Actor,
    text: String,
    kinds: Vec<String>,
    sid: String,
) {
    // the member's own credentials (TheTVDB PIN) go with their search (design.md 4.1)
    let user_keys = crate::admin::user_keys(s, a).await;
    let state = s.clone();
    tokio::spawn(async move {
        state
            .fanout
            .run_as(&user_keys, &text, &kinds, 20, |r| {
                let state = state.clone();
                let sid = sid.clone();
                async move {
                    let mut dtos = Vec::new();
                    for i in &r.items {
                        dtos.push(dto::item(&state, i).await);
                    }
                    let payload = SearchResultsEventDto {
                        search_id: sid,
                        connector: r.connector,
                        items: dtos,
                        error: r.error,
                    };
                    state
                        .publish(AppEvent {
                            topic: SEARCH_RESULTS.into(),
                            entity: None,
                            audience: vec![a.user_id],
                            payload: serde_json::to_value(payload).unwrap_or_default(),
                        })
                        .await;
                }
            })
            .await;
    });
}

#[must_use]
pub fn sources(s: &ApiState) -> Vec<AttributionDto> {
    s.registry
        .infos()
        .into_iter()
        .map(|i| AttributionDto {
            connector: i.id,
            name: i.name,
            text: i.attribution.text,
            url: i.attribution.url,
            licence: i.attribution.licence,
        })
        .collect()
}

/// One page of the library, most recently added first; `after` is the previous page's
/// `next` (keyset paging, DEC-416). The items are read in one batch.
///
/// # Errors
/// A malformed cursor (`bad_request`); storage errors.
pub async fn library(
    s: &ApiState,
    a: Actor,
    after: Option<&str>,
    limit: u32,
) -> ApiResult<contracts::LibraryPageDto> {
    let after = after.map(crate::browse::parse_cursor).transpose()?;
    let (entries, total, next) = s.app.library_page(a, after.as_ref(), limit).await?;
    let ids: Vec<ItemId> = entries.iter().map(|e| e.item_id).collect();
    let items: std::collections::HashMap<ItemId, domain::catalog::CatalogItem> = s
        .app
        .repos
        .catalog
        .get_many(&ids)
        .await
        .map_err(app::AppError::from)?
        .into_iter()
        .map(|i| (i.id, i))
        .collect();
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let Some(item) = items.get(&e.item_id) else {
            return Err(app::AppError::NotFound("catalog item").into());
        };
        out.push(LibraryItemDto {
            entry: dto::entry(&e),
            item: dto::item(s, item).await,
        });
    }
    Ok(contracts::LibraryPageDto {
        items: out,
        total,
        next: next.as_ref().map(app::ports::LibraryCursor::encode),
        limit: limit.clamp(1, app::App::MAX_PAGE),
    })
}

/// # Errors
/// Unknown item or a status the kind does not allow.
pub async fn add(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    status: Option<String>,
) -> ApiResult<LibraryEntryDto> {
    let id = parse_id(item_id, ItemId)?;
    let e = s.app.add_to_library(a, id, status.map(Status::new)).await?;
    s.pin_cover(id).await;
    Ok(dto::entry(&e))
}

/// # Errors
/// Unknown item or a status the kind does not allow.
pub async fn set_status(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    status: String,
) -> ApiResult<LibraryEntryDto> {
    let e = s
        .app
        .set_status(a, parse_id(item_id, ItemId)?, Status::new(status), None)
        .await?;
    Ok(dto::entry(&e))
}

/// # Errors
/// Rating out of range or unknown item.
pub async fn rate(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    rating: Option<u8>,
) -> ApiResult<LibraryEntryDto> {
    let rating = rating
        .map(Rating::new)
        .transpose()
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let e = s
        .app
        .rate(a, parse_id(item_id, ItemId)?, rating, None)
        .await?;
    Ok(dto::entry(&e))
}

/// # Errors
/// Unknown item.
pub async fn progress(s: &ApiState, a: Actor, item_id: &str) -> ApiResult<Vec<ProgressEventDto>> {
    let events = s.app.progress(a, parse_id(item_id, ItemId)?).await?;
    Ok(events.iter().map(dto::progress).collect())
}

/// Clock skew allowed for a client timestamp ahead of the server (UX1-5).
pub const CLOCK_SKEW_MINUTES: i64 = 5;
/// Longest play: a week.
pub const MAX_PLAY_MINUTES: u32 = 7 * 24 * 60;
/// Longest play location, in characters.
pub const MAX_LOCATION_CHARS: usize = 200;

/// An optional RFC 3339 timestamp from a client: 400 if it does not parse, 422 `invalid` if
/// it lies more than [`CLOCK_SKEW_MINUTES`] after the server clock.
///
/// # Errors
/// As above.
pub(crate) fn client_time(
    s: &ApiState,
    field: &'static str,
    value: Option<&str>,
) -> ApiResult<Option<domain::Timestamp>> {
    let Some(v) = value else {
        return Ok(None);
    };
    let t = chrono::DateTime::parse_from_rfc3339(v)
        .map(|d| d.with_timezone(&chrono::Utc))
        .map_err(|_| ApiError::bad_request(format!("{field} must be RFC 3339")))?;
    if t > s.app.clock.now() + chrono::Duration::minutes(CLOCK_SKEW_MINUTES) {
        return Err(invalid(field, "lies in the future".into()));
    }
    Ok(Some(t))
}

fn invalid(field: &'static str, reason: String) -> ApiError {
    app::AppError::from(domain::DomainError::invalid(field, reason)).into()
}

fn check_len(field: &'static str, value: Option<&String>, max: usize) -> ApiResult<()> {
    if value.is_some_and(|v| v.chars().count() > max) {
        return Err(invalid(field, format!("longer than {max} characters")));
    }
    Ok(())
}

/// Log progress; with `with` (companion ids) a session with those companions and the user's
/// self-companion is logged as well ("watched with"). Replays with the same `client_op_id`
/// are no-ops.
///
/// # Errors
/// Invalid progress, ids or timestamps; unknown item or companions.
pub async fn log_progress(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    b: NewProgressDto,
) -> ApiResult<ProgressEventDto> {
    let item_id = parse_id(item_id, ItemId)?;
    let progress: Progress = serde_json::from_value(b.progress)
        .map_err(|e| ApiError::bad_request(format!("progress: {e}")))?;
    let occurred_at = client_time(s, "occurred_at", b.occurred_at.as_deref())?;
    let op = b.client_op_id.map(|o| parse_id(&o, |u| u)).transpose()?;
    if !b.with.is_empty() {
        // the session is logged first: not for a view the log would refuse (a date before
        // the start of a running rewatch run, DEC-591)
        s.app
            .check_progress(a, item_id, &progress, occurred_at, op)
            .await?;
        let me = s.app.user(a.user_id).await?;
        let mut participants = vec![Participant::new(me.self_companion)];
        for c in &b.with {
            participants.push(Participant::new(parse_id(c, CompanionId)?));
        }
        let mut session = app::NewSession::new(item_id, participants);
        session.started_at = occurred_at;
        // Idempotency key of the session, derived from the client op id (replays are no-ops).
        session.client_op_id = op.map(|o| {
            let h = blake3::hash(&[o.as_bytes().as_slice(), b"session"].concat());
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&h.as_bytes()[..16]);
            uuid::Uuid::from_bytes(bytes)
        });
        s.app.log_session(a, session).await?;
    }
    let e = s
        .app
        .log_progress(a, item_id, progress, occurred_at, op)
        .await?;
    s.pin_cover(item_id).await;
    Ok(dto::progress(&e))
}

/// Log a play (sessions progress model) with players, scores and the winner; answers the
/// play's progress event.
///
/// # Errors
/// Bad ids or timestamp, foreign companions, a kind without plays.
pub async fn log_play(
    s: &ApiState,
    a: Actor,
    item_id: &str,
    b: contracts::NewPlayDto,
) -> ApiResult<ProgressEventDto> {
    let item_id = parse_id(item_id, ItemId)?;
    let mut players = Vec::new();
    for p in &b.players {
        let mut part = Participant::new(parse_id(&p.companion_id, CompanionId)?);
        part.score = p.score;
        part.winner = p.winner;
        players.push(part);
    }
    if players.is_empty() {
        players.push(Participant::new(
            s.app.user(a.user_id).await?.self_companion,
        ));
    }
    let mut play = app::NewSession::new(item_id, players);
    play.started_at = client_time(s, "started_at", b.started_at.as_deref())?;
    if b.duration_minutes.is_some_and(|m| m > MAX_PLAY_MINUTES) {
        return Err(invalid(
            "duration_minutes",
            format!("longer than {MAX_PLAY_MINUTES} minutes"),
        ));
    }
    check_len("location", b.location.as_ref(), MAX_LOCATION_CHARS)?;
    check_len("notes", b.notes.as_ref(), domain::review::MAX_REVIEW_CHARS)?;
    play.duration_minutes = b.duration_minutes;
    play.location = b.location.filter(|l| !l.trim().is_empty());
    play.notes = b.notes.filter(|n| !n.trim().is_empty());
    play.coop_outcome = match b.coop.as_deref() {
        None | Some("") => None,
        Some("won") => Some(domain::session::CoopOutcome::Won),
        Some("lost") => Some(domain::session::CoopOutcome::Lost),
        Some(_) => return Err(ApiError::bad_request("coop must be `won` or `lost`")),
    };
    play.client_op_id = b.client_op_id.map(|o| parse_id(&o, |u| u)).transpose()?;
    play.game_group = b
        .game_group_id
        .as_deref()
        .filter(|g| !g.is_empty())
        .map(|g| parse_id(g, domain::ids::GameGroupId))
        .transpose()?;
    let (_, e) = s.app.log_play(a, play).await?;
    s.pin_cover(item_id).await;
    Ok(dto::progress(&e))
}

/// # Errors
/// Storage errors.
pub async fn companions(s: &ApiState, a: Actor) -> ApiResult<Vec<CompanionDto>> {
    Ok(s.app
        .companions(a)
        .await?
        .iter()
        .map(dto::companion)
        .collect())
}

/// # Errors
/// Invalid name.
pub async fn create_companion(s: &ApiState, a: Actor, name: &str) -> ApiResult<CompanionDto> {
    Ok(dto::companion(&s.app.create_companion(a, name).await?))
}

/// A list as its owner sees it (with the public link of a link-shared list).
#[must_use]
pub fn list_dto(l: &List) -> ListDto {
    ListDto {
        id: l.id.0.to_string(),
        name: l.name.clone(),
        description: l.description.clone(),
        visibility: l.visibility.choice().as_str().into(),
        item_ids: l.items.iter().map(|i| i.item_id.0.to_string()).collect(),
        share_path: match &l.visibility {
            Visibility::Link { token } => Some(crate::sharing::shared_list_path(token)),
            _ => None,
        },
    }
}

/// # Errors
/// Storage errors.
pub async fn lists(s: &ApiState, a: Actor) -> ApiResult<Vec<ListDto>> {
    Ok(s.app.lists(a).await?.iter().map(list_dto).collect())
}

/// # Errors
/// Invalid name.
pub async fn create_list(s: &ApiState, a: Actor, name: &str) -> ApiResult<ListDto> {
    let l = s
        .app
        .create_list(a, name, None, Visibility::Private)
        .await?;
    Ok(list_dto(&l))
}

/// # Errors
/// Unknown list or item, or not the owner.
pub async fn add_to_list(
    s: &ApiState,
    a: Actor,
    list_id: &str,
    item_id: &str,
) -> ApiResult<ListDto> {
    let l = s
        .app
        .add_to_list(
            a,
            parse_id(list_id, ListId)?,
            parse_id(item_id, ItemId)?,
            None,
        )
        .await?;
    s.pin_cover(
        l.items
            .last()
            .map_or(ItemId(uuid::Uuid::nil()), |i| i.item_id),
    )
    .await;
    Ok(list_dto(&l))
}

/// The user's library as rows of the generic format, with the lists each entry is on. The
/// history is the effective one: an undone log and its undo event are not in the file (the
/// import refuses the undo type, review OT2E-8, DEC-593).
async fn export_rows(s: &ApiState, a: Actor) -> ApiResult<Vec<importers::ImportRow>> {
    let lists = s.app.lists(a).await?;
    let mut rows = Vec::new();
    for e in s.app.library(a).await? {
        let item = s.app.catalog_item(e.item_id).await?;
        let history = s
            .app
            .history(a, e.item_id)
            .await?
            .into_iter()
            .map(|p| importers::row::HistoryEntry {
                date: p.occurred_at.to_rfc3339(),
                progress: serde_json::to_value(&p.progress).ok(),
                event_id: None,
            })
            .collect();
        rows.push(importers::ImportRow {
            source_id: e.id.0.to_string(),
            kind: e.kind.to_string(),
            title: item.title,
            year: item.year,
            external_ids: item
                .external_ids
                .into_iter()
                .map(|x| (x.namespace, x.value))
                .collect(),
            status: Some(e.status.value.as_str().to_string()),
            rating: e.rating.value.map(Rating::value),
            notes: e.notes.value,
            history,
            lists: lists
                .iter()
                .filter(|l| l.items.iter().any(|i| i.item_id == e.item_id))
                .map(|l| l.name.clone())
                .collect(),
        });
    }
    Ok(rows)
}

/// The user's library in the generic format (JSON Lines), importable again.
///
/// # Errors
/// Storage errors.
pub async fn export(s: &ApiState, a: Actor) -> ApiResult<String> {
    let mut out = importers::generic::header_line();
    out.push('\n');
    for row in export_rows(s, a).await? {
        out.push_str(&serde_json::to_string(&row).unwrap_or_default());
        out.push('\n');
    }
    Ok(out)
}

/// The user's library as generic CSV (importable again; keeps finished dates only).
///
/// # Errors
/// Storage errors.
pub async fn export_csv(s: &ApiState, a: Actor) -> ApiResult<String> {
    Ok(importers::generic::csv_text(&export_rows(s, a).await?))
}

fn importer(s: &ApiState) -> ApiResult<&std::sync::Arc<importers::Importer>> {
    s.importer.as_ref().ok_or_else(|| {
        ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            "imports are not enabled",
        )
    })
}

fn import_error(e: importers::ImportError) -> ApiError {
    use axum::http::StatusCode;
    match e {
        importers::ImportError::UnknownFormat(f) => {
            ApiError::bad_request(format!("unknown import format `{f}`"))
        }
        importers::ImportError::Unknown(_) => {
            ApiError::new(StatusCode::NOT_FOUND, "not_found", "import not found")
        }
        importers::ImportError::UnknownRow(_) => {
            ApiError::new(StatusCode::NOT_FOUND, "not_found", "no such unmatched row")
        }
        importers::ImportError::Forbidden => {
            ApiError::new(StatusCode::FORBIDDEN, "forbidden", "forbidden")
        }
        importers::ImportError::Conflict { code, message } => {
            ApiError::new(StatusCode::CONFLICT, code, message)
        }
        importers::ImportError::App(a) => a.into(),
        other => {
            let msg = other.to_string();
            if msg.contains("UploadQuota") {
                ApiError::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "quota",
                    "upload quota exceeded",
                )
            } else {
                tracing::error!(error = %msg, "import failed");
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal error",
                )
            }
        }
    }
}

/// The record as the imports page reads it (`contracts::imports::ImportDto`), without the
/// internal storage key of the file and the owner id (UX3-26).
fn record(r: &importers::ImportRecord) -> serde_json::Value {
    let mut v = serde_json::to_value(r).unwrap_or_default();
    if let Some(o) = v.as_object_mut() {
        o.remove("source_key");
        o.remove("user_id");
    }
    v
}

/// Queue an import of `bytes` in `format`.
///
/// # Errors
/// Unknown format, quota.
pub async fn start_import(
    s: &ApiState,
    a: Actor,
    format: &str,
    bytes: Vec<u8>,
    dry_run: bool,
) -> ApiResult<serde_json::Value> {
    let r = importer(s)?
        .start(a, format, bytes, dry_run)
        .await
        .map_err(import_error)?;
    Ok(record(&r))
}

/// # Errors
/// Unknown import or not the owner.
pub async fn import_status(s: &ApiState, a: Actor, id: &str) -> ApiResult<serde_json::Value> {
    let id = parse_id(id, |u| u)?;
    let r = importer(s)?.status(a, id).await.map_err(import_error)?;
    Ok(record(&r))
}

/// Imports listed on the imports page.
pub const IMPORTS_LISTED: u32 = 20;

/// The user's imports, newest first (reports, previews and unmatched rows).
///
/// # Errors
/// Storage errors.
pub async fn imports(s: &ApiState, a: Actor) -> ApiResult<Vec<serde_json::Value>> {
    let list = importer(s)?
        .list(a, IMPORTS_LISTED)
        .await
        .map_err(import_error)?;
    Ok(list.iter().map(record).collect())
}

/// Run an import's file again for real (after a dry run, or as a retry).
///
/// # Errors
/// Unknown import or not the owner.
pub async fn rerun_import(s: &ApiState, a: Actor, id: &str) -> ApiResult<serde_json::Value> {
    let id = parse_id(id, |u| u)?;
    let r = importer(s)?.rerun(a, id).await.map_err(import_error)?;
    Ok(record(&r))
}

/// Ask a running import to stop at its next checkpoint.
///
/// # Errors
/// Unknown import, not the owner, finished (409 `import_finished`).
pub async fn cancel_import(s: &ApiState, a: Actor, id: &str) -> ApiResult<serde_json::Value> {
    let id = parse_id(id, |u| u)?;
    let r = importer(s)?.cancel(a, id).await.map_err(import_error)?;
    Ok(record(&r))
}

/// Match an unmatched row with a catalog item and import it.
///
/// # Errors
/// Unknown import, row or item; a dry run or another kind (409).
pub async fn match_import_row(
    s: &ApiState,
    a: Actor,
    id: &str,
    source_id: &str,
    item_id: &str,
) -> ApiResult<serde_json::Value> {
    let id = parse_id(id, |u| u)?;
    let item = parse_id(item_id, ItemId)?;
    let r = importer(s)?
        .match_row(a, id, source_id, item)
        .await
        .map_err(import_error)?;
    Ok(record(&r))
}
