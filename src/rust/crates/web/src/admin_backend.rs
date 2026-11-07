//! The server side of the UX phase 2 server functions (`crate::admin_api`): data-source
//! status and credentials, re-authentication, the setup wizard and the admin area. The
//! server binary implements it on top of `api::admin`, like [`crate::backend::Backend`].

use std::collections::BTreeMap;

use async_trait::async_trait;
use contracts::admin::{
    AdminInviteCreatedDto, AdminInviteDto, AdminOverviewDto, AdminSourceDto, AdminUserDto,
    AskAdminDto, AuditPageDto, AuditQueryDto, BrandCardDto, EvictionDto, InstanceSettingsDto,
    NewAdminInviteDto, NotificationDefaultsDto, ReauthInputDto, ReauthStatusDto, RecoveryCodesDto,
    RewrapDto, SecurityDto, SetupStateDto, SourceStatusPageDto, SourceTestDto, StorageJobsDto,
    TotpEnrolmentDto, UserCredentialDto,
};
use contracts::{SessionDto, SignUpDto};

use crate::backend::BResult;

/// Credential fields by name (`api_key`, `pin`, ...).
pub type Fields = BTreeMap<String, String>;

#[async_trait]
pub trait AdminBackend: Send + Sync {
    // data sources (every signed-in user)
    async fn source_status(&self, token: &str) -> BResult<SourceStatusPageDto>;
    async fn ask_admin(&self, token: &str, b: AskAdminDto) -> BResult<()>;
    async fn my_credentials(&self, token: &str) -> BResult<Vec<UserCredentialDto>>;
    async fn set_my_credential(
        &self,
        token: &str,
        source: &str,
        fields: Fields,
    ) -> BResult<Vec<UserCredentialDto>>;
    async fn clear_my_credential(
        &self,
        token: &str,
        source: &str,
    ) -> BResult<Vec<UserCredentialDto>>;

    // re-authentication (DEC-191)
    async fn reauth_status(&self, token: &str) -> BResult<ReauthStatusDto>;
    /// Returns the status and the regenerated session token.
    async fn reauth(&self, token: &str, b: ReauthInputDto) -> BResult<(ReauthStatusDto, String)>;

    // account security (Settings > Account)
    /// Returns the caller's new session token (every other session is signed out).
    async fn change_password(&self, token: &str, current: &str, new: &str) -> BResult<String>;
    async fn totp_begin(&self, token: &str) -> BResult<TotpEnrolmentDto>;
    async fn totp_confirm(&self, token: &str, code: &str) -> BResult<RecoveryCodesDto>;

    // setup wizard (no session before the first account)
    async fn setup_state(&self, token: Option<&str>) -> BResult<SetupStateDto>;
    async fn setup_account(&self, b: SignUpDto) -> BResult<SessionDto>;
    async fn setup_hosting(&self, token: &str, hosting: &str) -> BResult<SetupStateDto>;
    async fn setup_sources(&self, token: &str) -> BResult<SetupStateDto>;
    async fn setup_complete(&self, token: &str) -> BResult<()>;

    // admin area (admins only)
    async fn overview(&self, token: &str) -> BResult<AdminOverviewDto>;
    async fn settings(&self, token: &str) -> BResult<InstanceSettingsDto>;
    async fn update_settings(
        &self,
        token: &str,
        values: BTreeMap<String, serde_json::Value>,
    ) -> BResult<InstanceSettingsDto>;
    async fn users(&self, token: &str) -> BResult<Vec<AdminUserDto>>;
    async fn set_role(&self, token: &str, id: &str, role: &str) -> BResult<AdminUserDto>;
    async fn set_disabled(&self, token: &str, id: &str, disabled: bool) -> BResult<AdminUserDto>;
    async fn delete_user(&self, token: &str, id: &str) -> BResult<()>;
    async fn invites(&self, token: &str) -> BResult<Vec<AdminInviteDto>>;
    async fn create_invite(
        &self,
        token: &str,
        b: NewAdminInviteDto,
    ) -> BResult<AdminInviteCreatedDto>;
    async fn revoke_invite(&self, token: &str, id: &str) -> BResult<AdminInviteDto>;
    async fn sources(&self, token: &str) -> BResult<Vec<AdminSourceDto>>;
    async fn set_source_credential(
        &self,
        token: &str,
        id: &str,
        fields: Fields,
    ) -> BResult<AdminSourceDto>;
    async fn clear_source_credential(&self, token: &str, id: &str) -> BResult<AdminSourceDto>;
    async fn test_source(
        &self,
        token: &str,
        id: &str,
        fields: Option<Fields>,
    ) -> BResult<SourceTestDto>;
    async fn set_source_licence(
        &self,
        token: &str,
        id: &str,
        agreement_on_file: bool,
    ) -> BResult<AdminSourceDto>;
    async fn storage_jobs(&self, token: &str) -> BResult<StorageJobsDto>;
    async fn run_eviction(&self, token: &str) -> BResult<EvictionDto>;
    async fn failed_job(&self, token: &str, id: &str, retry: bool) -> BResult<()>;
    async fn notification_defaults(&self, token: &str) -> BResult<NotificationDefaultsDto>;
    async fn brand(&self, token: &str) -> BResult<BrandCardDto>;
    async fn security(&self, token: &str) -> BResult<SecurityDto>;
    async fn rewrap(&self, token: &str) -> BResult<RewrapDto>;
    async fn audit(&self, token: &str, q: AuditQueryDto) -> BResult<AuditPageDto>;
}
