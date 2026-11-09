//! Admin > Storage and jobs (OD-14: one page): the budget with its editable numbers (OD-10:
//! a value set by env or file is read-only with its origin), "Run eviction now", the job
//! lanes, failed jobs (retry, discard) and the cron list.

use std::collections::BTreeMap;

use contracts::admin::{FailedJobDto, SettingDto, StorageJobsDto};
use leptos::prelude::*;
use serde_json::Value;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant, FormField, Input,
    ProgressBar, Table, TableBody, TableCell, TableHead, TableHeader, TableRow, field_described_by,
};

use super::{AdminFrame, ConfirmDialog, SettingNote};
use crate::admin_api;
use crate::admin_state::{
    Origin, byte_value, find_setting, format_bytes, fraction_value, origin, percent_text,
    setting_label_key,
};
use crate::i18n::{t, use_i18n};
use crate::reauth::{gated, use_reauth};
use crate::settings::{Outcome, kept, loaded};

type Data = Resource<Result<StorageJobsDto, ServerFnError>>;

#[component]
pub fn Storage() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-storage") lead=t("admin-storage-lead")>
            <StorageBody/>
        </AdminFrame>
    }
}

#[component]
fn StorageBody() -> impl IntoView {
    let data: Data = Resource::new(|| (), |()| admin_api::storage_jobs());
    loaded(data, move |d| {
        let failed = d.failed.len();
        let settings = d.settings.clone();
        let i18n = use_i18n();
        view! {
            {d.at_hard_cap.then(|| view! {
                <Alert variant=AlertVariant::Destructive role=AlertRole::None class="mb-6 max-w-3xl">{t("admin-storage-full")}</Alert>
            })}
            {(failed > 0).then(|| view! {
                <Alert variant=AlertVariant::Warning role=AlertRole::None class="mb-6 max-w-3xl">
                    {i18n.t_args("admin-jobs-failed-attention", &[("count", failed.into())])} " "
                    <a class="underline" href="#adm-failed-h">{t("admin-jobs-failed")}</a>
                </Alert>
            })}
            <Budget d=d.clone() data=data/>
            <Jobs d=d data=data/>
            <super::usage::UsageMeasurement settings=settings/>
        }
    })
}

#[component]
fn Budget(d: StorageJobsDto, data: Data) -> impl IntoView {
    let i18n = use_i18n();
    let cap = d.effective_cap.max(1);
    // per mille of the cap keeps the bar inside u32
    let used = u32::try_from(d.used_bytes.saturating_mul(1000) / cap).unwrap_or(1000);
    let text = i18n.t_args(
        "admin-storage-used",
        &[
            ("used", format_bytes(d.used_bytes).into()),
            ("cap", format_bytes(d.effective_cap).into()),
        ],
    );
    let rows = [
        ("admin-storage-pinned", d.pinned_bytes),
        ("admin-storage-cache", d.cache_bytes),
        ("admin-storage-evictable", d.evictable_bytes),
        ("admin-storage-high", d.high_watermark_bytes),
        ("admin-storage-low", d.low_watermark_bytes),
    ];
    view! {
        <section aria-labelledby="adm-storage-h" class="mb-8 grid max-w-3xl gap-4">
            <h2 id="adm-storage-h" class="text-lg font-semibold">{t("admin-storage-title")}</h2>
            <p class="text-sm">{text.clone()} " " <span class="text-muted-foreground">{i18n.t_args("admin-storage-backend", &[("backend", d.backend.clone().into())])}</span></p>
            <ProgressBar label=t("admin-storage-title") value=used max=1000_u32 value_text=text/>
            <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1 text-sm">
                {rows.into_iter().map(|(key, n)| view! { <dt class="font-medium">{t(key)}</dt><dd>{format_bytes(n)}</dd> }).collect_view()}
            </dl>
            <Evict data=data/>
            <BudgetForm settings=d.settings data=data/>
        </section>
    }
}

#[component]
fn Evict(data: Data) -> impl IntoView {
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let message = kept("storage-evict", || None::<Result<String, String>>);
    let i18n = use_i18n();
    let run = move |_| {
        let i18n = i18n.clone();
        gated(
            reauth,
            busy,
            admin_api::run_eviction,
            Callback::new(move |r: Result<contracts::admin::EvictionDto, String>| {
                message.set(Some(r.map(|e| {
                    i18n.t_args(
                        "admin-storage-evicted",
                        &[
                            ("bytes", format_bytes(e.bytes).into()),
                            ("objects", e.objects.into()),
                        ],
                    )
                })));
                data.refetch();
            }),
        );
    };
    view! {
        <div class="grid gap-2">
            <Button variant=ButtonVariant::Outline class="w-fit" loading=busy on:click=run>{t("admin-storage-evict")}</Button>
            <Outcome message=message/>
        </div>
    }
}

/// A budget number's hint key, how a typed value is sent, and how a value is shown.
type BudgetField = (
    &'static str,
    fn(&str) -> Option<Value>,
    fn(&Value) -> String,
);

/// How each budget number is typed and sent.
fn budget_field(key: &str) -> BudgetField {
    fn bytes_text(v: &Value) -> String {
        v.as_u64()
            .map_or_else(|| v.as_str().unwrap_or_default().to_string(), format_bytes)
    }
    fn percent(v: &Value) -> String {
        v.as_f64().map(percent_text).unwrap_or_default()
    }
    match key {
        "storage.max_total_bytes" => ("admin-storage-limit-hint", byte_value, bytes_text),
        _ => ("admin-storage-percent-hint", fraction_value, percent),
    }
}

const BUDGET_KEYS: [&str; 3] = [
    "storage.max_total_bytes",
    "storage.high_watermark",
    "storage.low_watermark",
];

#[component]
fn BudgetForm(settings: Vec<SettingDto>, data: Data) -> impl IntoView {
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let message = kept("storage-budget", || None::<Result<String, String>>);
    let i18n = use_i18n();
    let fields: Vec<(SettingDto, RwSignal<String>)> = BUDGET_KEYS
        .iter()
        .filter_map(|k| find_setting(&settings, k).cloned())
        .map(|s| {
            let text = (budget_field(&s.key).2)(&s.value);
            (s, RwSignal::new(text))
        })
        .collect();
    let any_editable = fields.iter().any(|(s, _)| origin(s) == Origin::Editable);
    let to_send = fields.clone();
    let save = move || {
        let mut values = BTreeMap::new();
        for (s, v) in to_send
            .iter()
            .filter(|(s, _)| origin(s) == Origin::Editable)
        {
            let Some(value) = (budget_field(&s.key).1)(&v.get_untracked()) else {
                message.set(Some(Err(i18n.t_args(
                    "admin-storage-invalid",
                    &[("field", t(&setting_label_key(&s.key)).into())],
                ))));
                return;
            };
            values.insert(s.key.clone(), value);
        }
        let saved = i18n.t("admin-saved");
        let call = move || admin_api::update_admin_settings(values.clone());
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<_, String>| {
                message.set(Some(r.map(|_| saved.clone())));
                data.refetch();
            }),
        );
    };
    view! {
        <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); save(); }>
            <h3 class="font-semibold">{t("admin-storage-budget")}</h3>
            {fields.into_iter().map(|(s, v)| {
                let id = format!("adm-{}", s.key.replace('.', "-"));
                let fixed = origin(&s) != Origin::Editable;
                view! {
                    <div class="grid max-w-sm gap-1">
                        <FormField id=id.clone() label=t(&setting_label_key(&s.key)) description=t(budget_field(&s.key).0)>
                            <Input id=id.clone() value=v readonly=fixed described_by=field_described_by(&id)/>
                        </FormField>
                        <SettingNote s=s/>
                    </div>
                }
            }).collect_view()}
            <Outcome message=message/>
            {any_editable.then(|| view! { <Button r#type="submit" class="w-fit" loading=busy>{t("admin-save")}</Button> })}
        </form>
    }
}

#[component]
fn Jobs(d: StorageJobsDto, data: Data) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <section aria-labelledby="adm-jobs-h" class="mb-8 grid max-w-4xl gap-4">
            <h2 id="adm-jobs-h" class="text-lg font-semibold">{t("admin-jobs-title")}</h2>
            <Table>
                <caption class="sr-only">{t("admin-jobs-lanes")}</caption>
                <TableHeader><TableRow>
                    <TableHead>{t("admin-jobs-lane")}</TableHead>
                    <TableHead>{t("admin-jobs-queued")}</TableHead>
                    <TableHead>{t("admin-jobs-running")}</TableHead>
                    <TableHead>{t("admin-jobs-dead")}</TableHead>
                </TableRow></TableHeader>
                <TableBody>
                    {d.lanes.into_iter().map(|l| view! {
                        <TableRow>
                            <TableCell class="font-mono">{l.lane}</TableCell>
                            <TableCell>{l.queued}</TableCell>
                            <TableCell>{l.running}</TableCell>
                            <TableCell>{l.dead}</TableCell>
                        </TableRow>
                    }).collect_view()}
                </TableBody>
            </Table>
            <h3 id="adm-failed-h" class="font-semibold">{t("admin-jobs-failed")}</h3>
            {if d.failed.is_empty() {
                view! { <p class="text-sm text-muted-foreground">{t("admin-jobs-none-failed")}</p> }.into_any()
            } else {
                view! {
                    <ul class="grid gap-2">
                        {d.failed.into_iter().map(|j| view! { <FailedJob j=j data=data/> }).collect_view()}
                    </ul>
                }.into_any()
            }}
            <h3 class="font-semibold">{t("admin-jobs-cron")}</h3>
            <Table>
                <caption class="sr-only">{t("admin-jobs-cron")}</caption>
                <TableHeader><TableRow>
                    <TableHead>{t("admin-jobs-name")}</TableHead>
                    <TableHead>{t("admin-jobs-schedule")}</TableHead>
                    <TableHead>{t("admin-jobs-next")}</TableHead>
                </TableRow></TableHeader>
                <TableBody>
                    {d.cron.into_iter().map(|c| { let next = i18n.datetime(&c.next_run_at); view! {
                        <TableRow>
                            <TableCell class="font-mono">{c.name}</TableCell>
                            <TableCell class="font-mono">{c.schedule}</TableCell>
                            <TableCell>{next}</TableCell>
                        </TableRow>
                    }}).collect_view()}
                </TableBody>
            </Table>
        </section>
    }
}

#[component]
fn FailedJob(j: FailedJobDto, data: Data) -> impl IntoView {
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let confirm = RwSignal::new(false);
    let message = RwSignal::new(None::<Result<String, String>>);
    let i18n = use_i18n();
    let id = j.id.clone();
    let act = Callback::new(move |retry: bool| {
        let id = id.clone();
        let call = move || admin_api::failed_job(id.clone(), retry);
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<(), String>| match r {
                Ok(()) => data.refetch(),
                Err(e) => message.set(Some(Err(e))),
            }),
        );
    });
    let line = i18n.t_args(
        "admin-jobs-failed-line",
        &[
            ("attempts", j.attempts.into()),
            ("when", i18n.datetime(&j.failed_at).into()),
        ],
    );
    let name = format!("{} ({})", j.kind, j.lane);
    let retry_label = i18n.t_args("admin-jobs-retry-named", &[("name", name.clone().into())]);
    let discard_label = i18n.t_args("admin-jobs-discard-named", &[("name", name.clone().into())]);
    let i18n_title = i18n.clone();
    view! {
        <li data-testid="failed-job" class="grid gap-1 rounded-md border px-3 py-2 text-sm">
            <p><span class="font-mono font-medium">{name.clone()}</span> " " <span class="text-muted-foreground">{line}</span></p>
            {j.last_error.clone().map(|e| view! { <p class="break-all font-mono text-xs text-muted-foreground">{e}</p> })}
            <div class="flex flex-wrap gap-2">
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=retry_label loading=busy
                    on:click=move |_| act.run(true)>{t("admin-jobs-retry")}</Button>
                <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm attr:aria-label=discard_label
                    on:click=move |_| confirm.set(true)>{t("admin-jobs-discard")}</Button>
            </div>
            <Outcome message=message/>
            <ConfirmDialog open=confirm title=Signal::derive(move || i18n_title.t_args("admin-jobs-discard-title", &[("name", name.clone().into())]))
                body=Signal::derive(move || i18n.t("admin-jobs-discard-body")) confirm=t("admin-jobs-discard")
                on_confirm=Callback::new(move |()| act.run(false))/>
        </li>
    }
}
