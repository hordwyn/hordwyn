//! Persistent inline messages: `Alert`, `EmptyState` and `StatusDot` (design.md 7.2). Semantic
//! colour is always paired with an icon and a word (WCAG 1.4.1).

use leptos::prelude::*;

use crate::class::cn;
use crate::heading::heading;
use crate::icon::{Icon, IconName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertVariant {
    #[default]
    Info,
    Success,
    Warning,
    Destructive,
}

impl AlertVariant {
    #[must_use]
    pub const fn classes(self) -> &'static str {
        match self {
            Self::Info => "border-info/35 bg-info-soft",
            Self::Success => "border-success/35 bg-success-soft",
            Self::Warning => "border-warning/35 bg-warning-soft",
            Self::Destructive => "border-destructive/35 bg-destructive-soft",
        }
    }

    const fn icon(self) -> (IconName, &'static str) {
        match self {
            Self::Info => (IconName::Info, "text-info"),
            Self::Success => (IconName::CircleCheck, "text-success"),
            Self::Warning => (IconName::Warning, "text-warning"),
            Self::Destructive => (IconName::CircleAlert, "text-destructive-text"),
        }
    }
}

/// Live-region role of an [`Alert`]. The caller decides (design.md 7.2): `Alert` for errors
/// that appear after an action, `Status` for other messages that appear after an action,
/// `None` for messages that are part of the page from the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertRole {
    #[default]
    Status,
    Alert,
    None,
}

impl AlertRole {
    const fn attr(self) -> Option<&'static str> {
        match self {
            Self::Status => Some("status"),
            Self::Alert => Some("alert"),
            Self::None => None,
        }
    }
}

/// Inline persistent message with icon, optional title, body (children) and actions.
#[component]
pub fn Alert(
    #[prop(optional)] variant: AlertVariant,
    #[prop(optional)] role: AlertRole,
    #[prop(into, optional)] title: MaybeProp<String>,
    #[prop(optional, into)] actions: Option<ViewFn>,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let (icon, tone) = variant.icon();
    view! {
        <div
            role=role.attr()
            data-variant=format!("{variant:?}").to_lowercase()
            class=move || cn(&[
                "flex items-start gap-3 rounded-lg border px-4 py-3 text-sm text-foreground",
                variant.classes(),
                &class.get().unwrap_or_default(),
            ])
        >
            <Icon name=icon class=cn(&["mt-0.5 size-5", tone])/>
            <div class="grid flex-1 gap-1">
                {move || title.get().map(|t| view! { <p class="font-semibold">{t}</p> })}
                <div>{children()}</div>
                {actions.map(|a| view! { <div class="mt-1 flex flex-wrap gap-2">{a.run()}</div> })}
            </div>
        </div>
    }
}

/// Stand-in for an empty list or section: icon, heading, body and up to two actions.
/// `level` is the heading level (default 2).
#[component]
pub fn EmptyState(
    #[prop(into)] title: String,
    #[prop(optional, into)] icon: Option<IconName>,
    #[prop(into, optional)] body: MaybeProp<String>,
    #[prop(optional)] level: Option<u8>,
    #[prop(optional, into)] primary: Option<ViewFn>,
    #[prop(optional, into)] secondary: Option<ViewFn>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let has_actions = primary.is_some() || secondary.is_some();
    view! {
        <div class=move || cn(&[
            "grid justify-items-center gap-2 rounded-xl border border-dashed border-input px-6 py-10 text-center",
            &class.get().unwrap_or_default(),
        ])>
            {icon.map(|i| view! { <Icon name=i class="size-10 text-muted-foreground"/> })}
            {heading(level.unwrap_or(2), None, "text-base font-semibold".into(), title)}
            {move || body.get().map(|b| view! { <p class="max-w-prose text-sm text-muted-foreground">{b}</p> })}
            {has_actions.then(|| view! {
                <div class="mt-2 flex flex-wrap justify-center gap-2">
                    {primary.map(|p| p.run())}
                    {secondary.map(|s| s.run())}
                </div>
            })}
        </div>
    }
}

/// Health of a thing shown by [`StatusDot`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StatusState {
    Ok,
    Warn,
    Bad,
    #[default]
    Off,
}

impl StatusState {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Warn => "warn",
            Self::Bad => "bad",
            Self::Off => "off",
        }
    }

    const fn dot(self) -> &'static str {
        match self {
            Self::Ok => "bg-success",
            Self::Warn => "bg-warning",
            Self::Bad => "bg-destructive",
            Self::Off => "bg-input",
        }
    }
}

/// A coloured dot followed by its status word. The word is required: the dot alone never
/// carries meaning (WCAG 1.4.1) and is hidden from assistive technology.
#[component]
pub fn StatusDot(
    #[prop(into)] state: Signal<StatusState>,
    #[prop(into)] label: Signal<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    view! {
        <span
            data-state=move || state.get().name()
            class=move || cn(&["inline-flex items-center gap-1.5 text-sm font-medium", &class.get().unwrap_or_default()])
        >
            <span aria-hidden="true" class=move || cn(&["inline-block size-2 shrink-0 rounded-full", state.get().dot()])></span>
            <span>{move || label.get()}</span>
        </span>
    }
}
