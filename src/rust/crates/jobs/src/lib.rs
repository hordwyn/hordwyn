//! Postgres job queue: lanes, priorities, dedupe keys, retry with backoff, cron, and a worker
//! with graceful shutdown (D-3, D-6.4).
//!
//! Claiming uses `FOR UPDATE SKIP LOCKED`, so any number of workers can share the table.
//! Ordering is strict: lane first (`interactive` > `user_import` > `background_refresh` >
//! `prefetch`), then priority, then `run_at`. Bulk imports therefore never delay interactive
//! work by more than the jobs already running. Running jobs hold a lease; a crashed worker's
//! jobs are claimed again once their lease expires.
#![deny(unsafe_code)]

mod queue;
mod worker;

pub use queue::{Job, PgQueue, QueueError, backoff};
pub use worker::{Handler, Worker, WorkerConfig};

use app::ports::Lane;

pub(crate) const fn lane_num(l: Lane) -> i16 {
    match l {
        Lane::Interactive => 0,
        Lane::UserImport => 1,
        Lane::BackgroundRefresh => 2,
        Lane::Prefetch => 3,
    }
}

pub(crate) fn lane_from(n: i16) -> Lane {
    match n {
        0 => Lane::Interactive,
        1 => Lane::UserImport,
        2 => Lane::BackgroundRefresh,
        _ => Lane::Prefetch,
    }
}
