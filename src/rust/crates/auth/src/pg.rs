//! Postgres adapter for [`AuthStore`] and [`InviteStore`] (store-pg migrations
//! `0005_auth.sql`, `0016_admin.sql`). TOTP seeds are stored encrypted (purpose
//! `totp_secret`, addendum-1 2.2).

use std::sync::Arc;

use app::ports::{
    PortError, RepoError, RepoResult, RewrapBatch, SecretCipher, SecretRows, SecretSlot,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use sqlx::PgPool;
use uuid::Uuid;

use zeroize::Zeroizing;

use crate::store::{
    AuthStore, Credentials, InviteStore, OidcIdentity, RecoveryCode, Session, SessionSecret,
    SignupInvite,
};

/// Purpose of the encrypted TOTP seed.
pub const TOTP_PURPOSE: &str = "totp_secret";
const TOTP_FIELD: &str = "totp_secret";
/// Purpose of the secrets kept per session (`session_secrets`, DEC-317); the field is the kind.
pub const SESSION_SECRET_PURPOSE: &str = "session_secret";

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("auth db: {e}"))
}

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn re(e: sqlx::Error) -> RepoError {
    RepoError::Backend(format!("auth db: {e}"))
}

fn totp_slot(row: &str) -> SecretSlot<'_> {
    SecretSlot {
        purpose: TOTP_PURPOSE,
        row,
        field: TOTP_FIELD,
    }
}

fn session_slot<'a>(row: &'a str, kind: &'static str) -> SecretSlot<'a> {
    SecretSlot {
        purpose: SESSION_SECRET_PURPOSE,
        row,
        field: kind,
    }
}

#[derive(Clone)]
pub struct PgAuthStore {
    pool: PgPool,
    cipher: Arc<dyn SecretCipher>,
}

impl PgAuthStore {
    #[must_use]
    pub fn new(pool: PgPool, cipher: Arc<dyn SecretCipher>) -> Self {
        Self { pool, cipher }
    }

    /// Delete expired sessions (cron job).
    ///
    /// # Errors
    /// Database errors.
    pub async fn purge_expired(
        &self,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<u64, PortError> {
        Ok(
            sqlx::query!("DELETE FROM auth_sessions WHERE expires_at <= $1", now)
                .execute(&self.pool)
                .await
                .map_err(pe)?
                .rows_affected(),
        )
    }

    /// Accounts with an enrolled authenticator whose seed cannot be decrypted (overview).
    ///
    /// # Errors
    /// Database errors.
    pub async fn locked_totp_count(&self) -> Result<u64, PortError> {
        let rows = sqlx::query!(
            "SELECT user_id, totp_secret_enc FROM credentials WHERE totp_secret_enc IS NOT NULL"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?;
        Ok(rows
            .iter()
            .filter(|r| {
                let row = r.user_id.to_string();
                r.totp_secret_enc
                    .as_deref()
                    .is_some_and(|b| self.cipher.open(totp_slot(&row), b).is_err())
            })
            .count() as u64)
    }
}

#[async_trait]
impl AuthStore for PgAuthStore {
    async fn credentials(&self, user: UserId) -> Result<Option<Credentials>, PortError> {
        let r = sqlx::query!(
            "SELECT password_hash, totp_secret, totp_secret_enc, totp_enabled, totp_last_step
             FROM credentials WHERE user_id = $1",
            user.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        let row = user.0.to_string();
        Ok(r.map(|r| {
            let (totp_secret, totp_locked) = match (&r.totp_secret_enc, r.totp_secret) {
                (Some(enc), _) => match self.cipher.open(totp_slot(&row), enc) {
                    Ok(s) => (Some(s), false),
                    Err(_) => (None, true),
                },
                // not migrated yet (the start-up migration encrypts it)
                (None, plain) => (plain, false),
            };
            Credentials {
                user_id: user,
                password_hash: r.password_hash,
                totp_secret,
                totp_enabled: r.totp_enabled,
                totp_last_step: r.totp_last_step,
                totp_locked,
            }
        }))
    }

    async fn put_credentials(&self, c: &Credentials) -> Result<(), PortError> {
        let row = c.user_id.0.to_string();
        let enc = c
            .totp_secret
            .as_ref()
            .map(|s| self.cipher.seal(totp_slot(&row), s));
        // a locked seed is kept as it is (never deleted automatically)
        sqlx::query!(
            "INSERT INTO credentials (user_id, password_hash, totp_secret, totp_secret_enc,
                                      totp_enabled, totp_last_step)
             VALUES ($1, $2, NULL, $3, $4, $5)
             ON CONFLICT (user_id) DO UPDATE SET password_hash = $2, totp_secret = NULL,
                 totp_secret_enc = CASE WHEN $6 THEN credentials.totp_secret_enc ELSE $3 END,
                 totp_enabled = $4, totp_last_step = $5",
            c.user_id.0,
            c.password_hash,
            enc,
            c.totp_enabled,
            c.totp_last_step,
            c.totp_locked && c.totp_secret.is_none()
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn insert_session(&self, s: &Session) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO auth_sessions (token_hash, user_id, created_at, last_seen_at, expires_at,
                                        reauth_at, reauth_method, reauth_slide_at, provider_id,
                                        oidc_sid)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            s.token_hash,
            s.user_id.0,
            s.created_at,
            s.last_seen_at,
            s.expires_at,
            s.reauth_at,
            s.reauth_method,
            s.reauth_slide_at,
            s.provider_id,
            s.oidc_sid
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn session(&self, token_hash: &str) -> Result<Option<Session>, PortError> {
        let r = sqlx::query_as!(
            SessionRow,
            "SELECT token_hash, user_id, created_at, last_seen_at, expires_at, reauth_at,
                    reauth_method, reauth_slide_at, provider_id, oidc_sid
             FROM auth_sessions WHERE token_hash = $1",
            token_hash
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.map(Session::from))
    }

    async fn update_session(&self, s: &Session) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE auth_sessions SET last_seen_at = $2, expires_at = $3, reauth_at = $4,
                 reauth_method = $5, reauth_slide_at = $6
             WHERE token_hash = $1",
            s.token_hash,
            s.last_seen_at,
            s.expires_at,
            s.reauth_at,
            s.reauth_method,
            s.reauth_slide_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn delete_session(&self, token_hash: &str) -> Result<(), PortError> {
        sqlx::query!(
            "DELETE FROM auth_sessions WHERE token_hash = $1",
            token_hash
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn delete_sessions_for(&self, user: UserId) -> Result<(), PortError> {
        sqlx::query!("DELETE FROM auth_sessions WHERE user_id = $1", user.0)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }

    async fn delete_sessions_by_sid(&self, provider: Uuid, sid: &str) -> Result<u64, PortError> {
        Ok(sqlx::query!(
            "DELETE FROM auth_sessions WHERE provider_id = $1 AND oidc_sid = $2",
            provider,
            sid
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected())
    }

    async fn delete_sessions_via(&self, user: UserId, provider: Uuid) -> Result<u64, PortError> {
        Ok(sqlx::query!(
            "DELETE FROM auth_sessions WHERE user_id = $1 AND provider_id = $2",
            user.0,
            provider
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected())
    }

    async fn set_session_secret(
        &self,
        token_hash: &str,
        kind: SessionSecret,
        value: Option<&str>,
    ) -> Result<(), PortError> {
        let Some(v) = value else {
            sqlx::query!(
                "DELETE FROM session_secrets WHERE token_hash = $1 AND kind = $2",
                token_hash,
                kind.as_str()
            )
            .execute(&self.pool)
            .await
            .map_err(pe)?;
            return Ok(());
        };
        // a new id per value: the id is the row of the associated data
        let id = Uuid::now_v7();
        let row = id.to_string();
        let enc = self
            .cipher
            .seal(session_slot(&row, kind.as_str()), v.as_bytes());
        // no row when the session is gone meanwhile (it ended in between)
        sqlx::query!(
            "INSERT INTO session_secrets (id, token_hash, kind, value, created_at)
             SELECT $1, $2, $3, $4, now() WHERE EXISTS
                 (SELECT 1 FROM auth_sessions WHERE token_hash = $2)
             ON CONFLICT (token_hash, kind) DO UPDATE SET id = $1, value = $4, created_at = now()",
            id,
            token_hash,
            kind.as_str(),
            enc
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn session_secret(
        &self,
        token_hash: &str,
        kind: SessionSecret,
    ) -> Result<Option<Zeroizing<String>>, PortError> {
        let r = sqlx::query!(
            "SELECT id, value FROM session_secrets WHERE token_hash = $1 AND kind = $2",
            token_hash,
            kind.as_str()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.and_then(|r| {
            let row = r.id.to_string();
            match self
                .cipher
                .open(session_slot(&row, kind.as_str()), &r.value)
            {
                Ok(b) => String::from_utf8(b).ok().map(Zeroizing::new),
                Err(e) => {
                    // a key version that is not loaded: as if nothing were kept
                    tracing::warn!(error = %e, kind = kind.as_str(), "session secret locked");
                    None
                }
            }
        }))
    }

    async fn move_session_secrets(&self, from: &str, to: &str) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE session_secrets SET token_hash = $2 WHERE token_hash = $1",
            from,
            to
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn sessions_for(&self, user: UserId) -> Result<Vec<Session>, PortError> {
        Ok(sqlx::query_as!(
            SessionRow,
            "SELECT token_hash, user_id, created_at, last_seen_at, expires_at, reauth_at,
                    reauth_method, reauth_slide_at, provider_id, oidc_sid
             FROM auth_sessions WHERE user_id = $1 ORDER BY last_seen_at DESC",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(Session::from)
        .collect())
    }

    async fn identity(
        &self,
        issuer: &str,
        subject: &str,
    ) -> Result<Option<OidcIdentity>, PortError> {
        Ok(sqlx::query_as!(
            IdentityRow,
            "SELECT id, issuer, subject, user_id, provider_id, sid, linked_at, last_claims_hash,
                    last_groups
             FROM oidc_identities WHERE issuer = $1 AND subject = $2",
            issuer,
            subject
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(OidcIdentity::from))
    }

    async fn link_identity(&self, i: &OidcIdentity) -> Result<bool, PortError> {
        Ok(sqlx::query!(
            "INSERT INTO oidc_identities (id, issuer, subject, user_id, provider_id, sid,
                                          linked_at, last_claims_hash, last_groups)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (issuer, subject) DO NOTHING",
            i.id,
            i.issuer,
            i.subject,
            i.user_id.0,
            i.provider_id,
            i.sid,
            i.linked_at,
            i.last_claims_hash,
            i.last_groups.as_deref()
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected()
            == 1)
    }

    async fn touch_identity(&self, i: &OidcIdentity) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE oidc_identities SET provider_id = $3, sid = $4, last_claims_hash = $5,
                 last_groups = $6
             WHERE issuer = $1 AND subject = $2",
            i.issuer,
            i.subject,
            i.provider_id,
            i.sid,
            i.last_claims_hash,
            i.last_groups.as_deref()
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn unlink_identity(&self, user: UserId, id: Uuid) -> Result<bool, PortError> {
        Ok(sqlx::query!(
            "DELETE FROM oidc_identities WHERE id = $1 AND user_id = $2",
            id,
            user.0
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected()
            == 1)
    }

    async fn identities_of(&self, user: UserId) -> Result<Vec<OidcIdentity>, PortError> {
        Ok(sqlx::query_as!(
            IdentityRow,
            "SELECT id, issuer, subject, user_id, provider_id, sid, linked_at, last_claims_hash,
                    last_groups
             FROM oidc_identities WHERE user_id = $1 ORDER BY linked_at, issuer",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(OidcIdentity::from)
        .collect())
    }

    async fn put_recovery_codes(
        &self,
        user: UserId,
        codes: &[RecoveryCode],
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!("DELETE FROM totp_recovery_codes WHERE user_id = $1", user.0)
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        for c in codes {
            sqlx::query!(
                "INSERT INTO totp_recovery_codes (id, user_id, code_hash, created_at)
                 VALUES ($1, $2, $3, $4)",
                c.id,
                user.0,
                c.code_hash,
                at
            )
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        }
        tx.commit().await.map_err(pe)
    }

    async fn recovery_codes(&self, user: UserId) -> Result<Vec<RecoveryCode>, PortError> {
        Ok(sqlx::query!(
            "SELECT id, code_hash, used_at FROM totp_recovery_codes WHERE user_id = $1
             ORDER BY created_at, id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(|r| RecoveryCode {
            id: r.id,
            code_hash: r.code_hash,
            used: r.used_at.is_some(),
        })
        .collect())
    }

    async fn use_recovery_code(&self, id: Uuid, at: DateTime<Utc>) -> Result<bool, PortError> {
        Ok(sqlx::query!(
            "UPDATE totp_recovery_codes SET used_at = $2 WHERE id = $1 AND used_at IS NULL",
            id,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected()
            == 1)
    }

    async fn is_disabled(&self, user: UserId) -> Result<bool, PortError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM user_disabled WHERE user_id = $1) AS "e!""#,
            user.0
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pe)?)
    }

    async fn set_disabled(
        &self,
        user: UserId,
        by: Option<UserId>,
        at: Option<DateTime<Utc>>,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        if let Some(at) = at {
            sqlx::query!(
                "INSERT INTO user_disabled (user_id, disabled_by, disabled_at) VALUES ($1, $2, $3)
                 ON CONFLICT (user_id) DO NOTHING",
                user.0,
                by.map(|b| b.0),
                at
            )
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        } else {
            sqlx::query!("DELETE FROM user_disabled WHERE user_id = $1", user.0)
                .execute(&mut *tx)
                .await
                .map_err(pe)?;
        }
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)
    }

    async fn insert_password_reset(
        &self,
        r: &crate::store::PasswordReset,
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "DELETE FROM password_resets WHERE user_id = $1 AND used_at IS NULL",
            r.user.0
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        sqlx::query!(
            "INSERT INTO password_resets (id, user_id, token_hash, created_at, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
            r.id,
            r.user.0,
            r.token_hash,
            r.created_at,
            r.expires_at
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        tx.commit().await.map_err(pe)
    }

    async fn open_password_reset(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<UserId>, PortError> {
        Ok(sqlx::query_scalar!(
            "SELECT user_id FROM password_resets
             WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2",
            token_hash,
            now
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(UserId))
    }

    async fn complete_password_reset(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
        password_hash: &str,
    ) -> Result<Option<UserId>, PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        let Some(user) = sqlx::query_scalar!(
            "UPDATE password_resets SET used_at = $2
             WHERE token_hash = $1 AND used_at IS NULL AND expires_at > $2
             RETURNING user_id",
            token_hash,
            now
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(pe)?
        else {
            return Ok(None);
        };
        sqlx::query!(
            "INSERT INTO credentials (user_id, password_hash) VALUES ($1, $2)
             ON CONFLICT (user_id) DO UPDATE SET password_hash = $2",
            user,
            password_hash
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        sqlx::query!("DELETE FROM auth_sessions WHERE user_id = $1", user)
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        tx.commit().await.map_err(pe)?;
        Ok(Some(UserId(user)))
    }

    async fn purge_password_resets(&self, now: DateTime<Utc>) -> Result<u64, PortError> {
        Ok(
            sqlx::query!("DELETE FROM password_resets WHERE expires_at <= $1", now)
                .execute(&self.pool)
                .await
                .map_err(pe)?
                .rows_affected(),
        )
    }
}

async fn audit_opt(
    tx: &mut sqlx::PgConnection,
    audit: Option<&crate::admin::AuditEntry>,
) -> Result<(), PortError> {
    match audit {
        Some(e) => crate::admin_pg::insert_audit(tx, e).await,
        None => Ok(()),
    }
}

struct SessionRow {
    token_hash: String,
    user_id: Uuid,
    created_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    reauth_at: Option<DateTime<Utc>>,
    reauth_method: Option<String>,
    reauth_slide_at: Option<DateTime<Utc>>,
    provider_id: Option<Uuid>,
    oidc_sid: Option<String>,
}

impl From<SessionRow> for Session {
    fn from(r: SessionRow) -> Self {
        Self {
            token_hash: r.token_hash,
            user_id: UserId(r.user_id),
            created_at: r.created_at,
            last_seen_at: r.last_seen_at,
            expires_at: r.expires_at,
            reauth_at: r.reauth_at,
            reauth_method: r.reauth_method,
            reauth_slide_at: r.reauth_slide_at,
            provider_id: r.provider_id,
            oidc_sid: r.oidc_sid,
        }
    }
}

struct IdentityRow {
    id: Uuid,
    issuer: String,
    subject: String,
    user_id: Uuid,
    provider_id: Option<Uuid>,
    sid: Option<String>,
    linked_at: DateTime<Utc>,
    last_claims_hash: Option<String>,
    last_groups: Option<Vec<String>>,
}

impl From<IdentityRow> for OidcIdentity {
    fn from(r: IdentityRow) -> Self {
        Self {
            id: r.id,
            issuer: r.issuer,
            subject: r.subject,
            user_id: UserId(r.user_id),
            provider_id: r.provider_id,
            sid: r.sid,
            linked_at: r.linked_at,
            last_claims_hash: r.last_claims_hash,
            last_groups: r.last_groups,
        }
    }
}

struct InviteRow {
    id: Uuid,
    token_hash: String,
    email: Option<String>,
    note: Option<String>,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    used_by: Option<Uuid>,
    used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

impl From<InviteRow> for SignupInvite {
    fn from(r: InviteRow) -> Self {
        Self {
            id: r.id,
            token_hash: r.token_hash,
            email: r.email,
            note: r.note,
            created_by: r.created_by.map(UserId),
            created_at: r.created_at,
            expires_at: r.expires_at,
            used_by: r.used_by.map(UserId),
            used_at: r.used_at,
            revoked_at: r.revoked_at,
        }
    }
}

#[async_trait]
impl InviteStore for PgAuthStore {
    async fn insert_signup_invite(
        &self,
        i: &SignupInvite,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "INSERT INTO signup_invites (id, token_hash, email, note, created_by, created_at,
                                         expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
            i.id,
            i.token_hash,
            i.email,
            i.note,
            i.created_by.map(|u| u.0),
            i.created_at,
            i.expires_at
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)
    }

    async fn signup_invite_by_hash(&self, hash: &str) -> Result<Option<SignupInvite>, PortError> {
        Ok(sqlx::query_as!(
            InviteRow,
            "SELECT id, token_hash, email, note, created_by, created_at, expires_at, used_by,
                    used_at, revoked_at
             FROM signup_invites WHERE token_hash = $1",
            hash
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(SignupInvite::from))
    }

    async fn signup_invites(&self) -> Result<Vec<SignupInvite>, PortError> {
        Ok(sqlx::query_as!(
            InviteRow,
            "SELECT id, token_hash, email, note, created_by, created_at, expires_at, used_by,
                    used_at, revoked_at
             FROM signup_invites ORDER BY created_at DESC, id DESC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(SignupInvite::from)
        .collect())
    }

    async fn update_signup_invite(
        &self,
        i: &SignupInvite,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "UPDATE signup_invites SET used_by = $2, used_at = $3, revoked_at = $4 WHERE id = $1",
            i.id,
            i.used_by.map(|u| u.0),
            i.used_at,
            i.revoked_at
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)
    }

    async fn claim_signup_invite(&self, id: Uuid, now: DateTime<Utc>) -> Result<bool, PortError> {
        let done = sqlx::query!(
            "UPDATE signup_invites SET used_at = $2
             WHERE id = $1 AND used_at IS NULL AND revoked_at IS NULL AND expires_at > $2",
            id,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected();
        Ok(done == 1)
    }

    async fn finish_signup_invite(&self, id: Uuid, user: Option<UserId>) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE signup_invites
             SET used_by = $2, used_at = CASE WHEN $2::uuid IS NULL THEN NULL ELSE used_at END
             WHERE id = $1 AND used_by IS NULL",
            id,
            user.map(|u| u.0)
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }
}

/// The TOTP seed column for the rotation job and the start-up migration.
#[async_trait]
impl SecretRows for PgAuthStore {
    fn purpose(&self) -> &'static str {
        TOTP_PURPOSE
    }

    async fn seal_plaintext(&self, cipher: &dyn SecretCipher) -> RepoResult<u64> {
        let rows = sqlx::query!(
            r#"SELECT user_id, totp_secret AS "totp_secret!" FROM credentials
               WHERE totp_secret IS NOT NULL AND totp_secret_enc IS NULL"#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(re)?;
        let mut n = 0;
        for r in rows {
            let row = r.user_id.to_string();
            let enc = cipher.seal(totp_slot(&row), &r.totp_secret);
            n += sqlx::query!(
                "UPDATE credentials SET totp_secret_enc = $2, totp_secret = NULL
                 WHERE user_id = $1 AND totp_secret_enc IS NULL",
                r.user_id,
                enc
            )
            .execute(&self.pool)
            .await
            .map_err(re)?
            .rows_affected();
        }
        Ok(n)
    }

    async fn version_counts(&self) -> RepoResult<Vec<(u16, u64)>> {
        version_counts(&self.pool, "credentials", "totp_secret_enc").await
    }

    async fn rewrap_batch(
        &self,
        cipher: &dyn SecretCipher,
        active: u16,
        after: Option<Uuid>,
        limit: u32,
    ) -> RepoResult<RewrapBatch> {
        let rows = sqlx::query!(
            r#"SELECT user_id, totp_secret_enc AS "blob!" FROM credentials
               WHERE totp_secret_enc IS NOT NULL
                 AND ($3::uuid IS NULL OR user_id > $3)
                 AND CASE WHEN length(totp_secret_enc) >= 3
                     THEN get_byte(totp_secret_enc, 1) * 256 + get_byte(totp_secret_enc, 2) <> $1
                     ELSE false END
               ORDER BY user_id LIMIT $2"#,
            i32::from(active),
            i64::from(limit),
            after
        )
        .fetch_all(&self.pool)
        .await
        .map_err(re)?;
        let last = rows.last().map(|r| r.user_id);
        let (mut done, mut locked) = (0, 0);
        for r in rows {
            let row = r.user_id.to_string();
            match cipher.rewrap(totp_slot(&row), &r.blob) {
                Ok(Some(new)) => {
                    done += sqlx::query!(
                        "UPDATE credentials SET totp_secret_enc = $3
                         WHERE user_id = $1 AND totp_secret_enc = $2",
                        r.user_id,
                        r.blob,
                        new
                    )
                    .execute(&self.pool)
                    .await
                    .map_err(re)?
                    .rows_affected();
                }
                Ok(None) => {}
                Err(_) => locked += 1,
            }
        }
        Ok(RewrapBatch {
            rewrapped: done,
            locked,
            last,
        })
    }
}

/// Rows per key version of an encrypted column (bytes 1-2 of the envelope), no decryption.
///
/// # Errors
/// Database errors.
pub async fn version_counts(
    pool: &PgPool,
    table: &'static str,
    column: &'static str,
) -> RepoResult<Vec<(u16, u64)>> {
    // table and column are compile-time constants of the purpose registry
    let sql = format!(
        "SELECT (get_byte({column}, 1) * 256 + get_byte({column}, 2))::int AS v, COUNT(*) AS n
         FROM {table} WHERE {column} IS NOT NULL AND length({column}) >= 3
         GROUP BY 1 ORDER BY 1"
    );
    let rows: Vec<(i32, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .fetch_all(pool)
        .await
        .map_err(re)?;
    Ok(rows
        .into_iter()
        .filter_map(|(v, n)| Some((u16::try_from(v).ok()?, u64::try_from(n).ok()?)))
        .collect())
}

/// The secrets kept per session as a [`SecretRows`] purpose (re-wrap job, key status):
/// [`SESSION_SECRET_PURPOSE`], table `session_secrets`.
#[derive(Clone)]
pub struct PgSessionSecrets {
    pool: PgPool,
}

impl PgSessionSecrets {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SecretRows for PgSessionSecrets {
    fn purpose(&self) -> &'static str {
        SESSION_SECRET_PURPOSE
    }

    /// Never stored in plain text.
    async fn seal_plaintext(&self, _cipher: &dyn SecretCipher) -> RepoResult<u64> {
        Ok(0)
    }

    async fn version_counts(&self) -> RepoResult<Vec<(u16, u64)>> {
        version_counts(&self.pool, "session_secrets", "value").await
    }

    async fn rewrap_batch(
        &self,
        cipher: &dyn SecretCipher,
        active: u16,
        after: Option<Uuid>,
        limit: u32,
    ) -> RepoResult<RewrapBatch> {
        let rows = sqlx::query!(
            r#"SELECT id, kind, value FROM session_secrets
               WHERE ($3::uuid IS NULL OR id > $3)
                 AND CASE WHEN length(value) >= 3
                     THEN get_byte(value, 1) * 256 + get_byte(value, 2) <> $1
                     ELSE false END
               ORDER BY id LIMIT $2"#,
            i32::from(active),
            i64::from(limit),
            after
        )
        .fetch_all(&self.pool)
        .await
        .map_err(re)?;
        let last = rows.last().map(|r| r.id);
        let (mut done, mut locked) = (0, 0);
        for r in rows {
            let row = r.id.to_string();
            let kind = match r.kind.as_str() {
                "id_token" => SessionSecret::IdToken,
                _ => SessionSecret::PendingAction,
            };
            match cipher.rewrap(session_slot(&row, kind.as_str()), &r.value) {
                Ok(Some(new)) => {
                    done += sqlx::query!(
                        "UPDATE session_secrets SET value = $3 WHERE id = $1 AND value = $2",
                        r.id,
                        r.value,
                        new
                    )
                    .execute(&self.pool)
                    .await
                    .map_err(re)?
                    .rows_affected();
                }
                Ok(None) => {}
                Err(_) => locked += 1,
            }
        }
        Ok(RewrapBatch {
            rewrapped: done,
            locked,
            last,
        })
    }
}
