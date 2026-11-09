//! OIDC providers as configured by the admin (table `oidc_providers`, addendum-1
//! 4.2 "Data model") or by the environment (`auth.oidc`, a read-only row with provenance
//! `env`), and their persistence port.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::admin::AuditEntry;

/// Id of the environment provider's row (one at most).
pub const ENV_PROVIDER_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_7000_8000_0000_0000_0e17);
/// Purpose of the encrypted client secret (addendum-1 2.2).
pub const SECRET_PURPOSE: &str = "oidc_client_secret";
/// Scopes when none are configured.
pub const DEFAULT_SCOPES: [&str; 3] = ["openid", "profile", "email"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleSource {
    /// Roles are set by hand in the admin area.
    #[default]
    Local,
    /// Recomputed from `admin_groups` at every sign-in (OD-16).
    Provider,
}

impl RoleSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Provider => "provider",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "local" => Some(Self::Local),
            "provider" => Some(Self::Provider),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Provenance {
    #[default]
    Db,
    Env,
}

impl Provenance {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Db => "db",
            Self::Env => "env",
        }
    }
}

/// The stored client secret.
#[derive(Clone, Default, PartialEq, Eq)]
pub enum ClientSecret {
    #[default]
    None,
    Set(Zeroizing<String>),
    /// Stored, but its key version is not loaded (addendum-1 2.2 `locked`).
    Locked,
}

impl std::fmt::Debug for ClientSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::None => "None",
            Self::Set(_) => "Set(***)",
            Self::Locked => "Locked",
        })
    }
}

/// What a discovery document advertises (Discovery 1.0 section 3; RP-Initiated Logout 2.1;
/// Back-Channel Logout 2.1; Front-Channel Logout 3; RFC 9207 3).
// The discovery members as they are spelled (one flag each).
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Capabilities {
    pub end_session_endpoint: Option<String>,
    pub backchannel_logout_supported: bool,
    pub backchannel_logout_session_supported: bool,
    pub frontchannel_logout_supported: bool,
    pub code_challenge_methods_supported: Vec<String>,
    pub authorization_response_iss_parameter_supported: bool,
    pub userinfo_endpoint: Option<String>,
}

impl Capabilities {
    /// From a discovery document (unknown or missing members are defaults).
    #[must_use]
    pub fn from_discovery(doc: &Value) -> Self {
        serde_json::from_value(doc.clone()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Provider {
    pub id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: ClientSecret,
    pub scopes: Vec<String>,
    pub enabled: bool,
    pub is_default: bool,
    pub auto_create: bool,
    pub groups_claim: String,
    pub allowed_groups: Vec<String>,
    pub admin_groups: Vec<String>,
    pub role_source: RoleSource,
    pub provenance: Provenance,
    pub discovery: Option<Value>,
    pub discovery_fetched_at: Option<DateTime<Utc>>,
    pub capabilities: Capabilities,
    pub last_test: Option<Value>,
    pub last_test_at: Option<DateTime<Utc>>,
    pub last_callback_at: Option<DateTime<Utc>>,
    /// `ok` or `failed: <code>`.
    pub last_callback_result: Option<String>,
    pub created_by: Option<UserId>,
    pub updated_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Provider {
    /// A new enabled provider with the defaults of addendum-1 4.2.
    #[must_use]
    pub fn new(id: Uuid, slug: &str, issuer: &str, client_id: &str, at: DateTime<Utc>) -> Self {
        Self {
            id,
            slug: slug.into(),
            display_name: slug.into(),
            issuer: issuer.into(),
            client_id: client_id.into(),
            client_secret: ClientSecret::None,
            scopes: DEFAULT_SCOPES.iter().map(|s| (*s).to_string()).collect(),
            enabled: true,
            is_default: false,
            auto_create: false,
            groups_claim: "groups".into(),
            allowed_groups: Vec::new(),
            admin_groups: Vec::new(),
            role_source: RoleSource::Local,
            provenance: Provenance::Db,
            discovery: None,
            discovery_fetched_at: None,
            capabilities: Capabilities::default(),
            last_test: None,
            last_test_at: None,
            last_callback_at: None,
            last_callback_result: None,
            created_by: None,
            updated_by: None,
            created_at: at,
            updated_at: at,
        }
    }

    /// Do the rules need the groups claim?
    #[must_use]
    pub fn needs_groups(&self) -> bool {
        !self.allowed_groups.is_empty()
            || self.role_source == RoleSource::Provider
            || self.scopes.iter().any(|s| s == "groups")
    }
}

/// What a save does with the client secret.
#[derive(Clone, Copy)]
pub enum SecretChange<'a> {
    Keep,
    Set(&'a str),
    Clear,
}

/// Persistence of providers, their status fields and the single-use list.
#[async_trait]
pub trait ProviderStore: Send + Sync {
    /// Every provider, ordered by display name.
    async fn providers(&self) -> Result<Vec<Provider>, PortError>;
    async fn provider(&self, id: Uuid) -> Result<Option<Provider>, PortError> {
        Ok(self.providers().await?.into_iter().find(|p| p.id == id))
    }
    /// Insert or update the configuration fields (never the status fields) and the secret,
    /// with the audit rows in the same transaction (addendum-1 3.4). A default provider
    /// makes every other one non-default. `false` when the slug is taken.
    async fn save(
        &self,
        p: &Provider,
        secret: SecretChange<'_>,
        audit: &[AuditEntry],
    ) -> Result<bool, PortError>;
    /// `false` when there was no such provider.
    async fn delete(&self, id: Uuid, audit: &[AuditEntry]) -> Result<bool, PortError>;
    async fn record_discovery(
        &self,
        id: Uuid,
        doc: &Value,
        caps: &Capabilities,
        at: DateTime<Utc>,
    ) -> Result<(), PortError>;
    async fn record_test(
        &self,
        id: Uuid,
        result: &Value,
        at: DateTime<Utc>,
    ) -> Result<(), PortError>;
    async fn record_callback(
        &self,
        id: Uuid,
        result: &str,
        at: DateTime<Utc>,
    ) -> Result<(), PortError>;
    /// Mark `key` used until `expires_at`; `false` when it was used already (RFC 9700
    /// 4.2.4). Expired entries are dropped on the way.
    async fn claim_once(
        &self,
        key: &str,
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<bool, PortError>;
    /// Back-Channel Logout 2.6 step 7: accept a logout token's `key` (provider and `jti`)
    /// once until `expires_at`, across processes; `false` when it was seen. Expired entries
    /// are dropped on the way.
    async fn claim_jti(
        &self,
        key: &str,
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<bool, PortError>;
    /// Forget `key` again (ending the sessions failed, so the provider may retry).
    async fn release_jti(&self, key: &str) -> Result<(), PortError>;
}

/// In-memory [`ProviderStore`] (tests).
#[derive(Default)]
pub struct MemProviderStore {
    providers: Mutex<HashMap<Uuid, Provider>>,
    used: Mutex<HashSet<String>>,
}

#[async_trait]
impl ProviderStore for MemProviderStore {
    async fn providers(&self) -> Result<Vec<Provider>, PortError> {
        let mut v: Vec<Provider> = self
            .providers
            .lock()
            .expect("providers")
            .values()
            .cloned()
            .collect();
        v.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        Ok(v)
    }

    async fn save(
        &self,
        p: &Provider,
        secret: SecretChange<'_>,
        _audit: &[AuditEntry],
    ) -> Result<bool, PortError> {
        let mut all = self.providers.lock().expect("providers");
        if all.values().any(|o| o.slug == p.slug && o.id != p.id) {
            return Ok(false);
        }
        let mut next = p.clone();
        if let Some(old) = all.get(&p.id) {
            next.client_secret = old.client_secret.clone();
        }
        match secret {
            SecretChange::Keep => {}
            SecretChange::Set(s) => {
                next.client_secret = ClientSecret::Set(Zeroizing::new(s.into()));
            }
            SecretChange::Clear => next.client_secret = ClientSecret::None,
        }
        if next.is_default {
            for o in all.values_mut() {
                o.is_default = false;
            }
        }
        all.insert(p.id, next);
        Ok(true)
    }

    async fn delete(&self, id: Uuid, _audit: &[AuditEntry]) -> Result<bool, PortError> {
        Ok(self
            .providers
            .lock()
            .expect("providers")
            .remove(&id)
            .is_some())
    }

    async fn record_discovery(
        &self,
        id: Uuid,
        doc: &Value,
        caps: &Capabilities,
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        if let Some(p) = self.providers.lock().expect("providers").get_mut(&id) {
            p.discovery = Some(doc.clone());
            p.capabilities = caps.clone();
            p.discovery_fetched_at = Some(at);
        }
        Ok(())
    }

    async fn record_test(
        &self,
        id: Uuid,
        result: &Value,
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        if let Some(p) = self.providers.lock().expect("providers").get_mut(&id) {
            p.last_test = Some(result.clone());
            p.last_test_at = Some(at);
        }
        Ok(())
    }

    async fn record_callback(
        &self,
        id: Uuid,
        result: &str,
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        if let Some(p) = self.providers.lock().expect("providers").get_mut(&id) {
            p.last_callback_result = Some(result.into());
            p.last_callback_at = Some(at);
        }
        Ok(())
    }

    async fn claim_once(
        &self,
        key: &str,
        _expires_at: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<bool, PortError> {
        Ok(self.used.lock().expect("used").insert(key.to_string()))
    }

    async fn claim_jti(
        &self,
        key: &str,
        _expires_at: DateTime<Utc>,
        _now: DateTime<Utc>,
    ) -> Result<bool, PortError> {
        Ok(self.used.lock().expect("used").insert(format!("jti|{key}")))
    }

    async fn release_jti(&self, key: &str) -> Result<(), PortError> {
        self.used
            .lock()
            .expect("used")
            .remove(&format!("jti|{key}"));
        Ok(())
    }
}
