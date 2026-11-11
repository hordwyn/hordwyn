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
}

/// Browser entry point: read the server's config, then hydrate the server-rendered page.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
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
}
