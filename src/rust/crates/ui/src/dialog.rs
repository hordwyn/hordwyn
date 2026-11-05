//! Modal dialog and side sheet: `role="dialog"`, `aria-modal`, labelled by its title,
//! Escape and backdrop click close it, focus moves into the panel when it opens.

use leptos::html;
use leptos::prelude::*;

use crate::class::{cn, slug};
use crate::icon::{Icon, IconName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SheetSide {
    Left,
    #[default]
    Right,
}

#[component]
fn Panel(
    open: RwSignal<bool>,
    title: String,
    description: Option<String>,
    close_label: String,
    panel_class: &'static str,
    class: MaybeProp<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let base = slug(&title);
    let title_id = format!("{base}-title");
    let desc_id = description.as_ref().map(|_| format!("{base}-description"));
    let panel = NodeRef::<html::Div>::new();
    Effect::new(move |_| {
        if open.get()
            && let Some(el) = panel.get()
        {
            let _ = el.focus();
        }
    });
    let children = StoredValue::new(children);
    view! {
        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 bg-black/60" aria-hidden="true" on:click=move |_| open.set(false)></div>
            <div
                node_ref=panel
                role="dialog"
                aria-modal="true"
                aria-labelledby=title_id.clone()
                aria-describedby=desc_id.clone()
                tabindex="-1"
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "Escape" {
                        open.set(false);
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
                    on:click=move |_| open.set(false)
                    class="absolute right-4 top-4 rounded-sm opacity-70 hover:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                >
                    <Icon name=IconName::Close/>
                </button>
            </div>
        </Show>
    }
}

/// Centered modal dialog. `close_label` is the (translated) label of the close button.
#[component]
pub fn Dialog(
    open: RwSignal<bool>,
    #[prop(into)] title: String,
    #[prop(into)] close_label: String,
    #[prop(into, optional)] description: Option<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
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
    children: ChildrenFn,
) -> impl IntoView {
    let panel_class = match side {
        SheetSide::Left => "inset-y-0 left-0 h-full w-3/4 max-w-sm border-r",
        SheetSide::Right => "inset-y-0 right-0 h-full w-3/4 max-w-sm border-l",
    };
    view! {
        <Panel
            open=open
            title=title
            description=description
            close_label=close_label
            panel_class=panel_class
            class=class
            children=children
        />
    }
}
