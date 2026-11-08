//! Client address and request id of every request (DEC-284), for audit rows and logs.
//!
//! The client address is the socket peer, or, only when the peer is a trusted proxy
//! (`server.trusted_proxies`, CIDR list, empty by default), the first address in
//! `X-Forwarded-For` read from the right that is not itself a trusted proxy (so a client
//! cannot pick its own address by sending the header; a malformed entry stops the walk at the
//! last good hop, UX3-9). The request id is a new UUID (version 7), or a
//! trusted proxy's `X-Request-Id` when it is a short token; it is echoed in the answer.
//!
//! The layer keeps both in a task-local for the request, so every audit row written while
//! handling it (REST handlers and server functions alike) carries them without threading
//! them through each call.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;

/// Header with the request id.
pub const REQUEST_ID: &str = "x-request-id";
const FORWARDED_FOR: &str = "x-forwarded-for";
const MAX_REQUEST_ID: usize = 128;

/// Networks whose `X-Forwarded-For` and `X-Request-Id` are believed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies(Vec<(IpAddr, u8)>);

fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

impl TrustedProxies {
    /// Parse `10.0.0.0/8`, `192.0.2.1` (a single host), `fd00::/8`, ...
    ///
    /// # Errors
    /// The first entry that is no address or network.
    pub fn parse(list: &[String]) -> Result<Self, String> {
        let mut out = Vec::with_capacity(list.len());
        for raw in list {
            let entry = raw.trim();
            let (addr, bits) = match entry.split_once('/') {
                Some((a, b)) => (a, Some(b)),
                None => (entry, None),
            };
            let ip: IpAddr = addr
                .parse()
                .map_err(|_| format!("server.trusted_proxies: `{entry}` is no IP address"))?;
            let bad = || format!("server.trusted_proxies: bad prefix in `{entry}`");
            let parsed = bits
                .map(|b| b.parse::<u8>().map_err(|_| bad()))
                .transpose()?;
            // an IPv4-mapped IPv6 network folds to IPv4 with its prefix less 96 (UX3-20)
            let mapped = matches!(ip, IpAddr::V6(v6) if v6.to_ipv4_mapped().is_some());
            let ip = canonical(ip);
            let max = if ip.is_ipv4() { 32 } else { 128 };
            let bits = match parsed {
                Some(b) if mapped => b.checked_sub(96).filter(|b| *b <= 32).ok_or_else(bad)?,
                Some(b) => Some(b).filter(|b| *b <= max).ok_or_else(bad)?,
                None => max,
            };
            out.push((ip, bits));
        }
        Ok(Self(out))
    }

    #[must_use]
    pub fn contains(&self, ip: IpAddr) -> bool {
        let ip = canonical(ip);
        self.0.iter().any(|(net, bits)| match (net, ip) {
            (IpAddr::V4(n), IpAddr::V4(a)) => {
                let mask = u32::MAX.checked_shl(32 - u32::from(*bits)).unwrap_or(0);
                u32::from(*n) & mask == u32::from(a) & mask
            }
            (IpAddr::V6(n), IpAddr::V6(a)) => {
                let mask = u128::MAX.checked_shl(128 - u32::from(*bits)).unwrap_or(0);
                u128::from(*n) & mask == u128::from(a) & mask
            }
            _ => false,
        })
    }
}

/// What is known about the request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestMeta {
    pub client_addr: Option<String>,
    pub request_id: String,
}

tokio::task_local! {
    static META: RequestMeta;
}

/// The current request's meta, if called while handling one.
#[must_use]
pub fn current() -> Option<RequestMeta> {
    META.try_with(Clone::clone).ok()
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_REQUEST_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
}

/// Walk `X-Forwarded-For` from the right, starting at the trusted peer: each trusted hop is
/// skipped, the first untrusted entry is the client. A malformed entry stops the walk at
/// the last good hop (UX3-9): proxies append, so everything left of it came from the
/// client and cannot be believed. When every entry is trusted the leftmost one is reached.
fn forwarded_client(
    headers: &axum::http::HeaderMap,
    trusted: &TrustedProxies,
    peer: IpAddr,
) -> IpAddr {
    let entries: Vec<&str> = headers
        .get_all(FORWARDED_FOR)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .collect();
    let mut last_good = peer;
    for s in entries.into_iter().rev() {
        let Some(ip) = s
            .parse::<IpAddr>()
            .ok()
            .or_else(|| s.parse::<SocketAddr>().ok().map(|a| a.ip()))
            .map(canonical)
        else {
            break;
        };
        last_good = ip;
        if !trusted.contains(ip) {
            break;
        }
    }
    last_good
}

/// The meta of `req` as received from its peer.
#[must_use]
pub fn of(req: &Request, trusted: &TrustedProxies) -> RequestMeta {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| canonical(c.0.ip()));
    let from_proxy = peer.is_some_and(|p| trusted.contains(p));
    let client = if from_proxy {
        peer.map(|p| forwarded_client(req.headers(), trusted, p))
    } else {
        peer
    };
    let request_id = from_proxy
        .then(|| req.headers().get(REQUEST_ID))
        .flatten()
        .and_then(|v| v.to_str().ok())
        .filter(|v| valid_id(v))
        .map_or_else(|| uuid::Uuid::now_v7().to_string(), String::from);
    RequestMeta {
        client_addr: client.map(|c| c.to_string()),
        request_id,
    }
}

/// Middleware: work out the meta, keep it for the request, echo the request id.
pub async fn layer(
    State(trusted): State<Arc<TrustedProxies>>,
    req: Request,
    next: Next,
) -> Response {
    let meta = of(&req, &trusted);
    let id = meta.request_id.clone();
    let mut resp = META.scope(meta, next.run(req)).await;
    if let Ok(v) = HeaderValue::from_str(&id) {
        resp.headers_mut().insert(REQUEST_ID, v);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(list: &[&str]) -> TrustedProxies {
        TrustedProxies::parse(&list.iter().map(|s| (*s).to_string()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn networks_match_by_prefix() {
        let t = nets(&["10.0.0.0/8", "192.0.2.1", "fd00::/8"]);
        assert!(t.contains("10.200.1.1".parse().unwrap()));
        assert!(t.contains("::ffff:10.0.0.1".parse().unwrap()), "mapped v4");
        assert!(t.contains("192.0.2.1".parse().unwrap()));
        assert!(!t.contains("192.0.2.2".parse().unwrap()));
        assert!(t.contains("fd12::1".parse().unwrap()));
        assert!(!t.contains("fe80::1".parse().unwrap()));
        assert!(nets(&["0.0.0.0/0"]).contains("203.0.113.1".parse().unwrap()));
        assert!(TrustedProxies::parse(&["10.0.0.0/33".into()]).is_err());
        assert!(TrustedProxies::parse(&["proxy.local".into()]).is_err());
        // UX3-20: a mapped IPv6 network folds to IPv4 with its prefix less 96, as config accepts
        let mapped = nets(&["::ffff:10.0.0.0/104"]);
        assert!(mapped.contains("10.1.2.3".parse().unwrap()));
        assert!(!mapped.contains("11.0.0.1".parse().unwrap()));
        assert!(TrustedProxies::parse(&["::ffff:10.0.0.0/90".into()]).is_err());
    }

    fn client(xff: Option<&str>) -> Option<String> {
        let mut b = axum::http::Request::builder().uri("/");
        if let Some(v) = xff {
            b = b.header(FORWARDED_FOR, v);
        }
        let mut req = b.body(axum::body::Body::empty()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 443))));
        of(&req, &nets(&["10.0.0.0/8"])).client_addr
    }

    /// UX3-9: the chain is walked from the right; trusted hops are skipped, the first
    /// untrusted entry is the client, a malformed entry stops the walk at the last good hop.
    #[test]
    fn forwarded_chains_are_walked_from_the_right() {
        let c = |v: &str| client(Some(v));
        assert_eq!(client(None).as_deref(), Some("10.0.0.1"), "no header: peer");
        assert_eq!(c("203.0.113.7").as_deref(), Some("203.0.113.7"));
        assert_eq!(
            c("198.51.100.1, 203.0.113.7, 10.0.0.2").as_deref(),
            Some("203.0.113.7"),
            "a client-sent entry left of the real one is ignored"
        );
        assert_eq!(
            c("x, 203.0.113.7").as_deref(),
            Some("203.0.113.7"),
            "a malformed client-sent entry cannot hide the client"
        );
        assert_eq!(
            c("203.0.113.7, not-an-ip, 10.0.0.2").as_deref(),
            Some("10.0.0.2"),
            "malformed: the last good hop, not the peer"
        );
        assert_eq!(
            c("x").as_deref(),
            Some("10.0.0.1"),
            "nothing good in the chain: the peer"
        );
        assert_eq!(
            c("10.0.0.5, 10.0.0.2").as_deref(),
            Some("10.0.0.5"),
            "all trusted: the leftmost hop reached"
        );
        assert_eq!(c("203.0.113.7:4711").as_deref(), Some("203.0.113.7"));
        assert_eq!(c("[2001:db8::1]:80").as_deref(), Some("2001:db8::1"));
        let two = {
            let mut req = axum::http::Request::builder()
                .uri("/")
                .header(FORWARDED_FOR, "203.0.113.7")
                .header(FORWARDED_FOR, "10.0.0.3")
                .body(axum::body::Body::empty())
                .unwrap();
            req.extensions_mut()
                .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 443))));
            of(&req, &nets(&["10.0.0.0/8"])).client_addr
        };
        assert_eq!(
            two.as_deref(),
            Some("203.0.113.7"),
            "repeated headers in order"
        );
    }

    #[test]
    fn request_ids_are_short_tokens() {
        assert!(valid_id("req-abc_123"));
        assert!(valid_id("0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"));
        assert!(!valid_id(""));
        assert!(!valid_id("a b"));
        assert!(!valid_id(&"x".repeat(129)));
    }
}
