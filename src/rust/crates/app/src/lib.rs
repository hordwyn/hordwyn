//! Use-cases and ports (traits). Depends on `domain` only (+ async-trait, thiserror).
//!
//! [`App`] holds one adapter per port and exposes every use-case as an async method. The
//! REST API, server functions, importers and the sync endpoint all call these methods.
#![deny(unsafe_code)]

mod catalog;
mod companions;
mod error;
mod library;
mod lists;
pub mod ports;
mod sessions;
mod users;

use std::sync::Arc;

pub use catalog::NewCatalogItem;
pub use error::{AppError, AppResult};
pub use sessions::NewSession;

use domain::Timestamp;
use domain::hlc::Hlc;
use ports::{
    AppEvent, CatalogRepo, Clock, CompanionRepo, EventBus, IdGen, InviteRepo, KindCatalog,
    LibraryRepo, LinkRequestRepo, ListRepo, ProgressRepo, ReviewRepo, SessionRepo, UserRepo,
};

/// Repository adapters, one per aggregate.
#[derive(Clone)]
pub struct Repos {
    pub users: Arc<dyn UserRepo>,
    pub companions: Arc<dyn CompanionRepo>,
    pub link_requests: Arc<dyn LinkRequestRepo>,
    pub invites: Arc<dyn InviteRepo>,
    pub catalog: Arc<dyn CatalogRepo>,
    pub library: Arc<dyn LibraryRepo>,
    pub progress: Arc<dyn ProgressRepo>,
    pub reviews: Arc<dyn ReviewRepo>,
    pub lists: Arc<dyn ListRepo>,
    pub sessions: Arc<dyn SessionRepo>,
}

/// The application core: use-cases over ports.
#[derive(Clone)]
pub struct App {
    pub repos: Repos,
    pub kinds: Arc<dyn KindCatalog>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdGen>,
    pub events: Arc<dyn EventBus>,
}

impl App {
    fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// HLC stamp for a write: the client's stamp if it sent one, else a fresh server stamp.
    fn stamp(&self, client: Option<Hlc>) -> Hlc {
        client.unwrap_or_else(|| self.clock.hlc())
    }

    fn uuid(&self) -> uuid::Uuid {
        self.ids.new_id()
    }

    async fn publish(&self, event: AppEvent) {
        self.events.publish(event).await;
    }
}
