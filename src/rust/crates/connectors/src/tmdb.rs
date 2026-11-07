//! TMDB (movie, show). Terms and attribution: `docs/connectors/tmdb.md`.

use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, CredentialSpec, Ctx,
    Details, DetailsRequest, ImageKind, ImageRef, KeySlot, RateLimit, Search, SearchRequest,
    Snapshot,
};
use domain::catalog::ExternalId;
use kinds::builtin::{MovieAttributes, SeasonInfo, ShowAttributes};
use serde::Deserialize;

use crate::util::{date_of, get_json, non_empty, year_of};

pub const ID: &str = "tmdb";
pub const API_KEY_ENV: &str = "KK_TMDB_API_KEY";
pub const DEFAULT_API_BASE: &str = "https://api.themoviedb.org";
pub const DEFAULT_IMAGE_BASE: &str = "https://image.tmdb.org";
/// The API read access token (v4 bearer).
pub const CREDENTIALS: CredentialSpec = CredentialSpec {
    instance_fields: &["api_key"],
    user_fields: &[],
};

pub struct Tmdb {
    info: ConnectorInfo,
    api_base: String,
    image_base: String,
    token: KeySlot,
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

impl Tmdb {
    /// Connector against the real TMDB hosts.
    #[must_use]
    pub fn new(token: Option<String>) -> Self {
        Self::with_base_urls(token, DEFAULT_API_BASE, DEFAULT_IMAGE_BASE)
    }

    /// Connector against other base URLs (fixture server in e2e tests).
    #[must_use]
    pub fn with_base_urls(token: Option<String>, api_base: &str, image_base: &str) -> Self {
        let api_base = api_base.trim_end_matches('/').to_string();
        let image_base = image_base.trim_end_matches('/').to_string();
        let info = ConnectorInfo {
            id: ID.into(),
            name: "TMDB".into(),
            kinds: vec!["movie".into(), "show".into()],
            capabilities: vec![Capability::Search, Capability::Details, Capability::Images],
            rate_limit: RateLimit { per_second: 20, burst: 10, max_concurrency: 8 },
            auth: Auth::Bearer { env: API_KEY_ENV.into() },
            attribution: Attribution {
                text: "This product uses TMDB and the TMDB APIs but is not endorsed, certified, or otherwise approved by TMDB.".into(),
                url: "https://www.themoviedb.org/".into(),
                licence: "TMDB API Terms of Use (non-commercial, attribution required)".into(),
            },
            hosts: vec![host_of(&api_base), host_of(&image_base)],
            cache_ttl_secs: 7 * 24 * 3600,
        };
        Self {
            info,
            api_base,
            image_base,
            token: KeySlot::new(token),
        }
    }

    fn headers(&self) -> Result<Vec<(String, String)>, ConnectorError> {
        let token = self
            .token
            .get()
            .ok_or_else(|| ConnectorError::MissingCredentials(API_KEY_ENV.into()))?;
        Ok(vec![
            ("Authorization".into(), format!("Bearer {}", token.as_str())),
            ("Accept".into(), "application/json".into()),
        ])
    }

    /// # Errors
    /// A base URL from the configuration that does not parse (UX2-S11).
    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<String, ConnectorError> {
        let mut u = url::Url::parse(&format!("{}{path}", self.api_base))
            .map_err(|e| ConnectorError::Parse(format!("base url: {e}")))?;
        u.query_pairs_mut().extend_pairs(query);
        Ok(u.to_string())
    }

    fn image(&self, size: &str, path: Option<&str>, kind: ImageKind) -> Option<ImageRef> {
        let p = path.filter(|p| !p.is_empty())?;
        Some(ImageRef {
            kind,
            url: format!("{}/t/p/{size}{p}", self.image_base),
            width: None,
            height: None,
        })
    }

    fn images(&self, poster: Option<&str>, backdrop: Option<&str>) -> Vec<ImageRef> {
        self.image("w780", poster, ImageKind::Cover)
            .into_iter()
            .chain(self.image("w1280", backdrop, ImageKind::Backdrop))
            .collect()
    }

    fn tmdb_path(kind: &str) -> Result<&'static str, ConnectorError> {
        match kind {
            "movie" => Ok("movie"),
            "show" => Ok("tv"),
            other => Err(ConnectorError::Unsupported(format!("kind `{other}`"))),
        }
    }
}

#[derive(Deserialize)]
struct Page<T> {
    results: Vec<T>,
}

/// Search result row; movies use `title`/`release_date`, shows `name`/`first_air_date`.
#[derive(Deserialize)]
struct Hit {
    id: u64,
    title: Option<String>,
    name: Option<String>,
    original_title: Option<String>,
    original_name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    original_language: Option<String>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct MovieDetails {
    id: u64,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    runtime: Option<u32>,
    genres: Vec<Named>,
    original_language: Option<String>,
    imdb_id: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
}

#[derive(Deserialize)]
struct Season {
    season_number: u32,
    episode_count: u32,
}

#[derive(Deserialize)]
struct ShowDetails {
    id: u64,
    name: String,
    original_name: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    genres: Vec<Named>,
    networks: Vec<Named>,
    status: Option<String>,
    seasons: Vec<Season>,
    episode_run_time: Vec<u32>,
    original_language: Option<String>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
}

fn snake(s: &str) -> String {
    s.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn to_value<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
}

#[async_trait]
impl Search for Tmdb {
    async fn search(
        &self,
        ctx: &Ctx,
        req: &SearchRequest,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        let path = Self::tmdb_path(&req.kind)?;
        let lang = req.language.as_deref().unwrap_or("en-US");
        let url = self.url(
            &format!("/3/search/{path}"),
            &[
                ("query", req.query.as_str()),
                ("include_adult", "false"),
                ("language", lang),
                ("page", "1"),
            ],
        )?;
        let page: Page<Hit> = get_json(ctx, ID, "search", &url, self.headers()?).await?;
        Ok(page
            .results
            .into_iter()
            .take(req.limit as usize)
            .map(|h| {
                let date = h.release_date.as_deref().or(h.first_air_date.as_deref());
                let attributes = if req.kind == "movie" {
                    to_value(&MovieAttributes {
                        release_date: date_of(date),
                        original_language: non_empty(h.original_language.clone()),
                        ..MovieAttributes::default()
                    })
                } else {
                    to_value(&ShowAttributes {
                        first_air_date: date_of(date),
                        original_language: non_empty(h.original_language.clone()),
                        ..ShowAttributes::default()
                    })
                };
                Snapshot {
                    connector: ID.into(),
                    kind: req.kind.clone(),
                    external_ids: vec![ExternalId::new(ID, h.id.to_string())],
                    title: h.title.or(h.name).unwrap_or_default(),
                    original_title: non_empty(h.original_title.or(h.original_name)),
                    year: year_of(date),
                    description: non_empty(h.overview),
                    attributes,
                    images: self.images(h.poster_path.as_deref(), h.backdrop_path.as_deref()),
                    complete: false,
                }
            })
            .collect())
    }
}

#[async_trait]
impl Details for Tmdb {
    async fn details(&self, ctx: &Ctx, req: &DetailsRequest) -> Result<Snapshot, ConnectorError> {
        let path = Self::tmdb_path(&req.kind)?;
        let lang = req.language.as_deref().unwrap_or("en-US");
        let url = self.url(&format!("/3/{path}/{}", req.id), &[("language", lang)])?;
        if req.kind == "movie" {
            let m: MovieDetails = get_json(ctx, ID, "details", &url, self.headers()?).await?;
            let mut external_ids = vec![ExternalId::new(ID, m.id.to_string())];
            external_ids.extend(non_empty(m.imdb_id).map(|i| ExternalId::new("imdb", i)));
            Ok(Snapshot {
                connector: ID.into(),
                kind: req.kind.clone(),
                external_ids,
                year: year_of(m.release_date.as_deref()),
                attributes: to_value(&MovieAttributes {
                    genres: m.genres.into_iter().map(|g| g.name).collect(),
                    runtime_minutes: m.runtime.filter(|r| *r > 0),
                    release_date: date_of(m.release_date.as_deref()),
                    original_language: non_empty(m.original_language),
                }),
                title: m.title,
                original_title: non_empty(m.original_title),
                description: non_empty(m.overview),
                images: self.images(m.poster_path.as_deref(), m.backdrop_path.as_deref()),
                complete: true,
            })
        } else {
            let s: ShowDetails = get_json(ctx, ID, "details", &url, self.headers()?).await?;
            Ok(Snapshot {
                connector: ID.into(),
                kind: req.kind.clone(),
                external_ids: vec![ExternalId::new(ID, s.id.to_string())],
                year: year_of(s.first_air_date.as_deref()),
                attributes: to_value(&ShowAttributes {
                    genres: s.genres.into_iter().map(|g| g.name).collect(),
                    network: s.networks.into_iter().next().map(|n| n.name),
                    airing_status: s.status.as_deref().map(snake),
                    first_air_date: date_of(s.first_air_date.as_deref()),
                    seasons: s
                        .seasons
                        .into_iter()
                        .map(|x| SeasonInfo {
                            number: x.season_number,
                            episode_count: x.episode_count,
                        })
                        .collect(),
                    episode_runtime_minutes: s.episode_run_time.first().copied(),
                    original_language: non_empty(s.original_language),
                }),
                title: s.name,
                original_title: non_empty(s.original_name),
                description: non_empty(s.overview),
                images: self.images(s.poster_path.as_deref(), s.backdrop_path.as_deref()),
                complete: true,
            })
        }
    }
}

#[async_trait]
impl catalog_core::Images for Tmdb {
    async fn images(
        &self,
        ctx: &Ctx,
        req: &DetailsRequest,
    ) -> Result<Vec<ImageRef>, ConnectorError> {
        Ok(Details::details(self, ctx, req).await?.images)
    }
}

impl Connector for Tmdb {
    fn info(&self) -> &ConnectorInfo {
        &self.info
    }

    fn key_slot(&self) -> Option<KeySlot> {
        Some(self.token.clone())
    }

    fn credentials(&self) -> CredentialSpec {
        CREDENTIALS
    }

    fn probe(&self, key: Option<String>) -> Option<std::sync::Arc<dyn Connector>> {
        Some(std::sync::Arc::new(Self::with_base_urls(
            key,
            &self.api_base,
            &self.image_base,
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
