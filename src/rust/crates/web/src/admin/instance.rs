//! Admin > Instance (addendum-1 3.2-3.4, mockup `admin.html` annotations 4-8): the three
//! settings that decide how the instance behaves are open (hosting mode, DEC-282; sign-up
//! mode, moved here from Users; the public address, read-only from the configuration), and
//! the rest sits behind disclosures: Mail with "Save and send a test" (DEC-300), Secrets at
//! rest, and Advanced (session length and cookie security, folded in from the phase 2
//! Security page). Values set in the environment are read-only with the variable's name.

use std::collections::BTreeMap;

use contracts::admin::{MailSettingsDto, MailTestDto, SettingDto};
use leptos::prelude::*;
use serde_json::{Value, json};
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Button, Disclosure, DisclosureMode,
    FormField, Input, SecretField, Select, SelectOption, disclosure_storage_key,
    field_described_by,
};

use super::security::{Advanced, Data as SecurityData, SecretsAtRest, SecurityWarnings};
use super::users::SignupForm;
use super::{AdminFrame, SettingNote};
use crate::admin_api;
use crate::admin_state::{Origin, find_setting, origin, setting_label_key};
use crate::i18n::{t, use_i18n};
use crate::reauth::{gated, gated_settings, use_reauth};
use crate::rest;
use crate::settings::{Outcome, kept, loaded};

type Message = RwSignal<Option<Result<String, String>>>;
type MailData = Resource<Result<MailSettingsDto, ServerFnError>>;

/// The SMTP settings in form order (`mail.*`, DEC-300).
const MAIL_KEYS: [&str; 5] = [
    "mail.host",
    "mail.port",
    "mail.security",
    "mail.username",
    "mail.from",
];

#[component]
pub fn Instance() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-instance") lead=t("admin-instance-lead")>
            <InstanceBody/>
        </AdminFrame>
    }
}

#[component]
fn InstanceBody() -> impl IntoView {
    let settings = Resource::new(|| (), |()| admin_api::admin_settings());
    let security: SecurityData = Resource::new(|| (), |()| admin_api::admin_security());
    let mail: MailData = Resource::new(|| (), |()| admin_api::admin_mail());
    let open = |section: &str| disclosure_storage_key("instance", section);
    view! {
        <SecurityWarnings data=security/>
        {loaded(settings, |s| view! {
            {find_setting(&s.settings, "instance.hosting").cloned().map(|h| view! { <Hosting s=h/> })}
            {find_setting(&s.settings, "auth.signup").cloned().map(|x| view! {
                <section class="mb-8 max-w-2xl"><SignupForm s=x/></section>
            })}
            <PublicAddress url=s.public_url.clone()/>
        })}
        <div class="grid max-w-3xl gap-4">
            <Disclosure id="adm-mail" title=t("admin-mail-title") summary=t("admin-mail-summary")
                mode=DisclosureMode::Button level=2 storage_key=open("mail")>
                {loaded(mail, move |m| view! { <Mail m=m data=mail/> })}
            </Disclosure>
            <Disclosure id="adm-secrets" title=t("admin-secrets-title") summary=t("admin-secrets-summary")
                mode=DisclosureMode::Button level=2 storage_key=open("secrets")>
                <SecretsAtRest data=security/>
            </Disclosure>
            <Disclosure id="adm-advanced" title=t("admin-advanced-title") summary=t("admin-advanced-summary")
                mode=DisclosureMode::Button level=2 storage_key=open("advanced")>
                <Advanced data=security/>
            </Disclosure>
        </div>
    }
}

const HOSTING: [(&str, &str, &str); 2] = [
    ("home", "admin-hosting-home", "admin-hosting-home-hint"),
    (
        "hosted",
        "admin-hosting-hosted",
        "admin-hosting-hosted-hint",
    ),
];

/// The hosting mode after the wizard (DEC-282): two radios with their consequences.
#[component]
fn Hosting(s: SettingDto) -> impl IntoView {
    let reauth = use_reauth();
    let fixed = origin(&s) != Origin::Editable;
    let value = RwSignal::new(s.value.as_str().unwrap_or("home").to_string());
    let busy = RwSignal::new(false);
    let message = kept("instance-hosting", || None::<Result<String, String>>);
    let i18n = use_i18n();
    let save = move || {
        let saved = i18n.t("admin-saved");
        let values = BTreeMap::from([(
            "instance.hosting".to_string(),
            Value::from(value.get_untracked()),
        )]);
        gated_settings(
            reauth,
            busy,
            values,
            Callback::new(move |r: Result<_, String>| message.set(Some(r.map(|_| saved.clone())))),
        );
    };
    view! {
        <form class="mb-8 max-w-2xl" on:submit=move |ev| { ev.prevent_default(); save(); }>
            <fieldset class="grid gap-3">
                <legend class="mb-2 text-lg font-semibold">{t("admin-hosting-title")}</legend>
                {HOSTING.iter().map(|(mode, label, hint)| {
                    let id = format!("adm-hosting-{mode}");
                    let hint_id = format!("{id}-hint");
                    view! {
                        <div class="flex items-start gap-3">
                            <input type="radio" name="hosting" id=id.clone() value=*mode disabled=fixed
                                aria-describedby=hint_id.clone() class="mt-1 size-4 accent-primary"
                                prop:checked=move || value.get() == *mode
                                on:change=move |_| value.set((*mode).to_string())/>
                            <span class="grid gap-0.5">
                                <label for=id class="text-sm font-medium">{t(label)}</label>
                                <span id=hint_id class="text-sm text-muted-foreground">{t(hint)}</span>
                            </span>
                        </div>
                    }
                }).collect_view()}
            </fieldset>
            <p class="mt-2 text-sm text-muted-foreground">{t("admin-hosting-note")}</p>
            <div class="mt-3 grid gap-3">
                <SettingNote s=s.clone()/>
                <Outcome message=message/>
                {(!fixed).then(|| view! { <Button r#type="submit" class="w-fit" loading=busy>{t("admin-save")}</Button> })}
            </div>
        </form>
    }
}

/// `server.public_url`: a security boundary, so set by the configuration only (addendum-1
/// 3.2).
#[component]
fn PublicAddress(url: String) -> impl IntoView {
    let note = use_i18n().t_args(
        "admin-public-url-hint",
        &[("var", "APP__SERVER__PUBLIC_URL".into())],
    );
    view! {
        <section aria-labelledby="adm-public-url-h" class="mb-8 max-w-2xl" data-testid="public-url">
            <h2 id="adm-public-url-h" class="mb-2 text-lg font-semibold">{t("admin-public-url")}</h2>
            <p class="break-all font-mono text-sm">{url}</p>
            <p class="mt-1 flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                <Badge variant=BadgeVariant::Outline>{t("admin-setting-read-only")}</Badge>
                <span>{note}</span>
            </p>
        </section>
    }
}

/// The value sent for a mail field: the port as a number, an empty field as `null`.
#[must_use]
pub fn mail_value(key: &str, text: &str) -> Value {
    let t = text.trim();
    if t.is_empty() {
        return Value::Null;
    }
    if key == "mail.port"
        && let Ok(n) = t.parse::<u16>()
    {
        return Value::from(n);
    }
    Value::from(t)
}

/// A setting's value as the text of its field.
fn field_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The last test line ("Last test: 7 Nov 2026, 10:12: worked.").
fn last_test(m: &MailSettingsDto) -> Option<String> {
    let i18n = use_i18n();
    let at = m.last_test_at.as_deref()?;
    let result = m.last_test_result.as_deref().unwrap_or_default();
    let key = if result == "ok" {
        "admin-mail-last-ok"
    } else {
        "admin-mail-last-failed"
    };
    let reason = result.strip_prefix("failed: ").unwrap_or(result);
    Some(i18n.t_args(
        key,
        &[("at", i18n.datetime(at).into()), ("reason", reason.into())],
    ))
}

/// What "Save and send a test" answers: the save's mail warnings, then the test.
type Tested = (Vec<String>, MailTestDto);

/// Save the mail settings (when editable), the password (when typed), then send a test. The
/// save's mail warnings (`mail_transport_unusable`, DEC-378) come back with the test.
async fn save_and_test(
    values: Option<BTreeMap<String, Value>>,
    password: Option<String>,
    to: Option<String>,
) -> Result<Tested, ServerFnError> {
    let mut warnings = Vec::new();
    if let Some(v) = values {
        let saved = admin_api::update_admin_settings(v).await?;
        warnings.extend(
            saved
                .warnings
                .into_iter()
                .filter(|w| w.starts_with("mail_")),
        );
    }
    if let Some(p) = password {
        rest::send("PUT", "/admin/mail/password", Some(json!({"password": p}))).await?;
    }
    let test = rest::call::<MailTestDto>("POST", "/admin/mail/test", Some(json!({"to": to})))
        .await?
        .ok_or_else(|| ServerFnError::new("generic"))?;
    Ok((warnings, test))
}

/// The outcome line of a test.
fn test_message(texts: &crate::i18n::I18n, test: &MailTestDto) -> Result<String, String> {
    if test.ok {
        Ok(texts.t("admin-mail-test-ok"))
    } else {
        let reason = test.result.trim_start_matches("failed: ").to_string();
        Err(texts.t_args("admin-mail-test-failed", &[("reason", reason.into())]))
    }
}

/// Admin > Instance > Mail (addendum-1 3.2, DEC-300): SMTP for password resets and email
/// notifications; the password is write-only.
#[component]
fn Mail(m: MailSettingsDto, data: MailData) -> impl IntoView {
    let i18n = use_i18n();
    let fields: Vec<(SettingDto, RwSignal<String>)> = MAIL_KEYS
        .iter()
        .filter_map(|k| find_setting(&m.settings, k).cloned())
        .map(|s| {
            let v = RwSignal::new(field_text(&s.value));
            (s, v)
        })
        .collect();
    let editable = fields.iter().all(|(s, _)| origin(s) == Origin::Editable);
    let password = RwSignal::new(String::new());
    // the address outlives the refetch after a test (the form renders again)
    let to = kept("instance-mail-to", String::new);
    let replacing = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let message: Message = kept("instance-mail", || None);
    let warning = kept("instance-mail-warning", || None::<String>);
    let reauth = use_reauth();
    let stored = m.password_set.then(|| t("admin-mail-password-stored"));
    let last = last_test(&m);
    let note = fields.first().map(|(s, _)| s.clone());
    let values = fields.clone();
    let send = move |_| {
        let typed = values
            .iter()
            .map(|(s, v)| (s.key.clone(), mail_value(&s.key, &v.get_untracked())))
            .collect::<BTreeMap<_, _>>();
        let pw = password.get_untracked();
        let pw = (!pw.is_empty()).then_some(pw);
        let address = to.get_untracked().trim().to_string();
        let address = (!address.is_empty()).then_some(address);
        let texts = use_i18n();
        let values = editable.then_some(typed);
        // a `reauth_required` answer opens the dialog and runs it again (UX3-18)
        let call = move || save_and_test(values.clone(), pw.clone(), address.clone());
        let done = Callback::new(move |r: Result<Tested, String>| {
            match r {
                Ok((warnings, test)) => {
                    let w = warnings
                        .first()
                        .map(|w| texts.t(&format!("admin-warning-{w}")));
                    warning.set(w);
                    message.set(Some(test_message(&texts, &test)));
                }
                Err(e) => message.set(Some(Err(e))),
            }
            password.set(String::new());
            replacing.set(false);
            data.refetch();
        });
        gated(reauth, busy, call, done);
    };
    view! {
        <div class="grid gap-4">
            <p class="text-sm text-muted-foreground">{t("admin-mail-lead")}</p>
            <div class="grid gap-4 sm:grid-cols-2">
                {fields.into_iter().map(|(s, v)| view! { <MailField s=s value=v editable=editable/> }).collect_view()}
            </div>
            <SecretField id="adm-mail-password" label=t("admin-mail-password") value=password editing=replacing
                stored=stored replace_label=t("admin-replace") cancel_label=t("reauth-cancel")
                description=t("admin-mail-password-hint")/>
            {note.filter(|_| !editable).map(|s| view! { <SettingNote s=s/> })}
            <FormField id="adm-mail-to" label=t("admin-mail-to") description=t("admin-mail-to-hint")>
                <Input id="adm-mail-to" value=to inputmode="email" autocomplete="email" described_by=field_described_by("adm-mail-to")/>
            </FormField>
            {last.map(|l| view! { <p data-testid="mail-last-test" class="text-sm">{l}</p> })}
            {move || warning.get().map(|w| view! {
                <Alert variant=AlertVariant::Warning role=AlertRole::Status attr:data-testid="mail-warning">{w}</Alert>
            })}
            <Outcome message=message/>
            <div class="flex flex-wrap items-center gap-3">
                <Button loading=busy on:click=send>{i18n.t("admin-mail-send-test")}</Button>
                <span class="text-sm text-muted-foreground">{t("admin-mail-send-test-hint")}</span>
            </div>
        </div>
    }
}

/// One SMTP field; the connection security is a choice of three.
#[component]
fn MailField(s: SettingDto, value: RwSignal<String>, editable: bool) -> impl IntoView {
    let id = format!("adm-{}", s.key.replace('.', "-"));
    let label = t(&setting_label_key(&s.key));
    if s.key == "mail.security" {
        let options: Vec<SelectOption> = ["tls", "starttls", "none"]
            .iter()
            .map(|v| SelectOption {
                value: (*v).to_string(),
                label: t(&format!("admin-mail-security-{v}")),
            })
            .collect();
        return view! {
            <div class="grid gap-2">
                <ui::Label r#for=id.clone()>{label}</ui::Label>
                <Select id=id value=value options=options attr:disabled=!editable/>
            </div>
        }
        .into_any();
    }
    let (inputmode, autocomplete) = match s.key.as_str() {
        "mail.port" => ("numeric", "off"),
        "mail.from" => ("email", "off"),
        _ => ("text", "off"),
    };
    view! {
        <FormField id=id.clone() label=label>
            <Input id=id.clone() value=value inputmode=inputmode autocomplete=autocomplete readonly=!editable
                described_by=field_described_by(&id)/>
        </FormField>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_fields_send_a_number_for_the_port_and_null_when_empty() {
        assert_eq!(mail_value("mail.port", " 587 "), Value::from(587));
        assert_eq!(mail_value("mail.port", "smtp"), Value::from("smtp"));
        assert_eq!(mail_value("mail.host", "  "), Value::Null);
        assert_eq!(
            mail_value("mail.host", "mail.example.org"),
            Value::from("mail.example.org")
        );
        assert_eq!(field_text(&Value::from(25)), "25");
        assert_eq!(field_text(&Value::Null), "");
    }
}
