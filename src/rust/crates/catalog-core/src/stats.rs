//! Per-connector counters kept by the governed client (design.md 4.4): calls and 429s of
//! the last 24 hours, the last answer per class, the circuit state.
//!
//! Every process (`--role web`, `--role worker`) records into its own [`SourceStats`]; with a
//! [`StatsStore`] each one flushes its deltas every 60 s ([`SourceStats::sync`]) and reads
//! back what all processes recorded (DEC-247, 326). A status read is the last database
//! snapshot plus the deltas not yet flushed. The marks (last OK, last error, ...) keep the
//! newest value of each field, so flushes from several processes merge in any order.

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
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

/// The newest value of each field (merged field by field across processes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatsMarks {
    pub last_ok_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub last_auth_status: Option<u16>,
    pub last_auth_failure_at: Option<DateTime<Utc>>,
    pub last_rate_limited_at: Option<DateTime<Utc>>,
    pub circuit_opened_at: Option<DateTime<Utc>>,
    pub circuit_open_until: Option<DateTime<Utc>>,
    pub key_changed_at: Option<DateTime<Utc>>,
}

fn newer(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a > b,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

impl StatsMarks {
    /// Take every field of `o` that is newer than ours.
    pub fn merge(&mut self, o: &Self) {
        self.last_ok_at = self.last_ok_at.max(o.last_ok_at);
        self.last_rate_limited_at = self.last_rate_limited_at.max(o.last_rate_limited_at);
        self.key_changed_at = self.key_changed_at.max(o.key_changed_at);
        if newer(o.last_error_at, self.last_error_at) {
            self.last_error_at = o.last_error_at;
            self.last_error.clone_from(&o.last_error);
        }
        if newer(o.last_auth_failure_at, self.last_auth_failure_at) {
            self.last_auth_failure_at = o.last_auth_failure_at;
            self.last_auth_status = o.last_auth_status;
        }
        if newer(o.circuit_opened_at, self.circuit_opened_at) {
            self.circuit_opened_at = o.circuit_opened_at;
            self.circuit_open_until = o.circuit_open_until;
        }
    }
}

/// Calls and 429s per hour since the Unix epoch.
pub type Hours = BTreeMap<i64, (u64, u64)>;

/// What one connector's counters hold: hourly counts and the marks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConnectorStats {
    pub hours: Hours,
    pub marks: StatsMarks,
}

impl ConnectorStats {
    fn add(&mut self, o: &Self) {
        for (h, (c, r)) in &o.hours {
            let e = self.hours.entry(*h).or_default();
            e.0 += c;
            e.1 += r;
        }
        self.marks.merge(&o.marks);
    }

    fn bucket(&mut self, now: DateTime<Utc>) -> &mut (u64, u64) {
        self.hours.entry(hour_of(now)).or_default()
    }
}

fn hour_of(t: DateTime<Utc>) -> i64 {
    t.timestamp().div_euclid(3600)
}

/// Where the processes add up their counters (`catalog_core::pg::PgSourceStats`).
#[async_trait]
pub trait StatsStore: Send + Sync {
    /// Add the hourly counts and merge the marks of every connector.
    async fn add(&self, deltas: &HashMap<String, ConnectorStats>) -> Result<(), PortError>;
    /// Everything recorded from `since_hour` on, per connector.
    async fn load(&self, since_hour: i64) -> Result<HashMap<String, ConnectorStats>, PortError>;
}

#[derive(Default)]
struct State {
    /// Recorded here, not flushed yet.
    pending: HashMap<String, ConnectorStats>,
    /// Being written by a flush right now.
    flushing: HashMap<String, ConnectorStats>,
    /// Written, but not yet part of a snapshot (the load after the write failed).
    inflight: HashMap<String, ConnectorStats>,
    /// The database as of the last successful load.
    snapshot: HashMap<String, ConnectorStats>,
}

/// The second field serialises [`SourceStats::sync`]: two flushes at once would overwrite
/// `flushing` and lose or double-count deltas (UX4-22).
#[derive(Default)]
pub struct SourceStats(Mutex<State>, tokio::sync::Mutex<()>);

impl SourceStats {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn with(&self, id: &str, f: impl FnOnce(&mut ConnectorStats, DateTime<Utc>)) {
        let now = Utc::now();
        let mut s = self.state();
        let e = s.pending.entry(id.to_string()).or_default();
        // without a store nothing is ever flushed: keep only the window
        e.hours = e.hours.split_off(&(hour_of(now) - 23));
        f(e, now);
    }

    /// A request was sent upstream.
    pub fn record_call(&self, id: &str) {
        self.with(id, |e, now| e.bucket(now).0 += 1);
    }

    /// The upstream answered `status`.
    pub fn record_status(&self, id: &str, status: u16) {
        self.with(id, |e, now| match status {
            401 | 403 => {
                e.marks.last_auth_status = Some(status);
                e.marks.last_auth_failure_at = Some(now);
            }
            429 => {
                e.bucket(now).1 += 1;
                e.marks.last_rate_limited_at = Some(now);
            }
            s if s >= 500 => {
                e.marks.last_error_at = Some(now);
                e.marks.last_error = Some(format!("HTTP {s}"));
            }
            _ => e.marks.last_ok_at = Some(now),
        });
    }

    /// The request failed without an answer.
    pub fn record_error(&self, id: &str, what: &str) {
        self.with(id, |e, now| {
            e.marks.last_error_at = Some(now);
            e.marks.last_error = Some(what.to_string());
        });
    }

    /// The circuit opened for `cooldown`.
    pub fn record_circuit_open(&self, id: &str, cooldown: std::time::Duration) {
        self.with(id, |e, now| {
            e.marks.circuit_opened_at = Some(now);
            e.marks.circuit_open_until =
                Some(now + Duration::from_std(cooldown).unwrap_or(Duration::zero()));
        });
    }

    /// The key was replaced: earlier rejections no longer count (in every process after the
    /// next flush).
    pub fn key_changed(&self, id: &str) {
        self.with(id, |e, now| e.marks.key_changed_at = Some(now));
    }

    /// Did the provider reject the current key?
    #[must_use]
    pub fn rejected(&self, id: &str) -> bool {
        self.counters(id).last_auth_status.is_some()
    }

    /// The snapshot, the deltas in flight and the pending ones of `id`, merged.
    fn merged(&self, id: &str) -> ConnectorStats {
        let s = self.state();
        let mut all = ConnectorStats::default();
        for part in [&s.snapshot, &s.inflight, &s.flushing, &s.pending] {
            if let Some(c) = part.get(id) {
                all.add(c);
            }
        }
        all
    }

    #[must_use]
    pub fn counters(&self, id: &str) -> SourceCounters {
        let now = Utc::now();
        let all = self.merged(id);
        let since = hour_of(now) - 23;
        let m = &all.marks;
        let in_window = all.hours.range(since..);
        let auth_counts = newer(m.last_auth_failure_at, m.key_changed_at);
        let circuit_open = m.circuit_open_until.is_some_and(|u| u > now)
            && !newer(m.last_ok_at, m.circuit_opened_at);
        SourceCounters {
            calls_24h: in_window.clone().map(|(_, (c, _))| c).sum(),
            rate_limited_24h: in_window.map(|(_, (_, r))| r).sum(),
            last_ok_at: m.last_ok_at,
            last_error_at: m.last_error_at,
            last_error: m.last_error.clone(),
            last_auth_status: if auth_counts {
                m.last_auth_status
            } else {
                None
            },
            last_auth_failure_at: if auth_counts {
                m.last_auth_failure_at
            } else {
                None
            },
            last_rate_limited_at: m.last_rate_limited_at,
            circuit_open_until: if circuit_open {
                m.circuit_open_until
            } else {
                None
            },
        }
    }

    /// Flush the deltas recorded since the last flush into `store` and read back what every
    /// process recorded in the last 24 hours. A failed write keeps the deltas for the next
    /// flush; nothing is counted twice.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn sync(&self, store: &dyn StatsStore) -> Result<(), PortError> {
        let _one_at_a_time = self.1.lock().await;
        let deltas = {
            let mut s = self.state();
            let pending = std::mem::take(&mut s.pending);
            s.flushing.clone_from(&pending);
            pending
        };
        if !deltas.is_empty() {
            let written = store.add(&deltas).await;
            let mut s = self.state();
            let flushed = std::mem::take(&mut s.flushing);
            // written: shown from `inflight` until a load has it; failed: the next flush
            // tries again
            let target = if written.is_ok() {
                &mut s.inflight
            } else {
                &mut s.pending
            };
            for (id, c) in flushed {
                target.entry(id).or_default().add(&c);
            }
            written?;
        }
        let loaded = store.load(hour_of(Utc::now()) - 23).await?;
        let mut s = self.state();
        s.snapshot = loaded;
        s.inflight.clear();
        Ok(())
    }
}
