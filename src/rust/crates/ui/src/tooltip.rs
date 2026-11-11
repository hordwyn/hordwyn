//! Tooltip shown on hover and keyboard focus, linked with `aria-describedby`; Escape hides it.

use leptos::prelude::*;

use crate::class::slug;

#[component]
pub fn Tooltip(#[prop(into)] text: String, children: Children) -> impl IntoView {
    let show = RwSignal::new(false);
    let id = format!("tooltip-{}", slug(&text));
    view! {
        <span
            class="relative inline-flex"
            aria-describedby=id.clone()
            on:mouseenter=move |_| show.set(true)
            on:mouseleave=move |_| show.set(false)
            on:focusin=move |_| show.set(true)
            on:focusout=move |_| show.set(false)
            on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Escape" { show.set(false) }
        >
            {children()}
            <span
                id=id.clone()
                role="tooltip"
                class:hidden=move || !show.get()
                class="absolute bottom-full left-1/2 z-50 mb-2 -translate-x-1/2 whitespace-nowrap rounded-md bg-foreground px-3 py-1.5 text-xs text-background shadow-md"
            >
                {text.clone()}
            </span>
        </span>
    }
}
