use leptos::prelude::*;

use crate::class::{cn, cva};
use crate::icon::{Icon, IconName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Default,
    Secondary,
    Outline,
    Ghost,
    Destructive,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    Sm,
    #[default]
    Md,
    Lg,
    Icon,
}

impl ButtonVariant {
    #[must_use]
    pub const fn classes(self) -> &'static str {
        match self {
            Self::Default => "bg-primary text-primary-foreground hover:bg-primary/90",
            Self::Secondary => "bg-secondary text-secondary-foreground hover:bg-secondary/80",
            Self::Outline => {
                "border border-input bg-background hover:bg-accent hover:text-accent-foreground"
            }
            Self::Ghost => "hover:bg-accent hover:text-accent-foreground",
            Self::Destructive => {
                "bg-destructive text-destructive-foreground hover:bg-destructive/90"
            }
            Self::Link => "text-primary underline-offset-4 hover:underline",
        }
    }
}

impl ButtonSize {
    #[must_use]
    pub const fn classes(self) -> &'static str {
        match self {
            Self::Sm => "h-8 rounded-md px-3 text-xs",
            Self::Md => "h-9 px-4 py-2",
            Self::Lg => "h-10 rounded-md px-8",
            Self::Icon => "size-9",
        }
    }
}

const BASE: &str = "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm \
    font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 \
    focus-visible:ring-ring focus-visible:ring-offset-2 ring-offset-background \
    disabled:pointer-events-none disabled:opacity-50 cursor-pointer";

/// Button. Event handlers (`on:click`) and attributes added at the call site are forwarded
/// to the `<button>` element.
///
/// `loading` shows a spinner in front of the unchanged label, sets `aria-busy` and disables
/// the button so a pending request cannot be sent twice (design.md 7.1).
#[component]
pub fn Button(
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] size: ButtonSize,
    #[prop(into, optional)] disabled: Signal<bool>,
    #[prop(into, optional)] loading: Signal<bool>,
    #[prop(into, optional)] class: MaybeProp<String>,
    /// `button` (default), `submit` or `reset`.
    #[prop(into, optional)]
    r#type: Option<&'static str>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            type=r#type.unwrap_or("button")
            disabled=move || disabled.get() || loading.get()
            aria-busy=move || loading.get().then_some("true")
            class=move || cn(&[&cva(BASE, &[variant.classes(), size.classes()]), &class.get().unwrap_or_default()])
        >
            {move || loading.get().then(|| view! { <Icon name=IconName::Spinner class="animate-spin"/> })}
            {children()}
        </button>
    }
}
