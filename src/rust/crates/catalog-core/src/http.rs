//! The governed HTTP client: the only production `HttpFetch` adapter (D-6.3).
//!
//! Per request: connector known? -> host allowlisted? -> fresh cache hit? -> singleflight ->
//! circuit breaker -> priority gate (concurrency, lane order) -> `Retry-After` pause -> token
//! bucket -> send (conditional if a stale cached copy has validators) -> retry 429/5xx/network
//! with `Retry-After` or jittered backoff -> store in cache. Every call runs in a tracing span
//! with connector, op, cache status and HTTP status.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use app::ports::{HttpError, HttpFetch, HttpRequest, HttpResponse};
use async_trait::async_trait;
use chrono::Utc;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use tracing::Instrument;

use crate::breaker::Breaker;
use crate::cache::{CacheEntry, HttpCache, cache_key, ttl_secs};
use crate::contact::{ContactUse, OperatorContact};
use crate::gate::Gate;
use crate::model::{ConnectorInfo, RateLimit};
use crate::singleflight::SingleFlight;
use crate::stats::SourceStats;

/// Limits and allowlist for one connector.
#[derive(Debug, Clone)]
pub struct ConnectorPolicy {
    /// `host` or `host:port`.
    pub hosts: Vec<String>,
    pub rate_limit: RateLimit,
    pub cache_ttl_secs: u64,
    pub breaker_threshold: u32,
    pub breaker_cooldown: Duration,
    /// Longest `Retry-After` we wait for inside one request; longer pauses fail fast.
    pub max_retry_wait: Duration,
    pub max_retries: u32,
    /// Does the source get the operator contact in the `User-Agent`?
    pub contact: ContactUse,
    /// The source limits per address, not per key (it has none): every process of the
    /// instance takes its turn from one [`SharedPace`], so the web role, a separate worker
    /// and the shell commands together stay inside `rate_limit.per_second`.
    pub shared_pace: bool,
}

/// One pace per source for every process of an instance (review BOOK-8): the token bucket of
/// a [`GovernedHttp`] lives in its process, and a source without a key counts the requests
/// of an address, whoever sent them.
#[async_trait]
pub trait SharedPace: Send + Sync {
    /// Returns when the caller may send one request to `source`: the turns of all callers
    /// are at least `every` apart.
    async fn turn(&self, source: &str, every: Duration);
}

impl ConnectorPolicy {
    /// Policy for OIDC discovery, token and JWKS requests to the issuer.
    #[must_use]
    pub fn oidc(host: &str, port: Option<u16>) -> Self {
        let host = match port {
            Some(p) => format!("{host}:{p}"),
            None => host.to_string(),
        };
        Self {
            hosts: vec![host],
            rate_limit: RateLimit {
                per_second: 5,
                burst: 10,
                max_concurrency: 4,
            },
            // discovery documents and keys are cached by the client for its lifetime
            cache_ttl_secs: 0,
            breaker_threshold: 5,
            breaker_cooldown: Duration::from_secs(30),
            max_retry_wait: Duration::from_secs(5),
            max_retries: 1,
            contact: ContactUse::None,
            shared_pace: false,
        }
    }

    #[must_use]
    pub fn from_info(info: &ConnectorInfo) -> Self {
        Self {
            hosts: info.hosts.clone(),
            rate_limit: info.rate_limit.clone(),
            cache_ttl_secs: info.cache_ttl_secs,
            breaker_threshold: 5,
            breaker_cooldown: Duration::from_secs(30),
            max_retry_wait: Duration::from_secs(10),
            max_retries: 2,
            contact: info.contact,
            shared_pace: matches!(info.auth, crate::model::Auth::None),
        }
    }
}

/// The allowed hosts of one connector, replaceable at run time ([`GovernedHttp::host_list`]):
/// OIDC providers are added and changed in the admin area (phase 3 WP3-2).
#[derive(Debug, Default)]
pub struct HostList(std::sync::RwLock<Vec<String>>);

impl HostList {
    /// Allow exactly `hosts` (`host` or `host:port`) from the next request on.
    pub fn set(&self, hosts: Vec<String>) {
        *self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = hosts;
    }

    #[must_use]
    pub fn get(&self) -> Vec<String> {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

struct Governed {
    id: String,
    policy: ConnectorPolicy,
    hosts: Arc<HostList>,
    limiter: DefaultDirectRateLimiter,
    gate: std::sync::Arc<Gate>,
    breaker: Breaker,
    paused_until: Mutex<Option<Instant>>,
}

impl Governed {
    fn new(id: &str, policy: ConnectorPolicy) -> Self {
        let per_second =
            NonZeroU32::new(policy.rate_limit.per_second.max(1)).unwrap_or(NonZeroU32::MIN);
        let burst = NonZeroU32::new(policy.rate_limit.burst.max(1)).unwrap_or(NonZeroU32::MIN);
        Self {
            id: id.to_string(),
            limiter: RateLimiter::direct(Quota::per_second(per_second).allow_burst(burst)),
            gate: Gate::new(policy.rate_limit.max_concurrency.max(1) as usize),
            breaker: Breaker::new(policy.breaker_threshold, policy.breaker_cooldown),
            paused_until: Mutex::new(None),
            hosts: Arc::new(HostList(std::sync::RwLock::new(policy.hosts.clone()))),
            policy,
        }
    }

    fn allows_host(&self, url: &url::Url) -> bool {
        let Some(host) = url.host_str() else {
            return false;
        };
        let with_port = url.port().map(|p| format!("{host}:{p}"));
        self.hosts
            .get()
            .iter()
            .any(|h| h == host && url.port().is_none() || with_port.as_deref() == Some(h.as_str()))
    }

    fn pause_for(&self, d: Duration) {
        let until = Instant::now() + d;
        let mut p = self.paused_until.lock().expect("pause lock");
        if p.is_none_or(|old| old < until) {
            *p = Some(until);
        }
    }

    /// Wait for a `Retry-After` pause to end, or fail if it is longer than we may wait.
    async fn wait_pause(&self) -> Result<(), HttpError> {
        let until = *self.paused_until.lock().expect("pause lock");
        if let Some(until) = until {
            let left = until.saturating_duration_since(Instant::now());
            if left > self.policy.max_retry_wait {
                return Err(HttpError::RateLimited {
                    retry_after_ms: u64::try_from(left.as_millis()).unwrap_or(u64::MAX),
                });
            }
            tokio::time::sleep(left).await;
        }
        Ok(())
    }
}

pub struct GovernedHttp {
    client: reqwest::Client,
    connectors: HashMap<String, Governed>,
    cache: Option<HttpCache>,
    flights: SingleFlight<Result<HttpResponse, HttpError>>,
    upstream_calls: AtomicU64,
    stats: Option<Arc<SourceStats>>,
    /// `instance.hosting`, read per request: `hosted` shares the interactive lane fairly per
    /// user (design.md 4.1).
    hosting: Option<Arc<std::sync::RwLock<String>>>,
    /// The `<ident>/<version>` product and the operator contact, for the connectors whose
    /// policy asks for it.
    contact: Option<(String, OperatorContact)>,
    /// The pace the processes of the instance share, for the connectors whose policy asks
    /// for it.
    pace: Option<Arc<dyn SharedPace>>,
}

enum Step {
    Done(Result<HttpResponse, HttpError>),
    RetryAfter(Duration),
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// `Retry-After` as delta-seconds or HTTP-date.
fn retry_after(headers: &[(String, String)]) -> Option<Duration> {
    let v = header(headers, "retry-after")?.trim();
    if let Ok(secs) = v.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let at = httpdate::parse_http_date(v).ok()?;
    Some(
        at.duration_since(std::time::SystemTime::now())
            .unwrap_or_default(),
    )
}

fn jittered(attempt: u32) -> Duration {
    let base = 200u64 << attempt.min(6);
    Duration::from_millis(base / 2 + fastrand::u64(0..=base / 2))
}

impl GovernedHttp {
    /// # Panics
    /// If the TLS backend cannot be initialised.
    #[must_use]
    pub fn new(user_agent: &str, cache: Option<HttpCache>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client");
        Self {
            client,
            connectors: HashMap::new(),
            cache,
            flights: SingleFlight::default(),
            upstream_calls: AtomicU64::new(0),
            stats: None,
            hosting: None,
            contact: None,
            pace: None,
        }
    }

    /// Take a turn from `pace` before every request to a connector whose policy says
    /// [`ConnectorPolicy::shared_pace`] (after this client's own token bucket).
    #[must_use]
    pub fn with_shared_pace(mut self, pace: Arc<dyn SharedPace>) -> Self {
        self.pace = Some(pace);
        self
    }

    pub fn add_connector(&mut self, id: &str, policy: ConnectorPolicy) -> &mut Self {
        self.connectors
            .insert(id.to_string(), Governed::new(id, policy));
        self
    }

    /// The run-time allowlist of connector `id` (`None` for an unknown connector).
    #[must_use]
    pub fn host_list(&self, id: &str) -> Option<Arc<HostList>> {
        self.connectors.get(id).map(|g| g.hosts.clone())
    }

    /// Record calls, answers and circuit changes per connector (source status, design.md 4.4).
    #[must_use]
    pub fn with_stats(mut self, stats: Arc<SourceStats>) -> Self {
        self.stats = Some(stats);
        self
    }

    /// Read the hosting mode (`home` or `hosted`) live from `hosting` (the admin API's copy
    /// of `instance.hosting`).
    #[must_use]
    pub fn with_hosting(mut self, hosting: Arc<std::sync::RwLock<String>>) -> Self {
        self.hosting = Some(hosting);
        self
    }

    /// Send `<product> (<contact>)` as the `User-Agent` of the connectors whose policy asks
    /// for the operator contact (`product` is `<ident>/<version>`); every other connector
    /// keeps the client's own header and never carries the contact.
    #[must_use]
    pub fn with_contact(mut self, product: &str, contact: OperatorContact) -> Self {
        self.contact = Some((product.to_string(), contact));
        self
    }

    /// The operator contact, if one is set.
    fn contact_value(&self) -> Option<String> {
        self.contact.as_ref().and_then(|(_, c)| c.get())
    }

    /// The `User-Agent` of a connector that gets the contact: `<product> (<contact>)`.
    fn contact_agent(&self, g: &Governed) -> Option<String> {
        if g.policy.contact == ContactUse::None {
            return None;
        }
        let (product, contact) = self.contact.as_ref()?;
        Some(format!("{product} ({})", contact.get()?))
    }

    /// Hosted mode: interactive waiters are served round-robin per user.
    fn fair_share(&self) -> bool {
        self.hosting.as_ref().is_some_and(|h| {
            h.read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_str()
                == "hosted"
        })
    }

    fn failed(&self, g: &Governed) {
        if let (Some(cooldown), Some(s)) = (g.breaker.failure(), &self.stats) {
            s.record_circuit_open(&g.id, cooldown);
        }
    }

    fn note(&self, f: impl FnOnce(&SourceStats)) {
        if let Some(s) = &self.stats {
            f(s);
        }
    }

    /// Requests actually sent upstream (metrics, tests).
    #[must_use]
    pub fn upstream_calls(&self) -> u64 {
        self.upstream_calls.load(Ordering::Relaxed)
    }

    /// Is the circuit of `connector` open (admin notification, D-17)?
    #[must_use]
    pub fn circuit_open(&self, connector: &str) -> bool {
        self.connectors
            .get(connector)
            .is_some_and(|g| g.breaker.is_open())
    }

    async fn upstream(
        &self,
        g: &Governed,
        req: &HttpRequest,
        key: &str,
        cached: Option<(CacheEntry, Vec<u8>)>,
    ) -> Result<HttpResponse, HttpError> {
        let span = tracing::Span::current();
        let mut attempt = 0u32;
        loop {
            match self.attempt(g, req, key, cached.as_ref(), attempt).await {
                Step::Done(r) => {
                    if let Ok(resp) = &r {
                        span.record("status", resp.status);
                    }
                    return r;
                }
                Step::RetryAfter(d) => {
                    attempt += 1;
                    tokio::time::sleep(d).await;
                }
            }
        }
    }

    async fn attempt(
        &self,
        g: &Governed,
        req: &HttpRequest,
        key: &str,
        cached: Option<&(CacheEntry, Vec<u8>)>,
        attempt: u32,
    ) -> Step {
        let retries_left = attempt < g.policy.max_retries;
        if !g.breaker.allow() {
            return Step::Done(Err(HttpError::CircuitOpen));
        }
        let permit = g
            .gate
            .acquire_for(req.lane, req.user, self.fair_share())
            .await;
        if let Err(e) = g.wait_pause().await {
            return Step::Done(Err(e));
        }
        g.limiter.until_ready().await;
        if let Some(pace) = self.pace.as_ref().filter(|_| g.policy.shared_pace) {
            let per_second = g.policy.rate_limit.per_second.max(1);
            pace.turn(&g.id, Duration::from_secs(1) / per_second).await;
        }
        self.upstream_calls.fetch_add(1, Ordering::Relaxed);
        self.note(|s| s.record_call(&g.id));

        let method =
            reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut rb = self.client.request(method, &req.url);
        for (k, v) in &req.headers {
            rb = rb.header(k, v);
        }
        if let Some(agent) = self.contact_agent(g) {
            rb = rb.header(reqwest::header::USER_AGENT, agent);
        }
        if let Some((entry, _)) = cached {
            if let Some(etag) = &entry.etag {
                rb = rb.header("If-None-Match", etag);
            }
            if let Some(lm) = &entry.last_modified {
                rb = rb.header("If-Modified-Since", lm);
            }
        }
        if let Some(body) = &req.body {
            rb = rb.body(body.clone());
        }
        let sent = rb.send().await;
        let resp = match sent {
            Ok(r) => r,
            Err(e) => {
                drop(permit);
                self.note(|s| s.record_error(&g.id, "network"));
                self.failed(g);
                return if retries_left {
                    Step::RetryAfter(jittered(attempt))
                } else {
                    Step::Done(Err(HttpError::Network(e.without_url().to_string())))
                };
            }
        };
        let status = resp.status().as_u16();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let body = match resp.bytes().await {
            Ok(b) => b.to_vec(),
            Err(e) => {
                drop(permit);
                self.note(|s| s.record_error(&g.id, "network"));
                self.failed(g);
                return if retries_left {
                    Step::RetryAfter(jittered(attempt))
                } else {
                    Step::Done(Err(HttpError::Network(e.without_url().to_string())))
                };
            }
        };
        drop(permit);
        self.classify(g, req, key, cached, attempt, status, headers, body)
            .await
    }

    /// Decide what a received response means: done, retry after a pause, cache update.
    // the parts of one response attempt, passed on by `send` unchanged; a struct would only
    // repackage them for this one call (UX4-19)
    #[allow(clippy::too_many_arguments)]
    async fn classify(
        &self,
        g: &Governed,
        req: &HttpRequest,
        key: &str,
        cached: Option<&(CacheEntry, Vec<u8>)>,
        attempt: u32,
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Step {
        let retries_left = attempt < g.policy.max_retries;
        // a rejected member key or candidate key says nothing about the instance key
        if !(req.skip_auth_record && matches!(status, 401 | 403)) {
            self.note(|s| s.record_status(&g.id, status));
        }

        if status == 304
            && let Some((entry, cached_body)) = cached
        {
            g.breaker.success();
            tracing::Span::current().record("cache", "revalidated");
            let mut entry = entry.clone();
            let ttl = ttl_secs(&headers, g.policy.cache_ttl_secs);
            entry.fresh_until =
                Utc::now() + chrono::Duration::seconds(i64::try_from(ttl).unwrap_or(0));
            if let Some(cache) = &self.cache
                && let Err(e) = cache.index.put(key, entry.clone()).await
            {
                tracing::warn!(error = %e, "http cache update failed");
            }
            return Step::Done(Ok(HttpResponse {
                status: entry.status,
                headers: entry.headers,
                body: cached_body.clone(),
            }));
        }
        if status == 429 || status == 503 {
            if status == 503 {
                self.failed(g);
            }
            let wait = retry_after(&headers).unwrap_or_else(|| jittered(attempt));
            g.pause_for(wait);
            if retries_left && wait <= g.policy.max_retry_wait {
                return Step::RetryAfter(wait);
            }
            return Step::Done(Err(HttpError::RateLimited {
                retry_after_ms: u64::try_from(wait.as_millis()).unwrap_or(u64::MAX),
            }));
        }
        if status >= 500 {
            self.failed(g);
            if retries_left {
                return Step::RetryAfter(jittered(attempt));
            }
            return Step::Done(Ok(HttpResponse {
                status,
                headers,
                body,
            }));
        }
        g.breaker.success();
        let cacheable = req.method == "GET" && (200..300).contains(&status);
        let ttl = ttl_secs(&headers, g.policy.cache_ttl_secs);
        if cacheable
            && ttl > 0
            && let Some(cache) = &self.cache
        {
            let now = Utc::now();
            let entry = CacheEntry {
                body_hash: String::new(),
                status,
                headers: headers.clone(),
                etag: header(&headers, "etag").map(ToString::to_string),
                last_modified: header(&headers, "last-modified").map(ToString::to_string),
                fetched_at: now,
                fresh_until: now + chrono::Duration::seconds(i64::try_from(ttl).unwrap_or(0)),
                size: 0,
            };
            // A refused write (storage budget) only means the next request goes upstream.
            if let Err(e) = cache.store(key, entry, &body).await {
                tracing::warn!(error = %e, "http cache store failed");
            }
        }
        Step::Done(Ok(HttpResponse {
            status,
            headers,
            body,
        }))
    }
}

#[async_trait]
impl HttpFetch for GovernedHttp {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let span = tracing::info_span!(
            "http.request",
            connector = %req.connector,
            op = %req.op,
            cache = tracing::field::Empty,
            status = tracing::field::Empty,
        );
        async {
            let g = self.connectors.get(&req.connector).ok_or_else(|| {
                HttpError::Blocked(format!("unknown connector `{}`", req.connector))
            })?;
            // a source that requires the operator contact is not asked without one
            if g.policy.contact == ContactUse::Required && self.contact_value().is_none() {
                return Err(HttpError::Blocked(format!(
                    "connector `{}` needs the operator contact and none is set",
                    req.connector
                )));
            }
            let url = url::Url::parse(&req.url)
                .map_err(|e| HttpError::Blocked(format!("bad url: {e}")))?;
            if !g.allows_host(&url) {
                // host and path only: query strings may carry credentials
                tracing::warn!(
                    host = url.host_str().unwrap_or_default(),
                    path = url.path(),
                    "blocked request to a host outside the allowlist"
                );
                return Err(HttpError::Blocked(format!(
                    "host {} is not allowed for connector `{}`",
                    url.host_str().unwrap_or_default(),
                    req.connector
                )));
            }
            let cacheable = req.method == "GET" && req.body.is_none();
            let key = cache_key(&req.method, &req.url);
            let cached = match (&self.cache, cacheable) {
                (Some(cache), true) => cache.lookup(&key).await.unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "http cache lookup failed");
                    None
                }),
                _ => None,
            };
            let span = tracing::Span::current();
            if let Some((entry, body)) = &cached
                && entry.is_fresh(Utc::now())
            {
                span.record("cache", "hit");
                span.record("status", entry.status);
                return Ok(HttpResponse {
                    status: entry.status,
                    headers: entry.headers.clone(),
                    body: body.clone(),
                });
            }
            span.record("cache", if cached.is_some() { "stale" } else { "miss" });
            if !cacheable {
                return self.upstream(g, &req, &key, None).await;
            }
            let flight = format!("{}|{key}", req.connector);
            self.flights
                .run(&flight, || self.upstream(g, &req, &key, cached))
                .await
        }
        .instrument(span)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_seconds_and_date() {
        let h = |v: &str| vec![("Retry-After".to_string(), v.to_string())];
        assert_eq!(retry_after(&h("3")), Some(Duration::from_secs(3)));
        let future =
            httpdate::fmt_http_date(std::time::SystemTime::now() + Duration::from_secs(120));
        let d = retry_after(&h(&future)).unwrap();
        assert!(d > Duration::from_secs(100) && d <= Duration::from_secs(120));
        assert_eq!(retry_after(&h("soon")), None);
    }
}
