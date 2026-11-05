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

/// Scrollable table wrapper.
#[component]
pub fn Table(
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="relative w-full overflow-auto">
            <table class=move || cn(&["w-full caption-bottom text-sm", &class.get().unwrap_or_default()])>
                {children()}
            </table>
        </div>
    }
}

simple!(TableHeader, thead, "[&_tr]:border-b");
simple!(TableBody, tbody, "[&_tr:last-child]:border-0");
simple!(TableRow, tr, "border-b transition-colors hover:bg-muted/50");
simple!(
    TableHead,
    th,
    "h-10 px-2 text-left align-middle font-medium text-muted-foreground"
);
simple!(TableCell, td, "p-2 align-middle");
simple!(TableCaption, caption, "mt-4 text-sm text-muted-foreground");
