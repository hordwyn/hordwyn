use leptos::prelude::*;

use crate::class::{cn, cva};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Outline,
    Destructive,
}

impl BadgeVariant {
    #[must_use]
    pub const fn classes(self) -> &'static str {
        match self {
            Self::Default => "border-transparent bg-primary text-primary-foreground",
            Self::Secondary => "border-transparent bg-secondary text-secondary-foreground",
            Self::Outline => "text-foreground",
            Self::Destructive => "border-transparent bg-destructive text-destructive-foreground",
        }
    }
}

#[component]
pub fn Badge(
    #[prop(optional)] variant: BadgeVariant,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let base =
        "inline-flex items-center gap-1 rounded-md border px-2.5 py-0.5 text-xs font-semibold";
    view! {
        <span class=move || cn(&[&cva(base, &[variant.classes()]), &class.get().unwrap_or_default()])>
            {children()}
        </span>
    }
}
