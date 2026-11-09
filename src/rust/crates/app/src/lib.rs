//! Use-cases and ports (traits). Depends on `domain` only (+ async-trait, thiserror).
//!
//! [`App`] holds one adapter per port and exposes every use-case as an async method. The
//! REST API, server functions, importers and the sync endpoint all call these methods.
#![deny(unsafe_code)]

mod activity;
mod browse;
mod bulk;
mod catalog;
mod companions;
mod cursor;
mod episodes;
mod error;
mod game_groups;
mod goals;
mod library;
mod lists;
mod next_step;
mod people;
pub mod ports;
mod sessions;
mod stats;
pub mod time_zone;
mod users;

use std::sync::Arc;

pub use activity::{
    ActivityPage, ActivityPerson, ActivityQuery, ActivityRow, ActivityType, MAX_ACTIVITY_PAGE,
};
pub use browse::{
    CountCell, DEFAULT_PAGE, LibraryCounts, LibraryPage, LibraryQuery, LibraryRow, TagCount,
    UpNext, UpNextReason, series_of,
};
pub use bulk::{
    BulkFailure, BulkOp, BulkOutcome, BulkUndo, EntrySnapshot, MAX_BULK, MAX_UNDO_BYTES, StatusBack,
};
pub use catalog::{CatalogRefresh, ITEM_WATCH_SECS, NewCatalogItem, REFRESH_JOB};
pub use episodes::{EPISODES_JOB, SeasonFetch, SeasonKey, season_keys, season_needs_fetch};
pub use error::{AppError, AppResult};
pub use game_groups::{GameGroupInput, GameGroupView};
pub use goals::{ComparePair, GoalProgress, NewGoal, YearCompare, same_day_last_year};
pub use library::UNDO_WINDOW_HOURS;
pub use next_step::{NextStep, next_step};
pub use people::{CompanionStats, MergeSuggestion, MostPlayed, StatItem, suggest_merges};
pub use sessions::NewSession;
pub use stats::{
    Amounts, Bucket, GroupTotals, KindAmounts, MonthAmounts, SeriesPoint, StatsTotals, Totals,
    month_amounts, window_amounts,
};

use domain::Timestamp;
use domain::hlc::Hlc;
use ports::{
    AppEvent, CatalogRepo, Clock, CompanionRepo, EventBus, IdGen, InviteRepo, JobQueue,
    KindCatalog, LibraryRepo, LinkRequestRepo, ListRepo, Notification, Notifications, ProgressRepo,
    ReviewRepo, SessionRepo, UserRepo,
};

/// How far ahead of the server clock a client's HLC stamp may be (5 minutes).
pub const MAX_CLOCK_DRIFT_MS: u64 = 5 * 60 * 1000;

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
    pub jobs: Arc<dyn JobQueue>,
    pub notifications: Arc<dyn Notifications>,
}

impl App {
    fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// HLC stamp for a write: the client's stamp if it sent one, else a fresh server stamp.
    /// Client stamps more than [`MAX_CLOCK_DRIFT_MS`] ahead of the server are not trusted: a
    /// device with a wrong clock would otherwise win every later write and drag the server
    /// clock along (DEC-126). Such writes get a fresh server stamp instead.
    fn stamp(&self, client: Option<Hlc>) -> Hlc {
        let now_ms = u64::try_from(self.now().timestamp_millis()).unwrap_or(0);
        match client {
            Some(h) if h.wall_ms <= now_ms.saturating_add(MAX_CLOCK_DRIFT_MS) => {
                self.clock.observe(h);
                h
            }
            _ => self.clock.hlc(),
        }
    }

    fn uuid(&self) -> uuid::Uuid {
        self.ids.new_id()
    }

    async fn publish(&self, event: AppEvent) {
        self.events.publish(event).await;
    }

    async fn notify(&self, n: Notification) {
        self.notifications.emit(n).await;
    }
}
