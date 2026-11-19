//! Sending: the jobs `notify.deliver` (one delivery) and `notify.digest` (everything a
//! channel collected until a time). Channels sit behind the [`Notifier`] port.

use std::sync::Arc;

use app::ports::Clock;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use crate::Engine;
use crate::model::{ChannelConfig, Delivery, DeliveryState};

/// What a channel sends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Stable across retries (Matrix transaction id, `webhook-id`).
    pub id: Uuid,
    pub event: String,
    pub title: String,
    pub body: String,
    /// Absolute URL to open.
    pub link: Option<String>,
    /// When it is sent (webhook timestamp).
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SendError {
    /// Temporary (network, 5xx, 429): the job retries with backoff.
    #[error("{0}")]
    Retry(String),
    /// The request was refused; retrying does not help.
    #[error("{0}")]
    Permanent(String),
    /// The target no longer exists (web push 404/410, webhook 410): the channel is disabled.
    #[error("gone: {0}")]
    Gone(String),
}

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn send(&self, channel: &ChannelConfig, m: &Message) -> Result<(), SendError>;
}

/// A claim older than this is taken over: the run that made it died before it wrote the
/// outcome (a job lease is shorter, DEC-844), so the delivery is sent by the next run.
pub const SENDING_STALE_MINUTES: i64 = 15;

pub struct Deliverer {
    engine: Arc<Engine>,
    notifier: Arc<dyn Notifier>,
    clock: Arc<dyn Clock>,
    public_url: String,
    max_attempts: u32,
}

impl Deliverer {
    /// `public_url` makes app paths absolute; a delivery fails for good after `max_attempts`.
    #[must_use]
    pub fn new(
        engine: Arc<Engine>,
        notifier: Arc<dyn Notifier>,
        clock: Arc<dyn Clock>,
        public_url: &str,
        max_attempts: u32,
    ) -> Self {
        Self {
            engine,
            notifier,
            clock,
            public_url: public_url.trim_end_matches('/').to_string(),
            max_attempts: max_attempts.max(1),
        }
    }

    fn link(&self, path: Option<&String>) -> Option<String> {
        path.map(|p| {
            if p.starts_with('/') {
                format!("{}{p}", self.public_url)
            } else {
                p.clone()
            }
        })
    }

    async fn save(&self, d: &Delivery) -> Result<(), String> {
        self.engine
            .store()
            .update_delivery(d)
            .await
            .map_err(|e| e.to_string())
    }

    /// Tell the open pages of the channel's owner that an outcome was written: the log row
    /// and the health of the channel changed ([`crate::DELIVERY_TOPIC`]).
    async fn announce(&self, d: &Delivery) {
        self.engine.delivery_changed(d.user, d.channel).await;
    }

    async fn disable(&self, channel: Uuid) -> Result<(), String> {
        let store = self.engine.store();
        if let Some(mut c) = store.channel(channel).await.map_err(|e| e.to_string())? {
            c.enabled = false;
            store.put_channel(&c).await.map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Send a test message to a channel right away (the "test" button); nothing is logged.
    ///
    /// # Errors
    /// What the channel answered.
    pub async fn test(&self, channel: &crate::model::Channel) -> Result<(), SendError> {
        let mut args = serde_json::Map::new();
        args.insert("name".into(), channel.name.clone().into());
        let t = self.engine.templates();
        let locale = self
            .engine
            .locale_for(channel.user)
            .await
            .unwrap_or_else(|_| self.engine.locale().to_string());
        let r = t.render("test", channel.config.kind(), &locale, &args);
        let m = Message {
            id: Uuid::new_v5(&channel.id, self.clock.now().to_rfc3339().as_bytes()),
            event: "test".into(),
            title: r.title,
            body: r.body,
            link: self.link(Some(&"/settings/notifications".to_string())),
            at: self.clock.now(),
        };
        self.notifier.send(&channel.config, &m).await
    }

    /// Send one queued delivery.
    ///
    /// # Errors
    /// A temporary failure (the job is retried) or a storage error.
    pub async fn deliver(&self, id: Uuid) -> Result<(), String> {
        let store = self.engine.store();
        // claimed before it is sent: a second run (two workers, a lapsed lease) finds nothing to
        // claim and sends nothing (OT-37, #6)
        let now = self.clock.now();
        let stale = now - chrono::Duration::minutes(SENDING_STALE_MINUTES);
        let Some(mut d) = store
            .claim_delivery(id, now, stale)
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(());
        };
        let channel = store.channel(d.channel).await.map_err(|e| e.to_string())?;
        let Some(channel) = channel.filter(|c| c.enabled) else {
            d.state = DeliveryState::Suppressed;
            d.sent_at = None;
            d.last_error = Some("channel disabled or deleted".into());
            self.save(&d).await?;
            self.announce(&d).await;
            return Ok(());
        };
        let m = Message {
            id: d.id,
            event: d.event.clone(),
            title: d.title.clone(),
            body: d.body.clone(),
            link: self.link(d.link.as_ref()),
            at: self.clock.now(),
        };
        d.attempts += 1;
        let result = self.notifier.send(&channel.config, &m).await;
        let finished = self
            .finish(&mut d, result, self.clock.now(), DeliveryState::Queued)
            .await;
        // also for an attempt that is retried: the log shows it
        self.announce(&d).await;
        finished
    }

    /// Write the outcome of a claimed delivery; a retried one goes `back` to the state it was
    /// claimed from (`queued`, or `digest` for the next digest run).
    async fn finish(
        &self,
        d: &mut Delivery,
        result: Result<(), SendError>,
        now: DateTime<Utc>,
        back: DeliveryState,
    ) -> Result<(), String> {
        match result {
            Ok(()) => {
                d.state = DeliveryState::Sent;
                d.sent_at = Some(now);
                d.last_error = None;
                self.save(d).await
            }
            Err(SendError::Retry(e)) if d.attempts < self.max_attempts => {
                d.state = back;
                d.sent_at = None;
                d.last_error = Some(e.clone());
                self.save(d).await?;
                Err(e)
            }
            Err(e) => {
                if matches!(e, SendError::Gone(_)) {
                    self.disable(d.channel).await?;
                }
                d.state = DeliveryState::Failed;
                d.sent_at = None;
                d.last_error = Some(e.to_string());
                self.save(d).await
            }
        }
    }

    /// Send what a channel collected for its digest until `until` as one message.
    ///
    /// # Errors
    /// A temporary failure (the job is retried) or a storage error.
    pub async fn digest(&self, channel_id: Uuid, until: DateTime<Utc>) -> Result<(), String> {
        let store = self.engine.store();
        // claimed in one step, as a single delivery is (OT-37, #6)
        let now = self.clock.now();
        let stale = now - chrono::Duration::minutes(SENDING_STALE_MINUTES);
        let mut items = store
            .claim_digest(channel_id, until, now, stale)
            .await
            .map_err(|e| e.to_string())?;
        if items.is_empty() {
            return Ok(());
        }
        let channel = store.channel(channel_id).await.map_err(|e| e.to_string())?;
        let Some(channel) = channel.filter(|c| c.enabled) else {
            for d in &mut items {
                d.state = DeliveryState::Suppressed;
                d.sent_at = None;
                d.last_error = Some("channel disabled or deleted".into());
                self.save(d).await?;
            }
            self.announce(&items[0]).await;
            return Ok(());
        };
        let locale = self
            .engine
            .locale_for(channel.user)
            .await
            .map_err(|e| e.to_string())?;
        let t = self.engine.templates();
        let body = items
            .iter()
            .map(|d| {
                let mut line = format!("- {}", d.title);
                if !d.body.is_empty() {
                    line.push_str(": ");
                    line.push_str(&d.body);
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n");
        let m = Message {
            id: Uuid::new_v5(&channel_id, until.to_rfc3339().as_bytes()),
            event: "digest".into(),
            title: t.digest_title(&locale, items.len()),
            body,
            link: self.link(Some(&"/settings/notifications".to_string())),
            at: self.clock.now(),
        };
        let result = self.notifier.send(&channel.config, &m).await;
        let now = self.clock.now();
        let mut retry = None;
        for d in &mut items {
            d.attempts += 1;
            if let Err(e) = self
                .finish(d, result.clone(), now, DeliveryState::Digest)
                .await
            {
                retry = Some(e);
            }
        }
        // one announcement for the whole digest
        self.announce(&items[0]).await;
        retry.map_or(Ok(()), Err)
    }
}
