//! IGDB (video games), API v4. Terms and attribution: `docs/connectors/igdb.md`.
//!
//! Requests are `POST https://api.igdb.com/v4/games` with an `APICalypse` body; auth is a Twitch
//! app access token (client credentials), cached until shortly before it expires.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, CredentialSpec, Ctx,
    Details, DetailsRequest, ImageKind, ImageRef, KeySlot, RateLimit, Search, SearchRequest,
    Snapshot,
};
use domain::catalog::ExternalId;
use kinds::builtin::VideoGameAttributes;
use serde::Deserialize;

use crate::util::{non_empty, post_json};

pub const ID: &str = "igdb";
/// `<client_id>:<client_secret>` from the Twitch developer console.
pub const API_KEY_ENV: &str = "SOURCE_IGDB_API_KEY";
/// Credential fields (stored as JSON, joined with `:` into the key string).
pub const CREDENTIALS: CredentialSpec = CredentialSpec {
    instance_fields: &["client_id", "client_secret"],
    user_fields: &[],
};
const DEFAULT_API_BASE: &str = "https://api.igdb.com/v4";
const DEFAULT_TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const DEFAULT_IMAGE_BASE: &str = "https://images.igdb.com/igdb/image/upload";
const KIND: &str = "video_game";
const FIELDS: &str = "fields name,first_release_date,summary,url,cover.image_id,genres.name,\
platforms.name,game_modes.name,involved_companies.company.name,involved_companies.developer,\
involved_companies.publisher;";

pub struct Igdb {
    info: ConnectorInfo,
    api_base: String,
    token_url: String,
    image_base: String,
    key: KeySlot,
    token: Mutex<Option<(String, Instant, u64)>>,
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

impl Igdb {
    /// `key` is `<client_id>:<client_secret>`.
    #[must_use]
    pub fn new(key: Option<String>) -> Self {
        Self::with_base_urls(key, DEFAULT_API_BASE, DEFAULT_TOKEN_URL, DEFAULT_IMAGE_BASE)
    }

    /// Connector against other base URLs (fixture server in e2e tests).
    #[must_use]
    pub fn with_base_urls(
        key: Option<String>,
        api_base: &str,
        token_url: &str,
        image_base: &str,
    ) -> Self {
        let api_base = api_base.trim_end_matches('/').to_string();
        let image_base = image_base.trim_end_matches('/').to_string();
        let info = ConnectorInfo {
            id: ID.into(),
            name: "IGDB".into(),
            kinds: vec![KIND.into()],
            capabilities: vec![Capability::Search, Capability::Details, Capability::Images],
            // docs: 4 requests per second, at most 8 open requests
            rate_limit: RateLimit {
                per_second: 4,
                burst: 4,
                max_concurrency: 8,
            },
            auth: Auth::Bearer {
                env: API_KEY_ENV.into(),
            },
            attribution: Attribution {
                text: "Video game data from IGDB.com.".into(),
                url: "https://www.igdb.com/".into(),
                licence: "IGDB API under the Twitch Developer Services Agreement".into(),
            },
            hosts: vec![host_of(&api_base), host_of(token_url), host_of(&image_base)],
            cache_ttl_secs: 7 * 24 * 3600,
            contact: catalog_core::ContactUse::None,
        };
        Self {
            info,
            api_base,
            token_url: token_url.to_string(),
            image_base,
            key: KeySlot::new(key),
            token: Mutex::new(None),
        }
    }

    async fn headers(&self, ctx: &Ctx) -> Result<Vec<(String, String)>, ConnectorError> {
        let (id, secret) = self
            .key
            .get()
            .and_then(|k| {
                k.split_once(':')
                    .map(|(id, secret)| (id.trim().to_string(), secret.trim().to_string()))
            })
            .ok_or_else(|| ConnectorError::MissingCredentials(API_KEY_ENV.into()))?;
        let version = self.key.version();
        let cached = self
            .token
            .lock()
            .expect("token lock")
            .clone()
            .filter(|(_, until, v)| Instant::now() < *until && *v == version);
        let token = if let Some((t, _, _)) = cached {
            t
        } else {
            // a form body, as the Twitch docs show (client credentials grant flow): the secret
            // never travels in the URL, so it cannot end up in an error string or a log line
            let body = url::form_urlencoded::Serializer::new(String::new())
                .append_pair("client_id", &id)
                .append_pair("client_secret", &secret)
                .append_pair("grant_type", "client_credentials")
                .finish();
            let headers = vec![(
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            )];
            let t: Token = post_json(
                ctx,
                ID,
                "token",
                &self.token_url,
                headers,
                body.into_bytes(),
            )
            .await?;
            // refresh a minute early
            let until = Instant::now() + Duration::from_secs(t.expires_in.saturating_sub(60));
            *self.token.lock().expect("token lock") =
                Some((t.access_token.clone(), until, version));
            t.access_token
        };
        Ok(vec![
            ("Client-ID".into(), id),
            ("Authorization".into(), format!("Bearer {token}")),
            ("Accept".into(), "application/json".into()),
        ])
    }

    fn cover(&self, g: &Game) -> Vec<ImageRef> {
        g.cover
            .as_ref()
            .and_then(|c| c.image_id.as_deref())
            .filter(|i| !i.is_empty())
            .map(|i| ImageRef {
                kind: ImageKind::Cover,
                url: format!("{}/t_cover_big_2x/{i}.jpg", self.image_base),
                width: Some(528),
                height: Some(748),
            })
            .into_iter()
            .collect()
    }

    fn snapshot(&self, g: &Game, complete: bool) -> Snapshot {
        let date = g
            .first_release_date
            .and_then(|s| chrono::DateTime::from_timestamp(s, 0))
            .map(|d| d.date_naive());
        let companies = |pick: fn(&InvolvedCompany) -> bool| -> Vec<String> {
            g.involved_companies
                .iter()
                .filter(|c| pick(c))
                .filter_map(|c| c.company.as_ref().map(|n| n.name.clone()))
                .collect()
        };
        let attributes = VideoGameAttributes {
            platforms: g.platforms.iter().map(|p| p.name.clone()).collect(),
            genres: g.genres.iter().map(|p| p.name.clone()).collect(),
            developers: companies(|c| c.developer),
            publishers: companies(|c| c.publisher),
            release_date: date,
            game_modes: g.modes.iter().map(|p| p.name.clone()).collect(),
        };
        Snapshot {
            connector: ID.into(),
            kind: KIND.into(),
            external_ids: vec![ExternalId::new(ID, g.id.to_string())],
            images: self.cover(g),
            title: g.name.clone(),
            original_title: None,
            year: date.map(|d| chrono::Datelike::year(&d)),
            description: non_empty(g.summary.clone()),
            attributes: serde_json::to_value(attributes).unwrap_or_default(),
            complete,
            popularity: None,
        }
    }
}

#[derive(Deserialize)]
struct Token {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

#[derive(Deserialize)]
struct Cover {
    image_id: Option<String>,
}

#[derive(Deserialize)]
struct InvolvedCompany {
    company: Option<Named>,
    #[serde(default)]
    developer: bool,
    #[serde(default)]
    publisher: bool,
}

#[derive(Deserialize)]
struct Game {
    id: u64,
    name: String,
    first_release_date: Option<i64>,
    summary: Option<String>,
    cover: Option<Cover>,
    #[serde(default)]
    genres: Vec<Named>,
    #[serde(default)]
    platforms: Vec<Named>,
    #[serde(default, rename = "game_modes")]
    modes: Vec<Named>,
    #[serde(default)]
    involved_companies: Vec<InvolvedCompany>,
}

/// `APICalypse` string literal (double quotes and backslashes escaped).
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn check_kind(kind: &str) -> Result<(), ConnectorError> {
    if kind == KIND {
        Ok(())
    } else {
        Err(ConnectorError::Unsupported(format!("kind `{kind}`")))
    }
}

#[async_trait]
impl Search for Igdb {
    async fn search(
        &self,
        ctx: &Ctx,
        req: &SearchRequest,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        check_kind(&req.kind)?;
        let body = format!(
            "search {}; {FIELDS} where version_parent = null; limit {};",
            quoted(&req.query),
            req.limit.clamp(1, 50)
        );
        let headers = self.headers(ctx).await?;
        let url = format!("{}/games", self.api_base);
        let games: Vec<Game> =
            post_json(ctx, ID, "search", &url, headers, body.into_bytes()).await?;
        Ok(games
            .into_iter()
            .map(|g| self.snapshot(&g, false))
            .collect())
    }
}

#[async_trait]
impl Details for Igdb {
    async fn details(&self, ctx: &Ctx, req: &DetailsRequest) -> Result<Snapshot, ConnectorError> {
        check_kind(&req.kind)?;
        let id: u64 = req
            .id
            .parse()
            .map_err(|_| ConnectorError::Parse(format!("igdb id `{}`", req.id)))?;
        let body = format!("{FIELDS} where id = {id};");
        let headers = self.headers(ctx).await?;
        let url = format!("{}/games", self.api_base);
        let games: Vec<Game> =
            post_json(ctx, ID, "details", &url, headers, body.into_bytes()).await?;
        games
            .into_iter()
            .next()
            .map(|g| self.snapshot(&g, true))
            .ok_or(ConnectorError::NotFound)
    }
}

#[async_trait]
impl catalog_core::Images for Igdb {
    async fn images(
        &self,
        ctx: &Ctx,
        req: &DetailsRequest,
    ) -> Result<Vec<ImageRef>, ConnectorError> {
        Ok(Details::details(self, ctx, req).await?.images)
    }
}

impl Connector for Igdb {
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
        Some(std::sync::Arc::new(Self::with_base_urls(
            key,
            &self.api_base,
            &self.token_url,
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

#[cfg(test)]
mod tests {
    #[test]
    fn quotes_search_text() {
        assert_eq!(super::quoted(r#"say "hi"\"#), r#""say \"hi\"\\""#);
    }
}
