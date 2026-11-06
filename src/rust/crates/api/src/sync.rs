//! Sync protocol (D-12): clients pull "changes since cursor" and push command batches.
//!
//! Every command is idempotent: adds are upserts, mutable fields are last-writer-wins on the
//! client's HLC stamp (an older or equal stamp loses), progress events are keyed by the
//! command's `op_id`. Replaying a batch therefore changes nothing, and clients that edited
//! the same item offline converge once both have pushed and pulled.

use contracts::{
    ErrorDto, SyncBatchDto, SyncChangeDto, SyncChangesDto, SyncCommandDto, SyncEntryDto,
    SyncResultDto,
};
use domain::hlc::Hlc;
use domain::ids::{EntryId, ItemId};
use domain::kind::Status;
use domain::library::{LibraryEntry, Rating};
use domain::progress::Progress;
use domain::user::Actor;

use crate::error::ApiResult;
use crate::ops::parse_id;
use crate::{ApiError, ApiState};

/// Largest page of changes.
pub const MAX_CHANGES: i64 = 500;

fn hlc(s: &str) -> ApiResult<Hlc> {
    s.parse()
        .map_err(|_| ApiError::bad_request(format!("invalid hlc `{s}`")))
}

#[must_use]
pub fn entry_dto(e: &LibraryEntry) -> SyncEntryDto {
    SyncEntryDto {
        id: e.id.0.to_string(),
        item_id: e.item_id.0.to_string(),
        kind: e.kind.to_string(),
        status: e.status.value.as_str().to_string(),
        status_hlc: e.status.stamp.to_string(),
        rating: e.rating.value.map(Rating::value),
        rating_hlc: e.rating.stamp.to_string(),
        notes: e.notes.value.clone(),
        notes_hlc: e.notes.stamp.to_string(),
    }
}

async fn apply(s: &ApiState, a: Actor, op_id: &str, c: SyncCommandDto) -> ApiResult<()> {
    match c {
        SyncCommandDto::Add { item_id } => {
            s.app
                .add_to_library(a, parse_id(&item_id, ItemId)?, None)
                .await?;
        }
        SyncCommandDto::SetStatus {
            item_id,
            status,
            hlc: h,
        } => {
            s.app
                .set_status(
                    a,
                    parse_id(&item_id, ItemId)?,
                    Status::new(status),
                    Some(hlc(&h)?),
                )
                .await?;
        }
        SyncCommandDto::Rate {
            item_id,
            rating,
            hlc: h,
        } => {
            let rating = rating
                .map(Rating::new)
                .transpose()
                .map_err(|e| ApiError::bad_request(e.to_string()))?;
            s.app
                .rate(a, parse_id(&item_id, ItemId)?, rating, Some(hlc(&h)?))
                .await?;
        }
        SyncCommandDto::SetNotes {
            item_id,
            notes,
            hlc: h,
        } => {
            s.app
                .set_notes(a, parse_id(&item_id, ItemId)?, notes, Some(hlc(&h)?))
                .await?;
        }
        SyncCommandDto::LogProgress {
            item_id,
            progress,
            occurred_at,
        } => {
            let progress: Progress = serde_json::from_value(progress)
                .map_err(|e| ApiError::bad_request(format!("progress: {e}")))?;
            let at = occurred_at
                .map(|t| {
                    chrono::DateTime::parse_from_rfc3339(&t)
                        .map(|d| d.with_timezone(&chrono::Utc))
                        .map_err(|_| ApiError::bad_request("occurred_at must be RFC 3339"))
                })
                .transpose()?;
            s.app
                .log_progress(
                    a,
                    parse_id(&item_id, ItemId)?,
                    progress,
                    at,
                    Some(parse_id(op_id, |u| u)?),
                )
                .await?;
        }
    }
    Ok(())
}

/// Apply a batch; one result per command, in order. A failing command does not stop the
/// others.
pub async fn push(s: &ApiState, a: Actor, batch: SyncBatchDto) -> Vec<SyncResultDto> {
    let mut out = Vec::with_capacity(batch.commands.len());
    for c in batch.commands {
        let r = apply(s, a, &c.op_id, c.command).await;
        out.push(SyncResultDto {
            op_id: c.op_id,
            ok: r.is_ok(),
            error: r.err().map(|e| ErrorDto {
                code: e.code.into(),
                message: e.message,
            }),
        });
    }
    out
}

/// Changes after `cursor` (exclusive), with the current state of library entries.
///
/// # Errors
/// Storage errors, or sync not available.
pub async fn pull(s: &ApiState, a: Actor, cursor: i64, limit: i64) -> ApiResult<SyncChangesDto> {
    let feed = s.changes.as_ref().ok_or_else(|| {
        ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            "sync is not available",
        )
    })?;
    let limit = limit.clamp(1, MAX_CHANGES);
    let rows = feed
        .changes_since(a.user_id, cursor, limit + 1)
        .await
        .map_err(app::AppError::from)?;
    let more = rows.len() > usize::try_from(limit).unwrap_or(usize::MAX);
    let mut changes = Vec::new();
    for c in rows.into_iter().take(usize::try_from(limit).unwrap_or(0)) {
        let entry = if c.entity == "library_entry" && c.op != "delete" {
            s.app
                .repos
                .library
                .get_by_id(EntryId(c.id))
                .await
                .map_err(app::AppError::from)?
                .filter(|e| e.user_id == a.user_id)
                .map(|e| entry_dto(&e))
        } else {
            None
        };
        changes.push(SyncChangeDto {
            seq: c.seq,
            entity: c.entity,
            id: c.id.to_string(),
            op: c.op,
            entry,
        });
    }
    Ok(SyncChangesDto {
        cursor: changes.last().map_or(cursor, |c| c.seq),
        changes,
        more,
    })
}
