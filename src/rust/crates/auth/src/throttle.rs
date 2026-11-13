//! Slowing down guesses (SEC-5, DEC-535): every place that checks a password, a one-time
//! code, a recovery code, a reset link or an emailed link goes through [`Throttle`].
//!
//! Four rules, none of them a lock a stranger can set:
//! - The pair of account and client address waits after [`ThrottleSettings::free_failures`]
//!   failures, twice as long after each further one, up to
//!   [`ThrottleSettings::delay_max_secs`]. The count belongs to the pair, so guesses from
//!   one address never make the account wait for another address: nobody can keep the
//!   owner out of the local sign-in by guessing (OD-17).
//! - A client address has a budget of failures across all names
//!   ([`ThrottleSettings::address_failures`] per window); attempts in flight count against
//!   it, so a burst cannot run ahead of its own failures. Over budget, the address is
//!   refused for every name until the window ends.
//! - Wrong second factors (one-time and recovery codes) of an account are counted for the
//!   account alone, whatever the address, with the same growing wait (SEC2-1). Only someone
//!   who sent the right password reaches that check, so guesses by strangers never make the
//!   owner wait there.
//! - At most [`ThrottleSettings::verify_concurrency`] argon2 verifications run at once; the
//!   next ones queue.
//!
//! The counts live behind [`AttemptCounters`]; [`MemCounters`] keeps them in the process
//! (one web process is the supported shape, HANDOFF 4.8). The client address comes from the
//! request layer through [`client::scope`]; callers without one (the command line) are
//! slowed per account only.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

use crate::AuthError;

/// The limits (configuration `auth.throttle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrottleSettings {
    /// Failures of one account from one address that cost no wait.
    pub free_failures: u32,
    /// The wait after the first failure beyond the free ones; it doubles from there.
    pub delay_base_secs: u32,
    /// The longest wait.
    pub delay_max_secs: u32,
    /// Minutes without a failure after which a pair starts again, and the length of the
    /// budget window of an address.
    pub window_minutes: u32,
    /// Failures of one client address across all names per window.
    pub address_failures: u32,
    /// Password verifications that run at once (read when [`crate::Auth`] is built).
    pub verify_concurrency: u32,
}

impl Default for ThrottleSettings {
    fn default() -> Self {
        Self {
            free_failures: 3,
            delay_base_secs: 1,
            delay_max_secs: 300,
            window_minutes: 15,
            address_failures: 30,
            verify_concurrency: 8,
        }
    }
}

impl ThrottleSettings {
    /// The wait after `failures` failures of a pair.
    fn delay(&self, failures: u32) -> Duration {
        if failures <= self.free_failures {
            return Duration::zero();
        }
        let over = failures - self.free_failures - 1;
        let secs = u64::from(self.delay_base_secs)
            .checked_shl(over)
            .filter(|s| s >> over == u64::from(self.delay_base_secs))
            .unwrap_or(u64::MAX)
            .min(u64::from(self.delay_max_secs));
        Duration::seconds(i64::try_from(secs).unwrap_or(i64::MAX))
    }

    fn window(&self) -> Duration {
        Duration::minutes(i64::from(self.window_minutes.max(1)))
    }

    /// How long a pair's count is kept after its last failure.
    fn pair_window(&self) -> Duration {
        self.window()
            .max(Duration::seconds(i64::from(self.delay_max_secs)))
    }
}

/// The client address of the request being handled.
pub mod client {
    use std::future::Future;
    use std::net::IpAddr;

    tokio::task_local! {
        static CLIENT: Option<IpAddr>;
    }

    /// Run `f` as a request from `addr` (an IP address; anything else counts as unknown).
    pub async fn scope<F: Future>(addr: Option<&str>, f: F) -> F::Output {
        let ip = addr.and_then(|a| a.trim().parse::<IpAddr>().ok());
        CLIENT.scope(ip, f).await
    }

    /// The address set by [`scope`], if any.
    #[must_use]
    pub fn current() -> Option<IpAddr> {
        CLIENT.try_with(|c| *c).ok().flatten()
    }
}

/// What the budget of an address is counted for: the address itself for IPv4, the /64
/// network for IPv6 (one customer usually holds all of it).
fn address_key(ip: IpAddr) -> String {
    let v6 = match ip {
        IpAddr::V4(v4) => return v4.to_string(),
        IpAddr::V6(v6) => v6,
    };
    if let Some(v4) = v6.to_ipv4_mapped() {
        return v4.to_string();
    }
    let s = v6.segments();
    format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
}

/// The name part of a pair key. A name longer than any username cannot be an account; all
/// such names share one placeholder, so a submitted name never makes a key large (SEC2-5).
fn name_key(name: &str) -> &str {
    if name.len() > contracts::auth::USERNAME_MAX_LEN {
        "~too-long"
    } else {
        name
    }
}

/// The key of the count of wrong second factors (one-time and recovery codes) of an
/// account, whatever the address (SEC2-1).
fn code_key(account: &str) -> String {
    format!("t|{}", name_key(account))
}

/// Failures counted for one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    pub failures: u32,
    /// The first failure of this count.
    pub first: DateTime<Utc>,
    /// The latest failure.
    pub last: DateTime<Utc>,
}

/// When a count starts again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    /// A fixed window: `window` after the first failure.
    AfterFirst,
    /// A quiet time: `window` after the latest failure.
    AfterLast,
}

impl Count {
    fn over(&self, now: DateTime<Utc>, window: Duration, restart: Restart) -> bool {
        let from = match restart {
            Restart::AfterFirst => self.first,
            Restart::AfterLast => self.last,
        };
        now >= from + window
    }
}

/// Where the failure counts are kept. A store shared by several web processes would
/// implement this (DEC-535, the separable last step).
#[async_trait]
pub trait AttemptCounters: Send + Sync {
    /// The count of `key`, unless it is over.
    async fn get(
        &self,
        key: &str,
        now: DateTime<Utc>,
        window: Duration,
        restart: Restart,
    ) -> Result<Option<Count>, PortError>;

    /// Count one more failure of `key` (a count that is over starts again at one).
    async fn add(
        &self,
        key: &str,
        now: DateTime<Utc>,
        window: Duration,
        restart: Restart,
    ) -> Result<Count, PortError>;

    /// Forget the count of `key`.
    async fn clear(&self, key: &str) -> Result<(), PortError>;
}

/// Keys kept at most. When the map is full, keys whose own window is over go first, then
/// pairs and code counts (fewest failures, stalest first), address budgets last: evicting
/// an exhausted budget would hand it back (SEC2-6).
const CAPACITY: usize = 20_000;

/// One count with the window and restart rule it was last added with, so a purge ends
/// every key by its own rule.
#[derive(Debug, Clone, Copy)]
struct Kept {
    count: Count,
    window: Duration,
    restart: Restart,
}

/// Counts in the memory of this process.
#[derive(Default)]
pub struct MemCounters(Mutex<HashMap<String, Kept>>);

/// Which keys a full map gives up first (lower first).
fn eviction_rank(key: &str) -> u8 {
    u8::from(key.starts_with("a|"))
}

#[async_trait]
impl AttemptCounters for MemCounters {
    async fn get(
        &self,
        key: &str,
        now: DateTime<Utc>,
        window: Duration,
        restart: Restart,
    ) -> Result<Option<Count>, PortError> {
        let map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(map
            .get(key)
            .map(|k| k.count)
            .filter(|c| !c.over(now, window, restart)))
    }

    async fn add(
        &self,
        key: &str,
        now: DateTime<Utc>,
        window: Duration,
        restart: Restart,
    ) -> Result<Count, PortError> {
        let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if map.len() >= CAPACITY && !map.contains_key(key) {
            map.retain(|_, k| !k.count.over(now, k.window, k.restart));
            if map.len() >= CAPACITY
                && let Some(victim) = map
                    .iter()
                    .min_by_key(|(key, k)| (eviction_rank(key), k.count.failures, k.count.last))
                    .map(|(key, _)| key.clone())
            {
                map.remove(&victim);
            }
        }
        let fresh = Count {
            failures: 0,
            first: now,
            last: now,
        };
        let k = map.entry(key.to_string()).or_insert(Kept {
            count: fresh,
            window,
            restart,
        });
        if k.count.over(now, window, restart) {
            k.count = fresh;
        }
        k.window = window;
        k.restart = restart;
        k.count.failures = k.count.failures.saturating_add(1);
        k.count.last = now;
        Ok(k.count)
    }

    async fn clear(&self, key: &str) -> Result<(), PortError> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(key);
        Ok(())
    }
}

#[cfg(test)]
impl MemCounters {
    fn key_bytes(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .keys()
            .map(String::len)
            .sum()
    }
}

/// At most `n` password verifications at once; the others wait their turn.
pub(crate) struct VerifyGate(tokio::sync::Semaphore);

impl VerifyGate {
    pub(crate) fn new(n: u32) -> Self {
        Self(tokio::sync::Semaphore::new(n.max(1) as usize))
    }

    /// Wait for a free slot; the slot is free again when the permit is dropped.
    pub(crate) async fn enter(&self) -> Option<tokio::sync::SemaphorePermit<'_>> {
        // the semaphore is never closed
        self.0.acquire().await.ok()
    }

    #[cfg(test)]
    fn free(&self) -> usize {
        self.0.available_permits()
    }
}

/// The attempts of one address that are being checked right now.
#[derive(Default)]
struct InFlight(Mutex<HashMap<String, u32>>);

impl InFlight {
    fn count(&self, key: &str) -> u32 {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .copied()
            .unwrap_or(0)
    }

    fn enter(&self, key: &str) {
        *self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(key.to_string())
            .or_insert(0) += 1;
    }

    fn leave(&self, key: &str) {
        let mut map = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(n) = map.get_mut(key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                map.remove(key);
            }
        }
    }
}

pub(crate) struct Throttle {
    counters: Arc<dyn AttemptCounters>,
    in_flight: Arc<InFlight>,
    pub(crate) gate: VerifyGate,
}

/// One attempt that passed [`Throttle::begin`]. Settle it with [`Throttle::failed`] or
/// [`Throttle::succeeded`]; dropped without either (an error that says nothing about the
/// secret), it only gives its place in the address budget back.
pub(crate) struct Attempt {
    pair: Option<String>,
    address: Option<String>,
    in_flight: Arc<InFlight>,
}

impl Drop for Attempt {
    fn drop(&mut self) {
        if let Some(a) = &self.address {
            self.in_flight.leave(a);
        }
    }
}

fn wait(until: DateTime<Utc>, now: DateTime<Utc>) -> AuthError {
    // rounded up: "try again in 1 s" must be true after that second
    let ms = (until - now).num_milliseconds().max(1);
    AuthError::TooManyAttempts {
        retry_after_secs: (ms + 999) / 1000,
    }
}

impl Throttle {
    pub(crate) fn new(counters: Arc<dyn AttemptCounters>, verify_concurrency: u32) -> Self {
        Self {
            counters,
            in_flight: Arc::default(),
            gate: VerifyGate::new(verify_concurrency),
        }
    }

    /// May a check for `account` (a normalised username; `None` when the secret names no
    /// account, as a reset link) run now, from the current client address?
    pub(crate) async fn begin(
        &self,
        s: &ThrottleSettings,
        account: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<Attempt, AuthError> {
        let ip = client::current();
        let address = ip.map(|ip| format!("a|{}", address_key(ip)));
        let pair = account.map(|name| {
            let name = name_key(name);
            match ip {
                // the exact address: neighbours in a network do not share the wait of a pair
                Some(ip) => format!("p|{name}|{ip}"),
                None => format!("p|{name}|-"),
            }
        });
        if let Some(a) = &address {
            let counted = self
                .counters
                .get(a, now, s.window(), Restart::AfterFirst)
                .await?;
            let failures = counted.map_or(0, |c| c.failures);
            if failures.saturating_add(self.in_flight.count(a)) >= s.address_failures.max(1) {
                let until = counted.map_or(now + Duration::seconds(1), |c| c.first + s.window());
                return Err(wait(until, now));
            }
        }
        if let Some(p) = &pair
            && let Some(c) = self
                .counters
                .get(p, now, s.pair_window(), Restart::AfterLast)
                .await?
        {
            let until = c.last + s.delay(c.failures);
            if until > now {
                return Err(wait(until, now));
            }
        }
        if let Some(a) = &address {
            self.in_flight.enter(a);
        }
        Ok(Attempt {
            pair,
            address,
            in_flight: self.in_flight.clone(),
        })
    }

    /// The secret was wrong.
    pub(crate) async fn failed(
        &self,
        s: &ThrottleSettings,
        attempt: Attempt,
        now: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        if let Some(p) = &attempt.pair {
            self.counters
                .add(p, now, s.pair_window(), Restart::AfterLast)
                .await?;
        }
        if let Some(a) = &attempt.address {
            self.counters
                .add(a, now, s.window(), Restart::AfterFirst)
                .await?;
        }
        Ok(())
    }

    /// May a second factor of `account` be checked now? Called only after the password was
    /// right, so nobody without the password can drive this count up; it holds for the
    /// account whatever the address (SEC2-1).
    pub(crate) async fn code_begin(
        &self,
        s: &ThrottleSettings,
        account: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        if let Some(c) = self
            .counters
            .get(&code_key(account), now, s.pair_window(), Restart::AfterLast)
            .await?
        {
            let until = c.last + s.delay(c.failures);
            if until > now {
                return Err(wait(until, now));
            }
        }
        Ok(())
    }

    /// Count the outcome of a second-factor check of `account`: a wrong code is a failure,
    /// a right one starts the count again, other errors count as neither.
    pub(crate) async fn code_settle<T>(
        &self,
        s: &ThrottleSettings,
        account: &str,
        result: &Result<T, AuthError>,
        now: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        let key = code_key(account);
        match result {
            Err(AuthError::InvalidTotp) => {
                self.counters
                    .add(&key, now, s.pair_window(), Restart::AfterLast)
                    .await?;
            }
            Ok(_) => self.counters.clear(&key).await?,
            Err(_) => {}
        }
        Ok(())
    }

    /// The secret was right: the pair starts again. The budget of the address stays as it
    /// is, or one known account would refill it for guesses at the others.
    pub(crate) async fn succeeded(&self, attempt: Attempt) -> Result<(), AuthError> {
        if let Some(p) = &attempt.pair {
            self.counters.clear(p).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delay_doubles_from_the_base_up_to_the_maximum() {
        let s = ThrottleSettings::default();
        let secs: Vec<i64> = (0..=14).map(|n| s.delay(n).num_seconds()).collect();
        assert_eq!(
            secs,
            [0, 0, 0, 0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300]
        );
        assert_eq!(s.delay(u32::MAX).num_seconds(), 300);
        let none = ThrottleSettings {
            delay_base_secs: 0,
            ..s
        };
        assert_eq!(none.delay(50).num_seconds(), 0);
    }

    #[test]
    fn address_keys() {
        let key = |s: &str| address_key(s.parse().unwrap());
        assert_eq!(key("203.0.113.7"), "203.0.113.7");
        assert_eq!(key("::ffff:203.0.113.7"), "203.0.113.7");
        assert_eq!(key("2001:db8:1:2:3:4:5:6"), "2001:db8:1:2::/64");
        assert_eq!(key("2001:db8:1:2::1"), key("2001:db8:1:2:ffff::9"));
        assert_ne!(key("2001:db8:1:2::1"), key("2001:db8:1:3::1"));
    }

    /// SEC-5: the cap queues the ninth concurrent verification until one of the eight ends.
    #[tokio::test]
    async fn the_ninth_concurrent_verification_waits_for_a_free_slot() {
        let gate = Arc::new(VerifyGate::new(
            ThrottleSettings::default().verify_concurrency,
        ));
        let mut held = Vec::new();
        for _ in 0..8 {
            held.push(gate.enter().await);
        }
        assert_eq!(gate.free(), 0);
        let ninth = tokio::spawn({
            let gate = gate.clone();
            async move {
                let _slot = gate.enter().await;
            }
        });
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        assert!(!ninth.is_finished(), "the ninth waits");
        held.pop();
        ninth.await.unwrap();
    }

    /// SEC2-14: a huge number of free failures neither overflows nor ever waits.
    #[test]
    fn free_failures_at_the_maximum_do_not_overflow() {
        let s = ThrottleSettings {
            free_failures: u32::MAX,
            ..ThrottleSettings::default()
        };
        assert_eq!(s.delay(u32::MAX).num_seconds(), 0);
        assert_eq!(s.delay(7).num_seconds(), 0);
    }

    /// SEC2-5: the submitted name is part of a counter key; over-long names (no account can
    /// have one) do not make the keys large.
    #[tokio::test]
    async fn over_long_names_make_bounded_keys() {
        let counters = Arc::new(MemCounters::default());
        let t = Throttle::new(counters.clone(), 8);
        let s = ThrottleSettings {
            address_failures: 1000,
            ..ThrottleSettings::default()
        };
        let now = Utc::now();
        client::scope(Some("203.0.113.7"), async {
            for n in 0..31u8 {
                let name = format!("{n}{}", "x".repeat(200 * 1024));
                // over-long names share one pair, which soon waits
                if let Ok(a) = t.begin(&s, Some(&name), now).await {
                    t.failed(&s, a, now).await.unwrap();
                }
            }
        })
        .await;
        let bytes = counters.key_bytes();
        assert!(bytes < 4 * 1024, "{bytes} bytes of keys");
    }

    /// SEC2-6: a full map gives up a pair before the exhausted budget of an address, and
    /// purges each key by its own window.
    #[tokio::test]
    async fn a_full_map_keeps_address_budgets_before_pairs() {
        let c = MemCounters::default();
        let s = ThrottleSettings::default();
        let t0 = Utc::now();
        c.add("a|198.51.100.20", t0, s.window(), Restart::AfterFirst)
            .await
            .unwrap();
        for n in 1..CAPACITY {
            let at = t0 + Duration::milliseconds(i64::try_from(n).unwrap());
            c.add(
                &format!("p|n{n}|x"),
                at,
                s.pair_window(),
                Restart::AfterLast,
            )
            .await
            .unwrap();
        }
        // a key with a short window must not purge the address budget, whose window is on
        let late = t0 + Duration::minutes(10);
        c.add("p|new|x", late, Duration::minutes(5), Restart::AfterLast)
            .await
            .unwrap();
        let kept = c
            .get("a|198.51.100.20", late, s.window(), Restart::AfterFirst)
            .await
            .unwrap();
        assert!(kept.is_some(), "the address budget stays");
    }

    #[tokio::test]
    async fn attempts_in_flight_count_against_the_address() {
        let t = Throttle::new(Arc::new(MemCounters::default()), 8);
        let s = ThrottleSettings {
            address_failures: 2,
            ..ThrottleSettings::default()
        };
        let now = Utc::now();
        client::scope(Some("203.0.113.7"), async {
            let first = t.begin(&s, Some("anna"), now).await.unwrap();
            let second = t.begin(&s, Some("ben"), now).await.unwrap();
            // a third while two are being checked would be the third guess of two allowed
            assert!(matches!(
                t.begin(&s, Some("cara"), now).await,
                Err(AuthError::TooManyAttempts { .. })
            ));
            // one that ends without a verdict gives its place back
            drop(first);
            let third = t.begin(&s, Some("cara"), now).await.unwrap();
            t.failed(&s, second, now).await.unwrap();
            t.failed(&s, third, now).await.unwrap();
            let e = t.begin(&s, Some("dora"), now).await.err().unwrap();
            assert_eq!(e.retry_after_secs(), Some(15 * 60));
        })
        .await;
    }
}
