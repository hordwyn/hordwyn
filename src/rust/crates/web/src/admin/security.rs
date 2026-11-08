//! The security parts of Admin > Instance (OD-11, OD-12, addendum-1 2.2, 3.3 and 3.4; the
//! Security page of phase 2 folded in): the warnings, the re-authentication window, session
//! length and cookie security in "Advanced" (a cookie change applies from the next sign-in;
//! secure cookies need an https public address), and "Secrets at rest" with the key ring,
//! the rotation reminder (OD-09) and "Re-encrypt now".

use std::collections::BTreeMap;

use contracts::admin::{SecretsStatusDto, SecurityDto, SettingDto};
use leptos::prelude::*;
use serde_json::Value;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonVariant, Disclosure, FormField, Input, Label,
    Switch, Table, TableBody, TableCell, TableHead, TableHeader, TableRow, field_described_by,
};

use super::SettingNote;
use crate::admin_api;
use crate::admin_state::{Origin, days_value, find_setting, origin, setting_label_key};
use crate::i18n::{t, use_i18n};
use crate::reauth::{gated, gated_settings, use_reauth};
use crate::settings::{Outcome, kept, loaded};

pub(super) type Data = Resource<Result<SecurityDto, ServerFnError>>;

/// The warnings for the top of the page (the https one stays next to the cookie switch).
#[component]
pub(super) fn SecurityWarnings(data: Data) -> impl IntoView {
    view! {
        <Transition fallback=|| ()>
            {move || data.get().and_then(Result::ok).map(|d| top_warnings(&d).into_iter().map(|w| view! {
                <Alert variant=AlertVariant::Warning role=AlertRole::None class="mb-6 max-w-3xl">
                    {t(&format!("admin-warning-{w}"))}
                </Alert>
            }).collect_view())}
        </Transition>
    }
}

/// "Advanced": the re-authentication window, session length and cookie security.
#[component]
pub(super) fn Advanced(data: Data) -> impl IntoView {
    view! {
        <Window/>
        {loaded(data, move |d| view! {
            <Sessions settings=d.settings.clone() warnings=d.warnings.clone() data=data/>
            <p class="max-w-2xl text-sm text-muted-foreground">
                {use_i18n().t_args("admin-security-window-rule", &[
                    ("slide", d.reauth_slide_minutes.into()),
                    ("cap", d.reauth_cap_minutes.into()),
                ])}
            </p>
        })}
    }
}

/// "Secrets at rest": the key ring.
#[component]
pub(super) fn SecretsAtRest(data: Data) -> impl IntoView {
    loaded(
        data,
        move |d| view! { <Secrets s=d.secrets.clone() data=data/> },
    )
}

/// The warnings at the top; the https one is shown next to the cookie switch instead
/// (UX2-W9).
fn top_warnings(d: &SecurityDto) -> Vec<String> {
    let beside_cookies = find_setting(&d.settings, "auth.secure_cookies").is_some();
    d.warnings
        .iter()
        .filter(|w| !(beside_cookies && w.as_str() == "public_url_not_https"))
        .cloned()
        .collect()
}

/// "Confirmed for 12 more minutes", or a way to confirm now (DEC-191).
#[component]
fn Window() -> impl IntoView {
    let reauth = use_reauth();
    // the minutes count down (UX2-W20): the server's answer again each minute
    let tick = RwSignal::new(0_u32);
    Effect::new(move |_| {
        if let Ok(h) = set_interval_with_handle(
            move || tick.update(|t| *t += 1),
            std::time::Duration::from_secs(60),
        ) {
            on_cleanup(move || h.clear());
        }
    });
    let status = Resource::new(
        move || (reauth.version.get(), tick.get()),
        |_| admin_api::reauth_status(),
    );
    view! {
        <section aria-labelledby="adm-window-h" class="mb-6 max-w-3xl rounded-xl border bg-card p-4">
            <h3 id="adm-window-h" class="mb-2 font-semibold">{t("admin-security-window")}</h3>
            {loaded(status, move |s| {
                let i18n = use_i18n();
                match s.minutes_left.filter(|_| s.fresh) {
                    Some(m) => view! {
                        <p data-testid="reauth-window" class="text-sm">{i18n.t_args("reauth-window-fresh", &[("minutes", m.into())])}</p>
                    }.into_any(),
                    None => view! {
                        <div class="flex flex-wrap items-center gap-3">
                            <p data-testid="reauth-window" class="text-sm">{t("reauth-window-stale")}</p>
                            <Button variant=ButtonVariant::Outline on:click=move |_| reauth.ask(None)>{t("reauth-confirm-now")}</Button>
                        </div>
                    }.into_any(),
                }
            })}
        </section>
    }
}

#[component]
fn Sessions(settings: Vec<SettingDto>, warnings: Vec<String>, data: Data) -> impl IntoView {
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let message = kept("security-sessions", || None::<Result<String, String>>);
    let i18n = use_i18n();
    let days = find_setting(&settings, "auth.session_days").cloned();
    let cookies = find_setting(&settings, "auth.secure_cookies").cloned();
    let days_text = RwSignal::new(
        days.as_ref()
            .map(|s| s.value.to_string())
            .unwrap_or_default(),
    );
    let secure = RwSignal::new(
        cookies
            .as_ref()
            .and_then(|s| s.value.as_bool())
            .unwrap_or(false),
    );
    let editable =
        |s: &Option<SettingDto>| s.as_ref().is_some_and(|s| origin(s) == Origin::Editable);
    let (days_free, cookies_free) = (editable(&days), editable(&cookies));
    let no_https = warnings.iter().any(|w| w == "public_url_not_https");
    let save = move || {
        let mut values = BTreeMap::new();
        if days_free {
            let Some(v) = days_value(&days_text.get_untracked()) else {
                message.set(Some(Err(i18n.t("admin-security-days-invalid"))));
                return;
            };
            values.insert("auth.session_days".to_string(), v);
        }
        if cookies_free {
            values.insert(
                "auth.secure_cookies".to_string(),
                Value::Bool(secure.get_untracked()),
            );
        }
        let saved = i18n.t("admin-saved");
        gated_settings(
            reauth,
            busy,
            values,
            Callback::new(move |r: Result<_, String>| {
                message.set(Some(r.map(|_| saved.clone())));
                data.refetch();
            }),
        );
    };
    view! {
        <form class="mb-8 grid max-w-2xl gap-5" on:submit=move |ev| { ev.prevent_default(); save(); }>
            <h3 class="font-semibold">{t("admin-security-sessions")}</h3>
            {days.map(|s| view! {
                <div class="grid max-w-sm gap-1">
                    <FormField id="adm-session-days" label=t(&setting_label_key(&s.key)) description=t("admin-security-days-hint")>
                        <Input id="adm-session-days" value=days_text inputmode="numeric" readonly=!days_free
                            described_by=field_described_by("adm-session-days")/>
                    </FormField>
                    <SettingNote s=s/>
                </div>
            })}
            {cookies.map(|s| { let label = t(&setting_label_key(&s.key)); view! {
                <div class="grid gap-2">
                    <div class="flex items-center gap-3">
                        <Switch id="adm-secure-cookies" checked=secure attr:disabled=!cookies_free
                            attr:aria-describedby="adm-secure-cookies-hint"/>
                        <Label r#for="adm-secure-cookies">{label}</Label>
                    </div>
                    <p id="adm-secure-cookies-hint" class="text-sm text-muted-foreground">{t("admin-security-cookies-hint")}</p>
                    {no_https.then(|| view! {
                        <Alert variant=AlertVariant::Warning role=AlertRole::None>{t("admin-warning-public_url_not_https")}</Alert>
                    })}
                    <SettingNote s=s/>
                </div>
            }})}
            <Outcome message=message/>
            {(days_free || cookies_free).then(|| view! { <Button r#type="submit" class="w-fit" loading=busy>{t("admin-save")}</Button> })}
        </form>
    }
}

#[component]
fn Secrets(s: SecretsStatusDto, data: Data) -> impl IntoView {
    let i18n = use_i18n();
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let message = kept("security-secrets", || None::<Result<String, String>>);
    let texts = i18n.clone();
    let rewrap = move |_| {
        let texts = texts.clone();
        gated(
            reauth,
            busy,
            admin_api::rewrap_secrets,
            Callback::new(move |r: Result<contracts::admin::RewrapDto, String>| {
                message.set(Some(r.map(|r| {
                    texts.t_args(
                        "admin-secrets-rewrapped",
                        &[("count", r.rewrapped.into()), ("locked", r.locked.into())],
                    )
                })));
                data.refetch();
            }),
        );
    };
    let source = t(match s.key_source.as_str() {
        "file" => "admin-secrets-from-file",
        "env" => "admin-secrets-from-env",
        "generated" => "admin-secrets-generated",
        _ => "admin-secrets-ephemeral",
    });
    let count = s.versions.len();
    let rotation = i18n.t_args(
        "admin-health-secrets-rotation_due",
        &[("version", s.active_version.into())],
    );
    let locked = i18n.t_args(
        "admin-health-secrets-locked",
        &[("count", s.locked_rows.into())],
    );
    let version = i18n.t_args(
        "admin-secrets-version",
        &[("version", s.active_version.into())],
    );
    let versions_summary =
        i18n.t_args("admin-secrets-versions-summary", &[("count", count.into())]);
    view! {
        <div class="grid max-w-3xl gap-3">
            {s.rotation_due.then(|| view! {
                <Alert variant=AlertVariant::Info role=AlertRole::None>{rotation}</Alert>
            })}
            {(s.locked_rows > 0).then(|| view! {
                <Alert variant=AlertVariant::Destructive role=AlertRole::None>{locked}</Alert>
            })}
            <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
                <dt class="font-medium">{t("admin-secrets-active")}</dt>
                <dd>{version}</dd>
                <dt class="font-medium">{t("admin-secrets-source")}</dt><dd>{source}</dd>
                <dt class="font-medium">{t("admin-secrets-file")}</dt><dd class="break-all font-mono">{s.key_file.clone()}</dd>
                <dt class="font-medium">{t("admin-secrets-old")}</dt><dd>{s.rows_to_rewrap}</dd>
            </dl>
            <Button variant=ButtonVariant::Outline class="w-fit" loading=busy on:click=rewrap>{t("admin-secrets-rewrap")}</Button>
            <Outcome message=message/>
            <Disclosure id="adm-key-versions" title=t("admin-secrets-versions")
                summary=versions_summary>
                <Table>
                    <caption class="sr-only">{t("admin-secrets-versions")}</caption>
                    <TableHeader><TableRow>
                        <TableHead>{t("admin-secrets-key")}</TableHead>
                        <TableHead>{t("admin-secrets-values")}</TableHead>
                        <TableHead>{t("admin-secrets-state")}</TableHead>
                    </TableRow></TableHeader>
                    <TableBody>
                        {s.versions.into_iter().map(|v| {
                            let state = t(if v.active {
                                "admin-secrets-state-active"
                            } else if !v.loaded {
                                "admin-secrets-state-missing"
                            } else if v.removable {
                                "admin-secrets-state-removable"
                            } else {
                                "admin-secrets-state-loaded"
                            });
                            view! {
                                <TableRow>
                                    <TableCell>{format!("v{}", v.version)}</TableCell>
                                    <TableCell>{v.rows}</TableCell>
                                    <TableCell>{state}</TableCell>
                                </TableRow>
                            }
                        }).collect_view()}
                    </TableBody>
                </Table>
            </Disclosure>
        </div>
    }
}
