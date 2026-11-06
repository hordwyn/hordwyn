//! Server wiring: one binary, `--role=all|web|worker`. Builds every adapter, plugs them into
//! the `app` core and serves the web app, the REST API, SSE and `/assets`.
#![deny(unsafe_code)]

mod csrf;
mod handlers;
pub mod infra;
pub mod web_backend;

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
    pub search: Arc<dyn SearchIndex>,
    pub notify: Arc<notify::Engine>,
    pub deliverer: Arc<notify::Deliverer>,
}

/// Test and embedding hooks; production uses the defaults.
#[derive(Default)]
pub struct Overrides {
    /// Connectors to use instead of the compiled-in ones.
    pub registry: Option<Registry>,
    /// Outgoing HTTP instead of the governed client (tests).
    pub http: Option<Arc<dyn HttpFetch>>,
    /// Search index instead of the configured backend.
    pub search: Option<Arc<dyn SearchIndex>>,
}

/// Built-in kinds, limited to `features.kinds` when set, and their ids.
fn enabled_kinds(config: &AppConfig) -> (kinds::Registry, Vec<String>) {
    let mut kinds = kinds::Registry::with_builtins();
    if !config.features.kinds.is_empty() {
        kinds.retain(&config.features.kinds);
    }
    let ids = kinds.ids().map(ToString::to_string).collect();
    (kinds, ids)
}

/// The notification engine and deliverer, and the VAPID public key if web push is on.
async fn notify_engine(
    config: &AppConfig,
    brand: &brand::Brand,
    pool: &sqlx::PgPool,
    queue: &jobs::PgQueue,
    bus: &realtime::Bus,
    clock: Arc<dyn app::ports::Clock>,
) -> anyhow::Result<(Arc<notify::Engine>, Arc<notify::Deliverer>, Option<String>)> {
    let engine = Arc::new(notify::Engine::new(
        Arc::new(notify::pg::PgNotifyStore::new(pool.clone())),
        Arc::new(queue.clone()),
        Arc::new(bus.clone()),
        clock.clone(),
        Arc::new(infra::SystemIds),
        &brand.name,
        &brand.default_locale,
        brand.strings.clone(),
    ));
    engine
        .reload_templates()
        .await
        .context("notification templates")?;
    let (channels, web_push_key) =
        notify_channels(config, &brand.user_agent(env!("CARGO_PKG_VERSION")))?;
    let deliverer = Arc::new(notify::Deliverer::new(
        engine.clone(),
        Arc::new(channels),
        clock,
        &config.server.public_url,
        config.notify.max_attempts,
    ));
    Ok((engine, deliverer, web_push_key))
}

/// Notification channels from the configuration, and the VAPID public key if web push is on
/// (never yet, DEC-141).
fn notify_channels(
    cfg: &AppConfig,
    user_agent: &str,
) -> anyhow::Result<(notify::channels::Channels, Option<String>)> {
    use notify::channels::{Channels, email, http};
    let n = &cfg.notify;
    let email = n
        .smtp
        .as_ref()
        .map(|s| {
            email::Email::new(&email::SmtpSettings {
                host: s.host.clone(),
                port: s.port,
                security: match s.security {
                    config::SmtpSecurity::Tls => email::SmtpSecurity::Tls,
                    config::SmtpSecurity::Starttls => email::SmtpSecurity::StartTls,
                    config::SmtpSecurity::None => email::SmtpSecurity::None,
                },
                username: s.username.clone(),
                password: s.password.as_ref().map(|p| p.expose().to_string()),
                from: s.from.clone(),
            })
        })
        .transpose()
        .map_err(|e| anyhow::anyhow!("notify.smtp: {e}"))?;
    Ok((
        Channels {
            http: Some(http::HttpChannels::new(
                Arc::new(catalog_core::PlainHttp::new(
                    user_agent,
                    n.allow_private_targets,
                )),
                n.allow_private_targets,
            )),
            email,
        },
        // web push is not built yet (DEC-141)
        None,
    ))
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
    // Connectors that cannot work without credentials are only registered when they have them.
    let key = |id: &str| {
        cfg.connectors
            .get(id)
            .and_then(|c| c.api_key.as_ref())
            .map(|k| k.expose().to_string())
    };
    #[cfg(feature = "igdb")]
    if enabled("igdb")
        && let Some(k) = key("igdb")
    {
        r.register(Arc::new(connectors::igdb::Igdb::new(Some(k))));
    }
    #[cfg(feature = "podcastindex")]
    if enabled("podcastindex")
        && let Some(k) = key("podcastindex")
    {
        let c = match cfg
            .connectors
            .get("podcastindex")
            .and_then(|c| c.base_url.clone())
        {
            Some(base) => connectors::podcastindex::PodcastIndex::with_base_url(Some(k), &base),
            None => connectors::podcastindex::PodcastIndex::new(Some(k)),
        };
        r.register(Arc::new(c));
    }
    #[cfg(feature = "tvdb")]
    if enabled("tvdb")
        && let Some(k) = key("tvdb")
    {
        let c = match cfg.connectors.get("tvdb").and_then(|c| c.base_url.clone()) {
            Some(base) => connectors::tvdb::Tvdb::with_base_url(Some(k), &base),
            None => connectors::tvdb::Tvdb::new(Some(k)),
        };
        r.register(Arc::new(c));
    }
    let _ = (enabled, key);
    r
}

impl Server {
    /// Connect, migrate and build every adapter.
    ///
    /// # Errors
    /// Unreachable database, invalid brand or storage configuration.
    // The composition root: every adapter is wired here, in one place, on purpose.
    #[allow(clippy::too_many_lines)]
    pub async fn build(config: AppConfig, role: ServerRole, o: Overrides) -> anyhow::Result<Self> {
        let brand = brand::Brand::load(std::path::Path::new(&config.brand.dir))
            .with_context(|| format!("brand directory {}", config.brand.dir))?;
        let pool = store_pg::connect(&config.database.url, config.database.max_connections)
            .await
            .context("connect to Postgres")?;
        store_pg::migrate(&pool).await.context("migrations")?;

        let repos = pg_repos(&pool);
        // Web and worker in separate processes need cross-process events (LISTEN/NOTIFY).
        let bus = if role == ServerRole::All {
            realtime::Bus::local()
        } else {
            realtime::Bus::postgres(pool.clone()).await?
        };
        let queue = jobs::PgQueue::new(pool.clone());
        let clock: Arc<dyn app::ports::Clock> = Arc::new(infra::SystemClock::default());
        let (kinds, kind_ids) = enabled_kinds(&config);
        let (engine, deliverer, web_push_key) =
            notify_engine(&config, &brand, &pool, &queue, &bus, clock.clone()).await?;
        let app = App {
            repos,
            kinds: Arc::new(kinds),
            clock: clock.clone(),
            ids: Arc::new(infra::SystemIds),
            events: Arc::new(bus.clone()),
            jobs: Arc::new(queue.clone()),
            notifications: engine.clone(),
        };

        // storage watermark events also become admin notifications
        let budget_events =
            notify::AdminBridge::new(Arc::new(bus.clone()), engine.clone(), clock.clone());
        let budget = Arc::new(
            assets::StorageBudget::open(
                config.storage.clone(),
                Arc::new(assets::pg::PgObjectIndex::new(pool.clone())),
                clock.clone(),
                Arc::new(budget_events),
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
        let search = match o.search {
            Some(s) => s,
            None => search_index(&config, &pool).await,
        };
        let ident = brand.ident();
        let api = api::ApiState {
            app: app.clone(),
            auth,
            oidc,
            bus: bus.clone(),
            search: search.clone(),
            fanout: SearchFanout::new(app.clone(), registry.clone(), http.clone()),
            registry: registry.clone(),
            assets: Some(assets.clone()),
            changes: Some(Arc::new(store_pg::PgStore::new(pool.clone()))),
            notify: Some(api::NotifyApi {
                engine: engine.clone(),
                deliverer: deliverer.clone(),
                web_push_key,
            }),
            integrations: Some(Arc::new(integrations::Integrations::new(
                app.clone(),
                Arc::new(integrations::pg::PgIntegrationStore::new(pool.clone())),
            ))),
            public_url: config.server.public_url.clone(),
            importer: Some(Arc::new(importers::Importer::new(
                app.clone(),
                Arc::new(importers::pg::PgImportStore::new(pool.clone())),
                budget.clone(),
                Arc::new(queue.clone()),
            ))),
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
            search,
            notify: engine,
            deliverer,
        })
    }

    /// HTTP routes that do not need Leptos: health, REST API, SSE, assets.
    pub fn base_router(&self) -> Router {
        let r = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route(
                "/assets/{hash}/{file}",
                get(serve_asset).with_state(self.assets.clone()),
            )
            .merge(api::router(self.api.clone()));
        self.guarded(r)
    }

    /// Cross-site request protection for cookie-authenticated mutations (G2-1).
    fn guarded(&self, r: Router) -> Router {
        r.layer(axum::middleware::from_fn_with_state(
            csrf::public_origin(&self.config.server.public_url),
            csrf::guard,
        ))
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
        let indexer = {
            let ix = search::indexer::Indexer::new(
                self.app.clone(),
                self.pool.clone(),
                self.search.clone(),
            );
            let stop = stop.clone();
            tokio::spawn(async move { ix.run(stop).await })
        };
        self.worker().run(stop).await;
        let _ = ticker.await;
        let _ = indexer.await;
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

/// The configured search backend. If Meilisearch cannot be reached at start-up the process
/// uses the Postgres fallback instead of refusing to start (logged as an error).
async fn search_index(config: &AppConfig, pool: &sqlx::PgPool) -> Arc<dyn SearchIndex> {
    let pg = || -> Arc<dyn SearchIndex> { Arc::new(search::pg::PgSearch::new(pool.clone())) };
    match config.search.backend {
        config::SearchBackend::Postgres => pg(),
        config::SearchBackend::Meilisearch => {
            let Some(key) = &config.search.meili_master_key else {
                tracing::error!("search.meili_master_key is not set; using Postgres search");
                return pg();
            };
            match search::meili::MeiliIndex::connect(
                &config.search.meili_url,
                key.expose(),
                &config.search.index_prefix,
            )
            .await
            {
                Ok(m) => Arc::new(m),
                Err(e) => {
                    tracing::error!(error = %e, "Meilisearch unavailable; using Postgres search");
                    pg()
                }
            }
        }
    }
}

impl Server {
    /// Per-request context for the web app: brand and the server-function backend.
    pub fn web_context(&self) -> impl Fn() + Clone + Send + Sync + 'static {
        let brand = Arc::new(self.brand.clone());
        let backend: Arc<dyn web::backend::Backend> =
            Arc::new(web_backend::WebBackend(self.api.clone()));
        move || {
            leptos::prelude::provide_context(brand.clone());
            leptos::prelude::provide_context(backend.clone());
        }
    }
}

impl Server {
    /// Everything the web role serves: base routes, server functions and the Leptos pages.
    pub fn app_router(&self, options: &leptos::prelude::LeptosOptions) -> Router {
        use leptos_axum::LeptosRoutes;
        let routes = leptos_axum::generate_route_list(web::App);
        let context = self.web_context();
        let pages = Router::new()
            .route(
                "/_fn/{*fn_name}",
                axum::routing::any({
                    let context = context.clone();
                    move |req| leptos_axum::handle_server_fns_with_context(context.clone(), req)
                }),
            )
            .leptos_routes_with_context(options, routes, context, {
                let options = options.clone();
                move || web::shell(options.clone())
            })
            .fallback(leptos_axum::file_and_error_handler(web::shell))
            .with_state(options.clone());
        let pages = self.guarded(pages);
        self.base_router().merge(pages)
    }
}

/// Every repository port on Postgres.
fn pg_repos(pool: &sqlx::PgPool) -> Repos {
    let store = Arc::new(store_pg::PgStore::new(pool.clone()));
    Repos {
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
    }
}
