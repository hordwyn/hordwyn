//! Navigation: settings `SideNav` and the mobile `TabBar`. Plain links (`<a href>`), so a
//! client-side router intercepts them and they work without JavaScript; the link whose
//! `href` equals `current` gets `aria-current="page"`.

use leptos::prelude::*;

use crate::class::cn;
use crate::icon::{Icon, IconName};

/// A navigation link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavItem {
    pub href: String,
    pub label: String,
    pub icon: Option<IconName>,
    /// Count or badge text, rendered inside the link so it is part of the accessible name
    /// ("Notifications 2 unread", design.md 8).
    pub count: Option<String>,
}

impl NavItem {
    #[must_use]
    pub fn new(href: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            href: href.into(),
            label: label.into(),
            icon: None,
            count: None,
        }
    }

    #[must_use]
    pub const fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    #[must_use]
    pub fn count(mut self, count: impl Into<String>) -> Self {
        self.count = Some(count.into());
        self
    }
}

/// A group of [`NavItem`]s with an optional heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavGroup {
    pub heading: Option<String>,
    pub items: Vec<NavItem>,
}

impl NavGroup {
    #[must_use]
    pub const fn new(items: Vec<NavItem>) -> Self {
        Self {
            heading: None,
            items,
        }
    }

    #[must_use]
    pub fn heading(mut self, heading: impl Into<String>) -> Self {
        self.heading = Some(heading.into());
        self
    }
}

fn current_attr(current: Signal<String>, href: String) -> impl Fn() -> Option<&'static str> {
    move || (current.get() == href).then_some("page")
}

/// Vertical navigation with group headings (settings, admin). Each group's list is labelled
/// by its heading (`{id}-group-{n}`).
#[component]
pub fn SideNav(
    #[prop(into)] id: String,
    /// Accessible name of the `<nav>` landmark.
    #[prop(into)]
    label: String,
    groups: Vec<NavGroup>,
    /// Path of the page being shown.
    #[prop(into)]
    current: Signal<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    view! {
        <nav aria-label=label class=move || cn(&["grid gap-1", &class.get().unwrap_or_default()])>
            {groups.into_iter().enumerate().map(|(n, g)| {
                let heading_id = format!("{id}-group-{n}");
                let labelled = g.heading.as_ref().map(|_| heading_id.clone());
                view! {
                    {g.heading.map(|h| view! {
                        <p id=heading_id class="px-2 pb-1 pt-3 text-xs font-medium uppercase tracking-wide text-muted-foreground">{h}</p>
                    })}
                    <ul aria-labelledby=labelled class="grid gap-0.5">
                        {g.items.into_iter().map(|i| {
                            let is_current = current_attr(current, i.href.clone());
                            view! {
                                <li>
                                    <a
                                        href=i.href
                                        aria-current=is_current
                                        class="flex min-h-9 items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground \
                                               hover:bg-accent hover:text-accent-foreground focus-visible:outline-none \
                                               focus-visible:ring-2 focus-visible:ring-ring \
                                               aria-[current=page]:bg-secondary aria-[current=page]:font-medium \
                                               aria-[current=page]:text-foreground"
                                    >
                                        {i.icon.map(|ic| view! { <Icon name=ic class="size-5"/> })}
                                        <span class="flex-1">{i.label}</span>
                                        {i.count.map(|c| view! { <span class="text-xs">{c}</span> })}
                                    </a>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }
            }).collect_view()}
        </nav>
    }
}

/// The centre button of a [`TabBar`] (for example "Add").
#[derive(Clone)]
pub struct TabBarAction {
    pub label: String,
    pub icon: IconName,
    pub on_press: Callback<()>,
}

impl TabBarAction {
    #[must_use]
    pub fn new(label: impl Into<String>, icon: IconName, on_press: Callback<()>) -> Self {
        Self {
            label: label.into(),
            icon,
            on_press,
        }
    }
}

const TAB: &str = "flex min-h-12 flex-col items-center justify-center gap-0.5 rounded-md px-1 text-xs \
    text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring \
    aria-[current=page]:font-semibold aria-[current=page]:text-foreground";

fn tab_link(i: NavItem, current: Signal<String>) -> impl IntoView {
    let is_current = current_attr(current, i.href.clone());
    view! {
        <li class="flex-1">
            <a href=i.href aria-current=is_current class=TAB>
                {i.icon.map(|ic| view! { <Icon name=ic class="size-5"/> })}
                <span>{i.label}</span>
                {i.count.map(|c| view! { <span class="sr-only">{c}</span> })}
            </a>
        </li>
    }
}

/// Mobile bottom navigation: links with icon and label, and an optional centre action placed
/// between the first and second half of `items`. Fixed to the bottom and hidden from 47.5rem
/// (760 px) up, the breakpoint where `input.css` sets `--tab-bar-height` to 0, unless
/// `inline` is set (gallery, previews).
#[component]
pub fn TabBar(
    #[prop(into)] label: String,
    items: Vec<NavItem>,
    #[prop(into)] current: Signal<String>,
    #[prop(optional, into)] action: Option<TabBarAction>,
    #[prop(optional)] inline: bool,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let mut first = items;
    let second = first.split_off(first.len().div_ceil(2));
    view! {
        <nav
            aria-label=label
            class=move || cn(&[
                "border-t bg-background px-1 pb-[env(safe-area-inset-bottom)]",
                if inline { "" } else { "fixed inset-x-0 bottom-0 z-40 min-[47.5rem]:hidden" },
                &class.get().unwrap_or_default(),
            ])
        >
            <ul class="flex items-stretch">
                {first.into_iter().map(|i| tab_link(i, current)).collect_view()}
                {action.map(|a| view! {
                    <li class="flex flex-1 justify-center">
                        <button
                            type="button"
                            on:click=move |_| a.on_press.run(())
                            class="flex min-h-12 flex-col items-center justify-center gap-0.5 text-xs font-medium \
                                   focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-md cursor-pointer"
                        >
                            <span class="inline-flex size-11 items-center justify-center rounded-full bg-primary text-primary-foreground">
                                <Icon name=a.icon class="size-5"/>
                            </span>
                            <span>{a.label}</span>
                        </button>
                    </li>
                })}
                {second.into_iter().map(|i| tab_link(i, current)).collect_view()}
            </ul>
        </nav>
    }
}
