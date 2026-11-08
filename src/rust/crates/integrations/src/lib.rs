//! Inbound integrations (M14). v1 has one: a generic "watched" webhook in our own, documented
//! JSON format ([`WatchedEvent`]) that media servers and scripts post to with a
//! per-integration token. It records the play as a progress event (idempotent), adds the
//! item to the library and notifies the user. Jellyfin uses it through its webhook plugin
//! with a template the user writes (DEC-149); Radarr/Sonarr are not built (DEC-148).
#![deny(unsafe_code)]

mod call;
#[cfg(feature = "pg")]
pub mod pg;
mod store;

use std::collections::BTreeMap;
use std::sync::Arc;

use app::ports::{AppEvent, Audience, Lane, NewJob, Notification, PortError, Subject};
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

pub use call::{MAX_PAYLOAD, redact, remembered};
pub use store::{IntegrationStore, MemIntegrationStore};

/// SSE topic of every call attributed to an integration (accepted, refused or a test), for
/// the open "Connect a sender" panel and the cards (design.md 5.9).
pub const CALL_TOPIC: &str = "integration.call";

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
    /// The sender's own id for this play, if it has one: repeated posts with the same id are
    /// one play. Without it, plays are told apart by `watched_at`, or (if that is missing
    /// too) by the arrival time rounded down to 15 minutes.
    #[serde(default)]
    pub id: Option<String>,
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
    /// HTTP status of the last attributed call (200, 422, ...).
    pub last_status: Option<u16>,
    /// The offending field of the last refused call.
    pub last_error_field: Option<String>,
    /// The body of the last call, redacted, at most [`MAX_PAYLOAD`] bytes.
    pub last_payload: Option<String>,
    /// What the last accepted call recorded (kept when later calls are refused).
    pub last_item: Option<LastItem>,
}

/// An item a call recorded, as the card shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastItem {
    pub id: ItemId,
    /// The title, for episodes with `2x04`.
    pub title: String,
}

/// What one attributed call leaves on its integration ([`IntegrationStore::touch`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallRecord {
    pub at: DateTime<Utc>,
    pub status: u16,
    /// `None` clears the last error.
    pub error: Option<String>,
    pub field: Option<String>,
    pub payload: Option<String>,
    /// `None` keeps the item of an earlier accepted call.
    pub item: Option<LastItem>,
}

/// The answer of a test call: nothing was recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestCall {
    /// The catalog item the sample's ids point to, if it is already known.
    pub matched: Option<LastItem>,
    /// The title the sample names, as shown.
    pub title: String,
}

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("unknown or revoked integration token")]
    Unauthorized,
    #[error("invalid event: {0}")]
    Invalid(String),
    /// A webhook body that is not a valid [`WatchedEvent`]; `field` names the offending
    /// field when it can be told (R-38).
    #[error("{message}")]
    Refused {
        field: Option<String>,
        message: String,
    },
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
    /// The item as shown (`Heat`, `Northern Lights Station 2x04`).
    pub title: String,
}

/// The title of what an event stands for, with `2x04` for episodes.
fn shown(e: &WatchedEvent, title: &str) -> String {
    match (e.kind, e.season, e.episode) {
        (WatchedKind::Episode, Some(s), Some(n)) => format!("{title} {s}x{n:02}"),
        _ => title.to_string(),
    }
}

/// The HTTP status a failed call is answered with (see `api::hooks`).
fn status_of(e: &IntegrationError) -> u16 {
    match e {
        IntegrationError::Unauthorized => 401,
        IntegrationError::NotFound => 404,
        IntegrationError::Invalid(_)
        | IntegrationError::Refused { .. }
        | IntegrationError::App(AppError::Domain(_)) => 422,
        IntegrationError::App(_) | IntegrationError::Port(_) => 500,
    }
}

fn field_of(e: &IntegrationError) -> Option<String> {
    match e {
        IntegrationError::Refused { field, .. } => field.clone(),
        _ => None,
    }
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

fn rule(field: &str, message: &str) -> IntegrationError {
    IntegrationError::Refused {
        field: Some(field.into()),
        message: message.into(),
    }
}

fn target(e: &WatchedEvent) -> Result<Target, IntegrationError> {
    let (kind, title, year, external, progress, at) = match e.kind {
        WatchedKind::Movie => (
            "movie",
            e.title.clone(),
            e.year,
            ids(&e.ids),
            Progress::Finished,
            "",
        ),
        WatchedKind::Episode => {
            let s = e
                .series
                .as_ref()
                .ok_or_else(|| rule("series", "episodes need `series`"))?;
            let Some(season) = e.season else {
                return Err(rule("season", "episodes need `season` and `episode`"));
            };
            let Some(episode) = e.episode else {
                return Err(rule("episode", "episodes need `season` and `episode`"));
            };
            (
                "show",
                s.title.clone(),
                s.year,
                ids(&s.ids),
                Progress::Episode { season, episode },
                "series.",
            )
        }
    };
    if title.trim().is_empty() {
        return Err(rule(&format!("{at}title"), "title is empty"));
    }
    if external.is_empty() {
        return Err(rule(
            &format!("{at}ids"),
            "at least one provider id (tmdb, imdb, tvdb, ...) is needed to find the item",
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
            last_status: None,
            last_error_field: None,
            last_payload: None,
            last_item: None,
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
        let integration = self.by_token(token).await?;
        let payload = serde_json::to_value(e).ok().map(|v| remembered(&v));
        self.attributed(&integration, Ok(e.clone()), payload).await
    }

    /// [`Self::receive`] for the raw body of `POST /hooks/watched`: the token is checked
    /// first, so a body that is not a valid event is refused *for this integration*, which
    /// keeps the offending field and the redacted body for its card (R-38).
    ///
    /// # Errors
    /// Unknown token, a refused body, app errors.
    pub async fn receive_body(
        &self,
        token: &str,
        body: &[u8],
    ) -> Result<Recorded, IntegrationError> {
        let integration = self.by_token(token).await?;
        let (event, payload) = call::parse(body);
        self.attributed(&integration, event, payload).await
    }

    async fn by_token(&self, token: &str) -> Result<Integration, IntegrationError> {
        self.store
            .by_token_hash(&self.app.ids.hash_token(token))
            .await?
            .ok_or(IntegrationError::Unauthorized)
    }

    /// Record a call of `integration`: the play, what the card remembers, and the event for
    /// the user's open pages.
    async fn attributed(
        &self,
        integration: &Integration,
        event: Result<WatchedEvent, IntegrationError>,
        payload: Option<String>,
    ) -> Result<Recorded, IntegrationError> {
        let at = self.app.clock.now();
        let result = match event {
            Ok(e) => self.record(integration, &e).await,
            Err(e) => Err(e),
        };
        let record = CallRecord {
            at,
            status: result.as_ref().map_or_else(status_of, |_| 200),
            error: result.as_ref().err().map(ToString::to_string),
            field: result.as_ref().err().and_then(field_of),
            payload,
            item: result.as_ref().ok().map(|r| LastItem {
                id: r.item_id,
                title: r.title.clone(),
            }),
        };
        self.store.touch(integration.id, &record).await?;
        self.announce(integration, &record, false).await;
        result
    }

    /// Publish [`CALL_TOPIC`] to the integration's user.
    async fn announce(&self, integration: &Integration, r: &CallRecord, test: bool) {
        self.app
            .events
            .publish(AppEvent {
                topic: CALL_TOPIC.into(),
                entity: None,
                audience: vec![integration.user],
                payload: serde_json::json!({
                    "integration_id": integration.id,
                    "status": r.status,
                    "field": r.field,
                    "error": r.error,
                    "test": test,
                    "item_id": r.item.as_ref().map(|i| i.id.0),
                    "title": r.item.as_ref().map(|i| i.title.clone()),
                    "at": r.at.to_rfc3339(),
                }),
            })
            .await;
    }

    /// "Send a test call from here" (design.md 5.9): validate a sample body and find the
    /// catalog item its ids point to, as a real call would, but record nothing (no play, no
    /// change to the card). The user's open pages hear about it, marked as a test.
    ///
    /// # Errors
    /// Not the caller's integration, a refused sample, storage errors.
    pub async fn test_call(
        &self,
        actor: Actor,
        id: Uuid,
        body: &[u8],
    ) -> Result<TestCall, IntegrationError> {
        let integration = self
            .store
            .by_user(actor.user_id)
            .await?
            .into_iter()
            .find(|i| i.id == id)
            .ok_or(IntegrationError::NotFound)?;
        let at = self.app.clock.now();
        let checked = match call::parse(body).0 {
            Ok(e) => self.matched(&e).await,
            Err(e) => Err(e),
        };
        let record = CallRecord {
            at,
            status: checked.as_ref().map_or_else(status_of, |_| 200),
            error: checked.as_ref().err().map(ToString::to_string),
            field: checked.as_ref().err().and_then(field_of),
            payload: None,
            item: checked.as_ref().ok().and_then(|t| t.matched.clone()),
        };
        self.announce(&integration, &record, true).await;
        checked
    }

    async fn matched(&self, e: &WatchedEvent) -> Result<TestCall, IntegrationError> {
        let t = target(e)?;
        let kind = KindId::new(t.kind).map_err(AppError::from)?;
        let mut found = None;
        for ext in &t.external {
            if let Some(item) = self
                .app
                .repos
                .catalog
                .by_external_id(&kind, ext)
                .await
                .map_err(AppError::from)?
            {
                found = Some(item);
                break;
            }
        }
        Ok(TestCall {
            matched: found.map(|item| LastItem {
                id: item.id,
                title: shown(e, &item.title),
            }),
            title: shown(e, t.title.trim()),
        })
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
            .find_or_create_catalog_item(NewCatalogItem {
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
        let now = self.app.clock.now();
        let at = e.watched_at.unwrap_or(now);
        // the same play posted again (retries, two senders) is one event (G2-8)
        let when = match (&e.id, e.watched_at) {
            (Some(id), _) => format!("id:{}", id.trim()),
            (None, Some(t)) => t.timestamp().to_string(),
            (None, None) => format!("~{}", now.timestamp() / 900),
        };
        let key = format!(
            "{kind}|{}|{:?}|{:?}|{}",
            external
                .iter()
                .map(|x| format!("{}:{}", x.namespace, x.value))
                .collect::<Vec<_>>()
                .join(","),
            e.season,
            e.episode,
            when
        );
        let op = Uuid::new_v5(&integration.id, key.as_bytes());
        let event = self
            .app
            .log_progress(actor, item.id, progress, Some(at), Some(op))
            .await?;
        let label = shown(e, &item.title);
        self.app
            .notifications
            .emit(
                Notification::new(
                    "integration.play_recorded",
                    Audience::Users(vec![user.id]),
                    format!("play:{op}"),
                )
                .arg("title", label.clone())
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
            title: label,
        })
    }
}
