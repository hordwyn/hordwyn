//! Account resolution after a verified sign-in (addendum-1 4.2 "Account resolution" and
//! "Group and claim to role mapping"):
//!
//! 1. `(issuer, sub)` known -> that account (OIDC Core 5.7: only `iss` + `sub`
//!    identify a person). Email is never synced (OD-21).
//! 2. Unknown, flow in mode `link` from a signed-in browser -> linked to that account.
//! 3. Unknown, a local account has the provider's verified email -> "Is this your
//!    account?": that account's password once, never an automatic link (OD-15).
//! 4. Unknown, no match -> a new account when this is the first one, the provider has
//!    `auto_create`, or sign-up is open; the username is the sanitised
//!    `preferred_username` with a numeric suffix on collision.
//!
//! `allowed_groups` refuse before any of this; `role_source = provider` recomputes the role
//! at each sign-in (OD-16), never demoting the last admin.

use chrono::Duration;
use contracts::auth::{DISPLAY_NAME_MAX_LEN, USERNAME_MAX_LEN, check_username, normalize_username};
use domain::ids::UserId;
use domain::user::{Role, User};
use serde_json::{Value, json};
use uuid::Uuid;

use super::cookie::{self, Mode, PENDING_MINUTES, PendingLink, PendingLogin};
use super::provider::{Provider, RoleSource};
use super::{Failure, FailureCode, LINK_KIND, Oidc};
use crate::{Auth, AuthError, Credentials, OidcIdentity, SessionToken};

/// A sign-in whose ID token passed validation.
pub(crate) struct Verified {
    pub provider: Provider,
    pub issuer: String,
    pub subject: String,
    /// The ID token's claims.
    pub claims: Value,
    /// From the ID token, else `UserInfo`; `None` when neither had the claim.
    pub groups: Option<Vec<String>>,
    pub sid: Option<String>,
    pub claims_hash: String,
    /// `auth_time` (Core 2), required after `max_age` (re-auth, the `max_age` probe).
    pub auth_time: Option<chrono::DateTime<chrono::Utc>>,
    /// The compact ID token, kept per session for `id_token_hint` at logout.
    pub id_token: String,
}

/// What the callback leads to.
#[derive(Debug)]
pub enum Outcome {
    SignedIn {
        token: SessionToken,
        return_to: String,
        /// The `oidc_hint` cookie value for the next silent sign-in.
        hint: Option<String>,
    },
    /// Silent sign-in needs the person: send the browser to this interactive request.
    Restart(super::Started),
    /// A provider Test probe finished (`silent_ok`, `silent_login_required`,
    /// `silent_unsupported`, `max_age_ok`, `max_age_ignored`, `max_age_error`).
    Probe { return_to: String, result: String },
    /// Re-authentication confirmed; the session id was regenerated.
    Reauthed {
        token: SessionToken,
        return_to: String,
    },
    /// Link mode: the identity is now linked to the signed-in account.
    Linked { return_to: String },
    /// Step 3: show "Is this your account?" with this signed ticket (a cookie).
    ConfirmLink { ticket: String },
}

/// Why "Is this your account?" did not link.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    /// Unknown, expired or used ticket.
    #[error("the link request expired, sign in again")]
    Ticket,
    /// The identity was linked to another account meanwhile.
    #[error("this sign-in is linked to another account")]
    InUse,
    /// No emailed link for this account: it has a password, no stored address, or mail is
    /// not set up.
    #[error("no link can be emailed for this account")]
    NoEmail,
    /// The emailed code is unknown, used, expired, or was asked for in another browser.
    #[error("this link does not work (any more)")]
    Code,
    /// Too many links asked for; try again in this many seconds.
    #[error("too many links asked for")]
    TooMany { retry_after_secs: i64 },
    #[error(transparent)]
    Auth(#[from] AuthError),
}

/// The groups claim: an array of strings, or one string.
pub(crate) fn groups(claims: &Value, name: &str) -> Option<Vec<String>> {
    match claims.get(name)? {
        Value::Array(a) => Some(
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect(),
        ),
        Value::String(s) => Some(vec![s.clone()]),
        _ => None,
    }
}

/// blake3 of the claims an account cares about (to notice changes, never to identify).
pub(crate) fn claims_hash(claims: &Value, groups: Option<&[String]>) -> String {
    let picked = json!({
        "sub": claims["sub"],
        "email": claims["email"],
        "email_verified": claims["email_verified"],
        "name": claims["name"],
        "preferred_username": claims["preferred_username"],
        "groups": groups,
    });
    blake3::hash(picked.to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// Map a provider username onto our rules (`[a-z0-9_.-]`, 3..=32).
#[must_use]
pub fn sanitize_username(u: &str) -> String {
    normalize_username(u)
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .take(USERNAME_MAX_LEN)
        .collect()
}

fn in_any(groups: Option<&[String]>, wanted: &[String]) -> bool {
    groups.is_some_and(|g| g.iter().any(|x| wanted.contains(x)))
}

/// The role a provider with `role_source = provider` gives (`None`: roles are local).
fn provider_role(p: &Provider, groups: Option<&[String]>) -> Option<Role> {
    (p.role_source == RoleSource::Provider).then(|| {
        if in_any(groups, &p.admin_groups) {
            Role::Admin
        } else {
            Role::Member
        }
    })
}

impl Oidc {
    pub(crate) async fn resolve(
        &self,
        auth: &Auth,
        v: Verified,
        pending: &PendingLogin,
        session: Option<UserId>,
    ) -> Result<Outcome, Failure> {
        let p = &v.provider;
        if !p.allowed_groups.is_empty() && !in_any(v.groups.as_deref(), &p.allowed_groups) {
            return Err(Failure::new(
                FailureCode::GroupRule,
                "not in an allowed group",
            ));
        }
        let known = auth.store.identity(&v.issuer, &v.subject).await?;
        let mut identity = match (known, pending.mode) {
            (Some(i), Mode::Link) => {
                if Some(i.user_id.0) != pending.user {
                    return Err(Failure::new(FailureCode::IdentityInUse, "linked elsewhere"));
                }
                i
            }
            (Some(i), _) => i,
            (None, Mode::Link) => return self.link_signed_in(auth, &v, pending, session).await,
            (None, _) => return self.unknown(auth, v, pending).await,
        };
        identity.provider_id = Some(p.id);
        identity.sid.clone_from(&v.sid);
        identity.last_claims_hash = Some(v.claims_hash.clone());
        identity.last_groups.clone_from(&v.groups);
        auth.store.touch_identity(&identity).await?;
        if pending.mode == Mode::Link {
            return Ok(Outcome::Linked {
                return_to: pending.return_to.clone(),
            });
        }
        self.sign_in(auth, identity.user_id, &v, pending).await
    }

    /// Step 1 (and after linking): sign in `user`, applying a provider-managed role.
    async fn sign_in(
        &self,
        auth: &Auth,
        user: UserId,
        v: &Verified,
        pending: &PendingLogin,
    ) -> Result<Outcome, Failure> {
        if auth.store.is_disabled(user).await? {
            return Err(Failure::new(FailureCode::AccountDisabled, "disabled"));
        }
        let account = auth.app.user(user).await.map_err(AuthError::from)?;
        self.apply_role(auth, &account, v).await?;
        let token = auth
            .create_session_from(
                user,
                &format!("oidc:{}", v.provider.slug),
                Some(v.provider.id),
                v.sid.clone(),
            )
            .await?;
        self.keep_id_token(auth, &token, &v.id_token).await;
        Ok(Outcome::SignedIn {
            token,
            return_to: pending.return_to.clone(),
            hint: Some(self.hint(v)),
        })
    }

    /// OD-16: recompute the role from the groups; the last admin who can sign in is never
    /// demoted (addendum-1 3.3), the change is audited as the system.
    async fn apply_role(&self, auth: &Auth, user: &User, v: &Verified) -> Result<(), Failure> {
        let Some(want) = provider_role(&v.provider, v.groups.as_deref()) else {
            return Ok(());
        };
        if want == user.role {
            return Ok(());
        }
        if want == Role::Member && !other_admin(auth, user.id).await? {
            tracing::warn!(user = %user.username, "provider role rule would demote the last admin; kept");
            return Ok(());
        }
        auth.app
            .set_role_unchecked(user.id, want)
            .await
            .map_err(AuthError::from)?;
        if let Some(store) = self.audit_store() {
            let role = |r: Role| if r == Role::Admin { "admin" } else { "member" };
            let e = crate::admin::AuditEntry {
                id: Uuid::now_v7(),
                at: self.clock().now(),
                actor_id: None,
                actor_kind: crate::admin::ActorKind::System,
                actor_name: None,
                action: "role.changed".into(),
                target_type: Some("user".into()),
                target_id: Some(user.id.0.to_string()),
                detail: json!({"from": role(user.role), "to": role(want), "via": "provider",
                               "provider": v.provider.slug}),
                client_addr: None,
                request_id: None,
            };
            if let Err(err) = store.audit(&e).await {
                tracing::error!(error = %err, "audit write failed");
            }
        }
        Ok(())
    }

    /// Step 2: mode `link`, started by the signed-in account that still owns this browser.
    async fn link_signed_in(
        &self,
        auth: &Auth,
        v: &Verified,
        pending: &PendingLogin,
        session: Option<UserId>,
    ) -> Result<Outcome, Failure> {
        let Some(user) = pending.user.map(UserId).filter(|u| session == Some(*u)) else {
            return Err(Failure::failed("link without the signed-in account"));
        };
        self.link(auth, user, v).await?;
        Ok(Outcome::Linked {
            return_to: pending.return_to.clone(),
        })
    }

    async fn link(&self, auth: &Auth, user: UserId, v: &Verified) -> Result<(), Failure> {
        let mut i = OidcIdentity::new(&v.issuer, &v.subject, user, self.clock().now());
        i.provider_id = Some(v.provider.id);
        i.sid.clone_from(&v.sid);
        i.last_claims_hash = Some(v.claims_hash.clone());
        i.last_groups.clone_from(&v.groups);
        if !auth.store.link_identity(&i).await? {
            return Err(Failure::new(FailureCode::IdentityInUse, "lost a race"));
        }
        Ok(())
    }

    /// Steps 3 and 4 for an identity nobody has.
    async fn unknown(
        &self,
        auth: &Auth,
        v: Verified,
        pending: &PendingLogin,
    ) -> Result<Outcome, Failure> {
        let verified = v.claims["email_verified"].as_bool() == Some(true);
        if verified && let Some(email) = v.claims["email"].as_str() {
            let users = auth.app.list_users().await.map_err(AuthError::from)?;
            if let Some(u) = users
                .iter()
                .filter(|u| {
                    u.email
                        .as_deref()
                        .is_some_and(|e| e.eq_ignore_ascii_case(email))
                })
                .min_by_key(|u| u.created_at)
            {
                return Ok(Outcome::ConfirmLink {
                    ticket: self.link_ticket(&v, u.id, pending),
                });
            }
        }
        let first = auth.app.user_count().await.map_err(AuthError::from)? == 0;
        if !first && !v.provider.auto_create && auth.settings().signup != crate::SignupMode::Open {
            return Err(Failure::new(FailureCode::NoAccount, "no account"));
        }
        let user = self.create(auth, &v, first).await?;
        self.link(auth, user.id, &v).await?;
        let token = auth
            .create_session_from(
                user.id,
                &format!("oidc:{}", v.provider.slug),
                Some(v.provider.id),
                v.sid.clone(),
            )
            .await?;
        self.keep_id_token(auth, &token, &v.id_token).await;
        Ok(Outcome::SignedIn {
            token,
            return_to: pending.return_to.clone(),
            hint: Some(self.hint(&v)),
        })
    }

    fn link_ticket(&self, v: &Verified, user: UserId, pending: &PendingLogin) -> String {
        let ticket = PendingLink {
            id: base64::Engine::encode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                crate::random_bytes::<16>(),
            ),
            provider: v.provider.id,
            issuer: v.issuer.clone(),
            subject: v.subject.clone(),
            user: user.0,
            sid: v.sid.clone(),
            groups: v.groups.clone(),
            claims_hash: v.claims_hash.clone(),
            return_to: pending.return_to.clone(),
        };
        let exp = (self.clock().now() + Duration::minutes(PENDING_MINUTES)).timestamp();
        cookie::seal(self.keys(), LINK_KIND, &ticket, exp)
    }

    /// Step 4: a new account named after `preferred_username`.
    async fn create(&self, auth: &Auth, v: &Verified, first: bool) -> Result<User, Failure> {
        let base = v.claims["preferred_username"]
            .as_str()
            .map(sanitize_username)
            .filter(|u| check_username(u).is_ok())
            .unwrap_or_else(|| {
                format!("user-{}", &blake3::hash(v.subject.as_bytes()).to_hex()[..8])
            });
        let username = free_username(auth, &base).await?;
        let display: String = v.claims["name"]
            .as_str()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or(&username)
            .chars()
            .take(DISPLAY_NAME_MAX_LEN)
            .collect();
        // only a verified address is kept (Core 5.1 `email_verified`)
        let email = (v.claims["email_verified"].as_bool() == Some(true))
            .then(|| v.claims["email"].as_str().map(String::from))
            .flatten();
        let role = if first {
            Role::Admin
        } else {
            provider_role(&v.provider, v.groups.as_deref()).unwrap_or(Role::Member)
        };
        let user = auth
            .app
            .register_user(&username, &display, email, role)
            .await
            .map_err(AuthError::from)?;
        auth.store
            .put_credentials(&Credentials::empty(user.id))
            .await?;
        Ok(user)
    }

    /// "Is this your account?": the account and provider of a ticket.
    ///
    /// # Errors
    /// [`LinkError::Ticket`].
    pub async fn link_pending(
        &self,
        auth: &Auth,
        ticket: Option<&str>,
    ) -> Result<(PendingLink, User, Provider, bool), LinkError> {
        let t: PendingLink = cookie::open(
            self.keys(),
            LINK_KIND,
            ticket.ok_or(LinkError::Ticket)?,
            self.clock().now().timestamp(),
        )
        .map_err(|_| LinkError::Ticket)?;
        let provider = self
            .store()
            .provider(t.provider)
            .await
            .map_err(AuthError::from)?
            .filter(|p| p.enabled)
            .ok_or(LinkError::Ticket)?;
        let user = auth
            .app
            .user(UserId(t.user))
            .await
            .map_err(AuthError::from)?;
        let has_password = auth
            .store
            .credentials(user.id)
            .await
            .map_err(AuthError::from)?
            .is_some_and(|c| c.password_hash.is_some());
        Ok((t, user, provider, has_password))
    }

    /// Confirm with the account's password (and TOTP when enrolled, through the normal
    /// throttled sign-in), then link and stay signed in. The ticket works once.
    ///
    /// # Errors
    /// [`LinkError`]; wrong passwords are [`AuthError::InvalidCredentials`].
    pub async fn confirm_link(
        &self,
        auth: &Auth,
        ticket: Option<&str>,
        password: &str,
        totp: Option<&str>,
    ) -> Result<(SessionToken, String), LinkError> {
        let (t, user, provider, _) = self.link_pending(auth, ticket).await?;
        let token = auth.sign_in(&user.username, password, totp).await?;
        let now = self.clock().now();
        let once_key = cookie::used_key(LINK_KIND, &t.id);
        let fresh = self
            .store()
            .claim_once(&once_key, now + Duration::minutes(PENDING_MINUTES + 1), now)
            .await
            .map_err(AuthError::from)?;
        let mut i = OidcIdentity::new(&t.issuer, &t.subject, user.id, now);
        i.provider_id = Some(provider.id);
        i.sid = t.sid;
        i.last_claims_hash = Some(t.claims_hash);
        i.last_groups = t.groups;
        let linked = fresh
            && auth
                .store
                .link_identity(&i)
                .await
                .map_err(AuthError::from)?;
        if !linked {
            auth.sign_out(&token.0).await?;
            return Err(if fresh {
                LinkError::InUse
            } else {
                LinkError::Ticket
            });
        }
        Ok((token, t.return_to))
    }
}

/// Is there an admin who can sign in other than `except`?
async fn other_admin(auth: &Auth, except: UserId) -> Result<bool, Failure> {
    for u in auth.app.list_users().await.map_err(AuthError::from)? {
        if u.role == Role::Admin && u.id != except && !auth.store.is_disabled(u.id).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `base`, else `base2`, `base3`, ... (shortened to fit 32 characters).
async fn free_username(auth: &Auth, base: &str) -> Result<String, Failure> {
    if auth
        .app
        .user_by_username(base)
        .await
        .map_err(AuthError::from)?
        .is_none()
    {
        return Ok(base.to_string());
    }
    for n in 2..1000u32 {
        let suffix = n.to_string();
        let stem: String = base.chars().take(USERNAME_MAX_LEN - suffix.len()).collect();
        let candidate = format!("{stem}{suffix}");
        if check_username(&candidate).is_ok()
            && auth
                .app
                .user_by_username(&candidate)
                .await
                .map_err(AuthError::from)?
                .is_none()
        {
            return Ok(candidate);
        }
    }
    Err(Failure::failed("no free username"))
}
