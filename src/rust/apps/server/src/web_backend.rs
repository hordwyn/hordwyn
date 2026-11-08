//! `web::Backend` for the web app's server functions, on top of the same operations as the
//! REST API (`api::ops`).

use api::{ApiError, ApiState, browse, notifications, ops, people, sharing};
use async_trait::async_trait;
use contracts::{
    AttributionDto, CompanionDto, ErrorDto, InviteDto, ItemPageDto, LibraryEntryDto,
    LinkRequestDto, ListDto, LoggedSessionDto, NewProgressDto, PrivacyDto, ProgressEventDto,
    SearchResponseDto, SessionDto, ShareLinkDto, SharedListDto, SignInDto, SignUpDto, UserDto,
};
use domain::user::Actor;
use web::backend::{BResult, Backend, ShellSession};

pub struct WebBackend(pub ApiState);

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn err(e: ApiError) -> ErrorDto {
    e.into_dto()
}

fn unauthorized() -> ErrorDto {
    ErrorDto {
        code: "unauthorized".into(),
        message: "sign in first".into(),
        retry_after_secs: None,
        field: None,
    }
}

impl WebBackend {
    async fn actor(&self, token: &str) -> BResult<Actor> {
        self.0
            .auth
            .session(token)
            .await
            .map_err(|e| err(e.into()))?
            .ok_or_else(unauthorized)
    }

    async fn session(&self, token: String) -> BResult<SessionDto> {
        let a = self.actor(&token).await?;
        let user = self
            .0
            .app
            .user(a.user_id)
            .await
            .map_err(|e| err(e.into()))?;
        Ok(SessionDto {
            token,
            user: api::dto_user(&user),
        })
    }
}

#[async_trait]
impl Backend for WebBackend {
    fn session_cookie(&self) -> String {
        self.0.naming.session_cookie.clone()
    }

    fn secure_cookies(&self) -> bool {
        self.0.cookie_secure()
    }

    fn session_days(&self) -> u32 {
        self.0.auth.settings().session_days
    }

    async fn me(&self, token: &str) -> Option<UserDto> {
        let a = self.actor(token).await.ok()?;
        self.0
            .app
            .user(a.user_id)
            .await
            .ok()
            .map(|u| api::dto_user(&u))
    }

    async fn shell_session(&self, token: &str) -> Option<ShellSession> {
        let a = self.actor(token).await.ok()?;
        let user = self.0.app.user(a.user_id).await.ok()?;
        // notifications may be disabled on this server: no bell count then
        let unread = notifications::unread_count(&self.0, a).await.unwrap_or(0);
        Some(ShellSession {
            display_name: user.display_name,
            username: user.username,
            is_admin: a.is_admin(),
            unread,
            today: self
                .0
                .app
                .today_for(a.user_id)
                .await
                .ok()
                .map(|d| d.format("%Y-%m-%d").to_string()),
            locale: user.locale,
        })
    }

    async fn setup_gate(&self) -> (bool, bool) {
        api::admin::setup_gate(&self.0).await.unwrap_or_else(|e| {
            tracing::warn!(error = %e.into_dto().message, "setup state unreadable");
            (true, true)
        })
    }

    async fn sign_in(&self, b: SignInDto) -> BResult<SessionDto> {
        // the sign-in mode applies here too (`sso_only` in `auto` mode, WP3-3)
        let t = api::password_sign_in(&self.0, &b, false)
            .await
            .map_err(err)?;
        self.session(t.0).await
    }

    async fn sign_up(&self, b: SignUpDto) -> BResult<SessionDto> {
        let (_, t) = self
            .0
            .auth
            .sign_up(auth::SignUp {
                username: b.username,
                display_name: b.display_name,
                email: b.email,
                password: b.password,
                invite: b.invite,
            })
            .await
            .map_err(|e| err(e.into()))?;
        self.session(t.0).await
    }

    async fn sign_out(&self, token: &str) {
        let _ = self.0.auth.sign_out(token).await;
    }

    async fn username_available(
        &self,
        b: contracts::auth::UsernameCheckDto,
    ) -> BResult<contracts::auth::UsernameAvailability> {
        self.0
            .auth
            .username_available(&b.username, b.invite.as_deref())
            .await
            .map_err(|e| err(e.into()))
    }

    async fn library(&self, token: &str, limit: u32) -> BResult<contracts::LibraryPageDto> {
        let a = self.actor(token).await?;
        ops::library(&self.0, a, 0, limit).await.map_err(err)
    }

    async fn item(&self, token: &str, id: &str) -> BResult<ItemPageDto> {
        let a = self.actor(token).await?;
        ops::item_page(&self.0, a, id).await.map_err(err)
    }

    async fn search(
        &self,
        token: &str,
        q: &str,
        kind: Option<String>,
        group: Option<String>,
    ) -> BResult<SearchResponseDto> {
        let a = self.actor(token).await?;
        ops::search(&self.0, a, q, kind, group).await.map_err(err)
    }

    async fn add(&self, token: &str, item_id: &str) -> BResult<LibraryEntryDto> {
        let a = self.actor(token).await?;
        ops::add(&self.0, a, item_id, None).await.map_err(err)
    }

    async fn log_progress(
        &self,
        token: &str,
        item_id: &str,
        p: NewProgressDto,
    ) -> BResult<ProgressEventDto> {
        let a = self.actor(token).await?;
        ops::log_progress(&self.0, a, item_id, p).await.map_err(err)
    }

    async fn log_play(
        &self,
        token: &str,
        item_id: &str,
        p: contracts::NewPlayDto,
    ) -> BResult<ProgressEventDto> {
        let a = self.actor(token).await?;
        ops::log_play(&self.0, a, item_id, p).await.map_err(err)
    }

    async fn rate(
        &self,
        token: &str,
        item_id: &str,
        rating: Option<u8>,
    ) -> BResult<LibraryEntryDto> {
        let a = self.actor(token).await?;
        ops::rate(&self.0, a, item_id, rating).await.map_err(err)
    }

    async fn companions(&self, token: &str) -> BResult<Vec<CompanionDto>> {
        let a = self.actor(token).await?;
        ops::companions(&self.0, a).await.map_err(err)
    }

    async fn create_companion(&self, token: &str, name: &str) -> BResult<CompanionDto> {
        let a = self.actor(token).await?;
        ops::create_companion(&self.0, a, name).await.map_err(err)
    }

    async fn people_page(&self, token: &str) -> BResult<contracts::PeoplePageDto> {
        let a = self.actor(token).await?;
        people::people_page(&self.0, a).await.map_err(err)
    }

    async fn person_page(&self, token: &str, id: &str) -> BResult<contracts::PersonPageDto> {
        let a = self.actor(token).await?;
        people::person_page(&self.0, a, id).await.map_err(err)
    }

    async fn imports(&self, token: &str) -> BResult<Vec<contracts::imports::ImportDto>> {
        let a = self.actor(token).await?;
        let list = ops::imports(&self.0, a).await.map_err(err)?;
        Ok(list
            .into_iter()
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect())
    }

    async fn lists_overview(&self, token: &str) -> BResult<contracts::ListsOverviewDto> {
        let a = self.actor(token).await?;
        people::lists_overview(&self.0, a).await.map_err(err)
    }

    async fn list_page(&self, token: &str, id: &str) -> BResult<contracts::ListPageDto> {
        let a = self.actor(token).await?;
        people::list_page(&self.0, a, id).await.map_err(err)
    }

    async fn lists(&self, token: &str) -> BResult<Vec<ListDto>> {
        let a = self.actor(token).await?;
        ops::lists(&self.0, a).await.map_err(err)
    }

    async fn create_list(&self, token: &str, name: &str) -> BResult<ListDto> {
        let a = self.actor(token).await?;
        ops::create_list(&self.0, a, name).await.map_err(err)
    }

    async fn add_to_list(&self, token: &str, list_id: &str, item_id: &str) -> BResult<ListDto> {
        let a = self.actor(token).await?;
        ops::add_to_list(&self.0, a, list_id, item_id)
            .await
            .map_err(err)
    }

    async fn sources(&self) -> Vec<AttributionDto> {
        ops::sources(&self.0)
    }

    async fn request_link(
        &self,
        token: &str,
        companion_id: &str,
        username: &str,
    ) -> BResult<LinkRequestDto> {
        let a = self.actor(token).await?;
        sharing::request_link(&self.0, a, companion_id, username)
            .await
            .map_err(err)
    }

    async fn link_requests(&self, token: &str) -> BResult<Vec<LinkRequestDto>> {
        let a = self.actor(token).await?;
        sharing::link_requests(&self.0, a).await.map_err(err)
    }

    async fn answer_link(&self, token: &str, id: &str, accept: bool) -> BResult<()> {
        let a = self.actor(token).await?;
        if accept {
            sharing::accept_link(&self.0, a, id).await.map(|_| ())
        } else {
            sharing::decline_link(&self.0, a, id).await
        }
        .map_err(err)
    }

    async fn unlink(&self, token: &str, companion_id: &str) -> BResult<CompanionDto> {
        let a = self.actor(token).await?;
        sharing::unlink(&self.0, a, companion_id).await.map_err(err)
    }

    async fn merge(&self, token: &str, keep: &str, remove: &str) -> BResult<CompanionDto> {
        let a = self.actor(token).await?;
        sharing::merge(&self.0, a, keep, remove).await.map_err(err)
    }

    async fn invite(&self, token: &str, companion_id: &str) -> BResult<InviteDto> {
        let a = self.actor(token).await?;
        sharing::invite(&self.0, a, companion_id, None)
            .await
            .map_err(err)
    }

    async fn sessions(&self, token: &str, shared: bool) -> BResult<Vec<LoggedSessionDto>> {
        let a = self.actor(token).await?;
        if shared {
            sharing::shared_sessions(&self.0, a).await
        } else {
            sharing::my_sessions(&self.0, a).await
        }
        .map_err(err)
    }

    async fn claim(&self, token: &str, session_id: &str) -> BResult<LoggedSessionDto> {
        let a = self.actor(token).await?;
        sharing::claim(&self.0, a, session_id).await.map_err(err)
    }

    async fn set_privacy(&self, token: &str, p: PrivacyDto) -> BResult<UserDto> {
        let a = self.actor(token).await?;
        sharing::set_privacy(&self.0, a, p).await.map_err(err)
    }

    async fn set_locale(&self, token: &str, locale: Option<String>) -> BResult<UserDto> {
        let a = self.actor(token).await?;
        sharing::set_locale(&self.0, a, contracts::LocaleDto { locale })
            .await
            .map_err(err)
    }

    async fn share_list(&self, token: &str, list_id: &str) -> BResult<ShareLinkDto> {
        let a = self.actor(token).await?;
        sharing::share_list(&self.0, a, list_id).await.map_err(err)
    }

    async fn shared_list(&self, share_token: &str) -> BResult<SharedListDto> {
        sharing::shared_list(&self.0, share_token)
            .await
            .map_err(err)
    }

    async fn integrations(&self, token: &str) -> BResult<Vec<contracts::IntegrationDto>> {
        let a = self.actor(token).await?;
        api::hooks::list(&self.0, a).await.map_err(err)
    }

    async fn create_integration(
        &self,
        token: &str,
        name: &str,
    ) -> BResult<contracts::IntegrationCreatedDto> {
        let a = self.actor(token).await?;
        api::hooks::create(&self.0, a, name).await.map_err(err)
    }

    async fn delete_integration(&self, token: &str, id: &str) -> BResult<()> {
        let a = self.actor(token).await?;
        api::hooks::delete(&self.0, a, id).await.map_err(err)
    }

    async fn notifications(&self, token: &str) -> BResult<contracts::NotificationPageDto> {
        let a = self.actor(token).await?;
        notifications::page(&self.0, a).await.map_err(err)
    }

    async fn mark_read(&self, token: &str, id: &str) -> BResult<()> {
        let a = self.actor(token).await?;
        notifications::mark_read(&self.0, a, id).await.map_err(err)
    }

    async fn create_channel(
        &self,
        token: &str,
        b: contracts::NewChannelDto,
    ) -> BResult<contracts::ChannelDto> {
        let a = self.actor(token).await?;
        notifications::create_channel(&self.0, a, b)
            .await
            .map_err(err)
    }

    async fn delete_channel(&self, token: &str, id: &str) -> BResult<()> {
        let a = self.actor(token).await?;
        notifications::delete_channel(&self.0, a, id)
            .await
            .map_err(err)
    }

    async fn test_channel(&self, token: &str, id: &str) -> BResult<()> {
        let a = self.actor(token).await?;
        notifications::test_channel(&self.0, a, id)
            .await
            .map_err(err)
    }

    async fn put_subscription(
        &self,
        token: &str,
        b: contracts::SubscriptionDto,
    ) -> BResult<contracts::SubscriptionDto> {
        let a = self.actor(token).await?;
        notifications::put_subscription(&self.0, a, b)
            .await
            .map_err(err)
    }

    async fn delete_subscription(&self, token: &str, id: &str) -> BResult<()> {
        let a = self.actor(token).await?;
        notifications::delete_subscription(&self.0, a, id)
            .await
            .map_err(err)
    }

    async fn kinds(&self, token: &str) -> BResult<contracts::KindsDto> {
        self.actor(token).await?;
        Ok(browse::kinds(&self.0))
    }

    async fn library_counts(&self, token: &str) -> BResult<contracts::LibraryCountsDto> {
        let a = self.actor(token).await?;
        browse::library_counts(&self.0, a).await.map_err(err)
    }

    async fn library_query(
        &self,
        token: &str,
        q: contracts::LibraryQueryDto,
    ) -> BResult<contracts::LibraryRowsDto> {
        let a = self.actor(token).await?;
        browse::library_query(&self.0, a, q).await.map_err(err)
    }

    async fn set_status(
        &self,
        token: &str,
        item_id: &str,
        status: &str,
    ) -> BResult<LibraryEntryDto> {
        let a = self.actor(token).await?;
        ops::set_status(&self.0, a, item_id, status.to_string())
            .await
            .map_err(err)
    }

    async fn tags(&self, token: &str) -> BResult<Vec<contracts::TagDto>> {
        let a = self.actor(token).await?;
        browse::tags(&self.0, a).await.map_err(err)
    }

    async fn add_tag(&self, token: &str, item_id: &str, tag: &str) -> BResult<Vec<String>> {
        let a = self.actor(token).await?;
        browse::add_tag(&self.0, a, item_id, tag).await.map_err(err)
    }

    async fn remove_tag(&self, token: &str, item_id: &str, tag: &str) -> BResult<Vec<String>> {
        let a = self.actor(token).await?;
        browse::remove_tag(&self.0, a, item_id, tag)
            .await
            .map_err(err)
    }

    async fn rename_tag(&self, token: &str, from: &str, to: &str) -> BResult<u64> {
        let a = self.actor(token).await?;
        browse::rename_tag(&self.0, a, from, to).await.map_err(err)
    }

    async fn set_entry_note(
        &self,
        token: &str,
        item_id: &str,
        note: Option<String>,
    ) -> BResult<LibraryEntryDto> {
        let a = self.actor(token).await?;
        browse::set_note(&self.0, a, item_id, note)
            .await
            .map_err(err)
    }

    async fn undo_progress(&self, token: &str, event_id: &str) -> BResult<ProgressEventDto> {
        let a = self.actor(token).await?;
        browse::undo(&self.0, a, event_id).await.map_err(err)
    }

    async fn activity(
        &self,
        token: &str,
        q: contracts::ActivityQueryDto,
    ) -> BResult<contracts::ActivityPageDto> {
        let a = self.actor(token).await?;
        browse::activity(&self.0, a, q).await.map_err(err)
    }

    async fn stats_totals(&self, token: &str) -> BResult<contracts::StatsTotalsDto> {
        let a = self.actor(token).await?;
        browse::stats_totals(&self.0, a).await.map_err(err)
    }

    async fn stats_series(
        &self,
        token: &str,
        group: Option<String>,
        bucket: Option<String>,
    ) -> BResult<contracts::StatsSeriesDto> {
        let a = self.actor(token).await?;
        browse::stats_series(&self.0, a, group, bucket)
            .await
            .map_err(err)
    }

    async fn home(&self, token: &str) -> BResult<contracts::HomeDto> {
        let a = self.actor(token).await?;
        browse::home(&self.0, a).await.map_err(err)
    }
}
