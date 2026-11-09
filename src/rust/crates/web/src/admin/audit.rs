//! Admin > Audit log (addendum-1 3.4, OD-13): every admin action, who did it and the
//! detail without secret values; filters by person and action live in the URL (a plain GET
//! form, so it works before hydration and links can be shared); Export CSV with the same
//! filters. Rows are kept for 365 days.

use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use ui::Button;

use super::{AdminFrame, AuditTable};
use crate::admin_api;
use crate::admin_state::{AuditFilter, audit_action_key};
use crate::i18n::t;
use crate::settings::loaded;

/// Rows per page of the log.
const PAGE: u32 = 100;

/// The actions the filter offers (the audited set of addendum-1 3.4).
const ACTIONS: [&str; 19] = [
    "setting.changed",
    "secret.replaced",
    "secret.cleared",
    "secrets.rewrapped",
    "role.changed",
    "user.disabled",
    "user.enabled",
    "user.deleted",
    "invite.created",
    "invite.revoked",
    "licence.recorded",
    "wizard.completed",
    "eviction.run",
    "job.retried",
    "job.discarded",
    "user.totp_reset",
    "user.identity_unlinked",
    "auth.link_emailed",
    "auth.link_confirmed",
];

#[component]
pub fn Audit() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-audit") lead=t("admin-audit-lead")>
            <AuditBody/>
        </AdminFrame>
    }
}

#[component]
fn AuditBody() -> impl IntoView {
    let query = use_query_map();
    let filter = Memo::new(move |_| {
        query.with(|q| {
            let (actor, action) = (
                q.get("actor").unwrap_or_default(),
                q.get("action").unwrap_or_default(),
            );
            AuditFilter::from_pairs([("actor", actor.as_str()), ("action", action.as_str())])
        })
    });
    let rows = Resource::new(
        move || filter.get(),
        |f| admin_api::audit_log(f.to_dto(PAGE)),
    );
    let users = Resource::new(|| (), |()| admin_api::admin_users());
    view! {
        <form method="get" action="/admin/audit" class="mb-4 flex max-w-3xl flex-wrap items-end gap-3">
            <div class="grid gap-2">
                <label for="adm-audit-actor" class="text-sm font-medium">{t("admin-audit-filter-who")}</label>
                <select id="adm-audit-actor" name="actor" class="h-9 rounded-md border border-input bg-background px-3 text-sm">
                    <option value="">{t("admin-audit-anyone")}</option>
                    <Suspense fallback=|| ()>
                        {move || users.get().and_then(Result::ok).map(|list| list.into_iter().map(|u| {
                            let selected = filter.with_untracked(|f| f.actor == u.id);
                            view! { <option value=u.id.clone() selected=selected>{u.display_name.clone()}</option> }
                        }).collect_view())}
                    </Suspense>
                </select>
            </div>
            <div class="grid gap-2">
                <label for="adm-audit-action" class="text-sm font-medium">{t("admin-audit-filter-what")}</label>
                <select id="adm-audit-action" name="action" class="h-9 rounded-md border border-input bg-background px-3 text-sm">
                    <option value="">{t("admin-audit-anything")}</option>
                    {ACTIONS.iter().map(|a| {
                        let selected = filter.with_untracked(|f| f.action == *a);
                        view! { <option value=*a selected=selected>{t(&audit_action_key(a))}</option> }
                    }).collect_view()}
                </select>
            </div>
            <Button r#type="submit">{t("admin-audit-apply")}</Button>
            <a class="inline-flex h-9 items-center text-sm underline" href=move || filter.get().csv_href()>{t("admin-audit-csv")}</a>
        </form>
        {loaded(rows, |page| if page.entries.is_empty() {
            view! { <p class="text-sm text-muted-foreground">{t("admin-audit-none")}</p> }.into_any()
        } else {
            view! { <div class="max-w-5xl"><AuditTable entries=page.entries caption=t("admin-nav-audit")/></div> }.into_any()
        })}
    }
}
