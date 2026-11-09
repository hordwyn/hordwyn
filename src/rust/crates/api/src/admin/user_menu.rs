//! Admin > Users row menu (DEC-336, WP4-2): "Reset authenticator" and "Unlink provider" for
//! another person. Both need a fresh proof (DEC-191), are audited in the change's
//! transaction (addendum-1 3.4) and tell the person in their inbox; unlinking the last way
//! to sign in is refused (`last_sign_in_method`, as for the person themselves).

use app::ports::{Audience, Notification};
use contracts::admin::AdminUserDto;
use domain::ids::UserId;
use serde_json::json;
use uuid::Uuid;

use super::{
    Caller, admin_user, entry, fresh, oidc, parse_uuid, pe, proof, refused, require_admin,
    target_user,
};
use crate::error::ApiResult;
use crate::{ApiError, ApiState};

/// Tell `user` what an admin changed on their account.
async fn tell(s: &ApiState, user: UserId, event: &str, by: &str, extra: Option<(&str, String)>) {
    let mut n = Notification::new(
        event,
        Audience::Users(vec![user]),
        format!("{event}:{}", Uuid::now_v7()),
    )
    .arg("by", by.to_string())
    .link("/settings/account");
    if let Some((k, v)) = extra {
        n = n.arg(k, v);
    }
    s.app.notifications.emit(n).await;
}

/// The row menu acts on other people; one's own authenticator and sign-ins are managed in
/// Settings > Account (UX4-10).
fn not_self(c: Caller<'_>, target: &domain::user::User) -> ApiResult<()> {
    if target.id == c.actor.user_id {
        return Err(refused(
            "self_change",
            "change your own sign-in methods in your account settings",
        ));
    }
    Ok(())
}

async fn admin_name(s: &ApiState, c: Caller<'_>) -> ApiResult<String> {
    Ok(s.app.user(c.actor.user_id).await?.display_name)
}

/// Remove another person's authenticator and recovery codes (gated).
///
/// # Errors
/// Not an admin, `not_found`, `no_authenticator`, `reauth_required`.
pub async fn reset_user_totp(s: &ApiState, c: Caller<'_>, id: &str) -> ApiResult<AdminUserDto> {
    require_admin(c.actor)?;
    let target = target_user(s, id).await?;
    not_self(c, &target)?;
    let enrolled = s
        .auth
        .store()
        .credentials(target.id)
        .await?
        .is_some_and(|x| x.totp_enabled || x.totp_secret.is_some() || x.totp_locked);
    if !enrolled {
        return Err(refused(
            "no_authenticator",
            "this person has no authenticator",
        ));
    }
    // `auto` needs an admin who can still use the sign-in page (addendum-1 3.3, UX4-10)
    if target.role == domain::user::Role::Admin
        && super::sign_in_auto(s).await
        && oidc::break_glass_admins(s, Some(target.id)).await? == 0
    {
        return Err(refused(
            "auto_needs_admin_authenticator",
            "this is the only admin with a password and an authenticator while sign-in goes straight to a provider",
        ));
    }
    fresh(s, c).await?;
    let detail = json!({"username": target.username, "proof": proof(s, c.token).await});
    let row = entry(
        s,
        Some(c.actor),
        "user.totp_reset",
        Some(("user", id.to_string())),
        detail,
    )
    .await;
    s.auth.reset_totp_audited(target.id, Some(&row)).await?;
    let by = admin_name(s, c).await?;
    tell(s, target.id, "account.authenticator_reset", &by, None).await;
    admin_user(s, &target, c.actor.user_id).await
}

/// Unlink one of another person's providers (gated). Refused when it would leave the
/// account with no way to sign in.
///
/// # Errors
/// Not an admin, `not_found`, `last_sign_in_method`, `reauth_required`.
pub async fn unlink_user_identity(
    s: &ApiState,
    c: Caller<'_>,
    id: &str,
    identity: &str,
) -> ApiResult<()> {
    require_admin(c.actor)?;
    let target = target_user(s, id).await?;
    not_self(c, &target)?;
    let identity = parse_uuid(identity, "linked sign-in")?;
    let linked = oidc::linked_identities(s, target.id).await?;
    let Some(link) = linked.iter().find(|i| i.id == identity.to_string()) else {
        return Err(ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            "linked sign-in not found",
        ));
    };
    if !link.can_unlink {
        return Err(refused(
            "last_sign_in_method",
            "this is the person's only way to sign in",
        ));
    }
    fresh(s, c).await?;
    let detail = json!({
        "username": target.username, "provider": link.provider, "issuer": link.issuer,
        "proof": proof(s, c.token).await
    });
    let row = entry(
        s,
        Some(c.actor),
        "user.identity_unlinked",
        Some(("user", id.to_string())),
        detail,
    )
    .await;
    s.auth
        .store()
        .unlink_identity(target.id, identity, Some(&row))
        .await
        .map_err(pe)?;
    let by = admin_name(s, c).await?;
    tell(
        s,
        target.id,
        "account.sign_in_unlinked",
        &by,
        Some(("provider", link.provider.clone())),
    )
    .await;
    Ok(())
}
