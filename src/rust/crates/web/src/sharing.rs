//! Sharing helpers (D-9): error texts of the sharing actions, absolute links people copy and
//! send, and the per-user privacy switch (Settings > Privacy, and its copy on the invite
//! dialog). The People and Lists pages are `people` and `lists`.

use leptos::prelude::*;
use ui::Label;

use crate::api;
use crate::i18n::t;

/// Error text for sharing actions (`sharing-error-<code>`).
pub(crate) fn share_error(e: &ServerFnError) -> String {
    let key = format!("sharing-error-{}", api::error_code(e));
    let text = t(&key);
    if text == key {
        t("error-generic")
    } else {
        text
    }
}

/// Absolute URL of an app path, for links people copy and send.
pub(crate) fn absolute(path: &str) -> String {
    #[cfg(feature = "hydrate")]
    if let Some(origin) = web_sys::window().and_then(|w| w.location().origin().ok()) {
        return format!("{origin}{path}");
    }
    path.to_string()
}
/// Per-user privacy: whether linked participants see the sessions you log (Settings >
/// Privacy).
#[component]
pub(crate) fn Privacy() -> impl IntoView {
    let user = expect_context::<crate::app::UserResource>();
    let on = RwSignal::new(true);
    Effect::new(move |_| {
        if let Some(Ok(Some(u))) = user.get() {
            on.set(u.share_sessions);
        }
    });
    let save = Action::new(move |v: &bool| {
        let v = *v;
        async move {
            if let Ok(u) = api::set_privacy(v).await {
                on.set(u.share_sessions);
            }
        }
    });
    view! {
        <section class="max-w-md" aria-labelledby="privacy-title">
            <h2 id="privacy-title" class="mb-2 font-medium">{t("privacy-title")}</h2>
            <div class="flex items-center gap-3">
                <input id="share-sessions" type="checkbox" class="size-4 rounded border-input accent-primary"
                    prop:checked=move || on.get()
                    on:change=move |ev| { let v = event_target_checked(&ev); on.set(v); save.dispatch(v); }/>
                <Label r#for="share-sessions">{t("privacy-share-sessions")}</Label>
            </div>
        </section>
    }
}
