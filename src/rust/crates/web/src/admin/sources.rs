//! Admin > Data sources (design.md 6.10 as trimmed by addendum-1 1.3): the attention block,
//! a table (source, kinds, status, action) whose rows expand to the source card with the
//! write-only key fields (addendum-1 2.3), "how to get a key", Test before saving, the
//! licence record (hosted mode) and the attribution preview.

use std::collections::BTreeMap;

use contracts::admin::{AdminSourceDto, SourceTestDto};
use leptos::prelude::*;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant, Checkbox, Label,
    SecretField, StatusDot, Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
};

use super::{AdminFrame, Attention, ConfirmDialog};
use crate::admin_api;
use crate::admin_state::{key_guide, source_state};
use crate::i18n::{kind_label, t, use_i18n};
use crate::reauth::{gated, use_reauth};
use crate::settings::{Outcome, kept, loaded};

pub(crate) type Sources = Resource<Result<Vec<AdminSourceDto>, ServerFnError>>;

type Message = RwSignal<Option<Result<String, String>>>;

#[component]
pub fn Sources() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-sources") lead=t("admin-sources-lead")>
            <SourcesBody/>
        </AdminFrame>
    }
}

fn needs_attention(s: &AdminSourceDto) -> bool {
    !matches!(s.status.state.as_str(), "working" | "disabled")
}

/// Whether a row's card is open: kept across refetches, so a save does not close it, and
/// shared with the attention block's link.
fn card_open(s: &AdminSourceDto) -> RwSignal<bool> {
    let open = needs_attention(s) && s.status.state != "not_set_up";
    kept(&format!("src-open-{}", s.status.id), move || open)
}

#[component]
fn SourcesBody() -> impl IntoView {
    let data: Sources = Resource::new(|| (), |()| admin_api::admin_sources());
    view! {
        {loaded(data, move |list| {
            let attention: Vec<AdminSourceDto> = list.iter().filter(|s| needs_attention(s)).cloned().collect();
            let licence = list.iter().any(|s| s.licence_warning);
            view! {
                {(!attention.is_empty()).then(|| view! { <SourcesAttention list=attention/> })}
                {licence.then(|| view! {
                    <Alert variant=AlertVariant::Info role=AlertRole::None class="mb-6 max-w-3xl">{t("admin-sources-licence-banner")}</Alert>
                })}
                <Table attr:data-testid="src-table" class="max-w-4xl">
                    <caption class="sr-only">{t("admin-nav-sources")}</caption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t("admin-sources-source")}</TableHead>
                            <TableHead>{t("admin-sources-kinds")}</TableHead>
                            <TableHead>{t("admin-sources-status")}</TableHead>
                            <TableHead><span class="sr-only">{t("admin-users-actions")}</span></TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        {list.into_iter().map(|s| view! { <SourceRows s=s data=data/> }).collect_view()}
                    </TableBody>
                </Table>
            }
        })}
        <p class="mt-4 text-sm"><a class="underline" href="/settings/sources#attribution">{t("admin-sources-attribution")}</a></p>
    }
}

/// The sources that need a look, each with its state and one action that opens its card
/// (D14, UX2-W14).
#[component]
fn SourcesAttention(list: Vec<AdminSourceDto>) -> impl IntoView {
    let i18n = use_i18n();
    let title = i18n.t_args("admin-sources-attention", &[("count", list.len().into())]);
    let rows = list
        .into_iter()
        .map(|s| {
            let open = card_open(&s);
            let state = t(&format!("source-state-{}", s.status.state));
            let name = s.status.name.clone();
            let line = i18n.t_args(
                "admin-sources-attention-line",
                &[("name", name.clone().into()), ("state", state.into())],
            );
            let action = i18n.t_args("admin-sources-attention-open", &[("name", name.into())]);
            let href = format!("#adm-src-{}", s.status.id);
            view! {
                <li class="flex flex-wrap items-center gap-x-3 gap-y-1">
                    <span>{line}</span>
                    <a class="text-sm font-medium underline" href=href on:click=move |_| open.set(true)>{action}</a>
                </li>
            }
        })
        .collect_view();
    view! { <Attention title=title><ul class="grid gap-1">{rows}</ul></Attention> }
}

#[component]
fn SourceRows(s: AdminSourceDto, data: Sources) -> impl IntoView {
    let i18n = use_i18n();
    let open = card_open(&s);
    let card_id = format!("adm-src-{}", s.status.id);
    let controls = card_id.clone();
    let name = s.status.name.clone();
    let dot = source_state(&s.status.state);
    let kinds = s
        .status
        .kinds
        .iter()
        .map(|k| kind_label(k))
        .collect::<Vec<_>>()
        .join(", ");
    let state = t(&format!("source-state-{}", s.status.state));
    let set_up = s.status.state == "not_set_up";
    let label = i18n.t_args(
        if set_up {
            "admin-sources-set-up-named"
        } else {
            "admin-sources-details-named"
        },
        &[("name", s.status.name.clone().into())],
    );
    view! {
        <TableRow attr:data-testid="src-row">
            <TableCell class="font-medium">{name}</TableCell>
            <TableCell class="text-muted-foreground">{kinds}</TableCell>
            <TableCell><StatusDot state=dot label=state/></TableCell>
            <TableCell>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=label
                    attr:aria-expanded=move || if open.get() { "true" } else { "false" }
                    attr:aria-controls=controls on:click=move |_| open.update(|o| *o = !*o)>
                    {if set_up { t("admin-sources-set-up") } else { t("admin-sources-details") }}
                </Button>
            </TableCell>
        </TableRow>
        <tr id=card_id hidden=move || !open.get() class="border-b">
            <td colspan="4" class="p-2"><SourceCard s=s data=data/></td>
        </tr>
    }
}

/// The last 24 hours and the last error, one line each.
fn usage_lines(s: &AdminSourceDto) -> Vec<String> {
    let i18n = use_i18n();
    let st = &s.status;
    let mut out = vec![i18n.t_args(
        "admin-sources-calls",
        &[
            ("calls", st.calls_24h.into()),
            ("limited", st.rate_limited_24h.into()),
        ],
    )];
    if let Some(at) = &st.last_ok_at {
        out.push(i18n.t_args(
            "admin-sources-last-ok",
            &[("when", i18n.datetime(at).into())],
        ));
    }
    if let (Some(at), Some(e)) = (&st.last_error_at, &st.last_error) {
        out.push(i18n.t_args(
            "admin-sources-last-error",
            &[
                ("when", i18n.datetime(at).into()),
                ("error", e.clone().into()),
            ],
        ));
    }
    out
}

#[component]
fn SourceCard(s: AdminSourceDto, data: Sources) -> impl IntoView {
    let lines = usage_lines(&s);
    // "Agreement on file" belongs to hosted mode (design.md 4.1, UX2-W2): the server
    // warns while nothing is recorded there, and a recorded answer stays visible
    let licence = s.licence_warning || s.agreement_on_file.is_some();
    view! {
        <div class="grid gap-4 rounded-xl border bg-card p-4">
            <ul class="grid gap-0.5 text-sm text-muted-foreground">
                {lines.into_iter().map(|l| view! { <li>{l}</li> }).collect_view()}
            </ul>
            <KeyHowTo s=s.clone()/>
            <Credential s=s.clone() data=data/>
            {licence.then(|| view! { <Licence s=s.clone() data=data/> })}
            <div class="text-sm">
                <p class="font-medium">{t("admin-sources-attribution-preview")}</p>
                <p lang="en" class="text-muted-foreground">{s.attribution.clone()}</p>
                <a class="underline" href=s.terms_url.clone() rel="noopener">{t("admin-sources-terms")}</a>
            </div>
        </div>
    }
}

/// "How to get a key" in three numbered lines with the provider's developer page
/// (design.md 5.1 step 3, 6.10; UX2-W1), while the card asks for a key.
#[component]
pub(crate) fn KeyHowTo(s: AdminSourceDto) -> impl IntoView {
    let asks =
        !matches!(s.credential.source.as_str(), "env" | "not_needed") && !s.fields.is_empty();
    key_guide(&s.status.id).filter(|_| asks).map(|guide| {
    let link = use_i18n().t_args(
        "admin-sources-howto-link",
        &[("name", s.status.name.clone().into())],
    );
    let id = format!("adm-howto-{}", s.status.id);
    let labelled = id.clone();
    view! {
        <section aria-labelledby=labelled data-testid="key-howto" class="grid gap-1 text-sm">
            <p id=id class="font-medium">{t("admin-sources-howto")}</p>
            <ol class="grid list-decimal gap-0.5 pl-5 text-muted-foreground">
                {guide.steps.iter().map(|k| view! { <li>{t(k)}</li> }).collect_view()}
            </ol>
            <a class="w-fit underline" href=guide.url rel="noopener noreferrer" target="_blank">{link}</a>
        </section>
    }
    })
}

/// The test result in words (`source-test-<result>`), with the sample title.
fn test_text(r: &SourceTestDto) -> String {
    let i18n = use_i18n();
    let mut text = i18n.t_args(
        &format!("source-test-{}", r.result),
        &[("ms", r.elapsed_ms.into())],
    );
    if let Some(title) = &r.sample_title {
        text.push(' ');
        text.push_str(&i18n.t_args("admin-sources-sample", &[("title", title.clone().into())]));
    }
    text
}

/// The typed fields, when any was typed (Test then checks them instead of the stored key).
fn typed(fields: &[(String, RwSignal<String>)]) -> Option<BTreeMap<String, String>> {
    let m: BTreeMap<String, String> = fields
        .iter()
        .map(|(k, v)| (k.clone(), v.get_untracked()))
        .collect();
    m.values().any(|v| !v.trim().is_empty()).then_some(m)
}

/// Test (re-auth gated): the typed fields or the stored key.
fn test_key(
    id: String,
    fields: Vec<(String, RwSignal<String>)>,
    testing: RwSignal<bool>,
    tested: Message,
) -> Callback<()> {
    let reauth = use_reauth();
    Callback::new(move |()| {
        let (id, f) = (id.clone(), typed(&fields));
        let call = move || admin_api::test_source(id.clone(), f.clone());
        gated(
            reauth,
            testing,
            call,
            Callback::new(move |r: Result<SourceTestDto, String>| {
                tested.set(Some(r.map(|r| test_text(&r))));
            }),
        );
    })
}

#[component]
pub(crate) fn Credential(s: AdminSourceDto, data: Sources) -> impl IntoView {
    let c = s.credential.clone();
    if c.source == "not_needed" || s.fields.is_empty() {
        return view! { <p class="text-sm">{t("admin-sources-no-key")}</p> }.into_any();
    }
    let id = s.status.id.clone();
    let tested = kept(
        &format!("src-tested-{id}"),
        || None::<Result<String, String>>,
    );
    let testing = RwSignal::new(false);
    let fields: Vec<(String, RwSignal<String>)> = s
        .fields
        .iter()
        .map(|f| (f.clone(), RwSignal::new(String::new())))
        .collect();
    let test = test_key(id, fields.clone(), testing, tested);
    if c.source == "env" {
        let var = c.env_var.clone().unwrap_or_default();
        let line = use_i18n().t_args("admin-setting-env", &[("var", var.into())]);
        return view! {
            <div class="grid gap-2">
                <p class="text-sm" data-testid="setting-origin">{line}</p>
                {c.shadowed.then(|| view! { <p class="text-sm text-muted-foreground">{t("admin-sources-shadowed")}</p> })}
                <TestButton busy=testing on_test=test/>
                <TestResult r=tested/>
            </div>
        }
        .into_any();
    }
    view! { <KeyForm s=s fields=fields test=test testing=testing tested=tested data=data/> }
        .into_any()
}

/// The write-only key fields of a key stored in the database (or none yet): Save, Test and
/// Remove key (DEC-264).
#[component]
fn KeyForm(
    s: AdminSourceDto,
    fields: Vec<(String, RwSignal<String>)>,
    test: Callback<()>,
    testing: RwSignal<bool>,
    tested: Message,
    data: Sources,
) -> impl IntoView {
    let i18n = use_i18n();
    let id = s.status.id.clone();
    let message = kept(
        &format!("src-saved-{id}"),
        || None::<Result<String, String>>,
    );
    let saving = RwSignal::new(false);
    let reauth = use_reauth();
    let stored = (s.credential.source != "missing").then(|| stored_line(&s.credential));
    let editing = RwSignal::new(false);
    let confirm = RwSignal::new(false);
    let save = {
        let (id, fields) = (id.clone(), fields.clone());
        move || {
            let Some(f) = typed(&fields) else { return };
            let id = id.clone();
            let call = move || admin_api::set_source_credential(id.clone(), f.clone());
            let saved = use_i18n().t("admin-saved");
            gated(
                reauth,
                saving,
                call,
                Callback::new(move |r: Result<AdminSourceDto, String>| {
                    message.set(Some(r.map(|_| saved.clone())));
                    data.refetch();
                }),
            );
        }
    };
    let clear = clear_key(id.clone(), saving, data, message);
    let has_stored = stored.is_some();
    let name = s.status.name.clone();
    let i18n_title = i18n.clone();
    view! {
        <form class="grid max-w-lg gap-3" on:submit=move |ev| { ev.prevent_default(); save(); }>
            {fields.iter().map(|(f, v)| {
                let label = t(&format!("admin-source-field-{f}"));
                let replace = i18n.t_args("settings-replace-named", &[("field", label.clone().into())]);
                view! {
                    <SecretField id=format!("adm-key-{id}-{f}") label=label value=*v
                        editing=editing stored=stored.clone() replace_label=t("settings-replace") replace_name=replace
                        cancel_label=t("reauth-cancel") description=t("admin-sources-key-hint")/>
                }
            }).collect_view()}
            <div class="flex flex-wrap gap-2">
                <TestButton busy=testing on_test=test/>
                <Show when=move || !has_stored || editing.get()>
                    <Button r#type="submit" size=ButtonSize::Sm loading=saving>{t("admin-save")}</Button>
                </Show>
                {has_stored.then(|| view! {
                    <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on:click=move |_| confirm.set(true)>{t("admin-sources-clear")}</Button>
                })}
            </div>
            <TestResult r=tested/>
            <Outcome message=message/>
        </form>
        <ConfirmDialog open=confirm title=Signal::derive(move || i18n_title.t_args("admin-sources-clear-title", &[("name", name.clone().into())]))
            body=Signal::derive(move || use_i18n().t("admin-sources-clear-body")) confirm=t("admin-sources-clear") on_confirm=clear/>
    }
}

/// "Clear key" after the confirm dialog (re-auth gated), then reload the list.
fn clear_key(id: String, saving: RwSignal<bool>, data: Sources, message: Message) -> Callback<()> {
    let reauth = use_reauth();
    Callback::new(move |()| {
        let id = id.clone();
        let call = move || admin_api::clear_source_credential(id.clone());
        gated(
            reauth,
            saving,
            call,
            Callback::new(move |r: Result<AdminSourceDto, String>| match r {
                Ok(_) => data.refetch(),
                Err(e) => message.set(Some(Err(e))),
            }),
        );
    })
}

fn stored_line(c: &contracts::admin::CredentialMetaDto) -> String {
    let i18n = use_i18n();
    if c.source == "locked" {
        return i18n.t("admin-sources-locked");
    }
    let when = c
        .set_at
        .as_deref()
        .map(|d| i18n.date(d))
        .unwrap_or_default();
    match &c.set_by {
        Some(by) => i18n.t_args(
            "admin-sources-set-by",
            &[("date", when.into()), ("name", by.clone().into())],
        ),
        None => i18n.t_args("admin-sources-set-on", &[("date", when.into())]),
    }
}

#[component]
fn TestButton(busy: RwSignal<bool>, on_test: Callback<()>) -> impl IntoView {
    view! {
        <Button variant=ButtonVariant::Outline size=ButtonSize::Sm loading=busy on:click=move |_| on_test.run(())>
            {t("admin-sources-test")}
        </Button>
    }
}

/// A working key as a polite status line; a failure as an alert (UX2-W6).
#[component]
fn TestResult(r: Message) -> impl IntoView {
    view! {
        <p role="status" data-testid="test-result" class="text-sm empty:hidden">
            {move || r.get().and_then(Result::ok)}
        </p>
        {move || r.get().and_then(Result::err).map(|e| view! {
            <Alert variant=AlertVariant::Destructive role=AlertRole::Alert attr:data-testid="test-error">{e}</Alert>
        })}
    }
}

/// The licence record of hosted mode (design.md 4.1): "Agreement on file". A refused or
/// cancelled change puts the box back (UX2-W10).
#[component]
pub(crate) fn Licence(s: AdminSourceDto, data: Sources) -> impl IntoView {
    let reauth = use_reauth();
    let checked = RwSignal::new(s.agreement_on_file.unwrap_or(false));
    let busy = RwSignal::new(false);
    let id = s.status.id.clone();
    let message = kept(&format!("src-licence-{id}"), || {
        None::<Result<String, String>>
    });
    let box_id = format!("adm-licence-{id}");
    let change = move |ev: leptos::ev::Event| {
        let v = event_target_checked(&ev);
        let id = id.clone();
        let call = move || admin_api::set_source_licence(id.clone(), v);
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<AdminSourceDto, String>| match r {
                Ok(_) => {
                    message.set(None);
                    data.refetch();
                }
                Err(e) => {
                    checked.set(!v);
                    message.set(Some(Err(e)));
                }
            }),
        );
    };
    view! {
        <div class="grid gap-1">
            <div class="flex items-center gap-2">
                <Checkbox id=box_id.clone() checked=checked on:change=change/>
                <Label r#for=box_id>{t("admin-sources-licence")}</Label>
            </div>
            <p class="text-sm text-muted-foreground">{t("admin-sources-licence-hint")}</p>
            <Outcome message=message/>
        </div>
    }
}
