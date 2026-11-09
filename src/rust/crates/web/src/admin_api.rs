//! Server functions of UX phase 2 (the UI packages build pages on these): data-source
//! status and credentials, "Ask admin", re-authentication (DEC-191), the setup wizard and
//! the admin area (addendum-1 3.4). Errors carry the stable code (`reauth_required`,
//! `last_admin`, ... see `contracts::admin`), read with [`crate::api::error_code`].

// `#[server]` generates the browser side of each function without an `.await` in its body.
#![allow(clippy::unused_async, clippy::unused_async_trait_impl)]

use std::collections::BTreeMap;

use contracts::UserDto;
use contracts::admin::{
    AdminInviteCreatedDto, AdminInviteDto, AdminOverviewDto, AdminSourceDto, AdminUserDto,
    AuditPageDto, AuditQueryDto, BrandCardDto, EvictionDto, InstanceSettingsDto, LinkedIdentityDto,
    MailSettingsDto, NotificationDefaultsDto, OidcAdminDto, OidcButtonDto, ReauthStatusDto,
    RecoveryCodesDto, RewrapDto, SecurityDto, SetupStateDto, SourceStatusPageDto, SourceTestDto,
    StorageJobsDto, TotpEnrolmentDto, UserCredentialDto,
};
use leptos::prelude::*;
use leptos::server_fn::codec::Json;

#[cfg(feature = "ssr")]
mod server {
    use std::sync::Arc;

    use leptos::prelude::*;

    pub use crate::api::server::{backend, code, require_token, set_session, token};

    pub fn admin() -> Result<Arc<dyn crate::admin_backend::AdminBackend>, ServerFnError> {
        use_context::<Arc<dyn crate::admin_backend::AdminBackend>>()
            .ok_or_else(|| ServerFnError::new("admin backend missing from context"))
    }

    /// The admin backend and the caller's session token.
    pub async fn admin_token()
    -> Result<(Arc<dyn crate::admin_backend::AdminBackend>, String), ServerFnError> {
        let t = require_token(backend()?.as_ref()).await?;
        Ok((admin()?, t))
    }
}

// ---------------------------------------------------------------- data sources

/// Every data source with its state (palette footer, no-results card, admin nudge).
#[server(prefix = "/_fn")]
pub async fn source_status() -> Result<SourceStatusPageDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.source_status(&t).await.map_err(server::code)
}

/// "Ask admin" for a source or a kind group.
#[server(prefix = "/_fn")]
pub async fn ask_admin(source: Option<String>, group: Option<String>) -> Result<(), ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.ask_admin(&t, contracts::admin::AskAdminDto { source, group })
        .await
        .map_err(server::code)
}

/// The member's own credentials (Settings > Data sources).
#[server(prefix = "/_fn")]
pub async fn my_credentials() -> Result<Vec<UserCredentialDto>, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.my_credentials(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn", input = Json)]
pub async fn set_my_credential(
    source: String,
    fields: BTreeMap<String, String>,
) -> Result<Vec<UserCredentialDto>, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.set_my_credential(&t, &source, fields)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn clear_my_credential(source: String) -> Result<Vec<UserCredentialDto>, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.clear_my_credential(&t, &source)
        .await
        .map_err(server::code)
}

// ---------------------------------------------------------------- re-authentication

#[server(prefix = "/_fn")]
pub async fn reauth_status() -> Result<ReauthStatusDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.reauth_status(&t).await.map_err(server::code)
}

/// Password, then TOTP or a recovery code; sets the regenerated session cookie.
#[server(prefix = "/_fn")]
pub async fn reauth(
    password: String,
    totp: Option<String>,
) -> Result<ReauthStatusDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    let (status, fresh) = a
        .reauth(
            &t,
            contracts::admin::ReauthInputDto {
                password,
                totp: totp.filter(|x| !x.is_empty()),
            },
        )
        .await
        .map_err(server::code)?;
    let b = server::backend()?;
    server::set_session(b.as_ref(), &fresh, i64::from(b.session_days()) * 86_400);
    Ok(status)
}

// ---------------------------------------------------------------- account security

/// Change the password; every other session is signed out and this one gets a new cookie.
#[server(prefix = "/_fn")]
pub async fn change_password(current: String, new: String) -> Result<(), ServerFnError> {
    let (a, t) = server::admin_token().await?;
    let fresh = a
        .change_password(&t, &current, &new)
        .await
        .map_err(server::code)?;
    let b = server::backend()?;
    server::set_session(b.as_ref(), &fresh, i64::from(b.session_days()) * 86_400);
    Ok(())
}

/// Start enrolling an authenticator (needs a fresh proof: `reauth_required`).
#[server(prefix = "/_fn")]
pub async fn totp_begin() -> Result<TotpEnrolmentDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.totp_begin(&t).await.map_err(server::code)
}

/// Turn the authenticator on with a first code; answers the recovery codes (shown once).
#[server(prefix = "/_fn")]
pub async fn totp_confirm(code: String) -> Result<RecoveryCodesDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.totp_confirm(&t, &code).await.map_err(server::code)
}

// ---------------------------------------------------------------- setup wizard

/// Where the wizard stands; `setup_done` once completed (the page answers 404 then).
#[server(prefix = "/_fn")]
pub async fn setup_state() -> Result<SetupStateDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::token(b.as_ref()).await;
    server::admin()?
        .setup_state(t.as_deref())
        .await
        .map_err(server::code)
}

/// Step 1: the first account; signs in.
#[server(prefix = "/_fn")]
pub async fn setup_account(
    username: String,
    display_name: String,
    password: String,
    email: Option<String>,
) -> Result<UserDto, ServerFnError> {
    let b = server::backend()?;
    let s = server::admin()?
        .setup_account(contracts::SignUpDto {
            username,
            display_name,
            password,
            email: email.filter(|e| !e.is_empty()),
            invite: None,
        })
        .await
        .map_err(server::code)?;
    server::set_session(b.as_ref(), &s.token, i64::from(b.session_days()) * 86_400);
    Ok(s.user)
}

#[server(prefix = "/_fn")]
pub async fn setup_hosting(hosting: String) -> Result<SetupStateDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.setup_hosting(&t, &hosting).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn setup_sources() -> Result<SetupStateDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.setup_sources(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn setup_complete() -> Result<(), ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.setup_complete(&t).await.map_err(server::code)
}

// ---------------------------------------------------------------- admin area

#[server(prefix = "/_fn")]
pub async fn admin_overview() -> Result<AdminOverviewDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.overview(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn admin_settings() -> Result<InstanceSettingsDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.settings(&t).await.map_err(server::code)
}

/// Change settings; `null` resets one to its default.
#[server(prefix = "/_fn", input = Json)]
pub async fn update_admin_settings(
    values: BTreeMap<String, serde_json::Value>,
) -> Result<InstanceSettingsDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.update_settings(&t, values).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn admin_users() -> Result<Vec<AdminUserDto>, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.users(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn set_user_role(id: String, role: String) -> Result<AdminUserDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.set_role(&t, &id, &role).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn set_user_disabled(id: String, disabled: bool) -> Result<AdminUserDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.set_disabled(&t, &id, disabled)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn delete_user(id: String) -> Result<(), ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.delete_user(&t, &id).await.map_err(server::code)
}

/// Users row menu: remove the person's authenticator (gated, DEC-336).
#[server(prefix = "/_fn")]
pub async fn reset_user_totp(id: String) -> Result<AdminUserDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.reset_user_totp(&t, &id).await.map_err(server::code)
}

/// Users row menu: unlink one of the person's providers (gated, DEC-336).
#[server(prefix = "/_fn")]
pub async fn unlink_user_identity(id: String, identity: String) -> Result<(), ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.unlink_user_identity(&t, &id, &identity)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn admin_invites() -> Result<Vec<AdminInviteDto>, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.invites(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn create_admin_invite(
    email: Option<String>,
    note: Option<String>,
) -> Result<AdminInviteCreatedDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.create_invite(&t, contracts::admin::NewAdminInviteDto { email, note })
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn revoke_admin_invite(id: String) -> Result<AdminInviteDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.revoke_invite(&t, &id).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn admin_sources() -> Result<Vec<AdminSourceDto>, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.sources(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn", input = Json)]
pub async fn set_source_credential(
    id: String,
    fields: BTreeMap<String, String>,
) -> Result<AdminSourceDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.set_source_credential(&t, &id, fields)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn clear_source_credential(id: String) -> Result<AdminSourceDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.clear_source_credential(&t, &id)
        .await
        .map_err(server::code)
}

/// Test the given fields before saving, or the stored key (`fields: None`).
#[server(prefix = "/_fn", input = Json)]
pub async fn test_source(
    id: String,
    fields: Option<BTreeMap<String, String>>,
) -> Result<SourceTestDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.test_source(&t, &id, fields).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn set_source_licence(
    id: String,
    agreement_on_file: bool,
) -> Result<AdminSourceDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.set_source_licence(&t, &id, agreement_on_file)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn storage_jobs() -> Result<StorageJobsDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.storage_jobs(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn run_eviction() -> Result<EvictionDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.run_eviction(&t).await.map_err(server::code)
}

/// Retry (`retry: true`) or discard a failed job.
#[server(prefix = "/_fn")]
pub async fn failed_job(id: String, retry: bool) -> Result<(), ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.failed_job(&t, &id, retry).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn notification_defaults() -> Result<NotificationDefaultsDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.notification_defaults(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn admin_brand() -> Result<BrandCardDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.brand(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn admin_security() -> Result<SecurityDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.security(&t).await.map_err(server::code)
}

/// "Re-encrypt now" (KEK rotation).
#[server(prefix = "/_fn")]
pub async fn rewrap_secrets() -> Result<RewrapDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.rewrap(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn", input = Json)]
pub async fn audit_log(query: AuditQueryDto) -> Result<AuditPageDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.audit(&t, query).await.map_err(server::code)
}

// ---------------------------------------------------------------- phase 3 (WP3-4)

/// What the sign-in page needs before anyone is signed in (addendum-1 4.3, 4.5).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct SignInPageDto {
    /// Enabled providers, for "Continue with <name>" (display name only, rule 11).
    pub providers: Vec<OidcButtonDto>,
    /// `auth.sign_in_mode = auto`: password sign-in only at `/sign-in/local`, for admins.
    pub auto: bool,
}

impl SignInPageDto {
    /// The provider "Try again" and the outage message name: the default one, else the first.
    #[must_use]
    pub fn default_provider(&self) -> Option<&OidcButtonDto> {
        self.providers
            .iter()
            .find(|p| p.is_default)
            .or_else(|| self.providers.first())
    }
}

#[server(prefix = "/_fn")]
pub async fn sign_in_page() -> Result<SignInPageDto, ServerFnError> {
    server::admin()?.sign_in_page().await.map_err(server::code)
}

/// Settings > Account > Linked sign-ins.
#[server(prefix = "/_fn")]
pub async fn my_identities() -> Result<Vec<LinkedIdentityDto>, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.identities(&t).await.map_err(server::code)
}

/// Admin > Sign-in and SSO: providers and the URIs to register.
#[server(prefix = "/_fn")]
pub async fn admin_oidc() -> Result<OidcAdminDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.oidc_admin(&t).await.map_err(server::code)
}

/// Admin > Instance > Mail.
#[server(prefix = "/_fn")]
pub async fn admin_mail() -> Result<MailSettingsDto, ServerFnError> {
    let (a, t) = server::admin_token().await?;
    a.mail(&t).await.map_err(server::code)
}
