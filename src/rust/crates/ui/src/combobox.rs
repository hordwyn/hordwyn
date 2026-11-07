//! Single-select combobox: text input with a popup listbox (WAI-ARIA combobox pattern). With
//! `on_create` it offers to create what was typed when no option has that name (inline
//! create, design.md 7.3 `CompanionPicker`).

use leptos::prelude::*;

use crate::class::{cn, slug};
use crate::form::SelectOption;

const OPTION: &str = "cursor-pointer rounded-sm px-2 py-1.5 text-sm";
const ACTIVE: &str = "bg-accent text-accent-foreground";

/// The combobox's state, shared by the input and the list.
#[derive(Clone, Copy)]
struct Combo {
    open: RwSignal<bool>,
    text: RwSignal<String>,
    active: RwSignal<usize>,
    selected: RwSignal<Option<String>>,
    filtered: Memo<Vec<SelectOption>>,
    /// The typed text when it can be created: not empty and no option has exactly that name.
    creatable: Memo<Option<String>>,
    clear_on_select: bool,
    on_create: Option<Callback<String>>,
}

impl Combo {
    fn rows(self) -> usize {
        self.filtered.with(Vec::len) + usize::from(self.creatable.with(Option::is_some))
    }

    fn choose(self, o: SelectOption) {
        self.text.set(if self.clear_on_select {
            String::new()
        } else {
            o.label.clone()
        });
        self.selected.set(Some(o.value));
        self.open.set(false);
    }

    fn create(self, t: String) {
        if let Some(cb) = self.on_create {
            self.text.set(String::new());
            self.open.set(false);
            cb.run(t);
        }
    }

    fn key(self, ev: &leptos::ev::KeyboardEvent) {
        let n = self.rows();
        match ev.key().as_str() {
            "ArrowDown" => {
                ev.prevent_default();
                if self.open.get_untracked() && n > 0 {
                    self.active.update(|a| *a = (*a + 1) % n);
                } else {
                    self.open.set(true);
                }
            }
            "ArrowUp" if n > 0 => {
                ev.prevent_default();
                self.active.update(|a| *a = (*a + n - 1) % n);
            }
            "Enter" if self.open.get_untracked() => {
                let at = self.active.get_untracked();
                if let Some(o) = self.filtered.with_untracked(|f| f.get(at).cloned()) {
                    ev.prevent_default();
                    self.choose(o);
                } else if let Some(t) = self.creatable.get_untracked() {
                    ev.prevent_default();
                    self.create(t);
                }
            }
            "Escape" => self.open.set(false),
            _ => {}
        }
    }
}

fn option_id(base: &str, value: &str) -> String {
    format!("{base}-opt-{}", slug(value))
}

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
    /// Empty the text after a choice (a picker that adds the choice elsewhere).
    #[prop(optional)]
    clear_on_select: bool,
    /// Called with the typed text when the person picks the "create" row.
    #[prop(optional, into)]
    on_create: Option<Callback<String>>,
    /// Label of the "create" row for the typed text (for example `Add "Zoe"`).
    #[prop(optional, into)]
    create_label: Option<Callback<String, String>>,
) -> impl IntoView {
    let text = RwSignal::new(String::new());
    let filtered = Memo::new(move |_| {
        let q = text.get().to_lowercase();
        options
            .get()
            .into_iter()
            .filter(|o| o.label.to_lowercase().contains(&q))
            .collect::<Vec<_>>()
    });
    let creatable = Memo::new(move |_| {
        on_create?;
        let t = text.get().trim().to_string();
        let lower = t.to_lowercase();
        let taken = options.with(|o| o.iter().any(|o| o.label.to_lowercase() == lower));
        (!t.is_empty() && !taken).then_some(t)
    });
    let c = Combo {
        open: RwSignal::new(false),
        text,
        active: RwSignal::new(0usize),
        selected,
        filtered,
        creatable,
        clear_on_select,
        on_create,
    };
    // show the selected label in the input
    Effect::new(move |_| {
        let sel = selected.get();
        if let Some(o) = options
            .get_untracked()
            .into_iter()
            .find(|o| Some(&o.value) == sel.as_ref())
            .filter(|_| !clear_on_select)
        {
            text.set(o.label);
        }
    });
    let list_id = format!("{id}-list");
    let base = id.clone();
    let active_id = move || {
        let at = c.active.get();
        filtered
            .with(|f| f.get(at).map(|o| option_id(&base, &o.value)))
            .or_else(|| {
                creatable
                    .with(Option::is_some)
                    .then(|| format!("{base}-create"))
            })
    };
    view! {
        <div class=move || cn(&["relative", &class.get().unwrap_or_default()])>
            <input
                id=id.clone()
                type="text"
                role="combobox"
                aria-label=label
                aria-expanded=move || if c.open.get() { "true" } else { "false" }
                aria-controls=list_id.clone()
                aria-autocomplete="list"
                aria-activedescendant=move || c.open.get().then(active_id.clone()).flatten()
                placeholder=move || placeholder.get()
                autocomplete="off"
                prop:value=move || text.get()
                on:input=move |ev| {
                    text.set(event_target_value(&ev));
                    c.active.set(0);
                    c.open.set(true);
                }
                on:keydown=move |ev| c.key(&ev)
                on:blur=move |_| c.open.set(false)
                class="flex h-9 w-full rounded-md border border-input bg-background px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            <Show when=move || c.open.get()>
                <ComboList c=c list_id=list_id.clone() base=id.clone() empty_label=empty_label.clone() create_label=create_label/>
            </Show>
        </div>
    }
}

/// The popup listbox: matching options, then the "create" row.
#[component]
fn ComboList(
    c: Combo,
    list_id: String,
    base: String,
    empty_label: String,
    create_label: Option<Callback<String, String>>,
) -> impl IntoView {
    let create_id = format!("{base}-create");
    view! {
        <ul id=list_id role="listbox" class="absolute z-50 mt-1 max-h-60 w-full overflow-auto rounded-md border bg-popover p-1 shadow-md">
            <Show when=move || c.rows() == 0>
                <li role="presentation" class="px-2 py-1.5 text-sm text-muted-foreground">{empty_label.clone()}</li>
            </Show>
            {move || c.filtered.get().into_iter().enumerate().map(|(i, o)| {
                let chosen = o.clone();
                let is_sel = c.selected.get().as_deref() == Some(o.value.as_str());
                view! {
                    <li
                        id=option_id(&base, &o.value)
                        role="option"
                        aria-selected=if is_sel { "true" } else { "false" }
                        // mousedown fires before the input's blur
                        on:mousedown=move |ev| {
                            ev.prevent_default();
                            c.choose(chosen.clone());
                        }
                        class=move || cn(&[OPTION, if c.active.get() == i { ACTIVE } else { "" }])
                    >
                        {o.label.clone()}
                    </li>
                }
            }).collect_view()}
            {move || c.creatable.get().map(|t| {
                let shown = create_label.map_or_else(|| t.clone(), |l| l.run(t.clone()));
                let at = c.filtered.with(Vec::len);
                view! {
                    <li
                        id=create_id.clone()
                        role="option"
                        aria-selected="false"
                        on:mousedown=move |ev| {
                            ev.prevent_default();
                            c.create(t.clone());
                        }
                        class=move || cn(&[OPTION, "font-medium", if c.active.get() == at { ACTIVE } else { "" }])
                    >
                        {shown}
                    </li>
                }
            })}
        </ul>
    }
}
