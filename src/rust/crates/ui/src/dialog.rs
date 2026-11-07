//! Modal dialog and side sheet after the APG dialog pattern (design.md 8): `role="dialog"`,
//! `aria-modal`, labelled by its title; focus moves into the panel when it opens, Tab and
//! Shift+Tab stay inside it, Escape and backdrop click close it, and focus goes back to the
//! control that opened it (or to `return_to` when that control is gone, for example a menu
//! item that unmounted as the dialog opened).

use leptos::html;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;

use crate::class::{cn, slug};
use crate::icon::{Icon, IconName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetSide {
    Left,
    #[default]
    Right,
    /// A bottom sheet on phones (below `md`), the right-hand side panel from `md` on
    /// (design.md 7.3: log sheets).
    Adaptive,
}

/// Controls Tab can reach inside the panel.
const FOCUSABLE: &str = "a[href], button:not([disabled]), input:not([disabled]), \
    select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])";

fn focusables(panel: &web_sys::HtmlElement) -> Vec<web_sys::HtmlElement> {
    let Ok(list) = panel.query_selector_all(FOCUSABLE) else {
        return Vec::new();
    };
    (0..list.length())
        .filter_map(|i| list.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
        .collect()
}

fn active_element() -> Option<web_sys::HtmlElement> {
    document()
        .active_element()
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
}

/// Keep Tab inside the panel: wrap from the last control to the first and back.
fn trap_tab(panel: &web_sys::HtmlElement, ev: &leptos::ev::KeyboardEvent) {
    let items = focusables(panel);
    let (Some(first), Some(last)) = (items.first(), items.last()) else {
        ev.prevent_default();
        return;
    };
    let active = active_element();
    let at = |el: &web_sys::HtmlElement| active.as_ref().is_some_and(|a| a.is_same_node(Some(el)));
    let target = if ev.shift_key() {
        (at(first) || at(panel)).then_some(last)
    } else {
        at(last).then_some(first)
    };
    if let Some(t) = target {
        ev.prevent_default();
        let _ = t.focus();
    }
}

/// Focus the opener if it is still in the page, else the element with id `fallback`.
fn restore_focus(opener: Option<web_sys::HtmlElement>, fallback: Option<&str>) {
    let body = document().body();
    let opener = opener
        .filter(|o| o.is_connected() && !body.as_ref().is_some_and(|b| b.is_same_node(Some(o))));
    let target = opener.or_else(|| {
        fallback
            .and_then(|id| document().get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
    });
    if let Some(t) = target {
        let _ = t.focus();
    }
}

#[component]
fn Panel(
    open: RwSignal<bool>,
    title: String,
    description: Option<String>,
    close_label: String,
    panel_class: &'static str,
    class: MaybeProp<String>,
    return_to: Option<String>,
    can_close: Option<Callback<(), bool>>,
    children: ChildrenFn,
) -> impl IntoView {
    // every way out (Escape, backdrop, close button) asks `can_close` first
    let close = move || {
        if can_close.is_none_or(|c| c.run(())) {
            open.set(false);
        }
    };
    let base = slug(&title);
    let title_id = format!("{base}-title");
    let desc_id = description.as_ref().map(|_| format!("{base}-description"));
    let panel = NodeRef::<html::Div>::new();
    // The element focused when the dialog opened lives in the effect's own state: effects
    // run only in the browser, so no DOM handle is ever stored on the (multi-threaded) server.
    Effect::new(move |prev: Option<(bool, Option<web_sys::HtmlElement>)>| {
        let (was_open, mut opener) = prev.unwrap_or_default();
        let is_open = open.get();
        if is_open {
            if !was_open {
                opener = active_element();
            }
            // tracks the node ref too: the panel mounts after `open` turns true
            if let Some(el) = panel.get() {
                let _ = el.focus();
            }
        } else if was_open {
            restore_focus(opener.take(), return_to.as_deref());
        }
        (is_open, opener)
    });
    let children = StoredValue::new(children);
    view! {
        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 bg-black/60" aria-hidden="true" on:click=move |_| close()></div>
            <div
                node_ref=panel
                role="dialog"
                aria-modal="true"
                aria-labelledby=title_id.clone()
                aria-describedby=desc_id.clone()
                tabindex="-1"
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    match ev.key().as_str() {
                        "Escape" => {
                            ev.stop_propagation();
                            close();
                        }
                        "Tab" => {
                            if let Some(p) = panel.get_untracked() {
                                trap_tab(&p, &ev);
                            }
                        }
                        _ => {}
                    }
                }
                class=move || cn(&[
                    "fixed z-50 grid gap-4 border bg-background p-6 shadow-lg focus-visible:outline-none",
                    panel_class,
                    &class.get().unwrap_or_default(),
                ])
            >
                <div class="flex flex-col gap-1.5">
                    <h2 id=title_id.clone() class="text-lg font-semibold leading-none tracking-tight">{title.clone()}</h2>
                    {description.clone().map(|d| view! {
                        <p id=desc_id.clone() class="text-sm text-muted-foreground">{d}</p>
                    })}
                </div>
                {children.with_value(|c| c())}
                <button
                    type="button"
                    aria-label=close_label.clone()
                    on:click=move |_| close()
                    class="absolute right-4 top-4 rounded-sm opacity-70 hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                    <Icon name=IconName::Close/>
                </button>
            </div>
        </Show>
    }
}
/// Centered modal dialog. `close_label` is the (translated) label of the close button;
/// `return_to` is the id of the element that gets focus on close when the opener is gone.
/// Centered modal dialog. `close_label` is the (translated) label of the close button.
#[component]
pub fn Dialog(
    open: RwSignal<bool>,
    #[prop(into)] title: String,
    #[prop(into)] close_label: String,
    #[prop(into, optional)] description: Option<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] return_to: Option<String>,
    /// Asked before the panel closes; `false` keeps it open (a sheet with unsaved changes
    /// shows its discard question instead).
    #[prop(optional, into)]
    can_close: Option<Callback<(), bool>>,
    children: ChildrenFn,
) -> impl IntoView {
    view! {
        <Panel
            open=open
            title=title
            description=description
            close_label=close_label
            panel_class="left-1/2 top-1/2 w-full max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-lg"
            class=class
            return_to=return_to
            can_close=can_close
            children=children
        />
    }
}

/// Panel sliding in from a side.
#[component]
pub fn Sheet(
    open: RwSignal<bool>,
    #[prop(into)] title: String,
    #[prop(into)] close_label: String,
    #[prop(optional)] side: SheetSide,
    #[prop(into, optional)] description: Option<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] return_to: Option<String>,
    /// Asked before the panel closes; `false` keeps it open (a sheet with unsaved changes
    /// shows its discard question instead).
    #[prop(optional, into)]
    can_close: Option<Callback<(), bool>>,
    children: ChildrenFn,
) -> impl IntoView {
    let panel_class = match side {
        SheetSide::Left => "inset-y-0 left-0 h-full w-3/4 max-w-sm border-r",
        SheetSide::Right => "inset-y-0 right-0 h-full w-3/4 max-w-sm border-l",
        SheetSide::Adaptive => {
            "inset-x-0 bottom-0 max-h-[85dvh] overflow-y-auto rounded-t-xl border-t \
             md:inset-x-auto md:inset-y-0 md:right-0 md:left-auto md:h-full md:max-h-none md:w-full \
             md:max-w-md md:rounded-none md:border-t-0 md:border-l"
        }
    };
    view! {
        <Panel
            open=open
            title=title
            description=description
            close_label=close_label
            panel_class=panel_class
            class=class
            return_to=return_to
            can_close=can_close
            children=children
        />
    }
}
