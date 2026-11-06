//! Heading with a caller-chosen level, so components fit into the page outline.

use leptos::prelude::*;

/// Render `content` in `<h2>`..`<h6>`; levels outside that range are clamped (a component
/// never emits the page's `<h1>`).
pub(crate) fn heading(
    level: u8,
    id: Option<String>,
    class: String,
    content: impl IntoView + 'static,
) -> AnyView {
    match level.clamp(2, 6) {
        2 => view! { <h2 id=id class=class>{content}</h2> }.into_any(),
        3 => view! { <h3 id=id class=class>{content}</h3> }.into_any(),
        4 => view! { <h4 id=id class=class>{content}</h4> }.into_any(),
        5 => view! { <h5 id=id class=class>{content}</h5> }.into_any(),
        _ => view! { <h6 id=id class=class>{content}</h6> }.into_any(),
    }
}
