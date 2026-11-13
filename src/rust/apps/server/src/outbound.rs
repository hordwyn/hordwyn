//! The wiring of everything the server calls (SEC-4, DEC-534): the governed client of the
//! data sources and sign-in providers, the sign-in providers themselves, and the rules for
//! private addresses that the mail server and notification channels follow.
//!
//! One operator list, `outbound.allow_private_hosts`, is honoured by every outgoing request
//! (review SEC2-3): data sources, sign-in providers (discovery, keys, token, user info, by
//! name or as an IP address), the mail server and notification channels. On top of it:
//! connector base URLs and `[notify.smtp]` from file or environment name their host on
//! purpose and count as listed for their own feature; `notify.allow_private_targets`, or
//! else the hosting mode, allows sign-in providers and mail on any private address; for
//! notification channels only the setting itself does.

use std::sync::Arc;

use app::ports::HttpFetch;
use catalog_core::cache::HttpCache;
use catalog_core::{ConnectorPolicy, GovernedHttp, Registry};
use config::AppConfig;
use domain::net::{PrivateAllow, PrivateHosts};

/// May sign-in providers and the mail server be on any private address right now?
type PrivateTargets = Arc<dyn Fn() -> bool + Send + Sync>;

/// The rules for private addresses, read per request.
#[derive(Clone)]
pub(crate) struct Outbound {
    /// `instance.hosting`, shared by the admin API (which changes it), the governed client
    /// (fair share of the interactive lane in hosted mode, design.md 4.1) and the rule for
    /// private targets.
    pub(crate) hosting: Arc<std::sync::RwLock<String>>,
    private_targets: PrivateTargets,
    /// `outbound.allow_private_hosts`.
    listed: Vec<String>,
    /// `notify.allow_private_targets`, for notification channels (not set: no).
    channels_any: bool,
    /// `host:port` of `[notify.smtp]` from file or environment.
    smtp_host: Option<String>,
}

impl Outbound {
    pub(crate) fn new(config: &AppConfig) -> Self {
        let hosting = Arc::new(std::sync::RwLock::new(
            config.instance.hosting.as_str().to_string(),
        ));
        Self {
            private_targets: private_targets(config, hosting.clone()),
            hosting,
            listed: config.outbound.allow_private_hosts.clone(),
            channels_any: config.notify.allow_private_targets.unwrap_or(false),
            smtp_host: config.notify.smtp.as_ref().map(|s| {
                let host = match s.host.parse::<std::net::Ipv6Addr>() {
                    Ok(v6) => format!("[{v6}]"),
                    Err(_) => s.host.clone(),
                };
                format!("{host}:{}", s.port)
            }),
        }
    }

    /// Sign-in providers: the operator's list, or any private address where allowed.
    pub(crate) fn oidc(&self) -> PrivateAllow {
        PrivateAllow::new(
            self.private_targets.clone(),
            PrivateHosts::new(&self.listed),
        )
    }

    /// The mail server: as sign-in providers, and the host of `[notify.smtp]` from file or
    /// environment (an admin-set mail host is not on the list by itself).
    pub(crate) fn mail(&self) -> PrivateAllow {
        PrivateAllow::new(
            self.private_targets.clone(),
            PrivateHosts::new(self.listed.iter().chain(&self.smtp_host)),
        )
    }

    /// Notification channels: the operator's list, or any private address when
    /// `notify.allow_private_targets` is `true`. Members name these targets, so neither the
    /// hosting mode nor the operator's other hosts open them.
    pub(crate) fn channels(&self) -> PrivateAllow {
        let any = self.channels_any;
        PrivateAllow::new(Arc::new(move || any), PrivateHosts::new(&self.listed))
    }
}

/// DEC-534: `notify.allow_private_targets` when it is set, else by the hosting mode as it is
/// at the time of the call: yes on a home instance, no on a hosted one.
fn private_targets(config: &AppConfig, hosting: Arc<std::sync::RwLock<String>>) -> PrivateTargets {
    let fixed = config.notify.allow_private_targets;
    Arc::new(move || {
        fixed.unwrap_or_else(|| {
            hosting
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_str()
                != config::Hosting::Hosted.as_str()
        })
    })
}

/// The hosts that may be on a private address for any connector: the operator's list and
/// the host of every `connectors.<id>.base_url`, which only the operator can set (file or
/// environment) and which names the host on purpose.
pub(crate) fn private_hosts(config: &AppConfig) -> Vec<String> {
    let overrides = config.connectors.values().filter_map(|c| {
        let u = url::Url::parse(c.base_url.as_deref()?).ok()?;
        let host = u.host_str()?.to_string();
        Some(match u.port() {
            Some(p) => format!("{host}:{p}"),
            None => host,
        })
    });
    config
        .outbound
        .allow_private_hosts
        .iter()
        .cloned()
        .chain(overrides)
        .collect()
}

/// What the governed client is built from besides the configuration.
pub(crate) struct GovernedWith<'a> {
    pub(crate) brand: &'a brand::Brand,
    pub(crate) registry: &'a Registry,
    pub(crate) pool: &'a sqlx::PgPool,
    pub(crate) budget: Arc<assets::StorageBudget>,
    pub(crate) stats: Arc<catalog_core::SourceStats>,
    /// `instance.operator_contact`, shared with the admin API (which changes it).
    pub(crate) contact: catalog_core::OperatorContact,
}

/// The production `HttpFetch`: governed client with HTTP cache in the storage budget and one
/// policy (allowlist, rate limit) per connector, plus the OIDC connector whose allowlist
/// follows the configured providers at run time (the returned handle).
/// Confinement (SEC-4): a data source reaches only its documented hosts, redirects included,
/// and no private address except the hosts of [`private_hosts`]; the OIDC connector also
/// any private address where [`Outbound::oidc`] allows them as a whole.
pub(crate) fn governed_http(
    config: &AppConfig,
    w: GovernedWith<'_>,
    outbound: &Outbound,
) -> (Arc<dyn HttpFetch>, Option<Arc<catalog_core::HostList>>) {
    let cache = HttpCache {
        index: Arc::new(catalog_core::pg::PgCacheIndex::new(w.pool.clone())),
        blobs: w.budget,
    };
    let version = env!("CARGO_PKG_VERSION");
    let ua = w.brand.user_agent(version);
    let mut g = GovernedHttp::new(&ua, Some(cache)).with_private_hosts(private_hosts(config));
    for info in w.registry.infos() {
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
    let private_targets = outbound.private_targets.clone();
    policy.private = catalog_core::PrivateRule::when(move || private_targets());
    g.add_connector(auth::oidc::CONNECTOR, policy);
    let hosts = g.host_list(auth::oidc::CONNECTOR);
    let g = g
        .with_stats(w.stats)
        .with_hosting(outbound.hosting.clone())
        .with_contact(&format!("{}/{version}", w.brand.ident()), w.contact)
        // one pace per keyless source for every process on this database (BOOK-8)
        .with_shared_pace(Arc::new(catalog_core::pg::PgPace::new(w.pool.clone())));
    (Arc::new(g), hosts)
}

/// The governed client's run-time OIDC allowlist as the `auth` port.
struct OidcHosts(Arc<catalog_core::HostList>);

impl auth::oidc::HostAllowlist for OidcHosts {
    fn set_hosts(&self, hosts: Vec<String>) {
        self.0.set(hosts);
    }
}

pub(crate) struct OidcWiring<'a> {
    pub(crate) config: &'a AppConfig,
    pub(crate) pool: &'a sqlx::PgPool,
    pub(crate) cipher: Arc<dyn app::ports::SecretCipher>,
    pub(crate) keys: Arc<auth::secrets::LiveKeys>,
    pub(crate) clock: Arc<dyn app::ports::Clock>,
    pub(crate) http: Arc<dyn HttpFetch>,
    pub(crate) hosts: Option<Arc<catalog_core::HostList>>,
    pub(crate) allow_private: PrivateAllow,
}

/// The OIDC providers (admin area plus the environment's `auth.oidc` as a read-only row).
pub(crate) async fn oidc(w: OidcWiring<'_>) -> anyhow::Result<Option<Arc<auth::oidc::Oidc>>> {
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
        allow_private: w.allow_private,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Review SEC2-3: one operator list for every kind of outgoing request; the mail host of
    /// the configuration counts as listed for mail only; notification channels do not
    /// follow the hosting mode.
    #[test]
    fn one_list_for_every_outgoing_request() {
        let cfg = AppConfig::from_sources(
            "[instance]\nhosting = \"hosted\"\n[outbound]\nallow_private_hosts = [\"idp.lan\", \"hook.lan:8080\"]\n\
             [notify.smtp]\nhost = \"relay.lan\"\nport = 25\nfrom = \"x <x@example.org>\"\n",
            Vec::new(),
        )
        .unwrap();
        let o = Outbound::new(&cfg);
        assert!(o.oidc().host("idp.lan", None));
        assert!(!o.oidc().host("other.lan", None), "hosted: refused");
        assert!(o.mail().host("relay.lan", Some(25)));
        assert!(
            !o.oidc().host("relay.lan", Some(25)),
            "the mail host is for mail"
        );
        assert!(o.channels().host("hook.lan", Some(8080)));
        assert!(!o.channels().host("relay.lan", Some(25)));
        // a home instance allows providers and mail anywhere, channels still only listed
        *o.hosting.write().unwrap() = "home".into();
        assert!(o.oidc().host("other.lan", None));
        assert!(o.mail().host("other.lan", Some(587)));
        assert!(!o.channels().host("other.lan", None));
    }
}
