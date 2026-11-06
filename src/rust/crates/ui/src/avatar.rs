use leptos::prelude::*;

use crate::class::cn;

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

/// Round avatar: the image if `src` is set, otherwise the initials of `name`.
#[component]
pub fn Avatar(
    #[prop(into)] name: String,
    #[prop(into, optional)] src: MaybeProp<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
    /// Inside a control that has its own name: hidden from assistive technology.
    #[prop(optional)]
    decorative: bool,
) -> impl IntoView {
    let alt = name.clone();
    let text = initials(&name);
    if decorative {
        // drawn by CSS: no text node, so a labelled button's visible text is its name only
        return view! {
            <span
                aria-hidden="true"
                data-initials=text
                class=move || cn(&[
                    "relative flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-full bg-muted text-sm font-medium before:content-[attr(data-initials)]",
                    &class.get().unwrap_or_default(),
                ])
            ></span>
        }
        .into_any();
    }
    view! {
        <span class=move || cn(&[
            "relative flex size-9 shrink-0 items-center justify-center overflow-hidden rounded-full bg-muted text-sm font-medium",
            &class.get().unwrap_or_default(),
        ])>
            {move || match src.get() {
                Some(s) => view! { <img class="size-full object-cover" src=s alt=alt.clone()/> }.into_any(),
                None => view! { <span role="img" aria-label=alt.clone()>{text.clone()}</span> }.into_any(),
            }}
        </span>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    #[test]
    fn initials() {
        assert_eq!(super::initials("anna maria k"), "AM");
        assert_eq!(super::initials("bo"), "B");
    }
}
