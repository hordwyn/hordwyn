//! Command palette list: a combobox input filtering a listbox, arrow keys move the active
//! option (`aria-activedescendant`), Enter selects, Ctrl/Cmd+Enter runs the secondary action
//! (design.md 6.4: add as Planned or the next step). Used for the Ctrl-K search palette.
//!
//! The active option is remembered by its item, not by its position: a list that grows while
//! results stream in keeps the row the person chose. It is marked by an outline in the ring
//! colour (the background tint alone is below 3:1, WCAG 1.4.11) and scrolled into view when
//! the keys move it, and when the list is replaced under it.

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
    /// What leads the row; `None` for a row of text only.
    pub leading: Option<CommandLeading>,
}

/// The picture that leads a row (OT-2): an icon in a muted box, with a small cover over it when
/// there is one. The box has one size per shape and the slot one width, so labels line up; the
/// icon shows while the cover loads and when it fails. Decoration only: the label names the row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandLeading {
    pub icon: IconName,
    /// URL of a small cover.
    pub image: Option<String>,
    /// Square art (albums, board game boxes) instead of a 2:3 poster.
    pub square: bool,
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

/// The active option: the item chosen by key or pointer (`None`: the first row), where it
/// is in the list on screen and which item that is.
#[derive(Clone, Copy)]
struct Active {
    item: RwSignal<Option<String>>,
    index: Memo<usize>,
    id: Memo<Option<String>>,
}

/// What makes a row the same row from one list to the next (OT1B-8): its item, what it shows
/// and whether its group's heading stands before it. A row whose key stays keeps its
/// elements, so its cover is not loaded and painted again for every batch of results.
type RowKey = (String, String, Option<String>, Option<CommandLeading>);

fn row_key((item, heading): &(CommandItem, Option<String>)) -> RowKey {
    (
        item.id.clone(),
        item.label.clone(),
        heading.clone(),
        item.leading.clone(),
    )
}

/// The rows on screen with their headings: a heading stands before the first row of each run
/// of one group.
fn rows(items: &[CommandItem]) -> Vec<(CommandItem, Option<String>)> {
    let mut last_group: Option<&str> = None;
    items
        .iter()
        .map(|item| {
            let heading = (last_group != Some(item.group.as_str())).then(|| item.group.clone());
            last_group = Some(item.group.as_str());
            (item.clone(), heading)
        })
        .collect()
}

/// The element id of an item's option.
type OptionId = Box<dyn Fn(&str) -> String + Send + Sync>;

/// Bring the option into view inside whatever scrolls (the list on desktop, the dialog on
/// phones). The first row takes its group heading with it.
fn reveal(list_id: &str, option_id: &str, first: bool) {
    let doc = document();
    if first && let Some(list) = doc.get_element_by_id(list_id) {
        list.set_scroll_top(0);
    }
    if let Some(option) = doc.get_element_by_id(option_id) {
        let how = web_sys::ScrollIntoViewOptions::new();
        how.set_block(web_sys::ScrollLogicalPosition::Nearest);
        option.scroll_into_view_with_scroll_into_view_options(&how);
    }
}

/// When the list changes under the chosen item (the caller replaced it: the same text in
/// another scope) the item can land out of view: bring it back once the rows are drawn
/// (OT1A-8). A move by key or pointer changes the item and is not followed here: the keys
/// reveal on their own and a pointer is on its row. Rows that change elsewhere in the list
/// leave the position alone, so nothing scrolls.
fn follow_chosen(
    filtered: Memo<Vec<CommandItem>>,
    active: Active,
    list_id: String,
    option_id: StoredValue<OptionId>,
) {
    Effect::new(move |before: Option<(Option<String>, usize)>| {
        let now = (active.item.get(), active.index.get());
        if let Some((item, at)) = before
            && item == now.0
            && at != now.1
            && let Some(id) = filtered.with_untracked(|f| f.get(now.1).map(|i| i.id.clone()))
        {
            let option = option_id.with_value(|o| o(&id));
            let (list, first) = (list_id.clone(), now.1 == 0);
            request_animation_frame(move || reveal(&list, &option, first));
        }
        now
    });
}

/// Arrow keys move the active option (wrapping) and scroll it into view, Enter selects it,
/// Ctrl/Cmd+Enter runs the secondary action (Enter's when there is none).
fn on_key(
    ev: &leptos::ev::KeyboardEvent,
    filtered: Memo<Vec<CommandItem>>,
    active: Active,
    (list_id, option_id): (&str, StoredValue<OptionId>),
    (on_select, on_secondary): (Callback<CommandItem>, Option<Callback<CommandItem>>),
) {
    let n = filtered.with_untracked(Vec::len);
    let at = active.index.get_untracked();
    let go = |to: usize| {
        let Some(item) = filtered.with_untracked(|f| f.get(to).map(|i| i.id.clone())) else {
            return;
        };
        let option = option_id.with_value(|o| o(&item));
        active.item.set(Some(item));
        reveal(list_id, &option, to == 0);
        // and once more when the row has its hints: they can make it higher
        let list = list_id.to_string();
        request_animation_frame(move || reveal(&list, &option, to == 0));
    };
    match ev.key().as_str() {
        "ArrowDown" if n > 0 => {
            ev.prevent_default();
            go((at + 1) % n);
        }
        "ArrowUp" if n > 0 => {
            ev.prevent_default();
            go((at + n - 1) % n);
        }
        "Enter" => {
            if let Some(item) = filtered.with_untracked(|f| f.get(at).cloned()) {
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

/// `query` is exposed so callers can fetch results as the user types; the given `items` are
/// filtered by it again unless `filter` is off.
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
    /// Filter `items` by the typed text (label and keywords; the default). Off for items the
    /// caller already matched, such as server results: a typo-tolerant hit does not hold the
    /// typed text.
    #[prop(into, optional)]
    filter: Option<Signal<bool>>,
    /// Told which item is active (the first row while nothing is chosen), for a caller that
    /// adds rows while the list is open and must not move that row (OT1B-5).
    #[prop(optional)]
    active_item: Option<RwSignal<Option<String>>>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let chosen = RwSignal::new(None::<String>);
    let list_id = format!("{id}-list");
    let hints_id = format!("{id}-hints");
    let described = hints.as_ref().map(|_| hints_id.clone());
    let hints = StoredValue::new(hints);
    let filtered = Memo::new(move |_| {
        let q = query.get();
        let all = items.get();
        if filter.is_some_and(|f| !f.get()) {
            return all;
        }
        all.into_iter()
            .filter(|i| matches(i, &q))
            .collect::<Vec<_>>()
    });
    // a new query starts at the first row; a list that only changes keeps the chosen item
    Effect::new(move |_| {
        query.track();
        chosen.set(None);
    });
    let index = Memo::new(move |_| {
        let item = chosen.get();
        filtered.with(|f| {
            item.and_then(|id| f.iter().position(|i| i.id == id))
                .unwrap_or(0)
        })
    });
    let active_id = Memo::new(move |_| filtered.with(|f| f.get(index.get()).map(|i| i.id.clone())));
    let active = Active {
        item: chosen,
        index,
        id: active_id,
    };
    if let Some(out) = active_item {
        Effect::new(move |_| out.set(active_id.get()));
    }
    let prefix = id.clone();
    let option_id: StoredValue<OptionId> = StoredValue::new(Box::new(move |item_id: &str| {
        format!("{prefix}-opt-{}", slug(item_id))
    }));
    let keys_list = list_id.clone();
    follow_chosen(filtered, active, list_id.clone(), option_id);
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
                        filtered.with(|f| f.get(index.get()).map(|i| option_id.with_value(|o| o(&i.id))))
                    }
                    placeholder=placeholder
                    autocomplete="off"
                    prop:value=move || query.get()
                    on:input=move |ev| query.set(event_target_value(&ev))
                    on:keydown=move |ev| on_key(&ev, filtered, active, (&keys_list, option_id), (on_select, on_secondary))
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
                <For
                    each=move || filtered.with(|f| rows(f))
                    key=row_key
                    children=move |(item, heading): (CommandItem, Option<String>)| {
                        let id = option_id.with_value(|o| o(&item.id));
                        let hints = hints.get_value();
                        view! { <CommandOption item=item id=id heading=heading active=active on_select=on_select hints=hints/> }
                    }
                />
            </ul>
        </div>
    }
}

/// One option, with its group heading before it when the group changes. Rows are at least
/// 44 px high on phones (WCAG 2.5.8, design.md 6.4). The active row shows the key hints
/// (hidden from screen readers: the input's description says them) from 40 rem up: below
/// that they lay over a title that wraps, and a phone has no such keys (v1-polish P4).
#[component]
fn CommandOption(
    item: CommandItem,
    id: String,
    heading: Option<String>,
    active: Active,
    on_select: Callback<CommandItem>,
    hints: Option<CommandHints>,
) -> impl IntoView {
    // by item, not by position: the row keeps its elements while rows are added around it
    let own = item.id.clone();
    let selected = Memo::new(move |_| active.id.with(|a| a.as_deref() == Some(own.as_str())));
    let selected = move || selected.get();
    let label = item.label.clone();
    let hovered = item.id.clone();
    let item_leading = item.leading.clone();
    let hint = move || {
        let h = hints.clone().filter(|_| selected())?;
        let second = h.secondary.get();
        Some(view! {
            <span aria-hidden="true" data-hints="" class="ml-auto hidden shrink-0 flex-wrap items-center justify-end gap-x-3 gap-y-1 pl-3 sm:flex">
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
            on:mousemove=move |_| active.item.set(Some(hovered.clone()))
            on:click=move |_| on_select.run(item.clone())
            class=move || cn(&[
                "flex cursor-pointer select-none items-center rounded-sm px-2 py-1.5 text-sm max-[47.5rem]:min-h-11",
                if selected() { "bg-accent text-accent-foreground outline-2 -outline-offset-2 outline-ring" } else { "" },
            ])
        >
            {item_leading.map(|l| view! { <Leading leading=l/> })}
            <span class="min-w-0">{label}</span>
            {hint}
        </li>
    }
}

/// The leading slot of an option: 2.5 rem wide whatever the shape of the box in it. A picture
/// that does not load is taken away, so the icon under it shows and nothing else: no broken
/// picture mark and no text (the picture has no alternative text, the label names the row).
/// The row is kept while the list changes around it: a picture that loads after a failure
/// (another address for the same row) is shown again.
#[component]
fn Leading(leading: CommandLeading) -> impl IntoView {
    let shape = if leading.square { "w-10" } else { "w-7" };
    let failed = RwSignal::new(false);
    view! {
        <span data-leading="" aria-hidden="true" class="mr-3 flex w-10 shrink-0 justify-center">
            <span class=cn(&["relative flex h-10 items-center justify-center overflow-hidden rounded-sm bg-muted text-muted-foreground", shape])>
                <Icon name=leading.icon/>
                {leading.image.map(|src| view! {
                    <img src=src alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer"
                        on:error=move |_| failed.set(true) on:load=move |_| failed.set(false)
                        class:hidden=move || failed.get()
                        class="absolute inset-0 h-full w-full object-cover"/>
                })}
            </span>
        </span>
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
            leading: None,
        };
        assert!(matches(&i, "matrix"));
        assert!(matches(&i, "WACHOW"));
        assert!(matches(&i, ""));
        assert!(!matches(&i, "alien"));
    }

    /// OT1B-7 seen from the list: a heading stands before every run of one group, so a
    /// caller that keeps a group together gets one heading for it.
    #[test]
    fn a_heading_leads_each_run_of_a_group() {
        let item = |id: &str, group: &str| CommandItem {
            id: id.into(),
            label: id.into(),
            group: group.into(),
            keywords: String::new(),
            leading: None,
        };
        let list = [
            item("a", "g1"),
            item("b", "g1"),
            item("c", "g2"),
            item("d", "g1"),
        ];
        let headings: Vec<Option<String>> = rows(&list).into_iter().map(|(_, h)| h).collect();
        assert_eq!(
            headings,
            [
                Some("g1".to_string()),
                None,
                Some("g2".to_string()),
                Some("g1".to_string())
            ]
        );
    }
}
