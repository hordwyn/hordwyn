//! Pages. Data comes from server functions; `entity.updated` events refetch (no polling).

use leptos::prelude::*;
use leptos_meta::Title;

use crate::api;
pub use crate::auth_pages::SignUp;
use crate::i18n::t;
pub use crate::sign_in::SignIn;

/// Full page load after sign-in/out so the server renders with the new cookie.
pub(crate) fn go(path: &str) {
    #[cfg(feature = "hydrate")]
    if let Some(w) = web_sys::window() {
        let _ = w.location().assign(path);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = path;
}

/// Reload the current page from the server (after a change the server renders, such as the
/// language).
pub(crate) fn reload() {
    #[cfg(feature = "hydrate")]
    if let Some(w) = web_sys::window() {
        let _ = w.location().reload();
    }
}

pub(crate) fn error_text(e: &ServerFnError) -> String {
    let code = api::error_code(e);
    let key = format!("auth-error-{code}");
    let text = t(&key);
    if text == key {
        t("error-generic")
    } else {
        text
    }
}

#[component]
pub(crate) fn PageTitle(#[prop(into)] text: String) -> impl IntoView {
    let product = expect_context::<crate::app::ClientConfig>().product_name;
    view! {
        <Title text=format!("{text} - {product}")/>
        <h1 class="mb-6 text-2xl font-semibold tracking-tight">{text}</h1>
    }
}

/// Redirect to sign-in when a server function says the caller is not signed in.
pub(crate) fn needs_sign_in(e: &ServerFnError) -> bool {
    api::error_code(e) == "unauthorized"
}

/// The server refused the query as malformed (400) or invalid (422). Filters come from a
/// URL that may be old or hand-edited, so the pages show "no match" with "Clear filters"
/// instead of an error whose Retry cannot help (UX1-7).
pub(crate) fn bad_query(e: &ServerFnError) -> bool {
    matches!(api::error_code(e).as_str(), "bad_request" | "invalid")
}

/// A key that changes on every call. The data pages render each new value of a resource
/// afresh, `<For each=move || data.get().map(|d| (fresh(n), d)) key=|(k, _)| *k ...>`,
/// instead of rebuilding the previous render in place, which kept event handlers whose
/// state was already disposed (found on the item page; UX1-2 applies it to every list).
pub(crate) fn fresh(counter: StoredValue<u32>) -> u32 {
    counter.update_value(|v| *v = v.wrapping_add(1));
    counter.get_value()
}

/// A value computed from other reactive values: a memo in the browser, on the server a
/// derived signal that is computed on every read.
///
/// The rule (LR-1): on the server no memo reads a resource. The server renders on several
/// threads; a memo that caches a resource read there can miss the resource's answer when it
/// arrives on another thread while the memo computes, and stays at "not loaded" for good. The
/// page is then sent without a part that the browser renders from the same resource, and
/// hydration panics (the Library's status tabs, race diagnosis 2.2; earlier cases of the same
/// family: DEC-398, DEC-456). The browser has one thread and keeps its memos, so nothing is
/// rendered more often there. `tests/server_memos.rs` holds the rule.
pub(crate) fn derived<T>(f: impl Fn() -> T + Send + Sync + 'static) -> Signal<T>
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    #[cfg(feature = "hydrate")]
    let value = Memo::new(move |_| f()).into();
    #[cfg(not(feature = "hydrate"))]
    let value = Signal::derive(f);
    value
}
