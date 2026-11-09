//! Admin > Storage and jobs: "Usage measurement" (WP4-5, design.md 9, research R-54,
//! addendum-1 D16). What the logging funnel counts, the `usage.funnel` switch (read-only with
//! its variable when env or file set it) and the totals of the last 28 days per flow and
//! outcome, with the daily rows as CSV. The totals load when the section opens.

use std::collections::BTreeMap;

use contracts::admin::SettingDto;
use contracts::usage::UsageReportDto;
use leptos::prelude::*;
use ui::{
    Button, Disclosure, DisclosureMode, Switch, Table, TableBody, TableCell, TableHead,
    TableHeader, TableRow, disclosure_storage_key,
};

use super::SettingNote;
use crate::admin_api;
use crate::admin_state::{Origin, find_setting, origin};
use crate::i18n::{t, use_i18n};
use crate::reauth::{gated, use_reauth};
use crate::settings::{Outcome, kept};

/// The setting's key.
const KEY: &str = "usage.funnel";

/// Totals per (flow, subject, outcome) over the report's days, in that order.
#[must_use]
pub fn totals(r: &UsageReportDto) -> Vec<((String, String, String), u64)> {
    let mut m: BTreeMap<(String, String, String), u64> = BTreeMap::new();
    for row in &r.rows {
        *m.entry((row.flow.clone(), row.subject.clone(), row.outcome.clone()))
            .or_default() += row.n;
    }
    m.into_iter().collect()
}

#[component]
pub fn UsageMeasurement(settings: Vec<SettingDto>) -> impl IntoView {
    let open = RwSignal::new(false);
    let report = RwSignal::new(None::<Result<UsageReportDto, String>>);
    let i18n = use_i18n();
    Effect::new(move |_| {
        if open.get() && report.with_untracked(Option::is_none) {
            let failed = i18n.t("error-generic");
            leptos::task::spawn_local(async move {
                let r = crate::rest::call::<UsageReportDto>("GET", "/admin/usage", None).await;
                report.set(Some(match r {
                    Ok(Some(r)) => Ok(r),
                    _ => Err(failed),
                }));
            });
        }
    });
    let setting = find_setting(&settings, KEY).cloned();
    view! {
        <Disclosure id="adm-usage" title=t("admin-usage-title") summary=t("admin-usage-summary")
            mode=DisclosureMode::Button level=2 open=open storage_key=disclosure_storage_key("storage", "usage")
            class="mb-8 max-w-4xl">
            <div class="grid gap-4 pt-2">
                <p class="text-sm">{t("admin-usage-what")}</p>
                {setting.map(|s| view! { <UsageSwitch s=s/> })}
                {move || report.get().map(|r| match r {
                    Ok(r) => view! { <UsageTable r=r/> }.into_any(),
                    Err(e) => view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> }.into_any(),
                })}
                <p class="text-sm">
                    <a class="underline" href="/api/v1/admin/usage.csv" download="usage.csv">{t("admin-usage-csv")}</a>
                </p>
            </div>
        </Disclosure>
    }
}

#[component]
fn UsageSwitch(s: SettingDto) -> impl IntoView {
    let on = RwSignal::new(s.value.as_bool().unwrap_or(true));
    let editable = origin(&s) == Origin::Editable;
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let message = kept("storage-usage", || None::<Result<String, String>>);
    let saved = use_i18n().t("admin-saved");
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let values =
            BTreeMap::from([(KEY.to_string(), serde_json::Value::Bool(on.get_untracked()))]);
        let saved = saved.clone();
        let call = move || admin_api::update_admin_settings(values.clone());
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<_, String>| {
                message.set(Some(r.map(|_| saved.clone())));
            }),
        );
    };
    let state = move || {
        if on.get() {
            t("admin-usage-on")
        } else {
            t("admin-usage-off")
        }
    };
    view! {
        <form class="grid gap-2" on:submit=save>
            {if editable {
                view! {
                    <div class="flex items-center gap-3">
                        <Switch id="adm-usage-funnel" checked=on/>
                        <label for="adm-usage-funnel" class="text-sm font-medium">{t("admin-setting-usage_funnel")}</label>
                    </div>
                }.into_any()
            } else {
                view! { <p class="text-sm font-medium">{t("admin-setting-usage_funnel")} ": " {state}</p> }.into_any()
            }}
            <SettingNote s=s/>
            <Outcome message=message/>
            {editable.then(|| view! { <Button r#type="submit" class="w-fit" loading=busy>{t("admin-save")}</Button> })}
        </form>
    }
}

#[component]
fn UsageTable(r: UsageReportDto) -> impl IntoView {
    let i18n = use_i18n();
    let rows = totals(&r);
    let caption = i18n.t_args(
        "admin-usage-caption",
        &[
            ("days", r.days.into()),
            ("retention", r.retention_days.into()),
        ],
    );
    if rows.is_empty() {
        return view! { <p class="text-sm text-muted-foreground">{t("admin-usage-none")}</p> }
            .into_any();
    }
    view! {
        <Table>
            <caption class="text-left text-sm text-muted-foreground">{caption}</caption>
            <TableHeader><TableRow>
                <TableHead>{t("admin-usage-flow")}</TableHead>
                <TableHead>{t("admin-usage-section")}</TableHead>
                <TableHead>{t("admin-usage-outcome")}</TableHead>
                <TableHead>{t("admin-usage-count")}</TableHead>
            </TableRow></TableHeader>
            <TableBody>
                {rows.into_iter().map(|((flow, subject, outcome), n)| view! {
                    <TableRow>
                        <TableCell class="font-mono">{flow}</TableCell>
                        <TableCell class="font-mono">{subject}</TableCell>
                        <TableCell class="font-mono">{outcome}</TableCell>
                        <TableCell>{n}</TableCell>
                    </TableRow>
                }).collect_view()}
            </TableBody>
        </Table>
    }
    .into_any()
}
