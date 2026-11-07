//! Storage of the admin area (addendum-1 section 3, design.md 4): the database side of the
//! instance settings, the audit log, data-source credentials (encrypted, purpose
//! `connector_credential`), licence records per source and the key versions seen.
//!
//! The rules (provenance, lock-out guards, re-authentication) live in `api::admin`; this is
//! the port and its adapters.

use std::collections::BTreeMap;
use std::sync::Mutex;

use app::ports::{PortError, SecretLocked};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use uuid::Uuid;

/// Purpose of the encrypted data-source credentials.
pub const CREDENTIAL_PURPOSE: &str = "connector_credential";
pub(crate) const CREDENTIAL_FIELD: &str = "secret";

/// Database values of the instance settings; `None` means "not set here".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoredSettings {
    pub hosting: Option<String>,
    pub signup: Option<String>,
    pub session_days: Option<u32>,
    pub secure_cookies: Option<bool>,
    pub storage_max_total_bytes: Option<u64>,
    pub storage_high_watermark: Option<f64>,
    pub storage_low_watermark: Option<f64>,
    pub setup_completed_at: Option<DateTime<Utc>>,
    /// Last completed wizard step (resumable wizard).
    pub setup_step: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub updated_by: Option<UserId>,
}

/// Who did it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    User,
    System,
    Cli,
}

impl ActorKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
            Self::Cli => "cli",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "user" => Self::User,
            "cli" => Self::Cli,
            _ => Self::System,
        }
    }
}

/// One row of the admin audit log. `detail` never holds secret values.
#[derive(Debug, Clone, PartialEq)]
pub struct AuditEntry {
    pub id: Uuid,
    pub at: DateTime<Utc>,
    pub actor_id: Option<UserId>,
    pub actor_kind: ActorKind,
    /// Username at the time (kept when the account is deleted later).
    pub actor_name: Option<String>,
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub detail: serde_json::Value,
    pub client_addr: Option<String>,
    pub request_id: Option<String>,
}

/// Filters of the audit query; newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditFilter {
    pub actor: Option<UserId>,
    /// Exact action, or a prefix ending in `.` (`user.`).
    pub action: Option<String>,
    pub target_type: Option<String>,
    /// Rows strictly older than this (paging).
    pub before: Option<DateTime<Utc>>,
    pub after: Option<DateTime<Utc>>,
    pub limit: u32,
}

impl AuditFilter {
    fn matches(&self, e: &AuditEntry) -> bool {
        self.actor.is_none_or(|a| e.actor_id == Some(a))
            && self.action.as_deref().is_none_or(|a| {
                if a.ends_with('.') {
                    e.action.starts_with(a)
                } else {
                    e.action == a
                }
            })
            && self
                .target_type
                .as_deref()
                .is_none_or(|t| e.target_type.as_deref() == Some(t))
            && self.before.is_none_or(|b| e.at < b)
            && self.after.is_none_or(|a| e.at >= a)
    }
}

/// A stored data-source credential. `fields` is `Err` when the value is locked. `Debug`
/// prints the field names only, never the decrypted values.
#[derive(Clone, PartialEq)]
pub struct StoredCredential {
    pub id: Uuid,
    pub connector: String,
    /// `None`: the instance credential; `Some`: a member's own (design.md 4.2).
    pub user: Option<UserId>,
    pub fields: Result<BTreeMap<String, String>, SecretLocked>,
    pub created_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub last_tested_at: Option<DateTime<Utc>>,
    pub last_result: Option<String>,
}

impl std::fmt::Debug for StoredCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fields = self.fields.as_ref().map(|m| m.keys().collect::<Vec<_>>());
        f.debug_struct("StoredCredential")
            .field("id", &self.id)
            .field("connector", &self.connector)
            .field("user", &self.user)
            .field("fields", &fields)
            .field("created_by", &self.created_by)
            .field("created_at", &self.created_at)
            .field("last_tested_at", &self.last_tested_at)
            .field("last_result", &self.last_result)
            .finish()
    }
}

/// Hosted mode: is an agreement with the provider on file (design.md 4.1)?
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LicenceRecord {
    pub connector: String,
    pub agreement_on_file: bool,
    pub recorded_by: Option<UserId>,
    pub recorded_at: DateTime<Utc>,
}

#[async_trait]
pub trait AdminStore: Send + Sync {
    async fn settings(&self) -> Result<StoredSettings, PortError>;
    async fn put_settings(&self, s: &StoredSettings) -> Result<(), PortError>;

    async fn audit(&self, e: &AuditEntry) -> Result<(), PortError>;
    async fn audit_query(&self, f: &AuditFilter) -> Result<Vec<AuditEntry>, PortError>;
    /// Delete rows older than `before` (retention, OD-13); returns the count.
    async fn audit_purge(&self, before: DateTime<Utc>) -> Result<u64, PortError>;

    async fn credential(
        &self,
        connector: &str,
        user: Option<UserId>,
    ) -> Result<Option<StoredCredential>, PortError>;
    /// Every instance credential.
    async fn instance_credentials(&self) -> Result<Vec<StoredCredential>, PortError>;
    async fn user_credentials(&self, user: UserId) -> Result<Vec<StoredCredential>, PortError>;
    /// Insert or replace (encrypting the fields).
    async fn put_credential(
        &self,
        connector: &str,
        user: Option<UserId>,
        fields: &BTreeMap<String, String>,
        by: Option<UserId>,
        at: DateTime<Utc>,
    ) -> Result<StoredCredential, PortError>;
    async fn delete_credential(
        &self,
        connector: &str,
        user: Option<UserId>,
    ) -> Result<bool, PortError>;
    async fn record_test(&self, id: Uuid, at: DateTime<Utc>, result: &str)
    -> Result<(), PortError>;

    async fn licences(&self) -> Result<Vec<LicenceRecord>, PortError>;
    async fn put_licence(&self, l: &LicenceRecord) -> Result<(), PortError>;

    /// Key versions and when each was first seen (OD-09 reminder).
    async fn key_versions(&self) -> Result<Vec<(u16, DateTime<Utc>)>, PortError>;
    async fn record_key_versions(
        &self,
        versions: &[u16],
        at: DateTime<Utc>,
    ) -> Result<(), PortError>;

    /// Is the database reachable (overview health row)?
    async fn ping(&self) -> Result<(), PortError>;
}

#[derive(Default)]
struct MemState {
    settings: StoredSettings,
    audit: Vec<AuditEntry>,
    creds: Vec<(StoredCredential, BTreeMap<String, String>)>,
    licences: BTreeMap<String, LicenceRecord>,
    keys: BTreeMap<u16, DateTime<Utc>>,
}

/// In-memory admin store (tests). Credentials are kept in plain text here.
#[derive(Default)]
pub struct MemAdminStore(Mutex<MemState>);

impl MemAdminStore {
    fn s(&self) -> std::sync::MutexGuard<'_, MemState> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[async_trait]
impl AdminStore for MemAdminStore {
    async fn settings(&self) -> Result<StoredSettings, PortError> {
        Ok(self.s().settings.clone())
    }

    async fn put_settings(&self, s: &StoredSettings) -> Result<(), PortError> {
        self.s().settings = s.clone();
        Ok(())
    }

    async fn audit(&self, e: &AuditEntry) -> Result<(), PortError> {
        self.s().audit.push(e.clone());
        Ok(())
    }

    async fn audit_query(&self, f: &AuditFilter) -> Result<Vec<AuditEntry>, PortError> {
        let mut v: Vec<AuditEntry> = self
            .s()
            .audit
            .iter()
            .filter(|e| f.matches(e))
            .cloned()
            .collect();
        v.sort_by_key(|e| std::cmp::Reverse((e.at, e.id)));
        v.truncate(f.limit as usize);
        Ok(v)
    }

    async fn audit_purge(&self, before: DateTime<Utc>) -> Result<u64, PortError> {
        let mut s = self.s();
        let n = s.audit.len();
        s.audit.retain(|e| e.at >= before);
        Ok((n - s.audit.len()) as u64)
    }

    async fn credential(
        &self,
        connector: &str,
        user: Option<UserId>,
    ) -> Result<Option<StoredCredential>, PortError> {
        Ok(self
            .s()
            .creds
            .iter()
            .find(|(c, _)| c.connector == connector && c.user == user)
            .map(|(c, _)| c.clone()))
    }

    async fn instance_credentials(&self) -> Result<Vec<StoredCredential>, PortError> {
        Ok(self
            .s()
            .creds
            .iter()
            .filter(|(c, _)| c.user.is_none())
            .map(|(c, _)| c.clone())
            .collect())
    }

    async fn user_credentials(&self, user: UserId) -> Result<Vec<StoredCredential>, PortError> {
        Ok(self
            .s()
            .creds
            .iter()
            .filter(|(c, _)| c.user == Some(user))
            .map(|(c, _)| c.clone())
            .collect())
    }

    async fn put_credential(
        &self,
        connector: &str,
        user: Option<UserId>,
        fields: &BTreeMap<String, String>,
        by: Option<UserId>,
        at: DateTime<Utc>,
    ) -> Result<StoredCredential, PortError> {
        let mut s = self.s();
        s.creds
            .retain(|(c, _)| !(c.connector == connector && c.user == user));
        let c = StoredCredential {
            id: Uuid::now_v7(),
            connector: connector.to_string(),
            user,
            fields: Ok(fields.clone()),
            created_by: by,
            created_at: at,
            last_tested_at: None,
            last_result: None,
        };
        s.creds.push((c.clone(), fields.clone()));
        Ok(c)
    }

    async fn delete_credential(
        &self,
        connector: &str,
        user: Option<UserId>,
    ) -> Result<bool, PortError> {
        let mut s = self.s();
        let n = s.creds.len();
        s.creds
            .retain(|(c, _)| !(c.connector == connector && c.user == user));
        Ok(s.creds.len() < n)
    }

    async fn record_test(
        &self,
        id: Uuid,
        at: DateTime<Utc>,
        result: &str,
    ) -> Result<(), PortError> {
        for (c, _) in &mut self.s().creds {
            if c.id == id {
                c.last_tested_at = Some(at);
                c.last_result = Some(result.to_string());
            }
        }
        Ok(())
    }

    async fn licences(&self) -> Result<Vec<LicenceRecord>, PortError> {
        Ok(self.s().licences.values().cloned().collect())
    }

    async fn put_licence(&self, l: &LicenceRecord) -> Result<(), PortError> {
        self.s().licences.insert(l.connector.clone(), l.clone());
        Ok(())
    }

    async fn key_versions(&self) -> Result<Vec<(u16, DateTime<Utc>)>, PortError> {
        Ok(self.s().keys.iter().map(|(v, t)| (*v, *t)).collect())
    }

    async fn record_key_versions(
        &self,
        versions: &[u16],
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        let mut s = self.s();
        for v in versions {
            s.keys.entry(*v).or_insert(at);
        }
        Ok(())
    }

    async fn ping(&self) -> Result<(), PortError> {
        Ok(())
    }
}
