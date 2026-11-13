//! UX phase 2 on the server: the admin area (addendum-1 3.4), instance settings with
//! provenance (3.2, OD-10, OD-11), lock-out guards (3.3), the audit log (OD-13),
//! re-authentication (DEC-191), data-source status and credentials (design.md 4) and the
//! setup wizard (design.md 5.1). REST routes (`admin_routes`) and the web app's server
//! functions call these operations.

mod mail;
mod oidc;
mod probes;
mod setup;
mod sources;
mod user_menu;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use app::ports::{HttpFetch, SecretCipher, SecretRows};
use auth::admin::{ActorKind, AdminStore, AuditEntry, AuditFilter, StoredSettings};
use axum::http::StatusCode;
use chrono::{DateTime, Duration, Utc};
use contracts::admin::{
    AdminInviteCreatedDto, AdminInviteDto, AdminOverviewDto, AdminUserDto, AuditEntryDto,
    AuditPageDto, AuditQueryDto, BrandCardDto, CronDto, EvictionDto, FailedJobDto, HealthRowDto,
    InstanceSettingsDto, KeyBackupDto, KeyVersionDto, LaneCountsDto, NewAdminInviteDto, RewrapDto,
    SecretsStatusDto, SecurityDto, SettingDto, SettingsPatchDto, StorageJobsDto,
};
use domain::ids::UserId;
use domain::user::{Actor, Role};
use serde_json::{Value, json};
use uuid::Uuid;

pub use mail::{
    MAIL_KEYS, MailSettings, MailTransport, SharedTransport, mail, rebuild as rebuild_mail,
    send_test_mail, set_mail_password,
};
pub(crate) use mail::{mail_ready, send_mail};
pub use oidc::{
    ENV_VAR as OIDC_ENV_VAR, create_provider, delete_provider, linked_identities, oidc_admin,
    oidc_buttons, set_provider_secret, test_provider, unlink_identity, update_provider,
};
pub use probes::{PROBE_IDS, STALE_SECS, run_probes};
pub use setup::{
    change_password, reauth, reauth_status, setup_account, setup_complete, setup_gate,
    setup_hosting, setup_sources, setup_state, totp_begin, totp_confirm,
};
pub use sources::{
    ask_admin, clear_my_credential, clear_source_credential, load_credentials,
    load_source_switches, my_credentials, search_reasons, set_my_credential, set_source_credential,
    set_source_enabled, set_source_licence, source_status, sources, test_source, user_keys,
};
pub use user_menu::{reset_user_totp, unlink_user_identity};

use crate::error::ApiResult;
use crate::{ApiError, ApiState};

/// Audit retention (OD-13).
pub const AUDIT_RETENTION_DAYS: i64 = 365;
/// OD-09: remind to rotate the key-encryption key after two years.
pub const KEY_ROTATION_DAYS: i64 = 730;
/// Rows per re-wrap batch (addendum-1 2.2).
pub const REWRAP_BATCH: u32 = 100;

/// Where an explicitly set value came from (`config::Origin`, without the dependency).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedBy {
    File,
    Env(String),
}

/// One instance setting the admin UI manages.
#[derive(Debug, Clone, Copy)]
pub struct SettingDef {
    pub key: &'static str,
    pub env: &'static str,
    /// Changing it needs a fresh proof (DEC-191, research 4.2).
    pub gated: bool,
    /// Applies from the next session (OD-11).
    pub next_session: bool,
}

/// Key of the operator contact (an e-mail address or https URL, `catalog_core::check_contact`).
pub const OPERATOR_CONTACT: &str = "instance.operator_contact";

pub const SETTINGS: [SettingDef; 15] = [
    // v1-books: sent to the sources that ask clients to identify themselves
    SettingDef {
        key: OPERATOR_CONTACT,
        env: "APP__INSTANCE__OPERATOR_CONTACT",
        gated: false,
        next_session: false,
    },
    SettingDef {
        key: "instance.hosting",
        env: "APP__INSTANCE__HOSTING",
        gated: false,
        next_session: false,
    },
    SettingDef {
        key: "auth.signup",
        env: "APP__AUTH__SIGNUP",
        gated: true,
        next_session: false,
    },
    SettingDef {
        key: "auth.session_days",
        env: "APP__AUTH__SESSION_DAYS",
        gated: true,
        next_session: true,
    },
    SettingDef {
        key: "auth.secure_cookies",
        env: "APP__AUTH__SECURE_COOKIES",
        gated: true,
        next_session: true,
    },
    SettingDef {
        key: "storage.max_total_bytes",
        env: "APP__STORAGE__MAX_TOTAL_BYTES",
        gated: false,
        next_session: false,
    },
    SettingDef {
        key: "storage.high_watermark",
        env: "APP__STORAGE__HIGH_WATERMARK",
        gated: false,
        next_session: false,
    },
    SettingDef {
        key: "storage.low_watermark",
        env: "APP__STORAGE__LOW_WATERMARK",
        gated: false,
        next_session: false,
    },
    // addendum-1 4.3: defined here, enforced with the OIDC providers (phase 3 WP3-3)
    SettingDef {
        key: "auth.sign_in_mode",
        env: "APP__AUTH__SIGN_IN_MODE",
        gated: true,
        next_session: false,
    },
    SettingDef {
        key: "mail.host",
        env: "APP__NOTIFY__SMTP__HOST",
        gated: true,
        next_session: false,
    },
    SettingDef {
        key: "mail.port",
        env: "APP__NOTIFY__SMTP__PORT",
        gated: true,
        next_session: false,
    },
    SettingDef {
        key: "mail.security",
        env: "APP__NOTIFY__SMTP__SECURITY",
        gated: true,
        next_session: false,
    },
    SettingDef {
        key: "mail.username",
        env: "APP__NOTIFY__SMTP__USERNAME",
        gated: true,
        next_session: false,
    },
    SettingDef {
        key: "mail.from",
        env: "APP__NOTIFY__SMTP__FROM",
        gated: true,
        next_session: false,
    },
    // WP4-5: the logging funnel (Admin > Storage and jobs)
    SettingDef {
        key: "usage.funnel",
        env: "APP__USAGE__FUNNEL",
        gated: false,
        next_session: false,
    },
];

/// Values from the config file and environment: the defaults, and the keys they fix.
#[derive(Debug, Clone, Default)]
pub struct ConfigValues {
    /// The configured value of every key in [`SETTINGS`] (the built-in default unless fixed).
    pub values: BTreeMap<String, Value>,
    /// Keys set explicitly by the file or the environment (read-only in the UI).
    pub fixed: BTreeMap<String, FixedBy>,
    pub public_url: String,
    /// Connector id -> variable that sets its key (`APP__CONNECTORS__TMDB__API_KEY`).
    pub source_env: BTreeMap<String, String>,
    /// Compiled-in sources switched off in the configuration: (id, name, kinds).
    pub disabled_sources: Vec<(String, String, Vec<String>)>,
    /// `[notify.smtp] password` from env or file (used when the configuration sets mail).
    pub mail_password: Option<EnvSecret>,
}

/// A secret value from the configuration; `Debug` never prints it.
#[derive(Clone, Default)]
pub struct EnvSecret(pub zeroize::Zeroizing<String>);

impl std::fmt::Debug for EnvSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

/// The parts of the overview that come from the process configuration.
#[derive(Debug, Clone, Default)]
pub struct HealthInputs {
    /// `meilisearch`, `postgres`, `postgres_fallback`.
    pub search_backend: String,
    pub oidc_configured: bool,
}

/// The loaded secrets key ring (replaceable by SIGHUP, DEC-283) and where it came from.
#[derive(Debug, Clone)]
pub struct KeyInfo {
    pub live: Arc<auth::secrets::LiveKeys>,
    /// Where a reload reads the keys again.
    pub options: auth::secrets::KeyOptions,
    /// `file`, `env`, `generated`, `ephemeral`.
    pub source: String,
    pub key_file: String,
}

impl KeyInfo {
    #[must_use]
    pub fn active(&self) -> u16 {
        self.live.active_version()
    }

    #[must_use]
    pub fn loaded(&self) -> Vec<u16> {
        self.live.versions()
    }
}

/// Everything the admin operations need besides [`ApiState`].
#[derive(Clone)]
pub struct AdminApi {
    pub store: Arc<dyn AdminStore>,
    pub config: Arc<ConfigValues>,
    pub cipher: Arc<dyn SecretCipher>,
    pub keys: KeyInfo,
    pub secret_rows: Vec<Arc<dyn SecretRows>>,
    pub budget: Option<Arc<assets::StorageBudget>>,
    pub queue: Option<jobs::PgQueue>,
    /// The governed client (Test of a data source).
    pub http: Arc<dyn HttpFetch>,
    pub brand: BrandCardDto,
    pub health: HealthInputs,
    /// Live values the request path reads.
    pub secure_cookies: Arc<AtomicBool>,
    pub hosting: Arc<RwLock<String>>,
    /// `instance.operator_contact` as the governed client and the source registry read it.
    pub contact: catalog_core::OperatorContact,
    /// The email transport (swapped when the mail settings change); `None` without the
    /// email channel.
    pub mail: Option<SharedTransport>,
    /// Usage beacons per person and minute (WP4-5).
    pub usage_limiter: Arc<crate::usage::Limiter>,
}

impl AdminApi {
    #[must_use]
    pub fn secure_cookies(&self) -> bool {
        self.secure_cookies.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn hosting(&self) -> String {
        self.hosting
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    #[must_use]
    pub fn hosted(&self) -> bool {
        self.hosting() == "hosted"
    }
}

/// The signed-in caller with the session token (gated actions check the fresh proof).
#[derive(Debug, Clone, Copy)]
pub struct Caller<'a> {
    pub actor: Actor,
    pub token: &'a str,
}

pub(crate) fn admin(s: &ApiState) -> ApiResult<&AdminApi> {
    s.admin.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "the admin area is not enabled",
        )
    })
}

pub(crate) fn require_admin(a: Actor) -> ApiResult<()> {
    if a.is_admin() {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "admins only",
        ))
    }
}

/// A lock-out rule refused the change (addendum-1 3.3).
pub(crate) fn refused(code: &'static str, msg: &str) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, code, msg)
}

pub(crate) fn invalid(code: &'static str, msg: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, code, msg)
}

fn port(e: &app::ports::PortError) -> ApiError {
    tracing::error!(error = %e, "admin storage error");
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        "internal error",
    )
}

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn pe(e: app::ports::PortError) -> ApiError {
    port(&e)
}

pub(crate) fn ts(t: DateTime<Utc>) -> String {
    t.to_rfc3339()
}

fn parse_ts(s: &str) -> ApiResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| ApiError::bad_request(format!("`{s}` is not an RFC 3339 time")))
}

pub(crate) fn parse_uuid(s: &str, what: &'static str) -> ApiResult<Uuid> {
    Uuid::parse_str(s).map_err(|_| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{what} not found"),
        )
    })
}

/// An audit row for `actor` (or the system), with the client address and request id of the
/// request being handled (DEC-284). Pass it to the store write of the change, which writes
/// it in the same transaction (addendum-1 3.4).
pub(crate) async fn entry(
    s: &ApiState,
    actor: Option<Actor>,
    action: &str,
    target: Option<(&str, String)>,
    detail: Value,
) -> AuditEntry {
    let name = match actor {
        Some(x) => s
            .app
            .user_opt(x.user_id)
            .await
            .ok()
            .flatten()
            .map(|u| u.username),
        None => None,
    };
    let meta = crate::request_meta::current();
    AuditEntry {
        id: Uuid::now_v7(),
        at: s.app.clock.now(),
        actor_id: actor.map(|x| x.user_id),
        actor_kind: if actor.is_some() {
            ActorKind::User
        } else {
            ActorKind::System
        },
        actor_name: name,
        action: action.to_string(),
        target_type: target.as_ref().map(|(t, _)| (*t).to_string()),
        target_id: target.map(|(_, id)| id),
        detail,
        client_addr: meta.as_ref().and_then(|m| m.client_addr.clone()),
        request_id: meta.map(|m| m.request_id),
    }
}

/// Write an audit row on its own, after the change: for operations that are not one
/// database write (role changes and account deletion go through the domain repositories
/// with their own transaction, re-encryption, eviction, job retries; DEC-298). A failed
/// write is logged, never shown.
pub(crate) async fn audit(
    s: &ApiState,
    actor: Option<Actor>,
    action: &str,
    target: Option<(&str, String)>,
    detail: Value,
) {
    let Some(a) = s.admin.as_ref() else {
        return;
    };
    let e = entry(s, actor, action, target, detail).await;
    if let Err(err) = a.store.audit(&e).await {
        tracing::error!(error = %err, action, "audit write failed");
    }
}

/// The proof method of the caller's session, recorded with gated actions.
async fn proof(s: &ApiState, token: &str) -> Value {
    s.auth
        .reauth_status(token)
        .await
        .ok()
        .and_then(|r| r.method)
        .map_or(Value::Null, Value::String)
}

/// Check the fresh proof of a gated action (DEC-191).
pub(crate) async fn fresh(s: &ApiState, c: Caller<'_>) -> ApiResult<()> {
    s.auth.require_fresh(c.token).await?;
    Ok(())
}

// ---------------------------------------------------------------- settings

fn stored_value(st: &StoredSettings, key: &str) -> Option<Value> {
    match key {
        "instance.hosting" => st.hosting.clone().map(Value::String),
        "auth.signup" => st.signup.clone().map(Value::String),
        "auth.session_days" => st.session_days.map(Value::from),
        "auth.secure_cookies" => st.secure_cookies.map(Value::Bool),
        "storage.max_total_bytes" => st.storage_max_total_bytes.map(Value::from),
        "storage.high_watermark" => st.storage_high_watermark.map(Value::from),
        "storage.low_watermark" => st.storage_low_watermark.map(Value::from),
        "auth.sign_in_mode" => st.sign_in_mode.clone().map(Value::String),
        "mail.host" => st.mail.host.clone().map(Value::String),
        "mail.port" => st.mail.port.map(Value::from),
        "mail.security" => st.mail.security.clone().map(Value::String),
        "mail.username" => st.mail.username.clone().map(Value::String),
        "mail.from" => st.mail.from.clone().map(Value::String),
        "usage.funnel" => st.usage_funnel.map(Value::Bool),
        OPERATOR_CONTACT => st.operator_contact.clone().map(Value::String),
        _ => None,
    }
}

fn one_of(v: &Value, allowed: &[&str]) -> ApiResult<String> {
    v.as_str()
        .filter(|s| allowed.contains(s))
        .map(String::from)
        .ok_or_else(|| {
            invalid(
                "invalid_setting",
                format!("use one of {}", allowed.join(", ")),
            )
        })
}

fn fraction(v: &Value) -> ApiResult<f64> {
    v.as_f64()
        .filter(|f| *f > 0.0 && *f <= 1.0)
        .ok_or_else(|| invalid("invalid_setting", "a fraction above 0 and at most 1"))
}

/// Validate and store one value (`None`: back to the default).
fn set_stored(st: &mut StoredSettings, key: &str, v: Option<&Value>) -> ApiResult<()> {
    match key {
        "instance.hosting" => st.hosting = v.map(|v| one_of(v, &["home", "hosted"])).transpose()?,
        "auth.signup" => {
            st.signup = v
                .map(|v| one_of(v, &["open", "invite", "closed"]))
                .transpose()?;
        }
        "auth.session_days" => {
            st.session_days = v
                .map(|v| {
                    v.as_u64()
                        .filter(|d| (1..=365).contains(d))
                        .and_then(|d| u32::try_from(d).ok())
                        .ok_or_else(|| invalid("invalid_setting", "1 to 365 days"))
                })
                .transpose()?;
        }
        "auth.secure_cookies" => {
            st.secure_cookies = v
                .map(|v| {
                    v.as_bool()
                        .ok_or_else(|| invalid("invalid_setting", "true or false"))
                })
                .transpose()?;
        }
        "storage.max_total_bytes" => {
            st.storage_max_total_bytes = v
                .map(|v| match v {
                    Value::String(t) => t.parse::<assets::ByteSize>().map(|b| b.0).ok(),
                    _ => v.as_u64(),
                })
                .map(|b| {
                    b.filter(|b| *b > 0)
                        .ok_or_else(|| invalid("invalid_setting", "a size such as 20GiB"))
                })
                .transpose()?;
        }
        "storage.high_watermark" => st.storage_high_watermark = v.map(fraction).transpose()?,
        "storage.low_watermark" => st.storage_low_watermark = v.map(fraction).transpose()?,
        "auth.sign_in_mode" => {
            st.sign_in_mode = v.map(|v| one_of(v, &["page", "auto"])).transpose()?;
        }
        "usage.funnel" => {
            st.usage_funnel = v
                .map(|v| {
                    v.as_bool()
                        .ok_or_else(|| invalid("invalid_setting", "true or false"))
                })
                .transpose()?;
        }
        OPERATOR_CONTACT => {
            // an empty text clears it, like `null`
            st.operator_contact = v
                .filter(|v| v.as_str().is_none_or(|s| !s.trim().is_empty()))
                .map(|v| {
                    v.as_str()
                        .ok_or(catalog_core::ContactError::Form)
                        .and_then(catalog_core::check_contact)
                        .map_err(|e| invalid("invalid_contact", e.to_string()))
                })
                .transpose()?;
        }
        k if k.starts_with("mail.") => set_mail(st, k, v)?,
        _ => {
            return Err(invalid(
                "unknown_setting",
                format!("unknown setting `{key}`"),
            ));
        }
    }
    Ok(())
}

/// A short single-line text (host names, addresses).
fn line(v: &Value, what: &str) -> ApiResult<String> {
    v.as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty() && t.len() <= 320 && !t.contains(['\n', '\r']))
        .map(String::from)
        .ok_or_else(|| invalid("invalid_setting", what.to_string()))
}

fn set_mail(st: &mut StoredSettings, key: &str, v: Option<&Value>) -> ApiResult<()> {
    let m = &mut st.mail;
    match key {
        "mail.host" => {
            m.host = v.map(|v| line(v, "a host name")).transpose()?;
            if m.host.as_ref().is_some_and(|h| h.contains(' ')) {
                return Err(invalid("invalid_setting", "a host name"));
            }
        }
        "mail.port" => {
            m.port = v
                .map(|v| {
                    v.as_u64()
                        .and_then(|p| u16::try_from(p).ok())
                        .filter(|p| *p > 0)
                        .ok_or_else(|| invalid("invalid_setting", "a port from 1 to 65535"))
                })
                .transpose()?;
        }
        "mail.security" => {
            m.security = v
                .map(|v| one_of(v, &["tls", "starttls", "none"]))
                .transpose()?;
        }
        "mail.username" => m.username = v.map(|v| line(v, "a user name")).transpose()?,
        "mail.from" => {
            m.from = v
                .map(|v| line(v, "an address such as Shelf <noreply@example.org>"))
                .transpose()?;
            if m.from.as_ref().is_some_and(|f| !f.contains('@')) {
                return Err(invalid(
                    "invalid_setting",
                    "an address such as Shelf <noreply@example.org>",
                ));
            }
        }
        _ => {
            return Err(invalid(
                "unknown_setting",
                format!("unknown setting `{key}`"),
            ));
        }
    }
    Ok(())
}

/// The effective value of one managed setting (`Null` for an unknown key).
pub(crate) fn effective_value(a: &AdminApi, st: &StoredSettings, key: &str) -> Value {
    effective(a, st).remove(key).unwrap_or(Value::Null)
}

/// Effective values: file/env > database > default.
fn effective(a: &AdminApi, st: &StoredSettings) -> BTreeMap<String, Value> {
    SETTINGS
        .iter()
        .map(|d| {
            let configured = a.config.values.get(d.key).cloned().unwrap_or(Value::Null);
            let v = if a.config.fixed.contains_key(d.key) {
                configured
            } else {
                stored_value(st, d.key).unwrap_or(configured)
            };
            (d.key.to_string(), v)
        })
        .collect()
}

/// Is sign-in mode `auto` in force (addendum-1 4.3)? Read from the database on each call,
/// so `cli admin disable-sso-only` and other processes take effect at once; storage errors
/// count as `page` (the sign-in page is never hidden by a hiccup).
pub async fn sign_in_auto(s: &ApiState) -> bool {
    let Some(a) = s.admin.as_ref() else {
        return false;
    };
    match a.store.settings().await {
        Ok(st) => effective(a, &st).get("auth.sign_in_mode") == Some(&Value::from("auto")),
        Err(e) => {
            tracing::warn!(error = %e, "reading the sign-in mode failed; using the sign-in page");
            false
        }
    }
}

fn setting_dtos(a: &AdminApi, st: &StoredSettings, keys: &[&str]) -> Vec<SettingDto> {
    let eff = effective(a, st);
    SETTINGS
        .iter()
        .filter(|d| keys.is_empty() || keys.contains(&d.key))
        .map(|d| {
            let fixed = a.config.fixed.get(d.key);
            let stored = stored_value(st, d.key);
            SettingDto {
                key: d.key.into(),
                value: eff.get(d.key).cloned().unwrap_or(Value::Null),
                source: match (fixed, &stored) {
                    (Some(FixedBy::Env(_)), _) => "env",
                    (Some(FixedBy::File), _) => "file",
                    (None, Some(_)) => "db",
                    (None, None) => "default",
                }
                .into(),
                env_var: match fixed {
                    Some(FixedBy::Env(v)) => Some(v.clone()),
                    _ => None,
                },
                env_name: d.env.into(),
                editable: fixed.is_none(),
                shadowed_value: fixed.and(stored),
                needs_reauth: d.gated,
                applies_next_session: d.next_session,
            }
        })
        .collect()
}

fn https(a: &AdminApi) -> bool {
    a.config.public_url.starts_with("https://")
}

fn warnings(a: &AdminApi, eff: &BTreeMap<String, Value>) -> Vec<String> {
    let mut w = Vec::new();
    if !https(a) {
        w.push("public_url_not_https".into());
        if eff.get("auth.secure_cookies") == Some(&Value::Bool(true)) {
            w.push("secure_cookies_over_http".into());
        }
    }
    w
}

/// Push the effective values into the running services (sign-up mode, session length,
/// cookie flag, storage limits, hosting mode). Called on start and after every change.
pub fn apply(s: &ApiState, st: &StoredSettings) {
    let Some(a) = s.admin.as_ref() else {
        return;
    };
    let eff = effective(a, st);
    let get = |k: &str| eff.get(k).cloned().unwrap_or(Value::Null);
    s.auth.update_settings(|x| {
        x.signup = match get("auth.signup").as_str() {
            Some("open") => auth::SignupMode::Open,
            Some("closed") => auth::SignupMode::Closed,
            _ => auth::SignupMode::Invite,
        };
        if let Some(d) = get("auth.session_days")
            .as_u64()
            .and_then(|d| u32::try_from(d).ok())
        {
            x.session_days = d.max(1);
        }
    });
    a.secure_cookies.store(
        get("auth.secure_cookies").as_bool().unwrap_or(true),
        Ordering::Relaxed,
    );
    if let Some(h) = get("instance.hosting").as_str() {
        *a.hosting
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = h.to_string();
    }
    // a value the check refuses (a configuration typo) leaves the sources without a contact
    if let Err(e) = a.contact.set(get(OPERATOR_CONTACT).as_str()) {
        tracing::warn!(error = %e, "instance.operator_contact is not usable; sending none");
        let _ = a.contact.set(None);
    }
    if let Some(b) = &a.budget {
        let max = get("storage.max_total_bytes")
            .as_u64()
            .map(assets::ByteSize);
        let high = get("storage.high_watermark").as_f64();
        let low = get("storage.low_watermark").as_f64();
        if let (Some(max), Some(high), Some(low)) = (max, high, low)
            && let Err(e) = b.set_limits(max, high, low)
        {
            tracing::warn!(error = %e, "stored storage limits are invalid; keeping the previous ones");
        }
    }
}

/// Read the stored settings and apply them (start-up).
///
/// # Errors
/// Storage errors.
pub async fn load_settings(s: &ApiState) -> ApiResult<()> {
    let a = admin(s)?;
    let st = a.store.settings().await.map_err(pe)?;
    apply(s, &st);
    Ok(())
}

/// Admin > Instance: every managed setting with its provenance.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn settings(s: &ApiState, actor: Actor) -> ApiResult<InstanceSettingsDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let st = a.store.settings().await.map_err(pe)?;
    let eff = effective(a, &st);
    Ok(InstanceSettingsDto {
        settings: setting_dtos(a, &st, &[]),
        warnings: warnings(a, &eff),
        public_url: a.config.public_url.clone(),
    })
}

/// Change settings. Keys set by env or file are refused (`fixed_by_config`); sign-in
/// related keys need a fresh proof; secure cookies over plain HTTP are refused because they
/// would lock everyone out (OD-11). Each change writes an audit row with old and new value.
///
/// # Errors
/// See above; `invalid_setting`, `unknown_setting`.
pub async fn update_settings(
    s: &ApiState,
    c: Caller<'_>,
    patch: SettingsPatchDto,
) -> ApiResult<InstanceSettingsDto> {
    require_admin(c.actor)?;
    let a = admin(s)?;
    let defs: Vec<SettingDef> = patch
        .values
        .keys()
        .map(|k| {
            SETTINGS
                .iter()
                .find(|d| d.key == k)
                .copied()
                .ok_or_else(|| invalid("unknown_setting", format!("unknown setting `{k}`")))
        })
        .collect::<ApiResult<_>>()?;
    if let Some(d) = defs.iter().find(|d| a.config.fixed.contains_key(d.key)) {
        return Err(refused(
            "fixed_by_config",
            &format!("{} is set by the configuration ({})", d.key, d.env),
        ));
    }
    if defs.iter().any(|d| d.gated) {
        fresh(s, c).await?;
    }
    let old = a.store.settings().await.map_err(pe)?;
    let mut next = old.clone();
    for (k, v) in &patch.values {
        set_stored(&mut next, k, (!v.is_null()).then_some(v))?;
    }
    let eff = effective(a, &next);
    if eff.get("auth.secure_cookies") == Some(&Value::Bool(true))
        && patch.values.contains_key("auth.secure_cookies")
        && !https(a)
    {
        return Err(refused(
            "secure_cookies_need_https",
            "secure cookies need an https public address",
        ));
    }
    // addendum-1 4.3: the preconditions of `auto` (the fresh proof was checked above)
    if patch.values.contains_key("auth.sign_in_mode")
        && eff.get("auth.sign_in_mode") == Some(&Value::from("auto"))
    {
        oidc::check_auto(s).await?;
    }
    check_storage(a, &eff)?;
    next.updated_at = Some(s.app.clock.now());
    next.updated_by = Some(c.actor.user_id);
    let before = effective(a, &old);
    let method = proof(s, c.token).await;
    let mut rows = Vec::with_capacity(patch.values.len());
    for k in patch.values.keys() {
        let detail = json!({"key": k, "from": before.get(k), "to": eff.get(k), "proof": method});
        rows.push(
            entry(
                s,
                Some(c.actor),
                "setting.changed",
                Some(("setting", k.clone())),
                detail,
            )
            .await,
        );
    }
    a.store.put_settings(&next, &rows).await.map_err(pe)?;
    apply(s, &next);
    if patch.values.contains_key(crate::usage::SETTING) {
        a.usage_limiter.forget_setting();
    }
    let mail_ok =
        !patch.values.keys().any(|k| k.starts_with("mail.")) || mail::rebuild(s).await.is_ok();
    if patch.values.keys().any(|k| k.starts_with("mail.")) {
        probes::refresh_probe(s, "mail").await;
    }
    let mut out = settings(s, c.actor).await?;
    if !mail_ok {
        // stored, but mail is off until the settings are fixed (UX3-18)
        out.warnings.push("mail_transport_unusable".into());
    }
    Ok(out)
}

fn check_storage(a: &AdminApi, eff: &BTreeMap<String, Value>) -> ApiResult<()> {
    let Some(b) = &a.budget else {
        return Ok(());
    };
    let mut cfg = b.config();
    cfg.max_total_bytes = assets::ByteSize(
        eff.get("storage.max_total_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(cfg.max_total_bytes.0),
    );
    cfg.high_watermark = eff
        .get("storage.high_watermark")
        .and_then(Value::as_f64)
        .unwrap_or(cfg.high_watermark);
    cfg.low_watermark = eff
        .get("storage.low_watermark")
        .and_then(Value::as_f64)
        .unwrap_or(cfg.low_watermark);
    if cfg.http_cache_max_bytes > cfg.max_total_bytes {
        cfg.http_cache_max_bytes = cfg.max_total_bytes;
    }
    cfg.validate().map_err(|e| invalid("invalid_setting", e))
}

// ---------------------------------------------------------------- users and invites

async fn admin_user(s: &ApiState, u: &domain::user::User, me: UserId) -> ApiResult<AdminUserDto> {
    let store = s.auth.store();
    let creds = store.credentials(u.id).await?;
    let mut with = Vec::new();
    if creds.as_ref().is_some_and(|c| c.password_hash.is_some()) {
        with.push("password".to_string());
    }
    if creds.as_ref().is_some_and(|c| c.totp_enabled) {
        with.push("totp".to_string());
    }
    if !store.identities_of(u.id).await?.is_empty() {
        with.push("oidc".to_string());
    }
    let last_seen = store
        .sessions_for(u.id)
        .await?
        .iter()
        .map(|x| x.last_seen_at)
        .max();
    Ok(AdminUserDto {
        id: u.id.0.to_string(),
        username: u.username.clone(),
        display_name: u.display_name.clone(),
        email: u.email.clone(),
        role: role_str(u.role).into(),
        disabled: s.auth.is_disabled(u.id).await?,
        signs_in_with: with,
        last_seen_at: last_seen.map(ts),
        created_at: ts(u.created_at),
        is_self: u.id == me,
        role_via_provider: oidc::role_via_provider(s, u.id).await?,
        identities: oidc::linked_identities(s, u.id).await?,
    })
}

const fn role_str(r: Role) -> &'static str {
    match r {
        Role::Admin => "admin",
        Role::Member => "member",
    }
}

/// Admin > Users.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn users(s: &ApiState, actor: Actor) -> ApiResult<Vec<AdminUserDto>> {
    require_admin(actor)?;
    let mut out = Vec::new();
    for u in s.app.list_users().await? {
        out.push(admin_user(s, &u, actor.user_id).await?);
    }
    Ok(out)
}

/// Admins that can still sign in, other than `except`.
async fn other_active_admins(s: &ApiState, except: UserId) -> ApiResult<usize> {
    let mut n = 0;
    for u in s.app.list_users().await? {
        if u.role == Role::Admin && u.id != except && !s.auth.is_disabled(u.id).await? {
            n += 1;
        }
    }
    Ok(n)
}

/// The change would leave no admin who can sign in (addendum-1 3.3).
async fn guard_last_admin(s: &ApiState, target: &domain::user::User) -> ApiResult<()> {
    if target.role == Role::Admin && other_active_admins(s, target.id).await? == 0 {
        return Err(refused(
            "last_admin",
            "this is the last admin; make someone else admin first",
        ));
    }
    Ok(())
}

async fn target_user(s: &ApiState, id: &str) -> ApiResult<domain::user::User> {
    Ok(s.app.user(UserId(parse_uuid(id, "user")?)).await?)
}

/// Grant or revoke the admin role (gated). Own role changes are refused, the last admin
/// cannot be demoted.
///
/// # Errors
/// `self_role_change`, `last_admin`, `reauth_required`, `invalid_setting`.
pub async fn set_role(
    s: &ApiState,
    c: Caller<'_>,
    id: &str,
    role: &str,
) -> ApiResult<AdminUserDto> {
    require_admin(c.actor)?;
    let role = match role {
        "admin" => Role::Admin,
        "member" => Role::Member,
        _ => return Err(invalid("invalid_setting", "role is admin or member")),
    };
    // the lock-out guards first: a refused change must not slide the proof (UX2-11)
    let target = target_user(s, id).await?;
    if target.id == c.actor.user_id {
        return Err(refused(
            "self_role_change",
            "you cannot change your own role",
        ));
    }
    if let Some(p) = oidc::role_via_provider(s, target.id).await? {
        return Err(refused(
            "role_managed_by_provider",
            &format!("this role comes from the groups at {p}"),
        ));
    }
    if role == Role::Member {
        guard_last_admin(s, &target).await?;
    }
    fresh(s, c).await?;
    let from = role_str(target.role);
    let user = s.app.set_role_unchecked(target.id, role).await?;
    let detail = json!({"from": from, "to": role_str(role), "proof": proof(s, c.token).await});
    audit(
        s,
        Some(c.actor),
        "role.changed",
        Some(("user", id.to_string())),
        detail,
    )
    .await;
    admin_user(s, &user, c.actor.user_id).await
}

/// Disable or enable an account (gated). Disabling yourself or the last admin is refused.
///
/// # Errors
/// `self_disable`, `last_admin`, `reauth_required`.
pub async fn set_disabled(
    s: &ApiState,
    c: Caller<'_>,
    id: &str,
    disabled: bool,
) -> ApiResult<AdminUserDto> {
    require_admin(c.actor)?;
    let target = target_user(s, id).await?;
    if disabled {
        if target.id == c.actor.user_id {
            return Err(refused(
                "self_disable",
                "you cannot disable your own account",
            ));
        }
        guard_last_admin(s, &target).await?;
    }
    fresh(s, c).await?;
    let action = if disabled {
        "user.disabled"
    } else {
        "user.enabled"
    };
    let detail = json!({"username": target.username, "proof": proof(s, c.token).await});
    let row = entry(
        s,
        Some(c.actor),
        action,
        Some(("user", id.to_string())),
        detail,
    )
    .await;
    s.auth
        .set_disabled(target.id, Some(c.actor.user_id), disabled, Some(&row))
        .await?;
    admin_user(s, &target, c.actor.user_id).await
}

/// Delete an account and everything it owns (gated). Not yourself, not the last admin.
///
/// # Errors
/// `self_delete`, `last_admin`, `reauth_required`.
pub async fn delete_user(s: &ApiState, c: Caller<'_>, id: &str) -> ApiResult<()> {
    require_admin(c.actor)?;
    let target = target_user(s, id).await?;
    if target.id == c.actor.user_id {
        return Err(refused(
            "self_delete",
            "you cannot delete your own account here",
        ));
    }
    guard_last_admin(s, &target).await?;
    fresh(s, c).await?;
    s.app.delete_user(c.actor, target.id).await?;
    let detail = json!({"username": target.username, "proof": proof(s, c.token).await});
    audit(
        s,
        Some(c.actor),
        "user.deleted",
        Some(("user", id.to_string())),
        detail,
    )
    .await;
    Ok(())
}

fn invite_dto(i: &auth::SignupInvite, now: DateTime<Utc>) -> AdminInviteDto {
    let state = if i.revoked_at.is_some() {
        "revoked"
    } else if i.used_at.is_some() {
        "used"
    } else if i.expires_at <= now {
        "expired"
    } else {
        "open"
    };
    AdminInviteDto {
        id: i.id.to_string(),
        email: i.email.clone(),
        note: i.note.clone(),
        created_by: i.created_by.map(|u| u.0.to_string()),
        created_at: ts(i.created_at),
        expires_at: ts(i.expires_at),
        state: state.into(),
        used_by: i.used_by.map(|u| u.0.to_string()),
    }
}

/// Admin invites, newest first.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn invites(s: &ApiState, actor: Actor) -> ApiResult<Vec<AdminInviteDto>> {
    require_admin(actor)?;
    let now = s.app.clock.now();
    Ok(s.auth
        .signup_invites()
        .await?
        .iter()
        .map(|i| invite_dto(i, now))
        .collect())
}

/// A new invite; the token and link are shown once.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn create_invite(
    s: &ApiState,
    actor: Actor,
    b: NewAdminInviteDto,
) -> ApiResult<AdminInviteCreatedDto> {
    require_admin(actor)?;
    let email = b.email.filter(|e| !e.trim().is_empty());
    let note = b.note.filter(|n| !n.trim().is_empty());
    // the target id is filled in with the new invite's id
    let row = entry(s, Some(actor), "invite.created", None, json!({})).await;
    let (inv, token) = s
        .auth
        .create_signup_invite(actor.user_id, email, note, Some(row))
        .await?;
    Ok(AdminInviteCreatedDto {
        invite: invite_dto(&inv, s.app.clock.now()),
        url: format!(
            "{}/sign-up?invite={token}",
            s.public_url.trim_end_matches('/')
        ),
        token,
    })
}

/// # Errors
/// Not an admin, unknown invite, storage errors.
pub async fn revoke_invite(s: &ApiState, actor: Actor, id: &str) -> ApiResult<AdminInviteDto> {
    require_admin(actor)?;
    let row = entry(
        s,
        Some(actor),
        "invite.revoked",
        Some(("invite", id.to_string())),
        json!({}),
    )
    .await;
    let inv = s
        .auth
        .revoke_signup_invite(parse_uuid(id, "invite")?, Some(&row))
        .await?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "invite not found"))?;
    Ok(invite_dto(&inv, s.app.clock.now()))
}

// ---------------------------------------------------------------- audit log

fn audit_dto(e: AuditEntry) -> AuditEntryDto {
    AuditEntryDto {
        id: e.id.to_string(),
        at: ts(e.at),
        actor_kind: e.actor_kind.as_str().into(),
        actor_id: e.actor_id.map(|u| u.0.to_string()),
        actor_name: e.actor_name,
        action: e.action,
        target_type: e.target_type,
        target_id: e.target_id,
        detail: e.detail,
        client_addr: e.client_addr,
        request_id: e.request_id,
    }
}

/// The audit log with filters, newest first.
///
/// # Errors
/// Not an admin, bad filters, storage errors.
pub async fn audit_log(s: &ApiState, actor: Actor, q: AuditQueryDto) -> ApiResult<AuditPageDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let filter = AuditFilter {
        actor: q
            .actor
            .as_deref()
            .map(|id| parse_uuid(id, "user").map(UserId))
            .transpose()?,
        action: q.action.filter(|x| !x.is_empty()),
        target_type: q.target_type.filter(|x| !x.is_empty()),
        before: q.before.as_deref().map(parse_ts).transpose()?,
        after: q.after.as_deref().map(parse_ts).transpose()?,
        limit,
    };
    let rows = a.store.audit_query(&filter).await.map_err(pe)?;
    let next_before = (rows.len() == limit as usize)
        .then(|| rows.last().map(|e| ts(e.at)))
        .flatten();
    Ok(AuditPageDto {
        entries: rows.into_iter().map(audit_dto).collect(),
        next_before,
    })
}

/// One CSV cell. A value a spreadsheet would run as a formula gets a leading `'` (the
/// helper the library export shares, UX2-9, UX3-5; usernames and invite notes are free
/// text); commas, quotes and line breaks are quoted.
pub(crate) fn csv_field(v: &str) -> String {
    let v = importers::cell::guard(v).into_owned();
    if v.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// The same rows as CSV (Export CSV).
///
/// # Errors
/// As [`audit_log`].
pub async fn audit_csv(s: &ApiState, actor: Actor, q: AuditQueryDto) -> ApiResult<String> {
    let page = audit_log(s, actor, q).await?;
    let mut out =
        String::from("at,actor,action,target_type,target_id,detail,client_addr,request_id\n");
    for e in page.entries {
        let actor = e.actor_name.unwrap_or(e.actor_kind);
        let cols = [
            e.at,
            actor,
            e.action,
            e.target_type.unwrap_or_default(),
            e.target_id.unwrap_or_default(),
            e.detail.to_string(),
            e.client_addr.unwrap_or_default(),
            e.request_id.unwrap_or_default(),
        ];
        let line: Vec<String> = cols.iter().map(|c| csv_field(c)).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    Ok(out)
}

/// Delete audit rows older than [`AUDIT_RETENTION_DAYS`] (daily job).
///
/// # Errors
/// Storage errors.
pub async fn purge_audit(
    store: &dyn AdminStore,
    now: DateTime<Utc>,
) -> Result<u64, app::ports::PortError> {
    store
        .audit_purge(now - Duration::days(AUDIT_RETENTION_DAYS))
        .await
}

// ---------------------------------------------------------------- secrets

/// Rows per key version over every purpose, the locked count and the reminder (OD-09).
///
/// # Errors
/// Storage errors.
pub async fn secrets_status(s: &ApiState) -> ApiResult<SecretsStatusDto> {
    let a = admin(s)?;
    let mut rows: BTreeMap<u16, u64> = BTreeMap::new();
    for r in &a.secret_rows {
        for (v, n) in r
            .version_counts()
            .await
            .map_err(|e| port(&app::ports::PortError(e.to_string())))?
        {
            *rows.entry(v).or_default() += n;
        }
    }
    let seen: BTreeMap<u16, DateTime<Utc>> = a
        .store
        .key_versions()
        .await
        .map_err(pe)?
        .into_iter()
        .collect();
    let loaded = a.keys.loaded();
    let active = a.keys.active();
    let mut versions: Vec<u16> = loaded.clone();
    versions.extend(rows.keys().copied().filter(|v| !loaded.contains(v)));
    versions.sort_unstable_by(|x, y| y.cmp(x));
    let now = s.app.clock.now();
    let dtos: Vec<KeyVersionDto> = versions
        .iter()
        .map(|v| {
            let n = rows.get(v).copied().unwrap_or(0);
            KeyVersionDto {
                version: *v,
                active: *v == active,
                loaded: loaded.contains(v),
                rows: n,
                first_seen_at: seen.get(v).copied().map(ts),
                removable: *v != active && loaded.contains(v) && n == 0,
            }
        })
        .collect();
    let locked_rows = dtos.iter().filter(|d| !d.loaded).map(|d| d.rows).sum();
    let rows_to_rewrap = dtos
        .iter()
        .filter(|d| d.loaded && !d.active)
        .map(|d| d.rows)
        .sum();
    let rotation_due = seen
        .get(&active)
        .is_some_and(|t| now - *t > Duration::days(KEY_ROTATION_DAYS));
    Ok(SecretsStatusDto {
        key_source: a.keys.source.clone(),
        key_file: a.keys.key_file.clone(),
        active_version: active,
        versions: dtos,
        locked_rows,
        rows_to_rewrap,
        rotation_due,
    })
}

/// Re-wrap every value on an older key version with the active one (KEK rotation); runs
/// in batches, idempotent, counts go to the audit log (never values).
///
/// # Errors
/// Storage errors.
pub async fn rewrap_all(a: &AdminApi) -> Result<RewrapDto, app::ports::RepoError> {
    let (mut rewrapped, mut locked) = (0, 0);
    for r in &a.secret_rows {
        // keyset paging over the whole table: locked rows are passed once and counted once
        let mut after = None;
        loop {
            let b = r
                .rewrap_batch(a.cipher.as_ref(), a.keys.active(), after, REWRAP_BATCH)
                .await?;
            rewrapped += b.rewrapped;
            locked += b.locked;
            match b.last {
                Some(last) => after = Some(last),
                None => break,
            }
        }
    }
    Ok(RewrapDto { rewrapped, locked })
}

/// "Re-encrypt now" (admin; not gated, audited).
///
/// # Errors
/// Not an admin, storage errors.
pub async fn rewrap(s: &ApiState, actor: Actor) -> ApiResult<RewrapDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let r = rewrap_all(a)
        .await
        .map_err(|e| port(&app::ports::PortError(e.to_string())))?;
    let detail =
        json!({"rewrapped": r.rewrapped, "locked": r.locked, "key_version": a.keys.active()});
    audit(s, Some(actor), "secrets.rewrapped", None, detail).await;
    Ok(r)
}

/// The key an admin still has to confirm the backup of (OPS-4, DEC-620): the active
/// version, unless it is confirmed or the key is the temporary one of a test harness.
async fn key_backup_due(a: &AdminApi) -> ApiResult<Option<KeyBackupDto>> {
    if a.keys.source == "ephemeral" {
        return Ok(None);
    }
    let version = a.keys.active();
    if a.store
        .key_backup_confirmed(version)
        .await
        .map_err(pe)?
        .is_some()
    {
        return Ok(None);
    }
    Ok(Some(KeyBackupDto {
        version,
        key_file: (a.keys.source != "env").then(|| a.keys.key_file.clone()),
    }))
}

/// "I have backed up the key" on Admin > Overview (gated, audited with the key version and
/// the proof method; never the key). Confirming twice is no error and writes one audit row.
///
/// # Errors
/// Not an admin, `reauth_required`, storage errors.
pub async fn confirm_key_backup(s: &ApiState, c: Caller<'_>) -> ApiResult<()> {
    require_admin(c.actor)?;
    let a = admin(s)?;
    // the gate comes first, also for the temporary key of test harnesses (nothing to
    // record there): the page behaves the same on every instance (review V1OPS-8)
    fresh(s, c).await?;
    if a.keys.source == "ephemeral" {
        return Ok(());
    }
    let version = a.keys.active();
    let detail = json!({"key_version": version, "proof": proof(s, c.token).await});
    let e = entry(s, Some(c.actor), "secrets.backup_confirmed", None, detail).await;
    a.store
        .confirm_key_backup(version, s.app.clock.now(), &e)
        .await
        .map_err(pe)?;
    Ok(())
}

/// Key versions that still encrypt at least one stored value.
///
/// # Errors
/// Storage errors.
pub async fn versions_in_use(a: &AdminApi) -> Result<Vec<u16>, app::ports::RepoError> {
    let mut used = Vec::new();
    for r in &a.secret_rows {
        for (v, n) in r.version_counts().await? {
            if n > 0 && !used.contains(&v) {
                used.push(v);
            }
        }
    }
    used.sort_unstable();
    Ok(used)
}

/// SIGHUP (DEC-283): read the key file / `APP__SECRETS__KEYS` again and use the new ring,
/// unless it drops a version that still encrypts stored values; then the loaded ring stays
/// and the reason is returned (and logged by the caller). Audited as the system.
///
/// # Errors
/// A temporary key (nothing to re-read), an unreadable key source, a dropped version in use,
/// storage errors; the loaded keys are kept in every case.
pub async fn reload_keys(s: &ApiState) -> Result<Vec<u16>, String> {
    let a = admin(s).map_err(|e| e.message)?;
    if a.keys.source == "ephemeral" {
        return Err("a temporary key is in use; there is nothing to re-read".into());
    }
    let next = auth::secrets::read_keys(&a.keys.options).map_err(|e| e.to_string())?;
    let in_use = versions_in_use(a).await.map_err(|e| e.to_string())?;
    let before = a.keys.loaded();
    a.keys
        .live
        .replace(next, &in_use)
        .map_err(|e| e.to_string())?;
    let loaded = a.keys.loaded();
    a.store
        .record_key_versions(&loaded, s.app.clock.now())
        .await
        .map_err(|e| e.to_string())?;
    let detail = json!({"from": before, "to": loaded, "active": a.keys.active()});
    audit(s, None, "secrets.reloaded", None, detail).await;
    Ok(loaded)
}

/// Admin > Security and sessions.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn security(s: &ApiState, actor: Actor) -> ApiResult<SecurityDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let st = a.store.settings().await.map_err(pe)?;
    let keys = ["auth.signup", "auth.session_days", "auth.secure_cookies"];
    Ok(SecurityDto {
        settings: setting_dtos(a, &st, &keys),
        warnings: warnings(a, &effective(a, &st)),
        secrets: secrets_status(s).await?,
        reauth_slide_minutes: auth::REAUTH_SLIDE_MINUTES,
        reauth_cap_minutes: auth::REAUTH_CAP_MINUTES,
    })
}

// ---------------------------------------------------------------- storage and jobs

fn lane_str(l: app::ports::Lane) -> &'static str {
    match l {
        app::ports::Lane::Interactive => "interactive",
        app::ports::Lane::UserImport => "user_import",
        app::ports::Lane::BackgroundRefresh => "background_refresh",
        app::ports::Lane::Prefetch => "prefetch",
    }
}

fn job_err(e: &jobs::QueueError) -> ApiError {
    tracing::error!(error = %e, "job queue error");
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal",
        "internal error",
    )
}

/// Admin > Storage and jobs (OD-14).
///
/// # Errors
/// Not an admin, storage errors.
pub async fn storage_jobs(s: &ApiState, actor: Actor) -> ApiResult<StorageJobsDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let budget = a
        .budget
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no storage budget"))?;
    let r = budget.report().await.map_err(|e| {
        tracing::error!(error = %e, "storage report failed");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal error",
        )
    })?;
    let (lanes, failed, cron) = match &a.queue {
        Some(q) => (
            q.lane_counts().await.map_err(|e| job_err(&e))?,
            q.dead_jobs(50).await.map_err(|e| job_err(&e))?,
            q.cron_list().await.map_err(|e| job_err(&e))?,
        ),
        None => (vec![], vec![], vec![]),
    };
    let st = a.store.settings().await.map_err(pe)?;
    Ok(StorageJobsDto {
        backend: serde_json::to_value(r.backend)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default(),
        used_bytes: r.usage.total,
        pinned_bytes: r.usage.pinned,
        cache_bytes: r.usage.cache,
        max_total_bytes: r.max_total_bytes,
        effective_cap: r.effective_cap,
        high_watermark_bytes: r.high_watermark_bytes,
        low_watermark_bytes: r.low_watermark_bytes,
        at_hard_cap: r.at_hard_cap,
        evictable_bytes: r.eviction.bytes,
        settings: setting_dtos(
            a,
            &st,
            &[
                "storage.max_total_bytes",
                "storage.high_watermark",
                "storage.low_watermark",
                crate::usage::SETTING,
            ],
        ),
        lanes: lanes
            .into_iter()
            .map(|l| LaneCountsDto {
                lane: lane_str(l.lane).into(),
                queued: l.queued,
                running: l.running,
                dead: l.dead,
            })
            .collect(),
        failed: failed
            .into_iter()
            .map(|j| FailedJobDto {
                id: j.id.to_string(),
                lane: lane_str(j.lane).into(),
                kind: j.kind,
                attempts: j.attempts,
                last_error: j.last_error,
                failed_at: ts(j.updated_at),
            })
            .collect(),
        cron: cron
            .into_iter()
            .map(|c| CronDto {
                name: c.name,
                schedule: c.schedule,
                kind: c.kind,
                next_run_at: ts(c.next_run_at),
            })
            .collect(),
    })
}

/// "Run eviction now".
///
/// # Errors
/// Not an admin, storage errors.
pub async fn run_eviction(s: &ApiState, actor: Actor) -> ApiResult<EvictionDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let budget = a
        .budget
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no storage budget"))?;
    let r = budget.evict().await.map_err(|e| {
        tracing::error!(error = %e, "eviction failed");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "internal error",
        )
    })?;
    let detail = json!({"objects": r.objects, "bytes": r.bytes});
    audit(s, Some(actor), "eviction.run", None, detail).await;
    Ok(EvictionDto {
        objects: r.objects,
        bytes: r.bytes,
        before: r.before,
        after: r.after,
    })
}

/// Retry (`retry = true`) or discard a failed job.
///
/// # Errors
/// Not an admin, unknown or not failed job.
pub async fn failed_job(s: &ApiState, actor: Actor, id: &str, retry: bool) -> ApiResult<()> {
    require_admin(actor)?;
    let a = admin(s)?;
    let q = a
        .queue
        .as_ref()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "not_found", "no job queue"))?;
    let uuid = parse_uuid(id, "job")?;
    let done = if retry {
        q.retry_dead(uuid).await
    } else {
        q.discard_dead(uuid).await
    }
    .map_err(|e| job_err(&e))?;
    if !done {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "no failed job with this id",
        ));
    }
    let action = if retry {
        "job.retried"
    } else {
        "job.discarded"
    };
    audit(
        s,
        Some(actor),
        action,
        Some(("job", id.to_string())),
        json!({}),
    )
    .await;
    Ok(())
}

// ---------------------------------------------------------------- overview

fn row(id: &str, status: &str, detail: &str, args: &[(&str, String)]) -> HealthRowDto {
    HealthRowDto {
        id: id.into(),
        status: status.into(),
        detail: detail.into(),
        args: args
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.clone()))
            .collect(),
        checked_at: None,
        checked_local: None,
        stale: false,
    }
}

async fn storage_row(a: &AdminApi) -> HealthRowDto {
    let Some(b) = &a.budget else {
        return row("storage", "off", "off", &[]);
    };
    match b.report().await {
        Ok(r) if r.at_hard_cap => row(
            "storage",
            "error",
            "full",
            &[
                ("used", r.usage.total.to_string()),
                ("cap", r.effective_cap.to_string()),
            ],
        ),
        Ok(r) if r.usage.total > r.high_watermark_bytes => row(
            "storage",
            "warning",
            "high",
            &[
                ("used", r.usage.total.to_string()),
                ("cap", r.effective_cap.to_string()),
            ],
        ),
        Ok(r) => row(
            "storage",
            "ok",
            "used",
            &[
                ("used", r.usage.total.to_string()),
                ("cap", r.effective_cap.to_string()),
            ],
        ),
        Err(_) => row("storage", "error", "unreadable", &[]),
    }
}

async fn jobs_row(a: &AdminApi) -> HealthRowDto {
    let Some(q) = &a.queue else {
        return row("jobs", "off", "off", &[]);
    };
    match q.lane_counts().await {
        Ok(l) => {
            let queued: u64 = l.iter().map(|c| c.queued).sum();
            let dead: u64 = l.iter().map(|c| c.dead).sum();
            let args = [("queued", queued.to_string()), ("failed", dead.to_string())];
            if dead > 0 {
                row("jobs", "warning", "failed", &args)
            } else {
                row("jobs", "ok", "queued", &args)
            }
        }
        Err(_) => row("jobs", "error", "unreadable", &[]),
    }
}

async fn sources_row(s: &ApiState, actor: Actor) -> ApiResult<HealthRowDto> {
    let status = source_status(s, actor).await?;
    let working = status
        .sources
        .iter()
        .filter(|x| x.state == "working")
        .count();
    let total = status
        .sources
        .iter()
        .filter(|x| x.state != "disabled")
        .count();
    let src_args = [
        ("working", working.to_string()),
        ("total", total.to_string()),
    ];
    let sources = if total > 0 && working == 0 {
        row("data_sources", "error", "none_working", &src_args)
    } else if working < total || !status.groups_without_source.is_empty() {
        row("data_sources", "warning", "some_not_working", &src_args)
    } else {
        row("data_sources", "ok", "working", &src_args)
    };
    Ok(sources)
}

async fn secrets_row(s: &ApiState) -> ApiResult<HealthRowDto> {
    let sec = secrets_status(s).await?;
    let secrets = if sec.locked_rows > 0 {
        row(
            "secrets",
            "error",
            "locked",
            &[("count", sec.locked_rows.to_string())],
        )
    } else if sec.rotation_due || sec.rows_to_rewrap > 0 {
        row(
            "secrets",
            "warning",
            if sec.rotation_due {
                "rotation_due"
            } else {
                "rewrap"
            },
            &[
                ("count", sec.rows_to_rewrap.to_string()),
                ("version", sec.active_version.to_string()),
            ],
        )
    } else {
        row(
            "secrets",
            "ok",
            "active",
            &[("version", sec.active_version.to_string())],
        )
    };
    Ok(secrets)
}

/// Admin > Overview: eight health rows as the probe job stored them (never probed in the
/// request, DEC-285), the attention block, the last three admin actions and the brand card.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn overview(s: &ApiState, actor: Actor) -> ApiResult<AdminOverviewDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let zone = s.app.user_zone(actor.user_id).await?;
    let health = probes::stored_rows(s, zone).await?;
    let attention = health
        .iter()
        .filter(|h| h.status == "warning" || h.status == "error")
        .map(|h| h.id.clone())
        .collect();
    let recent = a
        .store
        .audit_query(&AuditFilter {
            limit: 3,
            ..AuditFilter::default()
        })
        .await
        .map_err(pe)?
        .into_iter()
        .map(audit_dto)
        .collect();
    let hosted_banner = a.hosted() && sources::licence_missing(s).await?;
    let key_backup = key_backup_due(a).await?;
    Ok(AdminOverviewDto {
        key_backup,
        health,
        attention,
        recent,
        brand: a.brand.clone(),
        hosting: a.hosting(),
        hosted_banner,
    })
}

/// The read-only brand card.
///
/// # Errors
/// Not an admin.
pub fn brand(s: &ApiState, actor: Actor) -> ApiResult<BrandCardDto> {
    require_admin(actor)?;
    Ok(admin(s)?.brand.clone())
}

/// Admin > Notification defaults: the instance default rules and the event catalogue.
///
/// # Errors
/// Not an admin, notifications disabled.
pub async fn notification_defaults(
    s: &ApiState,
    actor: Actor,
) -> ApiResult<contracts::admin::NotificationDefaultsDto> {
    require_admin(actor)?;
    Ok(contracts::admin::NotificationDefaultsDto {
        defaults: crate::notifications::instance_defaults(s).await?,
        events: crate::notifications::catalogue(actor),
    })
}

#[cfg(test)]
mod tests {
    use super::csv_field;

    /// UX2-9: cells a spreadsheet would run as a formula start with `'` (OWASP CSV
    /// injection); quoting still applies.
    #[test]
    fn csv_cells_never_start_a_formula() {
        assert_eq!(csv_field("root"), "root");
        assert_eq!(csv_field("=HYPERLINK(\"x\")"), "\"'=HYPERLINK(\"\"x\"\")\"");
        for lead in ["+1", "-1", "@SUM(A1)", "\tx", "\rx"] {
            assert!(
                csv_field(lead).trim_start_matches('"').starts_with('\''),
                "{lead}"
            );
        }
        assert_eq!(csv_field("a,b"), "\"a,b\"");
    }
}
