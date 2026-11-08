use leptos::prelude::*;

use crate::class::cn;

macro_rules! simple {
    ($name:ident, $tag:ident, $base:expr) => {
        #[component]
        pub fn $name(#[prop(into, optional)] class: MaybeProp<String>, children: Children) -> impl IntoView {
            view! { <$tag class=move || cn(&[$base, &class.get().unwrap_or_default()])>{children()}</$tag> }
        }
    };
}

/// Scrollable table wrapper. It takes focus (`tabindex=0`), so a table wider than a small
/// screen can be scrolled with the keyboard (WCAG 2.1.1; axe `scrollable-region-focusable`).
#[component]
pub fn Table(
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div tabindex="0" class="relative w-full overflow-auto rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
            <table class=move || cn(&["w-full caption-bottom text-sm", &class.get().unwrap_or_default()])>
                {children()}
            </table>
        </div>
    }
}

simple!(TableHeader, thead, "[&_tr]:border-b");
simple!(TableBody, tbody, "[&_tr:last-child]:border-0");
simple!(TableRow, tr, "border-b transition-colors hover:bg-muted/50");
/// A column header (`scope="col"`, UX2-W19).
#[component]
pub fn TableHead(
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <th scope="col" class=move || cn(&["h-10 px-2 text-left align-middle font-medium text-muted-foreground", &class.get().unwrap_or_default()])>
            {children()}
        </th>
    }
}
simple!(TableCell, td, "p-2 align-middle");
simple!(TableCaption, caption, "mt-4 text-sm text-muted-foreground");
