//! Sign-up (`/sign-up`, `/invite/:token`) and sign-in (`/sign-in`) with form discipline
//! (design.md 5.2, 6.14): rules before typing, a live checklist, live normalisation, an
//! availability check after a pause, inline errors plus an error summary that takes focus,
//! and one alert for errors about the whole form. The logic lives in [`crate::auth`].

use std::sync::Arc;
use std::time::Duration;

use contracts::auth::{AuthField, UsernameAvailability, check_username, normalize_username};
use leptos::ev::SubmitEvent;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant, ErrorLink, ErrorSummary,
    FormField, Input, field_described_by,
};

use crate::api;
use crate::auth::{
    Placement, SignUpValues, check_throttled_message, display_name_error, field_id, form_message,
    form_message_after, password_error, password_hint, password_rules, placement,
    server_field_message, sign_up_errors, summary_title, username_error, username_hint,
    username_rules, username_saved_as,
};
use crate::i18n::{I18n, use_i18n};
use crate::pages::{PageTitle, go};

/// Pause after the last keystroke before the availability check (the server allows 30
/// checks a minute for the whole instance, so never per keystroke).
const CHECK_DELAY: Duration = Duration::from_millis(400);

/// Messages per field, in field order.
pub(crate) type FieldErrors = RwSignal<Vec<(AuthField, String)>>;

pub(crate) fn summary_links(errors: &[(AuthField, String)]) -> Vec<ErrorLink> {
    errors
        .iter()
        .map(|(f, m)| ErrorLink::new(field_id(*f), m.clone()))
        .collect()
}

pub(crate) fn error_of(errors: FieldErrors, field: AuthField) -> Option<String> {
    errors.with(|e| e.iter().find(|(f, _)| *f == field).map(|(_, m)| m.clone()))
}

/// Drop the stored (server) error of `field` once its value changes.
pub(crate) fn clear_on_change(errors: FieldErrors, field: AuthField, value: RwSignal<String>) {
    Effect::new(move |prev: Option<String>| {
        let v = value.get();
        if prev.is_some_and(|p| p != v) {
            errors.update(|e| e.retain(|(f, _)| *f != field));
        }
        v
    });
}

/// Move focus to the element `id` (browser only).
pub(crate) fn focus(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}

/// The label of a brand legal link: `legal-<key>` if there is such a message, else the key.
fn legal_label(i18n: &I18n, key: &str) -> String {
    let id = format!("legal-{key}");
    let text = i18n.t(&id);
    if text == id { key.to_string() } else { text }
}

/// The frame of the auth pages (design.md 6.14): the brand mark above a centred 28 rem card
/// and a footer with the brand's legal links.
#[component]
pub(crate) fn AuthFrame(children: Children) -> impl IntoView {
    let cfg = expect_context::<crate::ClientConfig>();
    let i18n = use_i18n();
    let legal = cfg.legal.clone();
    let footer = (!legal.is_empty()).then(|| {
        let links = legal
            .into_iter()
            .map(|(key, url)| {
                let label = legal_label(&i18n, &key);
                view! { <li><a href=url class="underline">{label}</a></li> }
            })
            .collect_view();
        view! {
            <footer class="mt-8 text-small text-muted-foreground">
                <nav aria-label=i18n.t("legal-nav")>
                    <ul class="flex flex-wrap justify-center gap-4">{links}</ul>
                </nav>
            </footer>
        }
    });
    view! {
        <div class="mx-auto w-full max-w-md">
            <p data-testid="brand-mark" class="mb-6 text-center text-h2 font-semibold tracking-tight">
                {cfg.product_name.clone()}
            </p>
            {children()}
            {footer}
        </div>
    }
}

/// The submit button of an auth form; disabled with "You are offline" under it while the
/// browser is offline (design.md 5.2).
#[component]
pub(crate) fn SubmitButton(#[prop(into)] pending: Signal<bool>, label: String) -> impl IntoView {
    let online = use_context::<crate::shell::Online>().map_or_else(|| RwSignal::new(true), |o| o.0);
    let offline_text = use_i18n().t("auth-offline");
    view! {
        <div class="grid gap-1">
            <Button r#type="submit" size=ButtonSize::Lg class="min-h-target-touch" loading=pending
                disabled=Signal::derive(move || !online.get())>
                {label}
            </Button>
            <Show when=move || !online.get()>
                <p class="text-small text-muted-foreground">{offline_text.clone()}</p>
            </Show>
        </div>
    }
}

/// The error summary; rebuilt on every submit with errors, so it takes focus each time.
#[component]
pub(crate) fn Summary(errors: RwSignal<Vec<ErrorLink>>) -> impl IntoView {
    let i18n = use_i18n();
    move || {
        let list = errors.get();
        let title = summary_title(&i18n, list.len());
        view! { <ErrorSummary id="error-summary" title=title errors=Signal::derive(move || list.clone())/> }
    }
}

/// One alert for errors about the whole form (closed sign-up, wrong credentials, throttle).
#[component]
pub(crate) fn FormAlert(error: RwSignal<Option<String>>) -> impl IntoView {
    move || {
        error.get().map(|e| {
            view! {
                <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{e}</Alert>
            }
        })
    }
}

/// Password input with a Show toggle (`aria-pressed`) in the trailing slot.
#[component]
pub(crate) fn PasswordInput(
    value: RwSignal<String>,
    autocomplete: &'static str,
    #[prop(into)] invalid: Signal<bool>,
) -> impl IntoView {
    let shown = RwSignal::new(false);
    let r#type = Signal::derive(move || if shown.get() { "text" } else { "password" }.to_string());
    let label = use_i18n().t("auth-show");
    view! {
        <Input
            id="password"
            value=value
            r#type=r#type
            autocomplete=autocomplete
            described_by=field_described_by("password")
            invalid=invalid
            trailing=move || {
                let label = label.clone();
                view! {
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm class="h-9"
                        attr:aria-controls="password"
                        attr:aria-pressed=move || if shown.get() { "true" } else { "false" }
                        on:click=move |_| shown.update(|s| *s = !*s)>
                        {label}
                    </Button>
                }
            }
        />
    }
}

/// State of the username field: the latest availability answer and the debounce timer.
#[derive(Clone, Copy)]
struct Availability {
    answer: RwSignal<Option<UsernameAvailability>>,
    /// The last check was throttled (the instance-wide limit, DEC-184), not the person.
    throttled: RwSignal<bool>,
    asked: StoredValue<String>,
    timer: StoredValue<Option<TimeoutHandle>>,
    check: Action<String, ()>,
}

impl Availability {
    fn new(invite: Option<String>, form_error: RwSignal<Option<String>>, i18n: Arc<I18n>) -> Self {
        let answer = RwSignal::new(None::<UsernameAvailability>);
        let throttled = RwSignal::new(false);
        let check = Action::new(move |name: &String| {
            let (name, invite, i18n) = (name.clone(), invite.clone(), i18n.clone());
            async move {
                match api::username_available(name, invite).await {
                    Ok(a) => {
                        throttled.set(false);
                        answer.set(Some(a));
                    }
                    // closed sign-up or a dead invite: say so before the person submits
                    Err(e) => {
                        let code = api::error_code(&e);
                        if matches!(code.as_str(), "signup_closed" | "invalid_invite") {
                            form_error.set(Some(form_message(&i18n, &code)));
                        }
                        throttled.set(code == "too_many_attempts");
                    }
                }
            }
        });
        Self {
            answer,
            throttled,
            asked: StoredValue::new(String::new()),
            timer: StoredValue::new(None),
            check,
        }
    }

    /// Ask the server about `normalized` now, once per distinct value.
    fn ask(self, normalized: String) {
        if let Some(t) = self.timer.get_value() {
            t.clear();
        }
        if check_username(&normalized).is_err() || self.asked.get_value() == normalized {
            return;
        }
        self.asked.set_value(normalized.clone());
        self.check.dispatch(normalized);
    }

    /// Ask after [`CHECK_DELAY`] unless the value changes again.
    fn ask_later(self, value: RwSignal<String>) {
        if let Some(t) = self.timer.get_value() {
            t.clear();
        }
        let normalized = normalize_username(&value.get_untracked());
        if check_username(&normalized).is_err() {
            return;
        }
        let handle = set_timeout_with_handle(
            move || {
                if normalize_username(&value.get_untracked()) == normalized {
                    self.ask(normalized);
                }
            },
            CHECK_DELAY,
        );
        self.timer.set_value(handle.ok());
    }

    /// Suggestions when the server said `normalized` is taken.
    fn taken(self, normalized: &str) -> Option<Vec<String>> {
        self.answer.with(|a| match a {
            Some(UsernameAvailability::Taken {
                username,
                suggestions,
            }) if username == normalized => Some(suggestions.clone()),
            _ => None,
        })
    }

    fn available(self, normalized: &str) -> bool {
        self.answer.with(|a| {
            matches!(a, Some(UsernameAvailability::Available { username }) if username == normalized)
        })
    }

    /// The server said "taken" on submit: show it now, fetch suggestions.
    fn mark_taken(self, normalized: String) {
        self.answer.set(Some(UsernameAvailability::Taken {
            username: normalized.clone(),
            suggestions: Vec::new(),
        }));
        self.asked.set_value(String::new());
        self.ask(normalized);
    }
}

/// Free usernames as buttons that fill the field.
#[component]
fn Suggestions(username: RwSignal<String>, avail: Availability) -> impl IntoView {
    let i18n = use_i18n();
    let group = i18n.t("auth-username-suggestions");
    move || {
        let normalized = normalize_username(&username.get());
        let list = avail.taken(&normalized).filter(|s| !s.is_empty())?;
        let i18n = i18n.clone();
        Some(view! {
            <div role="group" aria-label=group.clone() class="-mt-2 flex flex-wrap gap-2">
                {list.into_iter().map(|s| {
                    let label = i18n.t_args("auth-username-use", &[("username", s.clone().into())]);
                    let pick = s.clone();
                    view! {
                        <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=label
                            on:click=move |_| {
                                username.set(pick.clone());
                                avail.answer.set(Some(UsernameAvailability::Available { username: pick.clone() }));
                                avail.asked.set_value(pick.clone());
                                focus(field_id(AuthField::Username));
                            }>
                            {s}
                        </Button>
                    }
                }).collect_view()}
            </div>
        })
    }
}

/// The username field of the sign-up form.
#[component]
fn UsernameField(
    username: RwSignal<String>,
    avail: Availability,
    /// Rule errors show after the first blur (when not empty) or after a submit.
    touched: RwSignal<bool>,
    submitted: RwSignal<bool>,
    server: FieldErrors,
) -> impl IntoView {
    let i18n = use_i18n();
    let (rules_i, status_i, err_i) = (i18n.clone(), i18n.clone(), i18n.clone());
    let rules = Signal::derive(move || username_rules(&rules_i, &username.get()));
    let status = Signal::derive(move || {
        let raw = username.get();
        let normalized = normalize_username(&raw);
        let mut parts: Vec<String> = username_saved_as(&status_i, &raw).into_iter().collect();
        if avail.throttled.get() {
            parts.push(check_throttled_message(&status_i));
        }
        if avail.available(&normalized) {
            parts.push(status_i.t_args(
                "auth-username-available",
                &[("username", normalized.into())],
            ));
        }
        (!parts.is_empty()).then(|| parts.join(". "))
    });
    let error = Signal::derive(move || {
        let raw = username.get();
        let taken = avail.taken(&normalize_username(&raw));
        let show = taken.is_some() || submitted.get() || (touched.get() && !raw.trim().is_empty());
        let local = if show {
            username_error(&err_i, &raw, taken.as_deref())
        } else {
            None
        };
        local.or_else(|| error_of(server, AuthField::Username))
    });
    Effect::new(move |_| {
        username.track();
        avail.ask_later(username);
    });
    view! {
        <FormField id="username" label=i18n.t("auth-username") description=username_hint(&i18n)
            rules=rules rule_met_label=i18n.t("auth-rule-met") rule_unmet_label=i18n.t("auth-rule-unmet")
            status=status error=error>
            <div on:focusout=move |_| {
                touched.set(true);
                avail.ask(normalize_username(&username.get_untracked()));
            }>
                <Input id="username" value=username autocomplete="username"
                    described_by=field_described_by("username")
                    invalid=Signal::derive(move || error.with(Option::is_some))/>
            </div>
        </FormField>
        <Suggestions username=username avail=avail/>
    }
}

#[derive(Clone, Copy)]
struct SignUpForm {
    username: RwSignal<String>,
    display: RwSignal<String>,
    email: RwSignal<String>,
    password: RwSignal<String>,
    /// After the first submit every field shows its error live.
    submitted: RwSignal<bool>,
    touched: RwSignal<bool>,
    server: FieldErrors,
    summary: RwSignal<Vec<ErrorLink>>,
    form_error: RwSignal<Option<String>>,
}

impl SignUpForm {
    fn new() -> Self {
        let f = Self {
            username: RwSignal::new(String::new()),
            display: RwSignal::new(String::new()),
            email: RwSignal::new(String::new()),
            password: RwSignal::new(String::new()),
            submitted: RwSignal::new(false),
            touched: RwSignal::new(false),
            server: RwSignal::new(Vec::new()),
            summary: RwSignal::new(Vec::new()),
            form_error: RwSignal::new(None),
        };
        clear_on_change(f.server, AuthField::Username, f.username);
        clear_on_change(f.server, AuthField::DisplayName, f.display);
        clear_on_change(f.server, AuthField::Password, f.password);
        f
    }

    fn values(self) -> SignUpValues {
        SignUpValues {
            username: self.username.get_untracked(),
            display_name: self.display.get_untracked(),
            password: self.password.get_untracked(),
        }
    }

    /// Show the server's answer at its field (and in the summary) or in the form alert.
    fn server_error(self, i18n: &I18n, code: &str, v: &SignUpValues, avail: Availability) {
        match placement(code) {
            Placement::Field(field) => {
                if field == AuthField::Username && code == "username_taken" {
                    avail.mark_taken(normalize_username(&v.username));
                    self.touched.set(true);
                }
                let errs = vec![(field, server_field_message(i18n, code, v))];
                self.summary.set(summary_links(&errs));
                self.server.set(errs);
            }
            Placement::Form | Placement::TotpStep => {
                self.form_error.set(Some(form_message(i18n, code)));
            }
        }
    }
}

#[component]
pub fn SignUp() -> impl IntoView {
    let i18n = use_i18n();
    // `/invite/:token`: promotion of a companion (D-9); the account is linked on creation
    let params = use_params_map();
    let invite = params.with_untracked(|p| p.get("token"));
    view! {
        <AuthFrame>
            <PageTitle text=i18n.t("auth-sign-up-title")/>
            <SignUpFields invite=invite/>
        </AuthFrame>
    }
}

/// Create the account: the first one through the setup wizard (`setup`, then run it), else
/// a sign-up (with the invite, if any) that opens the library. Server refusals go back to
/// the fields.
fn sign_up_action(
    f: SignUpForm,
    avail: Availability,
    invite: Option<String>,
    setup: Option<Callback<()>>,
    i18n: Arc<I18n>,
) -> Action<SignUpValues, ()> {
    Action::new(move |v: &SignUpValues| {
        let (v, inv, i18n) = (v.clone(), invite.clone(), i18n.clone());
        let email = f.email.get_untracked();
        async move {
            let display = v.display_name.clone();
            let (user, pass) = (v.username.clone(), v.password.clone());
            let result = match setup {
                Some(_) => crate::admin_api::setup_account(user, display, pass, Some(email)).await,
                None => api::sign_up(user, display, pass, Some(email), inv).await,
            };
            match (result, setup) {
                (Ok(_), Some(done)) => done.run(()),
                (Ok(_), None) => go("/library"),
                (Err(e), _) => f.server_error(&i18n, &api::error_code(&e), &v, avail),
            }
        }
    })
}

/// The sign-up form. With `setup` it creates the first account through the setup wizard
/// (design.md 5.1: the same fields and inline validation as 5.2) and runs `setup` after;
/// otherwise it signs up (with the invite, if any) and opens the library.
#[component]
pub(crate) fn SignUpFields(
    #[prop(optional_no_strip)] invite: Option<String>,
    #[prop(optional)] setup: Option<Callback<()>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let invited = invite.is_some();
    let f = SignUpForm::new();
    let avail = Availability::new(invite.clone(), f.form_error, i18n.clone());
    let submit = sign_up_action(f, avail, invite, setup, i18n.clone());
    let on_submit = {
        let i18n = i18n.clone();
        move |ev: SubmitEvent| {
            ev.prevent_default();
            f.submitted.set(true);
            f.touched.set(true);
            f.form_error.set(None);
            let v = f.values();
            let taken = avail.taken(&normalize_username(&v.username));
            let errs = sign_up_errors(&i18n, &v, taken.as_deref());
            f.summary.set(summary_links(&errs));
            if errs.is_empty() {
                submit.dispatch(v);
            }
        }
    };
    let (di, pi) = (i18n.clone(), i18n.clone());
    let display_error = Signal::derive(move || {
        f.submitted
            .get()
            .then(|| display_name_error(&di, &f.display.get(), &f.username.get()))
            .flatten()
            .or_else(|| error_of(f.server, AuthField::DisplayName))
    });
    let password_err = Signal::derive(move || {
        f.submitted
            .get()
            .then(|| password_error(&pi, &f.password.get()))
            .flatten()
            .or_else(|| error_of(f.server, AuthField::Password))
    });
    let pr = i18n.clone();
    let pw_rules = Signal::derive(move || password_rules(&pr, &f.password.get()));
    let (link, have) = (i18n.t("nav-sign-in"), i18n.t("auth-have-account"));
    let invited_text = i18n.t("auth-invited");
    let wizard = setup.is_some();
    let submit_label = i18n.t(if wizard {
        "setup-account-submit"
    } else {
        "auth-sign-up-submit"
    });
    view! {
            <form class="grid gap-4 rounded-xl border border-border bg-card p-6 shadow-sm" novalidate on:submit=on_submit>
                {invited.then(|| view! { <p class="text-sm text-muted-foreground">{invited_text}</p> })}
                <Summary errors=f.summary/>
                <FormAlert error=f.form_error/>
                <UsernameField username=f.username avail=avail touched=f.touched submitted=f.submitted server=f.server/>
                <FormField id="display_name" label=i18n.t("auth-display-name")
                    description=i18n.t("auth-display-name-hint") error=display_error>
                    <Input id="display_name" value=f.display autocomplete="nickname"
                        described_by=field_described_by("display_name")
                        invalid=Signal::derive(move || display_error.with(Option::is_some))/>
                </FormField>
                <FormField id="email" label=i18n.t("auth-email")>
                    <Input id="email" r#type="email" inputmode="email" value=f.email autocomplete="email"
                        described_by=field_described_by("email")/>
                </FormField>
                <FormField id="password" label=i18n.t("auth-password") description=password_hint(&i18n)
                    rules=pw_rules rule_met_label=i18n.t("auth-rule-met") rule_unmet_label=i18n.t("auth-rule-unmet")
                    error=password_err>
                    <PasswordInput value=f.password autocomplete="new-password"
                        invalid=Signal::derive(move || password_err.with(Option::is_some))/>
                </FormField>
                <SubmitButton pending=submit.pending() label=submit_label/>
                {(!wizard).then(|| view! {
                    <p class="text-sm text-muted-foreground">
                        {have} " " <A href="/sign-in" attr:class="underline">{link}</A>
                    </p>
                })}
            </form>
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SignInForm {
    pub(crate) username: RwSignal<String>,
    pub(crate) password: RwSignal<String>,
    pub(crate) totp: RwSignal<String>,
    pub(crate) need_totp: RwSignal<bool>,
    pub(crate) errors: FieldErrors,
    pub(crate) summary: RwSignal<Vec<ErrorLink>>,
    pub(crate) form_error: RwSignal<Option<String>>,
}

impl SignInForm {
    pub(crate) fn new() -> Self {
        let f = Self {
            username: RwSignal::new(String::new()),
            password: RwSignal::new(String::new()),
            totp: RwSignal::new(String::new()),
            need_totp: RwSignal::new(false),
            errors: RwSignal::new(Vec::new()),
            summary: RwSignal::new(Vec::new()),
            form_error: RwSignal::new(None),
        };
        clear_on_change(f.errors, AuthField::Username, f.username);
        clear_on_change(f.errors, AuthField::Password, f.password);
        clear_on_change(f.errors, AuthField::Totp, f.totp);
        f
    }

    pub(crate) fn show(self, errs: Vec<(AuthField, String)>) {
        self.summary.set(summary_links(&errs));
        self.errors.set(errs);
    }

    pub(crate) fn server_error(self, i18n: &I18n, code: &str, retry_after_secs: Option<u64>) {
        match placement(code) {
            Placement::TotpStep => {
                self.need_totp.set(true);
                // the step's own field takes focus once it is rendered
                request_animation_frame(|| focus(field_id(AuthField::Totp)));
            }
            Placement::Field(field) => {
                self.show(vec![(field, form_message(i18n, code))]);
            }
            Placement::Form => {
                self.form_error
                    .set(Some(form_message_after(i18n, code, retry_after_secs)));
            }
        }
    }
}

/// Step two of sign-in: the one-time code, with its own heading.
#[component]
pub(crate) fn TotpStep(f: SignInForm) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <h2 class="text-lg font-semibold">{i18n.t("auth-totp-title")}</h2>
        <FormField id="totp" label=i18n.t("auth-totp") description=i18n.t("auth-totp-lead")
            error=Signal::derive(move || error_of(f.errors, AuthField::Totp))>
            <Input id="totp" value=f.totp inputmode="numeric" autocomplete="one-time-code"
                described_by=field_described_by("totp")
                invalid=Signal::derive(move || error_of(f.errors, AuthField::Totp).is_some())/>
        </FormField>
        <Button variant=ButtonVariant::Ghost on:click=move |_| {
            f.need_totp.set(false);
            f.totp.set(String::new());
            f.show(Vec::new());
        }>{i18n.t("auth-back")}</Button>
    }
}
