//! Storage port of the notification engine, with an in-memory adapter for tests.

use std::collections::HashMap;
use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use uuid::Uuid;

use crate::model::{Channel, Delivery, DeliveryState, InboxItem, Subscription};
use crate::template::Overrides;

pub type StoreResult<T> = Result<T, PortError>;

#[async_trait]
pub trait NotifyStore: Send + Sync {
    async fn admins(&self) -> StoreResult<Vec<UserId>>;
    /// The language the user chose (`None`: no choice, DEC-201).
    async fn user_locale(&self, user: UserId) -> StoreResult<Option<String>>;

    async fn channels(&self, user: UserId) -> StoreResult<Vec<Channel>>;
    async fn channel(&self, id: Uuid) -> StoreResult<Option<Channel>>;
    async fn put_channel(&self, c: &Channel) -> StoreResult<()>;
    async fn delete_channel(&self, id: Uuid) -> StoreResult<()>;

    /// The user's own subscriptions.
    async fn subscriptions(&self, user: UserId) -> StoreResult<Vec<Subscription>>;
    /// Instance defaults (`user` is `None`).
    async fn defaults(&self) -> StoreResult<Vec<Subscription>>;
    async fn subscription(&self, id: Uuid) -> StoreResult<Option<Subscription>>;
    async fn put_subscription(&self, s: &Subscription) -> StoreResult<()>;
    async fn delete_subscription(&self, id: Uuid) -> StoreResult<()>;

    /// `false` if the user already has this event with this dedupe key.
    async fn insert_inbox(&self, i: &InboxItem) -> StoreResult<bool>;
    async fn inbox(&self, user: UserId, limit: u32) -> StoreResult<Vec<InboxItem>>;
    /// Unread inbox items of the user (the bell count, design.md 3.2).
    async fn count_unread(&self, user: UserId) -> StoreResult<u32>;
    async fn mark_read(&self, user: UserId, id: Uuid, at: DateTime<Utc>) -> StoreResult<()>;
    /// Mark every unread inbox row of the user read ("Mark all read"); how many were unread.
    async fn mark_all_read(&self, user: UserId, at: DateTime<Utc>) -> StoreResult<u32>;

    /// `false` if this channel already has this event with this dedupe key.
    async fn insert_delivery(&self, d: &Delivery) -> StoreResult<bool>;
    async fn delivery(&self, id: Uuid) -> StoreResult<Option<Delivery>>;
    async fn update_delivery(&self, d: &Delivery) -> StoreResult<()>;
    /// Newest first.
    async fn deliveries(&self, user: UserId, limit: u32) -> StoreResult<Vec<Delivery>>;
    /// The latest delivery of each of the user's channels that was not suppressed (channel
    /// health, design.md 6.8), in no particular order.
    async fn latest_deliveries(&self, user: UserId) -> StoreResult<Vec<Delivery>>;
    /// Digest entries of a channel due at or before `until`, oldest first.
    /// Claim a queued delivery for sending (OT-37, #6): in one step `queued` (or a `sending`
    /// whose claim at `sent_at` is older than `stale_before`) becomes `sending` with
    /// `sent_at` = `now`. `None` when there is nothing to claim (gone, done, or another run
    /// holds it), so two runs never both send it.
    async fn claim_delivery(
        &self,
        id: Uuid,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
    ) -> StoreResult<Option<Delivery>>;
    /// Claim what a channel collected for its digest until `until` (oldest first), as
    /// [`Self::claim_delivery`] does for one delivery: `digest` rows due by then (and stale
    /// `sending` ones) become `sending`.
    async fn claim_digest(
        &self,
        channel: Uuid,
        until: DateTime<Utc>,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
    ) -> StoreResult<Vec<Delivery>>;
    /// Deliveries of an event to a channel created since `since` that were not suppressed.
    async fn count_since(
        &self,
        channel: Uuid,
        event: &str,
        since: DateTime<Utc>,
    ) -> StoreResult<u32>;

    /// Admin template overrides (`locale -> key -> text`).
    async fn template_overrides(&self) -> StoreResult<Overrides>;
    async fn set_template_override(
        &self,
        locale: &str,
        key: &str,
        text: Option<&str>,
    ) -> StoreResult<()>;
}

#[derive(Default)]
struct Mem {
    admins: Vec<UserId>,
    locales: HashMap<UserId, String>,
    channels: HashMap<Uuid, Channel>,
    subscriptions: HashMap<Uuid, Subscription>,
    inbox: Vec<InboxItem>,
    deliveries: Vec<Delivery>,
    overrides: Overrides,
}

#[derive(Default)]
pub struct MemNotifyStore(Mutex<Mem>);

impl MemNotifyStore {
    fn m(&self) -> std::sync::MutexGuard<'_, Mem> {
        self.0.lock().expect("lock")
    }

    pub fn add_admin(&self, u: UserId) {
        self.m().admins.push(u);
    }

    pub fn set_locale(&self, u: UserId, locale: &str) {
        self.m().locales.insert(u, locale.to_string());
    }
}

#[async_trait]
impl NotifyStore for MemNotifyStore {
    async fn admins(&self) -> StoreResult<Vec<UserId>> {
        Ok(self.m().admins.clone())
    }

    async fn user_locale(&self, user: UserId) -> StoreResult<Option<String>> {
        Ok(self.m().locales.get(&user).cloned())
    }

    async fn channels(&self, user: UserId) -> StoreResult<Vec<Channel>> {
        let mut v: Vec<_> = self
            .m()
            .channels
            .values()
            .filter(|c| c.user == user)
            .cloned()
            .collect();
        v.sort_by_key(|c| c.created_at);
        Ok(v)
    }

    async fn channel(&self, id: Uuid) -> StoreResult<Option<Channel>> {
        Ok(self.m().channels.get(&id).cloned())
    }

    async fn put_channel(&self, c: &Channel) -> StoreResult<()> {
        self.m().channels.insert(c.id, c.clone());
        Ok(())
    }

    async fn delete_channel(&self, id: Uuid) -> StoreResult<()> {
        self.m().channels.remove(&id);
        Ok(())
    }

    async fn subscriptions(&self, user: UserId) -> StoreResult<Vec<Subscription>> {
        Ok(self
            .m()
            .subscriptions
            .values()
            .filter(|s| s.user == Some(user))
            .cloned()
            .collect())
    }

    async fn defaults(&self) -> StoreResult<Vec<Subscription>> {
        Ok(self
            .m()
            .subscriptions
            .values()
            .filter(|s| s.user.is_none())
            .cloned()
            .collect())
    }

    async fn subscription(&self, id: Uuid) -> StoreResult<Option<Subscription>> {
        Ok(self.m().subscriptions.get(&id).cloned())
    }

    async fn put_subscription(&self, s: &Subscription) -> StoreResult<()> {
        self.m().subscriptions.insert(s.id, s.clone());
        Ok(())
    }

    async fn delete_subscription(&self, id: Uuid) -> StoreResult<()> {
        self.m().subscriptions.remove(&id);
        Ok(())
    }

    async fn insert_inbox(&self, i: &InboxItem) -> StoreResult<bool> {
        let mut m = self.m();
        if m.inbox
            .iter()
            .any(|x| x.user == i.user && x.event == i.event && x.dedupe_key == i.dedupe_key)
        {
            return Ok(false);
        }
        m.inbox.push(i.clone());
        Ok(true)
    }

    async fn inbox(&self, user: UserId, limit: u32) -> StoreResult<Vec<InboxItem>> {
        let mut v: Vec<_> = self
            .m()
            .inbox
            .iter()
            .filter(|i| i.user == user)
            .cloned()
            .collect();
        v.sort_by_key(|i| std::cmp::Reverse((i.created_at, i.id)));
        v.truncate(limit as usize);
        Ok(v)
    }

    async fn latest_deliveries(&self, user: UserId) -> StoreResult<Vec<Delivery>> {
        let mut latest: HashMap<Uuid, Delivery> = HashMap::new();
        for d in self
            .m()
            .deliveries
            .iter()
            .filter(|d| d.user == user && d.state != DeliveryState::Suppressed)
        {
            let newer = latest
                .get(&d.channel)
                .is_none_or(|x| (x.created_at, x.id) < (d.created_at, d.id));
            if newer {
                latest.insert(d.channel, d.clone());
            }
        }
        Ok(latest.into_values().collect())
    }

    async fn count_unread(&self, user: UserId) -> StoreResult<u32> {
        let n = self
            .m()
            .inbox
            .iter()
            .filter(|i| i.user == user && i.read_at.is_none())
            .count();
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    async fn mark_read(&self, user: UserId, id: Uuid, at: DateTime<Utc>) -> StoreResult<()> {
        for i in &mut self.m().inbox {
            if i.id == id && i.user == user && i.read_at.is_none() {
                i.read_at = Some(at);
            }
        }
        Ok(())
    }

    async fn mark_all_read(&self, user: UserId, at: DateTime<Utc>) -> StoreResult<u32> {
        let mut n = 0_u32;
        for i in &mut self.m().inbox {
            if i.user == user && i.read_at.is_none() {
                i.read_at = Some(at);
                n = n.saturating_add(1);
            }
        }
        Ok(n)
    }

    async fn insert_delivery(&self, d: &Delivery) -> StoreResult<bool> {
        let mut m = self.m();
        if m.deliveries
            .iter()
            .any(|x| x.channel == d.channel && x.event == d.event && x.dedupe_key == d.dedupe_key)
        {
            return Ok(false);
        }
        m.deliveries.push(d.clone());
        Ok(true)
    }

    async fn delivery(&self, id: Uuid) -> StoreResult<Option<Delivery>> {
        Ok(self.m().deliveries.iter().find(|d| d.id == id).cloned())
    }

    async fn update_delivery(&self, d: &Delivery) -> StoreResult<()> {
        if let Some(x) = self.m().deliveries.iter_mut().find(|x| x.id == d.id) {
            *x = d.clone();
        }
        Ok(())
    }

    async fn deliveries(&self, user: UserId, limit: u32) -> StoreResult<Vec<Delivery>> {
        let mut v: Vec<_> = self
            .m()
            .deliveries
            .iter()
            .filter(|d| d.user == user)
            .cloned()
            .collect();
        v.sort_by_key(|d| std::cmp::Reverse((d.created_at, d.id)));
        v.truncate(limit as usize);
        Ok(v)
    }

    async fn claim_delivery(
        &self,
        id: Uuid,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
    ) -> StoreResult<Option<Delivery>> {
        let mut m = self.m();
        let Some(d) = m.deliveries.iter_mut().find(|d| d.id == id) else {
            return Ok(None);
        };
        let stale = d.state == DeliveryState::Sending && d.sent_at.is_none_or(|t| t < stale_before);
        if d.state != DeliveryState::Queued && !stale {
            return Ok(None);
        }
        d.state = DeliveryState::Sending;
        d.sent_at = Some(now);
        Ok(Some(d.clone()))
    }

    async fn claim_digest(
        &self,
        channel: Uuid,
        until: DateTime<Utc>,
        now: DateTime<Utc>,
        stale_before: DateTime<Utc>,
    ) -> StoreResult<Vec<Delivery>> {
        let mut m = self.m();
        let mut v = Vec::new();
        for d in m
            .deliveries
            .iter_mut()
            .filter(|d| d.channel == channel && d.due_at <= until)
        {
            let stale =
                d.state == DeliveryState::Sending && d.sent_at.is_none_or(|t| t < stale_before);
            if d.state == DeliveryState::Digest || stale {
                d.state = DeliveryState::Sending;
                d.sent_at = Some(now);
                v.push(d.clone());
            }
        }
        v.sort_by_key(|d| (d.created_at, d.id));
        Ok(v)
    }

    async fn count_since(
        &self,
        channel: Uuid,
        event: &str,
        since: DateTime<Utc>,
    ) -> StoreResult<u32> {
        let n = self
            .m()
            .deliveries
            .iter()
            .filter(|d| {
                d.channel == channel
                    && d.event == event
                    && d.created_at >= since
                    && d.state != DeliveryState::Suppressed
            })
            .count();
        Ok(u32::try_from(n).unwrap_or(u32::MAX))
    }

    async fn template_overrides(&self) -> StoreResult<Overrides> {
        Ok(self.m().overrides.clone())
    }

    async fn set_template_override(
        &self,
        locale: &str,
        key: &str,
        text: Option<&str>,
    ) -> StoreResult<()> {
        let mut m = self.m();
        let l = m.overrides.entry(locale.into()).or_default();
        match text {
            Some(t) => {
                l.insert(key.into(), t.into());
            }
            None => {
                l.remove(key);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delivery(channel: Uuid, n: i64, state: DeliveryState) -> Delivery {
        Delivery {
            id: Uuid::from_u128(u128::try_from(n).unwrap()),
            user: UserId(Uuid::nil()),
            channel,
            event: "admin.signup".into(),
            dedupe_key: n.to_string(),
            title: String::new(),
            body: String::new(),
            link: None,
            state,
            attempts: 1,
            last_error: None,
            due_at: DateTime::from_timestamp(n, 0).unwrap(),
            created_at: DateTime::from_timestamp(n, 0).unwrap(),
            sent_at: None,
        }
    }

    #[tokio::test]
    async fn latest_delivery_per_channel_skips_suppressed_and_mark_all_read_counts() {
        let s = MemNotifyStore::default();
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        for d in [
            delivery(a, 1, DeliveryState::Sent),
            delivery(a, 2, DeliveryState::Failed),
            delivery(a, 3, DeliveryState::Suppressed),
            delivery(b, 4, DeliveryState::Sent),
        ] {
            s.insert_delivery(&d).await.unwrap();
        }
        let mut latest = s.latest_deliveries(UserId(Uuid::nil())).await.unwrap();
        latest.sort_by_key(|d| d.created_at);
        let states: Vec<_> = latest.iter().map(|d| (d.channel, d.state)).collect();
        assert_eq!(
            states,
            vec![(a, DeliveryState::Failed), (b, DeliveryState::Sent)]
        );

        let user = UserId(Uuid::from_u128(9));
        for n in 0..3_i64 {
            s.insert_inbox(&InboxItem {
                id: Uuid::from_u128(100 + u128::try_from(n).unwrap()),
                user,
                event: "admin.signup".into(),
                dedupe_key: n.to_string(),
                title: String::new(),
                body: String::new(),
                link: None,
                created_at: DateTime::from_timestamp(n, 0).unwrap(),
                read_at: None,
            })
            .await
            .unwrap();
        }
        let now = DateTime::from_timestamp(10, 0).unwrap();
        s.mark_read(user, Uuid::from_u128(100), now).await.unwrap();
        assert_eq!(s.mark_all_read(user, now).await.unwrap(), 2);
        assert_eq!(s.count_unread(user).await.unwrap(), 0);
        assert_eq!(s.mark_all_read(user, now).await.unwrap(), 0);
    }
}
