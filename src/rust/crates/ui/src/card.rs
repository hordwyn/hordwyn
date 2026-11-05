use leptos::prelude::*;

use crate::class::cn;

macro_rules! simple {
    ($(#[$m:meta])* $name:ident, $tag:ident, $base:expr) => {
        $(#[$m])*
        #[component]
        pub fn $name(#[prop(into, optional)] class: MaybeProp<String>, children: Children) -> impl IntoView {
            view! { <$tag class=move || cn(&[$base, &class.get().unwrap_or_default()])>{children()}</$tag> }
        }
    };
}

simple!(
    /// Surface for grouped content.
    Card, div, "rounded-xl border bg-card text-card-foreground shadow-sm"
);
simple!(CardHeader, div, "flex flex-col gap-1.5 p-6");
simple!(CardTitle, h3, "font-semibold leading-none tracking-tight");
simple!(CardDescription, p, "text-sm text-muted-foreground");
simple!(CardContent, div, "p-6 pt-0");
simple!(CardFooter, div, "flex items-center p-6 pt-0");
