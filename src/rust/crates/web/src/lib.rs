//! Leptos web application: routes, pages and domain widgets.
#![deny(unsafe_code)]

/// Name of this crate, used by the bootstrap smoke test.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "web");
    }
}
