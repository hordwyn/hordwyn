//! Admin > Sign-in and SSO (addendum-1 3.4, 4.2-4.5; mockup `admin.html` annotations 9 and
//! 10; DEC-314, DEC-321, DEC-324, DEC-326): the URIs to register at a provider, the provider
//! table with a detail per row (client id, the write-only client secret, scopes, account
//! rules, Test with its outcomes and the two browser probes), "Add a provider", the sign-in
//! mode radios with their consequences and the server's preconditions, and why front-channel
//! logout is not used. Every change is re-auth gated and audited by the server.

use contracts::admin::{
    OidcAdminDto, OidcProviderDto, OidcProviderInputDto, OidcTestDto, SettingDto,
};
use leptos::prelude::*;
use leptos_router::hooks::use_query_map;
use serde_json::{Value, json};
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant,
    Checkbox, CopyField, Disclosure, DisclosureMode, FormField, Input, Label, SecretField,
    StatusDot, StatusState, Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
    field_described_by,
};

use super::{AdminFrame, ConfirmDialog, SettingNote};
use crate::admin_api;
use crate::admin_state::{Origin, find_setting, origin};
use crate::i18n::{t, use_i18n};
use crate::reauth::{gated, gated_settings, use_reauth};
use crate::rest;
use crate::settings::{Outcome, kept, loaded};
use crate::sign_in::encode;

type Data = Resource<Result<OidcAdminDto, ServerFnError>>;
type Message = RwSignal<Option<Result<String, String>>>;

/// Where the probes come back to (DEC-324: `?probe=<outcome>`).
const HERE: &str = "/admin/sso";

#[component]
pub fn SignInSso() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-sso") lead=t("admin-sso-lead")>
            <SsoBody/>
        </AdminFrame>
    }
}

#[component]
fn SsoBody() -> impl IntoView {
    let data: Data = Resource::new(|| (), |()| admin_api::admin_oidc());
    let settings = Resource::new(|| (), |()| admin_api::admin_settings());
    view! {
        <ProbeNotice/>
        {loaded(data, move |d| {
            let default = d.providers.iter().find(|p| p.is_default && p.enabled)
                .or_else(|| d.providers.iter().find(|p| p.enabled))
                .map(|p| p.display_name.clone());
            view! {
                <Providers list=d.providers.clone() data=data/>
                <NewProvider data=data/>
                <Register d=d.clone()/>
                {loaded(settings, move |s| find_setting(&s.settings, "auth.sign_in_mode").cloned().map(|m| {
                    view! { <Mode s=m provider=default.clone()/> }
                }))}
            }
        })}
        <p class="mt-8 max-w-2xl text-sm text-muted-foreground">{t("admin-sso-front-channel")}</p>
    }
}

/// The answer of a probe round trip (`?probe=` / `?probe_error=`, DEC-324).
#[component]
fn ProbeNotice() -> impl IntoView {
    let q = use_query_map();
    let (probe, error) = q.with_untracked(|q| (q.get("probe"), q.get("probe_error")));
    let i18n = use_i18n();
    view! {
        {probe.map(|p| view! {
            <Alert variant=AlertVariant::Info role=AlertRole::Status class="mb-6 max-w-3xl">
                {probe_text(&p)}
            </Alert>
        })}
        {error.map(|code| view! {
            <Alert variant=AlertVariant::Destructive role=AlertRole::Alert class="mb-6 max-w-3xl">
                {i18n.t_args("admin-sso-probe-error", &[("code", code.into())])}
            </Alert>
        })}
    }
}

/// `admin-sso-probe-<outcome>`; an outcome this build has no words for shows as itself.
fn probe_text(outcome: &str) -> String {
    let key = format!("admin-sso-probe-{outcome}");
    let text = t(&key);
    if text == key {
        outcome.to_string()
    } else {
        text
    }
}

/// The exact URIs to register at every provider (addendum-1 4.2).
#[component]
fn Register(d: OidcAdminDto) -> impl IntoView {
    let rows = [
        ("adm-sso-redirect", "admin-sso-redirect-uri", d.redirect_uri),
        (
            "adm-sso-logout",
            "admin-sso-logout-uri",
            d.post_logout_redirect_uri,
        ),
        (
            "adm-sso-backchannel",
            "admin-sso-backchannel-uri",
            d.backchannel_logout_uri,
        ),
    ];
    view! {
        <section aria-labelledby="adm-sso-register-h" class="mb-8 grid max-w-3xl gap-3">
            <h2 id="adm-sso-register-h" class="text-lg font-semibold">{t("admin-sso-register")}</h2>
            <p class="text-sm text-muted-foreground">{t("admin-sso-register-lead")}</p>
            {rows.into_iter().map(|(id, label, value)| view! {
                <FormField id=id label=t(label)>
                    <CopyField id=id value=Signal::stored(value) copy_label=t("settings-copy") copied_label=t("settings-copied")/>
                </FormField>
            }).collect_view()}
        </section>
    }
}

/// The status of a provider row: off, its last Test, or not tested yet.
fn row_state(p: &OidcProviderDto) -> (StatusState, String) {
    if !p.enabled {
        return (StatusState::Off, t("admin-sso-state-off"));
    }
    match &p.last_test {
        Some(x) if x.ok => (StatusState::Ok, t("admin-sso-state-ok")),
        Some(_) => (StatusState::Bad, t("admin-sso-state-failed")),
        None => (StatusState::Warn, t("admin-sso-state-untested")),
    }
}

#[component]
fn Providers(list: Vec<OidcProviderDto>, data: Data) -> impl IntoView {
    if list.is_empty() {
        return view! { <p class="mb-6 max-w-2xl text-sm text-muted-foreground">{t("admin-sso-none")}</p> }.into_any();
    }
    // the detail forms sit under the table, not in a cell of it: a form inside a scrolled
    // table needs two-dimensional scrolling at 360 px (UX3-8, WCAG 1.4.10)
    let panels = list
        .iter()
        .cloned()
        .map(|p| view! { <ProviderPanel p=p data=data/> })
        .collect_view();
    view! {
        <Table stacked=true attr:data-testid="sso-table" class="mb-6 max-w-4xl">
            <caption class="sr-only">{t("admin-sso-providers")}</caption>
            <TableHeader>
                <TableRow>
                    <TableHead>{t("admin-sso-provider")}</TableHead>
                    <TableHead>{t("admin-sso-issuer")}</TableHead>
                    <TableHead>{t("admin-sso-linked")}</TableHead>
                    <TableHead>{t("admin-sso-status")}</TableHead>
                    <TableHead><span class="sr-only">{t("admin-sso-actions")}</span></TableHead>
                </TableRow>
            </TableHeader>
            <TableBody>
                {list.into_iter().map(|p| view! { <ProviderRow p=p/> }).collect_view()}
            </TableBody>
        </Table>
        <div class="mb-6 grid max-w-4xl grid-cols-[minmax(0,1fr)] gap-4">{panels}</div>
    }
    .into_any()
}

#[component]
fn ProviderRow(p: OidcProviderDto) -> impl IntoView {
    let open = kept(&format!("sso-open-{}", p.id), || false);
    let (dot, word) = row_state(&p);
    let label = use_i18n().t_args(
        "admin-sso-details",
        &[("name", p.display_name.clone().into())],
    );
    let controls = format!("adm-sso-{}", p.slug);
    let (name, issuer) = (p.display_name.clone(), p.issuer.clone());
    let linked = use_i18n().number(f64::from(
        u32::try_from(p.linked_accounts).unwrap_or(u32::MAX),
    ));
    let (default, env) = (p.is_default, p.provenance == "env");
    view! {
        <TableRow attr:data-testid="sso-row">
            <TableCell label=t("admin-sso-provider")>
                <span class="font-medium">{name}</span>
                {default.then(|| view! { " " <Badge variant=BadgeVariant::Secondary>{t("admin-sso-default")}</Badge> })}
                {env.then(|| view! { " " <Badge variant=BadgeVariant::Outline>{t("admin-setting-read-only")}</Badge> })}
            </TableCell>
            <TableCell label=t("admin-sso-issuer") class="break-all font-mono text-xs">{issuer}</TableCell>
            <TableCell label=t("admin-sso-linked") attr:data-testid="sso-linked">{linked}</TableCell>
            <TableCell label=t("admin-sso-status")><StatusDot state=dot label=word/></TableCell>
            <TableCell label=t("admin-sso-actions")>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-expanded=move || open.get().to_string()
                    attr:aria-controls=controls on:click=move |_| open.update(|o| *o = !*o)>
                    {label}
                </Button>
            </TableCell>
        </TableRow>
    }
}

/// A provider's detail form, opened by its row's Details button (shares the open state).
#[component]
fn ProviderPanel(p: OidcProviderDto, data: Data) -> impl IntoView {
    let open = kept(&format!("sso-open-{}", p.id), || false);
    let id = format!("adm-sso-{}", p.slug);
    let name = use_i18n().t_args(
        "admin-sso-details",
        &[("name", p.display_name.clone().into())],
    );
    view! {
        <section id=id aria-label=name hidden=move || !open.get()
            class="min-w-0">
            <ProviderDetail p=p data=data/>
        </section>
    }
}

/// The form state of a provider (create and change share it).
#[derive(Clone, Copy)]
struct ProviderForm {
    display_name: RwSignal<String>,
    slug: RwSignal<String>,
    issuer: RwSignal<String>,
    client_id: RwSignal<String>,
    scopes: RwSignal<String>,
    groups_claim: RwSignal<String>,
    allowed_groups: RwSignal<String>,
    admin_groups: RwSignal<String>,
    enabled: RwSignal<bool>,
    is_default: RwSignal<bool>,
    auto_create: RwSignal<bool>,
    provider_roles: RwSignal<bool>,
}

/// A list typed with commas or spaces.
#[must_use]
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

impl ProviderForm {
    fn new(p: Option<&OidcProviderDto>) -> Self {
        let s = |f: fn(&OidcProviderDto) -> String| RwSignal::new(p.map(f).unwrap_or_default());
        Self {
            display_name: s(|p| p.display_name.clone()),
            slug: s(|p| p.slug.clone()),
            issuer: s(|p| p.issuer.clone()),
            client_id: s(|p| p.client_id.clone()),
            scopes: s(|p| p.scopes.join(" ")),
            groups_claim: s(|p| p.groups_claim.clone()),
            allowed_groups: s(|p| p.allowed_groups.join(", ")),
            admin_groups: s(|p| p.admin_groups.join(", ")),
            enabled: RwSignal::new(p.is_none_or(|p| p.enabled)),
            is_default: RwSignal::new(p.is_some_and(|p| p.is_default)),
            auto_create: RwSignal::new(p.is_some_and(|p| p.auto_create)),
            provider_roles: RwSignal::new(p.is_some_and(|p| p.role_source == "provider")),
        }
    }

    fn input(self, client_secret: Option<String>) -> OidcProviderInputDto {
        let text = |s: RwSignal<String>| s.get_untracked().trim().to_string();
        let scopes = words(&self.scopes.get_untracked());
        let claim = text(self.groups_claim);
        OidcProviderInputDto {
            slug: text(self.slug),
            display_name: text(self.display_name),
            issuer: text(self.issuer),
            client_id: text(self.client_id),
            client_secret,
            scopes: (!scopes.is_empty()).then_some(scopes),
            enabled: self.enabled.get_untracked(),
            is_default: self.is_default.get_untracked(),
            auto_create: self.auto_create.get_untracked(),
            groups_claim: (!claim.is_empty()).then_some(claim),
            allowed_groups: words(&self.allowed_groups.get_untracked()),
            admin_groups: words(&self.admin_groups.get_untracked()),
            role_source: Some(
                if self.provider_roles.get_untracked() {
                    "provider"
                } else {
                    "local"
                }
                .to_string(),
            ),
        }
    }
}

/// A gated REST change answering a provider (create, change, secret).
fn provider_call(
    method: &'static str,
    path: String,
    body: Value,
) -> impl Fn() -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<OidcProviderDto, ServerFnError>>>,
> + Clone
+ Send
+ Sync
+ 'static {
    move || {
        let (path, body) = (path.clone(), body.clone());
        Box::pin(async move {
            rest::call::<OidcProviderDto>(method, &path, Some(body))
                .await?
                .ok_or_else(|| ServerFnError::new("generic"))
        })
    }
}

/// The admin's error text, or the saved line, after a change; the table refetches.
fn after_change(message: Message, data: Data) -> Callback<Result<OidcProviderDto, String>> {
    let saved = t("admin-saved");
    Callback::new(move |r: Result<OidcProviderDto, String>| {
        let ok = r.is_ok();
        message.set(Some(r.map(|_| saved.clone())));
        if ok {
            data.refetch();
        }
    })
}

#[component]
fn TextField(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    value: RwSignal<String>,
    #[prop(into, optional)] hint: Option<String>,
    #[prop(optional)] readonly: bool,
) -> impl IntoView {
    view! {
        <FormField id=id.clone() label=label description=hint>
            <Input id=id.clone() value=value autocomplete="off" readonly=readonly described_by=field_described_by(&id)/>
        </FormField>
    }
}

#[component]
fn CheckField(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    checked: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2">
            <Checkbox id=id.clone() checked=checked/>
            <Label r#for=id>{label}</Label>
        </div>
    }
}

/// The fields of a provider; `prefix` keeps the ids apart (one form per row).
#[component]
fn ProviderFields(f: ProviderForm, #[prop(into)] prefix: String, creating: bool) -> impl IntoView {
    let id = |name: &str| format!("{prefix}-{name}");
    view! {
        <div class="grid gap-4 sm:grid-cols-2">
            <TextField id=id("name") label=t("admin-sso-display-name") value=f.display_name hint=t("admin-sso-display-name-hint")/>
            <TextField id=id("slug") label=t("admin-sso-slug") value=f.slug hint=t("admin-sso-slug-hint") readonly=!creating/>
            <TextField id=id("issuer") label=t("admin-sso-issuer-url") value=f.issuer hint=t("admin-sso-issuer-hint")/>
            <TextField id=id("client") label=t("admin-sso-client-id") value=f.client_id/>
            {(!creating).then(|| view! {
                <TextField id=id("scopes") label=t("admin-sso-scopes") value=f.scopes hint=t("admin-sso-scopes-hint")/>
            })}
        </div>
        <div class="grid gap-2">
            <CheckField id=id("enabled") label=t("admin-sso-enabled") checked=f.enabled/>
            <CheckField id=id("default") label=t("admin-sso-is-default") checked=f.is_default/>
        </div>
    }
}

/// Account rules of a provider (addendum-1 4.2 "Group and claim to role mapping", OD-16).
#[component]
fn AccountRules(f: ProviderForm, #[prop(into)] prefix: String) -> impl IntoView {
    let id = |name: &str| format!("{prefix}-{name}");
    view! {
        <fieldset class="grid gap-3">
            <legend class="mb-1 font-semibold">{t("admin-sso-rules")}</legend>
            <CheckField id=id("auto") label=t("admin-sso-auto-create") checked=f.auto_create/>
            <div class="grid gap-4 sm:grid-cols-2">
                <TextField id=id("claim") label=t("admin-sso-groups-claim") value=f.groups_claim hint=t("admin-sso-groups-claim-hint")/>
                <TextField id=id("allowed") label=t("admin-sso-allowed-groups") value=f.allowed_groups hint=t("admin-sso-allowed-groups-hint")/>
                <TextField id=id("admins") label=t("admin-sso-admin-groups") value=f.admin_groups hint=t("admin-sso-admin-groups-hint")/>
            </div>
            <CheckField id=id("roles") label=t("admin-sso-provider-roles") checked=f.provider_roles/>
            <p class="text-sm text-muted-foreground">{t("admin-sso-rules-hint")}</p>
        </fieldset>
    }
}

#[component]
fn ProviderDetail(p: OidcProviderDto, data: Data) -> impl IntoView {
    if p.provenance == "env" {
        return view! { <EnvProvider p=p data=data/> }.into_any();
    }
    let reauth = use_reauth();
    let f = ProviderForm::new(Some(&p));
    let prefix = format!("adm-sso-{}", p.slug);
    let busy = RwSignal::new(false);
    let message: Message = kept(&format!("sso-msg-{}", p.id), || None);
    let path = format!("/admin/oidc/providers/{}", p.id);
    let save = {
        let path = path.clone();
        move |_| {
            let body = serde_json::to_value(f.input(None)).unwrap_or_default();
            gated(
                reauth,
                busy,
                provider_call("PUT", path.clone(), body),
                after_change(message, data),
            );
        }
    };
    let confirm = RwSignal::new(false);
    let delete = Callback::new(move |()| {
        let path = path.clone();
        let call = move || {
            let path = path.clone();
            async move { rest::send("DELETE", &path, None).await }
        };
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
    let name = p.display_name.clone();
    let i18n = use_i18n();
    let confirm_title = i18n.t_args("admin-sso-delete-title", &[("name", name.into())]);
    view! {
        <div class="grid min-w-0 gap-5 rounded-xl border bg-card p-4">
            <ProviderFields f=f prefix=prefix.clone() creating=false/>
            <ClientSecret p=p.clone() data=data/>
            <AccountRules f=f prefix=prefix.clone()/>
            <Outcome message=message/>
            <div class="flex flex-wrap gap-2">
                <Button loading=busy on:click=save>{t("admin-save")}</Button>
                <Button variant=ButtonVariant::Ghost on:click=move |_| confirm.set(true)>{t("admin-sso-delete")}</Button>
            </div>
            <TestPanel p=p data=data/>
            <ConfirmDialog open=confirm title=Signal::stored(confirm_title) body=Signal::derive(move || t("admin-sso-delete-body"))
                confirm=t("admin-sso-delete") on_confirm=delete/>
        </div>
    }
    .into_any()
}

/// The provider from `auth.oidc` in the configuration: read-only, with its variable.
#[component]
fn EnvProvider(p: OidcProviderDto, data: Data) -> impl IntoView {
    let note = use_i18n().t_args(
        "admin-setting-env",
        &[(
            "var",
            p.env_var
                .clone()
                .unwrap_or_else(|| "APP__AUTH__OIDC".into())
                .into(),
        )],
    );
    view! {
        <div class="grid gap-3 rounded-xl border bg-card p-4 text-sm">
            <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-1">
                <dt class="font-medium">{t("admin-sso-client-id")}</dt><dd class="break-all font-mono">{p.client_id.clone()}</dd>
                <dt class="font-medium">{t("admin-sso-scopes")}</dt><dd>{p.scopes.join(" ")}</dd>
            </dl>
            <p class="flex flex-wrap items-center gap-2 text-muted-foreground">
                <Badge variant=BadgeVariant::Outline>{t("admin-setting-read-only")}</Badge>
                <span data-testid="setting-origin">{note}</span>
            </p>
            <TestPanel p=p data=data/>
        </div>
    }
}

/// The write-only client secret (addendum-1 2.3): "stored" line with Replace, never the value.
#[component]
fn ClientSecret(p: OidcProviderDto, data: Data) -> impl IntoView {
    let reauth = use_reauth();
    let value = RwSignal::new(String::new());
    let editing = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let message: Message = kept(&format!("sso-secret-{}", p.id), || None);
    let stored = p.secret_set.then(|| {
        t(if p.secret_locked {
            "admin-sso-secret-locked"
        } else {
            "admin-sso-secret-stored"
        })
    });
    let path = format!("/admin/oidc/providers/{}/secret", p.id);
    let id = format!("adm-sso-{}-secret", p.slug);
    let save = move |_| {
        let body = json!({"secret": value.get_untracked()});
        let done = after_change(message, data);
        gated(
            reauth,
            busy,
            provider_call("PUT", path.clone(), body),
            Callback::new(move |r| {
                value.set(String::new());
                editing.set(false);
                done.run(r);
            }),
        );
    };
    let show_save = move || !p.secret_set || editing.get();
    view! {
        <div class="grid gap-2">
            <SecretField id=id label=t("admin-sso-client-secret") value=value editing=editing stored=stored
                replace_label=t("admin-replace") cancel_label=t("reauth-cancel") description=t("admin-sso-client-secret-hint")/>
            <Show when=show_save>
                <Button variant=ButtonVariant::Outline class="w-fit" loading=busy on:click=save.clone()>{t("admin-sso-save-secret")}</Button>
            </Show>
            <Outcome message=message/>
        </div>
    }
}

/// Where a probe round trip starts (DEC-324), coming back here.
#[must_use]
pub fn probe_url(slug: &str, mode: &str) -> String {
    format!(
        "/api/v1/auth/oidc/start?provider={}&mode={mode}&next={}",
        encode(slug),
        encode(HERE)
    )
}

/// Test: discovery and what the provider advertises, plus the two browser probes (OD-19,
/// od-12 4.4 step 5).
#[component]
fn TestPanel(p: OidcProviderDto, data: Data) -> impl IntoView {
    let result = kept(&format!("sso-test-{}", p.id), || p.last_test.clone());
    let busy = RwSignal::new(false);
    let error: Message = RwSignal::new(None);
    let path = format!("/admin/oidc/providers/{}/test", p.id);
    let test = move |_| {
        let path = path.clone();
        let i18n = use_i18n();
        busy.set(true);
        leptos::task::spawn_local(async move {
            let r = rest::call::<OidcTestDto>("POST", &path, Some(json!({}))).await;
            busy.set(false);
            match r {
                Ok(t) => {
                    error.set(None);
                    result.set(t);
                    data.refetch();
                }
                Err(e) => error.set(Some(Err(crate::settings::admin_error_with(&i18n, &e)))),
            }
        });
    };
    let slug = p.slug.clone();
    view! {
        <section class="grid gap-3">
            <h3 class="font-semibold">{t("admin-sso-test-title")}</h3>
            <Button variant=ButtonVariant::Outline class="w-fit" loading=busy on:click=test>{t("admin-sso-test")}</Button>
            <Outcome message=error/>
            {move || result.get().map(|r| view! { <TestOutcome r=r/> })}
            <p class="flex flex-wrap gap-4 text-sm">
                <a class="underline" rel="external" href=probe_url(&slug, "probe_silent")>{t("admin-sso-probe-silent")}</a>
                <a class="underline" rel="external" href=probe_url(&slug, "probe_max_age")>{t("admin-sso-probe-max-age")}</a>
            </p>
            <p class="text-sm text-muted-foreground">{t("admin-sso-probe-hint")}</p>
        </section>
    }
}

fn yes_no(v: bool, yes: &str, no: &str) -> String {
    t(if v { yes } else { no })
}

/// The outcome lines of a Test, in words.
#[component]
fn TestOutcome(r: OidcTestDto) -> impl IntoView {
    let i18n = use_i18n();
    let discovery = if r.discovery == "ok" {
        t("admin-sso-test-discovery-ok")
    } else {
        let reason = r.discovery.trim_start_matches("failed: ").to_string();
        i18n.t_args(
            "admin-sso-test-discovery-failed",
            &[("reason", reason.into())],
        )
    };
    let groups = t(match r.groups_claim_seen {
        Some(true) => "admin-sso-test-groups-seen",
        Some(false) => "admin-sso-test-groups-missing",
        None => "admin-sso-test-groups-unknown",
    });
    let silent = r
        .silent_probe
        .as_deref()
        .map_or_else(|| t("admin-sso-probe-not-run"), probe_text);
    let max_age = t(match r.max_age_probe {
        Some(true) => "admin-sso-probe-max_age_ok",
        Some(false) => "admin-sso-probe-max_age_ignored",
        None => "admin-sso-probe-not-run",
    });
    let lines = vec![
        (t("admin-sso-test-discovery"), discovery),
        (
            t("admin-sso-test-end-session"),
            yes_no(r.end_session, "admin-sso-offered", "admin-sso-not-offered"),
        ),
        (
            t("admin-sso-test-backchannel"),
            yes_no(
                r.backchannel_logout,
                "admin-sso-offered",
                "admin-sso-not-offered",
            ),
        ),
        (
            t("admin-sso-test-frontchannel"),
            yes_no(
                r.frontchannel_logout,
                "admin-sso-offered-unused",
                "admin-sso-not-offered",
            ),
        ),
        (
            t("admin-sso-test-pkce"),
            yes_no(
                r.pkce_s256,
                "admin-sso-supported",
                "admin-sso-not-advertised",
            ),
        ),
        (
            t("admin-sso-test-iss"),
            yes_no(
                r.iss_parameter,
                "admin-sso-supported",
                "admin-sso-not-advertised",
            ),
        ),
        (t("admin-sso-test-groups"), groups),
        (t("admin-sso-test-silent"), silent),
        (t("admin-sso-test-max-age"), max_age),
    ];
    let when = i18n.t_args("admin-sso-test-at", &[("at", i18n.datetime(&r.at).into())]);
    view! {
        <div data-testid="sso-test-result" class="grid gap-2">
            <p class="text-sm font-medium">{if r.ok { t("admin-sso-test-ok") } else { t("admin-sso-test-failed") }} " " {when}</p>
            <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-1 text-sm">
                {lines.into_iter().map(|(k, v)| view! { <dt class="font-medium">{k}</dt><dd class="min-w-0 break-all">{v}</dd> }).collect_view()}
            </dl>
        </div>
    }
}

/// "Add a provider" (addendum-1 3.4): name, issuer, client id, the write-only secret; Test
/// then runs discovery from the row.
#[component]
fn NewProvider(data: Data) -> impl IntoView {
    let reauth = use_reauth();
    let f = ProviderForm::new(None);
    let secret = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let message: Message = kept("sso-new", || None);
    let open = kept("sso-new-open", || false);
    // the slug follows the name until the admin types one
    Effect::new(move |prev: Option<String>| {
        let auto = ui::slug(&f.display_name.get());
        if prev.as_ref().is_none_or(|p| *p == f.slug.get_untracked()) {
            f.slug.set(auto.clone());
        }
        auto
    });
    let add = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let pw = secret.get_untracked();
        let body =
            serde_json::to_value(f.input((!pw.is_empty()).then_some(pw))).unwrap_or_default();
        let done = after_change(message, data);
        gated(
            reauth,
            busy,
            provider_call("POST", "/admin/oidc/providers".into(), body),
            Callback::new(move |r: Result<OidcProviderDto, String>| {
                if let Ok(p) = &r {
                    kept(&format!("sso-open-{}", p.id), || true).set(true);
                    secret.set(String::new());
                }
                done.run(r);
            }),
        );
    };
    view! {
        <Disclosure id="adm-sso-add" title=t("admin-sso-add") summary=t("admin-sso-add-summary")
            mode=DisclosureMode::Button level=2 open=open class="mb-8 max-w-3xl">
            <form class="grid gap-4" on:submit=add>
                <ProviderFields f=f prefix="adm-sso-new" creating=true/>
                <FormField id="adm-sso-new-secret" label=t("admin-sso-client-secret") description=t("admin-sso-client-secret-hint")>
                    <Input id="adm-sso-new-secret" r#type="password" value=secret autocomplete="off"
                        described_by=field_described_by("adm-sso-new-secret")/>
                </FormField>
                <AccountRules f=f prefix="adm-sso-new"/>
                <Outcome message=message/>
                <Button r#type="submit" class="w-fit" loading=busy>{t("admin-sso-add-submit")}</Button>
            </form>
        </Disclosure>
    }
}

/// The sign-in mode (addendum-1 4.3, mockup annotation 10): the consequence in the label,
/// the server's preconditions refused inline (`auto_needs_*`, DEC-321).
#[component]
fn Mode(s: SettingDto, provider: Option<String>) -> impl IntoView {
    let reauth = use_reauth();
    let fixed = origin(&s) != Origin::Editable;
    let value = RwSignal::new(s.value.as_str().unwrap_or("page").to_string());
    let busy = RwSignal::new(false);
    let message: Message = kept("sso-mode", || None);
    let i18n = use_i18n();
    let name = provider.unwrap_or_else(|| t("signin-provider-fallback"));
    let auto_label = i18n.t_args("admin-sso-mode-auto", &[("provider", name.clone().into())]);
    let auto_hint = i18n.t_args("admin-sso-mode-auto-hint", &[("provider", name.into())]);
    let options = [
        (
            "page",
            t("admin-sso-mode-page"),
            t("admin-sso-mode-page-hint"),
        ),
        ("auto", auto_label, auto_hint),
    ];
    let save = move || {
        let saved = i18n.t("admin-saved");
        let values = std::collections::BTreeMap::from([(
            "auth.sign_in_mode".to_string(),
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
                <legend class="mb-2 text-lg font-semibold">{t("admin-sso-mode")}</legend>
                {options.into_iter().map(|(mode, label, hint)| {
                    let id = format!("adm-mode-{mode}");
                    let hint_id = format!("{id}-hint");
                    view! {
                        <div class="flex items-start gap-3">
                            <input type="radio" name="sign-in-mode" id=id.clone() value=mode disabled=fixed
                                aria-describedby=hint_id.clone() class="mt-1 size-4 accent-primary"
                                prop:checked=move || value.get() == mode
                                on:change=move |_| value.set(mode.to_string())/>
                            <span class="grid gap-0.5">
                                <label for=id class="text-sm font-medium">{label}</label>
                                <span id=hint_id class="text-sm text-muted-foreground">{hint}</span>
                            </span>
                        </div>
                    }
                }).collect_view()}
            </fieldset>
            <p class="mt-2 text-sm text-muted-foreground">{t("admin-sso-mode-needs")}</p>
            <div class="mt-3 grid gap-3">
                <SettingNote s=s.clone()/>
                <Outcome message=message/>
                {(!fixed).then(|| view! { <Button r#type="submit" class="w-fit" loading=busy>{t("admin-save")}</Button> })}
            </div>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_lists_and_probe_links() {
        assert_eq!(
            words("media-admins, family  kids,"),
            ["media-admins", "family", "kids"]
        );
        assert_eq!(words(" "), Vec::<String>::new());
        assert_eq!(
            probe_url("pocket", "probe_silent"),
            "/api/v1/auth/oidc/start?provider=pocket&mode=probe_silent&next=%2Fadmin%2Fsso"
        );
    }
}
