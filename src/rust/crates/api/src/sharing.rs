//! Sharing and companion flows (D-9): link requests, promotion by invite, demotion, merge,
//! sessions shared with the caller and "copy to my log", per-user privacy, link-shared lists.
//! Shared by the REST handlers and the web app's server functions.

use contracts::{
    CompanionDto, InviteDto, LinkRequestDto, LoggedSessionDto, ParticipantDto, PrivacyDto,
    ShareLinkDto, SharedListDto, UserDto,
};
use domain::ids::{CompanionId, LinkRequestId, ListId, SessionId};
use domain::session::Session;
use domain::user::Actor;

use crate::error::ApiResult;
use crate::ops::parse_id;
use crate::{ApiState, dto};

/// Path of the public page of a link-shared list.
#[must_use]
pub fn shared_list_path(token: &str) -> String {
    format!("/shared/lists/{token}")
}

/// Path of the sign-up page that accepts an invite.
#[must_use]
pub fn invite_path(token: &str) -> String {
    format!("/invite/{token}")
}

/// Ask the user `username` to confirm that the companion stands for them.
///
/// # Errors
/// Unknown user or companion, or the link rules (see `App::request_link`).
pub async fn request_link(
    s: &ApiState,
    a: Actor,
    companion_id: &str,
    username: &str,
) -> ApiResult<LinkRequestDto> {
    let target = s
        .app
        .user_by_username(username.trim())
        .await?
        .ok_or(app::AppError::NotFound("user"))?;
    let req = s
        .app
        .request_link(a, parse_id(companion_id, CompanionId)?, target.id)
        .await?;
    link_request_dto(s, &req).await
}

async fn link_request_dto(
    s: &ApiState,
    r: &domain::companion::LinkRequest,
) -> ApiResult<LinkRequestDto> {
    let from = s.app.user(r.from).await?;
    let owner = Actor {
        user_id: from.id,
        role: from.role,
    };
    let companion = s.app.companion(owner, r.companion_id).await?;
    Ok(LinkRequestDto {
        id: r.id.0.to_string(),
        companion_id: r.companion_id.0.to_string(),
        companion_name: companion.name,
        from_name: from.display_name,
    })
}

/// Pending link requests addressed to the caller.
///
/// # Errors
/// Storage errors.
pub async fn link_requests(s: &ApiState, a: Actor) -> ApiResult<Vec<LinkRequestDto>> {
    let mut out = Vec::new();
    for r in s.app.link_requests(a).await? {
        out.push(link_request_dto(s, &r).await?);
    }
    Ok(out)
}

/// # Errors
/// Not addressed to the caller or no longer pending.
pub async fn accept_link(s: &ApiState, a: Actor, id: &str) -> ApiResult<CompanionDto> {
    let c = s.app.accept_link(a, parse_id(id, LinkRequestId)?).await?;
    Ok(dto::companion(&c))
}

/// # Errors
/// Not addressed to the caller or no longer pending.
pub async fn decline_link(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    Ok(s.app.decline_link(a, parse_id(id, LinkRequestId)?).await?)
}

/// Demote: end the link; the companion keeps a snapshot, history is unchanged.
///
/// # Errors
/// Neither owner nor linked user, or the self-companion.
pub async fn unlink(s: &ApiState, a: Actor, id: &str) -> ApiResult<CompanionDto> {
    let c = s
        .app
        .unlink_companion(a, parse_id(id, CompanionId)?)
        .await?;
    Ok(dto::companion(&c))
}

/// Merge the duplicate `remove` into `keep`.
///
/// # Errors
/// Not owned or a merge rule is violated.
pub async fn merge(s: &ApiState, a: Actor, keep: &str, remove: &str) -> ApiResult<CompanionDto> {
    let c = s
        .app
        .merge_companions(
            a,
            parse_id(keep, CompanionId)?,
            parse_id(remove, CompanionId)?,
        )
        .await?;
    Ok(dto::companion(&c))
}

/// Promote a companion: a single-use invite to create the account that it stands for.
///
/// # Errors
/// Not the owner, or a self, merged or linked companion.
pub async fn invite(
    s: &ApiState,
    a: Actor,
    id: &str,
    email: Option<String>,
) -> ApiResult<InviteDto> {
    let email = email
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty());
    let (invite, token) = s
        .app
        .promote_companion(a, parse_id(id, CompanionId)?, email)
        .await?;
    Ok(InviteDto {
        id: invite.id.0.to_string(),
        path: invite_path(&token),
        token,
        expires_at: invite.expires_at.to_rfc3339(),
    })
}

async fn session_dto(s: &ApiState, a: Actor, session: &Session) -> ApiResult<LoggedSessionDto> {
    let names = s.app.participant_names(a, session.id).await?;
    let item = s.app.catalog_item(session.item_id).await?;
    let owner_name = s.app.user(session.owner).await?.display_name;
    Ok(LoggedSessionDto {
        id: session.id.0.to_string(),
        item_id: session.item_id.0.to_string(),
        item_title: item.title,
        owner_name,
        mine: session.owner == a.user_id,
        started_at: session.started_at.to_rfc3339(),
        participants: session
            .participants
            .iter()
            .zip(names)
            .map(|(p, (name, is_me))| ParticipantDto {
                companion_id: p.companion_id.0.to_string(),
                name,
                is_me,
                score: p.score,
                winner: p.winner,
            })
            .collect(),
    })
}

/// Sessions the caller logged, newest first.
///
/// # Errors
/// Storage errors.
pub async fn my_sessions(s: &ApiState, a: Actor) -> ApiResult<Vec<LoggedSessionDto>> {
    let mut out = Vec::new();
    for x in s.app.my_sessions(a).await? {
        out.push(session_dto(s, a, &x).await?);
    }
    Ok(out)
}

/// Sessions of other users the caller took part in (read-only).
///
/// # Errors
/// Storage errors.
pub async fn shared_sessions(s: &ApiState, a: Actor) -> ApiResult<Vec<LoggedSessionDto>> {
    let mut out = Vec::new();
    for x in s.app.shared_sessions(a).await? {
        out.push(session_dto(s, a, &x).await?);
    }
    Ok(out)
}

/// "Copy to my log" (idempotent).
///
/// # Errors
/// Not shared with the caller, or the caller's own session.
pub async fn claim(s: &ApiState, a: Actor, id: &str) -> ApiResult<LoggedSessionDto> {
    let copy = s.app.claim_session(a, parse_id(id, SessionId)?).await?;
    session_dto(s, a, &copy).await
}

/// # Errors
/// Storage errors.
pub async fn set_privacy(s: &ApiState, a: Actor, p: PrivacyDto) -> ApiResult<UserDto> {
    Ok(dto::user(&s.app.set_privacy(a, p.share_sessions).await?))
}

/// Share a list by link (a new link revokes the previous one).
///
/// # Errors
/// Unknown list or not the owner.
pub async fn share_list(s: &ApiState, a: Actor, id: &str) -> ApiResult<ShareLinkDto> {
    let token = s.app.share_list(a, parse_id(id, ListId)?).await?;
    Ok(ShareLinkDto {
        path: shared_list_path(&token),
        token,
    })
}

/// A link-shared list, readable without an account.
///
/// # Errors
/// Unknown or revoked token.
pub async fn shared_list(s: &ApiState, token: &str) -> ApiResult<SharedListDto> {
    let list = s.app.list_by_token(token).await?;
    let owner_name = s.app.user(list.owner).await?.display_name;
    let mut items = Vec::with_capacity(list.items.len());
    for i in &list.items {
        let item = s.app.catalog_item(i.item_id).await?;
        items.push(dto::item(s, &item).await);
    }
    Ok(SharedListDto {
        name: list.name,
        description: list.description,
        owner_name,
        items,
    })
}
