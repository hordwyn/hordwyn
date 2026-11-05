//! Leptos web application: routes, pages and domain widgets (features `ssr`, `hydrate`,
//! `csr`). Generic components come from `ui`.
#![deny(unsafe_code)]
// Leptos `#[component]` props are owned by design, and `view!` bodies are long markup.
#![allow(clippy::needless_pass_by_value, clippy::too_many_lines)]

mod app;
#[cfg(feature = "gallery")]
mod gallery;

pub use app::{App, shell};

/// Browser entry point: hydrate the server-rendered page.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
