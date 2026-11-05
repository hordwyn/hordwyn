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
) -> impl IntoView {
    let alt = name.clone();
    let text = initials(&name);
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
}

#[cfg(test)]
mod tests {
    #[test]
    fn initials() {
        assert_eq!(super::initials("anna maria k"), "AM");
        assert_eq!(super::initials("bo"), "B");
    }
}
