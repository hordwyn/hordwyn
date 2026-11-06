//! Persistence port for credentials, sessions and OIDC identities.

use std::collections::HashMap;
use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;

#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub user_id: UserId,
    /// PHC string (`$argon2id$...`); `None` for OIDC-only accounts.
    pub password_hash: Option<String>,
    pub totp_secret: Option<Vec<u8>>,
    pub totp_enabled: bool,
    /// Last accepted TOTP time step (replay protection).
    pub totp_last_step: Option<i64>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("user_id", &self.user_id)
            .field("totp_enabled", &self.totp_enabled)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// blake3 of the token; the token itself is never stored.
    pub token_hash: String,
    pub user_id: UserId,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcIdentity {
    pub issuer: String,
    pub subject: String,
    pub user_id: UserId,
}

#[async_trait]
pub trait AuthStore: Send + Sync {
    async fn credentials(&self, user: UserId) -> Result<Option<Credentials>, PortError>;
    async fn put_credentials(&self, c: &Credentials) -> Result<(), PortError>;
    async fn insert_session(&self, s: &Session) -> Result<(), PortError>;
    async fn session(&self, token_hash: &str) -> Result<Option<Session>, PortError>;
    async fn update_session(&self, s: &Session) -> Result<(), PortError>;
    async fn delete_session(&self, token_hash: &str) -> Result<(), PortError>;
    async fn delete_sessions_for(&self, user: UserId) -> Result<(), PortError>;
    async fn identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> Result<Option<OidcIdentity>, PortError>;
    async fn link_identity(&self, i: &OidcIdentity) -> Result<(), PortError>;
}

#[derive(Default)]
struct State {
    creds: HashMap<UserId, Credentials>,
    sessions: HashMap<String, Session>,
    identities: HashMap<(String, String), OidcIdentity>,
}

#[derive(Default)]
pub struct MemAuthStore {
    state: Mutex<State>,
}

impl MemAuthStore {
    fn s(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("auth store")
    }
}

#[async_trait]
impl AuthStore for MemAuthStore {
    async fn credentials(&self, user: UserId) -> Result<Option<Credentials>, PortError> {
        Ok(self.s().creds.get(&user).cloned())
    }

    async fn put_credentials(&self, c: &Credentials) -> Result<(), PortError> {
        self.s().creds.insert(c.user_id, c.clone());
        Ok(())
    }

    async fn insert_session(&self, s: &Session) -> Result<(), PortError> {
        self.s().sessions.insert(s.token_hash.clone(), s.clone());
        Ok(())
    }

    async fn session(&self, token_hash: &str) -> Result<Option<Session>, PortError> {
        Ok(self.s().sessions.get(token_hash).cloned())
    }

    async fn update_session(&self, s: &Session) -> Result<(), PortError> {
        self.insert_session(s).await
    }

    async fn delete_session(&self, token_hash: &str) -> Result<(), PortError> {
        self.s().sessions.remove(token_hash);
        Ok(())
    }

    async fn delete_sessions_for(&self, user: UserId) -> Result<(), PortError> {
        self.s().sessions.retain(|_, s| s.user_id != user);
        Ok(())
    }

    async fn identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> Result<Option<OidcIdentity>, PortError> {
        Ok(self
            .s()
            .identities
            .get(&(issuer.to_string(), subject.to_string()))
            .cloned())
    }

    async fn link_identity(&self, i: &OidcIdentity) -> Result<(), PortError> {
        self.s()
            .identities
            .insert((i.issuer.clone(), i.subject.clone()), i.clone());
        Ok(())
    }
}
