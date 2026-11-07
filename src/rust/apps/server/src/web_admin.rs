//! `web::admin_backend::AdminBackend` on top of `api::admin`, the same operations as the
//! REST routes.

use std::collections::BTreeMap;

use api::admin::{self, Caller};
use api::{ApiError, ApiState};
use async_trait::async_trait;
use contracts::admin::{
    AdminInviteCreatedDto, AdminInviteDto, AdminOverviewDto, AdminSourceDto, AdminUserDto,
    AskAdminDto, AuditPageDto, AuditQueryDto, BrandCardDto, CredentialInputDto, EvictionDto,
    HostingInputDto, InstanceSettingsDto, LicenceInputDto, NewAdminInviteDto,
    NotificationDefaultsDto, ReauthInputDto, ReauthStatusDto, RecoveryCodesDto, RewrapDto,
    SecurityDto, SettingsPatchDto, SetupStateDto, SourceStatusPageDto, SourceTestDto,
    SourceTestInputDto, StorageJobsDto, TotpEnrolmentDto, UserCredentialDto,
};
use contracts::{ErrorDto, SessionDto, SignUpDto};
use domain::user::Actor;
use web::admin_backend::{AdminBackend, Fields};
use web::backend::BResult;

pub struct WebAdmin(pub ApiState);

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn err(e: ApiError) -> ErrorDto {
    e.into_dto()
}

impl WebAdmin {
    async fn actor(&self, token: &str) -> BResult<Actor> {
        self.0
            .auth
            .session(token)
            .await
            .map_err(|e| err(e.into()))?
            .ok_or_else(|| err(ApiError::unauthorized()))
    }

    async fn caller<'a>(&self, token: &'a str) -> BResult<Caller<'a>> {
        Ok(Caller {
            actor: self.actor(token).await?,
            token,
        })
    }
}

#[async_trait]
impl AdminBackend for WebAdmin {
    async fn source_status(&self, token: &str) -> BResult<SourceStatusPageDto> {
        let a = self.actor(token).await?;
        admin::source_status(&self.0, a).await.map_err(err)
    }

    async fn ask_admin(&self, token: &str, b: AskAdminDto) -> BResult<()> {
        let a = self.actor(token).await?;
        admin::ask_admin(&self.0, a, b).await.map_err(err)
    }

    async fn my_credentials(&self, token: &str) -> BResult<Vec<UserCredentialDto>> {
        let a = self.actor(token).await?;
        admin::my_credentials(&self.0, a).await.map_err(err)
    }

    async fn set_my_credential(
        &self,
        token: &str,
        source: &str,
        fields: Fields,
    ) -> BResult<Vec<UserCredentialDto>> {
        let a = self.actor(token).await?;
        admin::set_my_credential(&self.0, a, source, CredentialInputDto { fields })
            .await
            .map_err(err)
    }

    async fn clear_my_credential(
        &self,
        token: &str,
        source: &str,
    ) -> BResult<Vec<UserCredentialDto>> {
        let a = self.actor(token).await?;
        admin::clear_my_credential(&self.0, a, source)
            .await
            .map_err(err)
    }

    async fn reauth_status(&self, token: &str) -> BResult<ReauthStatusDto> {
        admin::reauth_status(&self.0, token).await.map_err(err)
    }

    async fn reauth(&self, token: &str, b: ReauthInputDto) -> BResult<(ReauthStatusDto, String)> {
        admin::reauth(&self.0, token, b).await.map_err(err)
    }

    async fn change_password(&self, token: &str, current: &str, new: &str) -> BResult<String> {
        let a = self.actor(token).await?;
        admin::change_password(&self.0, a, current, new)
            .await
            .map_err(err)
    }

    async fn totp_begin(&self, token: &str) -> BResult<TotpEnrolmentDto> {
        admin::totp_begin(&self.0, token).await.map_err(err)
    }

    async fn totp_confirm(&self, token: &str, code: &str) -> BResult<RecoveryCodesDto> {
        let a = self.actor(token).await?;
        admin::totp_confirm(&self.0, a, code).await.map_err(err)
    }

    async fn setup_state(&self, token: Option<&str>) -> BResult<SetupStateDto> {
        let actor = match token {
            Some(t) => self.actor(t).await.ok(),
            None => None,
        };
        admin::setup_state(&self.0, actor).await.map_err(err)
    }

    async fn setup_account(&self, b: SignUpDto) -> BResult<SessionDto> {
        admin::setup_account(&self.0, b).await.map_err(err)
    }

    async fn setup_hosting(&self, token: &str, hosting: &str) -> BResult<SetupStateDto> {
        let a = self.actor(token).await?;
        let b = HostingInputDto {
            hosting: hosting.into(),
        };
        admin::setup_hosting(&self.0, a, b).await.map_err(err)
    }

    async fn setup_sources(&self, token: &str) -> BResult<SetupStateDto> {
        let a = self.actor(token).await?;
        admin::setup_sources(&self.0, a).await.map_err(err)
    }

    async fn setup_complete(&self, token: &str) -> BResult<()> {
        let a = self.actor(token).await?;
        admin::setup_complete(&self.0, a).await.map_err(err)
    }

    async fn overview(&self, token: &str) -> BResult<AdminOverviewDto> {
        let a = self.actor(token).await?;
        admin::overview(&self.0, a).await.map_err(err)
    }

    async fn settings(&self, token: &str) -> BResult<InstanceSettingsDto> {
        let a = self.actor(token).await?;
        admin::settings(&self.0, a).await.map_err(err)
    }

    async fn update_settings(
        &self,
        token: &str,
        values: BTreeMap<String, serde_json::Value>,
    ) -> BResult<InstanceSettingsDto> {
        let c = self.caller(token).await?;
        admin::update_settings(&self.0, c, SettingsPatchDto { values })
            .await
            .map_err(err)
    }

    async fn users(&self, token: &str) -> BResult<Vec<AdminUserDto>> {
        let a = self.actor(token).await?;
        admin::users(&self.0, a).await.map_err(err)
    }

    async fn set_role(&self, token: &str, id: &str, role: &str) -> BResult<AdminUserDto> {
        let c = self.caller(token).await?;
        admin::set_role(&self.0, c, id, role).await.map_err(err)
    }

    async fn set_disabled(&self, token: &str, id: &str, disabled: bool) -> BResult<AdminUserDto> {
        let c = self.caller(token).await?;
        admin::set_disabled(&self.0, c, id, disabled)
            .await
            .map_err(err)
    }

    async fn delete_user(&self, token: &str, id: &str) -> BResult<()> {
        let c = self.caller(token).await?;
        admin::delete_user(&self.0, c, id).await.map_err(err)
    }

    async fn invites(&self, token: &str) -> BResult<Vec<AdminInviteDto>> {
        let a = self.actor(token).await?;
        admin::invites(&self.0, a).await.map_err(err)
    }

    async fn create_invite(
        &self,
        token: &str,
        b: NewAdminInviteDto,
    ) -> BResult<AdminInviteCreatedDto> {
        let a = self.actor(token).await?;
        admin::create_invite(&self.0, a, b).await.map_err(err)
    }

    async fn revoke_invite(&self, token: &str, id: &str) -> BResult<AdminInviteDto> {
        let a = self.actor(token).await?;
        admin::revoke_invite(&self.0, a, id).await.map_err(err)
    }

    async fn sources(&self, token: &str) -> BResult<Vec<AdminSourceDto>> {
        let a = self.actor(token).await?;
        admin::sources(&self.0, a).await.map_err(err)
    }

    async fn set_source_credential(
        &self,
        token: &str,
        id: &str,
        fields: Fields,
    ) -> BResult<AdminSourceDto> {
        let a = self.actor(token).await?;
        admin::set_source_credential(&self.0, a, id, CredentialInputDto { fields })
            .await
            .map_err(err)
    }

    async fn clear_source_credential(&self, token: &str, id: &str) -> BResult<AdminSourceDto> {
        let a = self.actor(token).await?;
        admin::clear_source_credential(&self.0, a, id)
            .await
            .map_err(err)
    }

    async fn test_source(
        &self,
        token: &str,
        id: &str,
        fields: Option<Fields>,
    ) -> BResult<SourceTestDto> {
        let a = self.actor(token).await?;
        admin::test_source(&self.0, a, id, SourceTestInputDto { fields })
            .await
            .map_err(err)
    }

    async fn set_source_licence(
        &self,
        token: &str,
        id: &str,
        agreement_on_file: bool,
    ) -> BResult<AdminSourceDto> {
        let a = self.actor(token).await?;
        admin::set_source_licence(&self.0, a, id, LicenceInputDto { agreement_on_file })
            .await
            .map_err(err)
    }

    async fn storage_jobs(&self, token: &str) -> BResult<StorageJobsDto> {
        let a = self.actor(token).await?;
        admin::storage_jobs(&self.0, a).await.map_err(err)
    }

    async fn run_eviction(&self, token: &str) -> BResult<EvictionDto> {
        let a = self.actor(token).await?;
        admin::run_eviction(&self.0, a).await.map_err(err)
    }

    async fn failed_job(&self, token: &str, id: &str, retry: bool) -> BResult<()> {
        let a = self.actor(token).await?;
        admin::failed_job(&self.0, a, id, retry).await.map_err(err)
    }

    async fn notification_defaults(&self, token: &str) -> BResult<NotificationDefaultsDto> {
        let a = self.actor(token).await?;
        admin::notification_defaults(&self.0, a).await.map_err(err)
    }

    async fn brand(&self, token: &str) -> BResult<BrandCardDto> {
        let a = self.actor(token).await?;
        admin::brand(&self.0, a).map_err(err)
    }

    async fn security(&self, token: &str) -> BResult<SecurityDto> {
        let a = self.actor(token).await?;
        admin::security(&self.0, a).await.map_err(err)
    }

    async fn rewrap(&self, token: &str) -> BResult<RewrapDto> {
        let a = self.actor(token).await?;
        admin::rewrap(&self.0, a).await.map_err(err)
    }

    async fn audit(&self, token: &str, q: AuditQueryDto) -> BResult<AuditPageDto> {
        let a = self.actor(token).await?;
        admin::audit_log(&self.0, a, q).await.map_err(err)
    }
}
