//! Channel adapters. [`Channels`] picks the adapter for a channel's config; adapters left out
//! of the build (features `http`, `email`) fail permanently with a clear message.

use async_trait::async_trait;

use crate::deliver::{Message, Notifier, SendError};
use crate::model::ChannelConfig;

#[cfg(feature = "email")]
pub mod email;
#[cfg(feature = "http")]
pub mod http;

#[derive(Default)]
pub struct Channels {
    #[cfg(feature = "http")]
    pub http: Option<http::HttpChannels>,
    #[cfg(feature = "email")]
    pub email: std::sync::Arc<email::Mailer>,
}

fn missing(what: &str) -> SendError {
    SendError::Permanent(format!("{what} channels are not configured on this server"))
}

#[async_trait]
impl Notifier for Channels {
    #[cfg_attr(not(all(feature = "http", feature = "email")), allow(unused_variables))]
    async fn send(&self, channel: &ChannelConfig, m: &Message) -> Result<(), SendError> {
        match channel {
            ChannelConfig::Email { .. } => {
                #[cfg(feature = "email")]
                if let Some(e) = self.email.current() {
                    return e.send(channel, m).await;
                }
                Err(missing("email"))
            }
            // not built yet (DEC-141): the only maintained pure-Rust crate pins a crypto-bigint
            // with a constant-time advisory (RUSTSEC-2026-0387)
            ChannelConfig::WebPush { .. } => Err(SendError::Permanent(
                "web push is not available yet on this server".into(),
            )),
            _ => {
                #[cfg(feature = "http")]
                if let Some(h) = &self.http {
                    return h.send(channel, m).await;
                }
                Err(missing(channel.kind()))
            }
        }
    }
}
