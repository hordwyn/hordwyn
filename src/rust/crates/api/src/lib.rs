//! Versioned REST API `/api/v1` with an OpenAPI document (utoipa) and the SSE event stream.
//!
//! Handlers only call `app` use-cases (and `auth`); none of them awaits a provider: item
//! detail enqueues a refresh, search answers from local data and streams connector results
//! as `search.results` events (D-6).
#![deny(unsafe_code)]

mod dto;
mod error;
mod extract;
pub mod ops;
mod routes;

use std::sync::Arc;

use app::App;
use app::ports::SearchIndex;
use auth::Auth;
use catalog_core::Registry;
use catalog_core::search::SearchFanout;

pub use error::ApiError;
pub use extract::{Authed, session_token};
pub use routes::router;

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
    /// Title of the OpenAPI document.
    pub api_title: String,
}

impl Naming {
    #[must_use]
    pub fn from_prefix(prefix: &str, product: &str) -> Self {
        Self {
            session_cookie: format!("{prefix}_session"),
            oidc_cookie: format!("{prefix}_oidc"),
            api_title: format!("{product} API"),
        }
    }
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
    pub naming: Naming,
    pub secure_cookies: bool,
    /// Enabled kinds searched when the client does not name one.
    pub default_kinds: Vec<String>,
}
