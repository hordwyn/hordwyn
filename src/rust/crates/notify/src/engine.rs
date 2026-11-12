//! The engine behind [`app::ports::Notifications`]: every notification goes to the inbox of
//! each recipient (once per event and dedupe key), then to the channels of the matching
//! subscriptions as deliveries: queued now, after quiet hours, collected for a digest, or
//! suppressed by a rate limit. Sending runs as jobs ([`crate::Deliverer`]).

use std::collections::HashSet;
use std::sync::{Arc, RwLock};

use app::ports::{
    AppEvent, Audience, Clock, EntityRef, EventBus, IdGen, JobQueue, Lane, NewJob, Notification,
    Notifications,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use domain::ids::UserId;

use crate::model::{Delivery, DeliveryMode, DeliveryState, InboxItem};
use crate::plan::{effective, matching, next_digest, quiet_until, timezone};
use crate::store::{NotifyStore, StoreResult};
use crate::template::{Overrides, Templates};
use crate::{DELIVER_JOB, DIGEST_JOB};

/// Topic of the realtime event for a new inbox item.
pub const INBOX_TOPIC: &str = "notification.new";
/// Topic of the realtime event for a delivery whose outcome was written (its log row and the
/// health of its channel changed); payload `{"channel_id": ...}`.
pub const DELIVERY_TOPIC: &str = "notification.delivery";

pub struct Engine {
    store: Arc<dyn NotifyStore>,
    jobs: Arc<dyn JobQueue>,
    events: Arc<dyn EventBus>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGen>,
    product: String,
    locale: String,
    brand: Overrides,
    templates: RwLock<Arc<Templates>>,
}

impl Engine {
    /// `locale` is the instance language for texts; `brand` the brand's string overrides.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: Arc<dyn NotifyStore>,
        jobs: Arc<dyn JobQueue>,
        events: Arc<dyn EventBus>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGen>,
        product: &str,
        locale: &str,
        brand: Overrides,
    ) -> Self {
        let templates = Templates::new(product, &brand, &Overrides::new());
        Self {
            store,
            jobs,
            events,
            clock,
            ids,
            product: product.to_string(),
            locale: locale.to_string(),
            brand,
            templates: RwLock::new(Arc::new(templates)),
        }
    }

    /// Publish [`DELIVERY_TOPIC`] to `user`: the outcome of a delivery to `channel` was
    /// written.
    pub(crate) async fn delivery_changed(&self, user: UserId, channel: uuid::Uuid) {
        self.events
            .publish(AppEvent {
                topic: DELIVERY_TOPIC.into(),
                entity: None,
                audience: vec![user],
                payload: serde_json::json!({ "channel_id": channel }),
            })
            .await;
    }

    #[must_use]
    pub fn store(&self) -> &Arc<dyn NotifyStore> {
        &self.store
    }

    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    #[must_use]
    pub fn templates(&self) -> Arc<Templates> {
        self.templates
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The language of texts for `user`: their choice when it has built-in texts, else the
    /// instance language (DEC-201).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn locale_for(&self, user: UserId) -> StoreResult<String> {
        Ok(self
            .store
            .user_locale(user)
            .await?
            .filter(|l| crate::template::LOCALES.contains(&l.as_str()))
            .unwrap_or_else(|| self.locale.clone()))
    }

    /// Rebuild the texts after admin overrides changed.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn reload_templates(&self) -> StoreResult<()> {
        let admin = self.store.template_overrides().await?;
        let t = Templates::new(&self.product, &self.brand, &admin);
        *self
            .templates
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Arc::new(t);
        Ok(())
    }

    /// Like [`Notifications::emit`] but with errors.
    ///
    /// # Errors
    /// Storage or queue errors.
    pub async fn emit_checked(&self, n: &Notification) -> StoreResult<()> {
        let users = match &n.audience {
            Audience::Users(u) => u.clone(),
            Audience::Admins => self.store.admins().await?,
        };
        for u in users {
            self.for_user(n, u).await?;
        }
        Ok(())
    }

    /// State and due time of a new delivery: suppressed by the rate limit, collected for the
    /// next digest, or queued (after quiet hours).
    async fn schedule(
        &self,
        sub: &crate::model::Subscription,
        channel: uuid::Uuid,
        event: &str,
        now: DateTime<Utc>,
    ) -> StoreResult<(DeliveryState, DateTime<Utc>)> {
        if let Some(limit) = sub.rate_limit_per_hour
            && self
                .store
                .count_since(channel, event, now - Duration::hours(1))
                .await?
                >= limit
        {
            return Ok((DeliveryState::Suppressed, now));
        }
        let tz = timezone(&sub.timezone);
        Ok(match &sub.mode {
            DeliveryMode::Digest { cron } => match next_digest(cron, tz, now) {
                Ok(at) => (DeliveryState::Digest, at),
                Err(e) => {
                    tracing::warn!(subscription = %sub.id, error = %e, "invalid digest schedule; sending now");
                    (DeliveryState::Queued, now)
                }
            },
            DeliveryMode::Immediate => (
                DeliveryState::Queued,
                sub.quiet_hours
                    .and_then(|q| quiet_until(now, tz, q))
                    .unwrap_or(now),
            ),
        })
    }

    async fn for_user(&self, n: &Notification, user: UserId) -> StoreResult<()> {
        let now = self.clock.now();
        let locale = self.locale_for(user).await?;
        let t = self.templates();
        let inbox = t.render(&n.event, "inbox", &locale, &n.args);
        let item = InboxItem {
            id: self.ids.new_id(),
            user,
            event: n.event.clone(),
            dedupe_key: n.dedupe_key.clone(),
            title: inbox.title.clone(),
            body: inbox.body,
            link: n.link.clone(),
            created_at: now,
            read_at: None,
        };
        if !self.store.insert_inbox(&item).await? {
            // the same event already reached this user (dedupe)
            return Ok(());
        }
        self.events
            .publish(AppEvent {
                topic: INBOX_TOPIC.into(),
                entity: Some(EntityRef {
                    entity: "notification".into(),
                    id: item.id,
                }),
                audience: vec![user],
                payload: serde_json::json!({ "title": inbox.title, "event": n.event }),
            })
            .await;

        let subs = effective(
            &self.store.subscriptions(user).await?,
            &self.store.defaults().await?,
        );
        let wanted = matching(&subs, &n.event, &n.subject);
        if wanted.is_empty() {
            return Ok(());
        }
        let channels: Vec<_> = self
            .store
            .channels(user)
            .await?
            .into_iter()
            .filter(|c| c.enabled)
            .collect();
        let mut done = HashSet::new();
        for sub in wanted {
            for ch in channels.iter().filter(|c| match &sub.targets {
                crate::model::Targets::All => true,
                crate::model::Targets::Only(ids) => ids.contains(&c.id),
            }) {
                // one delivery per channel even if several subscriptions match
                if !done.insert(ch.id) {
                    continue;
                }
                let (state, due_at) = self.schedule(sub, ch.id, &n.event, now).await?;
                let r = t.render(&n.event, ch.config.kind(), &locale, &n.args);
                let d = Delivery {
                    id: self.ids.new_id(),
                    user,
                    channel: ch.id,
                    event: n.event.clone(),
                    dedupe_key: n.dedupe_key.clone(),
                    title: r.title,
                    body: r.body,
                    link: n.link.clone(),
                    state,
                    attempts: 0,
                    last_error: None,
                    due_at,
                    created_at: now,
                    sent_at: None,
                };
                if !self.store.insert_delivery(&d).await? {
                    continue;
                }
                if let Some(job) = job_for(&d, now) {
                    self.jobs.enqueue(job).await?;
                }
            }
        }
        Ok(())
    }
}

/// The job that sends a new delivery: now or later, alone or with its channel's digest.
fn job_for(d: &Delivery, now: DateTime<Utc>) -> Option<NewJob> {
    match d.state {
        DeliveryState::Queued => Some(NewJob {
            lane: Lane::Interactive,
            kind: DELIVER_JOB.into(),
            payload: serde_json::json!({ "delivery_id": d.id }),
            dedupe_key: Some(format!("notify:{}", d.id)),
            priority: 0,
            run_at: (d.due_at > now).then_some(d.due_at),
        }),
        DeliveryState::Digest => Some(NewJob {
            lane: Lane::Interactive,
            kind: DIGEST_JOB.into(),
            payload: serde_json::json!({ "channel_id": d.channel, "until": d.due_at }),
            dedupe_key: Some(format!("digest:{}:{}", d.channel, d.due_at.timestamp())),
            priority: 0,
            run_at: Some(d.due_at),
        }),
        _ => None,
    }
}

#[async_trait]
impl Notifications for Engine {
    async fn emit(&self, n: Notification) {
        if let Err(e) = self.emit_checked(&n).await {
            tracing::warn!(event = %n.event, error = %e, "notification not recorded");
        }
    }
}
