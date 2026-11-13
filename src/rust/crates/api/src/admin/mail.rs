//! Mail (SMTP) on Admin > Instance (addendum-1 3.2, DEC-245): host, port, security, user
//! and sender are instance settings (`mail.*`, provenance like every setting); the password
//! is stored encrypted (purpose `smtp_password`) and never returned. The environment's
//! `[notify.smtp]` wins and is read-only. Every change rebuilds the transport at once (no
//! restart); "Save and send a test" records when and how the last test went for the
//! Overview's mail row.

use std::sync::Arc;

use async_trait::async_trait;
use auth::admin::StoredSettings;
use contracts::admin::{
    HealthRowDto, MailPasswordDto, MailSettingsDto, MailTestDto, MailTestInputDto,
};
use domain::user::Actor;
use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::{
    AdminApi, Caller, admin, effective, entry, fresh, invalid, pe, proof, refused, require_admin,
    row, setting_dtos, ts,
};
use crate::ApiState;
use crate::error::ApiResult;

/// The keys of the mail settings.
pub const MAIL_KEYS: [&str; 5] = [
    "mail.host",
    "mail.port",
    "mail.security",
    "mail.username",
    "mail.from",
];

/// Everything needed to build the SMTP transport. `Debug` never prints the password.
#[derive(Clone, PartialEq, Eq)]
pub struct MailSettings {
    pub host: String,
    pub port: u16,
    /// `tls`, `starttls` or `none`.
    pub security: String,
    pub username: Option<String>,
    pub password: Option<Zeroizing<String>>,
    pub from: String,
}

impl std::fmt::Debug for MailSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MailSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("security", &self.security)
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "***"))
            .field("from", &self.from)
            .finish()
    }
}

/// The server's email transport, swappable at run time.
#[async_trait]
pub trait MailTransport: Send + Sync {
    /// Use these settings from now on (`None`: mail off).
    ///
    /// # Errors
    /// Settings the transport cannot use (bad host or sender); mail is then off.
    fn configure(&self, settings: Option<&MailSettings>) -> Result<(), String>;
    /// Send one plain-text message now.
    ///
    /// # Errors
    /// The SMTP error, or "mail is not configured".
    async fn send(&self, to: &str, subject: &str, body: &str) -> Result<(), String>;
}

fn text(eff: &std::collections::BTreeMap<String, Value>, k: &str) -> Option<String> {
    eff.get(k)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
}

/// Is the password fixed by the configuration (`[notify.smtp]` in env or file)?
fn env_fixed(a: &AdminApi) -> bool {
    a.config.fixed.contains_key("mail.host")
}

/// The effective settings: configuration (env/file) first, then the database.
pub(crate) async fn effective_mail(
    a: &AdminApi,
    st: &StoredSettings,
) -> ApiResult<Option<MailSettings>> {
    let eff = effective(a, st);
    let (Some(host), Some(from)) = (text(&eff, "mail.host"), text(&eff, "mail.from")) else {
        return Ok(None);
    };
    let password = if env_fixed(a) {
        a.config.mail_password.clone().map(|p| p.0)
    } else {
        match a.store.mail_password().await.map_err(pe)? {
            Some(Ok(p)) => Some(Zeroizing::new(p)),
            Some(Err(_)) => {
                tracing::warn!("the stored SMTP password is locked (its key is not loaded)");
                None
            }
            None => None,
        }
    };
    Ok(Some(MailSettings {
        host,
        port: eff
            .get("mail.port")
            .and_then(Value::as_u64)
            .and_then(|p| u16::try_from(p).ok())
            .unwrap_or(587),
        security: text(&eff, "mail.security").unwrap_or_else(|| "starttls".into()),
        username: text(&eff, "mail.username"),
        password,
        from,
    }))
}

/// Build the transport from the effective settings (start-up and after every change).
///
/// # Errors
/// The settings give no usable transport (mail is off then; UX3-18: the caller reports it)
/// or could not be read; the text is for the log.
pub async fn rebuild(s: &ApiState) -> Result<(), String> {
    let Some(a) = s.admin.as_ref() else {
        return Ok(());
    };
    let Some(t) = &a.mail else {
        return Ok(());
    };
    let settings = match a.store.settings().await {
        Ok(st) => effective_mail(a, &st).await,
        Err(e) => {
            tracing::error!(error = %e, "reading the mail settings failed");
            return Err(e.to_string());
        }
    };
    match settings {
        Ok(m) => t.configure(m.as_ref()).map_err(|e| {
            tracing::warn!(error = %e, "the mail settings cannot be used; mail is off");
            e
        }),
        Err(e) => {
            tracing::error!(error = %e.message, "reading the mail settings failed");
            Err(e.message)
        }
    }
}

fn password_source(a: &AdminApi, st: &StoredSettings) -> &'static str {
    if env_fixed(a) {
        if a.config.mail_password.is_some() {
            "env"
        } else {
            "none"
        }
    } else if st.mail.password_set {
        "db"
    } else {
        "none"
    }
}

fn dto(a: &AdminApi, st: &StoredSettings) -> MailSettingsDto {
    let eff = effective(a, st);
    MailSettingsDto {
        settings: setting_dtos(a, st, &MAIL_KEYS),
        configured: text(&eff, "mail.host").is_some() && text(&eff, "mail.from").is_some(),
        password_set: password_source(a, st) != "none",
        password_source: password_source(a, st).into(),
        last_test_at: st.mail.last_test_at.map(ts),
        last_test_result: st.mail.last_test_result.clone(),
    }
}

/// Admin > Instance > Mail.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn mail(s: &ApiState, actor: Actor) -> ApiResult<MailSettingsDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let st = a.store.settings().await.map_err(pe)?;
    Ok(dto(a, &st))
}

/// Replace (or with `null` remove) the SMTP password: encrypted, audited with purpose, key
/// version and proof, never returned. Gated (DEC-397): a stolen session must not redirect
/// mail. Refused while the configuration sets the mail settings.
///
/// # Errors
/// Not an admin, `fixed_by_config`, `reauth_required`, storage errors.
pub async fn set_mail_password(
    s: &ApiState,
    c: Caller<'_>,
    b: MailPasswordDto,
) -> ApiResult<MailSettingsDto> {
    require_admin(c.actor)?;
    let a = admin(s)?;
    if env_fixed(a) {
        return Err(refused(
            "fixed_by_config",
            "the mail settings are set by the configuration ([notify.smtp])",
        ));
    }
    fresh(s, c).await?;
    let password = b.password.filter(|p| !p.is_empty()).map(Zeroizing::new);
    let method = proof(s, c.token).await;
    let (action, detail) = if password.is_some() {
        (
            "secret.replaced",
            json!({"purpose": auth::admin::MAIL_PURPOSE, "key_version": a.keys.active(),
                "proof": method}),
        )
    } else {
        (
            "secret.cleared",
            json!({"purpose": auth::admin::MAIL_PURPOSE, "proof": method}),
        )
    };
    let row = entry(
        s,
        Some(c.actor),
        action,
        Some(("setting", "mail.password".into())),
        detail,
    )
    .await;
    a.store
        .put_mail_password(password.as_deref().map(String::as_str), Some(&row))
        .await
        .map_err(pe)?;
    // the password alone does not make a transport unusable; a failure is logged
    let _ = rebuild(s).await;
    mail(s, c.actor).await
}

/// "Send a test": one message to `to` (default: the admin's own address) with the settings
/// in effect now; the outcome is recorded (`mail.last_test_at`, result) and returned.
///
/// # Errors
/// Not an admin, `no_recipient`, storage errors. A failed send is an answer, not an error.
pub async fn send_test_mail(
    s: &ApiState,
    actor: Actor,
    input: MailTestInputDto,
) -> ApiResult<MailTestDto> {
    require_admin(actor)?;
    let adm = admin(s)?;
    let to = match input.to.filter(|t| !t.trim().is_empty()) {
        Some(t) => t,
        None => s
            .app
            .user(actor.user_id)
            .await?
            .email
            .ok_or_else(|| invalid("no_recipient", "give an address to send the test to"))?,
    };
    let st = adm.store.settings().await.map_err(pe)?;
    let outcome = match (effective_mail(adm, &st).await?, &adm.mail) {
        (None, _) | (_, None) => Err("mail is not configured".to_string()),
        (Some(settings), Some(transport)) => match transport.configure(Some(&settings)) {
            Ok(()) => {
                let subject = format!("{}: test message", adm.brand.name);
                let body = "This is a test message. Mail is set up and working.";
                transport.send(to.trim(), &subject, body).await
            }
            Err(err) => Err(err),
        },
    };
    // the admin sees the category, as it is stored (UX4-22, DEC-534): the transport's own
    // text can name hosts and say what answered on a port, so it goes to the log only
    if let Err(err) = &outcome {
        tracing::warn!(error = %err, "the mail test failed");
    }
    let result = stored_result(&outcome);
    let at = s.app.clock.now();
    adm.store.record_mail_test(at, &result).await.map_err(pe)?;
    super::probes::refresh_probe(s, "mail").await;
    Ok(MailTestDto {
        ok: outcome.is_ok(),
        result,
        at: ts(at),
    })
}

/// What is stored of a mail test (and read into `health_probes.args`): `ok` or `failed: <category>`
/// with one of `not_configured`, `private_target`, `connection`, `auth`, `rejected`, `send_failed`; never the
/// transport's text, which can name the SMTP host (UX4-22).
fn stored_result(outcome: &Result<(), String>) -> String {
    let Err(err) = outcome else {
        return "ok".to_string();
    };
    let e = err.to_ascii_lowercase();
    let code = if e.contains("not configured") {
        "not_configured"
    } else if e.contains("private address") {
        // `notify::channels::email::PRIVATE_ADDRESS` (SEC-4)
        "private_target"
    } else if e.contains("authentication") || e.contains("(535)") || e.contains("(534)") {
        "auth"
    } else if e.contains("connection") || e.contains("timed out") || e.contains("dns") {
        "connection"
    } else if e.contains("permanent error") || e.contains("transient error") {
        "rejected"
    } else {
        "send_failed"
    };
    format!("failed: {code}")
}

/// The Overview's mail row: off, configured, tested, or the last test failed.
pub(crate) fn health_row(a: &AdminApi, st: &StoredSettings) -> HealthRowDto {
    let eff = effective(a, st);
    if text(&eff, "mail.host").is_none() || text(&eff, "mail.from").is_none() {
        return row("mail", "off", "off", &[]);
    }
    match st.mail.last_test_result.as_deref() {
        Some("ok") => row(
            "mail",
            "ok",
            "tested",
            &[("at", st.mail.last_test_at.map(ts).unwrap_or_default())],
        ),
        Some(r) => row(
            "mail",
            "error",
            "test_failed",
            &[("result", r.trim_start_matches("failed: ").to_string())],
        ),
        None => row("mail", "ok", "configured", &[]),
    }
}

/// The transport handle for the admin API.
pub type SharedTransport = Arc<dyn MailTransport>;

/// Can mail be sent now: settings in effect and a transport (the emailed link, DEC-312)?
pub(crate) async fn mail_ready(s: &ApiState) -> bool {
    let Ok(adm) = admin(s) else {
        return false;
    };
    let Ok(st) = adm.store.settings().await else {
        return false;
    };
    adm.mail.is_some() && matches!(effective_mail(adm, &st).await, Ok(Some(_)))
}

/// Send one message with the settings in effect now (as the test mail does).
///
/// # Errors
/// Mail is not configured, the transport refused the settings, or the SMTP error.
pub(crate) async fn send_mail(
    s: &ApiState,
    to: &str,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    let adm = admin(s).map_err(|e| e.message)?;
    let st = adm.store.settings().await.map_err(|e| e.to_string())?;
    match (
        effective_mail(adm, &st).await.map_err(|e| e.message)?,
        &adm.mail,
    ) {
        (Some(settings), Some(transport)) => {
            transport.configure(Some(&settings))?;
            transport.send(to.trim(), subject, body).await
        }
        _ => Err("mail is not configured".into()),
    }
}

#[cfg(test)]
mod stored_result_tests {
    use super::stored_result;

    #[test]
    fn a_failed_test_is_stored_as_a_category_without_hosts_or_text() {
        // UX4-22: the raw transport text can name the SMTP host; only a category is kept
        let raw = [
            (
                "Connection error: connection refused (os error 111) smtp.example.org:587",
                "connection",
            ),
            ("timed out", "connection"),
            (
                "permanent error (535): 5.7.8 Authentication credentials invalid",
                "auth",
            ),
            (
                "permanent error (550): mailbox unavailable at mx.example.org",
                "rejected",
            ),
            ("mail is not configured", "not_configured"),
            ("something else at host.example.org", "send_failed"),
            ("the mail server is on a private address", "private_target"),
        ];
        for (text, code) in raw {
            let stored = stored_result(&Err(text.to_string()));
            assert_eq!(stored, format!("failed: {code}"), "{text}");
            assert!(!stored.contains("example.org"));
        }
        assert_eq!(stored_result(&Ok(())), "ok");
    }
}
