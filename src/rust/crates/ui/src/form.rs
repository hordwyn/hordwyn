//! Form controls. Inputs are controlled through signals; ids tie labels, descriptions and
//! errors together for assistive technology.

use leptos::prelude::*;

use crate::class::cn;

const FIELD: &str = "flex w-full rounded-md border border-input bg-background px-3 py-1 text-sm \
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
#[component]
pub fn Input(
    #[prop(into)] id: String,
    value: RwSignal<String>,
    #[prop(into, optional)] r#type: Option<&'static str>,
    #[prop(into, optional)] placeholder: MaybeProp<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(into, optional)] invalid: Signal<bool>,
    #[prop(into, optional)] described_by: MaybeProp<String>,
) -> impl IntoView {
    view! {
        <input
            id=id
            type=r#type.unwrap_or("text")
            placeholder=move || placeholder.get()
            disabled=disabled
            aria-invalid=move || invalid.get().then_some("true")
            aria-describedby=move || described_by.get()
            prop:value=move || value.get()
            on:input=move |ev| value.set(event_target_value(&ev))
            class=move || cn(&[FIELD, "h-9", &class.get().unwrap_or_default()])
        />
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

/// Label + control + optional description and error, wired with ids. The control must use
/// `id`, and `"{id}-description {id}-error"` as `described_by`.
#[component]
pub fn FormField(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into, optional)] description: MaybeProp<String>,
    #[prop(into, optional)] error: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let desc_id = format!("{id}-description");
    let err_id = format!("{id}-error");
    view! {
        <div class="grid gap-2">
            <Label r#for=id.clone()>{label}</Label>
            {children()}
            {move || description.get().map(|d| view! { <p id=desc_id.clone() class="text-sm text-muted-foreground">{d}</p> })}
            {move || error.get().map(|e| view! { <p id=err_id.clone() role="alert" class="text-sm font-medium text-destructive">{e}</p> })}
        </div>
    }
}
