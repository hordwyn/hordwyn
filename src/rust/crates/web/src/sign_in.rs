//! Sign-in with identity providers (addendum-1 4.3-4.5, design.md 5.2 and 6.14; DEC-326):
//! `/sign-in` with the provider buttons first and the password form in a disclosure (open
//! when there is no provider or the provider refused), the `?error=` states (the outage as
//! an alert with Try again and the break-glass link, the refusals in words) and "Your
//! session ended"; the break-glass `/sign-in/local`; `/signed-out`, which never signs
//! anyone in again (OD-20); "Is this your account?" at `/sign-in/link`; and the one-time
//! password reset link `/reset-password/:token` (DEC-301).

use std::fmt::Write as _;

use contracts::SessionDto;
use contracts::admin::OidcLinkPendingDto;
use contracts::auth::AuthField;
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_params_map, use_query_map};
use serde_json::json;
use ui::{Alert, AlertRole, AlertVariant, FormField, Input, Skeleton, field_described_by};

use crate::admin_api::{self, SignInPageDto};
use crate::api;
use crate::auth::{password_error, password_hint, password_rules, sign_in_errors};
use crate::auth_pages::{
    AuthFrame, FormAlert, PasswordInput, SignInForm, SubmitButton, Summary, TotpStep, error_of,
};
use crate::i18n::{t, use_i18n};
use crate::pages::{PageTitle, go};
use crate::rest;

/// A link that looks like the primary button (provider buttons, "Sign in" on `/signed-out`).
const LINK_BUTTON: &str = "inline-flex min-h-target-touch w-full items-center justify-center gap-2 \
    rounded-md bg-primary px-6 text-sm font-medium text-primary-foreground hover:bg-primary/90 \
    focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2";

/// The provider error codes the sign-in page explains (DEC-311, DEC-326); anything else reads
/// as `oidc_failed`.
const REFUSALS: [&str; 6] = [
    "oidc_denied",
    "group_rule",
    "no_account",
    "account_disabled",
    "identity_in_use",
    "oidc_failed",
];

/// A return path that stays on this site (`/x`, never `//x` or `/\x`), like the server's
/// check of `next`.
#[must_use]
pub fn local_path(p: &str) -> Option<String> {
    let ok = p.starts_with('/')
        && !p.starts_with("//")
        && !p.starts_with("/\\")
        && !p.chars().any(char::is_control);
    ok.then(|| p.to_string())
}

/// Percent-encode a query value (everything but the RFC 3986 unreserved characters).
#[must_use]
pub fn encode(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Where "Continue with <provider>" starts the interactive flow (DEC-311).
#[must_use]
pub fn start_url(slug: &str, next: Option<&str>) -> String {
    let mut url = format!("/api/v1/auth/oidc/start?provider={}", encode(slug));
    if let Some(n) = next {
        url.push_str("&next=");
        url.push_str(&encode(n));
    }
    url
}

/// The message key of a refusal code (`signin-error-<code>`).
#[must_use]
pub fn refusal_key(code: &str) -> String {
    let known = REFUSALS.contains(&code);
    format!("signin-error-{}", if known { code } else { "oidc_failed" })
}

/// The query parameter `name` of the current address.
fn query(name: &'static str) -> Option<String> {
    use_query_map().with_untracked(|q| q.get(name))
}

#[component]
pub fn SignIn() -> impl IntoView {
    let cfg = expect_context::<crate::ClientConfig>();
    let error = query("error");
    let capped = query("retry").as_deref() == Some("0");
    let next = query("next").and_then(|n| local_path(&n));
    let data = Resource::new(|| (), |()| admin_api::sign_in_page());
    let ended = cfg.session_ended && error.is_none();
    view! {
        <AuthFrame>
            <PageTitle text=t("auth-sign-in-title")/>
            <Suspense fallback=|| view! { <Skeleton class="h-48 w-full"/> }>
                {move || data.get().map(|r| {
                    let page = r.unwrap_or_default();
                    view! { <SignInBody page=page error=error.clone() capped=capped next=next.clone() ended=ended/> }
                })}
            </Suspense>
        </AuthFrame>
    }
}

/// The outage alert (addendum-1 4.4, mockup state 3): what happened, that data is safe,
/// Try again (not after the retry cap) and the break-glass link.
#[component]
fn Outage(provider: String, retry: Option<String>) -> impl IntoView {
    let i18n = use_i18n();
    let title = i18n.t_args("signin-outage-title", &[("provider", provider.into())]);
    let body = t(if retry.is_some() {
        "signin-outage-body"
    } else {
        "signin-outage-capped"
    });
    view! {
        <Alert variant=AlertVariant::Warning role=AlertRole::Alert title=title class="mb-4">
            <p>{body}</p>
            <p class="mt-2 flex flex-wrap gap-4">
                {retry.map(|href| view! { <a class="font-medium underline" rel="external" href=href>{t("signin-retry")}</a> })}
                <a class="underline" href="/sign-in/local">{t("signin-break-glass")}</a>
            </p>
        </Alert>
    }
}

/// The page under its heading: a notice, the provider buttons, the password form.
#[component]
fn SignInBody(
    page: SignInPageDto,
    error: Option<String>,
    capped: bool,
    next: Option<String>,
    ended: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let provider = page
        .default_provider()
        .map_or_else(|| t("signin-provider-fallback"), |p| p.display_name.clone());
    let outage = error.as_deref() == Some("oidc_unavailable");
    let notice = match error.as_deref() {
        Some("oidc_unavailable") => {
            let retry = (!capped)
                .then(|| page.default_provider().map(|p| start_url(&p.slug, next.as_deref())))
                .flatten();
            view! { <Outage provider=provider.clone() retry=retry/> }.into_any()
        }
        Some(code) => {
            let text = i18n.t_args(&refusal_key(code), &[("provider", provider.clone().into())]);
            view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert class="mb-4">{text}</Alert> }.into_any()
        }
        None if ended => view! {
            <Alert variant=AlertVariant::Info role=AlertRole::Status title=t("signin-session-ended-title") class="mb-4">
                {t("signin-session-ended-body")}
            </Alert>
        }
        .into_any(),
        None => ().into_any(),
    };
    let buttons = page
        .providers
        .iter()
        .map(|p| {
            let label = i18n.t_args("signin-continue", &[("provider", p.display_name.clone().into())]);
            view! { <li><a class=LINK_BUTTON rel="external" href=start_url(&p.slug, next.as_deref())>{label}</a></li> }
        })
        .collect_view();
    let has_providers = !page.providers.is_empty();
    let sso_hint = page
        .auto
        .then(|| i18n.t_args("signin-sso-only", &[("provider", provider.clone().into())]));
    let form = view! { <PasswordSignIn local=false next=next.clone() sign_up=!page.auto/> };
    let form = if has_providers {
        view! {
            <details data-testid="password-disclosure" open=error.is_some() class="mt-4">
                <summary class="min-h-target-touch cursor-pointer py-2 text-sm font-medium underline-offset-4 hover:underline">
                    {t("signin-password-instead")}
                </summary>
                <div class="mt-2">{form}</div>
            </details>
        }
        .into_any()
    } else {
        form.into_any()
    };
    // the break-glass page is linked from the outage notice and, in auto mode, here
    let footer = (page.auto && !outage).then(|| view! {
        <p class="mt-4 text-sm"><a class="underline" href="/sign-in/local">{t("signin-break-glass")}</a></p>
    });
    view! {
        {notice}
        {has_providers.then(|| view! { <ul class="grid gap-3">{buttons}</ul> })}
        {sso_hint.map(|h| view! { <p class="mt-3 text-sm text-muted-foreground">{h}</p> })}
        {form}
        {footer}
    }
}

/// Where a successful sign-in goes: the `next` path, else the library (Home after the
/// break-glass sign-in).
fn after_sign_in(next: Option<&str>, local: bool) -> String {
    next.map_or_else(
        || if local { "/" } else { "/library" }.to_string(),
        str::to_string,
    )
}

/// The username and password form (design.md 5.2), with the authenticator step; `local`
/// posts to the break-glass endpoint (DEC-322).
#[component]
fn PasswordSignIn(
    local: bool,
    next: Option<String>,
    #[prop(optional)] sign_up: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let f = SignInForm::new();
    let to = after_sign_in(next.as_deref(), local);
    let submit = {
        let i18n = i18n.clone();
        Action::new(move |creds: &(String, String, String)| {
            let ((user, pass, code), i18n, to) = (creds.clone(), i18n.clone(), to.clone());
            async move {
                let totp = (!code.is_empty()).then_some(code);
                let r = if local {
                    let body = json!({"username": user, "password": pass, "totp": totp});
                    rest::call::<SessionDto>("POST", "/auth/sign-in/local", Some(body))
                        .await
                        .map(|_| ())
                } else {
                    api::sign_in(user, pass, totp).await.map(|_| ())
                };
                match r {
                    Ok(()) => go(&to),
                    Err(err) => {
                        f.server_error(&i18n, &api::error_code(&err), api::retry_after_secs(&err));
                    }
                }
            }
        })
    };
    let on_submit = {
        let i18n = i18n.clone();
        move |ev: SubmitEvent| {
            ev.prevent_default();
            f.form_error.set(None);
            let (u, p, c) = (
                f.username.get_untracked(),
                f.password.get_untracked(),
                f.totp.get_untracked(),
            );
            let mut errs = sign_in_errors(&i18n, &u, &p);
            if f.need_totp.get_untracked() && c.trim().is_empty() {
                errs.push((AuthField::Totp, i18n.t("auth-totp-required")));
            }
            let ok = errs.is_empty();
            f.show(errs);
            if ok {
                submit.dispatch((u, p, c.trim().to_string()));
            }
        }
    };
    let user_err = Signal::derive(move || error_of(f.errors, AuthField::Username));
    let pass_err = Signal::derive(move || error_of(f.errors, AuthField::Password));
    let (link, have) = (i18n.t("nav-sign-up"), i18n.t("auth-no-account"));
    view! {
        <form class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm" novalidate on:submit=on_submit>
            <Summary errors=f.summary/>
            <div class="grid gap-4" hidden=move || f.need_totp.get()>
                <FormField id="username" label=i18n.t("auth-username") error=user_err>
                    <Input id="username" value=f.username autocomplete="username"
                        described_by=field_described_by("username")
                        invalid=Signal::derive(move || user_err.with(Option::is_some))/>
                </FormField>
                <FormField id="password" label=i18n.t("auth-password") error=pass_err>
                    <PasswordInput value=f.password autocomplete="current-password"
                        invalid=Signal::derive(move || pass_err.with(Option::is_some))/>
                </FormField>
            </div>
            <Show when=move || f.need_totp.get()>
                <TotpStep f=f/>
            </Show>
            <SubmitButton pending=submit.pending() label=i18n.t("auth-sign-in-submit")/>
            <FormAlert error=f.form_error/>
            {sign_up.then(|| view! {
                <p class="text-sm text-muted-foreground">
                    {have.clone()} " " <A href="/sign-up" attr:class="underline">{link.clone()}</A>
                </p>
            })}
        </form>
    }
}

/// `/sign-in/local` (addendum-1 4.3, OD-17 as changed): never linked from the normal flow,
/// never redirected; in `auto` mode only admins get in.
#[component]
pub fn LocalSignIn() -> impl IntoView {
    let next = query("next").and_then(|n| local_path(&n));
    view! {
        <AuthFrame>
            <PageTitle text=t("signin-local-title")/>
            <p class="-mt-3 mb-4 text-sm text-muted-foreground">{t("signin-local-lead")}</p>
            <PasswordSignIn local=true next=next/>
            <p class="mt-4 text-sm text-muted-foreground">{t("signin-local-audit")}</p>
        </AuthFrame>
    }
}

/// `/signed-out` (OD-20): says so and offers a way back in, never sends anyone anywhere.
#[component]
pub fn SignedOut() -> impl IntoView {
    view! {
        <AuthFrame>
            <PageTitle text=t("signed-out-title")/>
            <p class="-mt-3 mb-6 text-muted-foreground">{t("signed-out-lead")}</p>
            <a class=LINK_BUTTON href="/sign-in">{t("nav-sign-in")}</a>
        </AuthFrame>
    }
}

/// `/sign-in/link`, "Is this your account?" (addendum-1 4.2 step 3, OD-15): the provider's
/// verified address matches an account here; its password links them, once. The ticket
/// cookie lives on the API path, so the page asks the API from the browser.
#[component]
pub fn LinkAccount() -> impl IntoView {
    let pending = RwSignal::new(None::<Result<OidcLinkPendingDto, String>>);
    Effect::new(move |_| {
        leptos::task::spawn_local(async move {
            let r = rest::call::<OidcLinkPendingDto>("GET", "/auth/oidc/link", None).await;
            pending.set(Some(match r {
                Ok(Some(p)) => Ok(p),
                Ok(None) => Err("link_expired".into()),
                Err(e) => Err(api::error_code(&e)),
            }));
        });
    });
    view! {
        <AuthFrame>
            <PageTitle text=t("signin-link-title")/>
            {move || match pending.get() {
                None => view! { <Skeleton class="h-40 w-full"/> }.into_any(),
                Some(Ok(p)) => view! { <LinkForm p=p/> }.into_any(),
                Some(Err(code)) => view! { <LinkGone code=code/> }.into_any(),
            }}
        </AuthFrame>
    }
}

/// The link request expired or was used: start again.
#[component]
fn LinkGone(code: String) -> impl IntoView {
    let key = if code == "identity_in_use" {
        "signin-error-identity_in_use"
    } else {
        "signin-link-expired"
    };
    view! {
        <Alert variant=AlertVariant::Destructive role=AlertRole::Alert class="mb-4">{t(key)}</Alert>
        <a class="underline" href="/sign-in">{t("signin-link-back")}</a>
    }
}

#[component]
fn LinkForm(p: OidcLinkPendingDto) -> impl IntoView {
    let i18n = use_i18n();
    let lead = i18n.t_args(
        "signin-link-lead",
        &[
            ("provider", p.provider.clone().into()),
            ("username", p.username.clone().into()),
        ],
    );
    if !p.has_password {
        return view! {
            <p class="mb-4">{lead}</p>
            <Alert variant=AlertVariant::Info role=AlertRole::None class="mb-4">{t("signin-link-no-password")}</Alert>
            <a class="underline" href="/sign-in">{t("signin-link-back")}</a>
        }
        .into_any();
    }
    let password = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let need_code = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let submit = Action::new(move |(): &()| {
        let (pw, c) = (password.get_untracked(), code.get_untracked());
        let i18n = use_i18n();
        async move {
            let totp = (!c.trim().is_empty()).then(|| c.trim().to_string());
            let body = json!({"password": pw, "totp": totp});
            match rest::call::<SessionDto>("POST", "/auth/oidc/link", Some(body)).await {
                Ok(_) => go("/"),
                Err(e) if api::error_code(&e) == "totp_required" => need_code.set(true),
                Err(e) => error.set(Some(crate::auth::form_message_after(
                    &i18n,
                    &api::error_code(&e),
                    api::retry_after_secs(&e),
                ))),
            }
        }
    });
    let label = i18n.t_args(
        "signin-link-password",
        &[("username", p.username.clone().into())],
    );
    view! {
        <p class="mb-4">{lead}</p>
        <form class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm" novalidate
            on:submit=move |ev: SubmitEvent| { ev.prevent_default(); error.set(None); submit.dispatch(()); }>
            <FormField id="password" label=label>
                <PasswordInput value=password autocomplete="current-password" invalid=Signal::derive(move || error.with(Option::is_some))/>
            </FormField>
            <Show when=move || need_code.get()>
                <FormField id="totp" label=t("auth-totp") description=t("auth-totp-lead")>
                    <Input id="totp" value=code inputmode="numeric" autocomplete="one-time-code"
                        described_by=field_described_by("totp")/>
                </FormField>
            </Show>
            <SubmitButton pending=submit.pending() label=t("signin-link-submit")/>
            <FormAlert error=error/>
        </form>
        <p class="mt-4 text-sm"><a class="underline" href="/sign-in">{t("signin-link-not-mine")}</a></p>
    }
    .into_any()
}

/// `/reset-password/:token` (DEC-301): a one-time link from `cli admin reset-password`;
/// a new password ends every session of the account.
#[component]
pub fn ResetPassword() -> impl IntoView {
    let token = use_params_map().with_untracked(|p| p.get("token").unwrap_or_default());
    let i18n = use_i18n();
    let password = RwSignal::new(String::new());
    let submitted = RwSignal::new(false);
    let done = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let submit = Action::new(move |pw: &String| {
        let (pw, token) = (pw.clone(), token.clone());
        let i18n = use_i18n();
        async move {
            let body = json!({"token": token, "password": pw});
            match rest::send("POST", "/auth/reset-password", Some(body)).await {
                Ok(()) => done.set(true),
                Err(e) => {
                    let code = api::error_code(&e);
                    let key = format!("signin-reset-error-{code}");
                    let text = i18n.t(&key);
                    error.set(Some(if text == key {
                        crate::auth::form_message(&i18n, &code)
                    } else {
                        text
                    }));
                }
            }
        }
    });
    let pr = i18n.clone();
    let rules = Signal::derive(move || password_rules(&pr, &password.get()));
    let pe = i18n.clone();
    let field_error = Signal::derive(move || {
        submitted
            .get()
            .then(|| password_error(&pe, &password.get()))
            .flatten()
    });
    let on_submit = move |ev: SubmitEvent| {
        ev.prevent_default();
        submitted.set(true);
        error.set(None);
        if field_error.get_untracked().is_none() {
            submit.dispatch(password.get_untracked());
        }
    };
    view! {
        <AuthFrame>
            <PageTitle text=t("signin-reset-title")/>
            <Show when=move || done.get() fallback=move || view! {
                <form class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm" novalidate on:submit=on_submit>
                    <p class="text-sm text-muted-foreground">{t("signin-reset-lead")}</p>
                    <FormField id="password" label=t("settings-password-new") description=password_hint(&use_i18n())
                        rules=rules rule_met_label=t("auth-rule-met") rule_unmet_label=t("auth-rule-unmet") error=field_error>
                        <PasswordInput value=password autocomplete="new-password"
                            invalid=Signal::derive(move || field_error.with(Option::is_some))/>
                    </FormField>
                    <SubmitButton pending=submit.pending() label=t("signin-reset-submit")/>
                    <FormAlert error=error/>
                </form>
            }>
                <Alert variant=AlertVariant::Success role=AlertRole::Status class="mb-4">{t("signin-reset-done")}</Alert>
                <a class=LINK_BUTTON href="/sign-in">{t("nav-sign-in")}</a>
            </Show>
        </AuthFrame>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_paths_stay_on_this_site_and_are_encoded() {
        assert_eq!(local_path("/history?x=1"), Some("/history?x=1".into()));
        for bad in [
            "//evil.example",
            "/\\evil",
            "https://evil.example",
            "",
            "/a\nb",
        ] {
            assert_eq!(local_path(bad), None, "{bad:?}");
        }
        assert_eq!(
            start_url("pocket", Some("/items/1?tab=a b")),
            "/api/v1/auth/oidc/start?provider=pocket&next=%2Fitems%2F1%3Ftab%3Da%20b"
        );
        assert_eq!(
            start_url("pocket", None),
            "/api/v1/auth/oidc/start?provider=pocket"
        );
        assert_eq!(after_sign_in(None, false), "/library");
        assert_eq!(after_sign_in(None, true), "/");
        assert_eq!(after_sign_in(Some("/stats"), true), "/stats");
    }

    #[test]
    fn unknown_refusals_read_as_a_failed_sign_in() {
        assert_eq!(refusal_key("group_rule"), "signin-error-group_rule");
        assert_eq!(refusal_key("not_found"), "signin-error-oidc_failed");
        assert_eq!(refusal_key("<script>"), "signin-error-oidc_failed");
    }
}
