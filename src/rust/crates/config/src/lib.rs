//! Typed configuration: a TOML file plus `APP__SECTION__KEY` environment overrides, validated
//! at startup. The configuration reference page in the docs is generated from these structs
//! ([`reference_adoc`]); a test fails when the committed page is stale.
#![deny(unsafe_code)]

mod reference;

use std::collections::BTreeMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use assets::StorageConfig;
pub use reference::reference_adoc;

/// A secret value: deserialised from config or env, never serialised, `Debug` prints `***`.
#[derive(Clone, PartialEq, Eq, Default, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

/// Prefix of environment overrides: `APP__DATABASE__URL` sets `database.url`.
pub const ENV_PREFIX: &str = "APP__";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("reading {0}: {1}")]
    Read(String, std::io::Error),
    #[error("invalid configuration: {0}")]
    Parse(String),
    #[error("invalid configuration: {0}")]
    Invalid(String),
}

/// Which parts of the server a process runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServerRole {
    /// Web and worker in one process.
    #[default]
    All,
    /// HTTP only (pages, API, SSE).
    Web,
    /// Background jobs only.
    Worker,
}

impl ServerRole {
    #[must_use]
    pub fn runs_web(self) -> bool {
        matches!(self, Self::All | Self::Web)
    }

    #[must_use]
    pub fn runs_worker(self) -> bool {
        matches!(self, Self::All | Self::Worker)
    }
}

impl std::str::FromStr for ServerRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "all" => Ok(Self::All),
            "web" => Ok(Self::Web),
            "worker" => Ok(Self::Worker),
            _ => Err(format!("unknown role `{s}` (all, web, worker)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Address the HTTP server listens on.
    pub bind: String,
    /// Public base URL (links in emails, OIDC redirect, share links).
    pub public_url: String,
    /// Default role; the `--role` flag overrides it.
    pub role: ServerRole,
    /// Optional CDN base URL for `/assets/...` (D-7).
    pub assets_base_url: Option<String>,
    /// Reverse proxies (addresses or CIDR networks, e.g. `10.0.0.0/8`) whose `X-Forwarded-For`
    /// and `X-Request-Id` are believed for audit rows (DEC-284). Empty: the socket address.
    pub trusted_proxies: Vec<String>,
    /// Seconds a browser remembers to use HTTPS only for this host
    /// (`Strict-Transport-Security: max-age=...`). The header is sent only when `public_url`
    /// starts with `https://`; `0` never sends it (for a proxy that sets its own, or while an
    /// address is still being tried out: a browser that saw the header refuses plain HTTP
    /// for that host until the time is over).
    pub hsts_max_age_secs: u64,
    /// Send the content security policy as `Content-Security-Policy-Report-Only`: the
    /// browser reports what the policy would refuse in its console and refuses nothing.
    /// For finding out whether the policy is the reason something does not load.
    pub csp_report_only: bool,
    /// Seconds a client may take to send the head of a request (request line and headers);
    /// then the connection is closed. Also how long an idle keep-alive connection is kept:
    /// keep it above the time the reverse proxy keeps idle connections to the application
    /// (Caddy 2 minutes, nginx 60 seconds by default), or the proxy can send a request into
    /// a connection that is just being closed. The default is above both.
    pub header_read_timeout_secs: u64,
    /// Seconds a request body may pause between two pieces; then the request is answered
    /// `408`. Applies to every request, the import upload included.
    pub body_idle_timeout_secs: u64,
    /// Seconds the whole body of a request may take to arrive; then the request is answered
    /// `408`. The import upload has its own limit (`import_read_timeout_secs`).
    pub body_read_timeout_secs: u64,
    /// Seconds the body of an import upload (up to 64 MiB) may take to arrive; then the
    /// request is answered `408`. The default allows 64 MiB at about 0.3 Mbit/s.
    pub import_read_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:3000".into(),
            public_url: "http://localhost:3000".into(),
            role: ServerRole::All,
            assets_base_url: None,
            trusted_proxies: Vec::new(),
            hsts_max_age_secs: 31_536_000,
            csp_report_only: false,
            header_read_timeout_secs: 150,
            body_idle_timeout_secs: 30,
            body_read_timeout_secs: 60,
            import_read_timeout_secs: 1800,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct DatabaseConfig {
    /// Postgres connection URL.
    pub url: String,
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgres://app@localhost:5432/app".into(),
            max_connections: 20,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SearchBackend {
    #[default]
    Meilisearch,
    /// Postgres full-text search + `pg_trgm` (no Meilisearch needed).
    Postgres,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    pub backend: SearchBackend,
    pub meili_url: String,
    /// Master key; only the server uses it, clients never see it (D-13). Set it through
    /// `APP__SEARCH__MEILI_MASTER_KEY`.
    #[serde(skip_serializing)]
    pub meili_master_key: Option<Secret>,
    /// Prefix for index names, so several instances can share one Meilisearch.
    pub index_prefix: String,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            backend: SearchBackend::Meilisearch,
            meili_url: "http://localhost:7700".into(),
            meili_master_key: None,
            index_prefix: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignupMode {
    /// Anyone can create an account.
    Open,
    /// Only with an invite (companion promotion, admin invite).
    #[default]
    Invite,
    /// No new accounts (the first account is always allowed and becomes admin).
    Closed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OidcConfig {
    /// Issuer URL used for discovery (`/.well-known/openid-configuration`).
    pub issuer: String,
    pub client_id: String,
    /// Set through `APP__AUTH__OIDC__CLIENT_SECRET`.
    #[serde(skip_serializing)]
    pub client_secret: Option<Secret>,
    /// Button label on the sign-in page.
    pub display_name: String,
    /// Create an account on first OIDC sign-in even when sign-up is not open.
    pub auto_create: bool,
    /// Short name in URLs (`/api/v1/auth/oidc/start?provider=<slug>`).
    pub slug: String,
    /// Scopes to request; `openid` is always added. Empty: `openid profile email`.
    pub scopes: Vec<String>,
    /// Claim that lists the groups (ID token, else `UserInfo`).
    pub groups_claim: String,
    /// Only members of these groups may sign in (empty: everyone).
    pub allowed_groups: Vec<String>,
    /// Members of these groups are admins when `role_source = "provider"`.
    pub admin_groups: Vec<String>,
    /// `local` (roles are set in the admin area) or `provider` (recomputed from
    /// `admin_groups` at every sign-in; needs `admin_groups`, OD-16).
    pub role_source: OidcRoleSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OidcRoleSource {
    #[default]
    Local,
    Provider,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            client_id: String::new(),
            client_secret: None,
            display_name: "Single sign-on".into(),
            auto_create: false,
            slug: "sso".into(),
            scopes: Vec::new(),
            groups_claim: "groups".into(),
            allowed_groups: Vec::new(),
            admin_groups: Vec::new(),
            role_source: OidcRoleSource::Local,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AuthConfig {
    pub signup: SignupMode,
    /// Session lifetime in days (sliding).
    pub session_days: u32,
    /// Send cookies with `Secure` (disable only for plain-HTTP development).
    pub secure_cookies: bool,
    pub oidc: Option<OidcConfig>,
    /// `page` (sign-in page) or `auto` (straight to the default identity provider,
    /// addendum-1 4.3; needs a tested provider). `cli admin disable-sso-only` sets `page`.
    pub sign_in_mode: SignInMode,
    /// How guesses at passwords and codes are slowed down.
    pub throttle: ThrottleConfig,
}

/// Limits for every check of a password, a one-time code, a recovery code, a reset link or
/// an emailed sign-in link. Nothing here locks an account: the pair of account and client
/// address waits longer after each failure, and a client address has a budget across all
/// names. The client address is the one `server.trusted_proxies` yields. Counted per web
/// process, in memory. Read at start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ThrottleConfig {
    /// Failures of one account from one client address that cost no wait.
    pub free_failures: u32,
    /// Seconds to wait after the first failure beyond the free ones; the wait doubles with
    /// each further failure. `0` switches the wait off.
    pub delay_base_secs: u32,
    /// The longest wait in seconds.
    pub delay_max_secs: u32,
    /// Minutes without a failure after which an account starts again for an address; also
    /// the length of the budget window of a client address.
    pub window_minutes: u32,
    /// Failures one client address may have across all names within the window; then it is
    /// refused for every name until the window ends. Behind a reverse proxy that is not in
    /// `server.trusted_proxies` all visitors share the proxy's address and this budget.
    pub address_failures: u32,
    /// Password verifications (argon2id, about 19 MiB each) that run at once; further ones
    /// wait their turn.
    pub verify_concurrency: u32,
}

impl Default for ThrottleConfig {
    fn default() -> Self {
        Self {
            free_failures: 3,
            delay_base_secs: 1,
            delay_max_secs: 300,
            window_minutes: 15,
            address_failures: 30,
            verify_concurrency: 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SignInMode {
    /// The sign-in page with provider buttons first.
    #[default]
    Page,
    /// Go straight to the default provider.
    Auto,
}

impl SignInMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Auto => "auto",
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            signup: SignupMode::Invite,
            session_days: 30,
            secure_cookies: true,
            oidc: None,
            sign_in_mode: SignInMode::Page,
            throttle: ThrottleConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BrandConfig {
    /// Directory with `brand.toml`, logos and i18n overrides (D-10).
    pub dir: String,
}

impl Default for BrandConfig {
    fn default() -> Self {
        Self {
            dir: "config/brand/default".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ConnectorConfig {
    /// Compiled-in connectors are on unless switched off here.
    pub enabled: bool,
    /// API key or token. Prefer `APP__CONNECTORS__<ID>__API_KEY`; `SOURCE_<ID>_API_KEY` is
    /// read as a fallback.
    #[serde(skip_serializing)]
    pub api_key: Option<Secret>,
    /// Override of the API base URL (tests, mirrors). Its host is added to the allowlist.
    pub base_url: Option<String>,
}

impl Default for ConnectorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            api_key: None,
            base_url: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct JobsConfig {
    /// Concurrent jobs per worker process.
    pub concurrency: u32,
    /// Seconds a stopping worker waits for running jobs.
    pub shutdown_grace_secs: u64,
    /// Days after which catalog data from a source whose terms limit caching (TMDB: six
    /// months) is refreshed by the daily retention job, or purged when no refresh is possible
    /// (no key, source switched off). 1 to 170; such data is purged at 175 days in any case.
    pub catalog_max_age_days: u32,
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            shutdown_grace_secs: 20,
            catalog_max_age_days: 150,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SmtpSecurity {
    /// TLS from the first byte (usually port 465).
    Tls,
    /// STARTTLS upgrade (usually port 587).
    #[default]
    Starttls,
    /// No encryption: only for a relay on localhost or in a private network.
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SmtpConfig {
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    #[serde(default)]
    pub security: SmtpSecurity,
    #[serde(default)]
    pub username: Option<String>,
    /// Secret: set it with `APP__NOTIFY__SMTP__PASSWORD`.
    #[serde(default, skip_serializing)]
    pub password: Option<Secret>,
    /// Sender, e.g. `Shelf <noreply@example.org>`.
    pub from: String,
}

const fn default_smtp_port() -> u16 {
    587
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct NotifyConfig {
    /// May the server call targets in private and loopback networks that people name in the
    /// application? Not set: notification channels (ntfy, Gotify, webhooks, ...) may not,
    /// otherwise any member could make the server call internal services; sign-in providers
    /// (OIDC discovery, token, keys, user info) and the mail server follow
    /// `instance.hosting`: allowed in `home`, refused in `hosted`. `true` allows all of
    /// them, `false` refuses all of them, in either hosting mode.
    pub allow_private_targets: Option<bool>,
    /// Attempts per delivery before it is marked failed.
    pub max_attempts: u32,
    /// Email channel; email notifications are unavailable without it.
    pub smtp: Option<SmtpConfig>,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            allow_private_targets: None,
            max_attempts: 5,
            smtp: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TelemetryConfig {
    pub log_format: LogFormat,
    /// `tracing` filter directives, e.g. `info,sqlx=warn`. `RUST_LOG` overrides it.
    pub filter: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_format: LogFormat::Text,
            filter: "info,sqlx=warn".into(),
        }
    }
}

/// How the instance is run (design.md 4.1): it changes defaults, not features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Hosting {
    /// Free, for family and friends (the default, DEC-171).
    #[default]
    Home,
    /// Run for other people: licence reminders per data source.
    Hosted,
}

impl Hosting {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Hosted => "hosted",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct InstanceConfig {
    /// `home` or `hosted`. When set here or in the environment the admin area shows it
    /// read-only; otherwise the setup wizard and Admin > Instance store it in the database.
    pub hosting: Hosting,
    /// Contact of the person who runs the instance: an e-mail address (preferred) or an
    /// https URL, at most 200 characters. Sent in the `User-Agent` of requests to the data
    /// sources that ask clients to identify themselves (Open Library, Wikidata), to no other
    /// source; Wikidata is not asked without one.
    /// Short form: `SOURCE_OPERATOR_CONTACT`. When set here or in the environment the admin area
    /// shows it read-only; otherwise Admin > Data sources stores it in the database.
    pub operator_contact: Option<String>,
}

/// Encryption of secrets at rest (addendum-1 section 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct SecretsConfig {
    /// Key file, one key per line (`v<n>:<base64 of 32 bytes>`, newest first). Generated with
    /// mode 0600 on the first start when neither the file nor `keys` exists. Back it up
    /// together with the database.
    pub key_file: String,
    /// The same lines joined with `,`, for operators who prefer the environment
    /// (`APP__SECRETS__KEYS`). The key file wins when both exist.
    #[serde(skip_serializing)]
    pub keys: Option<Secret>,
    /// Use a temporary in-memory key when neither the key file nor `keys` exists. For test
    /// harnesses only: values encrypted under it cannot be read after a restart, so start-up
    /// still refuses when the database already holds encrypted values, and plain-text values
    /// from earlier versions are left unencrypted. Never implied by the storage backend.
    pub ephemeral: bool,
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            key_file: "data/secrets.key".into(),
            keys: None,
            ephemeral: false,
        }
    }
}

/// Where an explicitly set value came from (the admin area shows env and file values
/// read-only, with the variable name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    File,
    /// The environment variable that set it.
    Env(String),
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct FeaturesConfig {
    /// Enabled media kinds; empty = all built-in kinds (D-10 feature toggles).
    pub kinds: Vec<String>,
}

/// First-party usage measurement (WP4-5, design.md 9, research R-54).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct UsageConfig {
    /// Count the logging funnel: how many taps and how long logging takes, and which
    /// settings sections people open, as daily totals without any user or item, kept 180
    /// days. When set here or in the environment (`APP__USAGE__FUNNEL`) the admin area shows
    /// it read-only; otherwise Admin > Storage and jobs can turn it off.
    pub funnel: bool,
}

impl Default for UsageConfig {
    fn default() -> Self {
        Self { funnel: true }
    }
}

/// What the server may call. Data sources and cover downloads only reach the hosts their
/// connector documents (redirects are checked hop by hop against the same list), sign-in
/// providers and mail the configured hosts, and none of them an address in a private,
/// loopback or link-local network unless it is allowed: here per host (for every kind of
/// outgoing request), by `connectors.<id>.base_url` and `notify.smtp.host` for the host
/// they name, or by `notify.allow_private_targets` for sign-in providers, mail and
/// notification channels as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct OutboundConfig {
    /// Hosts that may be on a private, loopback or link-local address: `host` (the default
    /// port of the URL; for mail the ports 25, 465 and 587), `host:port`, an IP address
    /// (`10.0.0.5:8443`, `[fd00::1]:8080`), or `*.home.arpa` for every subdomain of a
    /// domain. Honoured by data sources, sign-in providers (discovery, keys, token, user
    /// info), the mail server and notification channels (webhooks, ntfy, ...): for example
    /// a metadata mirror, a sign-in provider or a mail relay in the local network of a
    /// hosted instance, or one webhook target in the LAN without allowing all of them. A
    /// host still has to be one the feature may call at all. Compared in lower case.
    pub allow_private_hosts: Vec<String>,
}

/// The whole configuration.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub search: SearchConfig,
    pub storage: StorageConfig,
    pub auth: AuthConfig,
    pub brand: BrandConfig,
    pub jobs: JobsConfig,
    pub telemetry: TelemetryConfig,
    pub features: FeaturesConfig,
    pub notify: NotifyConfig,
    /// Per connector id (`tmdb`, `igdb`, ...).
    pub connectors: BTreeMap<String, ConnectorConfig>,
    pub instance: InstanceConfig,
    pub secrets: SecretsConfig,
    pub usage: UsageConfig,
    pub outbound: OutboundConfig,
    /// Dotted keys set by the file or the environment (not part of the file format).
    #[serde(skip)]
    #[schemars(skip)]
    pub origins: BTreeMap<String, Origin>,
}

/// Parse an environment value as a TOML value (number, bool, array), else a string.
fn env_value(raw: &str) -> toml::Value {
    toml::from_str::<toml::Table>(&format!("v = {raw}"))
        .ok()
        .and_then(|mut t| t.remove("v"))
        .unwrap_or_else(|| toml::Value::String(raw.to_string()))
}

/// Dotted paths of every value in a TOML table (`auth.session_days`).
fn leaf_paths(table: &toml::Table, prefix: &str, out: &mut BTreeMap<String, Origin>) {
    for (k, v) in table {
        let path = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        if let toml::Value::Table(t) = v {
            leaf_paths(t, &path, out);
        } else {
            out.insert(path, Origin::File);
        }
    }
}

/// Short form of `APP__INSTANCE__OPERATOR_CONTACT`.
pub const OPERATOR_CONTACT_ENV: &str = "SOURCE_OPERATOR_CONTACT";

/// Prefix of the short variables of the data sources: `SOURCE_<ID>_API_KEY` and
/// [`OPERATOR_CONTACT_ENV`] (v1-release item 2: no brand in it, and no alias for the names
/// before v1).
pub const SOURCE_ENV_PREFIX: &str = "SOURCE_";

/// Apply `APP__A__B=value` overrides onto a TOML table, recording each key's variable.
fn apply_env(
    table: &mut toml::Table,
    vars: impl IntoIterator<Item = (String, String)>,
    origins: &mut BTreeMap<String, Origin>,
) {
    for (k, v) in vars {
        let Some(path) = k.strip_prefix(ENV_PREFIX) else {
            continue;
        };
        // an empty value means "not set" (a sourced `.env.example` lists every name empty)
        if v.trim().is_empty() {
            continue;
        }
        let parts: Vec<String> = path.split("__").map(str::to_ascii_lowercase).collect();
        if parts.iter().any(String::is_empty) {
            continue;
        }
        let mut cur = &mut *table;
        for p in &parts[..parts.len() - 1] {
            let entry = cur
                .entry(p.clone())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            if !entry.is_table() {
                *entry = toml::Value::Table(toml::Table::new());
            }
            cur = entry.as_table_mut().expect("table");
        }
        cur.insert(parts[parts.len() - 1].clone(), env_value(&v));
        origins.insert(parts.join("."), Origin::Env(k.clone()));
    }
}

impl AppConfig {
    /// Load from an optional TOML file plus the process environment.
    ///
    /// # Errors
    /// Unreadable file, parse errors, or failed validation.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let text = match path {
            Some(p) => std::fs::read_to_string(p)
                .map_err(|e| ConfigError::Read(p.display().to_string(), e))?,
            None => String::new(),
        };
        Self::from_sources(&text, std::env::vars())
    }

    /// Build from TOML text and environment variables (testable without the process env).
    ///
    /// # Errors
    /// Parse errors or failed validation.
    pub fn from_sources(
        toml_text: &str,
        env: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, ConfigError> {
        let env: Vec<(String, String)> = env.into_iter().collect();
        let mut table: toml::Table =
            toml::from_str(toml_text).map_err(|e| ConfigError::Parse(e.to_string()))?;
        let mut origins = BTreeMap::new();
        leaf_paths(&table, "", &mut origins);
        apply_env(&mut table, env.iter().cloned(), &mut origins);
        let mut cfg: Self = toml::Value::Table(table)
            .try_into()
            .map_err(|e: toml::de::Error| ConfigError::Parse(e.to_string()))?;
        // Short key variables (`SOURCE_TMDB_API_KEY`, KICKOFF section 5) as fallback.
        for (k, v) in &env {
            if let Some(id) = k
                .strip_prefix(SOURCE_ENV_PREFIX)
                .and_then(|r| r.strip_suffix("_API_KEY"))
            {
                let c = cfg
                    .connectors
                    .entry(id.to_ascii_lowercase())
                    .or_insert(ConnectorConfig {
                        enabled: true,
                        ..ConnectorConfig::default()
                    });
                if c.api_key.is_none() && !v.is_empty() {
                    c.api_key = Some(Secret::new(v.clone()));
                    origins.insert(
                        format!("connectors.{}.api_key", id.to_ascii_lowercase()),
                        Origin::Env(k.clone()),
                    );
                }
            }
        }
        // the short form of `instance.operator_contact`
        if cfg.instance.operator_contact.is_none()
            && let Some((k, v)) = env
                .iter()
                .find(|(k, v)| k == OPERATOR_CONTACT_ENV && !v.trim().is_empty())
        {
            cfg.instance.operator_contact = Some(v.trim().to_string());
            origins.insert("instance.operator_contact".into(), Origin::Env(k.clone()));
        }
        cfg.origins = origins;
        // host names are compared in lower case (review SEC2-11)
        for h in &mut cfg.outbound.allow_private_hosts {
            *h = h.trim().to_ascii_lowercase();
        }
        cfg.validate()?;
        Ok(cfg)
    }

    /// Where `key` (dotted, e.g. `auth.session_days`) was set, if it was set explicitly.
    #[must_use]
    pub fn origin(&self, key: &str) -> Option<&Origin> {
        self.origins.get(key)
    }

    /// # Errors
    /// The first invalid setting.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let bad = |m: String| Err(ConfigError::Invalid(m));
        if self.server.bind.parse::<std::net::SocketAddr>().is_err() {
            return bad(format!(
                "server.bind `{}` is not host:port",
                self.server.bind
            ));
        }
        if !self.server.public_url.starts_with("http://")
            && !self.server.public_url.starts_with("https://")
        {
            return bad("server.public_url must start with http:// or https://".into());
        }
        if !self.database.url.starts_with("postgres") {
            return bad("database.url must be a postgres:// URL".into());
        }
        if self.database.max_connections == 0 {
            return bad("database.max_connections must be positive".into());
        }
        if self.auth.session_days == 0 {
            return bad("auth.session_days must be positive".into());
        }
        if let Some(o) = &self.auth.oidc
            && (o.issuer.is_empty() || o.client_id.is_empty())
        {
            return bad("auth.oidc needs issuer and client_id".into());
        }
        if let Some(o) = &self.auth.oidc
            && o.role_source == OidcRoleSource::Provider
            && o.admin_groups.is_empty()
        {
            return bad("auth.oidc.role_source = \"provider\" needs admin_groups (OD-16)".into());
        }
        if let Some(p) = self.server.trusted_proxies.iter().find(|p| !is_network(p)) {
            return bad(format!(
                "server.trusted_proxies: `{p}` is no IP address or CIDR network"
            ));
        }
        let s = &self.server;
        if s.header_read_timeout_secs == 0
            || s.body_idle_timeout_secs == 0
            || s.body_read_timeout_secs == 0
            || s.import_read_timeout_secs == 0
        {
            return bad("server.header_read_timeout_secs, body_idle_timeout_secs, \
                 body_read_timeout_secs and import_read_timeout_secs must be positive"
                .into());
        }
        let t = &self.auth.throttle;
        if t.window_minutes == 0 || t.address_failures == 0 || t.verify_concurrency == 0 {
            return bad(
                "auth.throttle: window_minutes, address_failures and verify_concurrency must \
                 be positive"
                    .into(),
            );
        }
        if t.delay_max_secs < t.delay_base_secs {
            return bad("auth.throttle.delay_max_secs must not be below delay_base_secs".into());
        }
        if let Some(h) = self
            .outbound
            .allow_private_hosts
            .iter()
            .find(|h| !is_host(h))
        {
            return bad(format!(
                "outbound.allow_private_hosts: `{h}` is not `host`, `host:port` or \
                 `*.domain` (no scheme, no path)"
            ));
        }
        if self.jobs.concurrency == 0 {
            return bad("jobs.concurrency must be positive".into());
        }
        // below the retention job's 175-day hard limit (catalog_core::retention, DEC-453)
        if !(1..=170).contains(&self.jobs.catalog_max_age_days) {
            return bad("jobs.catalog_max_age_days must be between 1 and 170".into());
        }
        self.storage.validate().map_err(ConfigError::Invalid)
    }
}

/// `idp.lan`, `10.0.0.5:8443`, `[fd00::1]:8080`: a host as a URL writes it, with an optional
/// port.
fn is_host(s: &str) -> bool {
    // `*.home.arpa`: every subdomain (review SEC2-11; a bare leading dot is refused)
    let s = s.strip_prefix("*.").unwrap_or(s);
    if s.starts_with('.') || s.contains('*') {
        return false;
    }
    let (host, port) = match s.rsplit_once(':') {
        Some((h, p)) if !p.contains(']') => (h, Some(p)),
        _ => (s, None),
    };
    !host.is_empty()
        && port.is_none_or(|p| p.parse::<u16>().is_ok())
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '[' | ']' | ':'))
        && (!host.contains(':') || (host.starts_with('[') && host.ends_with(']')))
}

/// `10.0.0.0/8`, `192.0.2.1`, `fd00::/8`.
fn is_network(s: &str) -> bool {
    let (addr, bits) = s.split_once('/').map_or((s, None), |(a, b)| (a, Some(b)));
    let Ok(ip) = addr.trim().parse::<std::net::IpAddr>() else {
        return false;
    };
    let max = if ip.is_ipv4() { 32 } else { 128 };
    // an IPv4-mapped IPv6 network folds to IPv4 on the server, so its prefix is 96..=128
    let min = match ip {
        std::net::IpAddr::V6(v6) if v6.to_ipv4_mapped().is_some() => 96,
        _ => 0,
    };
    bits.is_none_or(|b| {
        b.trim()
            .parse::<u8>()
            .is_ok_and(|b| (min..=max).contains(&b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn defaults_are_valid() {
        AppConfig::from_sources("", env(&[])).unwrap();
    }

    /// DEC-453: the catalog retention age defaults to 150 days and stays below the job's
    /// 175-day hard limit.
    #[test]
    fn catalog_max_age_is_150_days_and_bounded() {
        let c = AppConfig::from_sources("", env(&[])).unwrap();
        assert_eq!(c.jobs.catalog_max_age_days, 150);
        let c =
            AppConfig::from_sources("", env(&[("APP__JOBS__CATALOG_MAX_AGE_DAYS", "30")])).unwrap();
        assert_eq!(c.jobs.catalog_max_age_days, 30);
        for bad in ["0", "171", "200"] {
            assert!(
                AppConfig::from_sources("", env(&[("APP__JOBS__CATALOG_MAX_AGE_DAYS", bad)]))
                    .is_err(),
                "{bad} days accepted"
            );
        }
    }

    /// WP4-5: the funnel is counted by default; the environment fixes the switch.
    #[test]
    fn the_usage_funnel_is_on_by_default_and_env_fixes_it() {
        let c = AppConfig::from_sources("", env(&[])).unwrap();
        assert!(c.usage.funnel);
        assert_eq!(c.origin("usage.funnel"), None);
        let c = AppConfig::from_sources("", env(&[("APP__USAGE__FUNNEL", "false")])).unwrap();
        assert!(!c.usage.funnel);
        assert_eq!(
            c.origin("usage.funnel"),
            Some(&Origin::Env("APP__USAGE__FUNNEL".into()))
        );
    }

    /// DEC-284 and addendum-1 4.3: trusted proxies (empty by default, CIDR checked) and the
    /// sign-in mode (`page` by default).
    /// sec2: every limit is a setting with a safe default.
    #[test]
    fn throttle_timeouts_and_outbound_have_safe_defaults_and_are_checked() {
        let d = AppConfig::default();
        assert_eq!(d.auth.throttle, ThrottleConfig::default());
        assert_eq!(
            (
                d.auth.throttle.free_failures,
                d.auth.throttle.delay_base_secs,
                d.auth.throttle.delay_max_secs,
                d.auth.throttle.window_minutes,
                d.auth.throttle.address_failures,
                d.auth.throttle.verify_concurrency
            ),
            (3, 1, 300, 15, 30, 8)
        );
        assert_eq!(
            (
                d.server.header_read_timeout_secs,
                d.server.body_idle_timeout_secs,
                d.server.body_read_timeout_secs
            ),
            (150, 30, 60)
        );
        assert_eq!(d.outbound.allow_private_hosts.len(), 0);
        assert_eq!(
            d.notify.allow_private_targets, None,
            "follows the hosting mode"
        );
        let c = AppConfig::from_sources(
            "[auth.throttle]\naddress_failures = 5\n[outbound]\nallow_private_hosts = [\"idp.lan\", \"10.0.0.5:8443\", \"[fd00::1]:8080\"]\n",
            env(&[
                ("APP__SERVER__BODY_READ_TIMEOUT_SECS", "20"),
                ("APP__NOTIFY__ALLOW_PRIVATE_TARGETS", "false"),
                ("APP__AUTH__THROTTLE__DELAY_MAX_SECS", "60"),
            ]),
        )
        .unwrap();
        assert_eq!(c.auth.throttle.address_failures, 5);
        assert_eq!(c.auth.throttle.delay_max_secs, 60);
        assert_eq!(
            c.auth.throttle.free_failures, 3,
            "the rest keeps its default"
        );
        assert_eq!(c.server.body_read_timeout_secs, 20);
        assert_eq!(c.notify.allow_private_targets, Some(false));
        assert_eq!(c.outbound.allow_private_hosts.len(), 3);
        for bad in [
            "[server]\nheader_read_timeout_secs = 0\n",
            "[server]\nbody_idle_timeout_secs = 0\n",
            "[server]\nbody_read_timeout_secs = 0\n",
            "[auth.throttle]\nverify_concurrency = 0\n",
            "[auth.throttle]\naddress_failures = 0\n",
            "[auth.throttle]\nwindow_minutes = 0\n",
            "[auth.throttle]\ndelay_base_secs = 10\ndelay_max_secs = 5\n",
            "[outbound]\nallow_private_hosts = [\"http://idp.lan\"]\n",
            "[outbound]\nallow_private_hosts = [\"idp.lan/path\"]\n",
            "[outbound]\nallow_private_hosts = [\"\"]\n",
            "[outbound]\nallow_private_hosts = [\"fd00::1\"]\n",
        ] {
            assert!(AppConfig::from_sources(bad, env(&[])).is_err(), "{bad}");
        }
    }

    /// Review SEC2-7: the import upload has a total time of its own; SEC2-11: private host
    /// entries are lower-cased, and only `*.` names a whole domain (a leading dot is refused
    /// instead of silently covering every subdomain).
    #[test]
    fn import_upload_time_and_private_host_entries() {
        let d = AppConfig::default();
        assert_eq!(d.server.import_read_timeout_secs, 1800);
        let c = AppConfig::from_sources(
            "[server]\nimport_read_timeout_secs = 600\n[outbound]\nallow_private_hosts = [\"IdP.LAN\", \"*.Home.Arpa\", \" relay.lan:25 \"]\n",
            env(&[]),
        )
        .unwrap();
        assert_eq!(c.server.import_read_timeout_secs, 600);
        assert_eq!(
            c.outbound.allow_private_hosts,
            ["idp.lan", "*.home.arpa", "relay.lan:25"]
        );
        for bad in [
            "[server]\nimport_read_timeout_secs = 0\n",
            "[outbound]\nallow_private_hosts = [\".lan\"]\n",
            "[outbound]\nallow_private_hosts = [\"*lan\"]\n",
            "[outbound]\nallow_private_hosts = [\"a.*.lan\"]\n",
        ] {
            assert!(AppConfig::from_sources(bad, env(&[])).is_err(), "{bad}");
        }
    }

    #[test]
    fn trusted_proxies_and_sign_in_mode() {
        let d = AppConfig::from_sources("", env(&[])).unwrap();
        assert_eq!(d.server.trusted_proxies.len(), 0);
        assert_eq!(d.auth.sign_in_mode, SignInMode::Page);
        let c = AppConfig::from_sources(
            "[server]\ntrusted_proxies = [\"10.0.0.0/8\", \"::1\"]\n",
            env(&[("APP__AUTH__SIGN_IN_MODE", "auto")]),
        )
        .unwrap();
        assert_eq!(c.server.trusted_proxies, ["10.0.0.0/8", "::1"]);
        assert_eq!(c.auth.sign_in_mode, SignInMode::Auto);
        assert!(
            AppConfig::from_sources("[server]\ntrusted_proxies = [\"10.0.0.0/40\"]\n", env(&[]))
                .is_err()
        );
        // UX3-20: a mapped IPv6 network needs a prefix of 96 or more, as the server checks
        let mapped = |n: &str| {
            AppConfig::from_sources(
                &format!("[server]\ntrusted_proxies = [\"{n}\"]\n"),
                env(&[]),
            )
        };
        assert!(mapped("::ffff:10.0.0.0/104").is_ok());
        assert!(mapped("::ffff:10.0.0.0/90").is_err());
    }

    #[test]
    fn env_overrides_file_values() {
        let c = AppConfig::from_sources(
            "[database]\nurl = \"postgres://file/db\"\nmax_connections = 5\n",
            env(&[
                ("APP__DATABASE__URL", "postgres://env/db"),
                ("APP__STORAGE__MAX_TOTAL_BYTES", "5GiB"),
                ("APP__AUTH__SECURE_COOKIES", "false"),
                ("APP__CONNECTORS__TMDB__API_KEY", "secret"),
                ("UNRELATED", "x"),
            ]),
        )
        .unwrap();
        assert_eq!(c.database.url, "postgres://env/db");
        assert_eq!(c.database.max_connections, 5);
        assert_eq!(c.storage.max_total_bytes, assets::ByteSize::gib(5));
        assert!(!c.auth.secure_cookies);
        assert_eq!(
            c.connectors["tmdb"].api_key.as_ref().map(Secret::expose),
            Some("secret")
        );
    }

    #[test]
    fn configuring_a_key_keeps_the_connector_enabled() {
        let c =
            AppConfig::from_sources("", env(&[("APP__CONNECTORS__TMDB__API_KEY", "k")])).unwrap();
        assert!(c.connectors["tmdb"].enabled);
        let off =
            AppConfig::from_sources("[connectors.tmdb]\nenabled = false\n", env(&[])).unwrap();
        assert!(!off.connectors["tmdb"].enabled);
    }

    /// v1-release item 2: the short key variables carry the neutral prefix `SOURCE_`; the
    /// prefix of the former codename is not read (no alias, greenfield until v1).
    #[test]
    fn source_key_variables_are_a_fallback() {
        let old = ["K", "K_TMDB_API_KEY"].concat();
        let c = AppConfig::from_sources("", env(&[(old.as_str(), "k")])).unwrap();
        assert!(!c.connectors.contains_key("tmdb"));
        let c = AppConfig::from_sources("", env(&[("SOURCE_TMDB_API_KEY", "k")])).unwrap();
        assert!(c.connectors["tmdb"].enabled);
        assert_eq!(
            c.connectors["tmdb"].api_key.as_ref().map(Secret::expose),
            Some("k")
        );
    }

    #[test]
    fn rejects_unknown_keys_and_bad_values() {
        assert!(AppConfig::from_sources("[server]\nbindd = \"x\"\n", env(&[])).is_err());
        assert!(AppConfig::from_sources("", env(&[("APP__SERVER__BIND", "nonsense")])).is_err());
        assert!(
            AppConfig::from_sources("", env(&[("APP__STORAGE__LOW_WATERMARK", "0.99")])).is_err()
        );
    }

    #[test]
    fn secrets_are_never_serialised() {
        let c = AppConfig::from_sources(
            "",
            env(&[
                ("APP__SEARCH__MEILI_MASTER_KEY", "m"),
                ("APP__CONNECTORS__TMDB__API_KEY", "t"),
            ]),
        )
        .unwrap();
        let out = toml::to_string(&c).unwrap();
        assert!(!out.contains("\"m\"") && !out.contains("\"t\""), "{out}");
    }

    #[test]
    fn debug_output_redacts_secrets() {
        let c = AppConfig::from_sources("", env(&[("APP__SEARCH__MEILI_MASTER_KEY", "topsecret")]))
            .unwrap();
        assert!(!format!("{c:?}").contains("topsecret"));
    }

    #[test]
    fn origins_name_the_file_and_the_environment_variable() {
        let c = AppConfig::from_sources(
            "[auth]\nsession_days = 7\n[storage]\nhigh_watermark = 0.8\n",
            env(&[
                ("APP__AUTH__SECURE_COOKIES", "false"),
                ("APP__STORAGE__HIGH_WATERMARK", "0.85"),
                ("SOURCE_TMDB_API_KEY", "k"),
            ]),
        )
        .unwrap();
        assert_eq!(c.origin("auth.session_days"), Some(&Origin::File));
        assert_eq!(
            c.origin("auth.secure_cookies"),
            Some(&Origin::Env("APP__AUTH__SECURE_COOKIES".into()))
        );
        // env wins over the file, and says so
        assert_eq!(
            c.origin("storage.high_watermark"),
            Some(&Origin::Env("APP__STORAGE__HIGH_WATERMARK".into()))
        );
        assert_eq!(
            c.origin("connectors.tmdb.api_key"),
            Some(&Origin::Env("SOURCE_TMDB_API_KEY".into()))
        );
        assert_eq!(c.origin("storage.max_total_bytes"), None);
        assert_eq!(c.origin("instance.hosting"), None);
    }

    #[test]
    fn instance_and_secrets_defaults() {
        let c = AppConfig::from_sources("", env(&[])).unwrap();
        assert_eq!(c.instance.hosting, Hosting::Home);
        assert_eq!(c.secrets.key_file, "data/secrets.key");
        let c = AppConfig::from_sources(
            "",
            env(&[
                ("APP__INSTANCE__HOSTING", "hosted"),
                ("APP__SECRETS__KEYS", "v1:abc"),
            ]),
        )
        .unwrap();
        assert_eq!(c.instance.hosting, Hosting::Hosted);
        assert!(!toml::to_string(&c).unwrap().contains("v1:abc"));
        assert!(!format!("{c:?}").contains("v1:abc"));
    }

    #[test]
    fn example_file_is_valid() {
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../config/app.example.toml"),
        )
        .unwrap();
        AppConfig::from_sources(&text, env(&[])).unwrap();
    }

    #[test]
    fn reference_doc_is_current() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../docs/modules/ROOT/pages/configuration.adoc");
        let want = reference_adoc();
        if std::env::var_os("UPDATE_DOCS").is_some() {
            std::fs::write(&path, &want).unwrap();
        }
        let have = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            have == want,
            "configuration.adoc is stale: run `UPDATE_DOCS=1 cargo test -p config`"
        );
    }

    /// UX2-S8: `config/.env.example` lists every variable with an empty value; sourcing it
    /// must not turn them into empty settings (an empty hosting mode or key file path).
    #[test]
    fn empty_environment_values_are_ignored() {
        let c = AppConfig::from_sources(
            "",
            env(&[
                ("APP__INSTANCE__HOSTING", ""),
                ("APP__SECRETS__KEY_FILE", ""),
                ("APP__SECRETS__KEYS", "  "),
                ("APP__CONNECTORS__TMDB__API_KEY", ""),
            ]),
        )
        .unwrap();
        assert_eq!(c.secrets.key_file, SecretsConfig::default().key_file);
        assert!(c.secrets.keys.is_none());
        assert_eq!(c.origin("secrets.key_file"), None);
        assert_eq!(c.origin("instance.hosting"), None);
        assert!(!c.connectors.contains_key("tmdb") || c.connectors["tmdb"].api_key.is_none());
    }

    /// v1-books 3.1.1: the operator contact comes from the file, from its `APP__` variable or
    /// from the short form; the long form wins, an empty value is "not set".
    #[test]
    fn the_operator_contact_has_a_short_variable() {
        let c = AppConfig::from_sources("", env(&[])).unwrap();
        assert_eq!(c.instance.operator_contact, None);
        assert_eq!(c.origin("instance.operator_contact"), None);
        let c = AppConfig::from_sources("", env(&[("SOURCE_OPERATOR_CONTACT", "")])).unwrap();
        assert_eq!(c.instance.operator_contact, None);
        let c = AppConfig::from_sources("", env(&[("SOURCE_OPERATOR_CONTACT", "ops@example.org")]))
            .unwrap();
        assert_eq!(
            c.instance.operator_contact.as_deref(),
            Some("ops@example.org")
        );
        assert_eq!(
            c.origin("instance.operator_contact"),
            Some(&Origin::Env("SOURCE_OPERATOR_CONTACT".into()))
        );
        let c = AppConfig::from_sources(
            "",
            env(&[
                ("SOURCE_OPERATOR_CONTACT", "ops@example.org"),
                ("APP__INSTANCE__OPERATOR_CONTACT", "https://example.org/me"),
            ]),
        )
        .unwrap();
        assert_eq!(
            c.instance.operator_contact.as_deref(),
            Some("https://example.org/me")
        );
        assert_eq!(
            c.origin("instance.operator_contact"),
            Some(&Origin::Env("APP__INSTANCE__OPERATOR_CONTACT".into()))
        );
    }

    /// UX2-1: the temporary key is an explicit switch, never implied by the asset backend.
    #[test]
    fn the_ephemeral_secrets_key_is_off_unless_asked_for() {
        let c = AppConfig::from_sources("[storage]\nbackend = \"memory\"\n", env(&[])).unwrap();
        assert!(!c.secrets.ephemeral);
        let c = AppConfig::from_sources("", env(&[("APP__SECRETS__EPHEMERAL", "true")])).unwrap();
        assert!(c.secrets.ephemeral);
    }
}
