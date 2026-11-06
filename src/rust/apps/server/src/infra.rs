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
