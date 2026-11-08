//! Shell-only escape hatches (addendum-1 3.3, DEC-283), used by the `cli` binary: they need
//! shell access to the server's configuration, not a browser session. Each writes an audit
//! row with actor `cli`. They open the database directly (no web server, no worker) and never
//! generate a secrets key.

use std::sync::Arc;

use anyhow::{Context, bail};
use app::App;
use app::ports::{Notification, SecretRows};
use auth::admin::{ActorKind, AdminStore, AuditEntry};
use auth::admin_pg::PgAdminStore;
use auth::secrets::{KeyRing, KeySource, LiveKeys};
use config::AppConfig;
use domain::user::{Role, User};
use serde_json::{Value, json};

/// Notifications are not sent from the shell.
struct Silent;

#[async_trait::async_trait]
impl app::ports::Notifications for Silent {
    async fn emit(&self, _n: Notification) {}
}

/// One key version as `cli secrets status` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyVersionRow {
    pub version: u16,
    pub rows: u64,
    pub loaded: bool,
    pub active: bool,
}

/// `cli secrets status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretsReport {
    /// `file`, `env` or `none` (no key configured).
    pub key_source: String,
    pub key_file: String,
    pub active_version: u16,
    pub versions: Vec<KeyVersionRow>,
    /// Values on a loaded, older version (the nightly job re-encrypts them).
    pub rows_to_rewrap: u64,
    /// Values whose key version is not loaded.
    pub locked_rows: u64,
}

/// The database and stores the escape hatches need.
pub struct Escape {
    app: App,
    auth: auth::Auth,
    admin: PgAdminStore,
    rows: Vec<Arc<dyn SecretRows>>,
    /// The configured keys, if any (`None`: nothing configured; no key is generated).
    ring: Option<Arc<LiveKeys>>,
    source: KeySource,
    key_file: std::path::PathBuf,
    public_url: String,
    pool: sqlx::PgPool,
    /// Where the configuration fixes `auth.sign_in_mode` (variable name or the file) and the
    /// value it fixes; `None` when the stored setting is in force (UX3-2).
    mode_fixed: Option<(String, config::SignInMode)>,
}

fn port(e: impl std::fmt::Display) -> anyhow::Error {
    anyhow::anyhow!("{e}")
}

impl Escape {
    /// Connect with the server's configuration and apply pending migrations.
    ///
    /// # Errors
    /// Unreachable database, a malformed key source.
    pub async fn open(cfg: &AppConfig) -> anyhow::Result<Self> {
        let pool = store_pg::connect(&cfg.database.url, 2)
            .await
            .context("connect to Postgres")?;
        store_pg::migrate(&pool).await.context("migrations")?;
        let opts = crate::admin_wiring::key_options(cfg);
        let (ring, source) = if opts.key_file.exists() {
            (Some(auth::secrets::read_keys(&opts)?), KeySource::File)
        } else if opts.env_keys.is_some() {
            (Some(auth::secrets::read_keys(&opts)?), KeySource::Env)
        } else {
            (None, KeySource::Ephemeral)
        };
        let ring = ring.map(|r| Arc::new(LiveKeys::new(r)));
        // without a configured key nothing can be decrypted; nothing is encrypted here either
        let cipher: Arc<dyn app::ports::SecretCipher> = match &ring {
            Some(r) => r.clone(),
            None => Arc::new(KeyRing::generate().0),
        };
        let queue = jobs::PgQueue::new(pool.clone());
        let app = App {
            repos: crate::pg_repos(&pool),
            kinds: Arc::new(kinds::Registry::with_builtins()),
            clock: Arc::new(crate::infra::SystemClock::default()),
            ids: Arc::new(crate::infra::SystemIds),
            events: Arc::new(realtime::Bus::local()),
            jobs: Arc::new(queue),
            notifications: Arc::new(Silent),
        };
        let auth_store = Arc::new(auth::pg::PgAuthStore::new(pool.clone(), cipher.clone()));
        let auth = auth::Auth::new(
            app.clone(),
            auth_store.clone(),
            auth::AuthSettings::default(),
        )
        .with_invites(auth_store);
        let rows = crate::admin_wiring::secret_rows(&pool, &cipher);
        Ok(Self {
            app,
            auth,
            admin: PgAdminStore::new(pool.clone(), cipher),
            rows,
            ring,
            source,
            key_file: opts.key_file,
            public_url: cfg.server.public_url.trim_end_matches('/').to_string(),
            pool,
            mode_fixed: cfg.origin("auth.sign_in_mode").map(|o| {
                let at = match o {
                    config::Origin::Env(v) => v.clone(),
                    config::Origin::File => "the configuration file".to_string(),
                };
                (at, cfg.auth.sign_in_mode)
            }),
        })
    }

    async fn audit(
        &self,
        action: &str,
        target: Option<(&str, String)>,
        detail: Value,
    ) -> anyhow::Result<()> {
        self.admin
            .audit(&Self::entry(action, target, detail))
            .await
            .map_err(|e| {
                anyhow::anyhow!("{action}: done, but the audit row could not be written: {e}")
            })
    }

    fn entry(action: &str, target: Option<(&str, String)>, detail: Value) -> AuditEntry {
        AuditEntry {
            id: uuid::Uuid::now_v7(),
            at: chrono::Utc::now(),
            actor_id: None,
            actor_kind: ActorKind::Cli,
            actor_name: None,
            action: action.into(),
            target_type: target.as_ref().map(|(t, _)| (*t).to_string()),
            target_id: target.map(|(_, id)| id),
            detail,
            client_addr: None,
            request_id: None,
        }
    }

    async fn user(&self, username: &str) -> anyhow::Result<User> {
        self.app
            .list_users()
            .await
            .map_err(port)?
            .into_iter()
            .find(|u| u.username == username)
            .with_context(|| format!("no account `{username}`"))
    }

    /// `cli admin promote <username>`: make an account admin (a wiped or lost last admin).
    ///
    /// # Errors
    /// Unknown account, storage errors.
    pub async fn promote(&self, username: &str) -> anyhow::Result<()> {
        let u = self.user(username).await?;
        let from = if u.role == Role::Admin {
            "admin"
        } else {
            "member"
        };
        self.app
            .set_role_unchecked(u.id, Role::Admin)
            .await
            .map_err(port)?;
        let detail = json!({"username": u.username, "from": from, "to": "admin"});
        self.audit("role.changed", Some(("user", u.id.0.to_string())), detail)
            .await
    }

    /// `cli admin reset-password <username>`: a one-time link (1 hour, single use).
    ///
    /// # Errors
    /// Unknown account, storage errors.
    pub async fn reset_password(&self, username: &str) -> anyhow::Result<String> {
        let u = self.user(username).await?;
        let token = self.auth.create_password_reset(u.id).await.map_err(port)?;
        let detail = json!({
            "username": u.username,
            "valid_minutes": auth::PASSWORD_RESET_MINUTES,
        });
        self.audit(
            "password_reset.created",
            Some(("user", u.id.0.to_string())),
            detail,
        )
        .await?;
        Ok(format!("{}/reset-password/{token}", self.public_url))
    }

    /// `cli admin reset-totp <username>`: remove the authenticator and its recovery codes.
    /// Returns whether one was enrolled.
    ///
    /// # Errors
    /// Unknown account, storage errors.
    pub async fn reset_totp(&self, username: &str) -> anyhow::Result<bool> {
        let u = self.user(username).await?;
        let had = self.auth.reset_totp(u.id).await.map_err(port)?;
        let detail = json!({"username": u.username, "was_enrolled": had});
        self.audit("totp.reset", Some(("user", u.id.0.to_string())), detail)
            .await?;
        Ok(had)
    }

    /// `cli admin disable-sso-only`: back to the sign-in page (`auth.sign_in_mode = page`).
    /// Returns whether anything changed.
    ///
    /// # Errors
    /// The configuration sets the mode to `auto` (change it there; the stored setting would
    /// change nothing, UX3-2), storage errors.
    pub async fn disable_sso_only(&self) -> anyhow::Result<bool> {
        match &self.mode_fixed {
            Some((at, config::SignInMode::Auto)) => bail!(
                "auth.sign_in_mode = auto is set by {at}, which wins over the stored setting; \
                 set it to `page` there (or remove it) and restart the server"
            ),
            // the configuration keeps the sign-in page in force whatever is stored
            Some((_, config::SignInMode::Page)) => return Ok(false),
            None => {}
        }
        let mut st = self.admin.settings().await.map_err(port)?;
        let from = st.sign_in_mode.clone().unwrap_or_else(|| "page".into());
        if from == "page" {
            return Ok(false);
        }
        st.sign_in_mode = Some("page".into());
        st.updated_at = Some(chrono::Utc::now());
        let detail = json!({"key": "auth.sign_in_mode", "from": from, "to": "page"});
        let row = Self::entry(
            "setting.changed",
            Some(("setting", "auth.sign_in_mode".into())),
            detail,
        );
        self.admin.put_settings(&st, &[row]).await.map_err(port)?;
        Ok(true)
    }

    /// `cli setup reopen`: the setup wizard answers again (a wiped admin account).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn setup_reopen(&self) -> anyhow::Result<()> {
        let mut st = self.admin.settings().await.map_err(port)?;
        let was = st.setup_completed_at;
        st.setup_completed_at = None;
        st.setup_step = None;
        let row = Self::entry(
            "setup.reopened",
            None,
            json!({"completed_at": was.map(|t| t.to_rfc3339())}),
        );
        self.admin.put_settings(&st, &[row]).await.map_err(port)
    }

    async fn counts(&self) -> anyhow::Result<std::collections::BTreeMap<u16, u64>> {
        let mut rows = std::collections::BTreeMap::new();
        for r in &self.rows {
            for (v, n) in r.version_counts().await.map_err(port)? {
                *rows.entry(v).or_default() += n;
            }
        }
        Ok(rows)
    }

    /// `cli secrets status`: key versions, rows per version, what is locked.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn secrets_status(&self) -> anyhow::Result<SecretsReport> {
        let rows = self.counts().await?;
        let loaded = self.ring.as_ref().map(|r| r.versions()).unwrap_or_default();
        let active = self.ring.as_ref().map_or(0, |r| r.active_version());
        let mut versions: Vec<u16> = loaded.clone();
        versions.extend(rows.keys().copied().filter(|v| !loaded.contains(v)));
        versions.sort_unstable_by(|a, b| b.cmp(a));
        let versions: Vec<KeyVersionRow> = versions
            .into_iter()
            .map(|v| KeyVersionRow {
                version: v,
                rows: rows.get(&v).copied().unwrap_or(0),
                loaded: loaded.contains(&v),
                active: v == active,
            })
            .collect();
        Ok(SecretsReport {
            key_source: match self.source {
                KeySource::File | KeySource::Generated => "file",
                KeySource::Env => "env",
                KeySource::Ephemeral => "none",
            }
            .into(),
            key_file: self.key_file.display().to_string(),
            active_version: active,
            rows_to_rewrap: versions
                .iter()
                .filter(|v| v.loaded && !v.active)
                .map(|v| v.rows)
                .sum(),
            locked_rows: versions.iter().filter(|v| !v.loaded).map(|v| v.rows).sum(),
            versions,
        })
    }

    fn key_file_only(&self) -> anyhow::Result<()> {
        if self.source != KeySource::File {
            bail!(
                "the keys do not come from the key file {} (APP__SECRETS__KEYS or none); \
                 edit that variable instead",
                self.key_file.display()
            );
        }
        Ok(())
    }

    /// `cli secrets rotate`: a new active key version in the key file; the old ones stay
    /// until nothing uses them. The server loads it on SIGHUP or restart.
    ///
    /// # Errors
    /// Keys not from the key file, I/O, storage errors.
    pub async fn secrets_rotate(&self) -> anyhow::Result<u16> {
        self.key_file_only()?;
        let v = auth::secrets::rotate_key_file(&self.key_file)?;
        self.audit("secrets.rotated", None, json!({"key_version": v}))
            .await?;
        Ok(v)
    }

    /// `cli secrets prune`: drop key versions from the key file that no stored value uses.
    ///
    /// # Errors
    /// Keys not from the key file, I/O, storage errors.
    pub async fn secrets_prune(&self) -> anyhow::Result<Vec<u16>> {
        self.key_file_only()?;
        // A process that has not reloaded still seals new values with the previous active
        // version; dropping that one now would lock them at the next restart (UX3-20).
        let active = self.ring.as_ref().map_or(0, |r| r.active_version());
        let seen = self.admin.key_versions().await.map_err(port)?;
        if !seen.iter().any(|(v, _)| *v == active) {
            bail!(
                "no server process has loaded key version {active} yet: send SIGHUP to every \
                 server and worker process (or restart them) first, then prune"
            );
        }
        let in_use: Vec<u16> = self
            .counts()
            .await?
            .into_iter()
            .filter(|(_, n)| *n > 0)
            .map(|(v, _)| v)
            .collect();
        let dropped = auth::secrets::prune_key_file(&self.key_file, &in_use)?;
        self.audit("secrets.pruned", None, json!({"dropped": dropped}))
            .await?;
        Ok(dropped)
    }

    /// The pool (the CLI's other commands).
    #[must_use]
    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }
}
