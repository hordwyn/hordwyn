//! Channels, subscriptions, deliveries and inbox items.

use chrono::{DateTime, NaiveTime, Utc};
use domain::ids::{CompanionId, ItemId, ListId, UserId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Where a message goes, with its settings. Secrets (tokens, webhook secrets) are stored with
/// the channel and never returned by the API ([`ChannelConfig::redacted`]) or logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChannelConfig {
    Email {
        to: String,
    },
    Ntfy {
        server: String,
        topic: String,
        #[serde(default)]
        token: Option<String>,
    },
    Gotify {
        server: String,
        token: String,
    },
    Discord {
        webhook_url: String,
    },
    Matrix {
        homeserver: String,
        room_id: String,
        access_token: String,
    },
    Apprise {
        server: String,
        /// Apprise URLs, e.g. `tgram://...`.
        urls: String,
    },
    /// Signed with the Standard Webhooks scheme.
    Webhook {
        url: String,
        secret: String,
    },
    WebPush {
        endpoint: String,
        p256dh: String,
        auth: String,
    },
}

const HIDDEN: &str = "********";

impl ChannelConfig {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Email { .. } => "email",
            Self::Ntfy { .. } => "ntfy",
            Self::Gotify { .. } => "gotify",
            Self::Discord { .. } => "discord",
            Self::Matrix { .. } => "matrix",
            Self::Apprise { .. } => "apprise",
            Self::Webhook { .. } => "webhook",
            Self::WebPush { .. } => "web_push",
        }
    }

    /// The config without secrets, for API answers. Discord webhook URLs and Apprise URLs
    /// contain tokens and are hidden as a whole.
    #[must_use]
    pub fn redacted(&self) -> Self {
        let mut c = self.clone();
        match &mut c {
            Self::Ntfy { token, .. } => {
                if token.is_some() {
                    *token = Some(HIDDEN.into());
                }
            }
            Self::Gotify { token, .. } => *token = HIDDEN.into(),
            Self::Discord { webhook_url } => *webhook_url = HIDDEN.into(),
            Self::Matrix { access_token, .. } => *access_token = HIDDEN.into(),
            Self::Apprise { urls, .. } => *urls = HIDDEN.into(),
            Self::Webhook { secret, .. } => *secret = HIDDEN.into(),
            Self::WebPush { p256dh, auth, .. } => {
                *p256dh = HIDDEN.into();
                *auth = HIDDEN.into();
            }
            Self::Email { .. } => {}
        }
        c
    }

    /// Keep secrets of `old` where `self` carries the redaction placeholder (updates from
    /// clients that only ever saw the redacted config).
    #[must_use]
    pub fn keep_secrets_of(mut self, old: &Self) -> Self {
        let keep = |new: &mut String, old: &String| {
            if new == HIDDEN {
                new.clone_from(old);
            }
        };
        match (&mut self, old) {
            (Self::Ntfy { token: Some(t), .. }, Self::Ntfy { token: Some(o), .. })
            | (Self::Gotify { token: t, .. }, Self::Gotify { token: o, .. })
            | (Self::Discord { webhook_url: t }, Self::Discord { webhook_url: o })
            | (
                Self::Matrix {
                    access_token: t, ..
                },
                Self::Matrix {
                    access_token: o, ..
                },
            )
            | (Self::Apprise { urls: t, .. }, Self::Apprise { urls: o, .. })
            | (Self::Webhook { secret: t, .. }, Self::Webhook { secret: o, .. }) => keep(t, o),
            (
                Self::WebPush { p256dh, auth, .. },
                Self::WebPush {
                    p256dh: op,
                    auth: oa,
                    ..
                },
            ) => {
                keep(p256dh, op);
                keep(auth, oa);
            }
            _ => {}
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Channel {
    pub id: Uuid,
    pub user: UserId,
    pub name: String,
    pub config: ChannelConfig,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum DeliveryMode {
    Immediate,
    /// Collected and sent together on a cron schedule (5 fields, in the subscription's
    /// timezone), e.g. `0 8 * * *` for every morning at eight.
    Digest {
        cron: String,
    },
}

/// Local times without immediate deliveries; may wrap midnight (22:00-07:00).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuietHours {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

/// Empty lists match anything.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filter {
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub lists: Vec<ListId>,
    #[serde(default)]
    pub items: Vec<ItemId>,
    #[serde(default)]
    pub companions: Vec<CompanionId>,
}

/// Which of the user's channels a subscription sends to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "ids", rename_all = "snake_case")]
pub enum Targets {
    /// Every enabled channel of the user (instance defaults use this).
    All,
    Only(Vec<Uuid>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    /// `None`: an instance default set by an admin, applying to every user who has no own
    /// subscription for the same event pattern (or always, if `locked`).
    pub user: Option<UserId>,
    /// An event id, `group.*` or `*`.
    pub event: String,
    #[serde(default)]
    pub filter: Filter,
    pub targets: Targets,
    pub mode: DeliveryMode,
    #[serde(default)]
    pub quiet_hours: Option<QuietHours>,
    /// IANA name, e.g. `Europe/Berlin`.
    pub timezone: String,
    /// Most deliveries per channel and event within an hour; more are suppressed (they stay
    /// in the inbox).
    #[serde(default)]
    pub rate_limit_per_hour: Option<u32>,
    /// `false` mutes the event pattern (an own subscription can mute a default).
    pub enabled: bool,
    /// Instance defaults only: users cannot override or mute it.
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryState {
    Queued,
    /// Waiting for the next digest of its channel.
    Digest,
    /// Claimed by a run that is sending it (OT-37, #6): `sent_at` holds the claim time until the
    /// outcome is written; a claim older than [`crate::SENDING_STALE_MINUTES`] is
    /// taken over (the run died).
    Sending,
    Sent,
    Failed,
    /// Rate-limited or the channel was disabled.
    Suppressed,
}

impl DeliveryState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Digest => "digest",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Failed => "failed",
            Self::Suppressed => "suppressed",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "queued" => Self::Queued,
            "digest" => Self::Digest,
            "sending" => Self::Sending,
            "sent" => Self::Sent,
            "failed" => Self::Failed,
            "suppressed" => Self::Suppressed,
            _ => return None,
        })
    }
}

/// One message to one channel; the delivery log shows these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    pub id: Uuid,
    pub user: UserId,
    pub channel: Uuid,
    pub event: String,
    pub dedupe_key: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
    pub state: DeliveryState,
    pub attempts: u32,
    pub last_error: Option<String>,
    /// When it may be sent (end of quiet hours, next digest).
    pub due_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub sent_at: Option<DateTime<Utc>>,
}

/// The in-app inbox: every notification of a user lands here, whatever the subscriptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: Uuid,
    pub user: UserId,
    pub event: String,
    pub dedupe_key: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_redacted_and_kept_on_update() {
        let c = ChannelConfig::Gotify {
            server: "https://push.example.org".into(),
            token: "s3cret".into(),
        };
        let r = c.redacted();
        assert_eq!(
            r,
            ChannelConfig::Gotify {
                server: "https://push.example.org".into(),
                token: HIDDEN.into()
            }
        );
        let updated = ChannelConfig::Gotify {
            server: "https://other.example.org".into(),
            token: HIDDEN.into(),
        }
        .keep_secrets_of(&c);
        assert_eq!(
            updated,
            ChannelConfig::Gotify {
                server: "https://other.example.org".into(),
                token: "s3cret".into()
            }
        );
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["kind"], "gotify");
    }
}
