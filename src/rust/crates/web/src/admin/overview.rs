//! Admin > Overview (addendum-1 3.4): what needs attention first, the eight health rows
//! (dot + word + one line + at most one action), recent admin actions, recent sign-ups
//! and the brand card.

use contracts::admin::{AdminUserDto, BrandCardDto, HealthRowDto};
use fluent_bundle::FluentValue;
use leptos::prelude::*;
use ui::{Alert, AlertRole, AlertVariant, StatusDot};

use super::{AdminFrame, Attention, AuditTable};
use crate::admin_api;
use crate::admin_state::health_state;
use crate::i18n::{t, use_i18n};
use crate::settings::loaded;

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
        _ => "admin-health-name-secrets",
    })
}

/// The status word next to the dot (never the dot alone, WCAG 1.4.1).
fn status_word(status: &str) -> String {
    t(match status {
        "ok" => "admin-status-ok",
        "warning" => "admin-status-warning",
        "error" => "admin-status-error",
        _ => "admin-status-off",
    })
}

/// The one-line detail (`admin-health-<id>-<detail>` with the server's arguments).
fn health_detail(h: &HealthRowDto) -> String {
    let args: Vec<(&str, FluentValue<'_>)> = h
        .args
        .iter()
        .map(|(k, v)| (k.as_str(), FluentValue::from(v.as_str())))
        .collect();
    use_i18n().t_args(&format!("admin-health-{}-{}", h.id, h.detail), &args)
}

/// Where a row's one action leads (rows without a page of their own have none).
fn health_action(id: &str) -> Option<&'static str> {
    match id {
        "storage" | "jobs" => Some("/admin/storage"),
        "data_sources" => Some("/admin/sources"),
        "secrets" => Some("/admin/security"),
        _ => None,
    }
}

#[component]
fn OverviewBody() -> impl IntoView {
    let data = Resource::new(|| (), |()| admin_api::admin_overview());
    let users = Resource::new(|| (), |()| admin_api::admin_users());
    view! {
        {loaded(data, |o| {
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
                        <p class="mt-2 text-sm"><a class="underline" href="/admin/audit">{t("admin-overview-audit")}</a></p>
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

#[component]
fn HealthRow(h: HealthRowDto) -> impl IntoView {
    let name = health_name(&h.id);
    let label = use_i18n().t_args("admin-overview-open", &[("name", name.clone().into())]);
    view! {
        <li data-testid="health-row" class="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-xl border bg-card px-4 py-3">
            <span class="font-medium sm:w-40 sm:shrink-0">{name}</span>
            <StatusDot state=health_state(&h.status) label=status_word(&h.status) class="sm:w-36 sm:shrink-0"/>
            <span class="min-w-0 flex-1 text-sm text-muted-foreground">{health_detail(&h)}</span>
            {health_action(&h.id).map(|href| view! {
                <a class="text-sm underline" href=href aria-label=label>{t("admin-overview-details")}</a>
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
        <p class="mt-2 text-sm"><a class="underline" href="/admin/users">{t("admin-nav-users")}</a></p>
    }
}

#[component]
fn BrandCard(b: BrandCardDto) -> impl IntoView {
    let line = use_i18n().t_args(
        "admin-overview-brand",
        &[
            ("name", b.name.clone().into()),
            (
                "locale",
                contracts::locale::locale_name(&b.default_locale).into(),
            ),
        ],
    );
    view! {
        <section aria-labelledby="adm-brand-h" class="rounded-xl border bg-card p-4 text-sm">
            <h2 id="adm-brand-h" class="mb-2 text-lg font-semibold">{t("admin-nav-brand")}</h2>
            <p>{line}</p>
            <p class="mt-2"><a class="underline" href="/admin/brand">{t("admin-overview-brand-link")}</a></p>
        </section>
    }
}
