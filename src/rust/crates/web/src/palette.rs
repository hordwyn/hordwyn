//! Ctrl-K search palette (D-6.6): local results first, then each connector's results as its
//! `search.results` event arrives for the current search id.

use contracts::{CatalogItemDto, SearchResultsEventDto};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use ui::{Command, CommandItem, Dialog};

use crate::events::use_events;
use crate::i18n::{t, use_i18n};

/// Whether the palette is open (shared with the header button).
#[derive(Clone, Copy)]
pub struct PaletteOpen(pub RwSignal<bool>);

fn to_item(i: &CatalogItemDto, group: &str) -> CommandItem {
    let kind = crate::i18n::kind_label(&i.kind);
    CommandItem {
        id: i.id.clone(),
        label: match i.year {
            Some(y) => format!("{} ({y})", i.title),
            None => i.title.clone(),
        },
        group: format!("{group} - {kind}"),
        keywords: i.original_title.clone().unwrap_or_default(),
    }
}

#[component]
pub fn SearchPalette() -> impl IntoView {
    let open = expect_context::<PaletteOpen>().0;
    let i18n = use_i18n();
    let query = RwSignal::new(String::new());
    let items = RwSignal::new(Vec::<CommandItem>::new());
    let search_id = RwSignal::new(None::<String>);
    let waiting = RwSignal::new(Vec::<String>::new());
    let events = use_events();

    // Ctrl-K / Cmd-K anywhere opens the palette.
    let _ = window_event_listener(leptos::ev::keydown, move |ev| {
        if (ev.ctrl_key() || ev.meta_key()) && ev.key().eq_ignore_ascii_case("k") {
            ev.prevent_default();
            open.set(true);
        }
    });

    // Debounced search on input.
    let generation = StoredValue::new(0u64);
    Effect::new(move |_| {
        let q = query.get();
        let g = generation.get_value() + 1;
        generation.set_value(g);
        if q.trim().chars().count() < 2 {
            items.set(vec![]);
            search_id.set(None);
            waiting.set(vec![]);
            return;
        }
        set_timeout(
            move || {
                if generation.get_value() != g {
                    return;
                }
                leptos::task::spawn_local(async move {
                    if let Ok(r) = crate::api::search(q, None).await
                        && generation.get_value() == g
                    {
                        let local = t("search-local");
                        items.set(r.items.iter().map(|i| to_item(i, &local)).collect());
                        waiting.set(r.pending);
                        search_id.set(Some(r.search_id));
                    }
                });
            },
            std::time::Duration::from_millis(250),
        );
    });

    // Streamed connector results.
    Effect::new(move |_| {
        let Some((topic, data)) = events.0.get() else {
            return;
        };
        if topic != "search.results" {
            return;
        }
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(&data) else {
            return;
        };
        let Ok(r) = serde_json::from_value::<SearchResultsEventDto>(ev["payload"].clone()) else {
            return;
        };
        if search_id.get_untracked().as_deref() != Some(r.search_id.as_str()) {
            return;
        }
        waiting.update(|w| w.retain(|c| c != &r.connector));
        items.update(|list| {
            for i in &r.items {
                if !list.iter().any(|x| x.id == i.id) {
                    list.push(to_item(i, &r.connector));
                }
            }
        });
    });

    let navigate = use_navigate();
    let on_select = Callback::new(move |item: CommandItem| {
        open.set(false);
        query.set(String::new());
        navigate(
            &format!("/items/{}", item.id),
            leptos_router::NavigateOptions::default(),
        );
    });
    let waiting_text = {
        let i18n = i18n.clone();
        Signal::derive(move || {
            let w = waiting.get();
            if w.is_empty() {
                String::new()
            } else {
                i18n.t_args("search-waiting", &[("sources", w.join(", ").into())])
            }
        })
    };
    let (label, placeholder, empty) = (
        i18n.t("search-label"),
        i18n.t("search-placeholder"),
        i18n.t("search-empty"),
    );
    view! {
        <Dialog open=open title=i18n.t("search-title") close_label=i18n.t("dialog-close")>
            <Command
                id="search"
                label=label.clone()
                placeholder=placeholder.clone()
                empty_label=empty.clone()
                items=Signal::derive(move || items.get())
                query=query
                on_select=on_select
            />
            <p class="mt-2 text-xs text-muted-foreground" aria-live="polite">{waiting_text}</p>
        </Dialog>
    }
}
