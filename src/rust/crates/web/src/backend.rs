//! The server side of the web app's server functions. `web` only knows this trait and the
//! `contracts` DTOs; the server binary implements it on top of the same operations as the
//! REST API, so both call the same `app` use-cases (D-3).

use async_trait::async_trait;
use axum::response::IntoResponse;
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
    /// Today in the user's time zone (`2026-02-01`, DEC-239); `None`: UTC.
    pub today: Option<String>,
    /// Is the logging funnel counted on this instance (WP4-5)?
    pub usage_funnel: bool,
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

/// Where the setup wizard stands, for the `/setup` page (design.md 5.1): completed means the
/// page is the 404 page. Put into the request extensions by [`resolve_shell_session`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupGate {
    pub completed: bool,
}

/// The wizard's own path.
pub const SETUP_PATH: &str = "/setup";

/// A page request (not a server function, a build asset or a file such as `favicon.ico`).
#[must_use]
pub fn is_page(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or_default().contains('.');
    !(file || path.starts_with("/_fn/") || path.starts_with("/pkg/"))
}

/// Middleware for the page routes: looks up the session once and stores a [`ShellSession`]
/// in the request extensions, where the shell's config reads it. Server functions and static
/// files are skipped. While no account exists every page answers `303 See Other` to
/// `/setup` (design.md 5.1); `/setup` itself gets a [`SetupGate`].
pub async fn resolve_shell_session(
    axum::extract::State(b): axum::extract::State<std::sync::Arc<dyn Backend>>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let path = req.uri().path().to_string();
    if !is_page(&path) {
        return next.run(req).await;
    }
    let session = match session_token(req.headers(), &b.session_cookie()) {
        Some(token) => b.shell_session(&token).await,
        None => None,
    };
    let setup = path == SETUP_PATH;
    if session.is_none() || setup {
        let (users, completed) = b.setup_gate().await;
        if !users && !setup {
            return axum::response::Redirect::to(SETUP_PATH).into_response();
        }
        if setup {
            req.extensions_mut().insert(SetupGate { completed });
        }
    }
    if let Some(s) = session {
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
    /// Does any account exist, and is the setup wizard completed (design.md 5.1)? Errors
    /// count as "yes" both times, so a storage hiccup never sends people to the wizard.
    async fn setup_gate(&self) -> (bool, bool);
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
        group: Option<String>,
    ) -> BResult<SearchResponseDto>;
    async fn add(&self, token: &str, item_id: &str) -> BResult<LibraryEntryDto>;
    async fn log_progress(
        &self,
        token: &str,
        item_id: &str,
        p: NewProgressDto,
    ) -> BResult<ProgressEventDto>;
    /// A play with players, scores and the winner (sessions progress model).
    async fn log_play(
        &self,
        token: &str,
        item_id: &str,
        p: contracts::NewPlayDto,
    ) -> BResult<ProgressEventDto>;
    async fn rate(
        &self,
        token: &str,
        item_id: &str,
        rating: Option<u8>,
    ) -> BResult<LibraryEntryDto>;

    async fn companions(&self, token: &str) -> BResult<Vec<CompanionDto>>;
    async fn create_companion(&self, token: &str, name: &str) -> BResult<CompanionDto>;

    // People and Lists pages (WP3-6)
    async fn people_page(&self, token: &str) -> BResult<contracts::PeoplePageDto>;
    async fn person_page(&self, token: &str, id: &str) -> BResult<contracts::PersonPageDto>;
    async fn lists_overview(&self, token: &str) -> BResult<contracts::ListsOverviewDto>;
    async fn list_page(&self, token: &str, id: &str) -> BResult<contracts::ListPageDto>;

    async fn lists(&self, token: &str) -> BResult<Vec<ListDto>>;
    async fn create_list(&self, token: &str, name: &str) -> BResult<ListDto>;
    async fn add_to_list(&self, token: &str, list_id: &str, item_id: &str) -> BResult<ListDto>;

    /// The user's imports, newest first (WP3-7).
    async fn imports(&self, token: &str) -> BResult<Vec<contracts::imports::ImportDto>>;

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

    // UX phase 1: library, Home, History, Stats
    async fn kinds(&self, token: &str) -> BResult<contracts::KindsDto>;
    async fn library_counts(&self, token: &str) -> BResult<contracts::LibraryCountsDto>;
    async fn library_query(
        &self,
        token: &str,
        q: contracts::LibraryQueryDto,
    ) -> BResult<contracts::LibraryRowsDto>;
    async fn set_status(
        &self,
        token: &str,
        item_id: &str,
        status: &str,
    ) -> BResult<LibraryEntryDto>;
    /// One operation on many library items (WP4-3), and its undo.
    async fn library_bulk(
        &self,
        token: &str,
        b: contracts::BulkRequestDto,
    ) -> BResult<contracts::BulkResponseDto>;
    async fn library_bulk_undo(&self, token: &str, undo: &str) -> BResult<()>;
    async fn tags(&self, token: &str) -> BResult<Vec<contracts::TagDto>>;
    async fn add_tag(&self, token: &str, item_id: &str, tag: &str) -> BResult<Vec<String>>;
    async fn remove_tag(&self, token: &str, item_id: &str, tag: &str) -> BResult<Vec<String>>;
    async fn rename_tag(&self, token: &str, from: &str, to: &str) -> BResult<u64>;
    async fn set_entry_note(
        &self,
        token: &str,
        item_id: &str,
        note: Option<String>,
    ) -> BResult<LibraryEntryDto>;
    async fn undo_progress(&self, token: &str, event_id: &str) -> BResult<ProgressEventDto>;
    async fn activity(
        &self,
        token: &str,
        q: contracts::ActivityQueryDto,
    ) -> BResult<contracts::ActivityPageDto>;
    async fn stats_totals(&self, token: &str) -> BResult<contracts::StatsTotalsDto>;
    async fn stats_series(
        &self,
        token: &str,
        group: Option<String>,
        bucket: Option<String>,
    ) -> BResult<contracts::StatsSeriesDto>;
    async fn home(&self, token: &str) -> BResult<contracts::HomeDto>;
    /// The caller's yearly goals (WP4-6).
    async fn goals(&self, token: &str) -> BResult<Vec<contracts::GoalDto>>;
    /// This year so far vs the same period last year, and the people seen most often.
    async fn stats_compare(&self, token: &str) -> BResult<contracts::StatsCompareDto>;
    /// The caller's game groups (the play sheet's chips).
    async fn game_groups(&self, token: &str) -> BResult<Vec<contracts::GameGroupDto>>;
}
