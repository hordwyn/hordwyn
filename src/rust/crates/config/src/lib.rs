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
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:3000".into(),
            public_url: "http://localhost:3000".into(),
            role: ServerRole::All,
            assets_base_url: None,
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
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            client_id: String::new(),
            client_secret: None,
            display_name: "Single sign-on".into(),
            auto_create: false,
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
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            signup: SignupMode::Invite,
            session_days: 30,
            secure_cookies: true,
            oidc: None,
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
    /// API key or token. Prefer `APP__CONNECTORS__<ID>__API_KEY`; `KK_<ID>_API_KEY` is read
    /// as a fallback.
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
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            shutdown_grace_secs: 20,
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
    /// Allow channel targets (ntfy, Gotify, webhooks, ...) in private and loopback networks.
    /// Off by default: otherwise any member could make the server call internal services.
    pub allow_private_targets: bool,
    /// Attempts per delivery before it is marked failed.
    pub max_attempts: u32,
    /// Email channel; email notifications are unavailable without it.
    pub smtp: Option<SmtpConfig>,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            allow_private_targets: false,
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

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct FeaturesConfig {
    /// Enabled media kinds; empty = all built-in kinds (D-10 feature toggles).
    pub kinds: Vec<String>,
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
}

/// Parse an environment value as a TOML value (number, bool, array), else a string.
fn env_value(raw: &str) -> toml::Value {
    toml::from_str::<toml::Table>(&format!("v = {raw}"))
        .ok()
        .and_then(|mut t| t.remove("v"))
        .unwrap_or_else(|| toml::Value::String(raw.to_string()))
}

/// Apply `APP__A__B=value` overrides onto a TOML table.
fn apply_env(table: &mut toml::Table, vars: impl IntoIterator<Item = (String, String)>) {
    for (k, v) in vars {
        let Some(path) = k.strip_prefix(ENV_PREFIX) else {
            continue;
        };
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
        apply_env(&mut table, env.iter().cloned());
        let mut cfg: Self = toml::Value::Table(table)
            .try_into()
            .map_err(|e: toml::de::Error| ConfigError::Parse(e.to_string()))?;
        // Legacy-style key variables (`KK_TMDB_API_KEY`, KICKOFF section 5) as fallback.
        for (k, v) in &env {
            if let Some(id) = k
                .strip_prefix("KK_")
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
                }
            }
        }
        cfg.validate()?;
        Ok(cfg)
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
        if self.jobs.concurrency == 0 {
            return bad("jobs.concurrency must be positive".into());
        }
        self.storage.validate().map_err(ConfigError::Invalid)
    }
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

    #[test]
    fn kk_key_variables_are_a_fallback() {
        let c = AppConfig::from_sources("", env(&[("KK_TMDB_API_KEY", "k")])).unwrap();
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
}
