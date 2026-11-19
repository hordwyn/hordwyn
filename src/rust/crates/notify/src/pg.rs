//! Postgres adapter for [`NotifyStore`] (store-pg migration `0001_v1_baseline.sql`).

use std::sync::Arc;

use app::ports::{
    PortError, RepoError, RepoResult, RewrapBatch, SecretCipher, SecretRows, SecretSlot,
};
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

/// Purpose of the encrypted channel config (addendum-1 2.2).
pub const CHANNEL_PURPOSE: &str = "channel_secret";
const CHANNEL_FIELD: &str = "config";

fn channel_slot(row: &str) -> SecretSlot<'_> {
    SecretSlot {
        purpose: CHANNEL_PURPOSE,
        row,
        field: CHANNEL_FIELD,
    }
}

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn re(e: sqlx::Error) -> RepoError {
    RepoError::Backend(format!("notifications db: {e}"))
}

/// Channel secrets are stored encrypted in `secret` (the whole config); `config` keeps the
/// redacted form, so a dump holds no tokens.
#[derive(Clone)]
pub struct PgNotifyStore {
    pool: PgPool,
    cipher: Arc<dyn SecretCipher>,
}

impl PgNotifyStore {
    #[must_use]
    pub fn new(pool: PgPool, cipher: Arc<dyn SecretCipher>) -> Self {
        Self { pool, cipher }
    }

    fn seal(&self, c: &Channel) -> StoreResult<(serde_json::Value, Vec<u8>)> {
        let full = serde_json::to_vec(&c.config).map_err(|e| PortError(e.to_string()))?;
        let row = c.id.to_string();
        Ok((
            to_json(&c.config.redacted())?,
            self.cipher.seal(channel_slot(&row), &full),
        ))
    }
}

struct ChannelRow {
    id: Uuid,
    user_id: Uuid,
    name: String,
    config: serde_json::Value,
    secret: Option<Vec<u8>>,
    enabled: bool,
    created_at: DateTime<Utc>,
}

impl ChannelRow {
    /// The full config from `secret`; a value that cannot be decrypted (`locked`) leaves the
    /// channel redacted and paused, so nothing is sent with placeholder secrets.
    fn into_channel(self, cipher: &dyn SecretCipher) -> StoreResult<Channel> {
        let row = self.id.to_string();
        let (config, enabled) = match &self.secret {
            Some(blob) => match cipher.open(channel_slot(&row), blob) {
                Ok(plain) => (
                    serde_json::from_slice(&plain)
                        .map_err(|e| PortError(format!("notifications db: {e}")))?,
                    self.enabled,
                ),
                Err(e) => {
                    tracing::warn!(channel = %self.id, version = ?e.version, "channel secret is locked");
                    (from_json(self.config)?, false)
                }
            },
            None => (from_json(self.config)?, self.enabled),
        };
        Ok(Channel {
            id: self.id,
            user: UserId(self.user_id),
            name: self.name,
            config,
            enabled,
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

    async fn user_locale(&self, user: UserId) -> StoreResult<Option<String>> {
        let l = sqlx::query_scalar!("SELECT locale FROM users WHERE id = $1", user.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?;
        Ok(l.flatten())
    }

    async fn channels(&self, user: UserId) -> StoreResult<Vec<Channel>> {
        sqlx::query_as!(
            ChannelRow,
            "SELECT id, user_id, name, config, secret, enabled, created_at FROM notify_channels
             WHERE user_id = $1 ORDER BY created_at, id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(|r| r.into_channel(self.cipher.as_ref()))
        .collect()
    }

    async fn channel(&self, id: Uuid) -> StoreResult<Option<Channel>> {
        sqlx::query_as!(
            ChannelRow,
            "SELECT id, user_id, name, config, secret, enabled, created_at FROM notify_channels
             WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(|r| r.into_channel(self.cipher.as_ref()))
        .transpose()
    }

    async fn put_channel(&self, c: &Channel) -> StoreResult<()> {
        let (redacted, secret) = self.seal(c)?;
        sqlx::query!(
            "INSERT INTO notify_channels (id, user_id, name, config, secret, enabled, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, config = EXCLUDED.config,
                 secret = EXCLUDED.secret, enabled = EXCLUDED.enabled",
            c.id,
            c.user.0,
            c.name,
            redacted,
            secret,
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

    async fn count_unread(&self, user: UserId) -> StoreResult<u32> {
        let n = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "n!" FROM notify_inbox WHERE user_id = $1 AND read_at IS NULL"#,
            user.0
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pe)?;
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
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

    async fn mark_all_read(&self, user: UserId, at: DateTime<Utc>) -> StoreResult<u32> {
        let r = sqlx::query!(
            "UPDATE notify_inbox SET read_at = $2 WHERE user_id = $1 AND read_at IS NULL",
            user.0,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(u32::try_from(r.rows_affected()).unwrap_or(u32::MAX))
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

    async fn latest_deliveries(&self, user: UserId) -> StoreResult<Vec<Delivery>> {
        sqlx::query_as!(
            DeliveryRow,
            "SELECT DISTINCT ON (channel_id) id, user_id, channel_id, event, dedupe_key, title,
                    body, link, state, attempts, last_error, due_at, created_at, sent_at
             FROM notify_deliveries WHERE user_id = $1 AND state <> 'suppressed'
             ORDER BY channel_id, created_at DESC, id DESC",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(DeliveryRow::into_delivery)
        .collect()
    }

    async fn claim_delivery(
        &self,
        id: Uuid,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
    ) -> StoreResult<Option<Delivery>> {
        // one statement: of two runs only one finds the row in a claimable state (OT-37)
        sqlx::query_as!(
            DeliveryRow,
            "UPDATE notify_deliveries SET state = 'sending', sent_at = $2
             WHERE id = $1
               AND (state = 'queued' OR (state = 'sending' AND (sent_at IS NULL OR sent_at < $3)))
             RETURNING id, user_id, channel_id, event, dedupe_key, title, body, link, state,
                    attempts, last_error, due_at, created_at, sent_at",
            id,
            now,
            stale_before
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(DeliveryRow::into_delivery)
        .transpose()
    }

    async fn claim_digest(
        &self,
        channel: Uuid,
        until: DateTime<Utc>,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
    ) -> StoreResult<Vec<Delivery>> {
        let mut v = sqlx::query_as!(
            DeliveryRow,
            "UPDATE notify_deliveries SET state = 'sending', sent_at = $3
             WHERE channel_id = $1 AND due_at <= $2
               AND (state = 'digest'
                    OR (state = 'sending' AND (sent_at IS NULL OR sent_at < $4)))
             RETURNING id, user_id, channel_id, event, dedupe_key, title, body, link, state,
                    attempts, last_error, due_at, created_at, sent_at",
            channel,
            until,
            now,
            stale_before
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(DeliveryRow::into_delivery)
        .collect::<StoreResult<Vec<_>>>()?;
        v.sort_by_key(|d| (d.created_at, d.id));
        Ok(v)
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

/// The channel secret column for the rotation job and the start-up migration.
#[async_trait]
impl SecretRows for PgNotifyStore {
    fn purpose(&self) -> &'static str {
        CHANNEL_PURPOSE
    }

    async fn seal_plaintext(&self, cipher: &dyn SecretCipher) -> RepoResult<u64> {
        let rows = sqlx::query!("SELECT id, config FROM notify_channels WHERE secret IS NULL")
            .fetch_all(&self.pool)
            .await
            .map_err(re)?;
        let mut n = 0;
        for r in rows {
            let config: crate::model::ChannelConfig = serde_json::from_value(r.config.clone())
                .map_err(|e| RepoError::Backend(format!("channel {}: {e}", r.id)))?;
            let row = r.id.to_string();
            let full =
                serde_json::to_vec(&config).map_err(|e| RepoError::Backend(e.to_string()))?;
            let redacted = serde_json::to_value(config.redacted())
                .map_err(|e| RepoError::Backend(e.to_string()))?;
            n += sqlx::query!(
                "UPDATE notify_channels SET secret = $2, config = $3 WHERE id = $1 AND secret IS NULL",
                r.id,
                cipher.seal(channel_slot(&row), &full),
                redacted
            )
            .execute(&self.pool)
            .await
            .map_err(re)?
            .rows_affected();
        }
        Ok(n)
    }

    async fn version_counts(&self) -> RepoResult<Vec<(u16, u64)>> {
        let rows = sqlx::query!(
            r#"SELECT (get_byte(secret, 1) * 256 + get_byte(secret, 2))::int AS "v!",
                      COUNT(*) AS "n!"
               FROM notify_channels WHERE secret IS NOT NULL AND length(secret) >= 3
               GROUP BY 1 ORDER BY 1"#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(re)?;
        Ok(rows
            .into_iter()
            .filter_map(|r| Some((u16::try_from(r.v).ok()?, u64::try_from(r.n).ok()?)))
            .collect())
    }

    async fn rewrap_batch(
        &self,
        cipher: &dyn SecretCipher,
        active: u16,
        after: Option<Uuid>,
        limit: u32,
    ) -> RepoResult<RewrapBatch> {
        let rows = sqlx::query!(
            r#"SELECT id, secret AS "blob!" FROM notify_channels
               WHERE secret IS NOT NULL
                 AND ($3::uuid IS NULL OR id > $3)
                 AND CASE WHEN length(secret) >= 3
                     THEN get_byte(secret, 1) * 256 + get_byte(secret, 2) <> $1
                     ELSE false END
               ORDER BY id LIMIT $2"#,
            i32::from(active),
            i64::from(limit),
            after
        )
        .fetch_all(&self.pool)
        .await
        .map_err(re)?;
        let last = rows.last().map(|r| r.id);
        let (mut done, mut locked) = (0, 0);
        for r in rows {
            let row = r.id.to_string();
            match cipher.rewrap(channel_slot(&row), &r.blob) {
                Ok(Some(new)) => {
                    done += sqlx::query!(
                        "UPDATE notify_channels SET secret = $3 WHERE id = $1 AND secret = $2",
                        r.id,
                        r.blob,
                        new
                    )
                    .execute(&self.pool)
                    .await
                    .map_err(re)?
                    .rows_affected();
                }
                Ok(None) => {}
                Err(_) => locked += 1,
            }
        }
        Ok(RewrapBatch {
            rewrapped: done,
            locked,
            last,
        })
    }
}
