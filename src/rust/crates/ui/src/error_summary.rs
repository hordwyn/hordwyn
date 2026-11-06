//! Error summary in the GOV.UK pattern: a list of the form's errors at the top, each linking
//! to its field; it takes focus when it appears so keyboard and screen-reader users land on it.

use leptos::html;
use leptos::prelude::*;

use crate::chip::focus_id;
use crate::class::cn;
use crate::heading::heading;

/// One entry of an [`ErrorSummary`]: the id of the field and the message shown next to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorLink {
    pub target: String,
    pub message: String,
}

impl ErrorLink {
    #[must_use]
    pub fn new(target: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            message: message.into(),
        }
    }
}

/// Renders nothing while `errors` is empty. When it becomes non-empty the container
/// (`tabindex="-1"`) receives focus; the links keep `href="#field"` and move focus to the
/// field itself when JavaScript runs. `level` is the heading level (default 2).
#[component]
pub fn ErrorSummary(
    #[prop(into)] id: String,
    #[prop(into)] title: String,
    #[prop(into)] errors: Signal<Vec<ErrorLink>>,
    #[prop(optional)] level: Option<u8>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let title_id = format!("{id}-title");
    let node = NodeRef::<html::Div>::new();
    Effect::new(move |_| {
        if !errors.get().is_empty()
            && let Some(el) = node.get()
        {
            let _ = el.focus();
        }
    });
    let level = level.unwrap_or(2);
    view! {
        <Show when=move || !errors.get().is_empty()>
            <div
                node_ref=node
                id=id.clone()
                role="group"
                tabindex="-1"
                aria-labelledby=title_id.clone()
                class=move || cn(&[
                    "grid gap-2 rounded-lg border-2 border-destructive p-4 focus-visible:outline-none \
                     focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 ring-offset-background",
                    &class.get().unwrap_or_default(),
                ])
            >
                {heading(level, Some(title_id.clone()), "text-base font-semibold".into(), title.clone())}
                <ul class="grid gap-1 text-sm">
                    <For each=move || errors.get() key=|e| e.target.clone() children=|e: ErrorLink| {
                        let target = e.target.clone();
                        view! {
                            <li>
                                <a
                                    href=format!("#{}", e.target)
                                    on:click=move |ev| {
                                        ev.prevent_default();
                                        focus_id(&target);
                                    }
                                    class="font-medium text-destructive-text underline underline-offset-4"
                                >
                                    {e.message}
                                </a>
                            </li>
                        }
                    }/>
                </ul>
            </div>
        </Show>
    }
}
