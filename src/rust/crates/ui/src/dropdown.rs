//! Menu button: `aria-haspopup`, `aria-expanded`, `role="menu"` with `menuitem`s, arrow keys
//! move focus, Escape closes and returns focus to the trigger, a press outside closes it.

use leptos::html;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;

use crate::class::{cn, slug};

#[derive(Clone)]
pub struct MenuItem {
    pub label: String,
    pub on_select: Callback<()>,
    pub destructive: bool,
}

fn focus_item(menu: &web_sys::HtmlElement, index: usize) {
    if let Ok(items) = menu.query_selector_all("[role=menuitem]")
        && let Some(node) = items.item(u32::try_from(index).unwrap_or(0))
        && let Ok(el) = node.dyn_into::<web_sys::HtmlElement>()
    {
        let _ = el.focus();
    }
}

/// May an open menu take the focus? Only when the focus is nowhere (on `body`: the item that
/// had it left with a render a refresh replaced) or already inside this dropdown; never from a
/// field or a dialog the person moved to while the menu stayed open (RACE-2, WCAG 3.2.1).
fn may_take_focus(root: &web_sys::Node) -> bool {
    document()
        .active_element()
        .is_none_or(|a| a.tag_name().eq_ignore_ascii_case("body") || root.contains(Some(&a)))
}

/// Is the target of `ev` inside `root`?
fn inside(root: &web_sys::Node, ev: &web_sys::Event) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
        .is_some_and(|n| root.contains(Some(&n)))
}

/// While `open`: a press outside `root` closes the menu (the focus goes where the press put
/// it, not back to the trigger). One window listener per open menu, removed when it closes.
fn close_on_outside_press(open: RwSignal<bool>, root: NodeRef<html::Div>) {
    let handle = StoredValue::new(None::<WindowListenerHandle>);
    let stop = move || {
        if let Some(h) = handle.try_update_value(Option::take).flatten() {
            h.remove();
        }
    };
    Effect::new(move |_| {
        if !open.get() {
            stop();
        } else if handle.with_value(Option::is_none) {
            let h = window_event_listener(leptos::ev::pointerdown, move |ev| {
                if root.get_untracked().is_some_and(|r| !inside(&r, &ev)) {
                    open.set(false);
                }
            });
            handle.set_value(Some(h));
        }
    });
    on_cleanup(stop);
}

/// Arrow keys, Home and End move the active item (wrapping); Escape and Tab close the menu.
fn menu_keys(
    active: RwSignal<usize>,
    labels: StoredValue<Vec<String>>,
    active_label: RwSignal<String>,
    count: usize,
    menu_ref: NodeRef<html::Div>,
    close: impl Fn() + Copy + 'static,
) -> impl Fn(leptos::ev::KeyboardEvent) + Copy + 'static {
    move |ev: leptos::ev::KeyboardEvent| {
        let next = match ev.key().as_str() {
            "ArrowDown" => (active.get_untracked() + 1) % count.max(1),
            "ArrowUp" => (active.get_untracked() + count.max(1) - 1) % count.max(1),
            "Home" => 0,
            "End" => count.saturating_sub(1),
            "Escape" | "Tab" => {
                close();
                return;
            }
            _ => return,
        };
        ev.prevent_default();
        active.set(next);
        active_label.set(
            labels
                .try_with_value(|l| l.get(next).cloned())
                .flatten()
                .unwrap_or_default(),
        );
        if let Some(menu) = menu_ref.get_untracked() {
            focus_item(&menu, next);
        }
    }
}

/// The menu's items: each runs its action, then the menu closes (focus back on the trigger).
fn menu_items(
    items: StoredValue<Vec<MenuItem>>,
    close: impl Fn() + Copy + 'static,
) -> impl IntoView {
    // `try_`: a menu opened on a trigger whose render was just replaced renders nothing
    items.try_with_value(|items| {
        items
            .iter()
            .map(|it| {
                let on_select = it.on_select;
                view! {
                    <button
                        type="button"
                        role="menuitem"
                        tabindex="-1"
                        on:click=move |_| {
                            on_select.run(());
                            close();
                        }
                        class=cn(&[
                            "flex w-full cursor-pointer items-center rounded-sm px-2 py-1.5 text-left text-sm outline-none focus:bg-accent focus:text-accent-foreground",
                            if it.destructive { "text-destructive-text" } else { "" },
                        ])
                    >
                        {it.label.clone()}
                    </button>
                }
            })
            .collect_view()
    })
}

#[component]
pub fn DropdownMenu(
    /// Accessible name of the trigger and id prefix.
    #[prop(into)]
    label: String,
    items: Vec<MenuItem>,
    #[prop(into, optional)] class: MaybeProp<String>,
    /// Element id prefix (default: from `label`); give one when several menus share a label.
    #[prop(into, optional)]
    id: Option<String>,
    /// Extra classes for the trigger button (for example a background over an image).
    #[prop(into, optional)]
    trigger_class: MaybeProp<String>,
    /// Trigger content (for example an icon); defaults to `label`.
    #[prop(optional)]
    trigger: Option<Children>,
    /// The trigger's tab stop (a roving grid sets `-1` on all but one card).
    #[prop(into, optional)]
    trigger_tabindex: MaybeProp<String>,
    /// Whether the menu is open, owned by the caller: a menu inside a view that a background
    /// refresh rebuilds stays open when the caller keeps this signal outside that view.
    #[prop(optional_no_strip, into)]
    open: Option<RwSignal<bool>>,
    /// The index of the item with the focus, owned by the caller next to `open`: the menu of
    /// a new render gives the focus back to the same item (RACE-2).
    #[prop(optional_no_strip, into)]
    active: Option<RwSignal<usize>>,
    /// The label of that item, owned by the caller next to `active`: when a new render has
    /// other items before it (an item came or went), the focus follows the label, not the
    /// position (#2).
    #[prop(optional_no_strip, into)]
    active_label: Option<RwSignal<String>>,
) -> impl IntoView {
    let open = open.unwrap_or_else(|| RwSignal::new(false));
    let active = active.unwrap_or_else(|| RwSignal::new(0usize));
    let active_label = active_label.unwrap_or_else(|| RwSignal::new(String::new()));
    let labels = StoredValue::new(items.iter().map(|i| i.label.clone()).collect::<Vec<_>>());
    let remembered = active_label.get_untracked();
    if !remembered.is_empty()
        && let Some(i) = labels.with_value(|l| l.iter().position(|x| *x == remembered))
    {
        active.set(i);
    }
    let set_first = move || {
        active.set(0);
        // `try_`: a click can reach the trigger of a menu whose render was just replaced
        active_label.set(
            labels
                .try_with_value(|l| l.first().cloned())
                .flatten()
                .unwrap_or_default(),
        );
    };
    let base = id.unwrap_or_else(|| slug(&label));
    let menu_id = format!("{base}-menu");
    let panel_id = menu_id.clone();
    let trigger_ref = NodeRef::<html::Button>::new();
    let menu_ref = NodeRef::<html::Div>::new();
    let root_ref = NodeRef::<html::Div>::new();
    let count = items.len();
    let items = StoredValue::new(items);
    Effect::new(move |_| {
        if open.get()
            && let Some(menu) = menu_ref.get()
            && root_ref.get_untracked().is_some_and(|r| may_take_focus(&r))
        {
            focus_item(&menu, active.get_untracked());
        }
    });
    close_on_outside_press(open, root_ref);
    let close = move || {
        open.set(false);
        if let Some(t) = trigger_ref.get_untracked() {
            let _ = t.focus();
        }
    };
    let on_menu_key = menu_keys(active, labels, active_label, count, menu_ref, close);
    let trigger_label = label.clone();
    view! {
        <div node_ref=root_ref class=move || cn(&["relative inline-block", &class.get().unwrap_or_default()])>
            <button
                node_ref=trigger_ref
                type="button"
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                aria-controls=menu_id.clone()
                aria-label=trigger_label.clone()
                tabindex=move || trigger_tabindex.get()
                on:click=move |_| {
                    set_first();
                    open.update(|o| *o = !*o);
                }
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "ArrowDown" {
                        ev.prevent_default();
                        set_first();
                        open.set(true);
                    }
                }
                class=move || cn(&[
                    "inline-flex h-9 items-center justify-center gap-2 rounded-md px-3 text-sm font-medium hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer",
                    &trigger_class.get().unwrap_or_default(),
                ])
            >
                {match trigger {
                    Some(t) => t().into_any(),
                    None => label.clone().into_any(),
                }}
            </button>
            <Show when=move || open.get()>
                <div
                    node_ref=menu_ref
                    id=panel_id.clone()
                    role="menu"
                    aria-label=label.clone()
                    on:keydown=on_menu_key
                    class="absolute right-0 z-50 mt-1 min-w-40 rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
                >
                    {menu_items(items, close)}
                </div>
            </Show>
        </div>
    }
}
