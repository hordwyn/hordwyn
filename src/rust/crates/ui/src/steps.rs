//! Progress through a sequence: `Steps` (wizard header) and `TaskList` (setup checklist).
//! Done and current states are conveyed in text and ARIA, not by colour alone.

use leptos::prelude::*;

use crate::class::cn;
use crate::icon::{Icon, IconName};

/// Ordered wizard steps. `current` is the zero-based index of the active step (it gets
/// `aria-current="step"`); earlier steps count as done and carry `done_label` as hidden text.
#[component]
pub fn Steps(
    /// Accessible name of the list, e.g. "Setup progress".
    #[prop(into)]
    label: String,
    #[prop(into)] steps: Signal<Vec<String>>,
    #[prop(into)] current: Signal<usize>,
    /// Hidden suffix for finished steps, e.g. "completed".
    #[prop(into)]
    done_label: String,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let done_label = StoredValue::new(done_label);
    view! {
        <ol
            aria-label=label
            class=move || cn(&["flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-muted-foreground", &class.get().unwrap_or_default()])
        >
            {move || {
                let cur = current.get();
                steps.get().into_iter().enumerate().map(|(i, name)| {
                    let (state, marker) = match i.cmp(&cur) {
                        std::cmp::Ordering::Less => ("done", "border-success bg-success text-background"),
                        std::cmp::Ordering::Equal => ("current", "border-primary bg-primary text-primary-foreground"),
                        std::cmp::Ordering::Greater => ("todo", "border-input"),
                    };
                    view! {
                        <li
                            data-state=state
                            aria-current=(state == "current").then_some("step")
                            class=cn(&["inline-flex items-center gap-2", if state == "current" { "font-semibold text-foreground" } else { "" }])
                        >
                            <span aria-hidden="true" class=cn(&["inline-flex size-6 items-center justify-center rounded-full border text-xs", marker])>
                                {if state == "done" {
                                    view! { <Icon name=IconName::Check class="size-3.5"/> }.into_any()
                                } else {
                                    (i + 1).to_string().into_any()
                                }}
                            </span>
                            <span>{name}</span>
                            {(state == "done").then(|| view! { <span class="sr-only">{done_label.get_value()}</span> })}
                        </li>
                    }
                }).collect_view()
            }}
        </ol>
    }
}

/// One row of a [`TaskList`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskItem {
    pub id: String,
    pub title: String,
    pub hint: Option<String>,
    pub done: bool,
    /// Where the task is done; the title becomes a link.
    pub href: Option<String>,
}

impl TaskItem {
    #[must_use]
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            hint: None,
            done: false,
            href: None,
        }
    }

    #[must_use]
    pub const fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    #[must_use]
    pub fn href(mut self, href: impl Into<String>) -> Self {
        self.href = Some(href.into());
        self
    }
}

/// Setup checklist: each row shows a state icon, the title (a link when `href` is set), an
/// optional hint and the state word (`done_label` / `todo_label`).
#[component]
pub fn TaskList(
    #[prop(into)] label: String,
    #[prop(into)] items: Signal<Vec<TaskItem>>,
    #[prop(into)] done_label: String,
    #[prop(into)] todo_label: String,
    /// Called with the id when a row link is clicked; `true` means it was handled here
    /// (the link is not followed). Before hydration the link works as a plain link.
    #[prop(optional, into)]
    on_open: Option<Callback<String, bool>>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let labels = StoredValue::new((done_label, todo_label));
    view! {
        <ul aria-label=label class=move || cn(&["grid gap-2", &class.get().unwrap_or_default()])>
            <For each=move || items.get() key=|t| (t.id.clone(), t.done) children=move |t: TaskItem| {
                let (done, todo) = labels.get_value();
                let id = t.id.clone();
                let title = match t.href {
                    Some(h) => view! {
                        <a href=h class="font-medium underline-offset-4 hover:underline"
                            on:click=move |ev| if on_open.is_some_and(|f| f.run(id.clone())) { ev.prevent_default(); }>
                            {t.title}
                        </a>
                    }.into_any(),
                    None => view! { <span class="font-medium">{t.title}</span> }.into_any(),
                };
                view! {
                    <li
                        data-state=if t.done { "done" } else { "todo" }
                        class=cn(&["flex items-center gap-3 rounded-lg border bg-card px-4 py-3", if t.done { "text-muted-foreground" } else { "" }])
                    >
                        {if t.done {
                            view! { <Icon name=IconName::CircleCheck class="size-5 text-success"/> }
                        } else {
                            view! { <Icon name=IconName::Circle class="size-5 text-muted-foreground"/> }
                        }}
                        <div class="grid flex-1 gap-0.5">
                            {title}
                            {t.hint.map(|h| view! { <span class="text-sm text-muted-foreground">{h}</span> })}
                        </div>
                        <span class="text-xs font-medium">{if t.done { done } else { todo }}</span>
                    </li>
                }
            }/>
        </ul>
    }
}
