//! The admin nudge on Home (design.md 4.4, 6.1; DEC-237): admins only, one status-first line
//! with one action, while the setup is open, a whole kind group has no working source, or a
//! source that is set up does not answer. "Snooze for 7 days" hides the same problem for a
//! week in this browser (a per-viewer convenience: a new problem shows at once).

use leptos::prelude::*;
use ui::{Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant};

use crate::admin_api;
use crate::i18n::{I18n, t, use_i18n};
use crate::source_hints::{Nudge, nudge, snooze_value, snoozed};

#[cfg(feature = "hydrate")]
const SNOOZE_KEY: &str = "app-nudge-snooze";

fn now_ms() -> f64 {
    #[cfg(feature = "hydrate")]
    {
        web_sys::js_sys::Date::now()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        0.0
    }
}

#[cfg(feature = "hydrate")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// The stored snooze, if the browser keeps one.
fn stored_snooze() -> Option<String> {
    #[cfg(feature = "hydrate")]
    {
        storage().and_then(|s| s.get_item(SNOOZE_KEY).ok().flatten())
    }
    #[cfg(not(feature = "hydrate"))]
    {
        None
    }
}

fn store_snooze(value: &str) {
    #[cfg(feature = "hydrate")]
    if let Some(s) = storage() {
        let _ = s.set_item(SNOOZE_KEY, value);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = value;
}

/// The line and the action's label of a nudge.
fn words(i18n: &I18n, n: &Nudge) -> (String, String) {
    match n {
        Nudge::Setup => (
            i18n.t("home-nudge-setup"),
            i18n.t("home-nudge-setup-action"),
        ),
        Nudge::NoSource(groups) => {
            let names: Vec<String> = groups
                .iter()
                .map(|g| i18n.t(&format!("kind-group-{g}")))
                .collect();
            (
                i18n.t_args(
                    "home-nudge-no-source",
                    &[
                        ("groups", names.join(", ").into()),
                        ("count", names.len().into()),
                    ],
                ),
                i18n.t("home-nudge-set-up"),
            )
        }
        Nudge::Failing(names) => (
            i18n.t_args(
                "home-nudge-failing",
                &[
                    ("sources", names.join(", ").into()),
                    ("count", names.len().into()),
                ],
            ),
            i18n.t("home-nudge-check"),
        ),
    }
}

#[component]
pub fn AdminNudge() -> impl IntoView {
    let data = Resource::new(|| (), |()| admin_api::source_status());
    // read after hydration only: the server cannot know this browser's snooze
    let snooze = RwSignal::new(None::<String>);
    Effect::new(move |_| snooze.set(stored_snooze()));
    view! {
        <Suspense fallback=|| ()>
            {move || data.get().and_then(Result::ok).and_then(|status| nudge(&status)).map(|n| {
                let i18n = use_i18n();
                let key = n.key();
                let hidden_key = key.clone();
                let hidden = move || snoozed(snooze.get().as_deref(), &hidden_key, now_ms());
                let (line, action) = words(&i18n, &n);
                let href = n.href();
                let on_snooze = move |_| {
                    let v = snooze_value(&key, now_ms());
                    store_snooze(&v);
                    snooze.set(Some(v));
                };
                view! {
                    <div data-testid="nudge" class="mb-6" hidden=hidden>
                        <Alert variant=AlertVariant::Warning role=AlertRole::None
                            actions=move || view! {
                                <a class="text-sm font-medium underline" href=href>{action.clone()}</a>
                                <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on:click=on_snooze.clone()>
                                    {t("home-nudge-snooze")}
                                </Button>
                            }>
                            {line}
                        </Alert>
                    </div>
                }
            })}
        </Suspense>
    }
}
