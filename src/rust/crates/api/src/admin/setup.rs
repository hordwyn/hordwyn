//! The setup wizard (design.md 5.1, 4.3) and re-authentication (DEC-191).
//!
//! Wizard state machine: `account` (no user yet, no session needed) -> `hosting` ->
//! `sources` -> `done`; `complete` sets `setup_completed_at`, after which every wizard
//! endpoint answers 404 `setup_done` (like an install lock). Completed steps are kept, so
//! leaving the wizard resumes where it stopped.

use axum::http::StatusCode;
use catalog_core::CredentialState;
use contracts::admin::{
    HostingInputDto, ReauthInputDto, ReauthStatusDto, RecoveryCodesDto, SetupSourceDto,
    SetupStateDto, TotpEnrolmentDto,
};
use contracts::{SessionDto, SignUpDto};
use domain::user::Actor;
use serde_json::json;

use super::{admin, audit, invalid, pe, require_admin, ts};
use crate::error::ApiResult;
use crate::{ApiError, ApiState};

const STEPS: [&str; 4] = ["account", "hosting", "sources", "done"];

fn setup_done() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "setup_done", "setup is complete")
}

async fn completed(s: &ApiState) -> ApiResult<bool> {
    Ok(admin(s)?
        .store
        .settings()
        .await
        .map_err(pe)?
        .setup_completed_at
        .is_some())
}

/// What the page routes need before rendering (design.md 5.1): does any account exist
/// (otherwise every page redirects to `/setup`), and is the wizard completed (then `/setup`
/// is the 404 page)? Without the admin store the wizard never existed: completed.
///
/// # Errors
/// Storage errors.
pub async fn setup_gate(s: &ApiState) -> ApiResult<(bool, bool)> {
    let users = s.app.user_count().await? > 0;
    let done = match s.admin.as_ref() {
        Some(a) => a
            .store
            .settings()
            .await
            .map_err(pe)?
            .setup_completed_at
            .is_some(),
        None => true,
    };
    Ok((users, done))
}

/// The step after the last completed one.
fn next_step(last: Option<&str>) -> &'static str {
    match last {
        None | Some("account") => "hosting",
        Some("hosting") => "sources",
        _ => "done",
    }
}

async fn setup_sources_dto(s: &ApiState) -> ApiResult<Vec<SetupSourceDto>> {
    let a = admin(s)?;
    let licences = a.store.licences().await.map_err(pe)?;
    Ok(s.registry
        .infos()
        .iter()
        .filter_map(|i| s.registry.get(&i.id))
        .map(|c| {
            let info = c.info();
            let state = s.registry.credential(&info.id);
            SetupSourceDto {
                id: info.id.clone(),
                name: info.name.clone(),
                kinds: info.kinds.clone(),
                fields: c
                    .credentials()
                    .instance_fields
                    .iter()
                    .map(|f| (*f).to_string())
                    .collect(),
                // a rejected key is "set by env" only when the environment set it; one typed in
                // the UI is "invalid" so the wizard still offers its fields (UX2-6)
                credential: match state {
                    CredentialState::Invalid if a.config.source_env.contains_key(&info.id) => "env",
                    CredentialState::Invalid => "invalid",
                    other => other.as_str(),
                }
                .into(),
                env_var: a.config.source_env.get(&info.id).cloned(),
                agreement_on_file: licences
                    .iter()
                    .find(|l| l.connector == info.id)
                    .map(|l| l.agreement_on_file),
            }
        })
        .collect())
}

/// Where the wizard stands. Without users anyone may ask (the account step); afterwards
/// only admins. 404 `setup_done` once completed.
///
/// # Errors
/// `setup_done`, `forbidden`, `unauthorized`.
pub async fn setup_state(s: &ApiState, actor: Option<Actor>) -> ApiResult<SetupStateDto> {
    if completed(s).await? {
        return Err(setup_done());
    }
    let a = admin(s)?;
    let first = s.app.user_count().await? == 0;
    let step = if first {
        "account"
    } else {
        require_admin(actor.ok_or_else(ApiError::unauthorized)?)?;
        let st = a.store.settings().await.map_err(pe)?;
        next_step(st.setup_step.as_deref())
    };
    let sources = setup_sources_dto(s).await?;
    let read_only = !sources.is_empty()
        && sources
            .iter()
            .all(|x| x.credential == "env" || x.credential == "not_needed");
    let mut checklist = vec![
        "first_item",
        "invite",
        "media_server",
        "notification_defaults",
    ];
    if a.keys.source == "generated" || a.keys.source == "file" {
        checklist.push("backup_key");
    }
    checklist.push("identity_provider");
    Ok(SetupStateDto {
        step: step.into(),
        steps: STEPS.iter().map(|x| (*x).to_string()).collect(),
        hosting: a.hosting(),
        hosting_fixed: a.config.fixed.contains_key("instance.hosting"),
        sources,
        sources_read_only: read_only,
        checklist: checklist.into_iter().map(String::from).collect(),
        key_file: (a.keys.source == "generated" || a.keys.source == "file")
            .then(|| a.keys.key_file.clone()),
    })
}

/// Step 1: the first account (becomes admin and is signed in). Refused once any account
/// exists.
///
/// # Errors
/// `setup_done`, `conflict` when an account exists, the sign-up field errors.
pub async fn setup_account(s: &ApiState, b: SignUpDto) -> ApiResult<SessionDto> {
    if completed(s).await? {
        return Err(setup_done());
    }
    if s.app.user_count().await? > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "conflict",
            "the admin account exists; sign in to continue the setup",
        ));
    }
    let (user, token) = s
        .auth
        .sign_up(auth::SignUp {
            username: b.username,
            display_name: b.display_name,
            email: b.email,
            password: b.password,
            invite: None,
        })
        .await?;
    let a = admin(s)?;
    let mut st = a.store.settings().await.map_err(pe)?;
    st.setup_step = Some("account".into());
    a.store.put_settings(&st).await.map_err(pe)?;
    Ok(SessionDto {
        token: token.0,
        user: crate::dto_user(&user),
    })
}

/// Step 2: home or hosted (stored unless the configuration fixes it).
///
/// # Errors
/// Not an admin, `setup_done`, `invalid_setting`.
pub async fn setup_hosting(
    s: &ApiState,
    actor: Actor,
    b: HostingInputDto,
) -> ApiResult<SetupStateDto> {
    require_admin(actor)?;
    if completed(s).await? {
        return Err(setup_done());
    }
    if !matches!(b.hosting.as_str(), "home" | "hosted") {
        return Err(invalid("invalid_setting", "home or hosted"));
    }
    let a = admin(s)?;
    let mut st = a.store.settings().await.map_err(pe)?;
    if !a.config.fixed.contains_key("instance.hosting") {
        let from = st.hosting.clone();
        st.hosting = Some(b.hosting.clone());
        let detail = json!({"key": "instance.hosting", "from": from, "to": b.hosting});
        audit(
            s,
            Some(actor),
            "setting.changed",
            Some(("setting", "instance.hosting".into())),
            detail,
        )
        .await;
    }
    st.setup_step = Some("hosting".into());
    a.store.put_settings(&st).await.map_err(pe)?;
    super::apply(s, &st);
    setup_state(s, Some(actor)).await
}

/// Step 3 done (keys are saved through the data-source endpoints; everything skippable).
///
/// # Errors
/// Not an admin, `setup_done`.
pub async fn setup_sources(s: &ApiState, actor: Actor) -> ApiResult<SetupStateDto> {
    require_admin(actor)?;
    if completed(s).await? {
        return Err(setup_done());
    }
    let a = admin(s)?;
    let mut st = a.store.settings().await.map_err(pe)?;
    st.setup_step = Some("sources".into());
    a.store.put_settings(&st).await.map_err(pe)?;
    setup_state(s, Some(actor)).await
}

/// Finish: lock the wizard (audited).
///
/// # Errors
/// Not an admin, `setup_done`.
pub async fn setup_complete(s: &ApiState, actor: Actor) -> ApiResult<()> {
    require_admin(actor)?;
    if completed(s).await? {
        return Err(setup_done());
    }
    let a = admin(s)?;
    let mut st = a.store.settings().await.map_err(pe)?;
    let now = s.app.clock.now();
    st.setup_completed_at = Some(now);
    st.setup_step = Some("done".into());
    a.store.put_settings(&st).await.map_err(pe)?;
    audit(
        s,
        Some(actor),
        "wizard.completed",
        None,
        json!({"hosting": a.hosting()}),
    )
    .await;
    Ok(())
}

// ---------------------------------------------------------------- re-authentication

fn status_dto(r: auth::ReauthStatus, now: chrono::DateTime<chrono::Utc>) -> ReauthStatusDto {
    let mut methods = Vec::new();
    if r.has_password {
        methods.push("password".to_string());
    }
    ReauthStatusDto {
        fresh: r.fresh,
        until: r.until.map(ts),
        method: r.method,
        has_password: r.has_password,
        has_totp: r.has_totp,
        methods,
        minutes_left: r
            .until
            .filter(|_| r.fresh)
            .map(|u| (u - now).num_minutes().max(0)),
    }
}

/// Is the session's proof fresh, and how can it be renewed?
///
/// # Errors
/// `reauth_required` for an unknown session.
pub async fn reauth_status(s: &ApiState, token: &str) -> ApiResult<ReauthStatusDto> {
    Ok(status_dto(
        s.auth.reauth_status(token).await?,
        s.app.clock.now(),
    ))
}

/// Prove it again (password, then TOTP or a recovery code). Returns the new status and the
/// regenerated session token; the caller sets the cookie. The OIDC round trip with
/// `max_age=0` arrives with phase 3 (`auth::Auth::confirm_oidc_reauth` is the hook).
///
/// # Errors
/// `invalid_credentials`, `totp_required`, `invalid_totp`, `too_many_attempts`,
/// `reauth_required`.
pub async fn reauth(
    s: &ApiState,
    token: &str,
    b: ReauthInputDto,
) -> ApiResult<(ReauthStatusDto, String)> {
    let fresh = s
        .auth
        .reauth(
            token,
            &b.password,
            b.totp.as_deref().filter(|t| !t.is_empty()),
        )
        .await?;
    let status = reauth_status(s, &fresh.0).await?;
    Ok((status, fresh.0))
}

// ---------------------------------------------------------------- account security

/// Start enrolling an authenticator; needs a fresh proof (DEC-191, `reauth_required`).
///
/// # Errors
/// `reauth_required`, storage errors.
pub async fn totp_begin(s: &ApiState, token: &str) -> ApiResult<TotpEnrolmentDto> {
    let a = s.auth.require_fresh(token).await?;
    let e = s.auth.totp_begin(a).await?;
    Ok(TotpEnrolmentDto {
        otpauth_url: e.otpauth_url,
        secret_base32: e.secret_base32,
    })
}

/// Turn the authenticator on with a first code; answers the recovery codes, shown once
/// (OD-08).
///
/// # Errors
/// `invalid_totp`, storage errors.
pub async fn totp_confirm(s: &ApiState, actor: Actor, code: &str) -> ApiResult<RecoveryCodesDto> {
    let codes = s.auth.totp_confirm(actor, code).await?;
    Ok(RecoveryCodesDto { codes })
}

/// Change the password; signs out every other session and returns the caller's new session
/// token (entering the current password is a fresh proof, DEC-191).
///
/// # Errors
/// `invalid_credentials` for a wrong current password, the password rules for the new one.
pub async fn change_password(
    s: &ApiState,
    actor: Actor,
    current: &str,
    new: &str,
) -> ApiResult<String> {
    Ok(s.auth.change_password(actor, current, new).await?.0)
}
