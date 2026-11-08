//! The admin area (addendum-1 3.4, design.md 6.13): eight pages in the settings side nav,
//! each with the anatomy H1, lead, attention block (only when something needs attention,
//! D14), open set, disclosures. Members get a clear "admins only" page and no data; the
//! server refuses them anyway (`forbidden`).

mod audit;
mod instance;
mod overview;
mod security;
mod sources;
mod sso;
mod storage;
mod users;

use contracts::admin::{AuditEntryDto, SettingDto};
use leptos::prelude::*;
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Button, ButtonVariant, Dialog, Disclosure,
    Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
};

pub use audit::Audit;
pub use instance::Instance;
pub use overview::Overview;
pub use sources::Sources;
pub(crate) use sources::{
    Credential as SourceCredential, KeyHowTo as SourceKeyHowTo, Licence as SourceLicence,
    Sources as SourcesResource,
};
pub use sso::SignInSso;
pub use storage::Storage;
pub use users::Users;

use crate::admin_api;
use crate::admin_state::{Origin, audit_action_key, audit_detail, origin};
use crate::app::ClientConfig;
use crate::i18n::{t, use_i18n};
use crate::pages::PageTitle;
use crate::settings::{Lead, SettingsLayout, loaded};

/// The frame of every admin page: the page for admins, the refusal for everyone else.
#[component]
fn AdminFrame(
    #[prop(into)] title: String,
    #[prop(into)] lead: String,
    children: Children,
) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    // signed out: the refusal alone, no "You" side nav (UX2-W21)
    if !cfg.signed_in {
        return view! { <AdminOnly signed_in=false/> }.into_any();
    }
    let body = if cfg.is_admin {
        view! {
            <div data-testid="admin-page">
                <PageTitle text=title/>
                <Lead text=lead/>
                {children()}
            </div>
        }
        .into_any()
    } else {
        view! { <AdminOnly signed_in=true/> }.into_any()
    };
    view! { <SettingsLayout>{body}</SettingsLayout> }.into_any()
}

#[component]
fn AdminOnly(signed_in: bool) -> impl IntoView {
    let (lead, href, back) = if signed_in {
        (
            t("admin-only-lead"),
            "/settings/account",
            t("admin-only-back"),
        )
    } else {
        (
            t("admin-only-lead-signed-out"),
            "/sign-in",
            t("nav-sign-in"),
        )
    };
    view! {
        <section data-testid="admin-only" class="max-w-xl py-8">
            <PageTitle text=t("admin-only-title")/>
            <p class="text-muted-foreground">{lead}</p>
            <p class="mt-4"><a class="underline" href=href>{back}</a></p>
        </section>
    }
}

/// Where a read-only or shadowed value comes from, and when a change applies (addendum-1
/// 3.2, OD-10/OD-11).
#[component]
fn SettingNote(s: SettingDto) -> impl IntoView {
    let i18n = use_i18n();
    let source = match origin(&s) {
        Origin::Env(var) => Some(i18n.t_args("admin-setting-env", &[("var", var.into())])),
        Origin::File => Some(i18n.t("admin-setting-file")),
        Origin::Editable => None,
    };
    let shadowed = s.shadowed_value.as_ref().map(|_| {
        i18n.t_args(
            "admin-setting-shadowed",
            &[("var", s.env_name.clone().into())],
        )
    });
    let next = s
        .applies_next_session
        .then(|| i18n.t("admin-setting-next-session"));
    view! {
        <div class="grid gap-1 text-sm text-muted-foreground">
            {source.map(|v| view! {
                <p class="flex flex-wrap items-center gap-2">
                    <Badge variant=BadgeVariant::Outline>{t("admin-setting-read-only")}</Badge>
                    <span data-testid="setting-origin">{v}</span>
                </p>
            })}
            {shadowed.map(|v| view! { <p>{v}</p> })}
            {next.map(|v| view! { <p>{v}</p> })}
        </div>
    }
}

/// Confirmation for a destructive action (D15: dialogs only for these, re-auth and what
/// the person asked for).
#[component]
pub(crate) fn ConfirmDialog(
    open: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] body: Signal<String>,
    #[prop(into)] confirm: String,
    on_confirm: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let confirm = StoredValue::new(confirm);
    view! {
        {move || {
            let close = i18n.t("dialog-close");
            let cancel = StoredValue::new(i18n.t("reauth-cancel"));
            view! {
                <Dialog open=open title=title.get() description=body.get() close_label=close>
                    <div class="flex flex-wrap justify-end gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| open.set(false)>{cancel.get_value()}</Button>
                        <Button variant=ButtonVariant::Destructive on:click=move |_| { open.set(false); on_confirm.run(()); }>
                            {confirm.get_value()}
                        </Button>
                    </div>
                </Dialog>
            }
        }}
    }
}

/// Who did an audited action: the person, `system` or `cli`.
fn audit_who(e: &AuditEntryDto) -> String {
    e.actor_name
        .clone()
        .unwrap_or_else(|| match e.actor_kind.as_str() {
            "cli" => t("admin-audit-actor-cli"),
            _ => t("admin-audit-actor-system"),
        })
}

/// What was done, in words; an action this build has no words for shows its id.
fn audit_what(e: &AuditEntryDto) -> String {
    let key = audit_action_key(&e.action);
    let text = t(&key);
    if text == key { e.action.clone() } else { text }
}

/// The audit table (Overview's recent actions and the Audit log page).
#[component]
fn AuditTable(entries: Vec<AuditEntryDto>, #[prop(into)] caption: String) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <Table attr:data-testid="audit-table">
            <caption class="sr-only">{caption}</caption>
            <TableHeader>
                <TableRow>
                    <TableHead>{t("admin-audit-when")}</TableHead>
                    <TableHead>{t("admin-audit-who")}</TableHead>
                    <TableHead>{t("admin-audit-what")}</TableHead>
                    <TableHead>{t("admin-audit-detail")}</TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {entries.into_iter().map(|e| {
                    let (when, who, what, detail) = (i18n.datetime(&e.at), audit_who(&e), audit_what(&e), audit_detail(&e.detail));
                    view! {
                        <TableRow attr:data-testid="audit-row">
                            <TableCell class="sm:whitespace-nowrap">{when}</TableCell>
                            <TableCell>{who}</TableCell>
                            <TableCell>{what}</TableCell>
                            <TableCell class="break-all text-muted-foreground">{detail}</TableCell>
                        </TableRow>
                    }
                }).collect_view()}
            </TableBody>
        </Table>
    }
}

// ---------------------------------------------------------------- notification defaults

/// Admin > Notification defaults (design.md 6.13): the instance rules, read-only here;
/// members change their own rules under Settings > Notifications.
#[component]
pub fn NotificationDefaults() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-notifications") lead=t("admin-defaults-lead")>
            <DefaultsBody/>
        </AdminFrame>
    }
}

#[component]
fn DefaultsBody() -> impl IntoView {
    let data = Resource::new(|| (), |()| admin_api::notification_defaults());
    loaded(data, |d| {
        let count = d.events.len();
        view! {
            <Table attr:data-testid="defaults-table" class="max-w-3xl">
                <caption class="sr-only">{t("admin-nav-notifications")}</caption>
                <TableHeader>
                    <TableRow>
                        <TableHead>{t("admin-defaults-event")}</TableHead>
                        <TableHead>{t("admin-defaults-delivery")}</TableHead>
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {d.defaults.is_empty().then(|| view! {
                        <TableRow><TableCell attr:colspan="2" class="text-muted-foreground">{t("admin-defaults-none")}</TableCell></TableRow>
                    })}
                    {d.defaults.iter().map(|s| (crate::notifications::event_label(&s.event), crate::notifications::mode_label(s))).collect::<Vec<_>>().into_iter().map(|(event, mode)| view! {
                        <TableRow>
                            <TableCell>{event}</TableCell>
                            <TableCell>{mode}</TableCell>
                        </TableRow>
                    }).collect_view()}
                </TableBody>
            </Table>
            <Disclosure id="adm-events" class="mt-6 max-w-3xl" title=t("admin-defaults-events")
                summary=use_i18n().t_args("admin-defaults-events-summary", &[("count", count.into())])>
                <ul class="grid gap-1 text-sm">
                    {d.events.into_iter().map(|e| {
                        let who = if e.audience == "admin" { t("admin-defaults-for-admins") } else { t("admin-defaults-for-everyone") };
                        view! { <li>{crate::notifications::event_label(&e.id)} " - " <span class="text-muted-foreground">{who}</span></li> }
                    }).collect_view()}
                </ul>
            </Disclosure>
        }
    })
}

// ---------------------------------------------------------------- brand card

/// One theme's accent with its contrast (the brand card on Overview, design.md 6.13).
#[component]
fn Swatch(s: contracts::admin::BrandSwatchDto) -> impl IntoView {
    let i18n = use_i18n();
    let theme = if s.theme == "dark" {
        t("admin-brand-dark")
    } else {
        t("admin-brand-light")
    };
    let (ratio, ok) = match s.ratio {
        Some(r) => (
            i18n.t_args("admin-brand-ratio", &[("ratio", format!("{r:.2}").into())]),
            r >= 4.5,
        ),
        None => (t("admin-brand-unreadable"), false),
    };
    let verdict = if ok {
        t("admin-brand-pass")
    } else {
        t("admin-brand-fail")
    };
    let style = format!("background-color: {}; color: {}", s.primary, s.foreground);
    view! {
        <li data-testid="brand-swatch" class="flex min-w-0 items-center gap-3 rounded-xl border bg-card p-3">
            <span aria-hidden="true" class="inline-flex size-12 shrink-0 items-center justify-center rounded-md border text-lg font-semibold" style=style>
                {t("admin-brand-sample")}
            </span>
            <span class="grid min-w-0 gap-0.5 text-sm">
                <span class="font-medium">{theme}</span>
                <span class="break-all font-mono text-xs text-muted-foreground">{format!("{} / {}", s.primary, s.foreground)}</span>
                <span>{ratio} " " {if ok {
                    view! { <Badge variant=BadgeVariant::Success>{verdict}</Badge> }.into_any()
                } else {
                    view! { <Badge variant=BadgeVariant::Warning>{verdict}</Badge> }.into_any()
                }}</span>
            </span>
        </li>
    }
}

/// An attention block (D14): only rendered when there is something to say.
#[component]
fn Attention(#[prop(into)] title: String, children: Children) -> impl IntoView {
    view! {
        <Alert variant=AlertVariant::Warning role=AlertRole::None class="mb-6 max-w-3xl" title=title>
            {children()}
        </Alert>
    }
}
