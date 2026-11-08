//! Server functions used by the pages. On the server they call the [`Backend`] from context
//! with the caller's session cookie; in the browser they are HTTP calls to `/_fn/...`.

// `#[server]` generates the browser side of each function without an `.await` in its body.
#![allow(clippy::unused_async, clippy::unused_async_trait_impl)]

use contracts::{
    AttributionDto, CompanionDto, InviteDto, ItemPageDto, LibraryEntryDto, LinkRequestDto, ListDto,
    LoggedSessionDto, NewProgressDto, ProgressEventDto, SearchResponseDto, ShareLinkDto,
    SharedListDto, UserDto,
};
use leptos::prelude::*;
use leptos::server_fn::codec::Json;

#[cfg(feature = "ssr")]
pub(crate) mod server {
    use std::sync::Arc;

    use axum::http::header::SET_COOKIE;
    use axum::http::{HeaderMap, HeaderValue};
    use contracts::ErrorDto;
    use leptos::prelude::*;

    pub use crate::backend::Backend;

    pub fn backend() -> Result<Arc<dyn Backend>, ServerFnError> {
        use_context::<Arc<dyn Backend>>()
            .ok_or_else(|| ServerFnError::new("backend missing from context"))
    }

    pub async fn token(b: &dyn Backend) -> Option<String> {
        let headers: HeaderMap = leptos_axum::extract().await.ok()?;
        crate::backend::session_token(&headers, &b.session_cookie())
    }

    pub async fn require_token(b: &dyn Backend) -> Result<String, ServerFnError> {
        token(b)
            .await
            .ok_or_else(|| ServerFnError::new("unauthorized"))
    }

    /// The error code travels to the page, which translates it (`auth-error-<code>`), with
    /// the seconds of a lock-out ([`super::encode_error`]).
    // Takes the error by value so it can be passed to `map_err` directly.
    #[allow(clippy::needless_pass_by_value)]
    pub fn code(e: ErrorDto) -> ServerFnError {
        ServerFnError::new(super::encode_error(&e.code, e.retry_after_secs))
    }

    pub fn set_session(b: &dyn Backend, token: &str, max_age: i64) {
        let secure = if b.secure_cookies() { "; Secure" } else { "" };
        let v = format!(
            "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}",
            b.session_cookie()
        );
        if let (Some(resp), Ok(v)) = (
            use_context::<leptos_axum::ResponseOptions>(),
            HeaderValue::from_str(&v),
        ) {
            resp.append_header(SET_COOKIE, v);
        }
    }
}

/// The signed-in user, if any.
#[server(prefix = "/_fn")]
pub async fn current_user() -> Result<Option<UserDto>, ServerFnError> {
    let b = server::backend()?;
    Ok(match server::token(b.as_ref()).await {
        Some(t) => b.me(&t).await,
        None => None,
    })
}

#[server(prefix = "/_fn")]
pub async fn sign_in(
    username: String,
    password: String,
    totp: Option<String>,
) -> Result<UserDto, ServerFnError> {
    let b = server::backend()?;
    let s = b
        .sign_in(contracts::SignInDto {
            username,
            password,
            totp: totp.filter(|t| !t.is_empty()),
        })
        .await
        .map_err(server::code)?;
    server::set_session(b.as_ref(), &s.token, i64::from(b.session_days()) * 86_400);
    Ok(s.user)
}

#[server(prefix = "/_fn")]
pub async fn sign_up(
    username: String,
    display_name: String,
    password: String,
    email: Option<String>,
    invite: Option<String>,
) -> Result<UserDto, ServerFnError> {
    let b = server::backend()?;
    let s = b
        .sign_up(contracts::SignUpDto {
            username,
            display_name,
            password,
            email: email.filter(|e| !e.is_empty()),
            invite: invite.filter(|i| !i.is_empty()),
        })
        .await
        .map_err(server::code)?;
    server::set_session(b.as_ref(), &s.token, i64::from(b.session_days()) * 86_400);
    Ok(s.user)
}

/// Is `username` free? The answer carries the normalised name (`contracts::auth`); errors
/// are `signup_closed`, `invalid_invite` and `too_many_attempts`.
#[server(prefix = "/_fn")]
pub async fn username_available(
    username: String,
    invite: Option<String>,
) -> Result<contracts::auth::UsernameAvailability, ServerFnError> {
    let b = server::backend()?;
    b.username_available(contracts::auth::UsernameCheckDto {
        username,
        invite: invite.filter(|i| !i.is_empty()),
    })
    .await
    .map_err(server::code)
}

/// The first `limit` entries of the library, most recently added first.
#[server(prefix = "/_fn")]
pub async fn library(limit: u32) -> Result<contracts::LibraryPageDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.library(&t, limit).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn item_page(id: String) -> Result<ItemPageDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.item(&t, &id).await.map_err(server::code)
}

/// `group` limits the search to one kind group (the palette scope segments, design.md 6.4).
#[server(prefix = "/_fn")]
pub async fn search(
    q: String,
    kind: Option<String>,
    group: Option<String>,
) -> Result<SearchResponseDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.search(&t, &q, kind, group).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn add_to_library(item_id: String) -> Result<LibraryEntryDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.add(&t, &item_id).await.map_err(server::code)
}

// nested JSON with numbers (`season`, `episode`) does not survive URL encoding: JSON input
#[server(prefix = "/_fn", input = Json)]
pub async fn log_progress(
    item_id: String,
    progress: NewProgressDto,
) -> Result<ProgressEventDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.log_progress(&t, &item_id, progress)
        .await
        .map_err(server::code)
}

/// Log a play (players, scores, winner) of a board or video game; answers its progress event.
#[server(prefix = "/_fn", input = Json)]
pub async fn log_play(
    item_id: String,
    play: contracts::NewPlayDto,
) -> Result<ProgressEventDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.log_play(&t, &item_id, play).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn rate(item_id: String, rating: Option<u8>) -> Result<LibraryEntryDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.rate(&t, &item_id, rating).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn companions() -> Result<Vec<CompanionDto>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.companions(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn create_companion(name: String) -> Result<CompanionDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.create_companion(&t, &name).await.map_err(server::code)
}

/// The People page (WP3-6): requests, people with stats, merge suggestions.
#[server(prefix = "/_fn")]
pub async fn people_page() -> Result<contracts::PeoplePageDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.people_page(&t).await.map_err(server::code)
}

/// A person page (WP3-6).
#[server(prefix = "/_fn")]
pub async fn person_page(id: String) -> Result<contracts::PersonPageDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.person_page(&t, &id).await.map_err(server::code)
}

/// The imports page: the user's imports, newest first (WP3-7).
#[server(prefix = "/_fn")]
pub async fn imports() -> Result<Vec<contracts::imports::ImportDto>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.imports(&t).await.map_err(server::code)
}

/// The Lists page's three tabs (WP3-6).
#[server(prefix = "/_fn")]
pub async fn lists_overview() -> Result<contracts::ListsOverviewDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.lists_overview(&t).await.map_err(server::code)
}

/// A list page (WP3-6).
#[server(prefix = "/_fn")]
pub async fn list_page(id: String) -> Result<contracts::ListPageDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.list_page(&t, &id).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn lists() -> Result<Vec<ListDto>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.lists(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn create_list(name: String) -> Result<ListDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.create_list(&t, &name).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn add_to_list(list_id: String, item_id: String) -> Result<ListDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.add_to_list(&t, &list_id, &item_id)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn sources() -> Result<Vec<AttributionDto>, ServerFnError> {
    Ok(server::backend()?.sources().await)
}

// ---------------------------------------------------------------- sharing (D-9)

#[server(prefix = "/_fn")]
pub async fn request_link(
    companion_id: String,
    username: String,
) -> Result<LinkRequestDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.request_link(&t, &companion_id, &username)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn link_requests() -> Result<Vec<LinkRequestDto>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.link_requests(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn answer_link(id: String, accept: bool) -> Result<(), ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.answer_link(&t, &id, accept).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn unlink(companion_id: String) -> Result<CompanionDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.unlink(&t, &companion_id).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn merge(keep: String, remove: String) -> Result<CompanionDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.merge(&t, &keep, &remove).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn invite(companion_id: String) -> Result<InviteDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.invite(&t, &companion_id).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn sessions(shared: bool) -> Result<Vec<LoggedSessionDto>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.sessions(&t, shared).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn claim(session_id: String) -> Result<LoggedSessionDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.claim(&t, &session_id).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn set_privacy(share_sessions: bool) -> Result<UserDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.set_privacy(&t, contracts::PrivacyDto { share_sessions })
        .await
        .map_err(server::code)
}

/// The signed-in user's language (`None`: automatic, DEC-201).
#[server(prefix = "/_fn")]
pub async fn set_locale(locale: Option<String>) -> Result<UserDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.set_locale(&t, locale).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn share_list(list_id: String) -> Result<ShareLinkDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.share_list(&t, &list_id).await.map_err(server::code)
}

/// A link-shared list; works without signing in.
#[server(prefix = "/_fn")]
pub async fn shared_list(token: String) -> Result<SharedListDto, ServerFnError> {
    server::backend()?
        .shared_list(&token)
        .await
        .map_err(server::code)
}

// ---------------------------------------------------------------- integrations (M14)

#[server(prefix = "/_fn")]
pub async fn integrations() -> Result<Vec<contracts::IntegrationDto>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.integrations(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn create_integration(
    name: String,
) -> Result<contracts::IntegrationCreatedDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.create_integration(&t, &name).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn delete_integration(id: String) -> Result<(), ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.delete_integration(&t, &id).await.map_err(server::code)
}

// ---------------------------------------------------------------- notifications (D-17)

#[server(prefix = "/_fn")]
pub async fn notifications() -> Result<contracts::NotificationPageDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.notifications(&t).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn mark_read(id: String) -> Result<(), ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.mark_read(&t, &id).await.map_err(server::code)
}

// nested JSON (`config`) does not survive URL encoding: JSON input
#[server(prefix = "/_fn", input = Json)]
pub async fn create_channel(
    channel: contracts::NewChannelDto,
) -> Result<contracts::ChannelDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.create_channel(&t, channel).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn delete_channel(id: String) -> Result<(), ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.delete_channel(&t, &id).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn test_channel(id: String) -> Result<(), ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.test_channel(&t, &id).await.map_err(server::code)
}

#[server(prefix = "/_fn", input = Json)]
pub async fn put_subscription(
    subscription: contracts::SubscriptionDto,
) -> Result<contracts::SubscriptionDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.put_subscription(&t, subscription)
        .await
        .map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn delete_subscription(id: String) -> Result<(), ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.delete_subscription(&t, &id).await.map_err(server::code)
}

// ---------------------------------------------------------------- UX phase 1

/// Kinds and activity groups (with their Fluent label keys).
#[server(prefix = "/_fn")]
pub async fn kinds() -> Result<contracts::KindsDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.kinds(&t).await.map_err(server::code)
}

/// Counts per group, kind and status for the group switcher, chips and tabs.
#[server(prefix = "/_fn")]
pub async fn library_counts() -> Result<contracts::LibraryCountsDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.library_counts(&t).await.map_err(server::code)
}

/// The Library page query (its URL state): filters, text, sort, paging.
#[server(prefix = "/_fn", input = Json)]
pub async fn library_query(
    query: contracts::LibraryQueryDto,
) -> Result<contracts::LibraryRowsDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.library_query(&t, query).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn set_status(item_id: String, status: String) -> Result<LibraryEntryDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.set_status(&t, &item_id, &status)
        .await
        .map_err(server::code)
}

/// Every tag of the user with its number of entries.
#[server(prefix = "/_fn")]
pub async fn tags() -> Result<Vec<contracts::TagDto>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.tags(&t).await.map_err(server::code)
}

/// Add a tag to an item's entry; answers the entry's tags.
#[server(prefix = "/_fn")]
pub async fn add_tag(item_id: String, tag: String) -> Result<Vec<String>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.add_tag(&t, &item_id, &tag).await.map_err(server::code)
}

#[server(prefix = "/_fn")]
pub async fn remove_tag(item_id: String, tag: String) -> Result<Vec<String>, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.remove_tag(&t, &item_id, &tag).await.map_err(server::code)
}

/// Rename (or merge) a tag on every entry; answers the number of entries changed.
#[server(prefix = "/_fn")]
pub async fn rename_tag(from: String, to: String) -> Result<u64, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.rename_tag(&t, &from, &to).await.map_err(server::code)
}

/// The private note of an entry; `None` or blank clears it.
#[server(prefix = "/_fn")]
pub async fn set_entry_note(
    item_id: String,
    note: Option<String>,
) -> Result<LibraryEntryDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.set_entry_note(&t, &item_id, note)
        .await
        .map_err(server::code)
}

/// Undo a recent progress event (the toast's Undo); answers the compensating event.
#[server(prefix = "/_fn")]
pub async fn undo_progress(event_id: String) -> Result<ProgressEventDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.undo_progress(&t, &event_id).await.map_err(server::code)
}

/// A page of the activity feed (Home, History).
#[server(prefix = "/_fn", input = Json)]
pub async fn activity_feed(
    query: contracts::ActivityQueryDto,
) -> Result<contracts::ActivityPageDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.activity(&t, query).await.map_err(server::code)
}

/// All-time totals overall and per group (Home charts).
#[server(prefix = "/_fn")]
pub async fn stats_totals() -> Result<contracts::StatsTotalsDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.stats_totals(&t).await.map_err(server::code)
}

/// Activity per `month` or `year` for a group (`None`/`all`: every group), Stats page.
#[server(prefix = "/_fn")]
pub async fn stats_series(
    group: Option<String>,
    bucket: Option<String>,
) -> Result<contracts::StatsSeriesDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.stats_series(&t, group, bucket)
        .await
        .map_err(server::code)
}

/// Everything the signed-in Home page shows.
#[server(prefix = "/_fn")]
pub async fn home() -> Result<contracts::HomeDto, ServerFnError> {
    let b = server::backend()?;
    let t = server::require_token(b.as_ref()).await?;
    b.home(&t).await.map_err(server::code)
}

/// Error code carried by a server function error (see `server::code`).
#[must_use]
pub fn error_code(e: &ServerFnError) -> String {
    match e {
        ServerFnError::ServerError(m) => m.split(';').next().unwrap_or_default().to_string(),
        _ => "generic".into(),
    }
}

/// The message of a server function error: the code, then `;retry_after=<secs>` for a
/// throttled call (server functions carry one string, not the `ErrorDto`).
#[must_use]
pub fn encode_error(code: &str, retry_after_secs: Option<u64>) -> String {
    match retry_after_secs {
        Some(s) => format!("{code}{RETRY_AFTER}{s}"),
        None => code.to_string(),
    }
}

const RETRY_AFTER: &str = ";retry_after=";

/// Seconds until a throttled call may be tried again (see [`encode_error`]).
#[must_use]
pub fn retry_after_secs(e: &ServerFnError) -> Option<u64> {
    match e {
        ServerFnError::ServerError(m) => m.split_once(RETRY_AFTER)?.1.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_carry_the_code_and_the_seconds_left() {
        let e = ServerFnError::new(encode_error("too_many_attempts", Some(120)));
        assert_eq!(error_code(&e), "too_many_attempts");
        assert_eq!(retry_after_secs(&e), Some(120));
        let e = ServerFnError::new(encode_error("invalid_credentials", None));
        assert_eq!(error_code(&e), "invalid_credentials");
        assert_eq!(retry_after_secs(&e), None);
        let odd = ServerFnError::new("too_many_attempts;retry_after=soon");
        assert_eq!(retry_after_secs(&odd), None);
        assert_eq!(error_code(&odd), "too_many_attempts");
    }
}
