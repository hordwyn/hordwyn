//! Fresh proof for sign-in related changes (DEC-191, docs/research/od-12-reauth-window.md),
//! TOTP recovery codes (OD-08), admin sign-up invites and disabled accounts.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use domain::ids::UserId;
use domain::user::Actor;
use uuid::Uuid;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::store::{InviteStore, RecoveryCode, Session, SessionSecret, SignupInvite};
use crate::{Auth, AuthError, SessionToken, hash_password, token_hash, verify_password};

/// The window slides by this much from the last proof or gated action.
pub const REAUTH_SLIDE_MINUTES: i64 = 15;
/// ...but never further than this from the proof.
pub const REAUTH_CAP_MINUTES: i64 = 60;
/// Largest age of `auth_time` an OIDC round trip may report (Keycloak's default, DEC-191).
pub const OIDC_AUTH_TIME_MAX_AGE_SECS: i64 = 300;
/// Recovery codes per enrolment.
pub const RECOVERY_CODES: usize = 8;
/// Days an admin invite stays valid (like companion invites).
pub const SIGNUP_INVITE_DAYS: i64 = 14;
/// How long a gated action waits for its OIDC re-authentication (like a pending sign-in).
pub const PENDING_ACTION_MINUTES: i64 = 10;
/// Largest pending action kept (JSON).
pub const PENDING_ACTION_MAX_BYTES: usize = 16 * 1024;

/// What the re-authentication dialog needs to know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReauthStatus {
    pub fresh: bool,
    /// When the current window closes (if fresh).
    pub until: Option<DateTime<Utc>>,
    pub method: Option<String>,
    pub has_password: bool,
    pub has_totp: bool,
}

/// Crockford base32 (no i, l, o, u): 32 symbols, 5 bits each.
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Is `code` shaped like a recovery code (10 symbols, optionally `xxxxx-xxxxx`) rather
/// than a 6-digit TOTP code?
#[must_use]
pub fn looks_like_recovery_code(code: &str) -> bool {
    normalize_code(code).is_some()
}

fn normalize_code(code: &str) -> Option<String> {
    let c: String = code
        .chars()
        .filter(|c| !matches!(c, '-' | ' '))
        .map(|c| c.to_ascii_lowercase())
        .collect();
    (c.len() == 10 && c.bytes().all(|b| ALPHABET.contains(&b))).then_some(c)
}

/// 10 random symbols (50 bits), shown as `xxxxx-xxxxx`.
fn new_code() -> String {
    let bytes = crate::random_bytes::<10>();
    let s: String = bytes
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b & 31)]))
        .collect();
    format!("{}-{}", &s[..5], &s[5..])
}

impl Auth {
    /// Use another invite store (Postgres in production).
    #[must_use]
    pub fn with_invites(mut self, invites: Arc<dyn InviteStore>) -> Self {
        self.invites = invites;
        self
    }

    /// When the fresh-proof window of `s` closes (`None`: no proof on record).
    fn window_end(s: &Session) -> Option<DateTime<Utc>> {
        let proof = s.reauth_at?;
        let last = s.reauth_slide_at.map_or(proof, |t| t.max(proof));
        Some(
            (last + Duration::minutes(REAUTH_SLIDE_MINUTES))
                .min(proof + Duration::minutes(REAUTH_CAP_MINUTES)),
        )
    }

    async fn live_session(&self, token: &str) -> Result<Session, AuthError> {
        let now = self.now();
        self.store
            .session(&token_hash(token))
            .await?
            .filter(|s| s.expires_at > now)
            .ok_or(AuthError::ReauthRequired)
    }

    /// The acting user if the session holds a fresh proof; slides the window.
    ///
    /// # Errors
    /// [`AuthError::ReauthRequired`] (also for an unknown session) or storage errors.
    pub async fn require_fresh(&self, token: &str) -> Result<Actor, AuthError> {
        let mut s = self.live_session(token).await?;
        let now = self.now();
        if Self::window_end(&s).is_none_or(|end| now >= end) {
            return Err(AuthError::ReauthRequired);
        }
        let user = self
            .app
            .user_opt(s.user_id)
            .await?
            .ok_or(AuthError::ReauthRequired)?;
        s.reauth_slide_at = Some(now);
        self.store.update_session(&s).await?;
        Ok(Actor {
            user_id: user.id,
            role: user.role,
        })
    }

    /// # Errors
    /// [`AuthError::ReauthRequired`] for an unknown session, storage errors.
    pub async fn reauth_status(&self, token: &str) -> Result<ReauthStatus, AuthError> {
        let s = self.live_session(token).await?;
        let creds = self.credentials_or_default(s.user_id).await?;
        let until = Self::window_end(&s).filter(|end| self.now() < *end);
        Ok(ReauthStatus {
            fresh: until.is_some(),
            until,
            method: s.reauth_method.clone(),
            has_password: creds.password_hash.is_some(),
            has_totp: creds.totp_enabled,
        })
    }

    /// Replace the session by a new id with a fresh proof made with `method`.
    async fn regenerate(&self, old: &Session, method: &str) -> Result<SessionToken, AuthError> {
        let token = URL_SAFE_NO_PAD.encode(crate::random_bytes::<32>());
        let now = self.now();
        self.store
            .insert_session(&Session {
                token_hash: token_hash(&token),
                last_seen_at: now,
                reauth_at: Some(now),
                reauth_method: Some(method.to_string()),
                reauth_slide_at: None,
                ..old.clone()
            })
            .await?;
        let new_hash = token_hash(&token);
        self.store
            .move_session_secrets(&old.token_hash, &new_hash)
            .await?;
        self.store.delete_session(&old.token_hash).await?;
        Ok(SessionToken(token))
    }

    /// Prove it is you again: password, then TOTP (or a recovery code) when enrolled. The
    /// session id is regenerated; the old token stops working. Throttled like sign-in.
    ///
    /// # Errors
    /// The sign-in errors, [`AuthError::ReauthRequired`] for an unknown session.
    pub async fn reauth(
        &self,
        token: &str,
        password: &str,
        totp: Option<&str>,
    ) -> Result<SessionToken, AuthError> {
        let s = self.live_session(token).await?;
        let user = self.app.user(s.user_id).await?;
        self.throttle.check(&user.username, self.now())?;
        let mut creds = self.credentials_or_default(user.id).await?;
        let hash = creds
            .password_hash
            .clone()
            .unwrap_or_else(|| crate::DUMMY_HASH.to_string());
        let ok = verify_password(password.to_string(), hash).await && creds.password_hash.is_some();
        let result = if ok {
            self.second_factor(&mut creds, totp, &user.username).await
        } else {
            Err(AuthError::InvalidCredentials)
        };
        match result {
            Ok(method) => {
                self.throttle.succeeded(&user.username);
                self.regenerate(&s, method).await
            }
            Err(e) => {
                if matches!(e, AuthError::InvalidCredentials | AuthError::InvalidTotp) {
                    self.throttle.failed(&user.username, self.now());
                }
                Err(e)
            }
        }
    }

    /// Hook for the OIDC round trip with `max_age=0` (phase 3): accepts the proof when the
    /// identity (`iss`, `sub`) belongs to the session's account and `auth_time` is at most
    /// [`OIDC_AUTH_TIME_MAX_AGE_SECS`] old; a provider that ignores `max_age` is refused,
    /// never passed through (DEC-191).
    ///
    /// # Errors
    /// [`AuthError::ReauthRequired`] when the identity differs or `auth_time` is too old.
    pub async fn confirm_oidc_reauth(
        &self,
        token: &str,
        issuer: &str,
        subject: &str,
        provider: &str,
        auth_time: DateTime<Utc>,
    ) -> Result<SessionToken, AuthError> {
        let s = self.live_session(token).await?;
        let same = self
            .store
            .identity(issuer, subject)
            .await?
            .is_some_and(|i| i.user_id == s.user_id);
        if !same || auth_time < self.now() - Duration::seconds(OIDC_AUTH_TIME_MAX_AGE_SECS) {
            return Err(AuthError::ReauthRequired);
        }
        self.regenerate(&s, &format!("oidc:{provider}")).await
    }

    /// Keep a gated action on the server while the browser does the OIDC round trip
    /// (od-12 4.4 step 2, D12 "never ask twice"); encrypted with the session, replaced by a
    /// newer one, handed back once by [`Auth::take_pending_action`] within
    /// [`PENDING_ACTION_MINUTES`].
    ///
    /// # Errors
    /// [`AuthError::ReauthRequired`] for an unknown session, [`AuthError::Store`] for an
    /// action above [`PENDING_ACTION_MAX_BYTES`].
    pub async fn keep_pending_action(
        &self,
        token: &str,
        action: &serde_json::Value,
    ) -> Result<(), AuthError> {
        let s = self.live_session(token).await?;
        let kept = serde_json::json!({"at": self.now().timestamp(), "action": action}).to_string();
        if kept.len() > PENDING_ACTION_MAX_BYTES {
            return Err(AuthError::Store("the pending action is too large".into()));
        }
        self.store
            .set_session_secret(&s.token_hash, SessionSecret::PendingAction, Some(&kept))
            .await?;
        Ok(())
    }

    /// The pending action of the session, once (`None`: none, or older than
    /// [`PENDING_ACTION_MINUTES`]).
    ///
    /// # Errors
    /// [`AuthError::ReauthRequired`] for an unknown session, storage errors.
    pub async fn take_pending_action(
        &self,
        token: &str,
    ) -> Result<Option<serde_json::Value>, AuthError> {
        let s = self.live_session(token).await?;
        let kept = self
            .store
            .session_secret(&s.token_hash, SessionSecret::PendingAction)
            .await?;
        if kept.is_some() {
            self.store
                .set_session_secret(&s.token_hash, SessionSecret::PendingAction, None)
                .await?;
        }
        let Some(v) = kept.and_then(|k| serde_json::from_str::<serde_json::Value>(&k).ok()) else {
            return Ok(None);
        };
        let at = v["at"].as_i64().unwrap_or(0);
        let young = self.now().timestamp() - at <= PENDING_ACTION_MINUTES * 60;
        Ok(young.then(|| v["action"].clone()))
    }

    pub(crate) async fn new_recovery_codes(&self, user: UserId) -> Result<Vec<String>, AuthError> {
        let plain: Vec<String> = (0..RECOVERY_CODES).map(|_| new_code()).collect();
        let mut stored = Vec::with_capacity(plain.len());
        for p in &plain {
            let normal = normalize_code(p).unwrap_or_default();
            stored.push(RecoveryCode {
                id: Uuid::now_v7(),
                code_hash: hash_password(normal).await?,
                used: false,
            });
        }
        self.store
            .put_recovery_codes(user, &stored, self.now())
            .await?;
        Ok(plain)
    }

    pub(crate) async fn use_recovery(&self, user: UserId, code: &str) -> Result<(), AuthError> {
        let normal = normalize_code(code).ok_or(AuthError::InvalidTotp)?;
        for c in self.store.recovery_codes(user).await? {
            if !c.used && verify_password(normal.clone(), c.code_hash.clone()).await {
                if self.store.use_recovery_code(c.id, self.now()).await? {
                    return Ok(());
                }
                break;
            }
        }
        Err(AuthError::InvalidTotp)
    }

    /// Disable or enable an account; disabling ends its sessions. Lock-out rules are checked
    /// by the caller (`api::admin`).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn set_disabled(
        &self,
        user: UserId,
        by: Option<UserId>,
        disabled: bool,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), AuthError> {
        let at = disabled.then(|| self.now());
        self.store.set_disabled(user, by, at, audit).await?;
        if disabled {
            self.store.delete_sessions_for(user).await?;
        }
        Ok(())
    }

    /// # Errors
    /// Storage errors.
    pub async fn is_disabled(&self, user: UserId) -> Result<bool, AuthError> {
        Ok(self.store.is_disabled(user).await?)
    }

    async fn open_invite(&self, token: &str) -> Result<Option<SignupInvite>, AuthError> {
        let now = self.now();
        Ok(self
            .invites
            .signup_invite_by_hash(&token_hash(token))
            .await?
            .filter(|i| i.used_at.is_none() && i.revoked_at.is_none() && i.expires_at > now))
    }

    pub(crate) async fn signup_invite_is_valid(&self, token: &str) -> Result<bool, AuthError> {
        Ok(self.open_invite(token).await?.is_some())
    }

    /// Claim an admin invite before its account is created (one conditional update).
    /// `Ok(None)`: the token is no admin invite (a companion invite, say);
    /// [`AuthError::InvalidInvite`]: it is used, revoked or expired.
    pub(crate) async fn claim_signup_invite(&self, token: &str) -> Result<Option<Uuid>, AuthError> {
        let Some(i) = self
            .invites
            .signup_invite_by_hash(&token_hash(token))
            .await?
        else {
            return Ok(None);
        };
        if self.invites.claim_signup_invite(i.id, self.now()).await? {
            Ok(Some(i.id))
        } else {
            Err(AuthError::InvalidInvite)
        }
    }

    /// Record who used a claimed invite, or give it back when no account was created.
    pub(crate) async fn finish_signup_invite(
        &self,
        id: Uuid,
        user: Option<UserId>,
    ) -> Result<(), AuthError> {
        Ok(self.invites.finish_signup_invite(id, user).await?)
    }

    /// A new admin invite; returns it and the token (shown once, only its hash is stored).
    /// `audit` is written in the same transaction, with the new invite as its target.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn create_signup_invite(
        &self,
        by: UserId,
        email: Option<String>,
        note: Option<String>,
        audit: Option<crate::admin::AuditEntry>,
    ) -> Result<(SignupInvite, String), AuthError> {
        let token = format!(
            "inv_{}",
            URL_SAFE_NO_PAD.encode(crate::random_bytes::<24>())
        );
        let now = self.now();
        let invite = SignupInvite {
            id: Uuid::now_v7(),
            token_hash: token_hash(&token),
            email,
            note,
            created_by: Some(by),
            created_at: now,
            expires_at: now + Duration::days(SIGNUP_INVITE_DAYS),
            used_by: None,
            used_at: None,
            revoked_at: None,
        };
        let audit = audit.map(|mut e| {
            e.target_type = Some("invite".into());
            e.target_id = Some(invite.id.to_string());
            e
        });
        self.invites
            .insert_signup_invite(&invite, audit.as_ref())
            .await?;
        Ok((invite, token))
    }

    /// Every admin invite, newest first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn signup_invites(&self) -> Result<Vec<SignupInvite>, AuthError> {
        Ok(self.invites.signup_invites().await?)
    }

    /// Revoke an open invite. Returns it, or `None` for an unknown id.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn revoke_signup_invite(
        &self,
        id: Uuid,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<Option<SignupInvite>, AuthError> {
        let Some(mut i) = self
            .invites
            .signup_invites()
            .await?
            .into_iter()
            .find(|i| i.id == id)
        else {
            return Ok(None);
        };
        if i.revoked_at.is_none() && i.used_at.is_none() {
            i.revoked_at = Some(self.now());
            self.invites.update_signup_invite(&i, audit).await?;
        }
        Ok(Some(i))
    }
}
