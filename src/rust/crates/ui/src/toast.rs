//! Toasts: a context to push messages and a live region that announces them.

use std::time::Duration;

use leptos::prelude::*;

use crate::class::cn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastVariant {
    #[default]
    Default,
    Destructive,
}

/// A button in a toast (design.md 7.1: Undo). Running it dismisses the toast.
#[derive(Debug, Clone)]
pub struct ToastAction {
    pub label: String,
    pub on_click: Callback<()>,
}

impl ToastAction {
    #[must_use]
    pub fn new(label: impl Into<String>, on_click: Callback<()>) -> Self {
        Self {
            label: label.into(),
            on_click,
        }
    }
}

impl PartialEq for ToastAction {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label
    }
}

impl Eq for ToastAction {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub variant: ToastVariant,
    pub action: Option<ToastAction>,
}

/// How long a plain toast stays.
pub const TOAST_SECS: u64 = 5;
/// How long a toast with an action stays (design.md 7.1: 10 s for undoable).
pub const ACTION_TOAST_SECS: u64 = 10;

#[derive(Clone, Copy)]
pub struct Toasts {
    list: RwSignal<Vec<Toast>>,
    next: RwSignal<u64>,
}

impl Toasts {
    /// Show a toast for five seconds.
    pub fn push(
        &self,
        title: impl Into<String>,
        description: Option<String>,
        variant: ToastVariant,
    ) {
        self.add(title.into(), description, variant, None, TOAST_SECS);
    }

    /// Show a toast with an action button (Undo) for ten seconds.
    pub fn push_action(
        &self,
        title: impl Into<String>,
        description: Option<String>,
        action: ToastAction,
    ) {
        self.add(
            title.into(),
            description,
            ToastVariant::Default,
            Some(action),
            ACTION_TOAST_SECS,
        );
    }

    fn add(
        &self,
        title: String,
        description: Option<String>,
        variant: ToastVariant,
        action: Option<ToastAction>,
        secs: u64,
    ) {
        let id = self.next.get_untracked();
        self.next.set(id + 1);
        self.list.update(|l| {
            l.push(Toast {
                id,
                title,
                description,
                variant,
                action,
            });
        });
        // timers exist only in the browser (toasts are never pushed while rendering on the
        // server)
        #[cfg(target_arch = "wasm32")]
        {
            let list = self.list;
            set_timeout(
                move || list.update(|l| l.retain(|t| t.id != id)),
                Duration::from_secs(secs),
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = Duration::from_secs(secs);
    }

    pub fn dismiss(&self, id: u64) {
        self.list.update(|l| l.retain(|t| t.id != id));
    }
}

/// Provide the toast context (once, near the root).
#[must_use = "the returned handle is also available through use_toasts()"]
pub fn provide_toasts() -> Toasts {
    let t = Toasts {
        list: RwSignal::new(Vec::new()),
        next: RwSignal::new(0),
    };
    provide_context(t);
    t
}

/// # Panics
/// If [`provide_toasts`] was not called above.
#[must_use]
pub fn use_toasts() -> Toasts {
    expect_context::<Toasts>()
}

/// The live region rendering current toasts. `dismiss_label` labels the close buttons. It sits
/// above the mobile tab bar (`--tab-bar-height`, 0 on wide screens).
#[component]
pub fn Toaster(#[prop(into)] dismiss_label: String) -> impl IntoView {
    let toasts = use_toasts();
    let dismiss_label = StoredValue::new(dismiss_label);
    view! {
        <div role="status" aria-live="polite" data-float="" class="fixed bottom-[calc(1rem+var(--tab-bar-height))] right-4 z-[100] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2">
            <For each=move || toasts.list.get() key=|t| t.id let:t>
                <div class=cn(&[
                    "rounded-md border p-4 shadow-lg",
                    match t.variant {
                        ToastVariant::Default => "bg-background text-foreground",
                        ToastVariant::Destructive => "border-destructive bg-destructive text-destructive-foreground",
                    },
                ])>
                    <div class="flex items-start justify-between gap-2">
                        <div class="grid gap-1">
                            <p class="text-sm font-semibold">{t.title.clone()}</p>
                            {t.description.clone().map(|d| view! { <p class="text-sm opacity-90">{d}</p> })}
                        </div>
                        <div class="flex shrink-0 items-center gap-1">
                            {t.action.clone().map(|a| view! {
                                <button
                                    type="button"
                                    class="inline-flex h-8 items-center rounded-md border border-input px-3 text-sm font-medium hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
                                    on:click=move |_| {
                                        toasts.dismiss(t.id);
                                        a.on_click.run(());
                                    }
                                >
                                    {a.label.clone()}
                                </button>
                            })}
                            <button
                                type="button"
                                class="inline-flex size-8 items-center justify-center rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
                                aria-label=dismiss_label.get_value()
                                on:click=move |_| toasts.dismiss(t.id)
                            >
                                <crate::icon::Icon name=crate::icon::IconName::Close/>
                            </button>
                        </div>
                    </div>
                </div>
            </For>
        </div>
    }
}
