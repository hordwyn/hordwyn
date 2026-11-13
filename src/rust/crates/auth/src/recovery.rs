//! Shell-only escape hatches (addendum-1 3.3, DEC-283): a one-time password reset link and
//! removing an authenticator. The CLI calls these; the link is used through
//! `POST /auth/reset-password`.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Duration;
use domain::ids::UserId;
use uuid::Uuid;

use crate::store::PasswordReset;
use crate::{Auth, AuthError, check_password, hash_password, token_hash};

/// A reset link is valid for one hour and one use (phase 3 plan, WP3-1 defaults).
pub const PASSWORD_RESET_MINUTES: i64 = 60;

impl Auth {
    /// A new one-time reset token for `user` (shown once; only its hash is stored).
    ///
    /// # Errors
    /// Unknown user, storage errors.
    pub async fn create_password_reset(&self, user: UserId) -> Result<String, AuthError> {
        self.app.user(user).await?;
        let token = format!(
            "pwr_{}",
            URL_SAFE_NO_PAD.encode(crate::random_bytes::<24>())
        );
        let now = self.now();
        self.store
            .insert_password_reset(&PasswordReset {
                id: Uuid::now_v7(),
                user,
                token_hash: token_hash(&token),
                created_at: now,
                expires_at: now + Duration::minutes(PASSWORD_RESET_MINUTES),
            })
            .await?;
        Ok(token)
    }

    /// Set a new password with a reset token. The new password is checked first, so a weak
    /// one does not use up the link. Every session of the account ends. `Ok(None)`: the
    /// token is unknown, used or expired; that counts against the budget of the client
    /// address (SEC-5; a link names no account).
    ///
    /// # Errors
    /// The password rules, [`AuthError::TooManyAttempts`], storage errors.
    pub async fn reset_password(
        &self,
        token: &str,
        new: &str,
    ) -> Result<Option<UserId>, AuthError> {
        check_password(new)?;
        let hash = token_hash(token);
        let attempt = self.attempt(None).await?;
        if self
            .store
            .open_password_reset(&hash, self.now())
            .await?
            .is_none()
        {
            self.attempt_failed(attempt).await?;
            return Ok(None);
        }
        drop(attempt);
        let password_hash = hash_password(new.to_string()).await?;
        // claim, password and sessions in one transaction: a storage error keeps the link
        Ok(self
            .store
            .complete_password_reset(&hash, self.now(), &password_hash)
            .await?)
    }

    /// Delete reset links that have expired (daily retention job, UX3-12).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn purge_password_resets(&self) -> Result<u64, AuthError> {
        Ok(self.store.purge_password_resets(self.now()).await?)
    }

    /// Remove the authenticator and its recovery codes (`cli admin reset-totp`). Returns
    /// whether one was enrolled.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn reset_totp(&self, user: UserId) -> Result<bool, AuthError> {
        self.reset_totp_audited(user, None).await
    }

    /// [`Auth::reset_totp`] with the audit row in the same transaction (an admin in Users,
    /// DEC-336).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn reset_totp_audited(
        &self,
        user: UserId,
        audit: Option<&crate::admin::AuditEntry>,
    ) -> Result<bool, AuthError> {
        Ok(self.store.reset_totp(user, audit).await?)
    }
}
