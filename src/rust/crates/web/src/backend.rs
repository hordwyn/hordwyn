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

/// What the app shell shows for a signed-in session (design.md 3.2). The server resolves it
/// before rendering a page ([`resolve_shell_session`]) so the first render is already right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSession {
    pub display_name: String,
    pub username: String,
    pub is_admin: bool,
    /// Unread inbox notifications.
    pub unread: u32,
    /// The language the user chose (`None`: automatic, DEC-201).
    pub locale: Option<String>,
}

/// Value of the cookie `name` in `headers`, if set and not empty.
#[must_use]
pub fn session_token(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, v)| *k == name && !v.is_empty())
        .map(|(_, v)| v.to_string())
}

/// Middleware for the page routes: looks up the session once and stores a [`ShellSession`]
/// in the request extensions, where the shell's config reads it. Server functions and static
/// files are skipped.
pub async fn resolve_shell_session(
    axum::extract::State(b): axum::extract::State<std::sync::Arc<dyn Backend>>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path();
    let page = !(path.starts_with("/_fn/") || path.starts_with("/pkg/"));
    if page
        && let Some(token) = session_token(req.headers(), &b.session_cookie())
        && let Some(s) = b.shell_session(&token).await
    {
        req.extensions_mut().insert(s);
    }
    next.run(req).await
}

#[async_trait]
pub trait Backend: Send + Sync {
    /// Name of the session cookie (from the brand).
    fn session_cookie(&self) -> String;
    fn secure_cookies(&self) -> bool;
    fn session_days(&self) -> u32;

    async fn me(&self, token: &str) -> Option<UserDto>;
    /// The shell's view of the session; `None` when the token is not a valid session.
    async fn shell_session(&self, token: &str) -> Option<ShellSession>;
    async fn sign_in(&self, b: SignInDto) -> BResult<SessionDto>;
    async fn sign_up(&self, b: SignUpDto) -> BResult<SessionDto>;
    async fn sign_out(&self, token: &str);
    /// Username availability (normalised like sign-up, rate limited, DEC-184).
    async fn username_available(
        &self,
        b: contracts::auth::UsernameCheckDto,
    ) -> BResult<contracts::auth::UsernameAvailability>;

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
    /// The user's language (`None`: automatic); `invalid` for an unsupported locale.
    async fn set_locale(&self, token: &str, locale: Option<String>) -> BResult<UserDto>;
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
