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
