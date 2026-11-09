//! Admin > Sign-in and SSO, server half (addendum-1 4.2; phase 3 WP3-2): OIDC providers with
//! a write-only, encrypted client secret (purpose `oidc_client_secret`), re-auth gated
//! changes (DEC-191) audited in the change's transaction (addendum-1 3.4), the provider
//! Test, the environment provider shown read-only, the Overview row from stored fields
//! (DEC-285) and the role lock for provider-managed roles (OD-16).

use auth::oidc::{
    DEFAULT_SCOPES, ENV_PROVIDER_ID, Oidc, Provenance, Provider, RoleSource, SECRET_PURPOSE,
    SecretChange, issuer_allowed,
};
use contracts::admin::{
    HealthRowDto, LinkedIdentityDto, OidcAdminDto, OidcButtonDto, OidcCapabilitiesDto,
    OidcProviderDto, OidcProviderInputDto, OidcSecretDto, OidcTestDto,
};
use domain::ids::UserId;
use domain::user::Actor;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{
    Caller, admin, entry, fresh, invalid, parse_uuid, pe, proof, refused, require_admin, row, ts,
};
use crate::error::ApiResult;
use crate::{ApiError, ApiState};

/// The variable family of the environment provider.
pub const ENV_VAR: &str = "APP__AUTH__OIDC";

pub(crate) fn oidc(s: &ApiState) -> ApiResult<&Oidc> {
    s.oidc.as_deref().ok_or_else(|| {
        ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            "OIDC is not enabled",
        )
    })
}

fn capabilities(p: &Provider) -> OidcCapabilitiesDto {
    let c = &p.capabilities;
    OidcCapabilitiesDto {
        end_session: c.end_session_endpoint.is_some(),
        backchannel_logout: c.backchannel_logout_supported,
        backchannel_logout_session: c.backchannel_logout_session_supported,
        frontchannel_logout: c.frontchannel_logout_supported,
        pkce_s256: c
            .code_challenge_methods_supported
            .iter()
            .any(|m| m == "S256"),
        iss_parameter: c.authorization_response_iss_parameter_supported,
    }
}

fn dto(o: &Oidc, p: &Provider) -> OidcProviderDto {
    OidcProviderDto {
        id: p.id.to_string(),
        slug: p.slug.clone(),
        display_name: p.display_name.clone(),
        issuer: p.issuer.clone(),
        client_id: p.client_id.clone(),
        secret_set: o.secret_set(p),
        secret_locked: matches!(p.client_secret, auth::oidc::ClientSecret::Locked),
        scopes: p.scopes.clone(),
        enabled: p.enabled,
        is_default: p.is_default,
        auto_create: p.auto_create,
        groups_claim: p.groups_claim.clone(),
        allowed_groups: p.allowed_groups.clone(),
        admin_groups: p.admin_groups.clone(),
        role_source: p.role_source.as_str().into(),
        provenance: p.provenance.as_str().into(),
        env_var: (p.provenance == Provenance::Env).then(|| ENV_VAR.to_string()),
        capabilities: capabilities(p),
        discovery_fetched_at: p.discovery_fetched_at.map(ts),
        last_test_at: p.last_test_at.map(ts),
        last_test: p
            .last_test
            .clone()
            .and_then(|v| serde_json::from_value(v).ok()),
        last_callback_at: p.last_callback_at.map(ts),
        last_callback_result: p.last_callback_result.clone(),
        linked_accounts: 0,
    }
}

/// Admin > Sign-in and SSO: providers and the URIs to register.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn oidc_admin(s: &ApiState, actor: Actor) -> ApiResult<OidcAdminDto> {
    require_admin(actor)?;
    let o = oidc(s)?;
    let providers = o.store().providers().await.map_err(pe)?;
    let base = o.public_url();
    let counts = s.auth.store().identity_counts().await.map_err(pe)?;
    let linked = |id: Uuid| counts.iter().find(|(p, _)| *p == id).map_or(0, |(_, n)| *n);
    Ok(OidcAdminDto {
        providers: providers
            .iter()
            .map(|p| OidcProviderDto {
                linked_accounts: linked(p.id),
                ..dto(o, p)
            })
            .collect(),
        redirect_uri: o.redirect_uri(),
        post_logout_redirect_uri: format!("{base}/signed-out"),
        backchannel_logout_uri: format!("{base}/api/v1/auth/oidc/backchannel-logout"),
    })
}

/// The sign-in page's buttons: enabled providers (public).
///
/// # Errors
/// Storage errors.
pub async fn oidc_buttons(s: &ApiState) -> ApiResult<Vec<OidcButtonDto>> {
    let Some(o) = s.oidc.as_deref() else {
        return Ok(Vec::new());
    };
    Ok(o.store()
        .providers()
        .await
        .map_err(pe)?
        .into_iter()
        .filter(|p| p.enabled)
        .map(|p| OidcButtonDto {
            slug: p.slug,
            display_name: p.display_name,
            is_default: p.is_default,
        })
        .collect())
}

fn valid_slug(s: &str) -> bool {
    let mut c = s.chars();
    s.len() <= 32
        && c.next()
            .is_some_and(|f| f.is_ascii_lowercase() || f.is_ascii_digit())
        && c.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '-')
}

fn list(v: &[String]) -> Vec<String> {
    let mut out: Vec<String> = v
        .iter()
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty())
        .collect();
    out.dedup();
    out
}

/// The input as a provider (`base` keeps id, provenance and timestamps).
fn apply(o: &Oidc, base: &mut Provider, b: &OidcProviderInputDto) -> ApiResult<()> {
    let slug = b.slug.trim();
    if !valid_slug(slug) {
        return Err(invalid(
            "invalid_slug",
            "a-z, 0-9 and -, at most 32 characters",
        ));
    }
    let name = b.display_name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(invalid(
            "invalid_setting",
            "the display name has 1 to 64 characters",
        ));
    }
    let issuer = b.issuer.trim();
    if openidconnect_url(issuer).is_none() {
        return Err(invalid("invalid_issuer", "the issuer is not a URL"));
    }
    if !issuer_allowed(issuer, o.public_url()) {
        return Err(invalid(
            "insecure_issuer",
            "use https (plain http only for a loopback issuer, or while this instance runs on http)",
        ));
    }
    if b.client_id.trim().is_empty() {
        return Err(invalid("invalid_setting", "the client id is missing"));
    }
    let role_source = match b.role_source.as_deref() {
        None => RoleSource::Local,
        Some(r) => RoleSource::parse(r)
            .ok_or_else(|| invalid("invalid_setting", "role_source is local or provider"))?,
    };
    let admin_groups = list(&b.admin_groups);
    if role_source == RoleSource::Provider && admin_groups.is_empty() {
        return Err(invalid(
            "admin_groups_required",
            "provider-managed roles need at least one admin group (OD-16)",
        ));
    }
    let mut scopes = vec!["openid".to_string()];
    let given = b
        .scopes
        .clone()
        .unwrap_or_else(|| DEFAULT_SCOPES.iter().map(|s| (*s).to_string()).collect());
    for sc in given {
        let sc = sc.trim().to_string();
        if sc.is_empty() || sc.contains(char::is_whitespace) {
            return Err(invalid("invalid_setting", "a scope is one word"));
        }
        if !scopes.contains(&sc) {
            scopes.push(sc);
        }
    }
    base.slug = slug.into();
    base.display_name = name.into();
    base.issuer = issuer.into();
    base.client_id = b.client_id.trim().into();
    base.scopes = scopes;
    base.enabled = b.enabled;
    base.is_default = b.is_default;
    base.auto_create = b.auto_create;
    base.groups_claim = b
        .groups_claim
        .as_deref()
        .map(str::trim)
        .filter(|g| !g.is_empty())
        .unwrap_or("groups")
        .into();
    base.allowed_groups = list(&b.allowed_groups);
    base.admin_groups = admin_groups;
    base.role_source = role_source;
    Ok(())
}

fn openidconnect_url(s: &str) -> Option<()> {
    url::Url::parse(s)
        .ok()
        .filter(|u| u.host_str().is_some())
        .map(|_| ())
}

/// addendum-1 3.3: a provider-role rule must not demote the acting admin. Their last
/// sign-in through this provider tells which groups they are in.
async fn guard_self_demotion(s: &ApiState, me: UserId, p: &Provider) -> ApiResult<()> {
    if p.role_source != RoleSource::Provider {
        return Ok(());
    }
    for i in s.auth.store().identities_of(me).await.map_err(pe)? {
        let admin = i
            .last_groups
            .as_deref()
            .is_some_and(|g| g.iter().any(|x| p.admin_groups.contains(x)));
        if i.provider_id == Some(p.id) && !admin {
            return Err(refused(
                "self_demotion",
                "this rule would take away your own admin role",
            ));
        }
    }
    Ok(())
}

fn summary(p: &Provider) -> Value {
    json!({"slug": p.slug, "issuer": p.issuer, "client_id": p.client_id,
           "enabled": p.enabled, "is_default": p.is_default, "auto_create": p.auto_create,
           "allowed_groups": p.allowed_groups, "admin_groups": p.admin_groups,
           "role_source": p.role_source.as_str()})
}

async fn secret_entry(
    s: &ApiState,
    actor: Actor,
    p: &Provider,
    set: bool,
) -> ApiResult<auth::admin::AuditEntry> {
    let a = admin(s)?;
    let detail = if set {
        json!({"purpose": SECRET_PURPOSE, "key_version": a.keys.active(), "provider": p.slug})
    } else {
        json!({"purpose": SECRET_PURPOSE, "provider": p.slug})
    };
    let action = if set {
        "secret.replaced"
    } else {
        "secret.cleared"
    };
    Ok(entry(
        s,
        Some(actor),
        action,
        Some(("oidc_provider", p.id.to_string())),
        detail,
    )
    .await)
}

fn slug_taken() -> ApiError {
    refused("slug_taken", "another provider uses this short name")
}

/// Add a provider (gated, audited with the secret in one transaction).
///
/// # Errors
/// The validation codes, `slug_taken`, `reauth_required`.
pub async fn create_provider(
    s: &ApiState,
    c: Caller<'_>,
    input: OidcProviderInputDto,
) -> ApiResult<OidcProviderDto> {
    require_admin(c.actor)?;
    let o = oidc(s)?;
    let now = s.app.clock.now();
    let mut p = Provider::new(Uuid::now_v7(), "", "", "", now);
    apply(o, &mut p, &input)?;
    p.created_by = Some(c.actor.user_id);
    p.updated_by = Some(c.actor.user_id);
    fresh(s, c).await?;
    let secret = input.client_secret.as_deref().filter(|x| !x.is_empty());
    let mut detail = summary(&p);
    detail["proof"] = proof(s, c.token).await;
    let mut rows = vec![
        entry(
            s,
            Some(c.actor),
            "oidc_provider.created",
            Some(("oidc_provider", p.id.to_string())),
            detail,
        )
        .await,
    ];
    if secret.is_some() {
        rows.push(secret_entry(s, c.actor, &p, true).await?);
    }
    let change = secret.map_or(SecretChange::Keep, SecretChange::Set);
    if !o.store().save(&p, change, &rows).await.map_err(pe)? {
        return Err(slug_taken());
    }
    o.changed(p.id).await;
    super::probes::refresh_probe(s, "identity_provider").await;
    provider_dto(o, p.id).await
}

async fn provider_dto(o: &Oidc, id: Uuid) -> ApiResult<OidcProviderDto> {
    let p = o
        .store()
        .provider(id)
        .await
        .map_err(pe)?
        .ok_or_else(not_found)?;
    Ok(dto(o, &p))
}

fn not_found() -> ApiError {
    ApiError::new(
        axum::http::StatusCode::NOT_FOUND,
        "not_found",
        "provider not found",
    )
}

async fn editable(o: &Oidc, id: &str) -> ApiResult<Provider> {
    let p = o
        .store()
        .provider(parse_uuid(id, "provider")?)
        .await
        .map_err(pe)?
        .ok_or_else(not_found)?;
    if p.provenance == Provenance::Env || p.id == ENV_PROVIDER_ID {
        return Err(refused(
            "fixed_by_config",
            "this provider is set by the configuration (APP__AUTH__OIDC)",
        ));
    }
    Ok(p)
}

/// Change a provider (gated, audited). The secret is changed separately.
///
/// # Errors
/// `fixed_by_config`, the validation codes, `self_demotion`, `slug_taken`,
/// `reauth_required`.
pub async fn update_provider(
    s: &ApiState,
    c: Caller<'_>,
    id: &str,
    input: OidcProviderInputDto,
) -> ApiResult<OidcProviderDto> {
    require_admin(c.actor)?;
    let o = oidc(s)?;
    let before = editable(o, id).await?;
    let mut p = before.clone();
    apply(o, &mut p, &input)?;
    guard_self_demotion(s, c.actor.user_id, &p).await?;
    fresh(s, c).await?;
    p.updated_by = Some(c.actor.user_id);
    p.updated_at = s.app.clock.now();
    let detail = json!({"before": summary(&before), "after": summary(&p),
                        "proof": proof(s, c.token).await});
    let row = entry(
        s,
        Some(c.actor),
        "oidc_provider.updated",
        Some(("oidc_provider", p.id.to_string())),
        detail,
    )
    .await;
    if !o
        .store()
        .save(&p, SecretChange::Keep, &[row])
        .await
        .map_err(pe)?
    {
        return Err(slug_taken());
    }
    o.changed(p.id).await;
    super::probes::refresh_probe(s, "identity_provider").await;
    provider_dto(o, p.id).await
}

/// Replace or (with `null`) remove the client secret: encrypted, never returned (gated).
///
/// # Errors
/// `fixed_by_config`, `reauth_required`.
pub async fn set_provider_secret(
    s: &ApiState,
    c: Caller<'_>,
    id: &str,
    input: OidcSecretDto,
) -> ApiResult<OidcProviderDto> {
    require_admin(c.actor)?;
    let o = oidc(s)?;
    let p = editable(o, id).await?;
    fresh(s, c).await?;
    let secret = input.secret.as_deref().filter(|x| !x.is_empty());
    let row = secret_entry(s, c.actor, &p, secret.is_some()).await?;
    let change = secret.map_or(SecretChange::Clear, SecretChange::Set);
    o.store().save(&p, change, &[row]).await.map_err(pe)?;
    o.changed(p.id).await;
    super::probes::refresh_probe(s, "identity_provider").await;
    provider_dto(o, p.id).await
}

/// Remove a provider (gated, audited). Refused when the acting admin would have no way
/// left to sign in.
///
/// # Errors
/// `fixed_by_config`, `last_sign_in_method`, `reauth_required`.
pub async fn delete_provider(s: &ApiState, c: Caller<'_>, id: &str) -> ApiResult<()> {
    require_admin(c.actor)?;
    let o = oidc(s)?;
    let p = editable(o, id).await?;
    let me = c.actor.user_id;
    let store = s.auth.store();
    let has_password = store
        .credentials(me)
        .await
        .map_err(pe)?
        .is_some_and(|x| x.password_hash.is_some());
    let others = store
        .identities_of(me)
        .await
        .map_err(pe)?
        .iter()
        .filter(|i| i.provider_id != Some(p.id))
        .count();
    if !has_password && others == 0 {
        return Err(refused(
            "last_sign_in_method",
            "you sign in only through this provider",
        ));
    }
    fresh(s, c).await?;
    let mut detail = summary(&p);
    detail["proof"] = proof(s, c.token).await;
    let row = entry(
        s,
        Some(c.actor),
        "oidc_provider.deleted",
        Some(("oidc_provider", p.id.to_string())),
        detail,
    )
    .await;
    o.store().delete(p.id, &[row]).await.map_err(pe)?;
    o.changed(p.id).await;
    super::probes::refresh_probe(s, "identity_provider").await;
    Ok(())
}

/// Test: a fresh discovery and what it advertises, and whether the testing admin's last
/// sign-in through this provider carried the groups claim; recorded for the overview.
///
/// # Errors
/// Not an admin, unknown provider, storage errors. A failed discovery is an answer.
pub async fn test_provider(s: &ApiState, actor: Actor, id: &str) -> ApiResult<OidcTestDto> {
    require_admin(actor)?;
    let o = oidc(s)?;
    let p = o
        .store()
        .provider(parse_uuid(id, "provider")?)
        .await
        .map_err(pe)?
        .ok_or_else(not_found)?;
    let found = o.rediscover(&p).await;
    let seen = s
        .auth
        .store()
        .identities_of(actor.user_id)
        .await
        .map_err(pe)?
        .into_iter()
        .find(|i| i.provider_id == Some(p.id))
        .map(|i| i.last_groups.is_some());
    let at = s.app.clock.now();
    let last = p.last_test.clone().unwrap_or(Value::Null);
    let prev = |k: &str| last.get(k).cloned().unwrap_or(Value::Null);
    let caps = found
        .as_ref()
        .map(|d| d.capabilities.clone())
        .unwrap_or_default();
    let out = OidcTestDto {
        ok: found.is_ok(),
        discovery: match &found {
            Ok(_) => "ok".into(),
            Err(f) => format!("failed: {}", f.code.as_str()),
        },
        end_session: caps.end_session_endpoint.is_some(),
        backchannel_logout: caps.backchannel_logout_supported,
        frontchannel_logout: caps.frontchannel_logout_supported,
        pkce_s256: caps
            .code_challenge_methods_supported
            .iter()
            .any(|m| m == "S256"),
        iss_parameter: caps.authorization_response_iss_parameter_supported,
        groups_claim_seen: seen,
        at: ts(at),
        // the probes are browser round trips (OD-19, od-12 4.4 step 5): their last outcome
        // stays with the Test, and the URLs start new ones
        silent_probe: prev("silent_probe").as_str().map(String::from),
        silent_probe_at: prev("silent_probe_at").as_str().map(String::from),
        max_age_probe: prev("max_age_probe").as_bool(),
        max_age_probe_at: prev("max_age_probe_at").as_str().map(String::from),
        silent_probe_url: Some(probe_url(&p.slug, "probe_silent")),
        max_age_probe_url: Some(probe_url(&p.slug, "probe_max_age")),
    };
    let stored = serde_json::to_value(&out).unwrap_or(Value::Null);
    o.store().record_test(p.id, &stored, at).await.map_err(pe)?;
    super::probes::refresh_probe(s, "identity_provider").await;
    Ok(out)
}

fn probe_url(slug: &str, mode: &str) -> String {
    format!("/api/v1/auth/oidc/start?provider={slug}&mode={mode}")
}

/// How long a passed Test counts for the `auto` precondition (addendum-1 4.3).
pub const AUTO_TEST_HOURS: i64 = 24;

/// The server-side preconditions of sign-in mode `auto` (addendum-1 4.3; the fresh proof
/// is checked by the gated settings write): an enabled default provider (or the only
/// enabled one) whose last Test passed within [`AUTO_TEST_HOURS`], and an admin who can
/// still sign in with a password and an enrolled authenticator (the break-glass account,
/// addendum-1 3.3).
///
/// # Errors
/// 409 `auto_needs_tested_provider`, `auto_needs_admin_authenticator`.
pub(crate) async fn check_auto(s: &ApiState) -> ApiResult<()> {
    let untested = || {
        refused(
            "auto_needs_tested_provider",
            "going straight to a provider needs a default provider with a passed Test in the last 24 hours",
        )
    };
    let o = s.oidc.as_deref().ok_or_else(untested)?;
    let enabled: Vec<Provider> = o
        .store()
        .providers()
        .await
        .map_err(pe)?
        .into_iter()
        .filter(|p| p.enabled)
        .collect();
    let default = enabled
        .iter()
        .find(|p| p.is_default)
        .or_else(|| enabled.first().filter(|_| enabled.len() == 1))
        .ok_or_else(untested)?;
    let now = s.app.clock.now();
    let passed = default.last_test.as_ref().is_some_and(|t| t["ok"] == true)
        && default
            .last_test_at
            .is_some_and(|t| now - t < chrono::Duration::hours(AUTO_TEST_HOURS));
    if !passed {
        return Err(untested());
    }
    if break_glass_admins(s, None).await? > 0 {
        return Ok(());
    }
    Err(refused(
        "auto_needs_admin_authenticator",
        "going straight to a provider needs an admin with a password and an authenticator",
    ))
}

/// Enabled admins (other than `except`) who can sign in on the page with a password and an
/// enrolled authenticator: the break-glass accounts of mode `auto` (addendum-1 3.3).
///
/// # Errors
/// Storage errors.
pub(crate) async fn break_glass_admins(s: &ApiState, except: Option<UserId>) -> ApiResult<usize> {
    let mut n = 0;
    for u in s.app.list_users().await? {
        if u.role != domain::user::Role::Admin
            || Some(u.id) == except
            || s.auth.is_disabled(u.id).await?
        {
            continue;
        }
        let creds = s.auth.store().credentials(u.id).await.map_err(pe)?;
        if creds.is_some_and(|c| c.password_hash.is_some() && c.totp_enabled) {
            n += 1;
        }
    }
    Ok(n)
}

/// The Overview row (DEC-285: stored fields only, no probe): off without an enabled
/// provider; error when the last Test or the last callback failed; else ok with the age of
/// the discovery document.
pub(crate) async fn health_row(s: &ApiState) -> HealthRowDto {
    let Some(o) = s.oidc.as_deref() else {
        return row("identity_provider", "off", "off", &[]);
    };
    let providers = match o.store().providers().await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "reading the providers failed");
            return row("identity_provider", "error", "unreadable", &[]);
        }
    };
    let enabled: Vec<&Provider> = providers.iter().filter(|p| p.enabled).collect();
    if enabled.is_empty() {
        return row("identity_provider", "off", "off", &[]);
    }
    for p in &enabled {
        // a failed Test counts until a sign-in through the provider worked after it
        let test_failed = p.last_test.as_ref().is_some_and(|t| t["ok"] == false);
        let ok_after =
            p.last_callback_result.as_deref() == Some("ok") && p.last_callback_at > p.last_test_at;
        if test_failed && !ok_after {
            return row(
                "identity_provider",
                "error",
                "test_failed",
                &[("provider", p.display_name.clone())],
            );
        }
        if p.last_callback_result
            .as_deref()
            .is_some_and(|r| r == "failed: oidc_unavailable")
        {
            return row(
                "identity_provider",
                "error",
                "unavailable",
                &[("provider", p.display_name.clone())],
            );
        }
    }
    let fetched = enabled.iter().filter_map(|p| p.discovery_fetched_at).min();
    match fetched {
        Some(at) => row("identity_provider", "ok", "discovered", &[("at", ts(at))]),
        None => row("identity_provider", "ok", "configured", &[]),
    }
}

/// Is any provider enabled (wizard checklist)?
pub(crate) async fn any_enabled(s: &ApiState) -> bool {
    match s.oidc.as_deref() {
        Some(o) => o
            .store()
            .providers()
            .await
            .is_ok_and(|p| p.iter().any(|p| p.enabled)),
        None => false,
    }
}

/// The provider that manages `user`'s role (`role_source = provider`), if any: badged
/// "via group" and locked in Users (addendum-1 4.2).
pub(crate) async fn role_via_provider(s: &ApiState, user: UserId) -> ApiResult<Option<String>> {
    let Some(o) = s.oidc.as_deref() else {
        return Ok(None);
    };
    let ids = s.auth.store().identities_of(user).await.map_err(pe)?;
    if ids.is_empty() {
        return Ok(None);
    }
    let providers = o.store().providers().await.map_err(pe)?;
    Ok(providers
        .into_iter()
        .find(|p| {
            p.enabled
                && p.role_source == RoleSource::Provider
                && ids.iter().any(|i| i.provider_id == Some(p.id))
        })
        .map(|p| p.display_name))
}

/// Can `user` still sign in without the identity `id`: a password, or another identity
/// through a provider that exists and is enabled (UX4-10: one through a disabled or deleted
/// provider is no way in).
async fn other_method(s: &ApiState, user: UserId, id: Uuid) -> ApiResult<bool> {
    let store = s.auth.store();
    let enabled: Vec<Uuid> = match s.oidc.as_deref() {
        Some(o) => o
            .store()
            .providers()
            .await
            .map_err(pe)?
            .into_iter()
            .filter(|p| p.enabled)
            .map(|p| p.id)
            .collect(),
        None => Vec::new(),
    };
    let password = store
        .credentials(user)
        .await
        .map_err(pe)?
        .is_some_and(|c| c.password_hash.is_some());
    let others = store
        .identities_of(user)
        .await
        .map_err(pe)?
        .iter()
        .filter(|i| i.id != id && i.provider_id.is_some_and(|p| enabled.contains(&p)))
        .count();
    Ok(password || others > 0)
}

/// Settings > Account > Linked sign-ins (addendum-1 4.2, design 6.12).
///
/// # Errors
/// Storage errors.
pub async fn linked_identities(s: &ApiState, user: UserId) -> ApiResult<Vec<LinkedIdentityDto>> {
    let providers = match s.oidc.as_deref() {
        Some(o) => o.store().providers().await.map_err(pe)?,
        None => Vec::new(),
    };
    let mut out = Vec::new();
    for i in s.auth.store().identities_of(user).await.map_err(pe)? {
        let p = providers.iter().find(|p| Some(p.id) == i.provider_id);
        out.push(LinkedIdentityDto {
            id: i.id.to_string(),
            provider: p.map_or_else(|| i.issuer.clone(), |p| p.display_name.clone()),
            provider_slug: p.map(|p| p.slug.clone()),
            issuer: i.issuer.clone(),
            linked_at: ts(i.linked_at),
            can_unlink: other_method(s, user, i.id).await?,
        });
    }
    Ok(out)
}

/// Unlink one of the caller's identities (gated, DEC-191). Refused when it would leave the
/// account with no way to sign in (addendum-1 4.2).
///
/// # Errors
/// `not_found`, `last_sign_in_method`, `reauth_required`.
pub async fn unlink_identity(s: &ApiState, c: Caller<'_>, id: &str) -> ApiResult<()> {
    let id = parse_uuid(id, "linked sign-in")?;
    let me = c.actor.user_id;
    let mine = s
        .auth
        .store()
        .identities_of(me)
        .await
        .map_err(pe)?
        .iter()
        .any(|i| i.id == id);
    if !mine {
        return Err(ApiError::new(
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            "linked sign-in not found",
        ));
    }
    if !other_method(s, me, id).await? {
        return Err(refused(
            "last_sign_in_method",
            "this is your only way to sign in; set a password or link another provider first",
        ));
    }
    fresh(s, c).await?;
    s.auth
        .store()
        .unlink_identity(me, id, None)
        .await
        .map_err(pe)?;
    Ok(())
}
