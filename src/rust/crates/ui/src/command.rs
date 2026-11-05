//! Command palette list: a combobox input filtering a listbox, arrow keys move the active
//! option (`aria-activedescendant`), Enter selects. Used for the Ctrl-K search palette.

use leptos::prelude::*;

use crate::class::{cn, slug};
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

fn matches(item: &CommandItem, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    q.is_empty()
        || item.label.to_lowercase().contains(&q)
        || item.keywords.to_lowercase().contains(&q)
}

/// `query` is exposed so callers can fetch results as the user types; local filtering still
/// applies to the given `items`.
#[component]
pub fn Command(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] placeholder: String,
    #[prop(into)] empty_label: String,
    #[prop(into)] items: Signal<Vec<CommandItem>>,
    query: RwSignal<String>,
    on_select: Callback<CommandItem>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let active = RwSignal::new(0usize);
    let list_id = format!("{id}-list");
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
    let option_id = {
        let id = id.clone();
        move |item_id: &str| format!("{id}-opt-{}", slug(item_id))
    };
    let option_id = StoredValue::new(option_id);
    let on_key = move |ev: leptos::ev::KeyboardEvent| {
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
                if let Some(item) =
                    filtered.with_untracked(|f| f.get(active.get_untracked()).cloned())
                {
                    ev.prevent_default();
                    on_select.run(item);
                }
            }
            _ => {}
        }
    };
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
                    aria-activedescendant=move || {
                        filtered.with(|f| f.get(active.get()).map(|i| option_id.with_value(|o| o(&i.id))))
                    }
                    placeholder=placeholder
                    autocomplete="off"
                    prop:value=move || query.get()
                    on:input=move |ev| query.set(event_target_value(&ev))
                    on:keydown=on_key
                    class="flex h-11 w-full bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground"
                />
            </div>
            <ul id=list_id.clone() role="listbox" class="max-h-80 overflow-y-auto p-1">
                <Show when=move || filtered.with(Vec::is_empty)>
                    <li role="presentation" class="py-6 text-center text-sm text-muted-foreground">{empty_label.clone()}</li>
                </Show>
                {move || {
                    let list = filtered.get();
                    let mut last_group = String::new();
                    list.into_iter().enumerate().map(|(i, item)| {
                        let heading = (item.group != last_group).then(|| item.group.clone());
                        last_group.clone_from(&item.group);
                        let selected = move || active.get() == i;
                        let chosen = item.clone();
                        view! {
                            {heading.map(|h| view! {
                                <li role="presentation" class="px-2 py-1.5 text-xs font-medium text-muted-foreground">{h}</li>
                            })}
                            <li
                                id=option_id.with_value(|o| o(&item.id))
                                role="option"
                                aria-selected=move || if selected() { "true" } else { "false" }
                                on:mousemove=move |_| active.set(i)
                                on:click=move |_| on_select.run(chosen.clone())
                                class=move || cn(&[
                                    "flex cursor-pointer select-none items-center rounded-sm px-2 py-1.5 text-sm",
                                    if selected() { "bg-accent text-accent-foreground" } else { "" },
                                ])
                            >
                                {item.label.clone()}
                            </li>
                        }
                    }).collect_view()
                }}
            </ul>
        </div>
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
