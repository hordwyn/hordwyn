//! The emailed one-time link of "Is this your account?" (addendum-1 4.2 step 3, DEC-312,
//! DEC-316): an account without a password cannot confirm the match with a password, so it
//! may ask for a link to its stored address (which only ever comes from a verified provider
//! claim, step 4). The link lives [`EMAIL_LINK_MINUTES`], works once, is stored as a hash
//! (the `password_resets` table with purpose `oidc_link`, never usable as a reset) and is
//! bound to the link ticket of the browser that asked for it.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Duration;
use domain::user::User;
use uuid::Uuid;

use super::cookie::{self, PendingLink};
use super::provider::Provider;
use super::resolve::LinkError;
use super::{LINK_KIND, Oidc};
use crate::store::PasswordReset;
use crate::{Auth, AuthError, OidcIdentity, SessionToken, token_hash};

/// How long an emailed link works (phase 4 plan, WP4-2 default).
pub const EMAIL_LINK_MINUTES: i64 = 30;
/// Links an account may ask for within [`EMAIL_LINK_WINDOW_MINUTES`] (per process).
pub const EMAIL_LINKS_PER_WINDOW: usize = 3;
pub const EMAIL_LINK_WINDOW_MINUTES: i64 = 15;

/// A link to send: the code for the URL, the renewed ticket cookie (it now lasts as long as
/// the link) and where it goes.
pub struct EmailedLink {
    pub code: String,
    pub ticket: String,
    pub to: String,
    pub user: User,
    pub provider: Provider,
}

impl std::fmt::Debug for EmailedLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailedLink")
            .field("user", &self.user.id)
            .finish_non_exhaustive()
    }
}

/// What a code is bound to: the single-use id of the asking browser's ticket.
fn binding(t: &PendingLink) -> String {
    cookie::used_key("email_link", &t.id)
}

impl Oidc {
    /// May "Email me a link" be offered: no password, a stored address, and mail works
    /// (`mail` is the caller's knowledge of the transport)?
    #[must_use]
    pub fn can_email(user: &User, has_password: bool, mail: bool) -> bool {
        mail && !has_password && user.email.as_deref().is_some_and(|e| !e.trim().is_empty())
    }

    /// Make a link for the account of `ticket`. The caller sends it and sets the renewed
    /// ticket cookie.
    ///
    /// # Errors
    /// [`LinkError::Ticket`], [`LinkError::NoEmail`], [`LinkError::TooMany`].
    pub async fn email_link(
        &self,
        auth: &Auth,
        ticket: Option<&str>,
        mail: bool,
    ) -> Result<EmailedLink, LinkError> {
        let (t, user, provider, has_password) = self.link_pending(auth, ticket).await?;
        if !Self::can_email(&user, has_password, mail) {
            return Err(LinkError::NoEmail);
        }
        let now = self.clock().now();
        if let Some(retry_after_secs) = self.guards.take_link_mail(user.id.0, now) {
            return Err(LinkError::TooMany { retry_after_secs });
        }
        let code = format!(
            "lnk_{}",
            URL_SAFE_NO_PAD.encode(crate::random_bytes::<24>())
        );
        let expires = now + Duration::minutes(EMAIL_LINK_MINUTES);
        auth.store
            .insert_sign_in_link(
                &PasswordReset {
                    id: Uuid::now_v7(),
                    user: user.id,
                    token_hash: token_hash(&code),
                    created_at: now,
                    expires_at: expires,
                },
                &binding(&t),
            )
            .await
            .map_err(AuthError::from)?;
        let ticket = cookie::seal(self.keys(), LINK_KIND, &t, expires.timestamp());
        Ok(EmailedLink {
            code,
            ticket,
            to: user.email.clone().unwrap_or_default(),
            user,
            provider,
        })
    }

    /// Use an emailed `code` in the browser holding `ticket`: link the identity and sign in
    /// through the provider. The code and the ticket work once.
    ///
    /// # Errors
    /// [`LinkError::Ticket`], [`LinkError::Code`], [`LinkError::InUse`], [`LinkError::NoEmail`]
    /// (a password was set since), [`AuthError::AccountDisabled`], [`AuthError::TooManyAttempts`].
    pub async fn confirm_email_link(
        &self,
        auth: &Auth,
        ticket: Option<&str>,
        code: &str,
    ) -> Result<(SessionToken, String, User), LinkError> {
        let (t, user, provider, has_password) = self.link_pending(auth, ticket).await?;
        // the conditions of the offer still hold (UX4-11): checked before anything is used
        // up, so a refusal leaves the code and the ticket as they were
        if has_password {
            return Err(LinkError::NoEmail);
        }
        if auth
            .store
            .is_disabled(user.id)
            .await
            .map_err(AuthError::from)?
        {
            return Err(AuthError::AccountDisabled.into());
        }
        let now = self.clock().now();
        // a wrong code is a wrong secret of this account (SEC-5)
        let attempt = auth.attempt(Some(&user.username)).await?;
        let claimed = auth
            .store
            .claim_sign_in_link(&token_hash(code.trim()), &binding(&t), now)
            .await
            .map_err(AuthError::from)?;
        if claimed != Some(user.id) {
            auth.attempt_failed(attempt).await?;
            return Err(LinkError::Code);
        }
        drop(attempt);
        let once_key = cookie::used_key(LINK_KIND, &t.id);
        let expires = now + Duration::minutes(EMAIL_LINK_MINUTES + 1);
        if !self
            .store()
            .claim_once(&once_key, expires, now)
            .await
            .map_err(AuthError::from)?
        {
            return Err(LinkError::Ticket);
        }
        let mut i = OidcIdentity::new(&t.issuer, &t.subject, user.id, now);
        i.provider_id = Some(provider.id);
        i.sid.clone_from(&t.sid);
        i.last_claims_hash = Some(t.claims_hash.clone());
        i.last_groups.clone_from(&t.groups);
        if !auth
            .store
            .link_identity(&i)
            .await
            .map_err(AuthError::from)?
        {
            return Err(LinkError::InUse);
        }
        let token = auth
            .create_session_from(user.id, "oidc", Some(provider.id), t.sid.clone())
            .await?;
        Ok((token, t.return_to, user))
    }
}
