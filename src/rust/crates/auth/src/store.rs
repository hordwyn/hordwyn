//! Persistence port for credentials, sessions, OIDC identities, recovery codes and disabled
//! accounts.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use uuid::Uuid;
use zeroize::Zeroizing;

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
    /// The provider the session was signed in through (back-channel logout, WP3-3).
    pub provider_id: Option<Uuid>,
    /// The provider session id (`sid`) of that sign-in.
    pub oidc_sid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcIdentity {
    pub id: Uuid,
    pub issuer: String,
    pub subject: String,
    pub user_id: UserId,
    /// The provider row (`None` when it was deleted).
    pub provider_id: Option<Uuid>,
    /// The provider session id of the last sign-in.
    pub sid: Option<String>,
    pub linked_at: DateTime<Utc>,
    /// blake3 of the identifying claims at the last sign-in.
    pub last_claims_hash: Option<String>,
    /// Groups the last sign-in carried; `None`: there was no groups claim.
    pub last_groups: Option<Vec<String>>,
}

impl OidcIdentity {
    /// A new link of (`issuer`, `subject`) to `user`.
    #[must_use]
    pub fn new(issuer: &str, subject: &str, user_id: UserId, at: DateTime<Utc>) -> Self {
        Self {
            id: Uuid::now_v7(),
            issuer: issuer.into(),
            subject: subject.into(),
            user_id,
            provider_id: None,
            sid: None,
            linked_at: at,
            last_claims_hash: None,
            last_groups: None,
        }
    }
}

/// A secret kept with one session (table `session_secrets`, encrypted, purpose
/// [`crate::pg::SESSION_SECRET_PURPOSE`]); it is dropped with the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionSecret {
    /// The ID token of the sign-in, the `id_token_hint` of RP-initiated logout.
    IdToken,
    /// A gated action waiting for an OIDC re-authentication round trip (od-12 4.4 step 2).
    PendingAction,
}

impl SessionSecret {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IdToken => "id_token",
            Self::PendingAction => "pending_action",
        }
    }
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
    /// End the sessions signed in through `provider` with OP session `sid` (back-channel
    /// logout); returns how many.
    async fn delete_sessions_by_sid(&self, provider: Uuid, sid: &str) -> Result<u64, PortError>;
    /// End the sessions of `user` signed in through `provider`; returns how many.
    async fn delete_sessions_via(&self, user: UserId, provider: Uuid) -> Result<u64, PortError>;
    /// Keep (`Some`) or drop (`None`) a secret of the session `token_hash`.
    async fn set_session_secret(
        &self,
        token_hash: &str,
        kind: SessionSecret,
        value: Option<&str>,
    ) -> Result<(), PortError>;
    async fn session_secret(
        &self,
        token_hash: &str,
        kind: SessionSecret,
    ) -> Result<Option<Zeroizing<String>>, PortError>;
    /// The session id changed (re-authentication): its secrets move along.
    async fn move_session_secrets(&self, from: &str, to: &str) -> Result<(), PortError>;
    /// Sessions of `user` (admin users page: last seen).
    async fn sessions_for(&self, user: UserId) -> Result<Vec<Session>, PortError>;
    async fn identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> Result<Option<OidcIdentity>, PortError>;
    /// Link a new identity; `false` when (`issuer`, `subject`) is linked already (to anyone:
    /// a link never moves an identity to another account).
    async fn link_identity(&self, i: &OidcIdentity) -> Result<bool, PortError>;
    /// Record the provider, `sid`, claims hash and groups of a sign-in.
    async fn touch_identity(&self, i: &OidcIdentity) -> Result<(), PortError>;
    /// Remove the identity `id` of `user`; `false` when there is no such link.
    async fn unlink_identity(&self, user: UserId, id: Uuid) -> Result<bool, PortError>;
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
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError>;
    /// A one-time password reset (only the token hash is stored); every other open reset of
    /// the same account is revoked (UX3-12: only the newest link works).
    async fn insert_password_reset(&self, r: &PasswordReset) -> Result<(), PortError>;
    /// The account of the open reset with this hash (not used, not expired), without using
    /// it (so a wrong token costs no password hashing).
    async fn open_password_reset(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<UserId>, PortError>;
    /// Use the open reset with this hash, set `password_hash` and end every session of its
    /// account, all or nothing (UX3-12); `None` when there is no such open reset.
    async fn complete_password_reset(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
        password_hash: &str,
    ) -> Result<Option<UserId>, PortError>;
    /// Delete resets that expired before `now` (daily retention job); returns the count.
    async fn purge_password_resets(&self, now: DateTime<Utc>) -> Result<u64, PortError>;
}

/// A one-time password reset link (`cli admin reset-password`, addendum-1 3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordReset {
    pub id: Uuid,
    pub user: UserId,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Default)]
struct State {
    creds: HashMap<UserId, Credentials>,
    sessions: HashMap<String, Session>,
    identities: HashMap<(String, String), OidcIdentity>,
    codes: HashMap<UserId, Vec<RecoveryCode>>,
    disabled: HashSet<UserId>,
    secrets: HashMap<(String, SessionSecret), String>,
    /// (reset, used)
    resets: Vec<(PasswordReset, bool)>,
}

#[derive(Default)]
pub struct MemAuthStore {
    state: Mutex<State>,
}

impl MemAuthStore {
    fn s(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().expect("auth store")
    }

    fn remove_sessions(&self, f: impl Fn(&Session) -> bool) -> u64 {
        let mut s = self.s();
        let gone: Vec<String> = s
            .sessions
            .values()
            .filter(|x| f(x))
            .map(|x| x.token_hash.clone())
            .collect();
        for h in &gone {
            s.sessions.remove(h);
        }
        s.secrets.retain(|(h, _), _| !gone.contains(h));
        gone.len() as u64
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
        let mut s = self.s();
        s.sessions.remove(token_hash);
        s.secrets.retain(|(h, _), _| h != token_hash);
        Ok(())
    }

    async fn delete_sessions_for(&self, user: UserId) -> Result<(), PortError> {
        self.remove_sessions(|s| s.user_id == user);
        Ok(())
    }

    async fn delete_sessions_by_sid(&self, provider: Uuid, sid: &str) -> Result<u64, PortError> {
        Ok(self.remove_sessions(|s| {
            s.provider_id == Some(provider) && s.oidc_sid.as_deref() == Some(sid)
        }))
    }

    async fn delete_sessions_via(&self, user: UserId, provider: Uuid) -> Result<u64, PortError> {
        Ok(self.remove_sessions(|s| s.user_id == user && s.provider_id == Some(provider)))
    }

    async fn set_session_secret(
        &self,
        token_hash: &str,
        kind: SessionSecret,
        value: Option<&str>,
    ) -> Result<(), PortError> {
        let mut s = self.s();
        let key = (token_hash.to_string(), kind);
        match value {
            Some(v) if s.sessions.contains_key(token_hash) => {
                s.secrets.insert(key, v.to_string());
            }
            Some(_) => {}
            None => {
                s.secrets.remove(&key);
            }
        }
        Ok(())
    }

    async fn session_secret(
        &self,
        token_hash: &str,
        kind: SessionSecret,
    ) -> Result<Option<Zeroizing<String>>, PortError> {
        Ok(self
            .s()
            .secrets
            .get(&(token_hash.to_string(), kind))
            .map(|v| Zeroizing::new(v.clone())))
    }

    async fn move_session_secrets(&self, from: &str, to: &str) -> Result<(), PortError> {
        let mut s = self.s();
        let moved: Vec<_> = s
            .secrets
            .iter()
            .filter(|((h, _), _)| h == from)
            .map(|((_, k), v)| (*k, v.clone()))
            .collect();
        s.secrets.retain(|(h, _), _| h != from);
        for (k, v) in moved {
            s.secrets.insert((to.to_string(), k), v);
        }
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

    async fn link_identity(&self, i: &OidcIdentity) -> Result<bool, PortError> {
        let mut s = self.s();
        let key = (i.issuer.clone(), i.subject.clone());
        if s.identities.contains_key(&key) {
            return Ok(false);
        }
        s.identities.insert(key, i.clone());
        Ok(true)
    }

    async fn touch_identity(&self, i: &OidcIdentity) -> Result<(), PortError> {
        if let Some(x) = self
            .s()
            .identities
            .get_mut(&(i.issuer.clone(), i.subject.clone()))
        {
            x.provider_id = i.provider_id;
            x.sid.clone_from(&i.sid);
            x.last_claims_hash.clone_from(&i.last_claims_hash);
            x.last_groups.clone_from(&i.last_groups);
        }
        Ok(())
    }

    async fn unlink_identity(&self, user: UserId, id: Uuid) -> Result<bool, PortError> {
        let mut s = self.s();
        let before = s.identities.len();
        s.identities
            .retain(|_, i| !(i.id == id && i.user_id == user));
        Ok(s.identities.len() < before)
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
        _audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError> {
        let mut s = self.s();
        if at.is_some() {
            s.disabled.insert(user);
        } else {
            s.disabled.remove(&user);
        }
        Ok(())
    }

    async fn insert_password_reset(&self, r: &PasswordReset) -> Result<(), PortError> {
        let mut s = self.s();
        s.resets.retain(|(o, used)| *used || o.user != r.user);
        s.resets.push((r.clone(), false));
        Ok(())
    }

    async fn open_password_reset(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<UserId>, PortError> {
        Ok(self
            .s()
            .resets
            .iter()
            .find(|(r, used)| !*used && r.token_hash == token_hash && r.expires_at > now)
            .map(|(r, _)| r.user))
    }

    async fn complete_password_reset(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
        password_hash: &str,
    ) -> Result<Option<UserId>, PortError> {
        let user = {
            let mut s = self.s();
            let Some(user) = s
                .resets
                .iter_mut()
                .find(|(r, used)| !*used && r.token_hash == token_hash && r.expires_at > now)
                .map(|(r, used)| {
                    *used = true;
                    r.user
                })
            else {
                return Ok(None);
            };
            s.creds
                .entry(user)
                .or_insert_with(|| Credentials::empty(user))
                .password_hash = Some(password_hash.to_string());
            user
        };
        self.remove_sessions(|x| x.user_id == user);
        Ok(Some(user))
    }

    async fn purge_password_resets(&self, now: DateTime<Utc>) -> Result<u64, PortError> {
        let mut s = self.s();
        let before = s.resets.len();
        s.resets.retain(|(r, _)| r.expires_at > now);
        Ok((before - s.resets.len()) as u64)
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
    async fn insert_signup_invite(
        &self,
        i: &SignupInvite,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError>;
    async fn signup_invite_by_hash(&self, hash: &str) -> Result<Option<SignupInvite>, PortError>;
    /// Every invite, newest first.
    async fn signup_invites(&self) -> Result<Vec<SignupInvite>, PortError>;
    async fn update_signup_invite(
        &self,
        i: &SignupInvite,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError>;
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
    async fn insert_signup_invite(
        &self,
        i: &SignupInvite,
        _audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError> {
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

    async fn update_signup_invite(
        &self,
        i: &SignupInvite,
        _audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError> {
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
