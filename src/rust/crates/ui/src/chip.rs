//! Toggle chips and segmented controls (design.md 7.2).

use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;

use crate::class::cn;
use crate::icon::{Icon, IconName};

/// Toggle button styled as a pill (`aria-pressed`). `count` is shown after the label and is
/// part of the accessible name; `on_change` runs after the state flipped.
#[component]
pub fn Chip(
    pressed: RwSignal<bool>,
    #[prop(optional, into)] icon: Option<IconName>,
    #[prop(into, optional)] count: MaybeProp<String>,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(optional, into)] on_change: Option<Callback<bool>>,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            type="button"
            aria-pressed=move || if pressed.get() { "true" } else { "false" }
            disabled=disabled
            on:click=move |_| {
                let next = !pressed.get_untracked();
                pressed.set(next);
                if let Some(cb) = on_change {
                    cb.run(next);
                }
            }
            class=move || cn(&[
                "inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-sm font-medium \
                 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring \
                 focus-visible:ring-offset-2 ring-offset-background disabled:pointer-events-none \
                 disabled:opacity-50 cursor-pointer",
                if pressed.get() {
                    "border-primary bg-primary text-primary-foreground"
                } else {
                    "border-input bg-background hover:bg-accent"
                },
                &class.get().unwrap_or_default(),
            ])
        >
            {icon.map(|i| view! { <Icon name=i/> })}
            {children()}
            {move || count.get().map(|n| view! {
                <span class=move || if pressed.get() { "font-normal opacity-75" } else { "font-normal text-muted-foreground" }>{n}</span>
            })}
        </button>
    }
}

/// One option of a [`Segmented`] control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedOption {
    pub value: String,
    pub label: String,
    pub icon: Option<IconName>,
    /// Id of the panel this tab controls (`SegmentedMode::Tabs` only).
    pub controls: Option<String>,
    /// Language of the label when it differs from the page (`lang`, WCAG 3.1.2).
    pub lang: Option<String>,
}

impl SegmentedOption {
    #[must_use]
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            icon: None,
            controls: None,
            lang: None,
        }
    }

    /// The label is in another language than the page (a language picker names each
    /// language in itself).
    #[must_use]
    pub fn lang(mut self, lang: impl Into<String>) -> Self {
        self.lang = Some(lang.into());
        self
    }

    #[must_use]
    pub const fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn controls(mut self, panel_id: impl Into<String>) -> Self {
        self.controls = Some(panel_id.into());
        self
    }
}

/// Semantics of a [`Segmented`] control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SegmentedMode {
    /// A choice that changes a setting or view (`radiogroup`/`radio`, `aria-checked`).
    #[default]
    Radio,
    /// Switches between panels (`tablist`/`tab`, `aria-selected`, `aria-controls`).
    Tabs,
}

/// Index of the option the arrow keys move to, or `None` for other keys.
fn step(key: &str, cur: usize, len: usize) -> Option<usize> {
    match key {
        "ArrowRight" | "ArrowDown" => Some((cur + 1) % len),
        "ArrowLeft" | "ArrowUp" => Some((cur + len - 1) % len),
        "Home" => Some(0),
        "End" => Some(len - 1),
        _ => None,
    }
}

pub(crate) fn focus_id(id: &str) {
    if let Some(el) = document().get_element_by_id(id)
        && let Ok(el) = el.dyn_into::<web_sys::HtmlElement>()
    {
        let _ = el.focus();
    }
}

/// Segmented control (group switcher, view toggle). One option is selected; the selected one
/// is the only tab stop and arrow keys move and select (APG radio group / tabs with
/// automatic activation). Option element ids are `{id}-{value}`.
#[component]
pub fn Segmented(
    #[prop(into)] id: String,
    /// Accessible name of the group.
    #[prop(into)]
    label: String,
    value: RwSignal<String>,
    options: Vec<SegmentedOption>,
    #[prop(optional)] mode: SegmentedMode,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let tabs = mode == SegmentedMode::Tabs;
    let values: Vec<String> = options.iter().map(|o| o.value.clone()).collect();
    let base = StoredValue::new(id);
    let known = StoredValue::new(values.clone());
    // the selected option is the tab stop; the first one when nothing (known) is selected
    let tab_stop = move |i: usize, v: &str| {
        let cur = value.get();
        cur == v || (i == 0 && known.with_value(|k| !k.contains(&cur)))
    };
    let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
        let cur = values
            .iter()
            .position(|v| *v == value.get_untracked())
            .unwrap_or(0);
        let Some(next) = step(&ev.key(), cur, values.len()) else {
            return;
        };
        ev.prevent_default();
        value.set(values[next].clone());
        focus_id(&format!("{}-{}", base.get_value(), values[next]));
    };
    view! {
        <div
            role=if tabs { "tablist" } else { "radiogroup" }
            aria-label=label
            on:keydown=on_keydown
            class=move || cn(&["inline-flex gap-1 rounded-lg bg-muted p-1", &class.get().unwrap_or_default()])
        >
            {options.into_iter().enumerate().map(|(i, o)| {
                let v = o.value.clone();
                let v_tab = o.value.clone();
                let v_click = o.value.clone();
                let selected = Memo::new(move |_| value.get() == v);
                let state = move || if selected.get() { "true" } else { "false" };
                view! {
                    <button
                        type="button"
                        id=format!("{}-{}", base.get_value(), o.value)
                        role=if tabs { "tab" } else { "radio" }
                        aria-checked=move || (!tabs).then(state)
                        aria-selected=move || tabs.then(state)
                        aria-controls=o.controls.clone()
                        lang=o.lang.clone()
                        tabindex=move || if tab_stop(i, &v_tab) { "0" } else { "-1" }
                        on:click=move |_| value.set(v_click.clone())
                        class=move || cn(&[
                            "inline-flex items-center gap-1.5 rounded-md px-3 py-1 text-sm font-medium \
                             transition-colors focus-visible:outline-none focus-visible:ring-2 \
                             focus-visible:ring-ring cursor-pointer",
                            if selected.get() {
                                "bg-background text-foreground shadow-sm"
                            } else {
                                "text-muted-foreground hover:text-foreground"
                            },
                        ])
                    >
                        {o.icon.map(|i| view! { <Icon name=i/> })}
                        {o.label}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn arrow_keys_wrap() {
        assert_eq!(super::step("ArrowRight", 2, 3), Some(0));
        assert_eq!(super::step("ArrowLeft", 0, 3), Some(2));
        assert_eq!(super::step("End", 0, 3), Some(2));
        assert_eq!(super::step("a", 0, 3), None);
    }
}
