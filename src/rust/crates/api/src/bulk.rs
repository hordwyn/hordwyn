//! Bulk edits of the library and removing an item, with one undo (WP4-3): REST handlers and
//! the operations the server functions share.

use app::{BulkOp, BulkOutcome, BulkUndo, MAX_BULK};
use auth::oidc::CookieKeys;
use axum::Json;
use axum::extract::{Path, State};
use contracts::{BulkRequestDto, BulkResponseDto, BulkResultDto, BulkUndoDto};
use domain::ids::{ItemId, ListId};
use domain::kind::Status;
use domain::user::Actor;

use crate::error::ApiResult;
use crate::extract::Authed;
use crate::ops::parse_id;
use crate::{ApiError, ApiState};

/// How long an undo token is accepted (the toast offering it lasts seconds).
pub const UNDO_TTL_SECS: i64 = 3600;
/// HKDF `info` of the undo token key (its own purpose beside `cookie_key`).
pub const UNDO_KEY_INFO: &[u8] = b"bulk-undo-token";
const UNDO_KIND: &str = "bulk_undo";

/// What the token carries: the undo and the person it was handed to (UX4-12).
#[derive(serde::Serialize, serde::Deserialize)]
struct Signed {
    user: uuid::Uuid,
    undo: BulkUndo,
}

/// The undo as an opaque token: HMAC-SHA256 signed (the pending-login cookie's format with a
/// key of its own purpose), bound to `user`, valid [`UNDO_TTL_SECS`] from `now`.
fn seal_undo(keys: &dyn CookieKeys, user: uuid::Uuid, now: i64, undo: &BulkUndo) -> String {
    let signed = Signed {
        user,
        undo: undo.clone(),
    };
    auth::oidc::seal_signed(keys, UNDO_KIND, &signed, now + UNDO_TTL_SECS)
}

/// The undo of a token made by [`seal_undo`] for `user` that has not expired.
fn open_undo(keys: &dyn CookieKeys, user: uuid::Uuid, now: i64, token: &str) -> Option<BulkUndo> {
    let signed: Signed = auth::oidc::open_signed(keys, UNDO_KIND, token.trim(), now).ok()?;
    (signed.user == user).then_some(signed.undo)
}

/// The undo token keys: derived from the live key ring (every loaded version verifies, so a
/// rotation does not break a pending undo); without the admin area (small test setups) a
/// random key of this process.
struct UndoKeys<'a>(&'a ApiState);

impl CookieKeys for UndoKeys<'_> {
    fn keys(&self) -> Vec<zeroize::Zeroizing<[u8; 32]>> {
        static KEY: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
        if let Some(adm) = &self.0.admin {
            return adm.keys.live.ring().derive(UNDO_KEY_INFO);
        }
        let key = KEY.get_or_init(|| {
            let mut k = [0u8; 32];
            getrandom::fill(&mut k).expect("os randomness");
            k
        });
        vec![zeroize::Zeroizing::new(*key)]
    }
}

fn now_secs(s: &ApiState) -> i64 {
    s.app.clock.now().timestamp()
}

fn op_of(b: &BulkRequestDto) -> ApiResult<BulkOp> {
    let need = |v: &Option<String>, what: &str| {
        v.clone()
            .ok_or_else(|| ApiError::bad_request(format!("`{}` needs `{what}`", b.op)))
    };
    Ok(match b.op.as_str() {
        "set_status" => BulkOp::SetStatus(Status::new(need(&b.status, "status")?)),
        "add_tag" => BulkOp::AddTag(need(&b.tag, "tag")?),
        "remove_tag" => BulkOp::RemoveTag(need(&b.tag, "tag")?),
        "add_to_list" => BulkOp::AddToList(parse_id(&need(&b.list_id, "list_id")?, ListId)?),
        "remove" => BulkOp::Remove,
        other => return Err(ApiError::bad_request(format!("unknown op `{other}`"))),
    })
}

fn response(s: &ApiState, a: Actor, out: BulkOutcome) -> BulkResponseDto {
    let results: Vec<BulkResultDto> = out
        .results
        .into_iter()
        .map(|(id, failure)| BulkResultDto {
            item_id: id.0.to_string(),
            ok: failure.is_none(),
            error: failure.map(|f| f.code().to_string()),
        })
        .collect();
    BulkResponseDto {
        done: u32::try_from(results.iter().filter(|r| r.ok).count()).unwrap_or(u32::MAX),
        results,
        undo: seal_undo(&UndoKeys(s), a.user_id.0, now_secs(s), &out.undo),
    }
}

/// One operation on many items.
///
/// # Errors
/// More than [`MAX_BULK`] ids, a malformed id or op (`bad_request`); an invalid tag
/// (`invalid`); a list of someone else (`forbidden` / `not_found`).
pub async fn bulk(s: &ApiState, a: Actor, b: BulkRequestDto) -> ApiResult<BulkResponseDto> {
    if b.item_ids.len() > MAX_BULK {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_BULK} items per request"
        )));
    }
    let op = op_of(&b)?;
    let ids = b
        .item_ids
        .iter()
        .map(|i| parse_id(i, ItemId))
        .collect::<ApiResult<Vec<_>>>()?;
    Ok(response(s, a, s.app.bulk(a, &ids, op).await?))
}

/// Remove one item from the library (history stays).
///
/// # Errors
/// Not in the library (`not_found`).
pub async fn remove(s: &ApiState, a: Actor, item_id: &str) -> ApiResult<BulkResponseDto> {
    let id = parse_id(item_id, ItemId)?;
    let undo = s.app.remove_from_library(a, id).await?;
    Ok(BulkResponseDto {
        results: vec![BulkResultDto {
            item_id: item_id.to_string(),
            ok: true,
            error: None,
        }],
        done: 1,
        undo: seal_undo(&UndoKeys(s), a.user_id.0, now_secs(s), &undo),
    })
}

/// Undo a bulk change or a removal.
///
/// # Errors
/// A token that is not ours, was changed, belongs to someone else or expired
/// (`bad_request`); one that no longer fits (`invalid`).
pub async fn undo(s: &ApiState, a: Actor, token: &str) -> ApiResult<()> {
    let undo = open_undo(&UndoKeys(s), a.user_id.0, now_secs(s), token)
        .ok_or_else(|| ApiError::bad_request("`undo` is not an undo token"))?;
    s.app.undo_bulk(a, &undo).await?;
    Ok(())
}

/// One operation on up to 500 items of the caller's library: `set_status`, `add_tag`,
/// `remove_tag`, `add_to_list`, `remove`. Ids not in the library fail alone; the answer has
/// one result per distinct id and an undo token.
#[utoipa::path(post, path = "/library/bulk", tag = "library", request_body = BulkRequestDto,
    responses((status = 200, body = BulkResponseDto), (status = 400, body = contracts::ErrorDto),
        (status = 422, body = contracts::ErrorDto)))]
pub(crate) async fn library_bulk(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<BulkRequestDto>,
) -> ApiResult<Json<BulkResponseDto>> {
    Ok(Json(bulk(&s, a, b).await?))
}

/// Put back what a bulk change or a removal changed.
#[utoipa::path(post, path = "/library/bulk/undo", tag = "library", request_body = BulkUndoDto,
    responses((status = 204), (status = 400, body = contracts::ErrorDto)))]
pub(crate) async fn library_bulk_undo(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Json(b): Json<BulkUndoDto>,
) -> ApiResult<axum::http::StatusCode> {
    undo(&s, a, &b.undo).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Remove an item from the library: the entry, its status, rating, note and tags go; plays,
/// progress, reviews and list memberships stay. Answers an undo token.
#[utoipa::path(delete, path = "/library/{item_id}", tag = "library",
    params(("item_id" = String, Path)),
    responses((status = 200, body = BulkResponseDto), (status = 404, body = contracts::ErrorDto)))]
pub(crate) async fn remove_from_library(
    State(s): State<ApiState>,
    Authed(a): Authed,
    Path(item_id): Path<String>,
) -> ApiResult<Json<BulkResponseDto>> {
    Ok(Json(remove(&s, a, &item_id).await?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth::oidc::StaticCookieKey;

    fn undo() -> BulkUndo {
        BulkUndo::TagAdded {
            tag: "Cosy".into(),
            items: vec![ItemId(uuid::Uuid::from_u128(7))],
        }
    }

    #[test]
    fn an_undo_token_is_signed_bound_to_its_user_and_expires() {
        // UX4-12: the token is no longer plain JSON a client can edit
        let key = StaticCookieKey([3; 32]);
        let (me, other) = (uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(2));
        let token = seal_undo(&key, me, 1_000, &undo());
        assert_eq!(open_undo(&key, me, 1_000, &token), Some(undo()));
        assert_eq!(open_undo(&key, other, 1_000, &token), None, "another user");
        let late = 1_000 + UNDO_TTL_SECS;
        assert_eq!(open_undo(&key, me, late, &token), None, "expired");
        assert_eq!(
            open_undo(&StaticCookieKey([4; 32]), me, 1_000, &token),
            None
        );
        // an edited body fails the signature
        let (body, tag) = token.rsplit_once('.').unwrap();
        let mut forged = body.to_string();
        forged.push('A');
        assert_eq!(open_undo(&key, me, 1_000, &format!("{forged}.{tag}")), None);
        assert_eq!(open_undo(&key, me, 1_000, "garbage"), None);
    }

    #[test]
    fn the_largest_undo_fits_the_request_body_limit() {
        let key = StaticCookieKey([3; 32]);
        let big = BulkUndo::TagAdded {
            tag: "x".repeat(app::MAX_UNDO_BYTES - 64),
            items: vec![],
        };
        let token = seal_undo(&key, uuid::Uuid::nil(), 0, &big);
        let body = serde_json::to_vec(&BulkUndoDto { undo: token }).unwrap();
        assert!(
            body.len() < crate::routes::JSON_BODY_LIMIT,
            "{}",
            body.len()
        );
    }
}
