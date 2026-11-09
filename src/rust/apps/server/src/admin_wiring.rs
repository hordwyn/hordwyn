//! Wiring of UX phase 2: the secrets key ring (addendum-1 2.2), the start-up migration of
//! plain-text secrets, the admin API with the configuration's provenance (3.2) and the jobs
//! of the audit retention (OD-13) and the key rotation.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Context;
use app::ports::{SecretCipher, SecretRows};
use auth::secrets::{KeyOptions, KeySource, LiveKeys};
use config::{AppConfig, Origin};
use serde_json::Value;
use zeroize::Zeroizing;

/// Daily audit retention (OD-13).
pub const AUDIT_RETENTION_JOB: &str = "admin.audit_retention";
/// Nightly re-wrap while values are on an old key version.
pub const REWRAP_JOB: &str = "secrets.rewrap";
/// The Overview health rows every minute (WP4-2, DEC-285).
pub const PROBES_JOB: &str = "admin.probes";
/// The daily funnel totals and their retention (WP4-5).
pub const USAGE_DAILY_JOB: &str = "usage.funnel_daily";

/// The loaded key ring and where it came from.
pub struct Keys {
    pub ring: Arc<LiveKeys>,
    pub source: KeySource,
}

/// Where to look for the keys. The temporary key is the explicit `secrets.ephemeral`
/// switch of test harnesses, never implied by the storage backend (UX2-1).
#[must_use]
pub fn key_options(config: &AppConfig) -> KeyOptions {
    KeyOptions {
        key_file: config.secrets.key_file.clone().into(),
        env_keys: config
            .secrets
            .keys
            .as_ref()
            .map(|k| Zeroizing::new(k.expose().to_string())),
        ephemeral: config.secrets.ephemeral,
    }
}

/// Load the key ring: file, then `APP__SECRETS__KEYS`, else generate the file (OD-06).
/// `secrets.ephemeral` (test harnesses) uses a temporary key instead of generating one.
///
/// # Errors
/// A malformed key source, or encrypted values without a key (refuse to start).
pub async fn load_keys(config: &AppConfig, pool: &sqlx::PgPool) -> anyhow::Result<Keys> {
    let has_rows = auth::admin_pg::has_encrypted_rows(pool)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let (ring, source) =
        auth::secrets::load(&key_options(config), has_rows).context("secrets key")?;
    Ok(Keys {
        ring: Arc::new(LiveKeys::new(ring)),
        source,
    })
}

/// Encrypt values stored in plain text by earlier versions (TOTP seeds, channel secrets).
/// Never under a temporary key: the values would be lost with the next restart (UX2-1);
/// they stay as they are until a real key is configured.
///
/// # Errors
/// Database errors.
pub async fn seal_plaintext(
    rows: &[Arc<dyn SecretRows>],
    cipher: &dyn SecretCipher,
    source: &KeySource,
) -> anyhow::Result<()> {
    if *source == KeySource::Ephemeral {
        tracing::warn!("temporary secrets key: stored plain-text secrets are left unencrypted");
        return Ok(());
    }
    for r in rows {
        let n = r
            .seal_plaintext(cipher)
            .await
            .map_err(|e| anyhow::anyhow!("sealing {}: {e}", r.purpose()))?;
        if n > 0 {
            tracing::info!(purpose = r.purpose(), count = n, "encrypted stored secrets");
        }
    }
    Ok(())
}

/// Every table column that holds an encrypted value, one [`SecretRows`] per purpose
/// (UX3-1). The server (re-wrap job, SIGHUP reload, security page) and the CLI (`secrets
/// status`, `prune`) both use this list, so a new purpose is counted everywhere or nowhere;
/// `the_registry_covers_every_secret_purpose` fails when a new `impl SecretRows` is missing.
#[must_use]
pub fn secret_rows(
    pool: &sqlx::PgPool,
    cipher: &Arc<dyn SecretCipher>,
) -> Vec<Arc<dyn SecretRows>> {
    vec![
        Arc::new(auth::pg::PgAuthStore::new(pool.clone(), cipher.clone())),
        // the ID token (logout hint) and pending gated action per session (WP3-3)
        Arc::new(auth::pg::PgSessionSecrets::new(pool.clone())),
        Arc::new(notify::pg::PgNotifyStore::new(pool.clone(), cipher.clone())),
        Arc::new(auth::admin_pg::PgAdminStore::new(
            pool.clone(),
            cipher.clone(),
        )),
        Arc::new(auth::admin_pg::PgMailSecret::new(pool.clone())),
        Arc::new(auth::oidc::PgProviderStore::new(
            pool.clone(),
            cipher.clone(),
        )),
    ]
}

fn fixed_by(o: &Origin) -> api::admin::FixedBy {
    match o {
        Origin::File => api::admin::FixedBy::File,
        Origin::Env(v) => api::admin::FixedBy::Env(v.clone()),
    }
}

const fn security(s: config::SmtpSecurity) -> &'static str {
    match s {
        config::SmtpSecurity::Tls => "tls",
        config::SmtpSecurity::Starttls => "starttls",
        config::SmtpSecurity::None => "none",
    }
}

/// Who fixes a managed setting. A `[notify.smtp]` block in env or file fixes every `mail.*`
/// key (the configuration wins as a whole, addendum-1 3.2), named after its variable.
fn origin_of(config: &AppConfig, key: &str) -> Option<api::admin::FixedBy> {
    if let Some(field) = key.strip_prefix("mail.") {
        config.notify.smtp.as_ref()?;
        let path = format!("notify.smtp.{field}");
        return Some(
            match config
                .origin(&path)
                .or_else(|| config.origin("notify.smtp.host"))
            {
                Some(Origin::Env(_)) => {
                    api::admin::FixedBy::Env(format!("APP__NOTIFY__SMTP__{}", field.to_uppercase()))
                }
                _ => api::admin::FixedBy::File,
            },
        );
    }
    config.origin(key).map(fixed_by)
}

/// The configured values of the managed settings and the keys env/file fix.
#[must_use]
pub fn config_values(
    config: &AppConfig,
    disabled_sources: Vec<(String, String, Vec<String>)>,
) -> api::admin::ConfigValues {
    let signup = match config.auth.signup {
        config::SignupMode::Open => "open",
        config::SignupMode::Invite => "invite",
        config::SignupMode::Closed => "closed",
    };
    let smtp = config.notify.smtp.as_ref();
    let values: BTreeMap<String, Value> = [
        (
            "instance.hosting",
            Value::from(config.instance.hosting.as_str()),
        ),
        ("auth.signup", Value::from(signup)),
        ("auth.session_days", Value::from(config.auth.session_days)),
        (
            "auth.secure_cookies",
            Value::from(config.auth.secure_cookies),
        ),
        (
            "storage.max_total_bytes",
            Value::from(config.storage.max_total_bytes.0),
        ),
        (
            "storage.high_watermark",
            Value::from(config.storage.high_watermark),
        ),
        (
            "storage.low_watermark",
            Value::from(config.storage.low_watermark),
        ),
        (
            "auth.sign_in_mode",
            Value::from(config.auth.sign_in_mode.as_str()),
        ),
        (
            "mail.host",
            smtp.map_or(Value::Null, |s| Value::from(s.host.as_str())),
        ),
        ("mail.port", Value::from(smtp.map_or(587, |s| s.port))),
        (
            "mail.security",
            Value::from(smtp.map_or("starttls", |s| security(s.security))),
        ),
        (
            "mail.username",
            smtp.and_then(|s| s.username.as_deref())
                .map_or(Value::Null, Value::from),
        ),
        (
            "mail.from",
            smtp.map_or(Value::Null, |s| Value::from(s.from.as_str())),
        ),
        ("usage.funnel", Value::from(config.usage.funnel)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let fixed = api::admin::SETTINGS
        .iter()
        .filter_map(|d| origin_of(config, d.key).map(|o| (d.key.to_string(), o)))
        .collect();
    let source_env = config
        .connectors
        .iter()
        .filter(|(_, c)| c.api_key.is_some())
        .map(|(id, _)| {
            let var = match config.origin(&format!("connectors.{id}.api_key")) {
                Some(Origin::Env(v)) => v.clone(),
                _ => format!("APP__CONNECTORS__{}__API_KEY", id.to_ascii_uppercase()),
            };
            (id.clone(), var)
        })
        .collect();
    api::admin::ConfigValues {
        values,
        fixed,
        public_url: config.server.public_url.clone(),
        source_env,
        disabled_sources,
        mail_password: smtp
            .and_then(|s| s.password.as_ref())
            .map(|p| api::admin::EnvSecret(Zeroizing::new(p.expose().to_string()))),
    }
}

/// What [`with_admin`] needs from the composition root.
pub struct AdminParts<'a> {
    pub config: &'a AppConfig,
    pub brand: &'a brand::Brand,
    pub pool: &'a sqlx::PgPool,
    pub keys: Keys,
    pub secret_rows: Vec<Arc<dyn SecretRows>>,
    pub budget: Arc<assets::StorageBudget>,
    pub queue: jobs::PgQueue,
    pub http: Arc<dyn app::ports::HttpFetch>,
    pub search_backend: &'static str,
    pub disabled_sources: Vec<(String, String, Vec<String>)>,
    pub mailer: Arc<notify::channels::email::Mailer>,
    /// `instance.hosting`, shared with the governed client.
    pub hosting: Arc<std::sync::RwLock<String>>,
}

/// Add the admin API to the state, then apply the stored settings and credentials.
///
/// # Errors
/// Database errors.
pub async fn with_admin(
    mut api: api::ApiState,
    p: AdminParts<'_>,
) -> anyhow::Result<api::ApiState> {
    let cfg = p.config;
    let store: Arc<dyn auth::admin::AdminStore> = Arc::new(auth::admin_pg::PgAdminStore::new(
        p.pool.clone(),
        p.keys.ring.clone(),
    ));
    let now = chrono::Utc::now();
    store
        .record_key_versions(&p.keys.ring.versions(), now)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    api.admin = Some(api::admin::AdminApi {
        store,
        config: Arc::new(config_values(cfg, p.disabled_sources)),
        cipher: p.keys.ring.clone(),
        keys: api::admin::KeyInfo {
            live: p.keys.ring.clone(),
            options: key_options(cfg),
            source: p.keys.source.as_str().into(),
            key_file: cfg.secrets.key_file.clone(),
        },
        usage_limiter: Arc::default(),
        secret_rows: p.secret_rows,
        budget: Some(p.budget),
        queue: Some(p.queue),
        http: p.http,
        brand: contracts::admin::BrandCardDto {
            name: p.brand.name.clone(),
            short_name: p.brand.short_name.clone(),
            default_locale: p.brand.default_locale.clone(),
            dir: cfg.brand.dir.clone(),
            swatches: p
                .brand
                .accent_pairs()
                .into_iter()
                .map(|f| contracts::admin::BrandSwatchDto {
                    theme: f.theme.to_string(),
                    primary: f.primary,
                    foreground: f.foreground,
                    ratio: f.ratio,
                })
                .collect(),
        },
        health: api::admin::HealthInputs {
            search_backend: p.search_backend.into(),
            oidc_configured: cfg.auth.oidc.is_some(),
        },
        secure_cookies: Arc::new(std::sync::atomic::AtomicBool::new(cfg.auth.secure_cookies)),
        hosting: p.hosting,
        mail: Some(Arc::new(EmailTransport(p.mailer))),
    });
    api::admin::load_settings(&api)
        .await
        .map_err(|e| anyhow::anyhow!("instance settings: {}", e.message))?;
    api::admin::load_credentials(&api)
        .await
        .map_err(|e| anyhow::anyhow!("data-source credentials: {}", e.message))?;
    api::admin::load_source_switches(&api)
        .await
        .map_err(|e| anyhow::anyhow!("source switches: {}", e.message))?;
    // a failure is logged; the mail card shows mail as off and the test says why
    let _ = api::admin::rebuild_mail(&api).await;
    Ok(api)
}

/// The SMTP transport behind `api::admin::MailTransport`, swapped in place.
pub struct EmailTransport(pub Arc<notify::channels::email::Mailer>);

/// notify's settings from the admin API's.
pub fn smtp_settings(m: &api::admin::MailSettings) -> notify::channels::email::SmtpSettings {
    use notify::channels::email::SmtpSecurity;
    notify::channels::email::SmtpSettings {
        host: m.host.clone(),
        port: m.port,
        security: match m.security.as_str() {
            "tls" => SmtpSecurity::Tls,
            "none" => SmtpSecurity::None,
            _ => SmtpSecurity::StartTls,
        },
        username: m.username.clone(),
        password: m.password.as_ref().map(|p| p.to_string()),
        from: m.from.clone(),
    }
}

#[async_trait::async_trait]
impl api::admin::MailTransport for EmailTransport {
    fn configure(&self, settings: Option<&api::admin::MailSettings>) -> Result<(), String> {
        match settings {
            None => {
                self.0.set(None);
                Ok(())
            }
            Some(m) => match notify::channels::email::Email::new(&smtp_settings(m)) {
                Ok(e) => {
                    self.0.set(Some(e));
                    Ok(())
                }
                Err(e) => {
                    self.0.set(None);
                    Err(e)
                }
            },
        }
    }

    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), String> {
        let Some(e) = self.0.current() else {
            return Err("mail is not configured".into());
        };
        e.send_text(to, subject, body).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fails the test if anything tries to seal.
    struct NoSeal;

    #[async_trait::async_trait]
    impl SecretRows for NoSeal {
        fn purpose(&self) -> &'static str {
            "test"
        }

        async fn seal_plaintext(&self, _cipher: &dyn SecretCipher) -> app::ports::RepoResult<u64> {
            panic!("sealed under a temporary key");
        }

        async fn version_counts(&self) -> app::ports::RepoResult<Vec<(u16, u64)>> {
            Ok(vec![])
        }

        async fn rewrap_batch(
            &self,
            _cipher: &dyn SecretCipher,
            _active: u16,
            _after: Option<uuid::Uuid>,
            _limit: u32,
        ) -> app::ports::RepoResult<app::ports::RewrapBatch> {
            Ok(app::ports::RewrapBatch::default())
        }
    }

    /// UX2-1: the memory asset backend no longer implies a temporary key.
    #[test]
    fn the_temporary_key_needs_the_explicit_switch() {
        let mem = AppConfig::from_sources("[storage]\nbackend = \"memory\"\n", Vec::new()).unwrap();
        assert!(!key_options(&mem).ephemeral);
        let eph = AppConfig::from_sources("[secrets]\nephemeral = true\n", Vec::new()).unwrap();
        assert!(key_options(&eph).ephemeral);
    }

    /// Every `impl SecretRows for` in the workspace sources (test doubles excluded).
    fn implementors() -> Vec<String> {
        fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    if !p.ends_with("target") && !p.ends_with("tests") {
                        walk(&p, out);
                    }
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).unwrap();
                    for l in text.lines() {
                        if let Some(rest) = l.trim().strip_prefix("impl SecretRows for ") {
                            let name = rest.trim_end_matches(" {").trim();
                            if name != "NoSeal" {
                                out.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut out = Vec::new();
        walk(&root.join("crates"), &mut out);
        walk(&root.join("apps"), &mut out);
        out.sort();
        out
    }

    /// UX3-1: the one registry the server and the CLI share has one entry per purpose and
    /// one per `SecretRows` implementation in the workspace, so prune cannot miss a purpose.
    #[tokio::test]
    async fn the_registry_covers_every_secret_purpose() {
        let pool = sqlx::PgPool::connect_lazy("postgres://nobody@127.0.0.1:1/none").unwrap();
        let cipher: Arc<dyn SecretCipher> = Arc::new(auth::secrets::KeyRing::generate().0);
        let rows = secret_rows(&pool, &cipher);
        let mut purposes: Vec<&str> = rows.iter().map(|r| r.purpose()).collect();
        purposes.sort_unstable();
        purposes.dedup();
        assert_eq!(purposes.len(), rows.len(), "one entry per purpose");
        assert_eq!(rows.len(), implementors().len(), "{:?}", implementors());
        for p in [
            auth::pg::TOTP_PURPOSE,
            auth::pg::SESSION_SECRET_PURPOSE,
            notify::pg::CHANNEL_PURPOSE,
            auth::admin::CREDENTIAL_PURPOSE,
            auth::admin::MAIL_PURPOSE,
            auth::oidc::SECRET_PURPOSE,
        ] {
            assert!(purposes.contains(&p), "{p} missing");
        }
    }

    /// UX2-1: plain-text secrets are never sealed under a temporary key.
    #[tokio::test]
    async fn nothing_is_sealed_under_a_temporary_key() {
        let rows: Vec<Arc<dyn SecretRows>> = vec![Arc::new(NoSeal)];
        let ring = auth::secrets::KeyRing::generate().0;
        seal_plaintext(&rows, &ring, &KeySource::Ephemeral)
            .await
            .unwrap();
    }
}
