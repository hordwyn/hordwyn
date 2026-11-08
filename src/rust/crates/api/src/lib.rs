//! Versioned REST API `/api/v1` with an OpenAPI document (utoipa) and the SSE event stream.
//!
//! Handlers only call `app` use-cases (and `auth`); none of them awaits a provider: item
//! detail enqueues a refresh, search answers from local data and streams connector results
//! as `search.results` events (D-6).
#![deny(unsafe_code)]

pub mod admin;
mod admin_routes;
pub mod browse;
mod dto;
mod error;
mod extract;
pub mod hooks;
pub mod notifications;
mod oidc_routes;
pub mod ops;
pub mod people;
mod people_routes;
pub mod request_meta;
mod routes;
pub mod sharing;
mod sso_routes;
pub mod sync;

use std::sync::Arc;

use app::App;
use app::ports::SearchIndex;
use auth::Auth;
use catalog_core::Registry;
use catalog_core::search::SearchFanout;

pub use error::ApiError;
pub use extract::{Authed, AuthedToken, session_token};
pub use routes::router;
pub use sso_routes::{
    AutoRedirect, SIGNED_OUT_PATH, auto_redirect, password_sign_in, sign_out_location, with_body,
};

/// User DTO (also used by the web backend).
#[must_use]
pub fn dto_user(u: &domain::user::User) -> contracts::UserDto {
    dto::user(u)
}

/// Cookie and header names, derived from the brand (D-10), never hard-coded.
#[derive(Debug, Clone)]
pub struct Naming {
    /// Session cookie, e.g. `<short_name>_session`.
    pub session_cookie: String,
    /// Cookie holding the pending OIDC login.
    pub oidc_cookie: String,
    /// Cookie holding an "Is this your account?" ticket (OIDC account linking).
    pub oidc_link_cookie: String,
    /// Signed hint for the next silent sign-in (provider, `login_hint`; addendum-1 4.2).
    pub oidc_hint_cookie: String,
    /// Loop guard: a silent attempt was made in the last 5 minutes (addendum-1 4.2 step 4).
    pub oidc_silent_cookie: String,
    /// Title of the OpenAPI document.
    pub api_title: String,
}

impl Naming {
    #[must_use]
    pub fn from_prefix(prefix: &str, product: &str) -> Self {
        Self {
            session_cookie: format!("{prefix}_session"),
            oidc_cookie: format!("{prefix}_oidc"),
            oidc_link_cookie: format!("{prefix}_oidc_link"),
            oidc_hint_cookie: format!("{prefix}_oidc_hint"),
            oidc_silent_cookie: format!("{prefix}_oidc_silent"),
            api_title: format!("{product} API"),
        }
    }
}

/// What the notification endpoints need.
#[derive(Clone)]
pub struct NotifyApi {
    pub engine: Arc<notify::Engine>,
    pub deliverer: Arc<notify::Deliverer>,
    /// VAPID public key, when web push is configured.
    pub web_push_key: Option<String>,
}

#[derive(Clone)]
pub struct ApiState {
    pub app: App,
    pub auth: Auth,
    pub oidc: Option<Arc<auth::oidc::Oidc>>,
    pub bus: realtime::Bus,
    pub search: Arc<dyn SearchIndex>,
    pub fanout: SearchFanout,
    pub registry: Registry,
    pub assets: Option<Arc<assets::AssetService>>,
    pub importer: Option<Arc<importers::Importer>>,
    /// Notifications (inbox, channels, subscriptions, delivery log).
    pub notify: Option<NotifyApi>,
    /// Inbound integrations (the watched webhook).
    pub integrations: Option<Arc<integrations::Integrations>>,
    /// Public base URL, for links handed out to other systems.
    pub public_url: String,
    /// Per-user change feed for sync clients.
    pub changes: Option<Arc<dyn app::ports::ChangeFeed>>,
    pub naming: Naming,
    pub secure_cookies: bool,
    /// Enabled kinds searched when the client does not name one.
    pub default_kinds: Vec<String>,
    /// The admin area, credentials and setup (UX phase 2); `None` in small test setups.
    pub admin: Option<admin::AdminApi>,
}

impl ApiState {
    /// Send cookies with `Secure`? The admin setting when the admin area is wired (it
    /// applies to cookies issued from now on, OD-11), else the configuration.
    #[must_use]
    pub fn cookie_secure(&self) -> bool {
        self.admin
            .as_ref()
            .map_or(self.secure_cookies, admin::AdminApi::secure_cookies)
    }
}
