use leptos::context::Provider;
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

/// A table part that names its role inside a stacked table ([`stacked_role`]).
macro_rules! roled {
    ($name:ident, $tag:ident, $role:expr, $base:expr) => {
        #[component]
        pub fn $name(#[prop(into, optional)] class: MaybeProp<String>, children: Children) -> impl IntoView {
            let role = stacked_role($role);
            view! { <$tag role=role class=move || cn(&[$base, &class.get().unwrap_or_default()])>{children()}</$tag> }
        }
    };
}

/// Below 47.5rem a `stacked` table shows each row as a card: the header row is hidden and
/// every cell shows its column name (the `label` of [`TableCell`]) above the value
/// (addendum-1 3.4 "Mobile", DEC-388).
const STACKED: &str = "max-[47.5rem]:block max-[47.5rem]:[&_thead]:hidden \
    max-[47.5rem]:[&_tbody]:grid max-[47.5rem]:[&_tbody]:gap-3 \
    max-[47.5rem]:[&_tr]:grid max-[47.5rem]:[&_tr]:gap-2 max-[47.5rem]:[&_tr]:rounded-xl \
    max-[47.5rem]:[&_tr]:border max-[47.5rem]:[&_tr]:p-3 max-[47.5rem]:[&_td]:block \
    max-[47.5rem]:[&_td]:p-0 max-[47.5rem]:[&_[data-cell-label]]:block";

/// Inside a `stacked` table (UX4-22): `display:block` and `grid` below 47.5rem drop the
/// implicit table roles, so the parts carry them explicitly there.
#[derive(Clone, Copy)]
struct Stacked(bool);

/// The explicit role of a table part, inside a stacked table only.
fn stacked_role(role: &'static str) -> Option<&'static str> {
    use_context::<Stacked>().filter(|s| s.0).map(|_| role)
}

/// Scrollable table wrapper. It takes focus (`tabindex=0`), so a table wider than a small
/// screen can be scrolled with the keyboard (WCAG 2.1.1; axe `scrollable-region-focusable`).
/// With `stacked`, rows become cards below 47.5rem instead (nothing to scroll there).
#[component]
pub fn Table(
    #[prop(into, optional)] class: MaybeProp<String>,
    /// Rows as labelled cards on narrow screens; give each cell a `label`.
    #[prop(optional)]
    stacked: bool,
    children: Children,
) -> impl IntoView {
    let (role, scope) = (stacked.then_some("table"), Stacked(stacked));
    let stacked = if stacked { STACKED } else { "" };
    view! {
        <div tabindex="0" class="relative w-full overflow-auto rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
            <table role=role class=move || cn(&["w-full caption-bottom text-sm", stacked, &class.get().unwrap_or_default()])>
                // a child owner: the flag reaches this table's parts only, not later siblings
                <Provider value=scope>{children()}</Provider>
            </table>
        </div>
    }
}

roled!(TableHeader, thead, "rowgroup", "[&_tr]:border-b");
roled!(TableBody, tbody, "rowgroup", "[&_tr:last-child]:border-0");
roled!(
    TableRow,
    tr,
    "row",
    "border-b transition-colors hover:bg-muted/50"
);
/// A column header (`scope="col"`, UX2-W19).
#[component]
pub fn TableHead(
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let role = stacked_role("columnheader");
    view! {
        <th scope="col" role=role class=move || cn(&["h-10 px-2 text-left align-middle font-medium text-muted-foreground", &class.get().unwrap_or_default()])>
            {children()}
        </th>
    }
}

/// A cell; `label` (the column name) shows above the value only in a stacked table on a
/// narrow screen, where the header row is hidden.
#[component]
pub fn TableCell(
    #[prop(into, optional)] class: MaybeProp<String>,
    #[prop(into, optional)] label: Option<String>,
    children: Children,
) -> impl IntoView {
    let role = stacked_role("cell");
    view! {
        <td role=role class=move || cn(&["p-2 align-middle", &class.get().unwrap_or_default()])>
            {label.map(|l| view! {
                <span data-cell-label="" class="hidden text-xs font-medium text-muted-foreground">{l}</span>
            })}
            {children()}
        </td>
    }
}
simple!(TableCaption, caption, "mt-4 text-sm text-muted-foreground");
