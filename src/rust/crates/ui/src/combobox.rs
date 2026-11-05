//! Single-select combobox: text input with a popup listbox (WAI-ARIA combobox pattern).

use leptos::prelude::*;

use crate::class::{cn, slug};
use crate::form::SelectOption;

#[component]
pub fn Combobox(
    #[prop(into)] id: String,
    #[prop(into)] label: String,
    #[prop(into)] options: Signal<Vec<SelectOption>>,
    /// Selected option value.
    selected: RwSignal<Option<String>>,
    #[prop(into)] empty_label: String,
    #[prop(into, optional)] placeholder: MaybeProp<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let empty_label = StoredValue::new(empty_label);
    let open = RwSignal::new(false);
    let text = RwSignal::new(String::new());
    let active = RwSignal::new(0usize);
    let list_id = format!("{id}-list");
    let base = id.clone();
    let filtered = Memo::new(move |_| {
        let q = text.get().to_lowercase();
        options
            .get()
            .into_iter()
            .filter(|o| o.label.to_lowercase().contains(&q))
            .collect::<Vec<_>>()
    });
    // show the selected label in the input
    Effect::new(move |_| {
        let sel = selected.get();
        if let Some(o) = options
            .get_untracked()
            .into_iter()
            .find(|o| Some(&o.value) == sel.as_ref())
        {
            text.set(o.label);
        }
    });
    let choose = move |o: SelectOption| {
        text.set(o.label.clone());
        selected.set(Some(o.value));
        open.set(false);
    };
    let on_key = move |ev: leptos::ev::KeyboardEvent| {
        let n = filtered.with_untracked(Vec::len);
        match ev.key().as_str() {
            "ArrowDown" => {
                ev.prevent_default();
                if open.get_untracked() && n > 0 {
                    active.update(|a| *a = (*a + 1) % n);
                } else {
                    open.set(true);
                }
            }
            "ArrowUp" if n > 0 => {
                ev.prevent_default();
                active.update(|a| *a = (*a + n - 1) % n);
            }
            "Enter" if open.get_untracked() => {
                if let Some(o) = filtered.with_untracked(|f| f.get(active.get_untracked()).cloned())
                {
                    ev.prevent_default();
                    choose(o);
                }
            }
            "Escape" => open.set(false),
            _ => {}
        }
    };
    let opt_id = move |v: &str| format!("{base}-opt-{}", slug(v));
    let opt_id = StoredValue::new(opt_id);
    view! {
        <div class=move || cn(&["relative", &class.get().unwrap_or_default()])>
            <input
                id=id.clone()
                type="text"
                role="combobox"
                aria-label=label
                aria-expanded=move || if open.get() { "true" } else { "false" }
                aria-controls=list_id.clone()
                aria-autocomplete="list"
                aria-activedescendant=move || {
                    open.get().then(|| filtered.with(|f| f.get(active.get()).map(|o| opt_id.with_value(|g| g(&o.value))))).flatten()
                }
                placeholder=move || placeholder.get()
                autocomplete="off"
                prop:value=move || text.get()
                on:input=move |ev| {
                    text.set(event_target_value(&ev));
                    active.set(0);
                    open.set(true);
                }
                on:keydown=on_key
                on:blur=move |_| open.set(false)
                class="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            <Show when=move || open.get()>
                <ul id=list_id.clone() role="listbox" class="absolute z-50 mt-1 max-h-60 w-full overflow-auto rounded-md border bg-popover p-1 shadow-md">
                    <Show when=move || filtered.with(Vec::is_empty)>
                        <li role="presentation" class="px-2 py-1.5 text-sm text-muted-foreground">{empty_label.get_value()}</li>
                    </Show>
                    {move || filtered.get().into_iter().enumerate().map(|(i, o)| {
                        let chosen = o.clone();
                        let is_sel = selected.get().as_deref() == Some(o.value.as_str());
                        view! {
                            <li
                                id=opt_id.with_value(|g| g(&o.value))
                                role="option"
                                aria-selected=if is_sel { "true" } else { "false" }
                                // mousedown fires before the input's blur
                                on:mousedown=move |ev| {
                                    ev.prevent_default();
                                    choose(chosen.clone());
                                }
                                class=move || cn(&[
                                    "cursor-pointer rounded-sm px-2 py-1.5 text-sm",
                                    if active.get() == i { "bg-accent text-accent-foreground" } else { "" },
                                ])
                            >
                                {o.label.clone()}
                            </li>
                        }
                    }).collect_view()}
                </ul>
            </Show>
        </div>
    }
}
