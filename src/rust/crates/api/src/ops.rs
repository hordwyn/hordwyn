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

/// # Errors
/// Unknown item.
pub async fn item(s: &ApiState, id: &str) -> ApiResult<CatalogItemDto> {
    let item = s.app.item_detail(parse_id(id, ItemId)?).await?;
    Ok(dto::item(s, &item).await)
}

/// Item page: item (refresh queued if stale), the viewer's entry, history, attribution.
///
/// # Errors
/// Unknown item.
pub async fn item_page(s: &ApiState, a: Actor, id: &str) -> ApiResult<ItemPageDto> {
    let item_id = parse_id(id, ItemId)?;
    let item = s.app.item_detail(item_id).await?;
    let entry = s.app.entry(a, item_id).await?;
    let progress = s.app.progress(a, item_id).await?;
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
    Ok(ItemPageDto {
        item: dto::item(s, &item).await,
        entry: entry.as_ref().map(dto::entry),
        progress: progress.iter().map(dto::progress).collect(),
        sources,
    })
}

/// Local results now; connector results follow as `search.results` events.
///
/// # Errors
/// Empty query.
pub async fn search(
    s: &ApiState,
    a: Actor,
    q: &str,
    kind: Option<String>,
) -> ApiResult<SearchResponseDto> {
    let text = q.trim().to_string();
    if text.is_empty() {
        return Err(ApiError::bad_request("empty query"));
    }
    let kinds = kind.map_or_else(|| s.default_kinds.clone(), |k| vec![k]);
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
    let mut items = Vec::new();
    for h in hits {
        if let Ok(id) = uuid::Uuid::parse_str(&h.id)
            && let Ok(item) = s.app.catalog_item(ItemId(id)).await
        {
            items.push(dto::item(s, &item).await);
        }
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
    let state = s.clone();
    let sid = search_id.clone();
    tokio::spawn(async move {
        state
            .fanout
            .run(&text, &kinds, 20, |r| {
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
    Ok(SearchResponseDto {
        search_id,
        items,
        pending,
    })
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

/// # Errors
/// Storage errors.
pub async fn library(
    s: &ApiState,
    a: Actor,
    offset: u64,
    limit: u32,
) -> ApiResult<contracts::LibraryPageDto> {
    let (entries, total) = s.app.library_page(a, offset, limit).await?;
    let mut out = Vec::with_capacity(entries.len());
    for e in entries {
        let item = s.app.catalog_item(e.item_id).await?;
        out.push(LibraryItemDto {
            entry: dto::entry(&e),
            item: dto::item(s, &item).await,
        });
    }
    Ok(contracts::LibraryPageDto {
        items: out,
        total,
        offset,
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
    let occurred_at = b
        .occurred_at
        .map(|t| {
            chrono::DateTime::parse_from_rfc3339(&t)
                .map(|d| d.with_timezone(&chrono::Utc))
                .map_err(|_| ApiError::bad_request("occurred_at must be RFC 3339"))
        })
        .transpose()?;
    let op = b.client_op_id.map(|o| parse_id(&o, |u| u)).transpose()?;
    if !b.with.is_empty() {
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

fn list_dto(l: &List) -> ListDto {
    ListDto {
        id: l.id.0.to_string(),
        name: l.name.clone(),
        description: l.description.clone(),
        visibility: match l.visibility {
            Visibility::Private => "private".into(),
            Visibility::Instance => "instance".into(),
            Visibility::Link { .. } => "link".into(),
        },
        item_ids: l.items.iter().map(|i| i.item_id.0.to_string()).collect(),
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

/// The user's library in the generic format (JSON Lines), importable again.
///
/// # Errors
/// Storage errors.
pub async fn export(s: &ApiState, a: Actor) -> ApiResult<String> {
    let mut out = importers::generic::header_line();
    out.push('\n');
    for e in s.app.library(a).await? {
        let item = s.app.catalog_item(e.item_id).await?;
        let history = s
            .app
            .progress(a, e.item_id)
            .await?
            .into_iter()
            .map(|p| importers::row::HistoryEntry {
                date: p.occurred_at.to_rfc3339(),
                progress: serde_json::to_value(&p.progress).ok(),
            })
            .collect();
        let row = importers::ImportRow {
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
        };
        out.push_str(&serde_json::to_string(&row).unwrap_or_default());
        out.push('\n');
    }
    Ok(out)
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
        importers::ImportError::Forbidden => {
            ApiError::new(StatusCode::FORBIDDEN, "forbidden", "forbidden")
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
    Ok(serde_json::to_value(r).unwrap_or_default())
}

/// # Errors
/// Unknown import or not the owner.
pub async fn import_status(s: &ApiState, a: Actor, id: &str) -> ApiResult<serde_json::Value> {
    let id = parse_id(id, |u| u)?;
    let r = importer(s)?.status(a, id).await.map_err(import_error)?;
    Ok(serde_json::to_value(r).unwrap_or_default())
}
