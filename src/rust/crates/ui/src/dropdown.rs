//! Menu button: `aria-haspopup`, `aria-expanded`, `role="menu"` with `menuitem`s, arrow keys
//! move focus, Escape closes and returns focus to the trigger.

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

#[component]
pub fn DropdownMenu(
    /// Accessible name of the trigger and id prefix.
    #[prop(into)]
    label: String,
    items: Vec<MenuItem>,
    #[prop(into, optional)] class: MaybeProp<String>,
    /// Trigger content (for example an icon); defaults to `label`.
    #[prop(optional)]
    trigger: Option<Children>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let active = RwSignal::new(0usize);
    let base = slug(&label);
    let menu_id = format!("{base}-menu");
    let trigger_ref = NodeRef::<html::Button>::new();
    let menu_ref = NodeRef::<html::Div>::new();
    let count = items.len();
    let items = StoredValue::new(items);
    Effect::new(move |_| {
        if open.get()
            && let Some(menu) = menu_ref.get()
        {
            focus_item(&menu, active.get_untracked());
        }
    });
    let close = move || {
        open.set(false);
        if let Some(t) = trigger_ref.get_untracked() {
            let _ = t.focus();
        }
    };
    let on_menu_key = move |ev: leptos::ev::KeyboardEvent| {
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
        if let Some(menu) = menu_ref.get_untracked() {
            focus_item(&menu, next);
        }
    };
    let trigger_label = label.clone();
    view! {
        <div class=move || cn(&["relative inline-block", &class.get().unwrap_or_default()])>
            <button
                node_ref=trigger_ref
                type="button"
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                aria-controls=menu_id.clone()
                aria-label=trigger_label.clone()
                on:click=move |_| {
                    active.set(0);
                    open.update(|o| *o = !*o);
                }
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "ArrowDown" {
                        ev.prevent_default();
                        active.set(0);
                        open.set(true);
                    }
                }
                class="inline-flex h-9 items-center justify-center gap-2 rounded-md px-3 text-sm font-medium hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
                {match trigger {
                    Some(t) => t().into_any(),
                    None => label.clone().into_any(),
                }}
            </button>
            <Show when=move || open.get()>
                <div
                    node_ref=menu_ref
                    id=format!("{}-menu", slug(&label))
                    role="menu"
                    aria-label=label.clone()
                    on:keydown=on_menu_key
                    class="absolute right-0 z-50 mt-1 min-w-40 rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
                >
                    {items.with_value(|items| items.iter().map(|it| {
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
                                    if it.destructive { "text-destructive" } else { "" },
                                ])
                            >
                                {it.label.clone()}
                            </button>
                        }
                    }).collect_view())}
                </div>
            </Show>
        </div>
    }
}
