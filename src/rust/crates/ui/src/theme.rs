//! Light/dark toggle: sets the `dark` class on `<html>` and remembers the choice in
//! `localStorage` under a brand-derived key.

use leptos::prelude::*;

use crate::icon::{Icon, IconName};

fn apply(dark: bool, key: &str) {
    if let Some(root) = document().document_element() {
        let _ = if dark {
            root.class_list().add_1("dark")
        } else {
            root.class_list().remove_1("dark")
        };
    }
    if let Ok(Some(storage)) = window().local_storage() {
        let _ = storage.set_item(key, if dark { "dark" } else { "light" });
    }
}

#[component]
pub fn ThemeToggle(
    /// Label announced when switching to dark.
    #[prop(into)]
    dark_label: String,
    /// Label announced when switching to light.
    #[prop(into)]
    light_label: String,
    /// `localStorage` key (derived from the brand, D-10).
    #[prop(into)]
    storage_key: String,
) -> impl IntoView {
    let dark = RwSignal::new(false);
    let key = StoredValue::new(storage_key);
    Effect::new(move |_| {
        // runs in the browser only: read the class the inline script set before paint
        let is_dark = document()
            .document_element()
            .is_some_and(|r| r.class_list().contains("dark"));
        dark.set(is_dark);
    });
    view! {
        <button
            type="button"
            aria-label=move || if dark.get() { light_label.clone() } else { dark_label.clone() }
            on:click=move |_| {
                let next = !dark.get_untracked();
                dark.set(next);
                key.with_value(|k| apply(next, k));
            }
            class="inline-flex size-9 items-center justify-center rounded-md hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
            {move || if dark.get() {
                view! { <Icon name=IconName::Sun/> }.into_any()
            } else {
                view! { <Icon name=IconName::Moon/> }.into_any()
            }}
        </button>
    }
}
