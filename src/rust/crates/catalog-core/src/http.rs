//! The governed HTTP client: the only production `HttpFetch` adapter (D-6.3).
//!
//! Per request: connector known? -> host allowlisted? -> fresh cache hit? -> singleflight ->
//! circuit breaker -> priority gate (concurrency, lane order) -> `Retry-After` pause -> token
//! bucket -> send (conditional if a stale cached copy has validators) -> retry 429/5xx/network
//! with `Retry-After` or jittered backoff -> store in cache. Every call runs in a tracing span
//! with connector, op, cache status and HTTP status.
//!
//! Confinement (SEC-4): the client follows no redirect by itself. A redirect of a `GET` or
//! `HEAD` is followed here, at most [`MAX_REDIRECTS`] times, and each hop is checked like the
//! first request: the host must be on the connector's list, and it must not be on a private,
//! loopback or link-local address unless the connector's [`PrivateRule`] or the operator's
//! host list ([`GovernedHttp::with_private_hosts`]) allows that. Names are checked when they
//! are resolved for the connection, so a DNS answer cannot change in between. The caller's
//! headers (API keys) go only to the host the request was made for.

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
    /// `host` or `host:port`, or `*.domain` for every subdomain of `domain` on the default
    /// port (`*.archive.org`: the numbered download hosts a cover redirects to; the domain
    /// itself is an entry of its own). A bare leading dot is no syntax (review SEC2-11).
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
    /// May the hosts of this connector be on private addresses ([`PrivateRule`])?
    pub private: PrivateRule,
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

/// May a connector reach hosts on private, loopback or link-local addresses (SEC-4)? Hosts
/// named with [`GovernedHttp::with_private_hosts`] may, whatever the rule says.
#[derive(Clone, Default)]
pub enum PrivateRule {
    /// No: the documented hosts of a data source are public.
    #[default]
    Blocked,
    /// Yes.
    Allowed,
    /// Ask per request (sign-in providers follow the hosting mode, DEC-534).
    When(Arc<dyn Fn() -> bool + Send + Sync>),
}

impl PrivateRule {
    /// A rule that is asked for every request.
    pub fn when(f: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self::When(Arc::new(f))
    }

    fn allows(&self) -> bool {
        match self {
            Self::Blocked => false,
            Self::Allowed => true,
            Self::When(f) => f(),
        }
    }
}

impl std::fmt::Debug for PrivateRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Blocked => "Blocked",
            Self::Allowed => "Allowed",
            Self::When(_) => "When(..)",
        })
    }
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
            private: PrivateRule::Blocked,
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
            private: PrivateRule::Blocked,
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

/// Redirects followed for one request.
pub const MAX_REDIRECTS: usize = 5;

/// Does the list entry `entry` (`host`, `host:port`, or `*.domain` for every subdomain of
/// `domain`) cover the host of `url`? An entry without a port covers the default port of
/// the scheme only; entries are compared in lower case (review SEC2-11). The same rules as
/// the operator's private hosts ([`domain::net::PrivateHosts`]).
fn host_matches(entry: &str, url: &url::Url) -> bool {
    url.host_str()
        .is_some_and(|host| domain::net::PrivateHosts::new([entry]).covers(host, url.port()))
}

/// Why nothing came back from upstream.
enum Refused {
    /// Confinement said no; nothing to retry and no sign of trouble upstream.
    Blocked(String),
    Network(String),
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
        self.hosts.get().iter().any(|h| host_matches(h, url))
    }

    /// Where a redirect from `from` to `location` leads, if the connector may go there:
    /// http(s) only, never from https down to http, and only to a host on its list.
    fn next_hop(&self, from: &url::Url, location: &str) -> Result<url::Url, Refused> {
        let next = from
            .join(location)
            .map_err(|e| Refused::Blocked(format!("bad redirect target: {e}")))?;
        let downgrade = from.scheme() == "https" && next.scheme() != "https";
        if !matches!(next.scheme(), "http" | "https") || downgrade {
            return Err(Refused::Blocked(format!(
                "redirect to a {} address is not followed",
                next.scheme()
            )));
        }
        if !self.allows_host(&next) {
            // the host only: paths and query strings may carry credentials
            return Err(Refused::Blocked(format!(
                "redirect to host {} is not allowed for connector `{}`",
                next.host_str().unwrap_or_default(),
                self.id
            )));
        }
        Ok(next)
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
    /// Resolves names to public addresses only.
    client: reqwest::Client,
    /// For hosts that may be on a private address.
    lan_client: reqwest::Client,
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
    /// Hosts that may be on a private address for any connector (the operator's list).
    private_hosts: domain::net::PrivateHosts,
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
        let builder = || {
            reqwest::Client::builder()
                .user_agent(user_agent)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                // redirects are followed in `send`, one checked hop at a time
                .redirect(reqwest::redirect::Policy::none())
                // no proxy from the environment: it would resolve the names itself, out of
                // reach of the private-address check (review SEC2-9)
                .no_proxy()
        };
        let client = builder()
            .dns_resolver(Arc::new(crate::resolve::PublicOnly))
            .build()
            .expect("http client");
        let lan_client = builder().build().expect("http client");
        Self {
            client,
            lan_client,
            connectors: HashMap::new(),
            cache,
            flights: SingleFlight::default(),
            upstream_calls: AtomicU64::new(0),
            stats: None,
            hosting: None,
            contact: None,
            pace: None,
            private_hosts: domain::net::PrivateHosts::default(),
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

    /// Hosts (`host` or `host:port`, as a connector's list names them) that may be on a
    /// private, loopback or link-local address: the operator's allowlist.
    #[must_use]
    pub fn with_private_hosts(mut self, hosts: Vec<String>) -> Self {
        self.private_hosts = domain::net::PrivateHosts::new(hosts);
        self
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

    /// The `User-Agent` of a connector that gets the contact: `<product> (<contact>)`. Read
    /// once per request (WD-7): the request is refused or sent with what this one read.
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
        agent: Option<&str>,
    ) -> Result<HttpResponse, HttpError> {
        let span = tracing::Span::current();
        let mut attempt = 0u32;
        loop {
            match self
                .attempt(g, req, key, cached.as_ref(), attempt, agent)
                .await
            {
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
        agent: Option<&str>,
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

        let sent = self.send(g, req, cached, agent).await;
        drop(permit);
        let (status, headers, body) = match sent {
            Ok(received) => received,
            Err(Refused::Blocked(why)) => {
                tracing::warn!(connector = %g.id, %why, "outbound request refused");
                return Step::Done(Err(HttpError::Blocked(why)));
            }
            Err(Refused::Network(e)) => {
                self.note(|s| s.record_error(&g.id, "network"));
                self.failed(g);
                return if retries_left {
                    Step::RetryAfter(jittered(attempt))
                } else {
                    Step::Done(Err(HttpError::Network(e)))
                };
            }
        };
        self.classify(g, req, key, cached, attempt, status, headers, body)
            .await
    }

    /// The client for `url`: the one that may reach private addresses when the connector's
    /// rule or the operator's list allows the host, else the public-only one, which is not
    /// asked at all for an IP literal in a private range.
    fn client_for(&self, g: &Governed, url: &url::Url) -> Result<&reqwest::Client, Refused> {
        let listed = url
            .host_str()
            .is_some_and(|h| self.private_hosts.covers(h, url.port()));
        if g.policy.private.allows() || listed {
            return Ok(&self.lan_client);
        }
        let literal = match url.host() {
            Some(url::Host::Ipv4(a)) => Some(std::net::IpAddr::V4(a)),
            Some(url::Host::Ipv6(a)) => Some(std::net::IpAddr::V6(a)),
            Some(url::Host::Domain(_)) | None => None,
        };
        if literal.is_some_and(domain::net::is_private_ip) {
            return Err(Refused::Blocked(format!(
                "host {} is on a private address",
                url.host_str().unwrap_or_default()
            )));
        }
        Ok(&self.client)
    }

    /// Send `req` and follow the redirects of a `GET` or `HEAD`, each hop checked against
    /// the connector's host list and the private-address rule. Other methods get a redirect
    /// back as the answer. The caller's headers and the cache validators only go to the
    /// origin the request was made for.
    async fn send(
        &self,
        g: &Governed,
        req: &HttpRequest,
        cached: Option<&(CacheEntry, Vec<u8>)>,
        agent: Option<&str>,
    ) -> Result<(u16, Vec<(String, String)>, Vec<u8>), Refused> {
        let method =
            reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let follow = matches!(method, reqwest::Method::GET | reqwest::Method::HEAD);
        let first =
            url::Url::parse(&req.url).map_err(|e| Refused::Blocked(format!("bad url: {e}")))?;
        let mut url = first.clone();
        for hop in 0..=MAX_REDIRECTS {
            let client = self.client_for(g, &url)?;
            let mut rb = client.request(method.clone(), url.clone());
            if url.origin() == first.origin() {
                for (k, v) in &req.headers {
                    rb = rb.header(k, v);
                }
            }
            // the contact names the application, not a secret: it goes along every hop (the
            // cover redirects of Open Library end on Internet Archive hosts, LB-4); it is the
            // value `fetch` read once for this request (WD-7)
            if let Some(agent) = agent {
                rb = rb.header(reqwest::header::USER_AGENT, agent);
            }
            if hop == 0 {
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
            }
            let resp = rb.send().await.map_err(|e| {
                if crate::resolve::refused_as_private(&e) {
                    Refused::Blocked(format!(
                        "host {} resolves to a private address",
                        url.host_str().unwrap_or_default()
                    ))
                } else {
                    Refused::Network(e.without_url().to_string())
                }
            })?;
            let status = resp.status().as_u16();
            let headers: Vec<(String, String)> = resp
                .headers()
                .iter()
                .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
                .collect();
            let target = matches!(status, 301 | 302 | 303 | 307 | 308)
                .then(|| header(&headers, "location"))
                .flatten()
                .filter(|_| follow);
            let Some(location) = target else {
                let body = resp
                    .bytes()
                    .await
                    .map_err(|e| Refused::Network(e.without_url().to_string()))?;
                return Ok((status, headers, body.to_vec()));
            };
            url = g.next_hop(&url, location)?;
        }
        Err(Refused::Blocked(format!(
            "more than {MAX_REDIRECTS} redirects"
        )))
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
            // the contact is read once for this request (WD-7); a source that requires it is
            // not asked without one
            let agent = self.contact_agent(g);
            if g.policy.contact == ContactUse::Required && agent.is_none() {
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
            // an IP literal in a private range is refused here, before the cache and the
            // limits; a name is checked when it is resolved for the connection
            if let Err(Refused::Blocked(why)) = self.client_for(g, &url) {
                tracing::warn!(connector = %req.connector, %why, "outbound request refused");
                return Err(HttpError::Blocked(why));
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
                return self.upstream(g, &req, &key, None, agent.as_deref()).await;
            }
            let flight = format!("{}|{key}", req.connector);
            self.flights
                .run(&flight, || {
                    self.upstream(g, &req, &key, cached, agent.as_deref())
                })
                .await
        }
        .instrument(span)
        .await
    }

    fn pause(&self, connector: &str, wait: Duration) {
        if let Some(g) = self.connectors.get(connector) {
            g.pause_for(wait);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_entries_cover_a_host_a_port_or_a_domain_with_its_subdomains() {
        let m = |entry: &str, url: &str| host_matches(entry, &url::Url::parse(url).unwrap());
        assert!(m("api.example.org", "https://api.example.org/v1"));
        assert!(!m("api.example.org", "https://api.example.org:8443/v1"));
        assert!(m("api.example.org:8443", "https://api.example.org:8443/v1"));
        assert!(!m("api.example.org:8443", "https://api.example.org/v1"));
        assert!(
            !m("example.org", "https://api.example.org/"),
            "no subdomains by itself"
        );
        // a wildcard entry (review SEC2-11): every subdomain, default port only; the domain
        // itself is an entry of its own
        assert!(m(
            "*.archive.org",
            "https://ia800703.us.archive.org/view_archive.php"
        ));
        assert!(!m("*.archive.org", "https://archive.org/download/x"));
        assert!(!m("*.archive.org", "https://notarchive.org/"));
        assert!(!m("*.archive.org", "https://archive.org.example.net/"));
        assert!(!m("*.archive.org", "https://ia800703.us.archive.org:8080/"));
        // a bare leading dot is no syntax: it covers nothing
        assert!(!m(".archive.org", "https://ia800703.us.archive.org/"));
        assert!(!m(".archive.org", "https://archive.org/"));
        assert!(
            m("API.Example.org", "https://api.example.org/v1"),
            "lower case"
        );
        assert!(m("127.0.0.1:3011", "http://127.0.0.1:3011/x"));
        assert!(m("[::1]:3011", "http://[::1]:3011/x"));
    }

    /// The cover redirects of Open Library (docs/connectors/openlibrary.md, 2026-11-10):
    /// `covers.openlibrary.org` -> `archive.org` -> `ia800703.us.archive.org`. Every hop is
    /// checked against the connector's list (the list of `connectors::openlibrary`); a hop
    /// to a host that is not on it is refused.
    #[test]
    fn a_cover_redirect_to_a_numbered_archive_host_is_followed_and_others_are_not() {
        let mut policy = ConnectorPolicy::oidc("", None);
        policy.hosts = [
            "openlibrary.org",
            "covers.openlibrary.org",
            "archive.org",
            "*.archive.org",
        ]
        .map(String::from)
        .to_vec();
        let g = Governed::new("openlibrary", policy);
        let cover =
            url::Url::parse("https://covers.openlibrary.org/b/id/8739161-M.jpg?default=false")
                .unwrap();
        let archive = g
            .next_hop(
                &cover,
                "https://archive.org/download/m_covers_0008/x.zip/8739161-M.jpg",
            )
            .ok()
            .expect("archive.org is listed");
        let ia = g
            .next_hop(
                &archive,
                "https://ia800703.us.archive.org/view_archive.php?archive=/x.zip&file=8739161-M.jpg",
            )
            .ok()
            .expect("*.archive.org covers the numbered hosts");
        assert_eq!(ia.host_str(), Some("ia800703.us.archive.org"));
        for refused in [
            "https://images.example.net/8739161-M.jpg",
            "https://archive.org.example.net/x.jpg",
            "https://notarchive.org/x.jpg",
            "https://ia800703.us.archive.org:8443/x.jpg",
            "http://ia800703.us.archive.org/x.jpg",
        ] {
            assert!(
                matches!(g.next_hop(&archive, refused), Err(Refused::Blocked(_))),
                "{refused}"
            );
        }
    }

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
