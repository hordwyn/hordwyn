//! Turns operational events on the bus into admin notifications: the storage budget's
//! `admin.storage.high_watermark` / `admin.storage.hard_cap` become `admin.storage_high` /
//! `admin.storage_full` (at most one per day each). Every event is still forwarded.

use std::sync::Arc;

use app::ports::{AppEvent, Audience, Clock, EventBus, Notification, Notifications};
use async_trait::async_trait;

pub struct AdminBridge {
    inner: Arc<dyn EventBus>,
    notifications: Arc<dyn Notifications>,
    clock: Arc<dyn Clock>,
}

impl AdminBridge {
    #[must_use]
    pub fn new(
        inner: Arc<dyn EventBus>,
        notifications: Arc<dyn Notifications>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            inner,
            notifications,
            clock,
        }
    }
}

/// `1536` -> `1.5 KiB`.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[async_trait]
impl EventBus for AdminBridge {
    async fn publish(&self, event: AppEvent) {
        let mapped = match event.topic.as_str() {
            "admin.storage.high_watermark" => Some(("admin.storage_high", "high")),
            "admin.storage.hard_cap" => Some(("admin.storage_full", "cap")),
            _ => None,
        };
        if let Some((id, cap_field)) = mapped {
            let n = |f: &str| event.payload[f].as_u64().unwrap_or(0);
            let day = self.clock.now().format("%Y-%m-%d");
            self.notifications
                .emit(
                    Notification::new(id, Audience::Admins, format!("{id}:{day}"))
                        .arg("used", bytes(n("used")))
                        .arg("cap", bytes(n(cap_field))),
                )
                .await;
        }
        self.inner.publish(event).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_sizes() {
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KiB");
        assert_eq!(bytes(5 * 1024 * 1024 * 1024), "5.0 GiB");
    }

    #[tokio::test]
    async fn storage_events_become_admin_notifications_and_are_forwarded() {
        let bus = Arc::new(testkit::RecordingEvents::default());
        let rec = Arc::new(testkit::RecordingNotifications::default());
        let b = AdminBridge::new(
            bus.clone(),
            rec.clone(),
            Arc::new(testkit::FixedClock::default()),
        );
        b.publish(AppEvent {
            topic: "admin.storage.hard_cap".into(),
            entity: None,
            audience: vec![],
            payload: serde_json::json!({ "used": 2048, "cap": 2048 }),
        })
        .await;
        b.publish(AppEvent::entity_updated("list", uuid::Uuid::nil(), vec![]))
            .await;
        assert_eq!(bus.all().len(), 2);
        let n = rec.all();
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].event, "admin.storage_full");
        assert_eq!(n[0].dedupe_key, "admin.storage_full:2026-01-01");
        assert_eq!(n[0].args["used"], "2.0 KiB");
    }
}
