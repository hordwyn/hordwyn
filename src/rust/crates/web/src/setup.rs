//! The setup wizard (`/setup`, design.md 4.3, 5.1; addendum-1 5.1, D12): admin account ->
//! hosting mode -> data sources -> done. The server keeps the wizard's state
//! (`SetupStateDto`), so leaving it and coming back resumes at the same step; after
//! completion `/setup` is the 404 page (the server sets the status, the page shows
//! `NotFound`). Before the first account exists every other page redirects here
//! (`backend::resolve_shell_session`).

use contracts::admin::{AdminSourceDto, SetupSourceDto, SetupStateDto};
use leptos::prelude::*;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonVariant, Disclosure, DisclosureMode, Steps,
    TaskItem, TaskList,
};

use crate::admin::{SourceCredential, SourceKeyHowTo, SourceLicence, SourcesResource};
use crate::admin_api;
use crate::app::ClientConfig;
use crate::i18n::{kind_label, t, use_i18n};
use crate::pages::{PageTitle, go};
use crate::settings::{admin_error, kept, loaded};
use crate::setup_state::{
    HOSTING, checklist_href, landing, open_sources, shown_step, step_index, step_label_key,
};

/// The wizard's shared state: the server's answer and the step the admin went back to.
#[derive(Clone, Copy)]
struct Wizard {
    state: RwSignal<SetupStateDto>,
    back_to: RwSignal<Option<String>>,
}

impl Wizard {
    /// The server moved on: show its step, focus the new heading.
    fn advance(self, next: SetupStateDto) {
        self.back_to.set(None);
        self.state.set(next);
        focus_step();
    }

    fn back(self, to: &str) {
        self.back_to.set(Some(to.to_string()));
        focus_step();
    }
}

/// Move focus to the step heading after a step change (browser only), so screen readers
/// start reading the new step.
fn focus_step() {
    #[cfg(feature = "hydrate")]
    request_animation_frame(|| {
        use wasm_bindgen::JsCast;
        if let Some(el) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("wizard-step"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = el.focus();
        }
    });
}

/// 404 for a completed wizard on the server's first answer (the page body follows the
/// `setup_state` answer on both sides, so hydration agrees).
fn not_found_status() {
    #[cfg(feature = "ssr")]
    {
        let completed = use_context::<axum::http::request::Parts>()
            .and_then(|p| p.extensions.get::<crate::backend::SetupGate>().copied())
            .is_some_and(|g| g.completed);
        if completed && let Some(r) = use_context::<leptos_axum::ResponseOptions>() {
            r.set_status(axum::http::StatusCode::NOT_FOUND);
        }
    }
}

#[component]
pub fn SetupPage() -> impl IntoView {
    not_found_status();
    let data = Resource::new(|| (), |()| admin_api::setup_state());
    view! {
        <Suspense fallback=|| ()>
            {move || data.get().map(|r| match r {
                Ok(state) => view! { <WizardView state=state/> }.into_any(),
                Err(e) if crate::api::error_code(&e) == "setup_done" => view! { <crate::shell::NotFound/> }.into_any(),
                Err(e) if matches!(crate::api::error_code(&e).as_str(), "unauthorized" | "forbidden") => view! { <SignInFirst/> }.into_any(),
                Err(e) => view! {
                    <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{admin_error(&e)}</Alert>
                }.into_any(),
            })}
        </Suspense>
    }
}

/// An account exists but the visitor is not its admin: the setup continues after sign-in.
#[component]
fn SignInFirst() -> impl IntoView {
    view! {
        <section class="mx-auto max-w-xl py-16 text-center">
            <PageTitle text=t("setup-title")/>
            <p class="text-muted-foreground">{t("setup-sign-in-lead")}</p>
            <p class="mt-6"><a class="text-primary underline" href="/sign-in">{t("nav-sign-in")}</a></p>
        </section>
    }
}

#[component]
fn WizardView(state: SetupStateDto) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    let w = Wizard {
        state: RwSignal::new(state),
        back_to: RwSignal::new(None),
    };
    let step = Memo::new(move |_| {
        let s = w.state.get();
        let back = w.back_to.get();
        shown_step(&s, back.as_deref()).to_string()
    });
    let labels = Signal::derive(move || {
        w.state
            .get()
            .steps
            .iter()
            .map(|s| t(step_label_key(s)))
            .collect::<Vec<_>>()
    });
    let current = Signal::derive(move || step_index(&w.state.get().steps, &step.get()));
    let title = i18n.t_args("setup-title-named", &[("product", cfg.product_name.into())]);
    view! {
        <div data-testid="wizard" class="mx-auto grid w-full max-w-2xl gap-6">
            <PageTitle text=title/>
            <Steps label=t("setup-progress") steps=labels current=current done_label=t("setup-step-done-label") class="-mt-4"/>
            {move || match step.get().as_str() {
                "account" => view! { <AccountStep/> }.into_any(),
                "hosting" => view! { <HostingStep w=w/> }.into_any(),
                "sources" => view! { <SourcesStep w=w/> }.into_any(),
                _ => view! { <DoneStep w=w/> }.into_any(),
            }}
        </div>
    }
}

/// The heading every step starts with (focus target after a step change).
#[component]
fn StepHeading(#[prop(into)] text: String, #[prop(into)] lead: String) -> impl IntoView {
    view! {
        <div class="grid gap-1">
            <h2 id="wizard-step" tabindex="-1" class="text-h2 font-semibold focus:outline-none">{text}</h2>
            <p class="text-sm text-muted-foreground">{lead}</p>
        </div>
    }
}

/// Step 1: the sign-up form; the account becomes the admin and is signed in. A full page
/// load then renders the signed-in shell, and the wizard resumes at step 2.
#[component]
fn AccountStep() -> impl IntoView {
    view! {
        <section aria-labelledby="wizard-step" class="grid gap-4">
            <StepHeading text=t("setup-account-title") lead=t("setup-account-lead")/>
            <crate::auth_pages::SignUpFields setup=Callback::new(|()| go("/setup"))/>
        </section>
    }
}

/// Step 2: two selectable cards, Home preselected (owner ruling) or the stored choice (D12).
#[component]
fn HostingStep(w: Wizard) -> impl IntoView {
    let s = w.state.get_untracked();
    let choice = RwSignal::new(if HOSTING.contains(&s.hosting.as_str()) {
        s.hosting.clone()
    } else {
        HOSTING[0].to_string()
    });
    let fixed = s.hosting_fixed;
    let error = RwSignal::new(None::<String>);
    let save = Action::new(move |h: &String| {
        let h = h.clone();
        async move {
            match admin_api::setup_hosting(h).await {
                Ok(next) => w.advance(next),
                Err(e) => error.set(Some(admin_error(&e))),
            }
        }
    });
    view! {
        <section aria-labelledby="wizard-step" class="grid gap-4">
            <StepHeading text=t("setup-hosting-title") lead=t("setup-hosting-lead")/>
            <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); save.dispatch(choice.get_untracked()); }>
                <div role="radiogroup" aria-labelledby="wizard-step" class="grid gap-3 min-[40rem]:grid-cols-2">
                    {HOSTING.iter().map(|h| view! { <HostingCard id=h choice=choice disabled=fixed/> }).collect_view()}
                </div>
                {fixed.then(|| view! { <p class="text-sm text-muted-foreground">{t("setup-hosting-fixed")}</p> })}
                {move || error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
                <div class="flex flex-wrap gap-2">
                    <Button r#type="submit" loading=save.pending()>{t("setup-continue")}</Button>
                </div>
            </form>
        </section>
    }
}

#[component]
fn HostingCard(id: &'static str, choice: RwSignal<String>, disabled: bool) -> impl IntoView {
    let input_id = format!("wizard-hosting-{id}");
    let desc_id = format!("{input_id}-description");
    let (for_id, described) = (input_id.clone(), desc_id.clone());
    let (title, body) = if id == "hosted" {
        (t("setup-hosting-hosted"), t("setup-hosting-hosted-body"))
    } else {
        (t("setup-hosting-home"), t("setup-hosting-home-body"))
    };
    view! {
        <label for=for_id class="flex cursor-pointer gap-3 rounded-xl border bg-card p-4 has-[:checked]:border-primary has-[:checked]:ring-2 has-[:checked]:ring-ring has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring">
            <input id=input_id type="radio" name="hosting" value=id class="mt-1 size-5 shrink-0 accent-[var(--primary)]"
                aria-describedby=described disabled=disabled
                prop:checked=move || choice.get() == id
                on:change=move |_| choice.set(id.to_string())/>
            <span class="grid gap-1">
                <span class="font-semibold">{title}</span>
                <span id=desc_id class="text-sm text-muted-foreground">{body}</span>
            </span>
        </label>
    }
}

/// Step 3: one card per compiled-in source; keys are optional and Continue never blocks.
#[component]
fn SourcesStep(w: Wizard) -> impl IntoView {
    let s = w.state.get_untracked();
    let hosted = s.hosting == "hosted";
    let error = RwSignal::new(None::<String>);
    let next = Action::new(move |(): &()| async move {
        match admin_api::setup_sources().await {
            Ok(next) => w.advance(next),
            Err(e) => error.set(Some(admin_error(&e))),
        }
    });
    let body = if s.sources_read_only {
        view! { <ReadOnlySources sources=s.sources.clone()/> }.into_any()
    } else {
        view! { <SourceCards open=open_sources(&s.sources) hosted=hosted/> }.into_any()
    };
    view! {
        <section aria-labelledby="wizard-step" class="grid gap-4">
            <StepHeading text=t("setup-sources-title") lead=t(if s.sources_read_only { "setup-sources-read-only" } else { "setup-sources-lead" })/>
            {hosted.then(|| view! {
                <Alert variant=AlertVariant::Info role=AlertRole::None>{t("setup-sources-hosted")}</Alert>
            })}
            {body}
            {move || error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
            <div class="flex flex-wrap gap-2">
                <Button loading=next.pending() on:click=move |_| { next.dispatch(()); }>{t("setup-continue")}</Button>
                <Button variant=ButtonVariant::Ghost on:click=move |_| w.back("hosting")>{t("setup-back")}</Button>
            </div>
        </section>
    }
}

fn kinds_line(kinds: &[String]) -> String {
    kinds
        .iter()
        .map(|k| kind_label(k))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Headless installs: every key comes from the environment, step 3 only summarises.
#[component]
fn ReadOnlySources(sources: Vec<SetupSourceDto>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <ul class="grid gap-2">
            {sources.into_iter().map(|s| {
                let line = match s.env_var.as_deref() {
                    Some(var) if s.credential == "env" => i18n.t_args("admin-setting-env", &[("var", var.to_string().into())]),
                    _ => i18n.t("admin-sources-no-key"),
                };
                view! {
                    <li class="grid gap-0.5 rounded-xl border bg-card p-4">
                        <span class="font-semibold">{s.name.clone()}</span>
                        <span class="text-sm text-muted-foreground">{kinds_line(&s.kinds)}</span>
                        <span class="text-sm">{line}</span>
                    </li>
                }
            }).collect_view()}
        </ul>
    }
}

/// The cards with key fields and Test (the admin page's own widgets); the first two that
/// need a key are open, the rest one disclosure row each (addendum-1 5.1).
#[component]
fn SourceCards(open: Vec<String>, hosted: bool) -> impl IntoView {
    let data: SourcesResource = Resource::new(|| (), |()| admin_api::admin_sources());
    view! {
        {loaded(data, move |list: Vec<AdminSourceDto>| {
            let open = open.clone();
            view! {
                <div class="grid gap-3">
                    {list.into_iter().map(|s| {
                        // kept: saving a key refetches the list, the card stays as it was
                        let starts_open = open.contains(&s.status.id);
                        let is_open = kept(&format!("wizard-open-{}", s.status.id), move || starts_open);
                        let state = t(&format!("source-state-{}", s.status.state));
                        let dot = crate::admin_state::source_state(&s.status.state);
                        let summary = kinds_line(&s.status.kinds);
                        view! {
                            <Disclosure id=format!("wizard-src-{}", s.status.id) title=s.status.name.clone() summary=summary
                                mode=DisclosureMode::Button level=3 open=is_open
                                status=move || view! { <ui::StatusDot state=dot label=state.clone()/> }>
                                <div class="grid gap-4">
                                    <SourceKeyHowTo s=s.clone()/>
                                    <SourceCredential s=s.clone() data=data/>
                                    {hosted.then(|| view! { <SourceLicence s=s.clone() data=data/> })}
                                    <p class="text-sm"><a class="text-primary underline" href=s.terms_url.clone() rel="noopener">{t("admin-sources-terms")}</a></p>
                                </div>
                            </Disclosure>
                        }
                    }).collect_view()}
                </div>
            }
        })}
    }
}

/// Step 4: the checklist, then lock the wizard and go on (Home, or Users for hosted).
#[component]
fn DoneStep(w: Wizard) -> impl IntoView {
    let i18n = use_i18n();
    // "Add your first item" opens the search palette (design.md 5.1 step 4, UX2-W11); the
    // link to the library stays for the time before hydration
    let palette = expect_context::<crate::palette::PaletteOpen>().0;
    let open_palette = Callback::new(move |id: String| {
        let first = id == "first_item";
        if first {
            palette.set(true);
        }
        first
    });
    let s = w.state.get_untracked();
    let items: Vec<TaskItem> = s
        .checklist
        .iter()
        .map(|id| {
            // the rows the server knows are done turn green (DEC-292, DEC-305)
            let item = TaskItem::new(id.clone(), t(&format!("setup-checklist-{id}")))
                .href(checklist_href(id))
                .done(s.done.contains(id));
            match (&s.key_file, id.as_str()) {
                (Some(file), "backup_key") => {
                    item.hint(i18n.t_args("setup-key-file", &[("file", file.clone().into())]))
                }
                _ => item,
            }
        })
        .collect();
    let to = landing(&s.hosting);
    let error = RwSignal::new(None::<String>);
    let finish = Action::new(move |(): &()| async move {
        match admin_api::setup_complete().await {
            Ok(()) => go(to),
            Err(e) => error.set(Some(admin_error(&e))),
        }
    });
    let label = t(if to == "/" {
        "setup-go-home"
    } else {
        "setup-go-users"
    });
    view! {
        <section aria-labelledby="wizard-step" class="grid gap-4">
            <StepHeading text=t("setup-done-title") lead=t("setup-done-lead")/>
            <TaskList label=t("setup-checklist") items=items done_label=t("setup-task-done") todo_label=t("setup-task-todo")
                on_open=open_palette/>
            {move || error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
            <div class="flex flex-wrap gap-2">
                <Button loading=finish.pending() on:click=move |_| { finish.dispatch(()); }>{label}</Button>
                <Button variant=ButtonVariant::Ghost on:click=move |_| w.back("sources")>{t("setup-back")}</Button>
            </div>
        </section>
    }
}
