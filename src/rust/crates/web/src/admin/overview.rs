//! Admin > Overview (addendum-1 3.4): what needs attention first, the eight health rows
//! (dot + word + one line + at most one action), recent admin actions, recent sign-ups
//! and the brand card.

use contracts::admin::{AdminOverviewDto, AdminUserDto, BrandCardDto, HealthRowDto, KeyBackupDto};
use fluent_bundle::FluentValue;
use leptos::prelude::*;
use ui::{Alert, AlertRole, AlertVariant, Button, StatusDot};

use super::{AdminFrame, Attention, AuditTable};
use crate::admin_api;
use crate::admin_state::health_state;
use crate::i18n::{I18n, t, use_i18n};
use crate::reauth::{gated, use_reauth};
use crate::settings::{Outcome, loaded};

#[component]
pub fn Overview() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-overview") lead=t("admin-overview-lead")>
            <OverviewBody/>
        </AdminFrame>
    }
}

/// The row's name (`admin-health-name-<id>`).
fn health_name(id: &str) -> String {
    t(match id {
        "database" => "admin-health-name-database",
        "search" => "admin-health-name-search",
        "storage" => "admin-health-name-storage",
        "jobs" => "admin-health-name-jobs",
        "mail" => "admin-health-name-mail",
        "data_sources" => "admin-health-name-data_sources",
        "identity_provider" => "admin-health-name-identity_provider",
        "reverse_proxy" => "admin-health-name-reverse_proxy",
        _ => "admin-health-name-secrets",
    })
}

/// The status word next to the dot (never the dot alone, WCAG 1.4.1).
fn status_word(status: &str) -> String {
    t(match status {
        "ok" => "admin-status-ok",
        "warning" => "admin-status-warning",
        "error" => "admin-status-error",
        "unknown" => "admin-status-unknown",
        _ => "admin-status-off",
    })
}

/// `YYYY-MM-DDTHH:MM` in the admin's zone (the server computes it, DEC-336) in the locale's
/// date and time format.
fn local_stamp(i18n: &I18n, s: &str) -> Option<String> {
    let day = s.get(0..10)?;
    let h: u32 = s.get(11..13)?.parse().ok()?;
    let m: u32 = s.get(14..16)?.parse().ok()?;
    Some(i18n.t_args(
        "datetime-local",
        &[
            ("date", i18n.date(day).into()),
            ("time", i18n.time(h, m).into()),
        ],
    ))
}

/// The one-line detail (`admin-health-<id>-<detail>` with the server's arguments); a time
/// argument shows in the admin's zone. A row the probe job has not written yet says so.
fn health_detail(h: &HealthRowDto) -> String {
    let i18n = use_i18n();
    if h.status == "unknown" {
        return i18n.t("admin-health-not-checked");
    }
    let local = h.args.get("at_local").and_then(|l| local_stamp(&i18n, l));
    let args: Vec<(&str, FluentValue<'_>)> = h
        .args
        .iter()
        .filter(|(k, _)| k.as_str() != "at_local")
        .map(|(k, v)| match (k.as_str(), &local) {
            ("at", Some(l)) => ("at", FluentValue::from(l.clone())),
            // the mail test's outcome is a category (`connection`, `auth`, ...), in words here
            ("result", _) if h.id == "mail" => (
                "result",
                FluentValue::from(i18n.t(&format!("admin-health-mail-result-{v}"))),
            ),
            _ => (k.as_str(), FluentValue::from(v.as_str())),
        })
        .collect();
    i18n.t_args(&format!("admin-health-{}-{}", h.id, h.detail), &args)
}

/// "Not checked recently" with the time of the last check, for a row older than three
/// minutes (the probe job is not running).
fn stale_note(h: &HealthRowDto) -> Option<String> {
    if !h.stale {
        return None;
    }
    let i18n = use_i18n();
    let at = h
        .checked_local
        .as_deref()
        .and_then(|l| local_stamp(&i18n, l))
        .unwrap_or_default();
    Some(i18n.t_args("admin-health-stale", &[("time", at.into())]))
}

/// Where a row's one action leads (rows without a page of their own have none).
fn health_action(id: &str) -> Option<&'static str> {
    match id {
        "storage" | "jobs" => Some("/admin/storage"),
        "data_sources" => Some("/admin/sources"),
        "mail" | "secrets" => Some("/admin/instance"),
        "identity_provider" => Some("/admin/sso"),
        _ => None,
    }
}

/// The block that says "Recorded" after the key backup was confirmed. It takes the focus
/// then: the reminder and its button, which had the focus, leave the page (review V1OPS-8,
/// WCAG 2.4.3).
const BACKUP_OUTCOME: &str = "adm-key-backup-outcome";
/// The same block once it shows its message (it is hidden while empty and cannot take the
/// focus then).
const BACKUP_OUTCOME_SHOWN: &str = "#adm-key-backup-outcome:not(:empty)";

#[component]
fn OverviewBody() -> impl IntoView {
    let data = Resource::new(|| (), |()| admin_api::admin_overview());
    let users = Resource::new(|| (), |()| admin_api::admin_users());
    // outlives the refetch that removes the reminder: "recorded" is said where it was
    let backup_message = RwSignal::new(None::<Result<String, String>>);
    view! {
        <div id=BACKUP_OUTCOME tabindex="-1" class="mb-6 max-w-3xl empty:hidden"><Outcome message=backup_message/></div>
        {loaded(data, move |o| {
            let attention: Vec<HealthRowDto> = o.health.iter().filter(|h| o.attention.contains(&h.id)).cloned().collect();
            let title = use_i18n().t_args("admin-overview-attention", &[("count", attention.len().into())]);
            view! {
                {(!attention.is_empty()).then(|| view! {
                    <Attention title=title>
                        <ul class="grid gap-1">
                            {attention.into_iter().map(|h| view! {
                                <li>
                                    <span class="font-medium">{health_name(&h.id)}</span> ": " {health_detail(&h)}
                                    {health_action(&h.id).map(|href| view! { " " <a class="underline" href=href>{t("admin-overview-fix")}</a> })}
                                </li>
                            }).collect_view()}
                        </ul>
                    </Attention>
                })}
                {o.key_backup.clone().map(|k| view! { <KeyBackup k=k data=data message=backup_message/> })}
                {o.hosted_banner.then(|| view! {
                    <Alert variant=AlertVariant::Info role=AlertRole::None class="mb-6 max-w-3xl">
                        {t("admin-overview-hosted")} " " <a class="underline" href="/admin/sources">{t("admin-nav-sources")}</a>
                    </Alert>
                })}
                <section aria-labelledby="adm-health-h" class="mb-8 max-w-3xl">
                    <h2 id="adm-health-h" class="mb-3 text-lg font-semibold">{t("admin-overview-health")}</h2>
                    <ul class="grid gap-2">
                        {o.health.iter().map(|h| view! { <HealthRow h=h.clone()/> }).collect_view()}
                    </ul>
                </section>
                <div class="grid max-w-3xl grid-cols-[minmax(0,1fr)] gap-8">
                    <section aria-labelledby="adm-recent-h">
                        <h2 id="adm-recent-h" class="mb-3 text-lg font-semibold">{t("admin-overview-recent")}</h2>
                        {if o.recent.is_empty() {
                            view! { <p class="text-sm text-muted-foreground">{t("admin-overview-recent-none")}</p> }.into_any()
                        } else {
                            view! { <AuditTable entries=o.recent.clone() caption=t("admin-overview-recent")/> }.into_any()
                        }}
                        <p class="mt-2 text-sm"><a class="text-primary underline" href="/admin/audit">{t("admin-overview-audit")}</a></p>
                    </section>
                    <BrandCard b=o.brand.clone()/>
                </div>
            }
        })}
        <section aria-labelledby="adm-signups-h" class="mt-8 max-w-3xl">
            <h2 id="adm-signups-h" class="mb-3 text-lg font-semibold">{t("admin-overview-signups")}</h2>
            {loaded(users, |list| view! { <RecentSignups users=list/> })}
        </section>
    }
}

/// The reminder to back up the secrets key (OPS-4, DEC-620): shown until an admin confirms
/// the backup of the active key version. Without the key a database backup cannot open the
/// stored secrets, and nothing regenerates it. The confirmation is a gated action.
#[component]
fn KeyBackup(
    k: KeyBackupDto,
    data: Resource<Result<AdminOverviewDto, ServerFnError>>,
    message: RwSignal<Option<Result<String, String>>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let location = match &k.key_file {
        Some(path) => i18n.t_args("admin-key-backup-file", &[("path", path.clone().into())]),
        None => i18n.t("admin-key-backup-env"),
    };
    let confirm = move |_| {
        let texts = i18n.clone();
        gated(
            reauth,
            busy,
            admin_api::confirm_key_backup,
            Callback::new(move |r: Result<(), String>| {
                let recorded = r.is_ok();
                message.set(Some(r.map(|()| texts.t("admin-key-backup-confirmed"))));
                data.refetch();
                if recorded {
                    // the refetch removes the reminder and the button that has the focus
                    crate::keyboard::when_present(BACKUP_OUTCOME_SHOWN, || {
                        let _ = crate::keyboard::focus(BACKUP_OUTCOME_SHOWN);
                    });
                }
            }),
        );
    };
    view! {
        <div data-testid="key-backup">
            <Alert variant=AlertVariant::Warning role=AlertRole::None class="mb-6 max-w-3xl"
                title=t("admin-key-backup-title")
                actions=move || view! {
                    <Button loading=busy on:click=confirm.clone()>{t("admin-key-backup-confirm")}</Button>
                }>
                <p>{t("admin-key-backup-body")}</p>
                <p class="mt-1 break-all">{location}</p>
            </Alert>
        </div>
    }
}

#[component]
fn HealthRow(h: HealthRowDto) -> impl IntoView {
    let name = health_name(&h.id);
    let label = use_i18n().t_args("admin-overview-open", &[("name", name.clone().into())]);
    view! {
        <li data-testid="health-row" class="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-xl border bg-card px-4 py-3">
            <span class="font-medium sm:w-40 sm:shrink-0">{name}</span>
            <StatusDot state=health_state(&h.status) label=status_word(&h.status) class="sm:w-36 sm:shrink-0"/>
            <span class="min-w-0 flex-1 text-sm text-muted-foreground">
                {health_detail(&h)}
                {stale_note(&h).map(|s| view! { <span data-testid="health-stale" class="block font-medium text-foreground">{s}</span> })}
            </span>
            {health_action(&h.id).map(|href| view! {
                <a class="text-sm text-primary underline" href=href aria-label=label>{t("admin-overview-details")}</a>
            })}
        </li>
    }
}

/// The three newest accounts (design.md 6.13 "recent sign-ups").
#[component]
fn RecentSignups(users: Vec<AdminUserDto>) -> impl IntoView {
    let i18n = use_i18n();
    let mut users = users;
    users.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    view! {
        <ul class="grid gap-1 text-sm">
            {users.into_iter().take(3).map(|u| view! {
                <li>
                    <span class="font-medium">{u.display_name.clone()}</span> " "
                    <span class="font-mono text-muted-foreground">{u.username.clone()}</span> " - "
                    {i18n.datetime(&u.created_at)}
                </li>
            }).collect_view()}
        </ul>
        <p class="mt-2 text-sm"><a class="text-primary underline" href="/admin/users">{t("admin-nav-users")}</a></p>
    }
}

/// The brand, read-only (addendum-1 3.4: the Branding page became this card): name, short
/// name, locale, where it is configured, and the accent of both themes with its contrast.
#[component]
fn BrandCard(b: BrandCardDto) -> impl IntoView {
    let i18n = use_i18n();
    let edit = i18n.t_args("admin-brand-edit", &[("dir", b.dir.clone().into())]);
    view! {
        <section aria-labelledby="adm-brand-h" class="rounded-xl border bg-card p-4 text-sm">
            <h2 id="adm-brand-h" class="mb-3 text-lg font-semibold">{t("admin-overview-brand-title")}</h2>
            <dl class="mb-4 grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-1">
                <dt class="font-medium">{t("admin-brand-name")}</dt><dd>{b.name.clone()}</dd>
                <dt class="font-medium">{t("admin-brand-short-name")}</dt><dd class="font-mono">{b.short_name.clone()}</dd>
                <dt class="font-medium">{t("admin-brand-locale")}</dt><dd>{contracts::locale::locale_name(&b.default_locale).to_string()}</dd>
                <dt class="font-medium">{t("admin-brand-dir")}</dt><dd class="break-all font-mono">{b.dir.clone()}</dd>
            </dl>
            <h3 class="mb-2 font-semibold">{t("admin-brand-accent")}</h3>
            <ul class="mb-3 grid grid-cols-[minmax(0,1fr)] gap-3 sm:grid-cols-2">
                {b.swatches.into_iter().map(|s| view! { <super::Swatch s=s/> }).collect_view()}
            </ul>
            <p class="break-all text-muted-foreground">{edit}</p>
        </section>
    }
}
