//! The signed pending-login cookie (addendum-1 4.2 "Redirect and callback").
//!
//! The state between the redirect to the provider and the callback travels in the browser,
//! HMAC-SHA256 signed with a key derived by HKDF from the secrets key ring (purpose
//! `cookie_key`, info [`COOKIE_KEY_INFO`]): a party that can set cookies for the host cannot
//! fix `state` (RFC 6749 10.12) or change the provider, mode or return path (RFC 9700 4.7.1:
//! state that carries data "MUST" be protected against tampering). Each value expires after
//! [`PENDING_MINUTES`]; single use is enforced by the caller against a server-side list of
//! used values ([`super::ProviderStore::claim_once`]).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{KeyInit, Mac};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

/// HKDF `info` of the cookie key (addendum-1 2.2 table, purpose `cookie_key`).
pub const COOKIE_KEY_INFO: &[u8] = b"oidc-pending-cookie";
/// Lifetime of a pending sign-in (the code itself lives at most 10 minutes, RFC 6749 4.1.2).
pub const PENDING_MINUTES: i64 = 10;
const FORMAT: &str = "v1";

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

/// Where the signing key comes from: the live key ring in production.
pub trait CookieKeys: Send + Sync {
    /// The keys that verify, the signing key first.
    fn keys(&self) -> Vec<Zeroizing<[u8; 32]>>;
}

impl CookieKeys for crate::secrets::LiveKeys {
    fn keys(&self) -> Vec<Zeroizing<[u8; 32]>> {
        // every loaded KEK verifies, so a SIGHUP rotation mid-sign-in does not break it
        self.ring().derive(COOKIE_KEY_INFO)
    }
}

/// A fixed key (tests, tools).
pub struct StaticCookieKey(pub [u8; 32]);

impl CookieKeys for StaticCookieKey {
    fn keys(&self) -> Vec<Zeroizing<[u8; 32]>> {
        vec![Zeroizing::new(self.0)]
    }
}

/// Why the browser went to the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// A sign-in button (or the default provider).
    Interactive,
    /// `prompt=none` (true SSO, WP3-3).
    Silent,
    /// Settings > Account > Link: attach the identity to the signed-in account.
    Link,
    /// Re-authentication round trip with `max_age=0` (DEC-191, WP3-3).
    Reauth,
    /// Provider Test: one `prompt=none` request by the testing admin (OD-19).
    ProbeSilent,
    /// Provider Test: one `max_age=0` request; is a fresh `auth_time` reported? (od-12 4.4)
    ProbeMaxAge,
}

impl Mode {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "interactive" => Self::Interactive,
            "silent" => Self::Silent,
            "link" => Self::Link,
            "reauth" => Self::Reauth,
            "probe_silent" => Self::ProbeSilent,
            "probe_max_age" => Self::ProbeMaxAge,
            _ => return None,
        })
    }
}

/// The pending sign-in kept in the browser between redirect and callback.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingLogin {
    pub provider: Uuid,
    /// The issuer the request went to (mix-up defence, RFC 9700 4.4.2).
    pub issuer: String,
    pub state: String,
    pub nonce: String,
    pub verifier: String,
    /// Local path to land on afterwards (always starts with one `/`).
    pub return_to: String,
    pub mode: Mode,
    /// `link` and `reauth`: the signed-in account the flow belongs to.
    pub user: Option<Uuid>,
}

impl std::fmt::Debug for PendingLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingLogin")
            .field("provider", &self.provider)
            .field("issuer", &self.issuer)
            .field("mode", &self.mode)
            .field("return_to", &self.return_to)
            .finish_non_exhaustive()
    }
}

/// The `oidc_hint` cookie (addendum-1 4.2 "True SSO" step 3): the provider that signed this
/// browser in last and a `login_hint` for its next silent request. It carries no token; the
/// ID token for `id_token_hint` stays on the server, per session (DEC-317).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hint {
    pub provider: Uuid,
    /// `preferred_username`, else `email` of the last sign-in (Core 3.1.2.1 `login_hint`).
    pub login_hint: Option<String>,
}

/// "Is this your account?" (addendum-1 4.2 step 3): a verified identity waiting for the
/// password of the local account its email matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingLink {
    /// Single-use id.
    pub id: String,
    pub provider: Uuid,
    pub issuer: String,
    pub subject: String,
    pub user: Uuid,
    pub sid: Option<String>,
    pub groups: Option<Vec<String>>,
    pub claims_hash: String,
    pub return_to: String,
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    /// What the value is (`login`, `link`): a value of one kind never passes as another.
    t: String,
    /// Expiry, unix seconds.
    exp: i64,
    v: T,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CookieError {
    #[error("malformed sign-in state")]
    Malformed,
    #[error("sign-in state signature mismatch")]
    BadSignature,
    #[error("sign-in state expired")]
    Expired,
}

fn mac(key: &[u8; 32], data: &[u8]) -> HmacSha256 {
    // HMAC accepts keys of any length
    let mut m = <HmacSha256 as KeyInit>::new_from_slice(key).expect("hmac key");
    m.update(data);
    m
}

/// `v1.<base64url(json)>.<base64url(hmac)>`.
///
/// # Panics
/// If the key source has no key (the key ring always has one).
#[must_use]
pub fn seal<T: Serialize>(keys: &dyn CookieKeys, kind: &str, value: &T, exp: i64) -> String {
    let json = serde_json::to_vec(&Envelope {
        t: kind.to_string(),
        exp,
        v: value,
    })
    .expect("serialisable");
    let body = format!("{FORMAT}.{}", URL_SAFE_NO_PAD.encode(json));
    let key = keys.keys().into_iter().next().expect("a cookie key");
    let tag = mac(&key, body.as_bytes()).finalize().into_bytes();
    format!("{body}.{}", URL_SAFE_NO_PAD.encode(tag))
}

/// Check the signature (constant time, any loaded key), the kind and the expiry.
///
/// # Errors
/// [`CookieError`].
pub fn open<T: DeserializeOwned>(
    keys: &dyn CookieKeys,
    kind: &str,
    raw: &str,
    now: i64,
) -> Result<T, CookieError> {
    let (body, tag) = raw.rsplit_once('.').ok_or(CookieError::Malformed)?;
    let tag = URL_SAFE_NO_PAD
        .decode(tag)
        .map_err(|_| CookieError::Malformed)?;
    if !keys
        .keys()
        .iter()
        .any(|k| mac(k, body.as_bytes()).verify_slice(&tag).is_ok())
    {
        return Err(CookieError::BadSignature);
    }
    let json = body
        .strip_prefix(FORMAT)
        .and_then(|b| b.strip_prefix('.'))
        .and_then(|b| URL_SAFE_NO_PAD.decode(b).ok())
        .ok_or(CookieError::Malformed)?;
    let env: Envelope<T> = serde_json::from_slice(&json).map_err(|_| CookieError::Malformed)?;
    if env.t != kind {
        return Err(CookieError::Malformed);
    }
    if env.exp <= now {
        return Err(CookieError::Expired);
    }
    Ok(env.v)
}

/// The key of a used value in the single-use list (no secret in the database).
#[must_use]
pub fn used_key(kind: &str, id: &str) -> String {
    blake3::hash(format!("{kind}|{id}").as_bytes())
        .to_hex()
        .to_string()
}

/// A safe local return path: one leading `/`, no scheme-relative `//` or `/\`, no control
/// characters; anything else is `/` (no open redirect).
#[must_use]
pub fn return_path(next: Option<&str>) -> String {
    match next {
        Some(p)
            if p.starts_with('/')
                && !p.starts_with("//")
                && !p.starts_with("/\\")
                && p.len() <= 512
                && !p.chars().any(|c| c.is_control() || c == '\\') =>
        {
            p.to_string()
        }
        _ => "/".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> PendingLogin {
        PendingLogin {
            provider: Uuid::nil(),
            issuer: "https://id.example.test".into(),
            state: "s".into(),
            nonce: "n".into(),
            verifier: "v".into(),
            return_to: "/library".into(),
            mode: Mode::Link,
            user: Some(Uuid::nil()),
        }
    }

    #[test]
    fn round_trip_with_the_key() {
        let k = StaticCookieKey([7; 32]);
        let raw = seal(&k, "login", &pending(), 100);
        assert_eq!(open::<PendingLogin>(&k, "login", &raw, 99), Ok(pending()));
    }

    #[test]
    fn tampering_is_refused() {
        let k = StaticCookieKey([7; 32]);
        let raw = seal(&k, "login", &pending(), 100);
        // a changed body (another return path) with the old tag
        let (body, tag) = raw.rsplit_once('.').unwrap();
        let mut other = pending();
        other.return_to = "/admin".into();
        let forged_body = seal(&k, "login", &other, 100);
        let forged = format!("{}.{tag}", forged_body.rsplit_once('.').unwrap().0);
        assert_eq!(
            open::<PendingLogin>(&k, "login", &forged, 1),
            Err(CookieError::BadSignature)
        );
        // a flipped tag byte
        let mut t = URL_SAFE_NO_PAD.decode(tag).unwrap();
        t[0] ^= 1;
        let flipped = format!("{body}.{}", URL_SAFE_NO_PAD.encode(t));
        assert_eq!(
            open::<PendingLogin>(&k, "login", &flipped, 1),
            Err(CookieError::BadSignature)
        );
        // another key (another instance, or a dropped key version)
        assert_eq!(
            open::<PendingLogin>(&StaticCookieKey([8; 32]), "login", &raw, 1),
            Err(CookieError::BadSignature)
        );
        assert_eq!(
            open::<PendingLogin>(&k, "login", "garbage", 1),
            Err(CookieError::Malformed)
        );
    }

    #[test]
    fn kind_and_expiry_are_checked() {
        let k = StaticCookieKey([7; 32]);
        let raw = seal(&k, "login", &pending(), 100);
        assert_eq!(
            open::<PendingLogin>(&k, "link", &raw, 1),
            Err(CookieError::Malformed)
        );
        assert_eq!(
            open::<PendingLogin>(&k, "login", &raw, 100),
            Err(CookieError::Expired)
        );
    }

    #[test]
    fn every_loaded_key_verifies_the_active_one_signs() {
        let ring = crate::secrets::KeyRing::parse(&format!(
            "v2:{}\nv1:{}",
            base64::engine::general_purpose::STANDARD.encode([2u8; 32]),
            base64::engine::general_purpose::STANDARD.encode([1u8; 32])
        ))
        .unwrap();
        let old = crate::secrets::KeyRing::parse(&format!(
            "v1:{}",
            base64::engine::general_purpose::STANDARD.encode([1u8; 32])
        ))
        .unwrap();
        let live_old = crate::secrets::LiveKeys::new(old);
        let live_new = crate::secrets::LiveKeys::new(ring);
        let raw = seal(&live_old, "login", &pending(), 100);
        // signed under v1, verified after v2 became active
        assert_eq!(
            open::<PendingLogin>(&live_new, "login", &raw, 1),
            Ok(pending())
        );
        let derived = live_new.keys();
        assert_eq!(derived.len(), 2);
        assert_ne!(*derived[0], [2u8; 32], "derived, not the KEK itself");
    }

    #[test]
    fn return_paths_stay_local() {
        assert_eq!(return_path(Some("/library?x=1")), "/library?x=1");
        for bad in [
            "//evil.example",
            "/\\evil",
            "https://evil",
            "library",
            "/a\nb",
        ] {
            assert_eq!(return_path(Some(bad)), "/", "{bad:?}");
        }
        assert_eq!(return_path(None), "/");
    }
}
