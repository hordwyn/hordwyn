//! Per-connector counters kept by the governed client (design.md 4.4): calls and 429s of
//! the last 24 hours, the last answer per class, the circuit state. In memory, per process.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};

/// What the status rows read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceCounters {
    pub calls_24h: u64,
    pub rate_limited_24h: u64,
    pub last_ok_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    /// Short, secret-free description (`HTTP 500`, `network`).
    pub last_error: Option<String>,
    /// 401 or 403 of the last rejection since the key last changed.
    pub last_auth_status: Option<u16>,
    pub last_auth_failure_at: Option<DateTime<Utc>>,
    pub last_rate_limited_at: Option<DateTime<Utc>>,
    pub circuit_open_until: Option<DateTime<Utc>>,
}

#[derive(Default)]
struct Entry {
    /// (hour since epoch, calls, 429s), oldest first, at most 24.
    hours: VecDeque<(i64, u64, u64)>,
    c: SourceCounters,
}

impl Entry {
    fn bucket(&mut self, now: DateTime<Utc>) -> &mut (i64, u64, u64) {
        let hour = now.timestamp().div_euclid(3600);
        while self.hours.front().is_some_and(|(h, _, _)| *h <= hour - 24) {
            self.hours.pop_front();
        }
        if self.hours.back().is_none_or(|(h, _, _)| *h != hour) {
            self.hours.push_back((hour, 0, 0));
        }
        self.hours.back_mut().expect("bucket just pushed")
    }
}

#[derive(Default)]
pub struct SourceStats(Mutex<HashMap<String, Entry>>);

impl SourceStats {
    fn with<R>(&self, id: &str, f: impl FnOnce(&mut Entry, DateTime<Utc>) -> R) -> R {
        let mut m = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(m.entry(id.to_string()).or_default(), Utc::now())
    }

    /// A request was sent upstream.
    pub fn record_call(&self, id: &str) {
        self.with(id, |e, now| e.bucket(now).1 += 1);
    }

    /// The upstream answered `status`.
    pub fn record_status(&self, id: &str, status: u16) {
        self.with(id, |e, now| match status {
            401 | 403 => {
                e.c.last_auth_status = Some(status);
                e.c.last_auth_failure_at = Some(now);
            }
            429 => {
                e.bucket(now).2 += 1;
                e.c.last_rate_limited_at = Some(now);
            }
            s if s >= 500 => {
                e.c.last_error_at = Some(now);
                e.c.last_error = Some(format!("HTTP {s}"));
            }
            _ => {
                e.c.last_ok_at = Some(now);
                e.c.circuit_open_until = None;
            }
        });
    }

    /// The request failed without an answer.
    pub fn record_error(&self, id: &str, what: &str) {
        self.with(id, |e, now| {
            e.c.last_error_at = Some(now);
            e.c.last_error = Some(what.to_string());
        });
    }

    /// The circuit opened for `cooldown`.
    pub fn record_circuit_open(&self, id: &str, cooldown: std::time::Duration) {
        self.with(id, |e, now| {
            e.c.circuit_open_until =
                Some(now + Duration::from_std(cooldown).unwrap_or(Duration::zero()));
        });
    }

    /// The key was replaced: earlier rejections no longer count.
    pub fn key_changed(&self, id: &str) {
        self.with(id, |e, _| {
            e.c.last_auth_status = None;
            e.c.last_auth_failure_at = None;
        });
    }

    /// Did the provider reject the current key?
    #[must_use]
    pub fn rejected(&self, id: &str) -> bool {
        self.counters(id).last_auth_status.is_some()
    }

    #[must_use]
    pub fn counters(&self, id: &str) -> SourceCounters {
        self.with(id, |e, now| {
            e.bucket(now);
            let mut c = e.c.clone();
            c.calls_24h = e.hours.iter().map(|h| h.1).sum();
            c.rate_limited_24h = e.hours.iter().map(|h| h.2).sum();
            if c.circuit_open_until.is_some_and(|u| u <= now) {
                c.circuit_open_until = None;
            }
            c
        })
    }
}
