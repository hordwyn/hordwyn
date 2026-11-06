//! The server side of the web app's server functions. `web` only knows this trait and the
//! `contracts` DTOs; the server binary implements it on top of the same operations as the
//! REST API, so both call the same `app` use-cases (D-3).

use async_trait::async_trait;
use contracts::{
    AttributionDto, CompanionDto, ErrorDto, InviteDto, ItemPageDto, LibraryEntryDto,
    LinkRequestDto, ListDto, LoggedSessionDto, NewProgressDto, PrivacyDto, ProgressEventDto,
    SearchResponseDto, SessionDto, ShareLinkDto, SharedListDto, SignInDto, SignUpDto, UserDto,
};

pub type BResult<T> = Result<T, ErrorDto>;

#[async_trait]
pub trait Backend: Send + Sync {
    /// Name of the session cookie (from the brand).
    fn session_cookie(&self) -> String;
    fn secure_cookies(&self) -> bool;
    fn session_days(&self) -> u32;

    async fn me(&self, token: &str) -> Option<UserDto>;
    async fn sign_in(&self, b: SignInDto) -> BResult<SessionDto>;
    async fn sign_up(&self, b: SignUpDto) -> BResult<SessionDto>;
    async fn sign_out(&self, token: &str);

    async fn library(&self, token: &str, limit: u32) -> BResult<contracts::LibraryPageDto>;
    async fn item(&self, token: &str, id: &str) -> BResult<ItemPageDto>;
    async fn search(
        &self,
        token: &str,
        q: &str,
        kind: Option<String>,
    ) -> BResult<SearchResponseDto>;
    async fn add(&self, token: &str, item_id: &str) -> BResult<LibraryEntryDto>;
    async fn log_progress(
        &self,
        token: &str,
        item_id: &str,
        p: NewProgressDto,
    ) -> BResult<ProgressEventDto>;
    async fn rate(
        &self,
        token: &str,
        item_id: &str,
        rating: Option<u8>,
    ) -> BResult<LibraryEntryDto>;

    async fn companions(&self, token: &str) -> BResult<Vec<CompanionDto>>;
    async fn create_companion(&self, token: &str, name: &str) -> BResult<CompanionDto>;

    async fn lists(&self, token: &str) -> BResult<Vec<ListDto>>;
    async fn create_list(&self, token: &str, name: &str) -> BResult<ListDto>;
    async fn add_to_list(&self, token: &str, list_id: &str, item_id: &str) -> BResult<ListDto>;

    async fn sources(&self) -> Vec<AttributionDto>;

    // sharing (D-9)
    async fn request_link(
        &self,
        token: &str,
        companion_id: &str,
        username: &str,
    ) -> BResult<LinkRequestDto>;
    async fn link_requests(&self, token: &str) -> BResult<Vec<LinkRequestDto>>;
    async fn answer_link(&self, token: &str, id: &str, accept: bool) -> BResult<()>;
    async fn unlink(&self, token: &str, companion_id: &str) -> BResult<CompanionDto>;
    async fn merge(&self, token: &str, keep: &str, remove: &str) -> BResult<CompanionDto>;
    async fn invite(&self, token: &str, companion_id: &str) -> BResult<InviteDto>;
    async fn sessions(&self, token: &str, shared: bool) -> BResult<Vec<LoggedSessionDto>>;
    async fn claim(&self, token: &str, session_id: &str) -> BResult<LoggedSessionDto>;
    async fn set_privacy(&self, token: &str, p: PrivacyDto) -> BResult<UserDto>;
    async fn share_list(&self, token: &str, list_id: &str) -> BResult<ShareLinkDto>;
    /// A link-shared list; `share_token` is the list's token, no session needed.
    async fn shared_list(&self, share_token: &str) -> BResult<SharedListDto>;

    // integrations (M14)
    async fn integrations(&self, token: &str) -> BResult<Vec<contracts::IntegrationDto>>;
    async fn create_integration(
        &self,
        token: &str,
        name: &str,
    ) -> BResult<contracts::IntegrationCreatedDto>;
    async fn delete_integration(&self, token: &str, id: &str) -> BResult<()>;

    // notifications (D-17)
    async fn notifications(&self, token: &str) -> BResult<contracts::NotificationPageDto>;
    async fn mark_read(&self, token: &str, id: &str) -> BResult<()>;
    async fn create_channel(
        &self,
        token: &str,
        b: contracts::NewChannelDto,
    ) -> BResult<contracts::ChannelDto>;
    async fn delete_channel(&self, token: &str, id: &str) -> BResult<()>;
    async fn test_channel(&self, token: &str, id: &str) -> BResult<()>;
    async fn put_subscription(
        &self,
        token: &str,
        b: contracts::SubscriptionDto,
    ) -> BResult<contracts::SubscriptionDto>;
    async fn delete_subscription(&self, token: &str, id: &str) -> BResult<()>;
}
