//! Postgres adapter for [`AuthStore`] (store-pg migration `0005_auth.sql`).

use app::ports::PortError;
use async_trait::async_trait;
use domain::ids::UserId;
use sqlx::PgPool;

use crate::store::{AuthStore, Credentials, OidcIdentity, Session};

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("auth db: {e}"))
}

#[derive(Clone)]
pub struct PgAuthStore {
    pool: PgPool,
}

impl PgAuthStore {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
}

#[async_trait]
impl AuthStore for PgAuthStore {
    async fn credentials(&self, user: UserId) -> Result<Option<Credentials>, PortError> {
        let r = sqlx::query!(
            "SELECT password_hash, totp_secret, totp_enabled, totp_last_step
             FROM credentials WHERE user_id = $1",
            user.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.map(|r| Credentials {
            user_id: user,
            password_hash: r.password_hash,
            totp_secret: r.totp_secret,
            totp_enabled: r.totp_enabled,
            totp_last_step: r.totp_last_step,
        }))
    }

    async fn put_credentials(&self, c: &Credentials) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO credentials (user_id, password_hash, totp_secret, totp_enabled, totp_last_step)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id) DO UPDATE SET password_hash = $2, totp_secret = $3,
                 totp_enabled = $4, totp_last_step = $5",
            c.user_id.0,
            c.password_hash,
            c.totp_secret,
            c.totp_enabled,
            c.totp_last_step
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn insert_session(&self, s: &Session) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO auth_sessions (token_hash, user_id, created_at, last_seen_at, expires_at)
             VALUES ($1, $2, $3, $4, $5)",
            s.token_hash,
            s.user_id.0,
            s.created_at,
            s.last_seen_at,
            s.expires_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn session(&self, token_hash: &str) -> Result<Option<Session>, PortError> {
        let r = sqlx::query!(
            "SELECT token_hash, user_id, created_at, last_seen_at, expires_at
             FROM auth_sessions WHERE token_hash = $1",
            token_hash
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.map(|r| Session {
            token_hash: r.token_hash,
            user_id: UserId(r.user_id),
            created_at: r.created_at,
            last_seen_at: r.last_seen_at,
            expires_at: r.expires_at,
        }))
    }

    async fn update_session(&self, s: &Session) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE auth_sessions SET last_seen_at = $2, expires_at = $3 WHERE token_hash = $1",
            s.token_hash,
            s.last_seen_at,
            s.expires_at
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
}
