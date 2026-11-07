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

pub mod api;
mod app;
mod auth;
mod auth_pages;
#[cfg(feature = "ssr")]
pub mod backend;
pub mod charts;
mod episode_list;
pub mod episode_plan;
mod events;
#[cfg(feature = "gallery")]
mod gallery;
mod history;
pub mod history_state;
mod home;
pub mod i18n;
mod integrations;
mod item;
mod library;
pub mod library_state;
pub mod log_form;
pub mod log_sheet;
pub mod next_step;
mod notifications;
mod pages;
mod palette;
mod sharing;
mod shell;
pub mod widgets;

pub use app::{App, CONFIG_ID, ClientConfig, shell};

/// Contexts every page needs (provided by the shell on the server, by `hydrate` in the
/// browser): messages and the search palette's open state.
pub fn provide_page_context(cfg: &ClientConfig) {
    use leptos::prelude::*;
    provide_context(cfg.i18n());
    provide_context(cfg.clone());
    provide_context(palette::PaletteOpen(RwSignal::new(false)));
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
