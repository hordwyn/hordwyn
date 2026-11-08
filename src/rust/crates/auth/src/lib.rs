//! Passwords (argon2id), sessions, TOTP, OIDC and roles.
//!
//! Account rows belong to `app` (users + self-companion); this crate owns credentials,
//! sessions and OIDC identities. Session tokens are random 256-bit values; only their blake3
//! hash is stored. Password hashing runs on tokio's blocking pool, never on a worker thread.
#![deny(unsafe_code)]

pub mod admin;
pub mod admin_pg;
mod fresh;
pub mod oidc;
pub mod pg;
mod recovery;
pub mod secrets;
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
use contracts::auth::{
    AuthErrorCode, DISPLAY_NAME_MAX_LEN, UsernameAvailability, UsernameRule, check_username,
    normalize_display_name, normalize_username,
};
use domain::ids::UserId;
use domain::user::{Actor, Role, User};
use thiserror::Error;

pub use fresh::{
    OIDC_AUTH_TIME_MAX_AGE_SECS, PENDING_ACTION_MAX_BYTES, PENDING_ACTION_MINUTES,
    REAUTH_CAP_MINUTES, REAUTH_SLIDE_MINUTES, RECOVERY_CODES, ReauthStatus,
};
pub use recovery::PASSWORD_RESET_MINUTES;
pub use store::{
    AuthStore, Credentials, InviteStore, MemAuthStore, MemInviteStore, OidcIdentity, PasswordReset,
    RecoveryCode, Session, SessionSecret, SignupInvite,
};

/// Minimum password length (from the shared rules in `contracts::auth`).
pub const MIN_PASSWORD_LEN: usize = contracts::auth::PASSWORD_MIN_LEN;
pub const MAX_PASSWORD_LEN: usize = contracts::auth::PASSWORD_MAX_LEN;

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
    #[error("passwords need at least {MIN_PASSWORD_LEN} characters")]
    PasswordTooShort,
    #[error("passwords have at most {MAX_PASSWORD_LEN} characters")]
    PasswordTooLong,
    #[error("{}", username_rule_message(*.0))]
    InvalidUsername(UsernameRule),
    #[error("that username is taken")]
    UsernameTaken,
    #[error("display names have at most {DISPLAY_NAME_MAX_LEN} characters")]
    DisplayNameTooLong,
    #[error(transparent)]
    App(#[from] AppError),
    #[error("auth storage: {0}")]
    Store(String),
    #[error("oidc: {0}")]
    Oidc(String),
    /// Too many failed sign-ins for this name; try again later (G2-4).
    #[error("too many failed attempts, try again in {retry_after_secs} s")]
    TooManyAttempts { retry_after_secs: i64 },
    #[error("this account is disabled")]
    AccountDisabled,
    /// The authenticator seed is stored but its key is not loaded (addendum-1 2.2).
    #[error("the authenticator cannot be checked right now")]
    TotpLocked,
    /// The action needs a fresh proof (DEC-191).
    #[error("confirm it is you first")]
    ReauthRequired,
}

fn username_rule_message(r: UsernameRule) -> &'static str {
    match r {
        UsernameRule::TooShort => "usernames need at least 3 characters",
        UsernameRule::TooLong => "usernames have at most 32 characters",
        UsernameRule::Space => "usernames cannot contain spaces",
        UsernameRule::InvalidChars => "usernames use only a-z, 0-9 and _ . -",
    }
}

impl AuthError {
    /// Seconds until a throttled caller may try again ([`AuthError::TooManyAttempts`]).
    #[must_use]
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            Self::TooManyAttempts { retry_after_secs } => u64::try_from(*retry_after_secs).ok(),
            _ => None,
        }
    }

    /// The stable code of a sign-up / sign-in error (`None` for internal errors and
    /// wrapped `App` errors).
    #[must_use]
    pub fn code(&self) -> Option<AuthErrorCode> {
        Some(match self {
            Self::InvalidCredentials => AuthErrorCode::InvalidCredentials,
            Self::TotpRequired => AuthErrorCode::TotpRequired,
            Self::InvalidTotp => AuthErrorCode::InvalidTotp,
            Self::SignupClosed => AuthErrorCode::SignupClosed,
            Self::InvalidInvite => AuthErrorCode::InvalidInvite,
            Self::PasswordTooShort => AuthErrorCode::PasswordTooShort,
            Self::PasswordTooLong => AuthErrorCode::PasswordTooLong,
            Self::InvalidUsername(r) => r.error(),
            Self::UsernameTaken => AuthErrorCode::UsernameTaken,
            Self::DisplayNameTooLong => AuthErrorCode::DisplayNameTooLong,
            Self::TooManyAttempts { .. } => AuthErrorCode::TooManyAttempts,
            Self::AccountDisabled => AuthErrorCode::AccountDisabled,
            Self::TotpLocked => AuthErrorCode::TotpLocked,
            Self::ReauthRequired => AuthErrorCode::ReauthRequired,
            Self::App(_) | Self::Store(_) | Self::Oidc(_) => return None,
        })
    }
}

/// Username availability lookups allowed per minute for the whole instance (DEC-184): the
/// check needs no session, so a per-name limit would not stop enumeration.
pub const USERNAME_CHECKS_PER_MINUTE: u32 = 30;
/// Free variants checked at most for a taken name.
const SUGGESTION_TRIES: u32 = 10;

/// A fixed one-minute window counter, per process.
#[derive(Default)]
struct Window(std::sync::Mutex<Option<(DateTime<Utc>, u32)>>);

impl Window {
    fn take(&self, now: DateTime<Utc>, limit: u32) -> Result<(), AuthError> {
        let len = Duration::minutes(1);
        let mut w = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match w.as_mut() {
            Some((start, n)) if now < *start + len => {
                if *n >= limit {
                    return Err(AuthError::TooManyAttempts {
                        retry_after_secs: (*start + len - now).num_seconds().max(1),
                    });
                }
                *n += 1;
            }
            _ => *w = Some((now, 1)),
        }
        Ok(())
    }
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
    if n < MIN_PASSWORD_LEN {
        Err(AuthError::PasswordTooShort)
    } else if n > MAX_PASSWORD_LEN {
        Err(AuthError::PasswordTooLong)
    } else {
        Ok(())
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
    invites: Arc<dyn InviteStore>,
    clock: Arc<dyn Clock>,
    settings: Arc<std::sync::RwLock<AuthSettings>>,
    throttle: Arc<Throttle>,
    username_checks: Arc<Window>,
}

impl Auth {
    #[must_use]
    pub fn new(app: App, store: Arc<dyn AuthStore>, settings: AuthSettings) -> Self {
        let clock = app.clock.clone();
        Self {
            app,
            store,
            invites: Arc::new(MemInviteStore::default()),
            clock,
            settings: Arc::new(std::sync::RwLock::new(settings)),
            throttle: Arc::default(),
            username_checks: Arc::default(),
        }
    }

    /// The current settings (admins change sign-up mode and session length at run time).
    #[must_use]
    pub fn settings(&self) -> AuthSettings {
        self.settings
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Change the settings; sessions created from now on use them.
    pub fn update_settings(&self, f: impl FnOnce(&mut AuthSettings)) {
        f(&mut self
            .settings
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner));
    }

    fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    /// Create an account. The very first account is always allowed and becomes admin;
    /// afterwards the sign-up mode decides (an invite always works unless sign-up is closed).
    /// The username is normalised first (`contracts::auth::normalize_username`).
    ///
    /// # Errors
    /// One error per field rule (username, display name, password), then closed sign-up or
    /// an invalid invite, then a taken username.
    pub async fn sign_up(&self, s: SignUp) -> Result<(User, SessionToken), AuthError> {
        // usernames are lowercase; "Anna " signs up as "anna" (and signs in as either)
        let username = normalize_username(&s.username);
        check_username(&username).map_err(AuthError::InvalidUsername)?;
        let display_name = normalize_display_name(&s.display_name, &username);
        if display_name.chars().count() > DISPLAY_NAME_MAX_LEN {
            return Err(AuthError::DisplayNameTooLong);
        }
        check_password(&s.password)?;
        let first = self.signup_gate(s.invite.as_deref()).await?;
        let invite = if first { None } else { s.invite.as_deref() };
        // A taken name answers before the (costly) hash; the `Conflict` mapping below still
        // covers a lost race. Hash before creating so a failure leaves no half-created account.
        if self.app.user_by_username(&username).await?.is_some() {
            return Err(AuthError::UsernameTaken);
        }
        let hash = hash_password(s.password.clone()).await?;
        // an admin invite is claimed before the account exists, so two sign-ups racing on
        // one invite cannot both get in (UX2-S4); a failed creation gives it back
        let claimed = match invite {
            Some(t) => self.claim_signup_invite(t).await?,
            None => None,
        };
        let created = self
            .create_account(
                first,
                invite.filter(|_| claimed.is_none()),
                &username,
                &display_name,
                s.email.clone(),
            )
            .await;
        if let Some(id) = claimed {
            self.finish_signup_invite(id, created.as_ref().ok().map(|u| u.id))
                .await?;
        }
        let user = created?;
        self.store
            .put_credentials(&Credentials {
                password_hash: Some(hash),
                ..Credentials::empty(user.id)
            })
            .await?;
        let token = self.create_session(user.id, "password").await?;
        Ok((user, token))
    }

    /// The account itself: through a companion invite (`companion_invite`), else a plain
    /// account (admin for the first one).
    async fn create_account(
        &self,
        first: bool,
        companion_invite: Option<&str>,
        username: &str,
        display_name: &str,
        email: Option<String>,
    ) -> Result<User, AuthError> {
        if let Some(token) = companion_invite {
            return self
                .app
                .accept_invite(token, username, display_name)
                .await
                .map_err(|e| match e {
                    AppError::NotFound(_) | AppError::Conflict(_) => AuthError::InvalidInvite,
                    e => AuthError::App(e),
                });
        }
        // an admin invite (or none) creates a plain member account
        let role = if first { Role::Admin } else { Role::Member };
        self.app
            .register_user(username, display_name, email, role)
            .await
            .map_err(|e| match e {
                // lost a race for the name
                AppError::Conflict(_) => AuthError::UsernameTaken,
                e => AuthError::App(e),
            })
    }

    /// May an account be created now: the first account, open sign-up, or a valid invite.
    /// Returns whether it is the first account. Runs before anything reveals whether a
    /// username exists.
    async fn signup_gate(&self, invite: Option<&str>) -> Result<bool, AuthError> {
        if self.app.user_count().await? == 0 {
            return Ok(true);
        }
        match (self.settings().signup, invite) {
            (SignupMode::Closed, _) | (SignupMode::Invite, None) => Err(AuthError::SignupClosed),
            (_, Some(token)) => {
                if self.app.invite_is_valid(token).await?
                    || self.signup_invite_is_valid(token).await?
                {
                    Ok(false)
                } else {
                    Err(AuthError::InvalidInvite)
                }
            }
            (SignupMode::Open, None) => Ok(false),
        }
    }

    /// Is `raw` (normalised like [`Auth::sign_up`]) free? Answers only when sign-up is
    /// possible for the caller, so it reveals no more than a sign-up attempt; lookups are
    /// limited to [`USERNAME_CHECKS_PER_MINUTE`] per instance (DEC-184). A taken name comes
    /// with up to three free variants.
    ///
    /// # Errors
    /// [`AuthError::TooManyAttempts`], [`AuthError::SignupClosed`],
    /// [`AuthError::InvalidInvite`] or storage errors.
    pub async fn username_available(
        &self,
        raw: &str,
        invite: Option<&str>,
    ) -> Result<UsernameAvailability, AuthError> {
        let username = normalize_username(raw);
        if let Err(rule) = check_username(&username) {
            return Ok(UsernameAvailability::Invalid { username, rule });
        }
        // the gate first: a closed instance does no lookup and uses none of the budget
        self.signup_gate(invite).await?;
        self.username_checks
            .take(self.now(), USERNAME_CHECKS_PER_MINUTE)?;
        if self.app.user_by_username(&username).await?.is_none() {
            return Ok(UsernameAvailability::Available { username });
        }
        let mut suggestions = Vec::new();
        for n in 2..2 + SUGGESTION_TRIES {
            let suffix = n.to_string();
            let base: String = username
                .chars()
                .take(contracts::auth::USERNAME_MAX_LEN - suffix.len())
                .collect();
            let candidate = format!("{base}{suffix}");
            if check_username(&candidate).is_ok()
                && self.app.user_by_username(&candidate).await?.is_none()
            {
                suggestions.push(candidate);
                if suggestions.len() == 3 {
                    break;
                }
            }
        }
        Ok(UsernameAvailability::Taken {
            username,
            suggestions,
        })
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
        let key = normalize_username(username);
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
            .user_by_username(&normalize_username(username))
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
        if self.store.is_disabled(user.id).await? {
            return Err(AuthError::AccountDisabled);
        }
        let method = self.second_factor(&mut creds, totp, &user.username).await?;
        self.create_session(user.id, method).await
    }

    /// The TOTP step when the account has one (a recovery code also works); returns the
    /// proof method (`password` or `password+totp`).
    async fn second_factor(
        &self,
        creds: &mut Credentials,
        code: Option<&str>,
        account: &str,
    ) -> Result<&'static str, AuthError> {
        if !creds.totp_enabled {
            return Ok("password");
        }
        if creds.totp_locked {
            return Err(AuthError::TotpLocked);
        }
        let code = code.ok_or(AuthError::TotpRequired)?;
        if fresh::looks_like_recovery_code(code) {
            self.use_recovery(creds.user_id, code).await?;
        } else {
            self.check_totp(creds, code, account).await?;
        }
        Ok("password+totp")
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
            .with_issuer(Some(self.settings().totp_issuer))
            .build()
            .map_err(|e| AuthError::Store(format!("totp: {e}")))
    }

    /// A new session; signing in counts as a fresh proof made with `method` (DEC-191).
    async fn create_session(&self, user: UserId, method: &str) -> Result<SessionToken, AuthError> {
        self.create_session_from(user, method, None, None).await
    }

    /// A new session, signed in through `provider` (OP session `sid`) when given.
    pub(crate) async fn create_session_from(
        &self,
        user: UserId,
        method: &str,
        provider: Option<uuid::Uuid>,
        sid: Option<String>,
    ) -> Result<SessionToken, AuthError> {
        let token = URL_SAFE_NO_PAD.encode(random_bytes::<32>());
        let now = self.now();
        self.store
            .insert_session(&Session {
                token_hash: token_hash(&token),
                user_id: user,
                created_at: now,
                last_seen_at: now,
                expires_at: now + Duration::days(i64::from(self.settings().session_days)),
                reauth_at: Some(now),
                reauth_method: Some(method.to_string()),
                reauth_slide_at: None,
                provider_id: provider,
                oidc_sid: sid,
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
            s.expires_at = now + Duration::days(i64::from(self.settings().session_days));
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

    /// Activate TOTP after the user proved they can generate codes. Returns the new
    /// recovery codes (OD-08), shown once; only their hashes are stored.
    ///
    /// # Errors
    /// [`AuthError::InvalidTotp`] or storage errors.
    pub async fn totp_confirm(&self, actor: Actor, code: &str) -> Result<Vec<String>, AuthError> {
        let user = self.app.user(actor.user_id).await?;
        let mut creds = self.credentials_or_default(actor.user_id).await?;
        self.check_totp(&mut creds, code, &user.username).await?;
        creds.totp_enabled = true;
        self.store.put_credentials(&creds).await?;
        self.new_recovery_codes(actor.user_id).await
    }

    async fn credentials_or_default(&self, user: UserId) -> Result<Credentials, AuthError> {
        Ok(self
            .store
            .credentials(user)
            .await?
            .unwrap_or_else(|| Credentials::empty(user)))
    }

    /// Change the password (signs out every other session). Wrong current passwords count
    /// against the same per-name budget as sign-in (UX2-S6).
    ///
    /// # Errors
    /// Wrong current password, [`AuthError::TooManyAttempts`] or a weak new one.
    pub async fn change_password(
        &self,
        actor: Actor,
        current: &str,
        new: &str,
    ) -> Result<SessionToken, AuthError> {
        check_password(new)?;
        let key = self.app.user(actor.user_id).await?.username;
        self.throttle.check(&key, self.now())?;
        let mut creds = self.credentials_or_default(actor.user_id).await?;
        if let Some(h) = &creds.password_hash {
            if !verify_password(current.to_string(), h.clone()).await {
                self.throttle.failed(&key, self.now());
                return Err(AuthError::InvalidCredentials);
            }
            self.throttle.succeeded(&key);
        }
        creds.password_hash = Some(hash_password(new.to_string()).await?);
        self.store.put_credentials(&creds).await?;
        self.store.delete_sessions_for(actor.user_id).await?;
        // entering the current password is a fresh proof (DEC-191)
        self.create_session(actor.user_id, "password").await
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
