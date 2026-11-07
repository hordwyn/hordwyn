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

use crate::store::{
    AuthStore, Credentials, InviteStore, OidcIdentity, RecoveryCode, Session, SignupInvite,
};

/// Purpose of the encrypted TOTP seed.
pub const TOTP_PURPOSE: &str = "totp_secret";
const TOTP_FIELD: &str = "totp_secret";

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
                                        reauth_at, reauth_method, reauth_slide_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            s.token_hash,
            s.user_id.0,
            s.created_at,
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

    async fn session(&self, token_hash: &str) -> Result<Option<Session>, PortError> {
        let r = sqlx::query_as!(
            SessionRow,
            "SELECT token_hash, user_id, created_at, last_seen_at, expires_at, reauth_at,
                    reauth_method, reauth_slide_at
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

    async fn sessions_for(&self, user: UserId) -> Result<Vec<Session>, PortError> {
        Ok(sqlx::query_as!(
            SessionRow,
            "SELECT token_hash, user_id, created_at, last_seen_at, expires_at, reauth_at,
                    reauth_method, reauth_slide_at
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
        let r = sqlx::query!(
            "SELECT user_id FROM oidc_identities WHERE issuer = $1 AND subject = $2",
            issuer,
            subject
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.map(|r| OidcIdentity {
            issuer: issuer.to_string(),
            subject: subject.to_string(),
            user_id: UserId(r.user_id),
        }))
    }

    async fn link_identity(&self, i: &OidcIdentity) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO oidc_identities (issuer, subject, user_id) VALUES ($1, $2, $3)
             ON CONFLICT (issuer, subject) DO UPDATE SET user_id = $3",
            i.issuer,
            i.subject,
            i.user_id.0
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn identities_of(&self, user: UserId) -> Result<Vec<OidcIdentity>, PortError> {
        Ok(sqlx::query!(
            "SELECT issuer, subject FROM oidc_identities WHERE user_id = $1 ORDER BY issuer",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(|r| OidcIdentity {
            issuer: r.issuer,
            subject: r.subject,
            user_id: user,
        })
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
    ) -> Result<(), PortError> {
        if let Some(at) = at {
            sqlx::query!(
                "INSERT INTO user_disabled (user_id, disabled_by, disabled_at) VALUES ($1, $2, $3)
                 ON CONFLICT (user_id) DO NOTHING",
                user.0,
                by.map(|b| b.0),
                at
            )
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        } else {
            sqlx::query!("DELETE FROM user_disabled WHERE user_id = $1", user.0)
                .execute(&self.pool)
                .await
                .map_err(pe)?;
        }
        Ok(())
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
    async fn insert_signup_invite(&self, i: &SignupInvite) -> Result<(), PortError> {
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
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
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

    async fn update_signup_invite(&self, i: &SignupInvite) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE signup_invites SET used_by = $2, used_at = $3, revoked_at = $4 WHERE id = $1",
            i.id,
            i.used_by.map(|u| u.0),
            i.used_at,
            i.revoked_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
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
