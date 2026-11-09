//! Command palette list: a combobox input filtering a listbox, arrow keys move the active
//! option (`aria-activedescendant`), Enter selects, Ctrl/Cmd+Enter runs the secondary action
//! (design.md 6.4: add as Planned or the next step). Used for the Ctrl-K search palette.

use leptos::prelude::*;

use crate::class::{cn, slug};
use crate::copy::KeyHint;
use crate::icon::{Icon, IconName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    pub id: String,
    pub label: String,
    /// Group heading (for example the connector a result came from).
    pub group: String,
    /// Extra words that match the query.
    pub keywords: String,
}

/// The key hints the active row shows (design.md 6.4): the keys and what they do. An empty
/// `secondary` text hides the second hint.
#[derive(Clone)]
pub struct CommandHints {
    pub enter_keys: Vec<String>,
    pub enter: Signal<String>,
    pub secondary_keys: Vec<String>,
    pub secondary: Signal<String>,
}

/// The hints as one sentence for the input's description ("Enter: Open. Ctrl+Enter: Add.").
fn hint_text(h: &CommandHints) -> String {
    let mut out = format!("{}: {}.", h.enter_keys.join("+"), h.enter.get());
    let second = h.secondary.get();
    if !second.is_empty() {
        out.push(' ');
        out.push_str(&h.secondary_keys.join("+"));
        out.push_str(": ");
        out.push_str(&second);
        out.push('.');
    }
    out
}

fn matches(item: &CommandItem, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    q.is_empty()
        || item.label.to_lowercase().contains(&q)
        || item.keywords.to_lowercase().contains(&q)
}

/// Arrow keys move the active option (wrapping), Enter selects it, Ctrl/Cmd+Enter runs the
/// secondary action (Enter's when there is none).
fn on_key(
    ev: &leptos::ev::KeyboardEvent,
    filtered: Memo<Vec<CommandItem>>,
    active: RwSignal<usize>,
    (on_select, on_secondary): (Callback<CommandItem>, Option<Callback<CommandItem>>),
) {
    let n = filtered.with_untracked(Vec::len);
    match ev.key().as_str() {
        "ArrowDown" if n > 0 => {
            ev.prevent_default();
            active.update(|a| *a = (*a + 1) % n);
        }
        "ArrowUp" if n > 0 => {
            ev.prevent_default();
            active.update(|a| *a = (*a + n - 1) % n);
        }
        "Enter" => {
            if let Some(item) = filtered.with_untracked(|f| f.get(active.get_untracked()).cloned())
            {
                ev.prevent_default();
                match on_secondary.filter(|_| ev.ctrl_key() || ev.meta_key()) {
                    Some(cb) => cb.run(item),
                    None => on_select.run(item),
                }
            }
        }
        _ => {}
    }
}

/// `query` is exposed so callers can fetch results as the user types; local filtering still
/// applies to the given `items`.
#[component]
pub fn Command(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] placeholder: String,
    /// Shown while nothing matches; an empty text hides the row (the caller explains why).
    #[prop(into)]
    empty_label: Signal<String>,
    #[prop(into)] items: Signal<Vec<CommandItem>>,
    query: RwSignal<String>,
    on_select: Callback<CommandItem>,
    /// Ctrl/Cmd+Enter on the active option.
    #[prop(optional_no_strip, into)]
    on_secondary: Option<Callback<CommandItem>>,
    /// Key hints on the active row, also given to the input as its description.
    #[prop(optional)]
    hints: Option<CommandHints>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let active = RwSignal::new(0usize);
    let list_id = format!("{id}-list");
    let hints_id = format!("{id}-hints");
    let described = hints.as_ref().map(|_| hints_id.clone());
    let hints = StoredValue::new(hints);
    let filtered = Memo::new(move |_| {
        let q = query.get();
        items
            .get()
            .into_iter()
            .filter(|i| matches(i, &q))
            .collect::<Vec<_>>()
    });
    Effect::new(move |_| {
        filtered.track();
        active.set(0);
    });
    let prefix = id.clone();
    let option_id =
        StoredValue::new(move |item_id: &str| format!("{prefix}-opt-{}", slug(item_id)));
    view! {
        <div class=move || cn(&["flex flex-col overflow-hidden rounded-md bg-popover text-popover-foreground", &class.get().unwrap_or_default()])>
            <div class="flex items-center gap-2 border-b px-3">
                <Icon name=IconName::Search class="opacity-50"/>
                <input
                    id=id.clone()
                    type="text"
                    role="combobox"
                    aria-label=label
                    aria-expanded="true"
                    aria-controls=list_id.clone()
                    aria-autocomplete="list"
                    aria-describedby=described
                    aria-activedescendant=move || {
                        filtered.with(|f| f.get(active.get()).map(|i| option_id.with_value(|o| o(&i.id))))
                    }
                    placeholder=placeholder
                    autocomplete="off"
                    prop:value=move || query.get()
                    on:input=move |ev| query.set(event_target_value(&ev))
                    on:keydown=move |ev| on_key(&ev, filtered, active, (on_select, on_secondary))
                    class="flex h-11 w-full bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground"
                />
                {hints.get_value().map(|h| view! {
                    <span id=hints_id class="sr-only">{move || hint_text(&h)}</span>
                })}
            </div>
            <ul id=list_id role="listbox" class="max-h-80 overflow-y-auto p-1 max-[47.5rem]:max-h-none">
                <Show when=move || filtered.with(Vec::is_empty) && !empty_label.with(String::is_empty)>
                    <li role="presentation" class="py-6 text-center text-sm text-muted-foreground">{move || empty_label.get()}</li>
                </Show>
                {move || {
                    let mut last_group = String::new();
                    filtered.get().into_iter().enumerate().map(|(i, item)| {
                        let heading = (item.group != last_group).then(|| item.group.clone());
                        last_group.clone_from(&item.group);
                        let id = option_id.with_value(|o| o(&item.id));
                        let hints = hints.get_value();
                        view! { <CommandOption item=item index=i id=id heading=heading active=active on_select=on_select hints=hints/> }
                    }).collect_view()
                }}
            </ul>
        </div>
    }
}

/// One option, with its group heading before it when the group changes. Rows are at least
/// 44 px high on phones (WCAG 2.5.8, design.md 6.4). The active row shows the key hints
/// (hidden from screen readers: the input's description says them).
#[component]
fn CommandOption(
    item: CommandItem,
    index: usize,
    id: String,
    heading: Option<String>,
    active: RwSignal<usize>,
    on_select: Callback<CommandItem>,
    hints: Option<CommandHints>,
) -> impl IntoView {
    let selected = move || active.get() == index;
    let label = item.label.clone();
    let hint = move || {
        let h = hints.clone().filter(|_| selected())?;
        let second = h.secondary.get();
        Some(view! {
            <span aria-hidden="true" class="ml-auto flex shrink-0 flex-wrap items-center justify-end gap-x-3 gap-y-1 pl-3">
                <span class="inline-flex items-center gap-1 text-xs"><KeyHint keys=h.enter_keys.clone()/> {h.enter.get()}</span>
                {(!second.is_empty()).then(|| view! {
                    <span class="inline-flex items-center gap-1 text-xs"><KeyHint keys=h.secondary_keys.clone()/> {second.clone()}</span>
                })}
            </span>
        })
    };
    view! {
        {heading.map(|h| view! {
            <li role="presentation" class="px-2 py-1.5 text-xs font-medium text-muted-foreground">{h}</li>
        })}
        <li
            id=id
            role="option"
            aria-selected=move || if selected() { "true" } else { "false" }
            on:mousemove=move |_| active.set(index)
            on:click=move |_| on_select.run(item.clone())
            class=move || cn(&[
                "flex cursor-pointer select-none items-center rounded-sm px-2 py-1.5 text-sm max-[47.5rem]:min-h-11",
                if selected() { "bg-accent text-accent-foreground" } else { "" },
            ])
        >
            <span class="min-w-0">{label}</span>
            {hint}
        </li>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering() {
        let i = CommandItem {
            id: "1".into(),
            label: "The Matrix".into(),
            group: "g".into(),
            keywords: "1999 wachowski".into(),
        };
        assert!(matches(&i, "matrix"));
        assert!(matches(&i, "WACHOW"));
        assert!(matches(&i, ""));
        assert!(!matches(&i, "alien"));
    }
}
