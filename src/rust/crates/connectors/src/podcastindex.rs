//! Podcast Index (podcasts), API 1.0. Terms and attribution: `docs/connectors/podcastindex.md`.
//!
//! Every request is signed: `X-Auth-Key`, `X-Auth-Date` (Unix seconds) and
//! `Authorization = sha1(key + secret + date)` as lowercase hex, plus a `User-Agent`.

use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, CredentialSpec, Ctx,
    Details, DetailsRequest, ImageKind, ImageRef, KeySlot, RateLimit, Search, SearchRequest,
    Snapshot,
};
use domain::catalog::ExternalId;
use kinds::builtin::PodcastAttributes;
use serde::Deserialize;
use sha1::{Digest, Sha1};

use crate::util::{get_json, non_empty};

pub const ID: &str = "podcastindex";
/// `<api_key>:<api_secret>`.
pub const API_KEY_ENV: &str = "KK_PODCASTINDEX_API_KEY";
/// Credential fields (stored as JSON, joined with `:` into the key string).
pub const CREDENTIALS: CredentialSpec = CredentialSpec {
    instance_fields: &["api_key", "api_secret"],
    user_fields: &[],
};
const DEFAULT_API_BASE: &str = "https://api.podcastindex.org/api/1.0";
const KIND: &str = "podcast";

pub struct PodcastIndex {
    info: ConnectorInfo,
    api_base: String,
    key: KeySlot,
    /// Unix time source (tests pin it).
    now: fn() -> i64,
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

fn unix_now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// `sha1(key + secret + date)` in lowercase hex (API docs, Authorization header).
#[must_use]
pub fn signature(key: &str, secret: &str, date: i64) -> String {
    use std::fmt::Write as _;
    let digest = Sha1::digest(format!("{key}{secret}{date}").as_bytes());
    digest.iter().fold(String::with_capacity(40), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

impl PodcastIndex {
    /// `key` is `<api_key>:<api_secret>`.
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
                name: "Podcast Index".into(),
                kinds: vec![KIND.into()],
                capabilities: vec![Capability::Search, Capability::Details, Capability::Images],
                // no published limit; stay polite
                rate_limit: RateLimit {
                    per_second: 2,
                    burst: 4,
                    max_concurrency: 2,
                },
                auth: Auth::ApiKey {
                    env: API_KEY_ENV.into(),
                },
                attribution: Attribution {
                    text: "Podcast data from the Podcast Index.".into(),
                    url: "https://podcastindex.org/".into(),
                    licence: "Podcast Index: the core index is free for any use".into(),
                },
                // artwork lives on the podcasters' own hosts; it is fetched as a redirect
                // target by the browser, not by the server
                hosts: vec![host_of(&api_base)],
                cache_ttl_secs: 24 * 3600,
            },
            api_base,
            key: KeySlot::new(key),
            now: unix_now,
        }
    }

    /// Fix the clock (deterministic tests).
    #[must_use]
    pub fn with_clock(mut self, now: fn() -> i64) -> Self {
        self.now = now;
        self
    }

    fn headers(&self) -> Result<Vec<(String, String)>, ConnectorError> {
        let (key, secret) = self
            .key
            .get()
            .and_then(|k| {
                k.split_once(':')
                    .map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))
            })
            .ok_or_else(|| ConnectorError::MissingCredentials(API_KEY_ENV.into()))?;
        let date = (self.now)();
        Ok(vec![
            ("X-Auth-Key".into(), key.clone()),
            ("X-Auth-Date".into(), date.to_string()),
            ("Authorization".into(), signature(&key, &secret, date)),
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
}

#[derive(Deserialize)]
struct Feed {
    id: u64,
    title: String,
    url: Option<String>,
    description: Option<String>,
    author: Option<String>,
    artwork: Option<String>,
    image: Option<String>,
    language: Option<String>,
    explicit: Option<bool>,
    #[serde(rename = "episodeCount")]
    episode_count: Option<u32>,
    #[serde(rename = "itunesId")]
    itunes_id: Option<u64>,
    #[serde(rename = "podcastGuid")]
    podcast_guid: Option<String>,
    #[serde(default)]
    categories: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Deserialize)]
struct SearchPage {
    #[serde(default)]
    feeds: Vec<Feed>,
}

#[derive(Deserialize)]
struct FeedPage {
    feed: Option<Feed>,
}

fn snapshot(f: Feed, complete: bool) -> Snapshot {
    let mut external_ids = vec![ExternalId::new(ID, f.id.to_string())];
    external_ids.extend(
        f.itunes_id
            .map(|i| ExternalId::new("itunes", i.to_string())),
    );
    external_ids.extend(non_empty(f.podcast_guid).map(|g| ExternalId::new("podcast_guid", g)));
    let artwork = non_empty(f.artwork).or_else(|| non_empty(f.image));
    let attributes = PodcastAttributes {
        author: non_empty(f.author),
        feed_url: non_empty(f.url),
        categories: f.categories.unwrap_or_default().into_values().collect(),
        episode_count: f.episode_count,
        language: non_empty(f.language),
        explicit: f.explicit,
    };
    Snapshot {
        connector: ID.into(),
        kind: KIND.into(),
        external_ids,
        title: f.title,
        original_title: None,
        year: None,
        description: non_empty(f.description),
        attributes: serde_json::to_value(attributes).unwrap_or_default(),
        images: artwork
            .map(|url| ImageRef {
                kind: ImageKind::Cover,
                url,
                width: None,
                height: None,
            })
            .into_iter()
            .collect(),
        complete,
    }
}

fn check_kind(kind: &str) -> Result<(), ConnectorError> {
    if kind == KIND {
        Ok(())
    } else {
        Err(ConnectorError::Unsupported(format!("kind `{kind}`")))
    }
}

#[async_trait]
impl Search for PodcastIndex {
    async fn search(
        &self,
        ctx: &Ctx,
        req: &SearchRequest,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        check_kind(&req.kind)?;
        let max = req.limit.clamp(1, 40).to_string();
        let url = self.url(
            "/search/byterm",
            &[("q", req.query.as_str()), ("max", &max)],
        )?;
        let page: SearchPage = get_json(ctx, ID, "search", &url, self.headers()?).await?;
        Ok(page.feeds.into_iter().map(|f| snapshot(f, false)).collect())
    }
}

#[async_trait]
impl Details for PodcastIndex {
    async fn details(&self, ctx: &Ctx, req: &DetailsRequest) -> Result<Snapshot, ConnectorError> {
        check_kind(&req.kind)?;
        let url = self.url("/podcasts/byfeedid", &[("id", req.id.as_str())])?;
        let page: FeedPage = get_json(ctx, ID, "details", &url, self.headers()?).await?;
        page.feed
            .map(|f| snapshot(f, true))
            .ok_or(ConnectorError::NotFound)
    }
}

#[async_trait]
impl catalog_core::Images for PodcastIndex {
    async fn images(
        &self,
        ctx: &Ctx,
        req: &DetailsRequest,
    ) -> Result<Vec<ImageRef>, ConnectorError> {
        Ok(Details::details(self, ctx, req).await?.images)
    }
}

impl Connector for PodcastIndex {
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
        Some(std::sync::Arc::new(
            Self::with_base_url(key, &self.api_base).with_clock(self.now),
        ))
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

#[cfg(test)]
mod tests {
    #[test]
    fn signature_is_lowercase_hex_sha1() {
        let s = super::signature("key", "secret", 1_613_713_388);
        assert_eq!(s.len(), 40);
        assert!(
            s.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
        assert_ne!(s, super::signature("key", "secret", 1_613_713_389));
    }
}
