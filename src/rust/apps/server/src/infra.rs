//! Production adapters for the small infrastructure ports.

use std::sync::Mutex;

use app::ports::{Clock, IdGen};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use domain::hlc::Hlc;

/// Wall clock plus this node's hybrid logical clock (D-12).
pub struct SystemClock {
    hlc: Mutex<Hlc>,
}

impl SystemClock {
    /// `node` distinguishes writers with equal timestamps (random per process).
    #[must_use]
    pub fn new(node: u32) -> Self {
        Self {
            hlc: Mutex::new(Hlc::new(0, 0, node)),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        let mut b = [0u8; 4];
        getrandom::fill(&mut b).expect("os randomness");
        Self::new(u32::from_le_bytes(b))
    }
}

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn hlc(&self) -> Hlc {
        let now_ms = u64::try_from(Utc::now().timestamp_millis()).unwrap_or(0);
        let mut h = self.hlc.lock().expect("hlc");
        *h = h.tick(now_ms);
        *h
    }

    fn observe(&self, remote: Hlc) {
        let now_ms = u64::try_from(Utc::now().timestamp_millis()).unwrap_or(0);
        let mut h = self.hlc.lock().expect("hlc");
        *h = h.receive(remote, now_ms);
    }
}

/// `UUIDv7` ids, 256-bit random tokens, blake3 token hashes.
#[derive(Default)]
pub struct SystemIds;

impl IdGen for SystemIds {
    fn new_id(&self) -> uuid::Uuid {
        uuid::Uuid::now_v7()
    }

    fn new_token(&self) -> String {
        let mut b = [0u8; 32];
        getrandom::fill(&mut b).expect("os randomness");
        URL_SAFE_NO_PAD.encode(b)
    }

    fn hash_token(&self, token: &str) -> String {
        blake3::hash(token.as_bytes()).to_hex().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hlc_is_monotonic() {
        let c = SystemClock::new(7);
        let a = c.hlc();
        let b = c.hlc();
        assert!(b > a);
    }

    #[test]
    fn tokens_are_random_and_hashed() {
        let ids = SystemIds;
        let (a, b) = (ids.new_token(), ids.new_token());
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
        assert_eq!(ids.hash_token(&a), ids.hash_token(&a));
        assert_ne!(ids.hash_token(&a), a);
    }
}

/// What an operator should hear at start-up about how the instance is reachable (SEC-6,
/// OPS-2); empty when nothing is odd. `public_url` and `bind` are the `server` settings,
/// `secure_cookies` the effective cookie setting.
///
/// - Plain HTTP at the public URL while the server listens beyond loopback: sign-in data
///   travels unencrypted unless a TLS proxy is in front.
/// - Secure cookies with a plain-HTTP public URL that is not this machine: browsers drop
///   the session cookie, nobody can sign in.
#[must_use]
pub fn exposure_warnings(public_url: &str, bind: &str, secure_cookies: bool) -> Vec<&'static str> {
    let Ok(url) = url::Url::parse(public_url) else {
        return Vec::new();
    };
    if url.scheme() != "http" {
        return Vec::new();
    }
    let local = |host: &str| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    };
    let mut out = Vec::new();
    let listens_beyond = bind
        .parse::<std::net::SocketAddr>()
        .is_ok_and(|a| !a.ip().is_loopback());
    if listens_beyond {
        out.push(
            "server.public_url is plain HTTP and the server listens beyond loopback: put a \
             reverse proxy with TLS in front and set APP__SERVER__PUBLIC_URL to its https address",
        );
    }
    if secure_cookies && !url.host_str().is_some_and(local) {
        out.push(
            "auth.secure_cookies is on but server.public_url is plain HTTP: browsers will not \
             send the session cookie and nobody can sign in; use https, or set \
             APP__AUTH__SECURE_COOKIES=false for a test on a trusted network",
        );
    }
    out
}

#[cfg(test)]
mod exposure_tests {
    use super::exposure_warnings;

    #[test]
    fn https_or_loopback_is_quiet() {
        assert_eq!(
            exposure_warnings("https://tracker.example.org", "0.0.0.0:3000", true).len(),
            0
        );
        assert_eq!(
            exposure_warnings("http://localhost:3000", "127.0.0.1:3000", true).len(),
            0
        );
        assert_eq!(
            exposure_warnings("http://localhost:3000", "[::1]:3000", false).len(),
            0
        );
    }

    #[test]
    fn plain_http_beyond_loopback_is_named() {
        // the compose default before PUBLIC_URL is set: only the first warning (the cookie
        // works on localhost)
        let w = exposure_warnings("http://localhost:3000", "0.0.0.0:3000", true);
        assert_eq!(w.len(), 1, "{w:?}");
        assert!(w[0].contains("plain HTTP"), "{w:?}");
        // a LAN test over http with secure cookies: nobody can sign in
        let w = exposure_warnings("http://192.168.1.20:3000", "0.0.0.0:3000", true);
        assert_eq!(w.len(), 2, "{w:?}");
        assert!(w[1].contains("secure_cookies"), "{w:?}");
        let w = exposure_warnings("http://192.168.1.20:3000", "0.0.0.0:3000", false);
        assert_eq!(w.len(), 1, "{w:?}");
        // an unparsable setting is not this function's business
        assert_eq!(exposure_warnings("nonsense", "nonsense", true).len(), 0);
    }
}
