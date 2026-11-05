//! In-memory fakes for every port, fixtures and recorded HTTP.
#![deny(unsafe_code)]
// Test support only: the fakes panic on poisoned mutexes, which only happens after another
// test thread already panicked.
#![allow(clippy::missing_panics_doc)]

pub mod fakes;
pub mod kinds;
pub mod store;

use std::sync::Arc;

use app::{App, Repos};

pub use fakes::{
    FixedClock, MemBlobs, MemJobs, MemSearch, RecordingEvents, RecordingNotifier, ReplayHttp,
    SeqIds,
};
pub use kinds::StaticKinds;
pub use store::MemStore;

/// An [`App`] wired to in-memory fakes, with handles to inspect them.
pub struct TestEnv {
    pub app: App,
    pub store: Arc<MemStore>,
    pub clock: Arc<FixedClock>,
    pub ids: Arc<SeqIds>,
    pub events: Arc<RecordingEvents>,
    pub jobs: Arc<MemJobs>,
}

impl TestEnv {
    /// Fakes with the small built-in [`StaticKinds`] set.
    #[must_use]
    pub fn new() -> Self {
        Self::with_kinds(Arc::new(StaticKinds::default()))
    }

    /// Fakes with a custom kind catalog (for example the real `kinds` registry).
    #[must_use]
    pub fn with_kinds(kinds: Arc<dyn app::ports::KindCatalog>) -> Self {
        let store = Arc::new(MemStore::default());
        let clock = Arc::new(FixedClock::default());
        let ids = Arc::new(SeqIds::default());
        let events = Arc::new(RecordingEvents::default());
        let jobs = Arc::new(MemJobs::default());
        let app = App {
            repos: Repos {
                users: store.clone(),
                companions: store.clone(),
                link_requests: store.clone(),
                invites: store.clone(),
                catalog: store.clone(),
                library: store.clone(),
                progress: store.clone(),
                reviews: store.clone(),
                lists: store.clone(),
                sessions: store.clone(),
            },
            kinds,
            clock: clock.clone(),
            ids: ids.clone(),
            events: events.clone(),
            jobs: jobs.clone(),
        };
        Self {
            app,
            store,
            clock,
            ids,
            events,
            jobs,
        }
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}
