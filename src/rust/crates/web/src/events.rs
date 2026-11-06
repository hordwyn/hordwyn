//! Server-Sent Events in the browser: one `EventSource` on `/api/v1/events` (session cookie)
//! for the whole page. Pages react to [`LastEvent`]; nothing ever polls (D-6.1).

use leptos::prelude::*;

/// Most recent event: `(topic, JSON data)`.
#[derive(Clone, Copy)]
pub struct LastEvent(pub RwSignal<Option<(String, String)>>);

/// Topics the page listens to (browser only).
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
pub const TOPICS: [&str; 4] = [
    "entity.updated",
    "search.results",
    "job.progress",
    "import.progress",
];

/// Provide [`LastEvent`] and, in the browser, connect the event stream.
pub fn provide_events() {
    let last = LastEvent(RwSignal::new(None));
    provide_context(last);
    #[cfg(feature = "hydrate")]
    connect(last);
}

#[cfg(feature = "hydrate")]
fn connect(last: LastEvent) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Ok(es) = web_sys::EventSource::new("/api/v1/events") else {
        return;
    };
    for topic in TOPICS {
        let cb =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |ev: web_sys::MessageEvent| {
                let data = ev.data().as_string().unwrap_or_default();
                last.0.set(Some((topic.to_string(), data)));
            });
        let _ = es.add_event_listener_with_callback(topic, cb.as_ref().unchecked_ref());
        // the page lives as long as the event source: keep the handler alive
        cb.forget();
    }
    // keep the connection object alive for the page's lifetime
    std::mem::forget(es);
}

/// The event signal of this page.
///
/// # Panics
/// If called outside the app (the root provides it).
#[must_use]
pub fn use_events() -> LastEvent {
    expect_context::<LastEvent>()
}

/// Does an `entity.updated` event concern `id`?
#[must_use]
pub fn mentions(data: &str, id: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v["entity"]["id"].as_str().map(|s| s == id))
        .unwrap_or(false)
}
