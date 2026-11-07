//! Wiring of UX phase 2: the secrets key ring (addendum-1 2.2), the start-up migration of
//! plain-text secrets, the admin API with the configuration's provenance (3.2) and the jobs
//! of the audit retention (OD-13) and the key rotation.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Context;
use app::ports::{SecretCipher, SecretRows};
use auth::secrets::{KeyOptions, KeyRing, KeySource};
use config::{AppConfig, Origin};
use serde_json::Value;
use zeroize::Zeroizing;

/// Daily audit retention (OD-13).
pub const AUDIT_RETENTION_JOB: &str = "admin.audit_retention";
/// Nightly re-wrap while values are on an old key version.
pub const REWRAP_JOB: &str = "secrets.rewrap";

/// The loaded key ring and where it came from.
pub struct Keys {
    pub ring: Arc<KeyRing>,
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
        ring: Arc::new(ring),
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

fn fixed_by(o: &Origin) -> api::admin::FixedBy {
    match o {
        Origin::File => api::admin::FixedBy::File,
        Origin::Env(v) => api::admin::FixedBy::Env(v.clone()),
    }
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
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    let fixed = api::admin::SETTINGS
        .iter()
        .filter_map(|d| {
            config
                .origin(d.key)
                .map(|o| (d.key.to_string(), fixed_by(o)))
        })
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
            active: p.keys.ring.active_version(),
            loaded: p.keys.ring.versions(),
            source: p.keys.source.as_str().into(),
            key_file: cfg.secrets.key_file.clone(),
        },
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
            mail_configured: cfg.notify.smtp.is_some(),
            oidc_configured: cfg.auth.oidc.is_some(),
        },
        secure_cookies: Arc::new(std::sync::atomic::AtomicBool::new(cfg.auth.secure_cookies)),
        hosting: Arc::new(std::sync::RwLock::new(cfg.instance.hosting.as_str().into())),
    });
    api::admin::load_settings(&api)
        .await
        .map_err(|e| anyhow::anyhow!("instance settings: {}", e.message))?;
    api::admin::load_credentials(&api)
        .await
        .map_err(|e| anyhow::anyhow!("data-source credentials: {}", e.message))?;
    Ok(api)
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

    /// UX2-1: plain-text secrets are never sealed under a temporary key.
    #[tokio::test]
    async fn nothing_is_sealed_under_a_temporary_key() {
        let rows: Vec<Arc<dyn SecretRows>> = vec![Arc::new(NoSeal)];
        let ring = KeyRing::generate().0;
        seal_plaintext(&rows, &ring, &KeySource::Ephemeral)
            .await
            .unwrap();
    }
}
