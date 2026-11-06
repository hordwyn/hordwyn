//! HTTP channels: ntfy, Gotify, Discord, Matrix, Apprise API and signed webhooks (Standard
//! Webhooks). Protocols and sources: `docs/research/notification-channels.md`.
//!
//! Requests go out through the `HttpFetch` port (D-5.2); the server plugs in a plain client
//! without provider allowlists, because targets are user-supplied URLs. For the same reason
//! addresses in private, loopback and link-local ranges are refused here unless the admin
//! allows them (`allow_private`, for LAN setups).

use std::net::IpAddr;
use std::sync::Arc;

use app::ports::{HttpFetch, HttpRequest, Lane};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use url::Url;

use crate::deliver::{Message, SendError};
use crate::model::ChannelConfig;

/// `connector` of the requests (logs and metrics of the HTTP adapter).
pub const CONNECTOR: &str = "notify";

/// Discord's limit for `content`.
const DISCORD_MAX: usize = 2000;

pub struct HttpChannels {
    http: Arc<dyn HttpFetch>,
    allow_private: bool,
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                // 100.64.0.0/10 (carrier-grade NAT)
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // fc00::/7 unique local, fe80::/10 link-local
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().is_some_and(|v4| is_private(IpAddr::V4(v4)))
        }
    }
}

fn status_error(status: u16, what: &str) -> SendError {
    let m = format!("{what} answered HTTP {status}");
    match status {
        410 => SendError::Gone(m),
        429 | 500..=599 => SendError::Retry(m),
        _ => SendError::Permanent(m),
    }
}

/// Standard Webhooks `v1` signature of `id.timestamp.body`. A `whsec_` secret is base64 of
/// the key; any other secret is used as is.
///
/// # Panics
/// Never: HMAC accepts keys of any length.
#[must_use]
pub fn sign(secret: &str, id: &str, timestamp: i64, body: &[u8]) -> String {
    let key = secret
        .strip_prefix("whsec_")
        .and_then(|s| STANDARD.decode(s).ok())
        .unwrap_or_else(|| secret.as_bytes().to_vec());
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).expect("any key length");
    mac.update(id.as_bytes());
    mac.update(b".");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    format!("v1,{}", STANDARD.encode(mac.finalize().into_bytes()))
}

fn text(m: &Message) -> String {
    let mut s = m.body.clone();
    if let Some(l) = &m.link {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(l);
    }
    s
}

/// Parse a user-supplied target URL; refuse non-HTTP schemes and, unless `allow_private`,
/// hosts that are or resolve to private, loopback or link-local addresses. (A DNS answer
/// that changes between this check and the request is not caught.)
///
/// # Errors
/// [`SendError::Permanent`] for refused targets, [`SendError::Retry`] if DNS fails.
pub async fn check_target(url: &str, allow_private: bool) -> Result<Url, SendError> {
    let u = Url::parse(url).map_err(|e| SendError::Permanent(format!("invalid URL: {e}")))?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err(SendError::Permanent("only http and https URLs".into()));
    }
    if allow_private {
        return Ok(u);
    }
    let host = u
        .host_str()
        .ok_or_else(|| SendError::Permanent("URL without host".into()))?;
    let port = u.port_or_known_default().unwrap_or(443);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let addrs: Vec<IpAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) => tokio::net::lookup_host((host, port))
            .await
            .map_err(|e| SendError::Retry(format!("cannot resolve {host}: {e}")))?
            .map(|a| a.ip())
            .collect(),
    };
    if addrs.iter().any(|ip| is_private(*ip)) {
        return Err(SendError::Permanent(format!(
            "{host} is a private address; an admin can allow private targets"
        )));
    }
    Ok(u)
}

fn json(v: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap_or_default()
}

fn push_path(mut u: Url, segments: &[&str]) -> Result<Url, SendError> {
    u.path_segments_mut()
        .map_err(|()| SendError::Permanent("URL cannot have a path".into()))?
        .pop_if_empty()
        .extend(segments);
    Ok(u)
}

impl HttpChannels {
    #[must_use]
    pub fn new(http: Arc<dyn HttpFetch>, allow_private: bool) -> Self {
        Self {
            http,
            allow_private,
        }
    }

    async fn target(&self, url: &str) -> Result<Url, SendError> {
        check_target(url, self.allow_private).await
    }

    async fn run(
        &self,
        what: &str,
        method: &str,
        url: Url,
        mut headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<(), SendError> {
        headers.push(("content-type".into(), "application/json".into()));
        let resp = self
            .http
            .fetch(HttpRequest {
                connector: CONNECTOR.into(),
                op: what.to_ascii_lowercase(),
                method: method.into(),
                url: url.into(),
                headers,
                body: Some(body),
                lane: Lane::Interactive,
            })
            .await
            .map_err(|e| SendError::Retry(format!("{what}: {e}")))?;
        if (200..300).contains(&resp.status) {
            Ok(())
        } else {
            Err(status_error(resp.status, what))
        }
    }

    async fn webhook(&self, url: &str, secret: &str, m: &Message) -> Result<(), SendError> {
        let target = self.target(url).await?;
        let ts = m.at.timestamp();
        let id = m.id.to_string();
        let body = json(&serde_json::json!({
            "type": m.event,
            "timestamp": m.at.to_rfc3339(),
            "data": { "title": m.title, "body": m.body, "link": m.link },
        }));
        let headers = vec![
            ("webhook-id".to_string(), id.clone()),
            ("webhook-timestamp".to_string(), ts.to_string()),
            (
                "webhook-signature".to_string(),
                sign(secret, &id, ts, &body),
            ),
        ];
        self.run("webhook", "POST", target, headers, body).await
    }

    /// # Errors
    /// See [`SendError`].
    pub async fn send(&self, channel: &ChannelConfig, m: &Message) -> Result<(), SendError> {
        match channel {
            ChannelConfig::Ntfy {
                server,
                topic,
                token,
            } => {
                let url = self.target(server).await?;
                let mut body = serde_json::json!({
                    "topic": topic, "title": m.title, "message": m.body,
                });
                if let Some(l) = &m.link {
                    body["click"] = l.clone().into();
                }
                let auth = token
                    .iter()
                    .map(|t| ("authorization".to_string(), format!("Bearer {t}")))
                    .collect();
                self.run("ntfy", "POST", url, auth, json(&body)).await
            }
            ChannelConfig::Gotify { server, token } => {
                let url = push_path(self.target(server).await?, &["message"])?;
                let mut body = serde_json::json!({
                    "title": m.title, "message": m.body, "priority": 5,
                });
                if let Some(l) = &m.link {
                    body["extras"] =
                        serde_json::json!({ "client::notification": { "click": { "url": l } } });
                }
                let key = vec![("x-gotify-key".to_string(), token.clone())];
                self.run("Gotify", "POST", url, key, json(&body)).await
            }
            ChannelConfig::Discord { webhook_url } => {
                let url = self.target(webhook_url).await?;
                let mut content = format!("**{}**\n{}", m.title, text(m));
                if content.chars().count() > DISCORD_MAX {
                    content = content.chars().take(DISCORD_MAX - 3).collect::<String>() + "...";
                }
                let body = serde_json::json!({ "content": content });
                self.run("Discord", "POST", url, vec![], json(&body)).await
            }
            ChannelConfig::Matrix {
                homeserver,
                room_id,
                access_token,
            } => {
                let txn = m.id.to_string();
                let url = push_path(
                    self.target(homeserver).await?,
                    // same transaction id on retries: the server sends the event once
                    &[
                        "_matrix",
                        "client",
                        "v3",
                        "rooms",
                        room_id,
                        "send",
                        "m.room.message",
                        &txn,
                    ],
                )?;
                let body = serde_json::json!({
                    "msgtype": "m.text",
                    "body": format!("{}\n{}", m.title, text(m)),
                });
                let auth = vec![(
                    "authorization".to_string(),
                    format!("Bearer {access_token}"),
                )];
                self.run("Matrix", "PUT", url, auth, json(&body)).await
            }
            ChannelConfig::Apprise { server, urls } => {
                let url = push_path(self.target(server).await?, &["notify", ""])?;
                let body = serde_json::json!({
                    "urls": urls, "title": m.title, "body": text(m), "type": "info",
                });
                self.run("Apprise", "POST", url, vec![], json(&body)).await
            }
            ChannelConfig::Webhook { url, secret } => self.webhook(url, secret, m).await,
            ChannelConfig::Email { .. } | ChannelConfig::WebPush { .. } => {
                Err(SendError::Permanent("not an HTTP channel".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ranges() {
        for ip in [
            "10.0.0.1",
            "192.168.1.2",
            "127.0.0.1",
            "169.254.1.1",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(is_private(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["93.184.216.34", "2001:db8::1", "100.128.0.1"] {
            assert!(!is_private(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn status_mapping() {
        assert!(matches!(status_error(410, "x"), SendError::Gone(_)));
        assert!(matches!(status_error(429, "x"), SendError::Retry(_)));
        assert!(matches!(status_error(503, "x"), SendError::Retry(_)));
        assert!(matches!(status_error(401, "x"), SendError::Permanent(_)));
    }

    #[test]
    fn signature_follows_standard_webhooks() {
        let secret = format!("whsec_{}", STANDARD.encode(b"secret-key-bytes-0123456789"));
        let a = sign(&secret, "msg_1", 1_700_000_000, b"{}");
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret-key-bytes-0123456789").unwrap();
        mac.update(b"msg_1.1700000000.{}");
        assert_eq!(
            a,
            format!("v1,{}", STANDARD.encode(mac.finalize().into_bytes()))
        );
        assert_ne!(sign("plain", "msg_1", 1_700_000_000, b"{}"), a);
    }
}
