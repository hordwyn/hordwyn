//! Static generator for the project website.
//!
//! The pages are Leptos views rendered once on the build machine (no hydration, no
//! WebAssembly): they use the components of the `ui` crate and the tokens of the brand
//! directory, so the website and the application share one design basis. All text, the
//! product name included, lives in the content directory (`content/`), never in this code.
#![deny(unsafe_code)]
// Leptos `#[component]` props are owned by design, and a component is a view that is always
// used; `must_use` on it adds nothing.
#![allow(clippy::needless_pass_by_value, clippy::must_use_candidate)]

pub mod content;
pub mod gate;
pub mod generate;
pub mod layout;
pub mod markdown;
pub mod pages;

use std::path::PathBuf;

/// Directory of this crate in the checkout (the defaults of the command line start here).
#[must_use]
pub fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Default locations in the checkout: content, default brand, theme script.
#[must_use]
pub fn default_options(css: PathBuf, out: PathBuf) -> generate::Options {
    let dir = crate_dir();
    generate::Options {
        content: dir.join("content"),
        brand: dir.join("../../../../config/brand/default"),
        assets: dir.join("../../../web/site"),
        licenses: dir.join("../../crates/ui/icons/vendor"),
        css,
        out,
    }
}
