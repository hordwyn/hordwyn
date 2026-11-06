//! Server wiring: one binary, `--role=all|web|worker`. Builds every adapter, plugs them into
//! the `app` core and serves the web app, the REST API, SSE and `/assets`.
#![deny(unsafe_code)]

mod handlers;
pub mod infra;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use app::ports::{EventBus, HttpFetch, JobQueue, SearchIndex};
use app::{App, Repos};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use catalog_core::cache::HttpCache;
use catalog_core::search::SearchFanout;
use catalog_core::{ConnectorPolicy, GovernedHttp, Registry};
use config::{AppConfig, ServerRole};
use tokio_util::sync::CancellationToken;

pub use handlers::REFRESH_LANE;

/// Everything a running process needs (cheap to clone: handles only).
#[derive(Clone)]
pub struct Server {
    pub config: AppConfig,
    pub brand: brand::Brand,
    pub app: App,
    pub api: api::ApiState,
    pub queue: jobs::PgQueue,
    pub budget: Arc<assets::StorageBudget>,
    pub assets: Arc<assets::AssetService>,
    pub registry: Registry,
    pub http: Arc<dyn HttpFetch>,
    pub bus: realtime::Bus,
    pub pool: sqlx::PgPool,
}

/// Test and embedding hooks; production uses the defaults.
#[derive(Default)]
pub struct Overrides {
    /// Connectors to use instead of the compiled-in ones.
    pub registry: Option<Registry>,
    /// Outgoing HTTP instead of the governed client (tests).
    pub http: Option<Arc<dyn HttpFetch>>,
    /// Search index (until the search crate is wired, M08).
    pub search: Option<Arc<dyn SearchIndex>>,
}

/// Compiled-in connectors enabled by the configuration (all by default).
fn default_registry(cfg: &AppConfig) -> Registry {
    let mut r = Registry::default();
    let enabled = |id: &str| cfg.connectors.get(id).is_none_or(|c| c.enabled);
    #[cfg(feature = "tmdb")]
    if enabled("tmdb") {
        let c = cfg.connectors.get("tmdb");
        let key = c
            .and_then(|c| c.api_key.as_ref())
            .map(|k| k.expose().to_string());
        let conn = match c.and_then(|c| c.base_url.clone()) {
            Some(base) => connectors::tmdb::Tmdb::with_base_urls(key, &base, &base),
            None => connectors::tmdb::Tmdb::new(key),
        };
        r.register(Arc::new(conn));
    }
    let _ = enabled;
    r
}

/// Search placeholder until the search crate is wired (M08): no local results.
struct NoSearch;

#[async_trait::async_trait]
impl SearchIndex for NoSearch {
    async fn upsert(
        &self,
        _: &str,
        _: Vec<app::ports::SearchDoc>,
    ) -> Result<(), app::ports::PortError> {
        Ok(())
    }
    async fn delete(&self, _: &str, _: Vec<String>) -> Result<(), app::ports::PortError> {
        Ok(())
    }
    async fn search(
        &self,
        _: app::ports::SearchQuery,
    ) -> Result<Vec<app::ports::SearchHit>, app::ports::PortError> {
        Ok(vec![])
    }
}

impl Server {
    /// Connect, migrate and build every adapter.
    ///
    /// # Errors
    /// Unreachable database, invalid brand or storage configuration.
    pub async fn build(config: AppConfig, role: ServerRole, o: Overrides) -> anyhow::Result<Self> {
        let brand = brand::Brand::load(std::path::Path::new(&config.brand.dir))
            .with_context(|| format!("brand directory {}", config.brand.dir))?;
        let pool = store_pg::connect(&config.database.url, config.database.max_connections)
            .await
            .context("connect to Postgres")?;
        store_pg::migrate(&pool).await.context("migrations")?;

        let store = Arc::new(store_pg::PgStore::new(pool.clone()));
        let repos = Repos {
            users: store.clone(),
            companions: store.clone(),
            link_requests: store.clone(),
            invites: store.clone(),
            catalog: store.clone(),
            library: store.clone(),
            progress: store.clone(),
            reviews: store.clone(),
            lists: store.clone(),
            sessions: store,
        };
        // Web and worker in separate processes need cross-process events (LISTEN/NOTIFY).
        let bus = if role == ServerRole::All {
            realtime::Bus::local()
        } else {
            realtime::Bus::postgres(pool.clone()).await?
        };
        let queue = jobs::PgQueue::new(pool.clone());
        let clock: Arc<dyn app::ports::Clock> = Arc::new(infra::SystemClock::default());
        let mut kinds = kinds::Registry::with_builtins();
        if !config.features.kinds.is_empty() {
            kinds.retain(&config.features.kinds);
        }
        let kind_ids: Vec<String> = kinds.ids().map(ToString::to_string).collect();
        let app = App {
            repos,
            kinds: Arc::new(kinds),
            clock: clock.clone(),
            ids: Arc::new(infra::SystemIds),
            events: Arc::new(bus.clone()),
            jobs: Arc::new(queue.clone()),
        };

        let budget = Arc::new(
            assets::StorageBudget::open(
                config.storage.clone(),
                Arc::new(assets::pg::PgObjectIndex::new(pool.clone())),
                clock.clone(),
                Arc::new(bus.clone()),
            )?
            .with_jobs(Arc::new(queue.clone())),
        );
        let registry = o.registry.unwrap_or_else(|| default_registry(&config));
        let http: Arc<dyn HttpFetch> = o
            .http
            .unwrap_or_else(|| governed_http(&config, &brand, &registry, &pool, budget.clone()));
        let assets = Arc::new(assets::AssetService::new(
            budget.clone(),
            Arc::new(assets::pg::PgAssetRepo::new(pool.clone())),
            http.clone(),
            Arc::new(queue.clone()),
            assets::CpuPool::default_size()?,
        ));
        let auth = auth::Auth::new(
            app.clone(),
            Arc::new(auth::pg::PgAuthStore::new(pool.clone())),
            auth_settings(&config, &brand),
        );
        let oidc = oidc(&config, http.clone());
        let ident = brand.ident();
        let api = api::ApiState {
            app: app.clone(),
            auth,
            oidc,
            bus: bus.clone(),
            search: o.search.unwrap_or_else(|| Arc::new(NoSearch)),
            fanout: SearchFanout::new(app.clone(), registry.clone(), http.clone()),
            registry: registry.clone(),
            assets: Some(assets.clone()),
            naming: api::Naming::from_prefix(&ident, &brand.name),
            secure_cookies: config.auth.secure_cookies,
            default_kinds: kind_ids,
        };
        Ok(Self {
            config,
            brand,
            app,
            api,
            queue,
            budget,
            assets,
            registry,
            http,
            bus,
            pool,
        })
    }

    /// HTTP routes that do not need Leptos: health, REST API, SSE, assets.
    pub fn base_router(&self) -> Router {
        Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route(
                "/assets/{hash}/{file}",
                get(serve_asset).with_state(self.assets.clone()),
            )
            .merge(api::router(self.api.clone()))
    }

    /// The job worker with every handler registered.
    #[must_use]
    pub fn worker(&self) -> jobs::Worker {
        handlers::worker(self)
    }

    /// Run the worker and the cron ticker until `stop` is cancelled.
    pub async fn run_worker(&self, stop: CancellationToken) {
        let ticker = {
            let queue = self.queue.clone();
            let stop = stop.clone();
            tokio::spawn(async move {
                loop {
                    if let Err(e) = queue.tick_cron(chrono::Utc::now()).await {
                        tracing::warn!(error = %e, "cron tick failed");
                    }
                    tokio::select! {
                        () = stop.cancelled() => break,
                        () = tokio::time::sleep(Duration::from_secs(30)) => {}
                    }
                }
            })
        };
        self.worker().run(stop).await;
        let _ = ticker.await;
    }

    #[must_use]
    pub fn jobs(&self) -> Arc<dyn JobQueue> {
        Arc::new(self.queue.clone())
    }

    #[must_use]
    pub fn events(&self) -> Arc<dyn EventBus> {
        Arc::new(self.bus.clone())
    }
}

/// `/assets/{hash}/{variant}.{ext}`: immutable variants, or a redirect to the provider.
async fn serve_asset(
    State(svc): State<Arc<assets::AssetService>>,
    Path((hash, file)): Path<(String, String)>,
) -> Response {
    let Some((variant, ext)) = file.rsplit_once('.') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match svc.serve(&hash, variant, ext).await {
        Ok(assets::Served::Bytes { mime, bytes }) => (
            [
                (header::CONTENT_TYPE, mime),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            bytes,
        )
            .into_response(),
        Ok(assets::Served::Redirect(url)) => (
            StatusCode::FOUND,
            [
                (header::LOCATION, url),
                (header::CACHE_CONTROL, "no-store".into()),
            ],
        )
            .into_response(),
        Ok(assets::Served::NotFound) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "serving asset failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// The production `HttpFetch`: governed client with HTTP cache in the storage budget and one
/// policy (allowlist, rate limit) per connector, plus the OIDC issuer.
fn governed_http(
    config: &AppConfig,
    brand: &brand::Brand,
    registry: &Registry,
    pool: &sqlx::PgPool,
    budget: Arc<assets::StorageBudget>,
) -> Arc<dyn HttpFetch> {
    let cache = HttpCache {
        index: Arc::new(catalog_core::pg::PgCacheIndex::new(pool.clone())),
        blobs: budget,
    };
    let ua = brand.user_agent(env!("CARGO_PKG_VERSION"));
    let mut g = GovernedHttp::new(&ua, Some(cache));
    for info in registry.infos() {
        g.add_connector(&info.id, ConnectorPolicy::from_info(&info));
    }
    if let Some(oidc) = &config.auth.oidc
        && let Ok(u) = url::Url::parse(&oidc.issuer)
        && let Some(host) = u.host_str()
    {
        g.add_connector(auth::oidc::CONNECTOR, ConnectorPolicy::oidc(host, u.port()));
    }
    Arc::new(g)
}

fn auth_settings(config: &AppConfig, brand: &brand::Brand) -> auth::AuthSettings {
    auth::AuthSettings {
        signup: match config.auth.signup {
            config::SignupMode::Open => auth::SignupMode::Open,
            config::SignupMode::Invite => auth::SignupMode::Invite,
            config::SignupMode::Closed => auth::SignupMode::Closed,
        },
        session_days: config.auth.session_days,
        totp_issuer: brand.name.clone(),
    }
}

fn oidc(config: &AppConfig, http: Arc<dyn HttpFetch>) -> Option<Arc<auth::oidc::Oidc>> {
    config.auth.oidc.as_ref().map(|c| {
        Arc::new(auth::oidc::Oidc::new(
            auth::oidc::OidcSettings {
                issuer: c.issuer.clone(),
                client_id: c.client_id.clone(),
                client_secret: c.client_secret.as_ref().map(|s| s.expose().to_string()),
                redirect_url: format!(
                    "{}/api/v1/auth/oidc/callback",
                    config.server.public_url.trim_end_matches('/')
                ),
                auto_create: c.auto_create,
            },
            http,
        ))
    })
}
