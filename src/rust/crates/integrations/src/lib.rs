//! Inbound integrations (M14). v1 has one: a generic "watched" webhook in our own, documented
//! JSON format ([`WatchedEvent`]) that media servers and scripts post to with a
//! per-integration token. It records the play as a progress event (idempotent), adds the
//! item to the library and notifies the user. Jellyfin uses it through its webhook plugin
//! with a template the user writes (DEC-149); Radarr/Sonarr are not built (DEC-148).
#![deny(unsafe_code)]

#[cfg(feature = "pg")]
pub mod pg;
mod store;

use std::collections::BTreeMap;
use std::sync::Arc;

use app::ports::{Audience, Lane, NewJob, Notification, PortError, Subject};
use app::{App, AppError, NewCatalogItem};
use chrono::{DateTime, Utc};
use domain::catalog::ExternalId;
use domain::ids::{ItemId, UserId};
use domain::kind::KindId;
use domain::progress::Progress;
use domain::user::Actor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub use store::{IntegrationStore, MemIntegrationStore};

/// Prefix of integration tokens (tells them apart from session tokens in logs and configs).
pub const TOKEN_PREFIX: &str = "itk_";

/// What was watched (`"movie"` or `"episode"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WatchedKind {
    Movie,
    Episode,
}

/// The series of an episode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub title: String,
    #[serde(default)]
    pub year: Option<i32>,
    /// Provider ids of the series, e.g. `{"tmdb": "95396", "tvdb": "371980"}`.
    #[serde(default)]
    pub ids: BTreeMap<String, String>,
}

/// Body of `POST /api/v1/hooks/watched`: one finished movie or episode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchedEvent {
    pub kind: WatchedKind,
    /// Movie title, or the episode title (informational for episodes).
    pub title: String,
    #[serde(default)]
    pub year: Option<i32>,
    /// Provider ids of the movie, e.g. `{"tmdb": "949", "imdb": "tt0113277"}`; they decide
    /// which catalog item is meant.
    #[serde(default)]
    pub ids: BTreeMap<String, String>,
    /// Episodes only.
    #[serde(default)]
    pub series: Option<Series>,
    #[serde(default)]
    pub season: Option<u32>,
    #[serde(default)]
    pub episode: Option<u32>,
    /// When it was watched (RFC 3339); default: when the webhook arrives.
    #[serde(default)]
    pub watched_at: Option<DateTime<Utc>>,
    /// Shown in the notification, e.g. `Jellyfin`.
    #[serde(default)]
    pub source: Option<String>,
}

/// JSON Schema of [`WatchedEvent`] (published in the docs).
#[must_use]
pub fn watched_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(WatchedEvent)).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Integration {
    pub id: Uuid,
    pub user: UserId,
    pub name: String,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("unknown or revoked integration token")]
    Unauthorized,
    #[error("invalid event: {0}")]
    Invalid(String),
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    App(#[from] AppError),
    #[error("integration storage: {0}")]
    Port(#[from] PortError),
}

/// What a webhook call recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub item_id: ItemId,
    pub event_id: Uuid,
}

pub struct Integrations {
    app: App,
    store: Arc<dyn IntegrationStore>,
}

fn ids(map: &BTreeMap<String, String>) -> Vec<ExternalId> {
    map.iter()
        .filter(|(ns, v)| !ns.trim().is_empty() && !v.trim().is_empty())
        .map(|(ns, v)| ExternalId::new(ns.trim().to_ascii_lowercase(), v.trim()))
        .collect()
}

/// The catalog item and progress an event stands for.
struct Target {
    kind: &'static str,
    title: String,
    year: Option<i32>,
    external: Vec<ExternalId>,
    progress: Progress,
}

fn target(e: &WatchedEvent) -> Result<Target, IntegrationError> {
    let (kind, title, year, external, progress) = match e.kind {
        WatchedKind::Movie => (
            "movie",
            e.title.clone(),
            e.year,
            ids(&e.ids),
            Progress::Finished,
        ),
        WatchedKind::Episode => {
            let s = e
                .series
                .as_ref()
                .ok_or_else(|| IntegrationError::Invalid("episodes need `series`".into()))?;
            let (Some(season), Some(episode)) = (e.season, e.episode) else {
                return Err(IntegrationError::Invalid(
                    "episodes need `season` and `episode`".into(),
                ));
            };
            (
                "show",
                s.title.clone(),
                s.year,
                ids(&s.ids),
                Progress::Episode { season, episode },
            )
        }
    };
    if title.trim().is_empty() {
        return Err(IntegrationError::Invalid("title is empty".into()));
    }
    if external.is_empty() {
        return Err(IntegrationError::Invalid(
            "at least one provider id (tmdb, imdb, tvdb, ...) is needed to find the item".into(),
        ));
    }
    Ok(Target {
        kind,
        title,
        year,
        external,
        progress,
    })
}

impl Integrations {
    #[must_use]
    pub fn new(app: App, store: Arc<dyn IntegrationStore>) -> Self {
        Self { app, store }
    }

    /// Create an integration; returns it and its token (shown once, only the hash is kept).
    ///
    /// # Errors
    /// Blank or long name, storage errors.
    pub async fn create(
        &self,
        actor: Actor,
        name: &str,
    ) -> Result<(Integration, String), IntegrationError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(IntegrationError::Invalid(
                "name: 1 to 100 characters".into(),
            ));
        }
        let token = format!("{TOKEN_PREFIX}{}", self.app.ids.new_token());
        let i = Integration {
            id: self.app.ids.new_id(),
            user: actor.user_id,
            name: name.into(),
            token_hash: self.app.ids.hash_token(&token),
            created_at: self.app.clock.now(),
            last_seen_at: None,
            last_error: None,
        };
        self.store.insert(&i).await?;
        Ok((i, token))
    }

    /// # Errors
    /// Storage errors.
    pub async fn list(&self, actor: Actor) -> Result<Vec<Integration>, IntegrationError> {
        Ok(self.store.by_user(actor.user_id).await?)
    }

    /// Revoke an integration (its token stops working).
    ///
    /// # Errors
    /// Not the caller's integration, storage errors.
    pub async fn delete(&self, actor: Actor, id: Uuid) -> Result<(), IntegrationError> {
        let mine = self.store.by_user(actor.user_id).await?;
        if !mine.iter().any(|i| i.id == id) {
            return Err(IntegrationError::NotFound);
        }
        Ok(self.store.delete(id).await?)
    }

    /// Handle a webhook call: record the play for the integration's user. The same event
    /// (same item, episode and time) posted twice is recorded once.
    ///
    /// # Errors
    /// Unknown token, invalid event, app errors.
    pub async fn receive(
        &self,
        token: &str,
        e: &WatchedEvent,
    ) -> Result<Recorded, IntegrationError> {
        let integration = self
            .store
            .by_token_hash(&self.app.ids.hash_token(token))
            .await?
            .ok_or(IntegrationError::Unauthorized)?;
        let now = self.app.clock.now();
        let result = self.record(&integration, e).await;
        let error = result.as_ref().err().map(ToString::to_string);
        self.store.touch(integration.id, now, error).await?;
        result
    }

    async fn record(
        &self,
        integration: &Integration,
        e: &WatchedEvent,
    ) -> Result<Recorded, IntegrationError> {
        let user = self.app.user(integration.user).await?;
        let actor = Actor {
            user_id: user.id,
            role: user.role,
        };
        let Target {
            kind,
            title,
            year,
            external,
            progress,
        } = target(e)?;
        let item = self
            .app
            .ensure_catalog_item(NewCatalogItem {
                kind: KindId::new(kind).map_err(AppError::from)?,
                title: title.trim().into(),
                original_title: None,
                year,
                description: None,
                external_ids: external.clone(),
                attributes: serde_json::json!({}),
                cover_url: None,
            })
            .await?;
        if item.needs_refresh(self.app.clock.now()) {
            // details come later, from the connectors (the queue dedupes repeated plays)
            self.app
                .jobs
                .enqueue(NewJob {
                    lane: Lane::BackgroundRefresh,
                    kind: app::REFRESH_JOB.into(),
                    payload: serde_json::json!({ "item_id": item.id }),
                    dedupe_key: Some(format!("{}:{}", app::REFRESH_JOB, item.id)),
                    priority: 0,
                    run_at: None,
                })
                .await?;
        }
        let at = e.watched_at.unwrap_or_else(|| self.app.clock.now());
        // the same play posted again (retries, two senders) is one event
        let key = format!(
            "{kind}|{}|{:?}|{:?}|{}",
            external
                .iter()
                .map(|x| format!("{}:{}", x.namespace, x.value))
                .collect::<Vec<_>>()
                .join(","),
            e.season,
            e.episode,
            at.timestamp()
        );
        let op = Uuid::new_v5(&integration.id, key.as_bytes());
        let event = self
            .app
            .log_progress(actor, item.id, progress, Some(at), Some(op))
            .await?;
        let shown = match (e.kind, e.season, e.episode) {
            (WatchedKind::Episode, Some(s), Some(n)) => format!("{} {s}x{n:02}", item.title),
            _ => item.title.clone(),
        };
        self.app
            .notifications
            .emit(
                Notification::new(
                    "integration.play_recorded",
                    Audience::Users(vec![user.id]),
                    format!("play:{op}"),
                )
                .arg("title", shown)
                .arg(
                    "source",
                    e.source.clone().unwrap_or_else(|| integration.name.clone()),
                )
                .subject(Subject {
                    kind: Some(kind.into()),
                    item: Some(item.id),
                    ..Subject::default()
                })
                .link(format!("/items/{}", item.id.0)),
            )
            .await;
        Ok(Recorded {
            item_id: item.id,
            event_id: event.id.0,
        })
    }
}
