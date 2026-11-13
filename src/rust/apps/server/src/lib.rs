//! Server wiring: one binary, `--role=all|web|worker`. Builds every adapter, plugs them into
//! the `app` core and serves the web app, the REST API, SSE and `/assets`.
#![deny(unsafe_code)]

mod admin_wiring;
pub mod book_stubs;
mod csrf;
pub mod escape;
mod handlers;
pub mod infra;
pub mod security_headers;
pub mod web_admin;
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

/// How often each process flushes its data-source counters (WP4-2 default).
pub const STATS_SYNC_SECS: u64 = 60;

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
    /// Proxies whose forwarded client address and request id are believed (DEC-284).
    pub trusted_proxies: Arc<api::request_meta::TrustedProxies>,
    /// The security headers of every answer (SEC-1).
    pub headers: Arc<security_headers::Headers>,
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
    cipher: Arc<dyn app::ports::SecretCipher>,
) -> anyhow::Result<NotifyParts> {
    let engine = Arc::new(notify::Engine::new(
        Arc::new(notify::pg::PgNotifyStore::new(pool.clone(), cipher)),
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
    let (channels, web_push_key, mailer) =
        notify_channels(config, &brand.user_agent(env!("CARGO_PKG_VERSION")))?;
    let deliverer = Arc::new(notify::Deliverer::new(
        engine.clone(),
        Arc::new(channels),
        clock,
        &config.server.public_url,
        config.notify.max_attempts,
    ));
    Ok(NotifyParts {
        engine,
        deliverer,
        web_push_key,
        mailer,
    })
}

struct NotifyParts {
    engine: Arc<notify::Engine>,
    deliverer: Arc<notify::Deliverer>,
    web_push_key: Option<String>,
    mailer: Arc<notify::channels::email::Mailer>,
}

/// Notification channels from the configuration, and the VAPID public key if web push is on
/// (never yet, DEC-141).
fn notify_channels(
    cfg: &AppConfig,
    user_agent: &str,
) -> anyhow::Result<(
    notify::channels::Channels,
    Option<String>,
    Arc<notify::channels::email::Mailer>,
)> {
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
    let mailer = Arc::new(email::Mailer::new(email));
    Ok((
        Channels {
            http: Some(http::HttpChannels::new(
                Arc::new(catalog_core::PlainHttp::new(
                    user_agent,
                    n.allow_private_targets,
                )),
                n.allow_private_targets,
            )),
            email: mailer.clone(),
        },
        // web push is not built yet (DEC-141)
        None,
        mailer,
    ))
}

/// Every compiled-in connector (always registered, design.md 4.2: a missing key is a
/// credential state, not an absence), except those the configuration switches off, which
/// are returned as (id, name, kinds) for the admin's source table.
fn default_registry(cfg: &AppConfig) -> (Registry, Vec<(String, String, Vec<String>)>) {
    let mut all: Vec<Arc<dyn catalog_core::Connector>> = Vec::new();
    let c = |id: &str| cfg.connectors.get(id);
    let key = |id: &str| {
        c(id)
            .and_then(|c| c.api_key.as_ref())
            .map(|k| k.expose().to_string())
    };
    let base = |id: &str| c(id).and_then(|c| c.base_url.clone());
    #[cfg(feature = "tmdb")]
    all.push(Arc::new(match base("tmdb") {
        Some(b) => connectors::tmdb::Tmdb::with_base_urls(key("tmdb"), &b, &b),
        None => connectors::tmdb::Tmdb::new(key("tmdb")),
    }));
    #[cfg(feature = "igdb")]
    all.push(Arc::new(connectors::igdb::Igdb::new(key("igdb"))));
    // no key; `base_url` moves the API and the covers to one other host (tests, a mirror)
    #[cfg(feature = "openlibrary")]
    all.push(Arc::new(match base("openlibrary") {
        Some(b) => connectors::openlibrary::OpenLibrary::with_base_urls(&b, &b),
        None => connectors::openlibrary::OpenLibrary::new(),
    }));
    #[cfg(feature = "podcastindex")]
    all.push(Arc::new(match base("podcastindex") {
        Some(b) => connectors::podcastindex::PodcastIndex::with_base_url(key("podcastindex"), &b),
        None => connectors::podcastindex::PodcastIndex::new(key("podcastindex")),
    }));
    #[cfg(feature = "tvdb")]
    all.push(Arc::new(match base("tvdb") {
        Some(b) => connectors::tvdb::Tvdb::with_base_url(key("tvdb"), &b),
        None => connectors::tvdb::Tvdb::new(key("tvdb")),
    }));
    let _ = (&key, &base);
    let mut r = Registry::default();
    let mut disabled = Vec::new();
    for conn in all {
        let info = conn.info().clone();
        if c(&info.id).is_none_or(|c| c.enabled) {
            r.register(conn);
        } else {
            disabled.push((info.id, info.name, info.kinds));
        }
    }
    (r, disabled)
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
        log_contrast(&brand);
        // in containers the database name may resolve only a moment after start-up
        let pool = store_pg::connect_with_retry(
            &config.database.url,
            config.database.max_connections,
            Duration::from_secs(60),
        )
        .await
        .context("connect to Postgres")?;
        store_pg::migrate(&pool).await.context("migrations")?;
        let keys = admin_wiring::load_keys(&config, &pool).await?;
        let cipher: Arc<dyn app::ports::SecretCipher> = keys.ring.clone();

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
        let NotifyParts {
            engine,
            deliverer,
            web_push_key,
            mailer,
        } = notify_engine(
            &config,
            &brand,
            &pool,
            &queue,
            &bus,
            clock.clone(),
            cipher.clone(),
        )
        .await?;
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
        let stats = Arc::new(catalog_core::SourceStats::default());
        let (registry, disabled_sources) = match o.registry {
            Some(r) => (r, Vec::new()),
            None => default_registry(&config),
        };
        // `instance.operator_contact`, shared by the admin API (which changes it), the registry
        // (a source that requires it is skipped without one) and the governed client (which
        // sends it to the sources that ask for it, and to no other)
        let contact = catalog_core::OperatorContact::default();
        contact
            .set(config.instance.operator_contact.as_deref())
            .map_err(|e| anyhow::anyhow!("instance.operator_contact: {e}"))?;
        let registry = registry
            .with_stats(stats.clone())
            .with_contact(contact.clone());
        // `instance.hosting`, shared by the admin API (which changes it) and the governed
        // client (fair share of the interactive lane in hosted mode, design.md 4.1)
        let hosting = Arc::new(std::sync::RwLock::new(
            config.instance.hosting.as_str().to_string(),
        ));
        let (http, oidc_hosts): (Arc<dyn HttpFetch>, _) = if let Some(h) = o.http {
            (h, None)
        } else {
            governed_http(
                &config,
                &brand,
                &registry,
                &pool,
                budget.clone(),
                stats,
                LiveSettings {
                    hosting: hosting.clone(),
                    contact: contact.clone(),
                },
            )
        };
        let assets = Arc::new(assets::AssetService::new(
            budget.clone(),
            Arc::new(assets::pg::PgAssetRepo::new(pool.clone())),
            http.clone(),
            Arc::new(queue.clone()),
            assets::CpuPool::default_size()?,
        ));
        let auth_store = Arc::new(auth::pg::PgAuthStore::new(pool.clone(), cipher.clone()));
        let auth = auth::Auth::new(
            app.clone(),
            auth_store.clone(),
            auth_settings(&config, &brand),
        )
        .with_invites(auth_store);
        let oidc = oidc(OidcWiring {
            config: &config,
            pool: &pool,
            cipher: cipher.clone(),
            keys: keys.ring.clone(),
            clock: clock.clone(),
            http: http.clone(),
            hosts: oidc_hosts,
        })
        .await?;
        let (search, search_backend) = match o.search {
            Some(s) => (s, "override"),
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
            admin: None,
        };
        let secret_rows = admin_wiring::secret_rows(&pool, &cipher);
        admin_wiring::seal_plaintext(&secret_rows, cipher.as_ref(), &keys.source).await?;
        let api = admin_wiring::with_admin(
            api,
            admin_wiring::AdminParts {
                config: &config,
                brand: &brand,
                pool: &pool,
                keys,
                secret_rows,
                budget: budget.clone(),
                queue: queue.clone(),
                http: http.clone(),
                search_backend,
                disabled_sources,
                mailer,
                hosting,
                contact,
            },
        )
        .await?;
        // the counters other processes flushed, and the Overview rows as of now (WP4-2)
        if let Some(st) = registry.stats()
            && let Err(e) = st
                .sync(&catalog_core::pg::PgSourceStats::new(pool.clone()))
                .await
        {
            tracing::warn!(error = %e, "reading the shared source counters failed");
        }
        if api.admin.is_some()
            && let Err(e) = api::admin::run_probes(&api).await
        {
            tracing::warn!(error = %e.message, "the first health probe failed");
        }
        let trusted_proxies = Arc::new(
            api::request_meta::TrustedProxies::parse(&config.server.trusted_proxies)
                .map_err(anyhow::Error::msg)?,
        );
        let headers = Arc::new(security_headers::Headers::new(
            &config.server,
            api.oidc
                .clone()
                .map(|o| o as Arc<dyn security_headers::FormTargets>),
        ));
        Ok(Self {
            config,
            trusted_proxies,
            headers,
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
            .route(
                "/brand/fonts/{fingerprint}/{file}",
                get(serve_font).with_state(Arc::new(self.brand.clone())),
            )
            .merge(api::router(self.api.clone()));
        self.guarded(r)
    }

    /// Cross-site request protection for cookie-authenticated mutations (G2-1).
    /// Also gives every request its client address and request id (DEC-284), and every
    /// answer its security headers (SEC-1): that layer is the outermost, so a request
    /// refused by one of the others is answered with them too.
    fn guarded(&self, r: Router) -> Router {
        r.layer(axum::middleware::from_fn_with_state(
            csrf::public_origin(&self.config.server.public_url),
            csrf::guard,
        ))
        .layer(axum::middleware::from_fn_with_state(
            self.trusted_proxies.clone(),
            api::request_meta::layer,
        ))
        .layer(axum::middleware::from_fn_with_state(
            self.headers.clone(),
            security_headers::layer,
        ))
    }

    /// The job worker with every handler registered.
    #[must_use]
    pub fn worker(&self) -> jobs::Worker {
        handlers::worker(self)
    }

    /// Register the cron entries of the admin jobs (audit retention, re-wrap); the worker
    /// does it on start.
    pub async fn schedule_admin_jobs(&self) {
        handlers::schedule_admin_jobs(self).await;
    }

    /// Run the worker and the cron ticker until `stop` is cancelled.
    pub async fn run_worker(&self, stop: CancellationToken) {
        self.schedule_admin_jobs().await;
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

    /// Flush this process's data-source counters every [`STATS_SYNC_SECS`] and read back
    /// what every process recorded (DEC-247/326), until `stop`; a last flush on the way out.
    pub async fn run_stats_sync(&self, stop: CancellationToken) {
        let Some(stats) = self.registry.stats().cloned() else {
            return;
        };
        let store = catalog_core::pg::PgSourceStats::new(self.pool.clone());
        loop {
            let stopping = tokio::select! {
                () = stop.cancelled() => true,
                () = tokio::time::sleep(Duration::from_secs(STATS_SYNC_SECS)) => false,
            };
            if let Err(e) = stats.sync(&store).await {
                tracing::warn!(error = %e, "flushing the source counters failed");
            }
            if stopping {
                break;
            }
        }
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

/// design.md 7.4, DEC-540: warn about every theme colour pair (accent, soft surfaces, group
/// tints) below WCAG AA: 4.5:1 for text (1.4.3), 3:1 for bars and markers (1.4.11).
fn log_contrast(brand: &brand::Brand) {
    for f in brand.contrast_findings() {
        if let Some(ratio) = f.ratio {
            tracing::warn!(
                theme = f.theme,
                foreground_token = %f.foreground_token,
                background_token = %f.background_token,
                foreground = %f.foreground,
                background = %f.background,
                ratio = format!("{ratio:.2}"),
                required = f.required,
                "brand theme contrast is below the WCAG AA minimum (1.4.3 text 4.5:1, 1.4.11 non-text 3:1)"
            );
        } else {
            tracing::warn!(
                theme = f.theme,
                foreground_token = %f.foreground_token,
                background_token = %f.background_token,
                foreground = %f.foreground,
                background = %f.background,
                "brand theme contrast cannot be checked: use a hex value or the rgb or oklch notation"
            );
        }
    }
}

/// `/brand/fonts/{fingerprint}/{file}`: the brand's self-hosted fonts (design.md 2.8). Only
/// files declared in `brand.toml` are served; the fingerprint changes with the content.
async fn serve_font(
    State(brand): State<Arc<brand::Brand>>,
    Path((fingerprint, file)): Path<(String, String)>,
) -> Response {
    let url = format!("{}{fingerprint}/{file}", brand::FONT_ROUTE);
    match brand.font_asset(&url) {
        Some(font) => (
            [
                (header::CONTENT_TYPE, brand::FontAsset::CONTENT_TYPE),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            axum::body::Bytes::from_owner(font.bytes.clone()),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
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

/// Instance settings the governed client reads with every request.
struct LiveSettings {
    /// `instance.hosting`.
    hosting: Arc<std::sync::RwLock<String>>,
    /// `instance.operator_contact`.
    contact: catalog_core::OperatorContact,
}

/// The production `HttpFetch`: governed client with HTTP cache in the storage budget and one
/// policy (allowlist, rate limit) per connector, plus the OIDC connector whose allowlist
/// follows the configured providers at run time (the returned handle).
fn governed_http(
    config: &AppConfig,
    brand: &brand::Brand,
    registry: &Registry,
    pool: &sqlx::PgPool,
    budget: Arc<assets::StorageBudget>,
    stats: Arc<catalog_core::SourceStats>,
    live: LiveSettings,
) -> (Arc<dyn HttpFetch>, Option<Arc<catalog_core::HostList>>) {
    let cache = HttpCache {
        index: Arc::new(catalog_core::pg::PgCacheIndex::new(pool.clone())),
        blobs: budget,
    };
    let version = env!("CARGO_PKG_VERSION");
    let ua = brand.user_agent(version);
    let mut g = GovernedHttp::new(&ua, Some(cache));
    for info in registry.infos() {
        g.add_connector(&info.id, ConnectorPolicy::from_info(&info));
    }
    let mut policy = ConnectorPolicy::oidc("", None);
    policy.hosts = config
        .auth
        .oidc
        .as_ref()
        .and_then(|o| url::Url::parse(&o.issuer).ok())
        .and_then(|u| {
            let host = u.host_str()?.to_string();
            Some(match u.port() {
                Some(p) => format!("{host}:{p}"),
                None => host,
            })
        })
        .into_iter()
        .collect();
    g.add_connector(auth::oidc::CONNECTOR, policy);
    let hosts = g.host_list(auth::oidc::CONNECTOR);
    let g = g
        .with_stats(stats)
        .with_hosting(live.hosting)
        .with_contact(&format!("{}/{version}", brand.ident()), live.contact)
        // one pace per keyless source for every process on this database (BOOK-8)
        .with_shared_pace(Arc::new(catalog_core::pg::PgPace::new(pool.clone())));
    (Arc::new(g), hosts)
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

/// The governed client's run-time OIDC allowlist as the `auth` port.
struct OidcHosts(Arc<catalog_core::HostList>);

impl auth::oidc::HostAllowlist for OidcHosts {
    fn set_hosts(&self, hosts: Vec<String>) {
        self.0.set(hosts);
    }
}

struct OidcWiring<'a> {
    config: &'a AppConfig,
    pool: &'a sqlx::PgPool,
    cipher: Arc<dyn app::ports::SecretCipher>,
    keys: Arc<auth::secrets::LiveKeys>,
    clock: Arc<dyn app::ports::Clock>,
    http: Arc<dyn HttpFetch>,
    hosts: Option<Arc<catalog_core::HostList>>,
}

/// The OIDC providers (admin area plus the environment's `auth.oidc` as a read-only row).
async fn oidc(w: OidcWiring<'_>) -> anyhow::Result<Option<Arc<auth::oidc::Oidc>>> {
    use auth::oidc::{EnvProvider, Provider, RoleSource};
    let config = w.config;
    let oidc = Arc::new(auth::oidc::Oidc::new(auth::oidc::OidcParts {
        store: Arc::new(auth::oidc::PgProviderStore::new(
            w.pool.clone(),
            w.cipher.clone(),
        )),
        http: w.http,
        keys: w.keys,
        clock: w.clock.clone(),
        hosts: w
            .hosts
            .map(|h| Arc::new(OidcHosts(h)) as Arc<dyn auth::oidc::HostAllowlist>),
        audit: Some(Arc::new(auth::admin_pg::PgAdminStore::new(
            w.pool.clone(),
            w.cipher,
        ))),
        public_url: config.server.public_url.trim_end_matches('/').to_string(),
    }));
    let env = config.auth.oidc.as_ref().map(|c| {
        let now = w.clock.now();
        let mut p = Provider::new(
            auth::oidc::ENV_PROVIDER_ID,
            &c.slug,
            &c.issuer,
            &c.client_id,
            now,
        );
        p.display_name.clone_from(&c.display_name);
        p.auto_create = c.auto_create;
        p.is_default = true;
        if !c.scopes.is_empty() {
            p.scopes = std::iter::once("openid".to_string())
                .chain(c.scopes.iter().filter(|s| *s != "openid").cloned())
                .collect();
        }
        p.groups_claim.clone_from(&c.groups_claim);
        p.allowed_groups.clone_from(&c.allowed_groups);
        p.admin_groups.clone_from(&c.admin_groups);
        p.role_source = match c.role_source {
            config::OidcRoleSource::Local => RoleSource::Local,
            config::OidcRoleSource::Provider => RoleSource::Provider,
        };
        EnvProvider {
            provider: p,
            secret: c
                .client_secret
                .as_ref()
                .map(|s| zeroize::Zeroizing::new(s.expose().to_string())),
        }
    });
    oidc.install_env(env)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(Some(oidc))
}

/// The configured search backend. If Meilisearch cannot be reached at start-up the process
/// uses the Postgres fallback instead of refusing to start (logged as an error).
/// Also names the backend in use for the admin overview (`postgres_fallback` when
/// Meilisearch was configured but is not usable).
async fn search_index(
    config: &AppConfig,
    pool: &sqlx::PgPool,
) -> (Arc<dyn SearchIndex>, &'static str) {
    let pg = || -> Arc<dyn SearchIndex> { Arc::new(search::pg::PgSearch::new(pool.clone())) };
    match config.search.backend {
        config::SearchBackend::Postgres => (pg(), "postgres"),
        config::SearchBackend::Meilisearch => {
            let Some(key) = &config.search.meili_master_key else {
                tracing::error!("search.meili_master_key is not set; using Postgres search");
                return (pg(), "postgres_fallback");
            };
            match search::meili::MeiliIndex::connect(
                &config.search.meili_url,
                key.expose(),
                &config.search.index_prefix,
            )
            .await
            {
                Ok(m) => (Arc::new(m), "meilisearch"),
                Err(e) => {
                    tracing::error!(error = %e, "Meilisearch unavailable; using Postgres search");
                    (pg(), "postgres_fallback")
                }
            }
        }
    }
}

impl Server {
    fn web_backend(&self) -> Arc<dyn web::backend::Backend> {
        Arc::new(web_backend::WebBackend(self.api.clone()))
    }

    /// Per-request context for the web app: brand and the server-function backend.
    pub fn web_context(&self) -> impl Fn() + Clone + Send + Sync + 'static {
        let brand = Arc::new(self.brand.clone());
        let backend = self.web_backend();
        let admin: Arc<dyn web::admin_backend::AdminBackend> =
            Arc::new(web_admin::WebAdmin(self.api.clone()));
        move || {
            // the nonce of this request's policy, in place of the one Leptos made for itself:
            // the hydration script, the scripts of the shell and the data streamed after it
            // read it from the context
            let nonce = leptos::prelude::with_context(|p: &axum::http::request::Parts| {
                p.extensions.get::<security_headers::CspNonce>().cloned()
            });
            if let Some(Some(nonce)) = nonce {
                leptos::prelude::provide_context(leptos::nonce::Nonce::from_value(nonce.0));
            }
            leptos::prelude::provide_context(brand.clone());
            leptos::prelude::provide_context(backend.clone());
            leptos::prelude::provide_context(admin.clone());
        }
    }
}

impl Server {
    /// Everything the web role serves: base routes, server functions and the Leptos pages.
    /// Pages (and the 404 page) get the session resolved before rendering, so the app shell
    /// is right on the first render (design.md 3.2).
    pub fn app_router(&self, options: &leptos::prelude::LeptosOptions) -> Router {
        use leptos_axum::LeptosRoutes;
        let routes = page_routes();
        let context = self.web_context();
        let pages = Router::new()
            .route(
                "/_fn/{*fn_name}",
                axum::routing::any({
                    let context = context.clone();
                    move |req| leptos_axum::handle_server_fns_with_context(context.clone(), req)
                }),
            )
            .leptos_routes_with_context(options, routes, context.clone(), {
                let options = options.clone();
                move || web::shell(options.clone())
            })
            .fallback(leptos_axum::file_and_error_handler_with_context(
                context,
                web::shell,
            ))
            .with_state(options.clone())
            .layer(axum::middleware::from_fn(pkg_no_cache))
            // inner: runs after the session is resolved (signed-in visits pass untouched)
            .layer(axum::middleware::from_fn_with_state(
                SsoPages {
                    api: self.api.clone(),
                    brand: Arc::new(self.brand.clone()),
                },
                sso_redirect,
            ))
            .layer(axum::middleware::from_fn_with_state(
                self.web_backend(),
                web::backend::resolve_shell_session,
            ))
            // runs before the session is resolved: nothing is looked up for a refused body
            .layer(axum::middleware::from_fn(body_limit));
        let pages = self.guarded(pages);
        self.base_router().merge(pages)
    }
}

/// The body limit of server functions, pages and every path without a route (SEC-2): at
/// most [`api::JSON_BODY_LIMIT`] bytes, as for a JSON request to the REST API (the import
/// upload goes through REST and keeps its own limit). A request that declares more is
/// answered `413` before anything of its body is read; one that declares no length is read
/// up to the limit and then answered `413`.
///
/// The body is read here, not behind a `DefaultBodyLimit`: the server-function handler
/// takes the request as it is, and what it does with a body that is too long (a `500` with
/// the reader's error text, after reading up to axum's built-in default) is the business of
/// a library, not a rule of this server. No server function or page reads its body as a
/// stream, so nothing is lost by collecting it first.
async fn body_limit(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    use futures::StreamExt;

    const LIMIT: usize = api::JSON_BODY_LIMIT;
    let too_large = || {
        api::ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "too_large",
            "the request body is too large",
        )
        .into_response()
    };
    let declared = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if declared.is_some_and(|n| usize::try_from(n).map_or(true, |n| n > LIMIT)) {
        return too_large();
    }
    let (parts, body) = req.into_parts();
    let mut pieces = body.into_data_stream();
    let mut read = Vec::new();
    while let Some(piece) = pieces.next().await {
        let Ok(piece) = piece else {
            // the sender broke off or sent a malformed body
            return api::ApiError::bad_request("the request body could not be read")
                .into_response();
        };
        if read.len() + piece.len() > LIMIT {
            return too_large();
        }
        read.extend_from_slice(&piece);
    }
    let req = axum::extract::Request::from_parts(parts, axum::body::Body::from(read));
    next.run(req).await
}

/// Is `path` a bundle file under the name cargo-leptos gives it with `hash-files`:
/// `<name>.<hash>.<extension>`, the hash being 22 characters of the URL-safe base64
/// alphabet (128 bits of the content's hash)? Such a name never gets other content.
fn hashed_bundle_file(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or_default();
    let mut parts = file.rsplitn(3, '.');
    let (Some(_extension), Some(hash), Some(name)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    !name.is_empty()
        && hash.len() == 22
        && hash
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

/// The cache directive of the bundle under `/pkg` (OPS-8, DEC-536, DEC-623). A file with a
/// content hash in its name is kept for good (`immutable`): a new build has new names, and
/// the page (never stored) names them. Every other file under `/pkg` gets `no-cache`: a
/// browser has to ask before it reuses its copy. A name that does not exist is answered
/// with the 404 page; that is a page (a nonce, and the shell with the name of a signed-in
/// visitor), so it gets no directive here and the header layer makes it `no-store` like
/// every page (DEC-658, review SEC1-4).
///
/// "Ask first" must not mean "download again", and the Leptos file handler does not pass
/// the request's headers on to the file service, so it never answers a conditional request
/// itself (seen: `200` with the whole file for a matching `If-Modified-Since`). Hence a
/// request whose `If-Modified-Since` is exactly the `Last-Modified` of the file gets an
/// empty `304` here. Exactly, not "not newer than": browsers send back the value they were
/// given, and a release whose files carry an older date still replaces the cached one.
async fn pkg_no_cache(req: axum::extract::Request, next: axum::middleware::Next) -> Response {
    if !req.uri().path().starts_with("/pkg/") {
        return next.run(req).await;
    }
    let since = req.headers().get(header::IF_MODIFIED_SINCE).cloned();
    let hashed = hashed_bundle_file(req.uri().path());
    let mut resp = next.run(req).await;
    if !resp.headers().contains_key(header::CACHE_CONTROL)
        && !security_headers::is_document(resp.headers())
    {
        let keep = hashed && resp.status() == StatusCode::OK;
        resp.headers_mut().insert(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static(if keep {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            }),
        );
    }
    let stamp = resp.headers().get(header::LAST_MODIFIED).cloned();
    match (since, stamp) {
        (Some(since), Some(stamp)) if resp.status() == StatusCode::OK && since == stamp => {
            let cache = resp.headers().get(header::CACHE_CONTROL).cloned();
            let mut not_modified = StatusCode::NOT_MODIFIED.into_response();
            let headers = not_modified.headers_mut();
            headers.insert(header::LAST_MODIFIED, stamp);
            if let Some(cache) = cache {
                headers.insert(header::CACHE_CONTROL, cache);
            }
            not_modified
        }
        _ => resp,
    }
}

/// The app's page routes, listed once per process. Listing renders the app with a
/// process-wide flag set (`leptos_server::SuppressResourceLoad`, set by
/// `leptos_router::RouteList::generate`) under which every resource created anywhere in the
/// process waits forever; a page rendered by another router at that moment never finished
/// (DEC-384). Listing once, before the first router exists, leaves no page in flight.
fn page_routes() -> Vec<leptos_axum::AxumRouteListing> {
    static ROUTES: std::sync::OnceLock<Vec<leptos_axum::AxumRouteListing>> =
        std::sync::OnceLock::new();
    ROUTES
        .get_or_init(|| leptos_axum::generate_route_list(web::App))
        .clone()
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

/// What [`sso_redirect`] needs.
#[derive(Clone)]
struct SsoPages {
    api: api::ApiState,
    brand: Arc<brand::Brand>,
}

/// Sign-in mode `auto` (addendum-1 4.3, 4.4; phase 3 WP3-3): a signed-out visit of a page
/// answers `302` to the default provider (or the outage page) with the `role=status`
/// interstitial as body (4.5 state 2). Signed-in visits, other methods, non-page paths and
/// the exemptions of `api::auto_redirect` (`/signed-out` above all, OD-20) pass through.
async fn sso_redirect(
    State(p): State<SsoPages>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    let signed_in = req
        .extensions()
        .get::<web::backend::ShellSession>()
        .is_some();
    let page = matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && web::backend::is_page(req.uri().path());
    if signed_in || !page {
        return next.run(req).await;
    }
    let target = req
        .uri()
        .path_and_query()
        .map_or_else(|| "/".to_string(), |pq| pq.as_str().to_string());
    let Some(r) = api::auto_redirect(&p.api, req.headers(), &target).await else {
        return next.run(req).await;
    };
    let Some((provider, url)) = r.provider.clone() else {
        return r.response;
    };
    let accept = req
        .headers()
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok());
    let locale = web::i18n::negotiate(accept, &p.brand.default_locale);
    let overrides = p.brand.strings.get(&locale).cloned().unwrap_or_default();
    let html = web::auth_redirect::interstitial(&locale, overrides, &provider, &url);
    api::with_body(r, html)
}
