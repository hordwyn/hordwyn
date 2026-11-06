//! Postgres adapter for [`NotifyStore`] (store-pg migration `0010_notifications.sql`).

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use serde::de::DeserializeOwned;
use sqlx::PgPool;
use uuid::Uuid;

use crate::model::{Channel, Delivery, DeliveryState, InboxItem, Subscription};
use crate::store::{NotifyStore, StoreResult};
use crate::template::Overrides;

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("notifications db: {e}"))
}

fn from_json<T: DeserializeOwned>(v: serde_json::Value) -> StoreResult<T> {
    serde_json::from_value(v).map_err(|e| PortError(format!("notifications db: {e}")))
}

fn to_json<T: serde::Serialize>(v: &T) -> StoreResult<serde_json::Value> {
    serde_json::to_value(v).map_err(|e| PortError(e.to_string()))
}

#[derive(Clone)]
pub struct PgNotifyStore {
    pool: PgPool,
}

impl PgNotifyStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

struct ChannelRow {
    id: Uuid,
    user_id: Uuid,
    name: String,
    config: serde_json::Value,
    enabled: bool,
    created_at: DateTime<Utc>,
}

impl ChannelRow {
    fn into_channel(self) -> StoreResult<Channel> {
        Ok(Channel {
            id: self.id,
            user: UserId(self.user_id),
            name: self.name,
            config: from_json(self.config)?,
            enabled: self.enabled,
            created_at: self.created_at,
        })
    }
}

struct SubRow {
    id: Uuid,
    user_id: Option<Uuid>,
    event: String,
    filter: serde_json::Value,
    targets: serde_json::Value,
    mode: serde_json::Value,
    quiet_hours: Option<serde_json::Value>,
    timezone: String,
    rate_limit_per_hour: Option<i32>,
    enabled: bool,
    locked: bool,
}

impl SubRow {
    fn into_sub(self) -> StoreResult<Subscription> {
        Ok(Subscription {
            id: self.id,
            user: self.user_id.map(UserId),
            event: self.event,
            filter: from_json(self.filter)?,
            targets: from_json(self.targets)?,
            mode: from_json(self.mode)?,
            quiet_hours: self.quiet_hours.map(from_json).transpose()?,
            timezone: self.timezone,
            rate_limit_per_hour: self.rate_limit_per_hour.and_then(|n| u32::try_from(n).ok()),
            enabled: self.enabled,
            locked: self.locked,
        })
    }
}

struct DeliveryRow {
    id: Uuid,
    user_id: Uuid,
    channel_id: Uuid,
    event: String,
    dedupe_key: String,
    title: String,
    body: String,
    link: Option<String>,
    state: String,
    attempts: i32,
    last_error: Option<String>,
    due_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    sent_at: Option<DateTime<Utc>>,
}

impl DeliveryRow {
    fn into_delivery(self) -> StoreResult<Delivery> {
        Ok(Delivery {
            id: self.id,
            user: UserId(self.user_id),
            channel: self.channel_id,
            event: self.event,
            dedupe_key: self.dedupe_key,
            title: self.title,
            body: self.body,
            link: self.link,
            state: DeliveryState::parse(&self.state)
                .ok_or_else(|| PortError(format!("unknown delivery state {}", self.state)))?,
            attempts: u32::try_from(self.attempts).unwrap_or(0),
            last_error: self.last_error,
            due_at: self.due_at,
            created_at: self.created_at,
            sent_at: self.sent_at,
        })
    }
}

struct InboxRow {
    id: Uuid,
    user_id: Uuid,
    event: String,
    dedupe_key: String,
    title: String,
    body: String,
    link: Option<String>,
    created_at: DateTime<Utc>,
    read_at: Option<DateTime<Utc>>,
}

impl From<InboxRow> for InboxItem {
    fn from(r: InboxRow) -> Self {
        Self {
            id: r.id,
            user: UserId(r.user_id),
            event: r.event,
            dedupe_key: r.dedupe_key,
            title: r.title,
            body: r.body,
            link: r.link,
            created_at: r.created_at,
            read_at: r.read_at,
        }
    }
}

fn limit(n: u32) -> i64 {
    i64::from(n.min(500))
}

#[async_trait]
impl NotifyStore for PgNotifyStore {
    async fn admins(&self) -> StoreResult<Vec<UserId>> {
        let ids = sqlx::query_scalar!("SELECT id FROM users WHERE role = 'admin' ORDER BY id")
            .fetch_all(&self.pool)
            .await
            .map_err(pe)?;
        Ok(ids.into_iter().map(UserId).collect())
    }

    async fn channels(&self, user: UserId) -> StoreResult<Vec<Channel>> {
        sqlx::query_as!(
            ChannelRow,
            "SELECT id, user_id, name, config, enabled, created_at FROM notify_channels
             WHERE user_id = $1 ORDER BY created_at, id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(ChannelRow::into_channel)
        .collect()
    }

    async fn channel(&self, id: Uuid) -> StoreResult<Option<Channel>> {
        sqlx::query_as!(
            ChannelRow,
            "SELECT id, user_id, name, config, enabled, created_at FROM notify_channels
             WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(ChannelRow::into_channel)
        .transpose()
    }

    async fn put_channel(&self, c: &Channel) -> StoreResult<()> {
        sqlx::query!(
            "INSERT INTO notify_channels (id, user_id, name, config, enabled, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, config = EXCLUDED.config,
                 enabled = EXCLUDED.enabled",
            c.id,
            c.user.0,
            c.name,
            to_json(&c.config)?,
            c.enabled,
            c.created_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn delete_channel(&self, id: Uuid) -> StoreResult<()> {
        sqlx::query!("DELETE FROM notify_channels WHERE id = $1", id)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }

    async fn subscriptions(&self, user: UserId) -> StoreResult<Vec<Subscription>> {
        sqlx::query_as!(
            SubRow,
            "SELECT id, user_id, event, filter, targets, mode, quiet_hours, timezone,
                    rate_limit_per_hour, enabled, locked
             FROM notify_subscriptions WHERE user_id = $1 ORDER BY id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(SubRow::into_sub)
        .collect()
    }

    async fn defaults(&self) -> StoreResult<Vec<Subscription>> {
        sqlx::query_as!(
            SubRow,
            "SELECT id, user_id, event, filter, targets, mode, quiet_hours, timezone,
                    rate_limit_per_hour, enabled, locked
             FROM notify_subscriptions WHERE user_id IS NULL ORDER BY id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(SubRow::into_sub)
        .collect()
    }

    async fn subscription(&self, id: Uuid) -> StoreResult<Option<Subscription>> {
        sqlx::query_as!(
            SubRow,
            "SELECT id, user_id, event, filter, targets, mode, quiet_hours, timezone,
                    rate_limit_per_hour, enabled, locked
             FROM notify_subscriptions WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(SubRow::into_sub)
        .transpose()
    }

    async fn put_subscription(&self, s: &Subscription) -> StoreResult<()> {
        let quiet = s.quiet_hours.as_ref().map(to_json).transpose()?;
        sqlx::query!(
            "INSERT INTO notify_subscriptions (id, user_id, event, filter, targets, mode,
                 quiet_hours, timezone, rate_limit_per_hour, enabled, locked)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (id) DO UPDATE SET event = EXCLUDED.event, filter = EXCLUDED.filter,
                 targets = EXCLUDED.targets, mode = EXCLUDED.mode,
                 quiet_hours = EXCLUDED.quiet_hours, timezone = EXCLUDED.timezone,
                 rate_limit_per_hour = EXCLUDED.rate_limit_per_hour,
                 enabled = EXCLUDED.enabled, locked = EXCLUDED.locked",
            s.id,
            s.user.map(|u| u.0),
            s.event,
            to_json(&s.filter)?,
            to_json(&s.targets)?,
            to_json(&s.mode)?,
            quiet,
            s.timezone,
            s.rate_limit_per_hour
                .map(|n| i32::try_from(n).unwrap_or(i32::MAX)),
            s.enabled,
            s.locked
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn delete_subscription(&self, id: Uuid) -> StoreResult<()> {
        sqlx::query!("DELETE FROM notify_subscriptions WHERE id = $1", id)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }

    async fn insert_inbox(&self, i: &InboxItem) -> StoreResult<bool> {
        let r = sqlx::query!(
            "INSERT INTO notify_inbox (id, user_id, event, dedupe_key, title, body, link,
                 created_at, read_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (user_id, event, dedupe_key) DO NOTHING",
            i.id,
            i.user.0,
            i.event,
            i.dedupe_key,
            i.title,
            i.body,
            i.link,
            i.created_at,
            i.read_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.rows_affected() == 1)
    }

    async fn inbox(&self, user: UserId, n: u32) -> StoreResult<Vec<InboxItem>> {
        Ok(sqlx::query_as!(
            InboxRow,
            "SELECT id, user_id, event, dedupe_key, title, body, link, created_at, read_at
             FROM notify_inbox WHERE user_id = $1 ORDER BY created_at DESC, id DESC LIMIT $2",
            user.0,
            limit(n)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(InboxItem::from)
        .collect())
    }

    async fn mark_read(&self, user: UserId, id: Uuid, at: DateTime<Utc>) -> StoreResult<()> {
        sqlx::query!(
            "UPDATE notify_inbox SET read_at = $3
             WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
            id,
            user.0,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn insert_delivery(&self, d: &Delivery) -> StoreResult<bool> {
        let r = sqlx::query!(
            "INSERT INTO notify_deliveries (id, user_id, channel_id, event, dedupe_key, title,
                 body, link, state, attempts, last_error, due_at, created_at, sent_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             ON CONFLICT (channel_id, event, dedupe_key) DO NOTHING",
            d.id,
            d.user.0,
            d.channel,
            d.event,
            d.dedupe_key,
            d.title,
            d.body,
            d.link,
            d.state.as_str(),
            i32::try_from(d.attempts).unwrap_or(i32::MAX),
            d.last_error,
            d.due_at,
            d.created_at,
            d.sent_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.rows_affected() == 1)
    }

    async fn delivery(&self, id: Uuid) -> StoreResult<Option<Delivery>> {
        sqlx::query_as!(
            DeliveryRow,
            "SELECT id, user_id, channel_id, event, dedupe_key, title, body, link, state,
                    attempts, last_error, due_at, created_at, sent_at
             FROM notify_deliveries WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(DeliveryRow::into_delivery)
        .transpose()
    }

    async fn update_delivery(&self, d: &Delivery) -> StoreResult<()> {
        sqlx::query!(
            "UPDATE notify_deliveries SET state = $2, attempts = $3, last_error = $4,
                 due_at = $5, sent_at = $6 WHERE id = $1",
            d.id,
            d.state.as_str(),
            i32::try_from(d.attempts).unwrap_or(i32::MAX),
            d.last_error,
            d.due_at,
            d.sent_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn deliveries(&self, user: UserId, n: u32) -> StoreResult<Vec<Delivery>> {
        sqlx::query_as!(
            DeliveryRow,
            "SELECT id, user_id, channel_id, event, dedupe_key, title, body, link, state,
                    attempts, last_error, due_at, created_at, sent_at
             FROM notify_deliveries WHERE user_id = $1
             ORDER BY created_at DESC, id DESC LIMIT $2",
            user.0,
            limit(n)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(DeliveryRow::into_delivery)
        .collect()
    }

    async fn due_digest(&self, channel: Uuid, until: DateTime<Utc>) -> StoreResult<Vec<Delivery>> {
        sqlx::query_as!(
            DeliveryRow,
            "SELECT id, user_id, channel_id, event, dedupe_key, title, body, link, state,
                    attempts, last_error, due_at, created_at, sent_at
             FROM notify_deliveries WHERE channel_id = $1 AND state = 'digest' AND due_at <= $2
             ORDER BY created_at, id",
            channel,
            until
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(DeliveryRow::into_delivery)
        .collect()
    }

    async fn count_since(
        &self,
        channel: Uuid,
        event: &str,
        since: DateTime<Utc>,
    ) -> StoreResult<u32> {
        let n = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!" FROM notify_deliveries
               WHERE channel_id = $1 AND event = $2 AND created_at >= $3
                 AND state <> 'suppressed'"#,
            channel,
            event,
            since
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pe)?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    async fn template_overrides(&self) -> StoreResult<Overrides> {
        let rows = sqlx::query!("SELECT locale, key, text FROM notify_templates")
            .fetch_all(&self.pool)
            .await
            .map_err(pe)?;
        let mut out = Overrides::new();
        for r in rows {
            out.entry(r.locale).or_default().insert(r.key, r.text);
        }
        Ok(out)
    }

    async fn set_template_override(
        &self,
        locale: &str,
        key: &str,
        text: Option<&str>,
    ) -> StoreResult<()> {
        match text {
            Some(t) => sqlx::query!(
                "INSERT INTO notify_templates (locale, key, text) VALUES ($1, $2, $3)
                 ON CONFLICT (locale, key) DO UPDATE SET text = EXCLUDED.text",
                locale,
                key,
                t
            )
            .execute(&self.pool)
            .await
            .map_err(pe)?,
            None => sqlx::query!(
                "DELETE FROM notify_templates WHERE locale = $1 AND key = $2",
                locale,
                key
            )
            .execute(&self.pool)
            .await
            .map_err(pe)?,
        };
        Ok(())
    }
}
