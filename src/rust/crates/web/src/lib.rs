//! Leptos web application: routes, pages and domain widgets (features `ssr`, `hydrate`,
//! `csr`). Generic components come from `ui`; data comes from server functions backed by
//! [`backend::Backend`], implemented by the server binary.
#![deny(unsafe_code)]
// Deeply nested view types exceed the default query depth in release wasm builds.
#![recursion_limit = "256"]
// Leptos `#[component]` props are owned by design. Components return views that are
// always used; `must_use` on them adds nothing. Long functions are allowed one by one,
// each with its reason (DEC-234).
#![allow(clippy::needless_pass_by_value, clippy::must_use_candidate)]

mod about;
mod account;
mod admin;
pub mod admin_api;
#[cfg(feature = "ssr")]
pub mod admin_backend;
pub mod admin_state;
pub mod api;
mod app;
mod auth;
mod auth_pages;
#[cfg(feature = "ssr")]
pub mod auth_redirect;
#[cfg(feature = "ssr")]
pub mod backend;
mod bulk;
pub mod charts;
mod episode_list;
pub mod episode_plan;
mod events;
pub mod funnel;
#[cfg(feature = "gallery")]
mod gallery;
mod game_groups;
mod history;
pub mod history_state;
mod home;
pub mod i18n;
pub mod imports;
mod integrations;
mod item;
pub mod item_words;
pub mod keyboard;
mod library;
pub mod library_state;
mod list_drag;
pub mod list_share;
mod lists;
pub mod log_form;
pub mod log_sheet;
pub mod manual_item;
pub mod next_step;
mod notifications;
mod notify_channels;
pub mod notify_rules;
mod notify_rules_view;
mod nudge;
pub mod offline;
mod pages;
mod palette;
mod people;
mod people_dialogs;
mod plays_tab;
mod reauth;
pub mod recent;
mod rest;
mod settings;
mod setup;
pub mod setup_state;
mod sharing;
mod shell;
mod sign_in;
mod sign_out;
pub mod source_hints;
mod stats_extras;
mod time_tiles;
pub mod tint;
pub mod widgets;

pub use app::{App, CONFIG_ID, ClientConfig, shell};

/// Contexts every page needs (provided by the shell on the server, by `hydrate` in the
/// browser): messages and the search palette's open state.
pub fn provide_page_context(cfg: &ClientConfig) {
    use leptos::prelude::*;
    provide_context(cfg.i18n());
    provide_context(cfg.clone());
    provide_context(palette::PaletteOpen(RwSignal::new(false)));
    provide_context(palette::PaletteScope(RwSignal::new(None)));
    provide_context(manual_item::ManualItem::new());
}

/// Browser entry point: read the server's config, then hydrate the server-rendered page.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    hydration::watch();
    let cfg = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(CONFIG_ID))
        .and_then(|e| e.text_content())
        .and_then(|json| serde_json::from_str::<ClientConfig>(&json).ok())
        .unwrap_or_default();
    leptos::mount::hydrate_body(move || {
        provide_page_context(&cfg);
        leptos::prelude::view! { <App/> }
    });
    hydration::done();
}

/// What a page says about its own start. Hydration walks the server's HTML once, top down;
/// a difference between that HTML and the browser's render panics in the middle of the walk,
/// and everything below that place stays without its handlers for good.
#[cfg(feature = "hydrate")]
mod hydration {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};

    use wasm_bindgen::JsCast;

    /// Set on `<body>` when the walk went through: the page reacts (the browser tests wait
    /// for it). Never set by an effect: effects queued before a panic still run after it.
    const DONE: &str = "data-hydrated";
    /// Set on `<body>` when the walk panicked.
    const FAILED: &str = "data-hydration-failed";
    /// `sessionStorage` key: this tab reloaded once for a failed hydration.
    const RETRY_KEY: &str = "app-hydration-retry";
    /// `sessionStorage` key: what was typed into the failed page, for the reloaded one.
    const DRAFTS_KEY: &str = "app-hydration-drafts";
    /// Input types whose value is not a person's typed text (or must not be kept).
    const NOT_TEXT: [&str; 8] = [
        "password", "hidden", "checkbox", "radio", "file", "submit", "button", "reset",
    ];

    static HYDRATING: AtomicBool = AtomicBool::new(false);

    fn document() -> Option<web_sys::Document> {
        web_sys::window()?.document()
    }

    fn body() -> Option<web_sys::HtmlElement> {
        document()?.body()
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.session_storage().ok()?
    }

    /// Before hydration: print panics to the console, and treat one during hydration as a
    /// page that has to be loaded again.
    pub fn watch() {
        HYDRATING.store(true, Ordering::Relaxed);
        std::panic::set_hook(Box::new(|info| {
            console_error_panic_hook::hook(info);
            if HYDRATING.swap(false, Ordering::Relaxed) {
                failed();
            }
        }));
    }

    /// Hydration went through. After the one reload of a failed start: the text typed into
    /// the failed page goes back into its fields, and the second reading of the page is
    /// announced with its reason (RACE-13).
    pub fn done() {
        HYDRATING.store(false, Ordering::Relaxed);
        let reloaded = storage().is_some_and(|s| {
            let was = s.get_item(RETRY_KEY).ok().flatten().is_some();
            let _ = s.remove_item(RETRY_KEY);
            was
        });
        let Some(b) = body() else { return };
        let _ = b.set_attribute(DONE, "1");
        if reloaded {
            // after the effects that hydration queued: they set each field from its signal
            // (`prop:value`) and would overwrite the text put back now
            leptos::prelude::set_timeout(restore_drafts, std::time::Duration::ZERO);
            announce_reload(&b);
        }
    }

    /// Hydration panicked: mark the page and load it again, once per tab (the mismatch came
    /// from one answer of the server, the next answer is rendered anew), keeping what was
    /// typed. A second failure in a row, or a tab without `sessionStorage`, does not reload
    /// (that could loop): it says so at the top of the page, with a link that loads it again
    /// (RACE-4).
    fn failed() {
        let Some(b) = body() else { return };
        let _ = b.set_attribute(FAILED, "1");
        let reloads = storage().filter(|s| {
            s.get_item(RETRY_KEY).ok().flatten().is_none() && s.set_item(RETRY_KEY, "1").is_ok()
        });
        match (reloads, web_sys::window()) {
            (Some(s), Some(w)) => {
                save_drafts(&s);
                let _ = w.location().reload();
            }
            _ => show_notice(&b),
        }
    }

    /// The typed text of the page's fields with an id (not passwords), by id.
    fn save_drafts(s: &web_sys::Storage) {
        let Some(list) =
            document().and_then(|d| d.query_selector_all("input[id], textarea[id]").ok())
        else {
            return;
        };
        let mut drafts = BTreeMap::new();
        for node in (0..list.length()).filter_map(|i| list.item(i)) {
            if let Some(input) = node.dyn_ref::<web_sys::HtmlInputElement>() {
                let typed = input.value();
                if !NOT_TEXT.contains(&input.type_().as_str())
                    && !typed.is_empty()
                    && typed != input.default_value()
                {
                    drafts.insert(input.id(), typed);
                }
            } else if let Some(area) = node.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                let typed = area.value();
                if !typed.is_empty() && area.default_value().is_ok_and(|d| d != typed) {
                    drafts.insert(area.id(), typed);
                }
            }
        }
        if let Ok(json) = serde_json::to_string(&drafts)
            && !drafts.is_empty()
        {
            let _ = s.set_item(DRAFTS_KEY, &json);
        }
    }

    /// Put the saved text back into the fields that are still empty, as typing would (an
    /// `input` event, so the page's state follows).
    fn restore_drafts() {
        let Some(s) = storage() else { return };
        let json = s.get_item(DRAFTS_KEY).ok().flatten();
        let _ = s.remove_item(DRAFTS_KEY);
        let Some(drafts) =
            json.and_then(|j| serde_json::from_str::<BTreeMap<String, String>>(&j).ok())
        else {
            return;
        };
        let Some(doc) = document() else { return };
        for (id, typed) in drafts {
            let Some(el) = doc.get_element_by_id(&id) else {
                continue;
            };
            if let Some(input) = el.dyn_ref::<web_sys::HtmlInputElement>() {
                if !input.value().is_empty() {
                    continue;
                }
                input.set_value(&typed);
            } else if let Some(area) = el.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                if !area.value().is_empty() {
                    continue;
                }
                area.set_value(&typed);
            } else {
                continue;
            }
            let init = web_sys::EventInit::new();
            init.set_bubbles(true);
            if let Ok(ev) = web_sys::Event::new_with_event_init_dict("input", &init) {
                let _ = el.dispatch_event(&ev);
            }
        }
    }

    /// A polite status line for screen readers: it is filled a moment after it is in the
    /// page, so that it is read out.
    fn announce_reload(b: &web_sys::HtmlElement) {
        let (Some(doc), Some(text)) = (document(), b.get_attribute("data-start-reloaded")) else {
            return;
        };
        let Ok(line) = doc.create_element("div") else {
            return;
        };
        let _ = line.set_attribute("role", "status");
        let _ = line.set_attribute("class", "sr-only");
        if b.append_child(&line).is_ok() {
            leptos::prelude::set_timeout(
                move || line.set_text_content(Some(&text)),
                std::time::Duration::from_millis(300),
            );
        }
    }

    /// The notice of a page that did not start: a sentence and a link that loads the page
    /// again. Plain DOM and a plain link, no handler: the app's code is not running.
    fn show_notice(b: &web_sys::HtmlElement) {
        let Some(doc) = document() else { return };
        let text = b
            .get_attribute("data-start-failed")
            .unwrap_or_else(|| "This page did not start properly.".to_string());
        let label = b
            .get_attribute("data-start-reload")
            .unwrap_or_else(|| "Reload".to_string());
        let here = web_sys::window()
            .and_then(|w| {
                let l = w.location();
                Some(format!("{}{}", l.pathname().ok()?, l.search().ok()?))
            })
            .unwrap_or_else(|| "/".to_string());
        let (Ok(note), Ok(sentence), Ok(link)) = (
            doc.create_element("div"),
            doc.create_element("p"),
            doc.create_element("a"),
        ) else {
            return;
        };
        let _ = note.set_attribute("role", "alert");
        let _ = note.set_attribute(
            "class",
            "m-4 flex flex-wrap items-center gap-3 rounded-md border border-destructive bg-background p-4 text-sm text-foreground",
        );
        sentence.set_text_content(Some(&text));
        let _ = link.set_attribute("href", &here);
        // a full load: the router (when it still runs) leaves an external link alone
        let _ = link.set_attribute("rel", "external");
        let _ = link.set_attribute(
            "class",
            &ui::link_classes(ui::ButtonVariant::Outline, ui::ButtonSize::Md),
        );
        link.set_text_content(Some(&label));
        let _ = note.append_child(&sentence);
        let _ = note.append_child(&link);
        let _ = b.prepend_with_node_1(&note);
    }
}
