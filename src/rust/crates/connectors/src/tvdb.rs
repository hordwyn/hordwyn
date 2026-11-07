//! `TheTVDB` (shows), API v4. Terms and attribution: `docs/connectors/tvdb.md`.
//!
//! Auth: `POST /v4/login` with the API key (and optional subscriber PIN) returns a bearer
//! token valid for one month; it is cached per PIN and renewed after 25 days. The PIN is the
//! requesting member's own (`Ctx::user_key`, the user-supported model of design.md 4.1)
//! when they stored one, else the one in the instance key (`<apikey>:<pin>`), if any.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, CredentialSpec, Ctx,
    Details, DetailsRequest, ImageKind, ImageRef, KeySlot, RateLimit, Search, SearchRequest,
    Snapshot,
};
use domain::catalog::ExternalId;
use kinds::builtin::{SeasonInfo, ShowAttributes};
use serde::Deserialize;

use crate::util::{date_of, get_json, non_empty, post_json, year_of};

pub const ID: &str = "tvdb";
/// `<apikey>` or `<apikey>:<pin>`.
pub const API_KEY_ENV: &str = "KK_TVDB_API_KEY";
/// Credential fields (stored as JSON, joined with `:` into the key string).
pub const CREDENTIALS: CredentialSpec = CredentialSpec {
    instance_fields: &["api_key"],
    user_fields: &["pin"],
};
const DEFAULT_API_BASE: &str = "https://api4.thetvdb.com/v4";
const IMAGE_HOST: &str = "artworks.thetvdb.com";
const KIND: &str = "show";

pub struct Tvdb {
    info: ConnectorInfo,
    api_base: String,
    key: KeySlot,
    /// Bearer tokens per PIN (`""`: none): token, renew after, key version.
    tokens: Mutex<HashMap<String, (String, Instant, u64)>>,
}

fn host_of(base: &str) -> String {
    url::Url::parse(base)
        .ok()
        .and_then(|u| {
            let host = u.host_str()?.to_string();
            Some(u.port().map_or(host.clone(), |p| format!("{host}:{p}")))
        })
        .unwrap_or_default()
}

impl Tvdb {
    #[must_use]
    pub fn new(key: Option<String>) -> Self {
        Self::with_base_url(key, DEFAULT_API_BASE)
    }

    #[must_use]
    pub fn with_base_url(key: Option<String>, api_base: &str) -> Self {
        let api_base = api_base.trim_end_matches('/').to_string();
        Self {
            info: ConnectorInfo {
                id: ID.into(),
                name: "TheTVDB".into(),
                kinds: vec![KIND.into()],
                capabilities: vec![Capability::Search, Capability::Details, Capability::Images],
                rate_limit: RateLimit {
                    per_second: 5,
                    burst: 5,
                    max_concurrency: 4,
                },
                auth: Auth::Bearer {
                    env: API_KEY_ENV.into(),
                },
                attribution: Attribution {
                    text: "Metadata provided by TheTVDB. Please consider adding missing information or subscribing.".into(),
                    url: "https://thetvdb.com/".into(),
                    licence: "TheTVDB API terms (key per product; attribution with a link required)".into(),
                },
                hosts: vec![host_of(&api_base), IMAGE_HOST.into()],
                cache_ttl_secs: 7 * 24 * 3600,
            },
            api_base,
            key: KeySlot::new(key),
            tokens: Mutex::new(HashMap::new()),
        }
    }

    async fn headers(&self, ctx: &Ctx) -> Result<Vec<(String, String)>, ConnectorError> {
        let (apikey, instance_pin) = self
            .key
            .get()
            .map(|k| match k.split_once(':') {
                Some((a, p)) => (a.trim().to_string(), Some(p.trim().to_string())),
                None => (k.trim().to_string(), None),
            })
            .ok_or_else(|| ConnectorError::MissingCredentials(API_KEY_ENV.into()))?;
        let pin = ctx
            .user_key
            .as_deref()
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .or(instance_pin)
            .filter(|p| !p.is_empty());
        let slot = pin.clone().unwrap_or_default();
        let version = self.key.version();
        let cached = self
            .tokens
            .lock()
            .expect("token lock")
            .get(&slot)
            .cloned()
            .filter(|(_, until, v)| Instant::now() < *until && *v == version);
        let token = if let Some((t, _, _)) = cached {
            t
        } else {
            let mut body = serde_json::json!({ "apikey": apikey });
            if let Some(pin) = pin {
                body["pin"] = pin.into();
            }
            let resp: Envelope<Login> = post_json(
                ctx,
                ID,
                "login",
                &format!("{}/login", self.api_base),
                vec![("Content-Type".into(), "application/json".into())],
                serde_json::to_vec(&body).unwrap_or_default(),
            )
            .await?;
            let t = resp.data.token;
            // tokens are valid for one month; renew well before
            let until = Instant::now() + Duration::from_hours(25 * 24);
            let mut tokens = self.tokens.lock().expect("token lock");
            // a new key version makes every older token useless
            tokens.retain(|_, (_, _, v)| *v == version);
            tokens.insert(slot, (t.clone(), until, version));
            t
        };
        Ok(vec![
            ("Authorization".into(), format!("Bearer {token}")),
            ("Accept".into(), "application/json".into()),
        ])
    }

    /// # Errors
    /// A base URL from the configuration that does not parse (UX2-S11).
    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<String, ConnectorError> {
        let mut u = url::Url::parse(&format!("{}{path}", self.api_base))
            .map_err(|e| ConnectorError::Parse(format!("base url: {e}")))?;
        if !query.is_empty() {
            u.query_pairs_mut().extend_pairs(query);
        }
        Ok(u.to_string())
    }
}

fn cover(url: Option<String>) -> Vec<ImageRef> {
    non_empty(url)
        .map(|url| ImageRef {
            kind: ImageKind::Cover,
            url,
            width: None,
            height: None,
        })
        .into_iter()
        .collect()
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Deserialize)]
struct Login {
    token: String,
}

#[derive(Deserialize)]
struct Hit {
    tvdb_id: Option<String>,
    name: Option<String>,
    overview: Option<String>,
    image_url: Option<String>,
    year: Option<String>,
    first_air_time: Option<String>,
    primary_language: Option<String>,
    network: Option<String>,
    status: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct SeasonType {
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct Season {
    number: Option<u32>,
    #[serde(rename = "type")]
    season_type: Option<SeasonType>,
}

#[derive(Deserialize)]
struct RemoteId {
    id: String,
    #[serde(rename = "sourceName")]
    source_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Series {
    id: u64,
    name: String,
    overview: Option<String>,
    image: Option<String>,
    first_aired: Option<String>,
    original_language: Option<String>,
    average_runtime: Option<u32>,
    status: Option<Named>,
    original_network: Option<Named>,
    #[serde(default)]
    genres: Vec<Named>,
    #[serde(default)]
    seasons: Vec<Season>,
    #[serde(default)]
    remote_ids: Vec<RemoteId>,
}

fn snake(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn check_kind(kind: &str) -> Result<(), ConnectorError> {
    if kind == KIND {
        Ok(())
    } else {
        Err(ConnectorError::Unsupported(format!("kind `{kind}`")))
    }
}

#[async_trait]
impl Search for Tvdb {
    async fn search(
        &self,
        ctx: &Ctx,
        req: &SearchRequest,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        check_kind(&req.kind)?;
        let limit = req.limit.clamp(1, 50).to_string();
        let url = self.url(
            "/search",
            &[
                ("query", req.query.as_str()),
                ("type", "series"),
                ("limit", &limit),
            ],
        )?;
        let headers = self.headers(ctx).await?;
        let page: Envelope<Vec<Hit>> = get_json(ctx, ID, "search", &url, headers).await?;
        Ok(page
            .data
            .into_iter()
            .filter_map(|h| {
                let id = h.tvdb_id.clone()?;
                let attributes = ShowAttributes {
                    genres: h.genres.clone(),
                    network: non_empty(h.network.clone()),
                    airing_status: h.status.as_deref().map(snake),
                    first_air_date: date_of(h.first_air_time.as_deref()),
                    original_language: non_empty(h.primary_language.clone()),
                    ..ShowAttributes::default()
                };
                Some(Snapshot {
                    connector: ID.into(),
                    kind: KIND.into(),
                    external_ids: vec![ExternalId::new(ID, id)],
                    title: h.name.unwrap_or_default(),
                    original_title: None,
                    year: h.year.as_deref().and_then(|y| y.parse().ok()),
                    description: non_empty(h.overview),
                    attributes: serde_json::to_value(attributes).unwrap_or_default(),
                    images: cover(h.image_url),
                    complete: false,
                })
            })
            .collect())
    }
}

#[async_trait]
impl Details for Tvdb {
    async fn details(&self, ctx: &Ctx, req: &DetailsRequest) -> Result<Snapshot, ConnectorError> {
        check_kind(&req.kind)?;
        let id: u64 = req
            .id
            .parse()
            .map_err(|_| ConnectorError::Parse(format!("tvdb id `{}`", req.id)))?;
        let url = self.url(&format!("/series/{id}/extended"), &[])?;
        let headers = self.headers(ctx).await?;
        let s: Envelope<Series> = get_json(ctx, ID, "details", &url, headers).await?;
        let s = s.data;
        let mut external_ids = vec![ExternalId::new(ID, s.id.to_string())];
        for r in &s.remote_ids {
            match r.source_name.as_deref() {
                Some("IMDB") => external_ids.push(ExternalId::new("imdb", r.id.clone())),
                Some("TheMovieDB.com") => external_ids.push(ExternalId::new("tmdb", r.id.clone())),
                _ => {}
            }
        }
        // aired order only; other orderings (DVD, absolute) describe the same episodes
        let mut seasons: Vec<SeasonInfo> = s
            .seasons
            .iter()
            .filter(|x| {
                x.season_type
                    .as_ref()
                    .and_then(|t| t.kind.as_deref())
                    .is_none_or(|t| t == "official")
            })
            .filter_map(|x| x.number)
            .map(|number| SeasonInfo {
                number,
                episode_count: 0,
            })
            .collect();
        seasons.sort_by_key(|x| x.number);
        seasons.dedup_by_key(|x| x.number);
        let attributes = ShowAttributes {
            genres: s.genres.into_iter().map(|g| g.name).collect(),
            network: s.original_network.map(|n| n.name),
            airing_status: s.status.map(|x| snake(&x.name)),
            first_air_date: date_of(s.first_aired.as_deref()),
            seasons,
            episode_runtime_minutes: s.average_runtime.filter(|r| *r > 0),
            original_language: non_empty(s.original_language),
        };
        Ok(Snapshot {
            connector: ID.into(),
            kind: KIND.into(),
            external_ids,
            year: year_of(s.first_aired.as_deref()),
            title: s.name,
            original_title: None,
            description: non_empty(s.overview),
            attributes: serde_json::to_value(attributes).unwrap_or_default(),
            images: cover(s.image),
            complete: true,
        })
    }
}

#[async_trait]
impl catalog_core::Images for Tvdb {
    async fn images(
        &self,
        ctx: &Ctx,
        req: &DetailsRequest,
    ) -> Result<Vec<ImageRef>, ConnectorError> {
        Ok(Details::details(self, ctx, req).await?.images)
    }
}

impl Connector for Tvdb {
    fn info(&self) -> &ConnectorInfo {
        &self.info
    }

    fn key_slot(&self) -> Option<KeySlot> {
        Some(self.key.clone())
    }

    fn credentials(&self) -> CredentialSpec {
        CREDENTIALS
    }

    fn probe(&self, key: Option<String>) -> Option<std::sync::Arc<dyn Connector>> {
        Some(std::sync::Arc::new(Self::with_base_url(
            key,
            &self.api_base,
        )))
    }

    fn as_search(&self) -> Option<&dyn Search> {
        Some(self)
    }

    fn as_details(&self) -> Option<&dyn Details> {
        Some(self)
    }

    fn as_images(&self) -> Option<&dyn catalog_core::Images> {
        Some(self)
    }
}
