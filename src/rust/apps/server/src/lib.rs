//! Server wiring: one binary, `--role=all|web|worker`. Builds every adapter, plugs them into
//! the `app` core and serves the web app, the REST API, SSE and `/assets`.
#![deny(unsafe_code)]

mod admin_wiring;
pub mod book_stubs;
mod csrf;
pub mod escape;
mod handlers;
pub mod infra;
mod outbound;
pub mod read_timeout;
pub mod security_headers;
pub mod serve;
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
use catalog_core::Registry;
use catalog_core::search::SearchFanout;
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

/// What the notification engine is built with besides the configuration and the stores.
struct NotifyWith {
    clock: Arc<dyn app::ports::Clock>,
    cipher: Arc<dyn app::ports::SecretCipher>,
    /// May the mail server be on a private address (DEC-534, review SEC2-3)?
    mail: domain::net::PrivateAllow,
    /// May notification channels be on a private address?
    channels: domain::net::PrivateAllow,
}

/// The notification engine and deliverer, and the VAPID public key if web push is on.
async fn notify_engine(
    config: &AppConfig,
    brand: &brand::Brand,
    pool: &sqlx::PgPool,
    queue: &jobs::PgQueue,
    bus: &realtime::Bus,
    with: NotifyWith,
) -> anyhow::Result<NotifyParts> {
    let NotifyWith {
        clock,
        cipher,
        mail,
        channels,
    } = with;
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
    let (channels, web_push_key, mailer) = notify_channels(
        config,
        &brand.user_agent(env!("CARGO_PKG_VERSION")),
        mail,
        channels,
    )?;
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
    mail: domain::net::PrivateAllow,
    channels: domain::net::PrivateAllow,
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
    // the mail server may be on a private address only where that is allowed (DEC-534)
    let mailer = Arc::new(email::Mailer::with_private_rule(email, mail));
    Ok((
        Channels {
            http: Some(http::HttpChannels::new(
                Arc::new(catalog_core::PlainHttp::new(user_agent, channels.clone())),
                channels,
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
    // no key; board games (DEC-641); asked only with the operator contact (`Required`)
    #[cfg(feature = "wikidata")]
    all.push(Arc::new(match base("wikidata") {
        Some(b) => connectors::wikidata::Wikidata::with_base_url(&b),
        None => connectors::wikidata::Wikidata::new(),
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

/// Connect (in containers the database name may resolve only a moment after start-up) and
/// migrate.
async fn connect(config: &AppConfig) -> anyhow::Result<sqlx::PgPool> {
    let pool = store_pg::connect_with_retry(
        &config.database.url,
        config.database.max_connections,
        Duration::from_secs(60),
    )
    .await
    .context("connect to Postgres")?;
    store_pg::migrate(&pool).await.context("migrations")?;
    Ok(pool)
}

/// The storage budget; its watermark events also become admin notifications.
fn storage_budget(
    config: &AppConfig,
    pool: &sqlx::PgPool,
    bus: &realtime::Bus,
    engine: &Arc<notify::Engine>,
    clock: &Arc<dyn app::ports::Clock>,
    queue: &jobs::PgQueue,
) -> anyhow::Result<Arc<assets::StorageBudget>> {
    let budget_events =
        notify::AdminBridge::new(Arc::new(bus.clone()), engine.clone(), clock.clone());
    Ok(Arc::new(
        assets::StorageBudget::open(
            config.storage.clone(),
            Arc::new(assets::pg::PgObjectIndex::new(pool.clone())),
            clock.clone(),
            Arc::new(budget_events),
        )?
        .with_jobs(Arc::new(queue.clone())),
    ))
}

/// The adapters everything else is built on.
struct Core {
    pool: sqlx::PgPool,
    keys: admin_wiring::Keys,
    bus: realtime::Bus,
    queue: jobs::PgQueue,
    clock: Arc<dyn app::ports::Clock>,
    app: App,
    kind_ids: Vec<String>,
    outbound: outbound::Outbound,
    engine: Arc<notify::Engine>,
    deliverer: Arc<notify::Deliverer>,
    web_push_key: Option<String>,
    mailer: Arc<notify::channels::email::Mailer>,
    budget: Arc<assets::StorageBudget>,
}

/// Database, keys, events, jobs, the application, notifications and the storage budget.
async fn core(config: &AppConfig, brand: &brand::Brand, role: ServerRole) -> anyhow::Result<Core> {
    let pool = connect(config).await?;
    let keys = admin_wiring::load_keys(config, &pool).await?;
    // Web and worker in separate processes need cross-process events (LISTEN/NOTIFY).
    let bus = if role == ServerRole::All {
        realtime::Bus::local()
    } else {
        realtime::Bus::postgres(pool.clone()).await?
    };
    let queue = jobs::PgQueue::new(pool.clone());
    let clock: Arc<dyn app::ports::Clock> = Arc::new(infra::SystemClock::default());
    let (kinds, kind_ids) = enabled_kinds(config);
    // the hosting mode and the rules for private addresses (DEC-534, review SEC2-3)
    let outbound = outbound::Outbound::new(config);
    let NotifyParts {
        engine,
        deliverer,
        web_push_key,
        mailer,
    } = notify_engine(
        config,
        brand,
        &pool,
        &queue,
        &bus,
        NotifyWith {
            clock: clock.clone(),
            cipher: keys.ring.clone(),
            mail: outbound.mail(),
            channels: outbound.channels(),
        },
    )
    .await?;
    let app = App {
        repos: pg_repos(&pool),
        kinds: Arc::new(kinds),
        clock: clock.clone(),
        ids: Arc::new(infra::SystemIds),
        events: Arc::new(bus.clone()),
        jobs: Arc::new(queue.clone()),
        notifications: engine.clone(),
    };
    let budget = storage_budget(config, &pool, &bus, &engine, &clock, &queue)?;
    Ok(Core {
        pool,
        keys,
        bus,
        queue,
        clock,
        app,
        kind_ids,
        outbound,
        engine,
        deliverer,
        web_push_key,
        mailer,
        budget,
    })
}

/// Local accounts and the sign-in providers.
async fn sign_in(
    config: &AppConfig,
    brand: &brand::Brand,
    c: &Core,
    http: Arc<dyn HttpFetch>,
    oidc_hosts: Option<Arc<catalog_core::HostList>>,
) -> anyhow::Result<(auth::Auth, Option<Arc<auth::oidc::Oidc>>)> {
    let cipher: Arc<dyn app::ports::SecretCipher> = c.keys.ring.clone();
    let auth_store = Arc::new(auth::pg::PgAuthStore::new(c.pool.clone(), cipher.clone()));
    let auth = auth::Auth::new(
        c.app.clone(),
        auth_store.clone(),
        auth_settings(config, brand),
    )
    .with_invites(auth_store);
    let oidc = outbound::oidc(outbound::OidcWiring {
        config,
        pool: &c.pool,
        cipher,
        keys: c.keys.ring.clone(),
        clock: c.clock.clone(),
        http,
        hosts: oidc_hosts,
        allow_private: c.outbound.oidc(),
    })
    .await?;
    Ok((auth, oidc))
}

/// The client address behind trusted proxies, and the security headers.
fn request_guards(
    config: &AppConfig,
    api: &api::ApiState,
) -> anyhow::Result<(
    Arc<api::request_meta::TrustedProxies>,
    Arc<security_headers::Headers>,
)> {
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
    Ok((trusted_proxies, headers))
}

/// What the API state is built from besides the core.
struct ApiWith<'a> {
    config: &'a AppConfig,
    brand: &'a brand::Brand,
    core: &'a Core,
    auth: auth::Auth,
    oidc: Option<Arc<auth::oidc::Oidc>>,
    search: &'a Arc<dyn SearchIndex>,
    registry: &'a Registry,
    http: &'a Arc<dyn HttpFetch>,
    assets: &'a Arc<assets::AssetService>,
}

/// The API state without the admin area (added by `admin_wiring::with_admin`).
fn api_state(w: ApiWith<'_>) -> api::ApiState {
    let c = w.core;
    let app = &c.app;
    api::ApiState {
        app: app.clone(),
        auth: w.auth,
        oidc: w.oidc,
        bus: c.bus.clone(),
        search: w.search.clone(),
        fanout: SearchFanout::new(app.clone(), w.registry.clone(), w.http.clone()),
        registry: w.registry.clone(),
        assets: Some(w.assets.clone()),
        changes: Some(Arc::new(store_pg::PgStore::new(c.pool.clone()))),
        notify: Some(api::NotifyApi {
            engine: c.engine.clone(),
            deliverer: c.deliverer.clone(),
            web_push_key: c.web_push_key.clone(),
        }),
        integrations: Some(Arc::new(integrations::Integrations::new(
            app.clone(),
            Arc::new(integrations::pg::PgIntegrationStore::new(c.pool.clone())),
        ))),
        public_url: w.config.server.public_url.clone(),
        importer: Some(Arc::new(
            importers::Importer::new(
                app.clone(),
                Arc::new(importers::pg::PgImportStore::new(c.pool.clone())),
                c.budget.clone(),
                Arc::new(c.queue.clone()),
            )
            .with_cover_rule(cover_rule(c.outbound.hosting.clone())),
        )),
        naming: api::Naming::from_prefix(&w.brand.ident(), &w.brand.name),
        secure_cookies: w.config.auth.secure_cookies,
        default_kinds: c.kind_ids.clone(),
        admin: None,
    }
}

/// At start: the counters other processes flushed, and the Overview rows as of now (WP4-2).
async fn first_checks(registry: &Registry, pool: &sqlx::PgPool, api: &api::ApiState) {
    if let Some(st) = registry.stats()
        && let Err(e) = st
            .sync(&catalog_core::pg::PgSourceStats::new(pool.clone()))
            .await
    {
        tracing::warn!(error = %e, "reading the shared source counters failed");
    }
    if api.admin.is_some()
        && let Err(e) = api::admin::run_probes(api).await
    {
        tracing::warn!(error = %e.message, "the first health probe failed");
    }
}

impl Server {
    /// Connect, migrate and build every adapter.
    ///
    /// # Errors
    /// Unreachable database, invalid brand or storage configuration.
    // The composition root: every adapter is wired here and in the helpers above, on purpose.
    pub async fn build(config: AppConfig, role: ServerRole, o: Overrides) -> anyhow::Result<Self> {
        let brand = brand::Brand::load(std::path::Path::new(&config.brand.dir))
            .with_context(|| format!("brand directory {}", config.brand.dir))?;
        log_contrast(&brand);
        let c = core(&config, &brand, role).await?;
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
            .with_contact(contact.clone())
            // the instance language: what a refresh asks the sources in, and a search of a
            // member who left their language automatic (WD-2, LB-35)
            .with_language(&brand.default_locale);
        let (http, oidc_hosts): (Arc<dyn HttpFetch>, _) = match o.http {
            Some(h) => (h, None),
            None => outbound::governed_http(
                &config,
                outbound::GovernedWith {
                    brand: &brand,
                    registry: &registry,
                    pool: &c.pool,
                    budget: c.budget.clone(),
                    stats,
                    contact: contact.clone(),
                },
                &c.outbound,
            ),
        };
        let assets = Arc::new(assets::AssetService::new(
            c.budget.clone(),
            Arc::new(assets::pg::PgAssetRepo::new(c.pool.clone())),
            http.clone(),
            Arc::new(c.queue.clone()),
            assets::CpuPool::default_size()?,
        ));
        let (auth, oidc) = sign_in(&config, &brand, &c, http.clone(), oidc_hosts).await?;
        let (search, search_backend) = match o.search {
            Some(s) => (s, "override"),
            None => search_index(&config, &c.pool).await,
        };
        let api = api_state(ApiWith {
            config: &config,
            brand: &brand,
            core: &c,
            auth,
            oidc,
            search: &search,
            registry: &registry,
            http: &http,
            assets: &assets,
        });
        let cipher: Arc<dyn app::ports::SecretCipher> = c.keys.ring.clone();
        let secret_rows = admin_wiring::secret_rows(&c.pool, &cipher);
        admin_wiring::seal_plaintext(&secret_rows, cipher.as_ref(), &c.keys.source).await?;
        let api = admin_wiring::with_admin(
            api,
            admin_wiring::AdminParts {
                config: &config,
                brand: &brand,
                pool: &c.pool,
                keys: c.keys,
                secret_rows,
                budget: c.budget.clone(),
                queue: c.queue.clone(),
                http: http.clone(),
                search_backend,
                disabled_sources,
                mailer: c.mailer,
                hosting: c.outbound.hosting.clone(),
                contact,
            },
        )
        .await?;
        first_checks(&registry, &c.pool, &api).await;
        let (trusted_proxies, headers) = request_guards(&config, &api)?;
        Ok(Self {
            config,
            trusted_proxies,
            headers,
            brand,
            app: c.app,
            api,
            queue: c.queue,
            budget: c.budget,
            assets,
            registry,
            http,
            bus: c.bus,
            pool: c.pool,
            search,
            notify: c.engine,
            deliverer: c.deliverer,
        })
    }

    /// HTTP routes that do not need Leptos: health, REST API, SSE, assets.
    pub fn base_router(&self) -> Router {
        let r = Router::new()
            .route("/healthz", get(|| async { "ok" }))
            .route(
                "/about/notices.txt",
                get(serve_notices).with_state(Arc::new(std::path::PathBuf::from(
                    &self.config.server.notices_path,
                ))),
            )
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
    /// refused by one of the others is answered with them too. Innermost, a request body
    /// gets its time limits (DEC-653): they run when a handler reads it.
    fn guarded(&self, r: Router) -> Router {
        r.layer(axum::middleware::from_fn_with_state(
            read_timeout::ReadLimits::from_config(&self.config.server),
            read_timeout::layer,
        ))
        .layer(axum::middleware::from_fn_with_state(
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

/// `/about/notices.txt`: the licence notices of the build's third-party parts (#18, DEC-855),
/// read from the file the image carries; `404` when the build has none (development).
async fn serve_notices(State(path): State<Arc<std::path::PathBuf>>) -> Response {
    match tokio::fs::read(path.as_path()).await {
        Ok(text) => (
            [
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (header::CACHE_CONTROL, "public, max-age=3600"),
            ],
            text,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
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

/// Who may bring a picture address in with an imported item entered by hand: the rule of
/// `POST /items` (`api::ops::may_edit_items`), everybody on a home instance and admins on a
/// hosted one, read per row from the shared `instance.hosting` (review MAN-3 of v1-manual).
fn cover_rule(hosting: Arc<std::sync::RwLock<String>>) -> importers::CoverRule {
    Arc::new(move |a: domain::user::Actor| {
        a.is_admin()
            || *hosting
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                != "hosted"
    })
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
        throttle: {
            let t = &config.auth.throttle;
            auth::ThrottleSettings {
                free_failures: t.free_failures,
                delay_base_secs: t.delay_base_secs,
                delay_max_secs: t.delay_max_secs,
                window_minutes: t.window_minutes,
                address_failures: t.address_failures,
                verify_concurrency: t.verify_concurrency,
            }
        },
    }
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
/// answered `413` before anything of its body is read.
///
/// Only the body of a server function (`/_fn/`) is read here, up to the limit, and then
/// answered `413` if there is more: the server-function handler takes the request as it
/// is, and what it does with a body that is too long (a `500` with the reader's error
/// text, after reading up to axum's built-in default) is the business of a library, not a
/// rule of this server. Every other path of this router (pages, files, paths without a
/// route) reads no body, so theirs is handed on unread, behind the same limit should
/// something read it after all: a body that stalls there holds no memory (DEC-653). How
/// long a body may take is the business of [`read_timeout`].
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
    if !req.uri().path().starts_with("/_fn/") {
        let req = req.map(|body| axum::body::Body::new(http_body_util::Limited::new(body, LIMIT)));
        return next.run(req).await;
    }
    let (parts, body) = req.into_parts();
    let mut pieces = body.into_data_stream();
    let mut read = Vec::new();
    while let Some(piece) = pieces.next().await {
        let Ok(piece) = piece else {
            // the sender broke off, sent a malformed body or was too slow (then the
            // read-timeout layer answers 408 in place of this)
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, RwLock};

    use domain::ids::UserId;
    use domain::user::{Actor, Role};

    #[test]
    fn an_import_takes_a_picture_address_as_post_items_does() {
        let actor = |role| Actor {
            user_id: UserId(uuid::Uuid::nil()),
            role,
        };
        let hosting = Arc::new(RwLock::new("home".to_string()));
        let rule = super::cover_rule(hosting.clone());
        assert!(rule(actor(Role::Member)), "a member of a home instance");
        assert!(rule(actor(Role::Admin)));
        // the mode is read per row: a change in the admin area counts at once
        *hosting.write().unwrap() = "hosted".into();
        assert!(!rule(actor(Role::Member)), "a member of a hosted instance");
        assert!(rule(actor(Role::Admin)));
    }
}
