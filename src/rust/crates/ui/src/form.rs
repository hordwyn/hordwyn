//! Form controls. Inputs are controlled through signals; ids tie labels, descriptions and
//! errors together for assistive technology.

use leptos::prelude::*;

use crate::class::cn;
use crate::icon::{Icon, IconName};

pub(crate) const FIELD: &str = "flex w-full rounded-md border border-input bg-background px-3 py-1 text-sm \
    shadow-sm transition-colors placeholder:text-muted-foreground focus-visible:outline-none \
    focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50";

#[component]
pub fn Label(
    #[prop(into)] r#for: String,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <label
            for=r#for
            class=move || cn(&["text-sm font-medium leading-none", &class.get().unwrap_or_default()])
        >
            {children()}
        </label>
    }
}

/// Text input bound to `value`.
///
/// `trailing` renders controls (Show, Copy) right after the field in the same row, so they
/// follow it in the tab order. `inputmode` and `autocomplete` are passed through unchanged
/// (WCAG 1.3.5: use the HTML autofill tokens for personal data).
#[component]
pub fn Input(
    #[prop(into)] id: String,
    value: RwSignal<String>,
    /// HTML input type (default `text`); reactive, so a Show button can switch `password` to `text`.
    #[prop(into, optional)]
    r#type: MaybeProp<String>,
    #[prop(into, optional)] placeholder: MaybeProp<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(into, optional)] invalid: Signal<bool>,
    #[prop(into, optional)] described_by: MaybeProp<String>,
    /// `numeric`, `email`, `tel`, ... (HTML `inputmode`).
    #[prop(into, optional)]
    inputmode: Option<&'static str>,
    /// HTML autofill token, e.g. `username`, `new-password`, `one-time-code`, `off`.
    #[prop(into, optional)]
    autocomplete: MaybeProp<String>,
    #[prop(into, optional)] readonly: Signal<bool>,
    #[prop(optional, into)] trailing: Option<ViewFn>,
) -> impl IntoView {
    let input = view! {
        <input
            id=id
            type=move || r#type.get().unwrap_or_else(|| "text".to_string())
            placeholder=move || placeholder.get()
            disabled=disabled
            readonly=readonly
            inputmode=inputmode
            autocomplete=move || autocomplete.get()
            aria-invalid=move || invalid.get().then_some("true")
            aria-describedby=move || described_by.get()
            prop:value=move || value.get()
            on:input=move |ev| value.set(event_target_value(&ev))
            class=move || cn(&[
                FIELD,
                "h-9 aria-[invalid=true]:border-destructive",
                if readonly.get() { "bg-muted" } else { "" },
                &class.get().unwrap_or_default(),
            ])
        />
    };
    match trailing {
        Some(t) => view! { <div class="flex items-center gap-2">{input}{t.run()}</div> }.into_any(),
        None => input.into_any(),
    }
}

#[component]
pub fn Textarea(
    #[prop(into)] id: String,
    value: RwSignal<String>,
    #[prop(into, optional)] placeholder: MaybeProp<String>,
    #[prop(into, optional)] rows: Option<u32>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] described_by: MaybeProp<String>,
) -> impl IntoView {
    view! {
        <textarea
            id=id
            rows=rows.unwrap_or(4)
            placeholder=move || placeholder.get()
            aria-describedby=move || described_by.get()
            prop:value=move || value.get()
            on:input=move |ev| value.set(event_target_value(&ev))
            class=move || cn(&[FIELD, "min-h-16 py-2", &class.get().unwrap_or_default()])
        ></textarea>
    }
}

/// Native checkbox (keyboard and screen-reader behaviour for free).
#[component]
pub fn Checkbox(
    #[prop(into)] id: String,
    checked: RwSignal<bool>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    view! {
        <input
            id=id
            type="checkbox"
            prop:checked=move || checked.get()
            on:change=move |ev| checked.set(event_target_checked(&ev))
            class=move || cn(&["size-4 rounded border-input accent-primary", &class.get().unwrap_or_default()])
        />
    }
}

/// Toggle switch (`role="switch"`), labelled by an external `<Label for=id>`.
#[component]
pub fn Switch(
    #[prop(into)] id: String,
    checked: RwSignal<bool>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    view! {
        <button
            id=id
            type="button"
            role="switch"
            aria-checked=move || if checked.get() { "true" } else { "false" }
            on:click=move |_| checked.update(|c| *c = !*c)
            class=move || cn(&[
                "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 \
                 border-transparent transition-colors focus-visible:outline-none focus-visible:ring-2 \
                 focus-visible:ring-ring",
                if checked.get() { "bg-primary" } else { "bg-input" },
                &class.get().unwrap_or_default(),
            ])
        >
            <span class=move || cn(&[
                "pointer-events-none block size-4 rounded-full bg-background shadow-lg transition-transform",
                if checked.get() { "translate-x-4" } else { "translate-x-0" },
            ])></span>
        </button>
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

/// Native `<select>` styled like the other fields.
#[component]
pub fn Select(
    #[prop(into)] id: String,
    value: RwSignal<String>,
    #[prop(into)] options: Signal<Vec<SelectOption>>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    view! {
        <select
            id=id
            prop:value=move || value.get()
            on:change=move |ev| value.set(event_target_value(&ev))
            class=move || cn(&[FIELD, "h-9", &class.get().unwrap_or_default()])
        >
            <For each=move || options.get() key=|o| o.value.clone() let:o>
                <option value=o.value.clone() selected=move || value.get() == o.value>{o.label.clone()}</option>
            </For>
        </select>
    }
}

/// State of one rule in a field's live checklist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleState {
    /// Not evaluated yet (empty field): neutral.
    #[default]
    Pending,
    Pass,
    Fail,
}

impl RuleState {
    const fn name(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

/// The `aria-describedby` value for a control inside `FormField id=id`: hint, rules, status
/// line and error, in reading order. Ids that are not rendered are ignored by browsers.
#[must_use]
pub fn field_described_by(id: &str) -> String {
    format!("{id}-description {id}-rules {id}-status {id}-error")
}

/// Label + control + optional description, live rule checklist, ok line and error, wired
/// with ids. The control must use `id`, and [`field_described_by`] as `described_by`.
///
/// `rules` is the live checklist (design.md 5.2): each rule shows an icon whose accessible
/// name is `rule_met_label` / `rule_unmet_label`, so the state is never colour only.
/// `status` is the ok line ("Available"), announced politely; `error` is announced as an
/// alert.
#[component]
pub fn FormField(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into, optional)] description: MaybeProp<String>,
    #[prop(into, optional)] error: MaybeProp<String>,
    #[prop(optional, into)] rules: Option<Signal<Vec<(String, RuleState)>>>,
    #[prop(into, optional)] rule_met_label: MaybeProp<String>,
    #[prop(into, optional)] rule_unmet_label: MaybeProp<String>,
    #[prop(into, optional)] status: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let desc_id = format!("{id}-description");
    let err_id = format!("{id}-error");
    let rules_id = format!("{id}-rules");
    let status_id = format!("{id}-status");
    let rules_view = rules.map(|rules| {
        view! {
            <ul id=rules_id class="grid gap-0.5 text-small">
                {move || rules.get().into_iter().map(|(text, state)| {
                    let (icon, tone, name) = match state {
                        RuleState::Pass => (IconName::Check, "text-success", rule_met_label.get()),
                        RuleState::Fail => (IconName::Close, "text-destructive-text", rule_unmet_label.get()),
                        RuleState::Pending => (IconName::Circle, "text-muted-foreground", None),
                    };
                    view! {
                        <li data-state=state.name() class=cn(&["flex items-center gap-1.5", tone])>
                            {match name {
                                Some(n) => view! { <Icon name=icon class="size-3.5" label=n/> }.into_any(),
                                None => view! { <Icon name=icon class="size-3.5"/> }.into_any(),
                            }}
                            <span>{text}</span>
                        </li>
                    }
                }).collect_view()}
            </ul>
        }
    });
    view! {
        <div class="grid gap-2">
            <Label r#for=id.clone()>{label}</Label>
            {children()}
            {move || description.get().map(|d| view! { <p id=desc_id.clone() class="text-sm text-muted-foreground">{d}</p> })}
            {rules_view}
            <p id=status_id role="status" class="empty:hidden flex items-center gap-1.5 text-sm font-medium text-success">
                {move || status.get().map(|s| view! { <Icon name=IconName::Check/><span>{s}</span> })}
            </p>
            {move || error.get().map(|e| view! {
                <p id=err_id.clone() role="alert" class="flex items-start gap-1.5 text-sm font-medium text-destructive-text">
                    <Icon name=IconName::CircleAlert class="mt-0.5"/>
                    <span>{e}</span>
                </p>
            })}
        </div>
    }
}
