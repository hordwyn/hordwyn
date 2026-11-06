//! Event bus -> Server-Sent Events (`entity.updated`, `job.progress`, `import.progress`, ...).
//!
//! In one process (`--role=all`) events go through a tokio broadcast channel. With separate
//! web and worker processes, [`Bus::postgres`] publishes through `pg_notify` and every process
//! forwards notifications into its local channel, so a worker's `entity.updated` reaches the
//! browsers connected to any web process.
#![deny(unsafe_code)]

use std::convert::Infallible;
use std::time::Duration;

use app::ports::{AppEvent, EventBus};
use async_trait::async_trait;
use axum::response::sse::{Event, KeepAlive, Sse};
use domain::user::Actor;
use futures::stream::{Stream, StreamExt};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_util::sync::CancellationToken;

/// Postgres notification channel.
pub const PG_CHANNEL: &str = "app_events";
/// `pg_notify` payloads must stay below 8000 bytes; larger events lose their payload.
const MAX_NOTIFY_BYTES: usize = 7900;

#[derive(Clone)]
pub struct Bus {
    tx: broadcast::Sender<AppEvent>,
    pg: Option<PgPool>,
    /// Cancelled on shutdown: ends every SSE stream so graceful shutdown can finish.
    closing: CancellationToken,
}

impl Bus {
    /// In-process bus.
    #[must_use]
    pub fn local() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            tx,
            pg: None,
            closing: CancellationToken::new(),
        }
    }

    /// Cross-process bus over `LISTEN/NOTIFY`. Spawns the listener task.
    ///
    /// # Errors
    /// If the listener cannot connect.
    pub async fn postgres(pool: PgPool) -> Result<Self, sqlx::Error> {
        let bus = Self {
            pg: Some(pool.clone()),
            ..Self::local()
        };
        let mut listener = sqlx::postgres::PgListener::connect_with(&pool).await?;
        listener.listen(PG_CHANNEL).await?;
        let tx = bus.tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.recv().await {
                    Ok(n) => match serde_json::from_str::<AppEvent>(n.payload()) {
                        Ok(ev) => {
                            let _ = tx.send(ev);
                        }
                        Err(e) => tracing::warn!(error = %e, "undecodable event notification"),
                    },
                    Err(e) => {
                        // PgListener reconnects on the next recv; avoid a hot loop meanwhile.
                        tracing::warn!(error = %e, "event listener error");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });
        Ok(bus)
    }

    /// End all event streams (server shutdown). New subscriptions end immediately.
    pub fn shutdown(&self) {
        self.closing.cancel();
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }

    /// Events visible to `viewer`, as an SSE stream with keep-alives.
    pub fn sse(&self, viewer: Actor) -> Sse<impl Stream<Item = Result<Event, Infallible>> + use<>> {
        let closing = self.closing.clone().cancelled_owned();
        Sse::new(events_for(self.subscribe(), viewer).take_until(closing))
            .keep_alive(KeepAlive::default())
    }
}

/// May `viewer` see `event`? Admin topics go to admins only; an empty audience means every
/// signed-in user (catalog updates); otherwise only the listed users.
#[must_use]
pub fn visible_to(event: &AppEvent, viewer: &Actor) -> bool {
    if event.topic.starts_with("admin.") {
        return viewer.is_admin();
    }
    event.audience.is_empty() || event.audience.contains(&viewer.user_id)
}

fn to_sse(ev: &AppEvent) -> Event {
    Event::default()
        .event(ev.topic.clone())
        .data(serde_json::to_string(ev).unwrap_or_default())
}

/// Filter a receiver for one viewer. A lagging client gets `stream.lagged` and should refetch.
pub fn events_for(
    rx: broadcast::Receiver<AppEvent>,
    viewer: Actor,
) -> impl Stream<Item = Result<Event, Infallible>> {
    BroadcastStream::new(rx).filter_map(move |r| {
        let out = match r {
            Ok(ev) if visible_to(&ev, &viewer) => Some(Ok(to_sse(&ev))),
            Ok(_) => None,
            Err(BroadcastStreamRecvError::Lagged(n)) => Some(Ok(Event::default()
                .event("stream.lagged")
                .data(n.to_string()))),
        };
        std::future::ready(out)
    })
}

#[async_trait]
impl EventBus for Bus {
    async fn publish(&self, mut event: AppEvent) {
        if let Some(pool) = &self.pg {
            let mut text = serde_json::to_string(&event).unwrap_or_default();
            if text.len() > MAX_NOTIFY_BYTES {
                event.payload = serde_json::json!({ "truncated": true });
                text = serde_json::to_string(&event).unwrap_or_default();
            }
            if let Err(e) = sqlx::query("SELECT pg_notify($1, $2)")
                .bind(PG_CHANNEL)
                .bind(&text)
                .execute(pool)
                .await
            {
                tracing::warn!(error = %e, topic = %event.topic, "event notify failed");
            }
        } else {
            // No receivers is fine (nobody connected).
            let _ = self.tx.send(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::UserId;
    use domain::user::Role;

    fn actor(role: Role) -> Actor {
        Actor {
            user_id: UserId(uuid::Uuid::now_v7()),
            role,
        }
    }

    fn ev(topic: &str, audience: Vec<UserId>) -> AppEvent {
        AppEvent {
            topic: topic.into(),
            entity: None,
            audience,
            payload: serde_json::Value::Null,
        }
    }

    #[test]
    fn audience_rules() {
        let me = actor(Role::Member);
        let admin = actor(Role::Admin);
        assert!(visible_to(&ev("entity.updated", vec![]), &me));
        assert!(visible_to(&ev("entity.updated", vec![me.user_id]), &me));
        assert!(!visible_to(&ev("entity.updated", vec![admin.user_id]), &me));
        assert!(!visible_to(&ev("admin.storage.hard_cap", vec![]), &me));
        assert!(visible_to(&ev("admin.storage.hard_cap", vec![]), &admin));
    }

    #[tokio::test]
    async fn stream_only_yields_visible_events() {
        let bus = Bus::local();
        let me = actor(Role::Member);
        let other = actor(Role::Member);
        let mut s = Box::pin(events_for(bus.subscribe(), me));
        bus.publish(ev("entity.updated", vec![other.user_id])).await;
        bus.publish(ev("job.progress", vec![me.user_id])).await;
        let first = tokio::time::timeout(Duration::from_secs(1), s.next())
            .await
            .unwrap()
            .unwrap();
        assert!(first.is_ok());
        // only one visible event was published, so nothing else arrives
        assert!(
            tokio::time::timeout(Duration::from_millis(100), s.next())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn shutdown_ends_streams() {
        let bus = Bus::local();
        let s = bus.sse(actor(Role::Member));
        bus.shutdown();
        let resp = axum::response::IntoResponse::into_response(s);
        let body = axum::body::to_bytes(resp.into_body(), 1 << 20);
        let out = tokio::time::timeout(Duration::from_secs(2), body).await;
        assert!(out.is_ok(), "stream did not end after shutdown");
    }

    #[tokio::test]
    async fn postgres_bus_delivers_across_instances() {
        let Some(pool) = store_pg::testing::fresh_db().await else {
            return;
        };
        let a = Bus::postgres(pool.clone()).await.unwrap();
        let b = Bus::postgres(pool).await.unwrap();
        let mut rx = b.subscribe();
        a.publish(ev("entity.updated", vec![])).await;
        let got = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.topic, "entity.updated");
    }
}
