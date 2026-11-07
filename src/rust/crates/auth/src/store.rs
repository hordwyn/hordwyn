//! Persistence port for credentials, sessions, OIDC identities, recovery codes and disabled
//! accounts.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub user_id: UserId,
    /// PHC string (`$argon2id$...`); `None` for OIDC-only accounts.
    pub password_hash: Option<String>,
    /// Plain seed in memory only; stored encrypted (purpose `totp_secret`).
    pub totp_secret: Option<Vec<u8>>,
    pub totp_enabled: bool,
    /// Last accepted TOTP time step (replay protection).
    pub totp_last_step: Option<i64>,
    /// The stored seed exists but cannot be decrypted (addendum-1 2.2 `locked`).
    pub totp_locked: bool,
}

impl Credentials {
    /// No password, no authenticator.
    #[must_use]
    pub fn empty(user_id: UserId) -> Self {
        Self {
            user_id,
            password_hash: None,
            totp_secret: None,
            totp_enabled: false,
            totp_last_step: None,
            totp_locked: false,
        }
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("user_id", &self.user_id)
            .field("totp_enabled", &self.totp_enabled)
            .field("totp_locked", &self.totp_locked)
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
    /// Last fresh proof (sign-in or re-authentication, DEC-191).
    pub reauth_at: Option<DateTime<Utc>>,
    /// `password`, `password+totp`, `oidc:<provider>`.
    pub reauth_method: Option<String>,
    /// Last gated action within the window (the window slides from here).
    pub reauth_slide_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcIdentity {
    pub issuer: String,
    pub subject: String,
    pub user_id: UserId,
}

/// One TOTP recovery code (argon2id hash only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryCode {
    pub id: Uuid,
    pub code_hash: String,
    pub used: bool,
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
    /// Sessions of `user` (admin users page: last seen).
    async fn sessions_for(&self, user: UserId) -> Result<Vec<Session>, PortError>;
    async fn identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> Result<Option<OidcIdentity>, PortError>;
    async fn link_identity(&self, i: &OidcIdentity) -> Result<(), PortError>;
    /// Identities linked to `user`.
    async fn identities_of(&self, user: UserId) -> Result<Vec<OidcIdentity>, PortError>;
    /// Replace every recovery code of `user`.
    async fn put_recovery_codes(
        &self,
        user: UserId,
        codes: &[RecoveryCode],
        at: DateTime<Utc>,
    ) -> Result<(), PortError>;
    async fn recovery_codes(&self, user: UserId) -> Result<Vec<RecoveryCode>, PortError>;
    /// Mark a code used; `false` if it was used already (a lost race).
    async fn use_recovery_code(&self, id: Uuid, at: DateTime<Utc>) -> Result<bool, PortError>;
    async fn is_disabled(&self, user: UserId) -> Result<bool, PortError>;
    async fn set_disabled(
        &self,
        user: UserId,
        by: Option<UserId>,
        at: Option<DateTime<Utc>>,
    ) -> Result<(), PortError>;
}

#[derive(Default)]
struct State {
    creds: HashMap<UserId, Credentials>,
    sessions: HashMap<String, Session>,
    identities: HashMap<(String, String), OidcIdentity>,
    codes: HashMap<UserId, Vec<RecoveryCode>>,
    disabled: HashSet<UserId>,
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

    async fn sessions_for(&self, user: UserId) -> Result<Vec<Session>, PortError> {
        Ok(self
            .s()
            .sessions
            .values()
            .filter(|s| s.user_id == user)
            .cloned()
            .collect())
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

    async fn identities_of(&self, user: UserId) -> Result<Vec<OidcIdentity>, PortError> {
        Ok(self
            .s()
            .identities
            .values()
            .filter(|i| i.user_id == user)
            .cloned()
            .collect())
    }

    async fn put_recovery_codes(
        &self,
        user: UserId,
        codes: &[RecoveryCode],
        _at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        self.s().codes.insert(user, codes.to_vec());
        Ok(())
    }

    async fn recovery_codes(&self, user: UserId) -> Result<Vec<RecoveryCode>, PortError> {
        Ok(self.s().codes.get(&user).cloned().unwrap_or_default())
    }

    async fn use_recovery_code(&self, id: Uuid, _at: DateTime<Utc>) -> Result<bool, PortError> {
        let mut s = self.s();
        for c in s.codes.values_mut().flatten() {
            if c.id == id {
                let fresh = !c.used;
                c.used = true;
                return Ok(fresh);
            }
        }
        Ok(false)
    }

    async fn is_disabled(&self, user: UserId) -> Result<bool, PortError> {
        Ok(self.s().disabled.contains(&user))
    }

    async fn set_disabled(
        &self,
        user: UserId,
        _by: Option<UserId>,
        at: Option<DateTime<Utc>>,
    ) -> Result<(), PortError> {
        let mut s = self.s();
        if at.is_some() {
            s.disabled.insert(user);
        } else {
            s.disabled.remove(&user);
        }
        Ok(())
    }
}

/// An admin invite to create an account (not tied to a companion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignupInvite {
    pub id: Uuid,
    /// blake3 of the token; the token is shown once.
    pub token_hash: String,
    pub email: Option<String>,
    pub note: Option<String>,
    pub created_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used_by: Option<UserId>,
    pub used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Admin invites (separate port so the account store stays small).
#[async_trait]
pub trait InviteStore: Send + Sync {
    async fn insert_signup_invite(&self, i: &SignupInvite) -> Result<(), PortError>;
    async fn signup_invite_by_hash(&self, hash: &str) -> Result<Option<SignupInvite>, PortError>;
    /// Every invite, newest first.
    async fn signup_invites(&self) -> Result<Vec<SignupInvite>, PortError>;
    async fn update_signup_invite(&self, i: &SignupInvite) -> Result<(), PortError>;
    /// Mark the invite used at `now` in one conditional update (not used, not revoked, not
    /// expired); `false` when another sign-up got it first or it is no longer open.
    async fn claim_signup_invite(&self, id: Uuid, now: DateTime<Utc>) -> Result<bool, PortError>;
    /// After a claim: who used it, or `None` to give it back (the account was not created).
    async fn finish_signup_invite(&self, id: Uuid, user: Option<UserId>) -> Result<(), PortError>;
}

#[derive(Default)]
pub struct MemInviteStore(Mutex<Vec<SignupInvite>>);

#[async_trait]
impl InviteStore for MemInviteStore {
    async fn insert_signup_invite(&self, i: &SignupInvite) -> Result<(), PortError> {
        self.0.lock().expect("invites").push(i.clone());
        Ok(())
    }

    async fn signup_invite_by_hash(&self, hash: &str) -> Result<Option<SignupInvite>, PortError> {
        Ok(self
            .0
            .lock()
            .expect("invites")
            .iter()
            .find(|i| i.token_hash == hash)
            .cloned())
    }

    async fn signup_invites(&self) -> Result<Vec<SignupInvite>, PortError> {
        let mut v = self.0.lock().expect("invites").clone();
        v.sort_by_key(|i| std::cmp::Reverse(i.created_at));
        Ok(v)
    }

    async fn update_signup_invite(&self, i: &SignupInvite) -> Result<(), PortError> {
        for x in self.0.lock().expect("invites").iter_mut() {
            if x.id == i.id {
                *x = i.clone();
            }
        }
        Ok(())
    }

    async fn claim_signup_invite(&self, id: Uuid, now: DateTime<Utc>) -> Result<bool, PortError> {
        let mut all = self.0.lock().expect("invites");
        let Some(i) = all.iter_mut().find(|i| {
            i.id == id && i.used_at.is_none() && i.revoked_at.is_none() && i.expires_at > now
        }) else {
            return Ok(false);
        };
        i.used_at = Some(now);
        Ok(true)
    }

    async fn finish_signup_invite(&self, id: Uuid, user: Option<UserId>) -> Result<(), PortError> {
        for i in self.0.lock().expect("invites").iter_mut() {
            if i.id == id && i.used_by.is_none() {
                if user.is_none() {
                    i.used_at = None;
                }
                i.used_by = user;
            }
        }
        Ok(())
    }
}
