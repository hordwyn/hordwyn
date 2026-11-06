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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub variant: ToastVariant,
}

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
        let id = self.next.get_untracked();
        self.next.set(id + 1);
        self.list.update(|l| {
            l.push(Toast {
                id,
                title: title.into(),
                description,
                variant,
            });
        });
        let list = self.list;
        set_timeout(
            move || list.update(|l| l.retain(|t| t.id != id)),
            Duration::from_secs(5),
        );
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
        <div role="status" aria-live="polite" class="fixed bottom-[calc(1rem+var(--tab-bar-height))] right-4 z-[100] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2">
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
                        <button
                            type="button"
                            class="text-xs underline"
                            aria-label=dismiss_label.get_value()
                            on:click=move |_| toasts.dismiss(t.id)
                        >
                            <crate::icon::Icon name=crate::icon::IconName::Close/>
                        </button>
                    </div>
                </div>
            </For>
        </div>
    }
}
