use leptos::prelude::*;

use crate::class::cn;

/// Loading placeholder; hidden from assistive technology.
#[component]
pub fn Skeleton(#[prop(into, optional)] class: MaybeProp<String>) -> impl IntoView {
    view! {
        <div
            aria-hidden="true"
            class=move || cn(&["animate-pulse rounded-md bg-muted", &class.get().unwrap_or_default()])
        ></div>
    }
}
