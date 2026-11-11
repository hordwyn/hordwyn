//! Modal dialog and side sheet after the APG dialog pattern (design.md 8): `role="dialog"`,
//! `aria-modal`, labelled by its title; focus moves into the panel when it opens, Tab and
//! Shift+Tab stay inside it, Escape and backdrop click close it, and focus goes back to the
//! control that opened it (or to `return_to` when that control is gone, for example a menu
//! item that unmounted as the dialog opened).
//!
//! Three presentations share one panel: [`Dialog`] (centered, small), [`Sheet`] (a side
//! panel) and [`FormDialog`] (forms such as the log dialogs, OT-10: a bottom sheet below
//! 640 px, centered from there, with a body that scrolls between the title and a footer).

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
    /// A bottom sheet on phones (below `md`), the right-hand side panel from `md` on (short
    /// forms next to a list; the log flows use [`FormDialog`], OT-10).
    Adaptive,
}

/// Controls inside the panel; [`focusables`] keeps those in the Tab order.
const FOCUSABLE: &str = "a[href], button:not([disabled]), input:not([disabled]), \
    select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])";

fn focusables(panel: &web_sys::HtmlElement) -> Vec<web_sys::HtmlElement> {
    let Ok(list) = panel.query_selector_all(FOCUSABLE) else {
        return Vec::new();
    };
    (0..list.length())
        .filter_map(|i| list.item(i))
        .filter_map(|n| n.dyn_into::<web_sys::HtmlElement>().ok())
        // `tabindex=-1` (roving groups, UX2-W3) is focusable but not a Tab stop
        .filter(|e| e.tab_index() >= 0)
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

/// Focus the control named `id` inside the panel (the re-auth password, UX2-W18), else
/// the panel itself.
fn focus_first(panel: &web_sys::HtmlElement, id: Option<&str>) {
    let named = id
        .and_then(|id| document().get_element_by_id(id))
        .filter(|e| panel.contains(Some(e)))
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    let _ = named.as_ref().unwrap_or(panel).focus();
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

/// The panel of [`Dialog`] and [`Sheet`]: a grid. `content-start` keeps its rows at their own
/// height when the panel is taller than they are (a side sheet has the full height; without
/// it the rows share the free space, OT-10).
const GRID_PANEL: &str = "fixed z-50 grid content-start gap-4 border bg-background p-6 shadow-lg \
    focus-visible:outline-none";
/// The panel of [`FormDialog`]: a column whose middle part scrolls. In a window too low for
/// the title, the floor of the body and the footer (320 x 256 is 1280 x 1024 at 400 %, WCAG
/// 1.4.10) the panel scrolls as a whole, so every part stays in reach (review OT2B-2).
const FORM_PANEL: &str = "fixed z-50 flex flex-col overflow-y-auto border bg-background shadow-lg \
    focus-visible:outline-none";
/// Room on the right for the close button.
const FORM_HEADER: &str = "flex shrink-0 flex-col gap-1.5 pt-6 pr-12 pb-4 pl-6";
/// The padding inside the scroller keeps the focus rings of its fields from being cut off.
/// It shrinks before anything else does, down to 8 rem (three rows of fields).
const FORM_BODY: &str = "min-h-32 flex-1 overflow-y-auto px-6 pt-1 pb-4";
/// Outside the body's scroller, so no field is ever under it (WCAG 2.4.11); clear of the
/// home indicator of a phone.
const FORM_FOOTER: &str =
    "shrink-0 border-t border-border px-6 pt-4 pb-[max(1rem,env(safe-area-inset-bottom))]";
/// Below 640 px a bottom sheet of at most 85 % of the height; from 640 px centered, never
/// taller or wider than the window less a 1 rem margin.
const FORM_PLACE: &str = "inset-x-0 bottom-0 max-h-[85dvh] rounded-t-xl sm:right-auto sm:bottom-auto \
    sm:left-1/2 sm:top-1/2 sm:max-h-[calc(100dvh-2rem)] sm:w-[calc(100%-2rem)] \
    sm:-translate-x-1/2 sm:-translate-y-1/2 sm:rounded-lg";

#[component]
fn Panel(
    open: RwSignal<bool>,
    title: Signal<String>,
    /// Read when the panel opens and followed while it is open.
    description: MaybeProp<String>,
    close_label: String,
    panel_class: &'static str,
    class: MaybeProp<String>,
    return_to: Option<String>,
    initial_focus: Option<String>,
    can_close: Option<Callback<(), bool>>,
    /// With a footer the panel is a column: title, scrolling body, footer ([`FormDialog`]).
    footer: Option<ViewFn>,
    children: ChildrenFn,
) -> impl IntoView {
    // every way out (Escape, backdrop, close button) asks `can_close` first
    let close = move || {
        if can_close.is_none_or(|c| c.run(())) {
            open.set(false);
        }
    };
    // the ids come from the first title: a title that changes later (the log dialog is named
    // after its step) keeps them
    let base = slug(&title.get_untracked());
    let title_id = format!("{base}-title");
    let desc_id = format!("{base}-description");
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
                focus_first(&el, initial_focus.as_deref());
            }
        } else if was_open {
            restore_focus(opener.take(), return_to.as_deref());
        }
        (is_open, opener)
    });
    let children = StoredValue::new(children);
    let footer = StoredValue::new(footer);
    let (base, header) = if footer.with_value(Option::is_some) {
        (FORM_PANEL, FORM_HEADER)
    } else {
        (GRID_PANEL, "flex flex-col gap-1.5")
    };
    view! {
        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 bg-black/60" aria-hidden="true" on:click=move |_| close()></div>
            <div
                node_ref=panel
                role="dialog"
                aria-modal="true"
                aria-labelledby=title_id.clone()
                aria-describedby={let id = desc_id.clone(); move || description.get().map(|_| id.clone())}
                tabindex="-1"
                on:keydown=move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
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
                class=move || cn(&[base, panel_class, &class.get().unwrap_or_default()])
            >
                <div class=header>
                    <h2 id=title_id.clone() class="text-lg font-semibold leading-none tracking-tight">{move || title.get()}</h2>
                    {let id = desc_id.clone(); move || description.get().map(|d| view! {
                        <p id=id.clone() class="text-sm text-muted-foreground">{d}</p>
                    })}
                </div>
                {match footer.get_value() {
                    None => children.with_value(|c| c()).into_any(),
                    Some(f) => view! {
                        <div data-dialog-body="" class=FORM_BODY>{children.with_value(|c| c())}</div>
                        <div data-dialog-footer="" class=FORM_FOOTER>{f.run()}</div>
                    }.into_any(),
                }}
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
#[component]
pub fn Dialog(
    open: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] close_label: String,
    #[prop(into, optional)] description: Option<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] return_to: Option<String>,
    /// Id of the control that takes focus when the panel opens (else the panel itself).
    #[prop(into, optional)]
    initial_focus: Option<String>,
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
            description=MaybeProp::from(description)
            close_label=close_label
            panel_class="left-1/2 top-1/2 w-full max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-lg"
            class=class
            return_to=return_to
            initial_focus=initial_focus
            can_close=can_close
            footer=None
            children=children
        />
    }
}

/// Panel sliding in from a side.
#[component]
pub fn Sheet(
    open: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] close_label: String,
    #[prop(optional)] side: SheetSide,
    #[prop(into, optional)] description: Option<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] return_to: Option<String>,
    /// Id of the control that takes focus when the panel opens (else the panel itself).
    #[prop(into, optional)]
    initial_focus: Option<String>,
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
            description=MaybeProp::from(description)
            close_label=close_label
            panel_class=panel_class
            class=class
            return_to=return_to
            initial_focus=initial_focus
            can_close=can_close
            footer=None
            children=children
        />
    }
}

/// Width of a [`FormDialog`] from 640 px on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogWidth {
    /// At most 32 rem: a short form in one column.
    #[default]
    Regular,
    /// At most 42 rem: a form with a table or two columns.
    Wide,
}

impl DialogWidth {
    #[must_use]
    pub const fn classes(self) -> &'static str {
        match self {
            Self::Regular => "sm:max-w-lg",
            Self::Wide => "sm:max-w-2xl",
        }
    }
}

/// Modal dialog for a form (OT-10): a bottom sheet below 640 px, centered from 640 px. The
/// title and the `footer` (the form's buttons) stay in place, the children scroll between
/// them when the form is taller than the window. A submit button in the footer names its
/// form with the `form` attribute. Focus, Tab, Escape, `can_close` and the return of focus
/// are those of [`Dialog`].
#[component]
pub fn FormDialog(
    open: RwSignal<bool>,
    #[prop(into)] title: Signal<String>,
    #[prop(into)] close_label: String,
    #[prop(optional)] width: DialogWidth,
    /// The line under the title; a signal is followed (the log dialogs name the item).
    #[prop(into, optional)]
    description: MaybeProp<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] return_to: Option<String>,
    /// Id of the control that takes focus when the panel opens (else the panel itself).
    #[prop(into, optional)]
    initial_focus: Option<String>,
    /// Asked before the panel closes; `false` keeps it open (a form with unsaved changes
    /// shows its discard question instead).
    #[prop(optional, into)]
    can_close: Option<Callback<(), bool>>,
    /// The buttons of the form, below the scrolling part.
    #[prop(into)]
    footer: ViewFn,
    children: ChildrenFn,
) -> impl IntoView {
    let class = Signal::derive(move || cn(&[width.classes(), &class.get().unwrap_or_default()]));
    view! {
        <Panel
            open=open
            title=title
            description=description
            close_label=close_label
            panel_class=FORM_PLACE
            class=MaybeProp::from(class)
            return_to=return_to
            initial_focus=initial_focus
            can_close=can_close
            footer=Some(footer)
            children=children
        />
    }
}
