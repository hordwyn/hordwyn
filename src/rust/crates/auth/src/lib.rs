//! Passwords (argon2id), sessions, TOTP, OIDC and roles.
//!
//! Account rows belong to `app` (users + self-companion); this crate owns credentials,
//! sessions and OIDC identities. Session tokens are random 256-bit values; only their blake3
//! hash is stored. Password hashing runs on tokio's blocking pool, never on a worker thread.
#![deny(unsafe_code)]

pub mod oidc;
pub mod pg;
mod store;

use std::sync::Arc;

use app::ports::Clock;
use app::{App, AppError};
use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use domain::ids::UserId;
use domain::user::{Actor, Role, User};
use thiserror::Error;

pub use store::{AuthStore, Credentials, MemAuthStore, OidcIdentity, Session};

/// Minimum password length (NIST SP 800-63B recommends at least 8; we ask for 10).
pub const MIN_PASSWORD_LEN: usize = 10;
pub const MAX_PASSWORD_LEN: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SignupMode {
    Open,
    #[default]
    Invite,
    Closed,
}

#[derive(Debug, Clone)]
pub struct AuthSettings {
    pub signup: SignupMode,
    pub session_days: u32,
    /// Shown in authenticator apps (from the brand config).
    pub totp_issuer: String,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            signup: SignupMode::Invite,
            session_days: 30,
            totp_issuer: "app".into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    /// Wrong username or password (deliberately not saying which).
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("a one-time code is required")]
    TotpRequired,
    #[error("invalid one-time code")]
    InvalidTotp,
    #[error("sign-up is not available")]
    SignupClosed,
    #[error("invalid invite")]
    InvalidInvite,
    #[error("password must be {MIN_PASSWORD_LEN} to {MAX_PASSWORD_LEN} characters")]
    WeakPassword,
    #[error("usernames are 3 to 32 characters: a-z, 0-9, _ . -")]
    InvalidUsername,
    #[error("display names are 1 to 100 characters")]
    InvalidDisplayName,
    #[error(transparent)]
    App(#[from] AppError),
    #[error("auth storage: {0}")]
    Store(String),
    #[error("oidc: {0}")]
    Oidc(String),
    /// Too many failed sign-ins for this name; try again later (G2-4).
    #[error("too many failed attempts, try again in {retry_after_secs} s")]
    TooManyAttempts { retry_after_secs: i64 },
}

/// Failed sign-ins per username allowed within [`THROTTLE_WINDOW_MINUTES`].
pub const MAX_FAILED_SIGN_INS: u32 = 10;
pub const THROTTLE_WINDOW_MINUTES: i64 = 15;
/// Names tracked at most (oldest windows are dropped first).
const THROTTLE_CAPACITY: usize = 10_000;

/// Failed sign-ins per lowercased username: (count, window start). In memory, per process.
#[derive(Default)]
struct Throttle(std::sync::Mutex<std::collections::HashMap<String, (u32, DateTime<Utc>)>>);

impl Throttle {
    fn check(&self, name: &str, now: DateTime<Utc>) -> Result<(), AuthError> {
        let window = Duration::minutes(THROTTLE_WINDOW_MINUTES);
        let map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match map.get(name) {
            Some((n, start)) if *n >= MAX_FAILED_SIGN_INS && now < *start + window => {
                Err(AuthError::TooManyAttempts {
                    retry_after_secs: (*start + window - now).num_seconds().max(1),
                })
            }
            _ => Ok(()),
        }
    }

    fn failed(&self, name: &str, now: DateTime<Utc>) {
        let window = Duration::minutes(THROTTLE_WINDOW_MINUTES);
        let mut map = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if map.len() >= THROTTLE_CAPACITY && !map.contains_key(name) {
            map.retain(|_, (_, start)| now < *start + window);
            if map.len() >= THROTTLE_CAPACITY
                && let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, (_, s))| *s)
                    .map(|(k, _)| k.clone())
            {
                map.remove(&oldest);
            }
        }
        let e = map.entry(name.to_string()).or_insert((0, now));
        if now >= e.1 + window {
            *e = (0, now);
        }
        e.0 += 1;
    }

    fn succeeded(&self, name: &str) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(name);
    }
}

impl From<app::ports::PortError> for AuthError {
    fn from(e: app::ports::PortError) -> Self {
        Self::Store(e.0)
    }
}

/// An opaque session token as handed to the client (cookie or bearer).
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(pub String);

impl std::fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionToken(***)")
    }
}

/// New random secret bytes.
///
/// # Panics
/// If the operating system has no randomness source.
#[must_use]
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).expect("os randomness");
    b
}

#[must_use]
pub fn token_hash(token: &str) -> String {
    blake3::hash(token.as_bytes()).to_hex().to_string()
}

/// argon2id with the crate defaults (RFC 9106 second recommended option).
async fn hash_password(password: String) -> Result<String, AuthError> {
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
            .map_err(|e| AuthError::Store(e.to_string()))
    })
    .await
    .map_err(|e| AuthError::Store(e.to_string()))?
}

async fn verify_password(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false)
}

fn check_password(p: &str) -> Result<(), AuthError> {
    let n = p.chars().count();
    if (MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&n) {
        Ok(())
    } else {
        Err(AuthError::WeakPassword)
    }
}

#[derive(Debug, Clone)]
pub struct SignUp {
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub password: String,
    /// Invite token (companion promotion or admin invite).
    pub invite: Option<String>,
}

/// TOTP enrolment data for the client (QR code from `otpauth_url`).
#[derive(Debug, Clone)]
pub struct TotpEnrolment {
    pub otpauth_url: String,
    pub secret_base32: String,
}

#[derive(Clone)]
pub struct Auth {
    app: App,
    store: Arc<dyn AuthStore>,
    clock: Arc<dyn Clock>,
    settings: AuthSettings,
    throttle: Arc<Throttle>,
}

impl Auth {
    #[must_use]
    pub fn new(app: App, store: Arc<dyn AuthStore>, settings: AuthSettings) -> Self {
        let clock = app.clock.clone();
        Self {
            app,
            store,
            clock,
            settings,
            throttle: Arc::default(),
        }
    }

    #[must_use]
    pub fn settings(&self) -> &AuthSettings {
        &self.settings
    }

    fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    /// Create an account. The very first account is always allowed and becomes admin;
    /// afterwards the sign-up mode decides (an invite always works unless sign-up is closed).
    ///
    /// # Errors
    /// Closed sign-up, invalid invite, weak password, taken username.
    pub async fn sign_up(&self, s: SignUp) -> Result<(User, SessionToken), AuthError> {
        check_password(&s.password)?;
        // usernames are lowercase; "Anna " signs up as "anna" (and signs in as either)
        let username = s.username.trim().to_lowercase();
        domain::user::validate_username(&username).map_err(|_| AuthError::InvalidUsername)?;
        let display_name = match s.display_name.trim() {
            "" => username.clone(),
            d => d.to_string(),
        };
        domain::companion::validate_companion_name(&display_name)
            .map_err(|_| AuthError::InvalidDisplayName)?;
        let first = self.app.user_count().await? == 0;
        let invite = if first { None } else { s.invite.as_deref() };
        if !first {
            match (self.settings.signup, invite) {
                (SignupMode::Closed, _) | (SignupMode::Invite, None) => {
                    return Err(AuthError::SignupClosed);
                }
                _ => {}
            }
        }
        // Hash first so a failure leaves no half-created account behind.
        let hash = hash_password(s.password.clone()).await?;
        let user = if let Some(token) = invite {
            self.app
                .accept_invite(token, &username, &display_name)
                .await
                .map_err(|e| match e {
                    AppError::NotFound(_) | AppError::Conflict(_) => AuthError::InvalidInvite,
                    e => AuthError::App(e),
                })?
        } else {
            let role = if first { Role::Admin } else { Role::Member };
            self.app
                .register_user(&username, &display_name, s.email.clone(), role)
                .await?
        };
        self.store
            .put_credentials(&Credentials {
                user_id: user.id,
                password_hash: Some(hash),
                totp_secret: None,
                totp_enabled: false,
                totp_last_step: None,
            })
            .await?;
        let token = self.create_session(user.id).await?;
        Ok((user, token))
    }

    /// Password (+ TOTP if enabled) sign-in.
    ///
    /// # Errors
    /// [`AuthError::InvalidCredentials`], [`AuthError::TotpRequired`],
    /// [`AuthError::InvalidTotp`].
    pub async fn sign_in(
        &self,
        username: &str,
        password: &str,
        totp: Option<&str>,
    ) -> Result<SessionToken, AuthError> {
        let key = username.trim().to_lowercase();
        self.throttle.check(&key, self.now())?;
        let result = self.sign_in_checked(username, password, totp).await;
        match &result {
            Err(AuthError::InvalidCredentials | AuthError::InvalidTotp) => {
                self.throttle.failed(&key, self.now());
            }
            Ok(_) => self.throttle.succeeded(&key),
            Err(_) => {}
        }
        result
    }

    async fn sign_in_checked(
        &self,
        username: &str,
        password: &str,
        totp: Option<&str>,
    ) -> Result<SessionToken, AuthError> {
        let user = self
            .app
            .user_by_username(&username.trim().to_lowercase())
            .await?;
        let creds = match &user {
            Some(u) => self.store.credentials(u.id).await?,
            None => None,
        };
        // Always run one verification so unknown users take as long as wrong passwords.
        let hash = creds
            .as_ref()
            .and_then(|c| c.password_hash.clone())
            .unwrap_or_else(|| DUMMY_HASH.to_string());
        let ok = verify_password(password.to_string(), hash).await;
        let (Some(user), Some(mut creds)) = (user, creds) else {
            return Err(AuthError::InvalidCredentials);
        };
        if !ok || creds.password_hash.is_none() {
            return Err(AuthError::InvalidCredentials);
        }
        if creds.totp_enabled {
            let code = totp.ok_or(AuthError::TotpRequired)?;
            self.check_totp(&mut creds, code, &user.username).await?;
        }
        self.create_session(user.id).await
    }

    async fn check_totp(
        &self,
        creds: &mut Credentials,
        code: &str,
        account: &str,
    ) -> Result<(), AuthError> {
        let secret = creds.totp_secret.clone().ok_or(AuthError::InvalidTotp)?;
        let totp = self.totp(secret, account)?;
        let now = u64::try_from(self.now().timestamp()).unwrap_or(0);
        let step = totp
            .check(code, now)
            .ok_or(AuthError::InvalidTotp)
            .map(|s| i64::try_from(s).unwrap_or(i64::MAX))?;
        // A code is valid once: reject replays of the same or an older step.
        if creds.totp_last_step.is_some_and(|last| step <= last) {
            return Err(AuthError::InvalidTotp);
        }
        creds.totp_last_step = Some(step);
        self.store.put_credentials(creds).await?;
        Ok(())
    }

    fn totp(&self, secret: Vec<u8>, account: &str) -> Result<totp_rs::Totp, AuthError> {
        totp_rs::Builder::new()
            .with_secret(secret)
            .with_account_name(account)
            .with_issuer(Some(self.settings.totp_issuer.as_str()))
            .build()
            .map_err(|e| AuthError::Store(format!("totp: {e}")))
    }

    async fn create_session(&self, user: UserId) -> Result<SessionToken, AuthError> {
        let token = URL_SAFE_NO_PAD.encode(random_bytes::<32>());
        let now = self.now();
        self.store
            .insert_session(&Session {
                token_hash: token_hash(&token),
                user_id: user,
                created_at: now,
                last_seen_at: now,
                expires_at: now + Duration::days(i64::from(self.settings.session_days)),
            })
            .await?;
        Ok(SessionToken(token))
    }

    /// Resolve a session token to the acting user. Sessions slide: each use (at most once
    /// an hour) extends the expiry.
    ///
    /// # Errors
    /// Storage errors (an unknown or expired token is `Ok(None)`).
    pub async fn session(&self, token: &str) -> Result<Option<Actor>, AuthError> {
        let hash = token_hash(token);
        let Some(mut s) = self.store.session(&hash).await? else {
            return Ok(None);
        };
        let now = self.now();
        if s.expires_at <= now {
            self.store.delete_session(&hash).await?;
            return Ok(None);
        }
        let Some(user) = self.app.user_opt(s.user_id).await? else {
            self.store.delete_session(&hash).await?;
            return Ok(None);
        };
        if now - s.last_seen_at > Duration::hours(1) {
            s.last_seen_at = now;
            s.expires_at = now + Duration::days(i64::from(self.settings.session_days));
            self.store.update_session(&s).await?;
        }
        Ok(Some(Actor {
            user_id: user.id,
            role: user.role,
        }))
    }

    /// # Errors
    /// Storage errors.
    pub async fn sign_out(&self, token: &str) -> Result<(), AuthError> {
        self.store.delete_session(&token_hash(token)).await?;
        Ok(())
    }

    /// Start TOTP enrolment: store a new secret (not yet active) and return it.
    ///
    /// # Errors
    /// Unknown user or storage errors.
    pub async fn totp_begin(&self, actor: Actor) -> Result<TotpEnrolment, AuthError> {
        let user = self.app.user(actor.user_id).await?;
        let mut creds = self.credentials_or_default(actor.user_id).await?;
        let secret = random_bytes::<20>().to_vec();
        let totp = self.totp(secret.clone(), &user.username)?;
        creds.totp_secret = Some(secret);
        creds.totp_enabled = false;
        creds.totp_last_step = None;
        self.store.put_credentials(&creds).await?;
        Ok(TotpEnrolment {
            otpauth_url: totp.to_url().map_err(|e| AuthError::Store(e.to_string()))?,
            secret_base32: totp_rs::Secret::from(creds.totp_secret.clone().unwrap_or_default())
                .to_base32(),
        })
    }

    /// Activate TOTP after the user proved they can generate codes.
    ///
    /// # Errors
    /// [`AuthError::InvalidTotp`] or storage errors.
    pub async fn totp_confirm(&self, actor: Actor, code: &str) -> Result<(), AuthError> {
        let user = self.app.user(actor.user_id).await?;
        let mut creds = self.credentials_or_default(actor.user_id).await?;
        self.check_totp(&mut creds, code, &user.username).await?;
        creds.totp_enabled = true;
        self.store.put_credentials(&creds).await?;
        Ok(())
    }

    async fn credentials_or_default(&self, user: UserId) -> Result<Credentials, AuthError> {
        Ok(self.store.credentials(user).await?.unwrap_or(Credentials {
            user_id: user,
            password_hash: None,
            totp_secret: None,
            totp_enabled: false,
            totp_last_step: None,
        }))
    }

    /// Change the password (signs out every other session).
    ///
    /// # Errors
    /// Wrong current password or a weak new one.
    pub async fn change_password(
        &self,
        actor: Actor,
        current: &str,
        new: &str,
    ) -> Result<SessionToken, AuthError> {
        check_password(new)?;
        let mut creds = self.credentials_or_default(actor.user_id).await?;
        if let Some(h) = &creds.password_hash
            && !verify_password(current.to_string(), h.clone()).await
        {
            return Err(AuthError::InvalidCredentials);
        }
        creds.password_hash = Some(hash_password(new.to_string()).await?);
        self.store.put_credentials(&creds).await?;
        self.store.delete_sessions_for(actor.user_id).await?;
        self.create_session(actor.user_id).await
    }

    #[must_use]
    pub fn store(&self) -> &Arc<dyn AuthStore> {
        &self.store
    }

    #[must_use]
    pub fn app(&self) -> &App {
        &self.app
    }
}

/// A valid argon2id hash of a random string, verified against for unknown users.
const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$Vr0CsUyoyGvOSxMmETYxf4ODmZ4H8sDJ5cUMk1HwKLA";

#[cfg(test)]
mod tests;
