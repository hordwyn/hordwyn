//! Re-authentication (OD-12, DEC-191, addendum-1 3.3): when a server function answers
//! `reauth_required`, the page hands its action to [`Reauth::intercept`]; the dialog asks
//! for the password (and the authenticator or a recovery code when enrolled) and, once the
//! server confirms, runs the action again, once. A security dialog is one of the three
//! interruptions D15 allows.
//!
//! An account with a linked provider can "Confirm with <provider>" instead (DEC-323,
//! DEC-326): the dialog keeps the gated action on the server (`PUT /auth/reauth/pending`),
//! the browser does the `max_age=0` round trip and comes back to the same page with
//! `?reauth=ok` (or `?reauth_error=<code>`, which opens the dialog again with the reason).
//! A settings change ([`gated_settings`]) is applied again from the kept action and the page
//! reloads with `?reauth=saved`; any other action asks the person to do it once more (the
//! proof is fresh then, so it goes through).

use std::collections::BTreeMap;

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonVariant, Dialog, FormField, Input,
    field_described_by,
};

use crate::admin_api;
use crate::i18n::{t, use_i18n};

/// A gated action waiting for the proof, and its page's cancel hook.
type Pending = (Callback<()>, Option<Callback<()>>);

/// The dialog's state, shared by every page (provided by the shell).
#[derive(Clone, Copy)]
pub struct Reauth {
    pub open: RwSignal<bool>,
    /// The action to run again once confirmed, and what to tell its page on Cancel.
    retry: RwSignal<Option<Pending>>,
    /// Bumped after each confirmation, so pages that show the window state refetch it.
    pub version: RwSignal<u32>,
    /// The settings change waiting for the proof ([`gated_settings`]), kept on the server
    /// across the provider round trip and applied again afterwards.
    settings: RwSignal<Option<Settings>>,
    /// Why the last provider round trip did not confirm (`?reauth_error=<code>`).
    returned_error: RwSignal<Option<String>>,
}

/// Instance setting values by key (`PUT /admin/settings`).
type Settings = BTreeMap<String, Value>;

/// What the dialog keeps on the server before the provider round trip (DEC-323): the page to
/// come back to and, for a settings change, its values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingAction {
    pub path: String,
    #[serde(default)]
    pub settings: Option<Settings>,
}

/// The page after a provider round trip: confirmed, refused with a code, or a settings
/// change that was applied again (`?reauth=saved`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Returned {
    Confirmed,
    Saved,
    Refused(String),
}

/// Read the round trip's answer from the query (`reauth`, `reauth_error`).
#[must_use]
pub fn returned(reauth: Option<&str>, error: Option<&str>) -> Option<Returned> {
    match (reauth, error) {
        (_, Some(code)) => Some(Returned::Refused(code.to_string())),
        (Some("ok"), None) => Some(Returned::Confirmed),
        (Some("saved"), None) => Some(Returned::Saved),
        _ => None,
    }
}

/// `path` (with its query) without the round trip's parameters, so a reload does not repeat
/// the notice.
#[must_use]
pub fn without_round_trip(path: &str) -> String {
    let (base, query) = path.split_once('?').unwrap_or((path, ""));
    let kept: Vec<&str> = query
        .split('&')
        .filter(|kv| {
            let k = kv.split('=').next().unwrap_or_default();
            !kv.is_empty() && k != "reauth" && k != "reauth_error"
        })
        .collect();
    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}

/// Where the provider round trip starts: the server's start address plus `next`.
#[must_use]
pub fn round_trip_url(start: &str, back: &str) -> String {
    let sep = if start.contains('?') { '&' } else { '?' };
    format!("{start}{sep}next={}", crate::sign_in::encode(back))
}

impl Default for Reauth {
    fn default() -> Self {
        Self::new()
    }
}

impl Reauth {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            retry: RwSignal::new(None),
            version: RwSignal::new(0),
            settings: RwSignal::new(None),
            returned_error: RwSignal::new(None),
        }
    }

    /// If `e` asks for a fresh proof, open the dialog and keep `retry` for afterwards.
    pub fn intercept(&self, e: &ServerFnError, retry: Callback<()>) -> bool {
        self.intercept_with(e, retry, None)
    }

    /// [`Self::intercept`], and run `cancelled` (once) if the person cancels the dialog.
    pub fn intercept_or(
        &self,
        e: &ServerFnError,
        retry: Callback<()>,
        cancelled: Callback<()>,
    ) -> bool {
        self.intercept_with(e, retry, Some(cancelled))
    }

    fn intercept_with(
        &self,
        e: &ServerFnError,
        retry: Callback<()>,
        cancelled: Option<Callback<()>>,
    ) -> bool {
        if crate::api::error_code(e) != "reauth_required" {
            return false;
        }
        self.retry.set(Some((retry, cancelled)));
        self.open.set(true);
        true
    }

    /// Open the dialog ("Confirm it is you" on the Security page passes no action).
    pub fn ask(&self, retry: Option<Callback<()>>) {
        self.retry.set(retry.map(|r| (r, None)));
        self.open.set(true);
    }

    /// The server accepted the proof: close, then run the kept action once.
    pub fn confirmed(&self) {
        self.open.set(false);
        self.version.update(|v| *v += 1);
        self.settings.set(None);
        if let Some((r, _)) = self.retry.try_update(Option::take).flatten() {
            r.run(());
        }
    }

    /// The person cancelled: drop the kept action and tell its page.
    pub fn cancel(&self) {
        let pending = self.retry.try_update(Option::take).flatten();
        self.settings.set(None);
        self.returned_error.set(None);
        self.open.set(false);
        if let Some((_, Some(c))) = pending {
            c.run(());
        }
    }
}

/// The shared state; a fresh one where no shell provided it (component tests).
#[must_use]
pub fn use_reauth() -> Reauth {
    use_context::<Reauth>().unwrap_or_else(|| {
        let r = Reauth::new();
        provide_context(r);
        r
    })
}

/// Run a change that may need a fresh proof: `busy` while it runs, then `done` with the
/// value or the error text. On `reauth_required` the dialog opens and, once confirmed,
/// runs `call` again (once) and reports that answer instead; Cancel reports the refusal
/// (`admin-error-reauth_required`), so the page can undo what it showed.
pub fn gated<T, F, Fut>(
    reauth: Reauth,
    busy: RwSignal<bool>,
    call: F,
    done: Callback<Result<T, String>>,
) where
    T: Send + Sync + 'static,
    F: Fn() -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<T, ServerFnError>> + 'static,
{
    // a save with the disclosures open at that moment (addendum-1 D16, WP4-5)
    let funnel = crate::funnel::Funnel::current();
    let i18n = use_i18n();
    // `done` runs after an `.await`, outside the page's owner: give it the owner back, so
    // it can read contexts (messages) like any handler
    let owner = Owner::current();
    let done = Callback::new(move |r: Result<T, String>| match &owner {
        Some(o) => o.with(|| done.run(r)),
        None => done.run(r),
    });
    busy.set(true);
    leptos::task::spawn_local(async move {
        let answer = call().await;
        busy.set(false);
        match answer {
            Ok(v) => {
                funnel.saved();
                done.run(Ok(v));
            }
            Err(e) => {
                let again = call.clone();
                let texts = i18n.clone();
                let retry = Callback::new(move |()| {
                    let (again, texts) = (again.clone(), texts.clone());
                    busy.set(true);
                    leptos::task::spawn_local(async move {
                        let r = again().await;
                        busy.set(false);
                        done.run(r.map_err(|e| crate::settings::admin_error_with(&texts, &e)));
                    });
                });
                // Cancel: nothing changed; the page hears it like any refusal (UX2-W10)
                let refused = crate::settings::admin_error_with(&i18n, &e);
                let cancelled = Callback::new(move |()| done.run(Err(refused.clone())));
                if !reauth.intercept_or(&e, retry, cancelled) {
                    done.run(Err(crate::settings::admin_error_with(&i18n, &e)));
                }
            }
        }
    });
}

/// [`gated`] for `PUT /admin/settings`: the values are also what the provider round trip
/// keeps and applies again afterwards (DEC-323).
pub fn gated_settings(
    reauth: Reauth,
    busy: RwSignal<bool>,
    values: Settings,
    done: Callback<Result<contracts::admin::InstanceSettingsDto, String>>,
) {
    reauth.settings.set(Some(values.clone()));
    let call = move || admin_api::update_admin_settings(values.clone());
    gated(
        reauth,
        busy,
        call,
        Callback::new(move |r| {
            reauth.settings.set(None);
            done.run(r);
        }),
    );
}

/// Text for a failed proof (`auth-error-<code>`, the sign-in wording).
fn proof_error(e: &ServerFnError) -> String {
    crate::pages::error_text(e)
}

/// Text for a round trip that came back refused (`reauth-error-<code>`).
fn returned_error(code: &str) -> String {
    let key = format!("reauth-error-{code}");
    let text = t(&key);
    if text == key {
        t("reauth-error-oidc_failed")
    } else {
        text
    }
}

/// The current path with its query (browser only).
fn here() -> String {
    #[cfg(feature = "hydrate")]
    if let Some(l) = web_sys::window().map(|w| w.location()) {
        return format!(
            "{}{}",
            l.pathname().unwrap_or_default(),
            l.search().unwrap_or_default()
        );
    }
    "/".into()
}

/// Replace the address without the round trip's parameters (no reload).
fn forget_round_trip() {
    #[cfg(feature = "hydrate")]
    if let Some(w) = web_sys::window()
        && let Ok(h) = w.history()
    {
        let _ = h.replace_state_with_url(
            &wasm_bindgen::JsValue::NULL,
            "",
            Some(&without_round_trip(&here())),
        );
    }
}

/// Keep the action on the server, then go to the provider (`max_age=0`).
async fn start_round_trip(r: Reauth, start: String) -> Result<(), ServerFnError> {
    let back = without_round_trip(&here());
    let action = PendingAction {
        path: back.clone(),
        settings: r.settings.get_untracked(),
    };
    let body = serde_json::json!({ "action": action });
    crate::rest::send("PUT", "/auth/reauth/pending", Some(body)).await?;
    crate::pages::go(&round_trip_url(&start, &back));
    Ok(())
}

/// Back from the provider with `?reauth=ok`: apply a kept settings change again and reload
/// with `?reauth=saved`, or ask for the change once more.
async fn resume(toasts: ui::Toasts, texts: std::sync::Arc<crate::i18n::I18n>) {
    let kept = crate::rest::call::<Value>("GET", "/auth/reauth/pending", None)
        .await
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_value::<PendingAction>(v.get("action")?.clone()).ok());
    match kept {
        Some(PendingAction {
            path,
            settings: Some(values),
        }) => match admin_api::update_admin_settings(values).await {
            Ok(_) => crate::pages::go(&round_trip_url_saved(&path)),
            Err(e) => toasts.push(
                crate::settings::admin_error_with(&texts, &e),
                None,
                ui::ToastVariant::Destructive,
            ),
        },
        _ => toasts.push(
            texts.t("reauth-returned-again"),
            None,
            ui::ToastVariant::Default,
        ),
    }
}

/// `path` with `reauth=saved`.
fn round_trip_url_saved(path: &str) -> String {
    // the kept path came back from the server: follow local paths only (UX3-19)
    let path = crate::sign_in::local_path(path).unwrap_or_else(|| "/".into());
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{path}{sep}reauth=saved")
}

/// On page load: the answer of a provider round trip, if this page is one's return.
fn after_round_trip(r: Reauth) {
    let q = leptos_router::hooks::use_query_map();
    let toasts = ui::use_toasts();
    let texts = use_i18n();
    Effect::new(move |_| {
        let back = q.with_untracked(|q| {
            returned(q.get("reauth").as_deref(), q.get("reauth_error").as_deref())
        });
        let Some(back) = back else { return };
        forget_round_trip();
        match back {
            Returned::Confirmed => {
                r.version.update(|v| *v += 1);
                leptos::task::spawn_local(resume(toasts, texts.clone()));
            }
            Returned::Saved => toasts.push(
                texts.t("reauth-returned-saved"),
                None,
                ui::ToastVariant::Default,
            ),
            Returned::Refused(code) => {
                r.returned_error.set(Some(returned_error(&code)));
                r.ask(None);
            }
        }
    });
}

/// The dialog, mounted once by the shell for a signed-in person.
#[component]
pub fn ReauthDialog() -> impl IntoView {
    let r = use_reauth();
    let i18n = use_i18n();
    let password = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let has_totp = RwSignal::new(false);
    let has_password = RwSignal::new(true);
    // "Confirm with <provider>": its name and where the round trip starts
    let provider = RwSignal::new(None::<(String, String)>);
    after_round_trip(r);
    // what the dialog asks for: loaded each time it opens
    let status = Action::new(|(): &()| admin_api::reauth_status());
    Effect::new(move |_| {
        if r.open.get() {
            password.set(String::new());
            code.set(String::new());
            error.set(r.returned_error.get_untracked());
            status.dispatch(());
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(s)) = status.value().get() {
            has_totp.set(s.has_totp);
            has_password.set(s.has_password);
            let oidc = s.methods.iter().any(|m| m == "oidc");
            provider.set(oidc.then(|| s.oidc_provider.zip(s.oidc_start)).flatten());
        }
    });
    let submit = Action::new(move |(): &()| {
        let (p, c) = (password.get_untracked(), code.get_untracked());
        async move {
            match admin_api::reauth(p, Some(c)).await {
                Ok(_) => r.confirmed(),
                Err(e) => error.set(Some(proof_error(&e))),
            }
        }
    });
    let round_trip = Action::new(move |start: &String| {
        let start = start.clone();
        async move {
            if let Err(e) = start_round_trip(r, start).await {
                error.set(Some(crate::settings::admin_error(&e)));
            }
        }
    });
    let pending = submit.pending();
    let invalid = Signal::derive(move || error.get().is_some());
    view! {
        <Dialog open=r.open title=i18n.t("reauth-title") description=i18n.t("reauth-lead")
            close_label=i18n.t("dialog-close") initial_focus="ra-password" can_close=Callback::new(move |()| { r.cancel(); true })>
            <div class="grid gap-4">
                {move || provider.get().map(|(name, start)| {
                    let label = use_i18n().t_args("reauth-with-provider", &[("provider", name.into())]);
                    let variant = if has_password.get() { ButtonVariant::Outline } else { ButtonVariant::Default };
                    view! {
                        <Button variant=variant attr:id="ra-provider" loading=round_trip.pending()
                            on:click=move |_| { round_trip.dispatch(start.clone()); }>
                            {label}
                        </Button>
                    }
                })}
                <Show when=move || has_password.get()>
                    <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); submit.dispatch(()); }>
                        <FormField id="ra-password" label=t("reauth-password")>
                            <Input id="ra-password" r#type="password" value=password autocomplete="current-password"
                                invalid=invalid described_by=field_described_by("ra-password")/>
                        </FormField>
                        <Show when=move || has_totp.get()>
                            <FormField id="ra-code" label=t("reauth-code") description=t("reauth-code-hint")>
                                <Input id="ra-code" value=code autocomplete="one-time-code"
                                    described_by=field_described_by("ra-code")/>
                            </FormField>
                        </Show>
                        <div class="flex flex-wrap justify-end gap-2">
                            <Button variant=ButtonVariant::Outline on:click=move |_| r.cancel()>{t("reauth-cancel")}</Button>
                            <Button r#type="submit" loading=pending>{t("reauth-confirm")}</Button>
                        </div>
                    </form>
                </Show>
                {move || error.get().map(|e| view! {
                    <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{e}</Alert>
                })}
                <Show when=move || !has_password.get()>
                    <div class="flex justify-end">
                        <Button variant=ButtonVariant::Outline on:click=move |_| r.cancel()>{t("reauth-cancel")}</Button>
                    </div>
                </Show>
            </div>
        </Dialog>
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    #[test]
    fn only_reauth_required_opens_the_dialog_and_the_retry_runs_once() {
        let owner = Owner::new();
        owner.with(|| {
            let r = Reauth::new();
            let runs = Arc::new(AtomicU32::new(0));
            let n = runs.clone();
            let retry = Callback::new(move |()| {
                n.fetch_add(1, Ordering::SeqCst);
            });
            let other = ServerFnError::new("last_admin");
            assert!(!r.intercept(&other, retry));
            assert!(!r.open.get_untracked());
            let gated = ServerFnError::new("reauth_required");
            assert!(r.intercept(&gated, retry));
            assert!(r.open.get_untracked());
            r.confirmed();
            assert!(!r.open.get_untracked());
            assert_eq!(runs.load(Ordering::SeqCst), 1);
            // a second confirmation does not run it again
            r.confirmed();
            assert_eq!(runs.load(Ordering::SeqCst), 1);
            assert_eq!(
                r.version.get_untracked(),
                2,
                "pages refetch the window state"
            );
            // cancelling drops the retry
            assert!(r.intercept(&gated, retry));
            r.cancel();
            r.confirmed();
            assert_eq!(runs.load(Ordering::SeqCst), 1);
        });
    }

    /// UX2-W10: a cancelled proof tells the page, so it can undo what it showed (the
    /// licence checkbox); a confirmed one does not.
    #[test]
    fn a_cancelled_proof_runs_the_cancel_hook_once_and_never_the_retry() {
        let owner = Owner::new();
        owner.with(|| {
            let r = Reauth::new();
            let (runs, cancels) = (Arc::new(AtomicU32::new(0)), Arc::new(AtomicU32::new(0)));
            let (n, c) = (runs.clone(), cancels.clone());
            let retry = Callback::new(move |()| {
                n.fetch_add(1, Ordering::SeqCst);
            });
            let cancelled = Callback::new(move |()| {
                c.fetch_add(1, Ordering::SeqCst);
            });
            let gated = ServerFnError::new("reauth_required");
            assert!(r.intercept_or(&gated, retry, cancelled));
            r.cancel();
            r.cancel();
            assert_eq!(cancels.load(Ordering::SeqCst), 1);
            assert_eq!(runs.load(Ordering::SeqCst), 0);
            assert!(r.intercept_or(&gated, retry, cancelled));
            r.confirmed();
            assert_eq!(runs.load(Ordering::SeqCst), 1);
            assert_eq!(cancels.load(Ordering::SeqCst), 1);
        });
    }

    /// DEC-326: the round trip comes back to the page it left, with `reauth=ok` or
    /// `reauth_error=<code>`, which the page reads and then drops from the address.
    #[test]
    fn the_provider_round_trip_returns_to_the_same_page() {
        assert_eq!(
            round_trip_url("/api/v1/auth/oidc/start?mode=reauth", "/admin/sso?tab=1"),
            "/api/v1/auth/oidc/start?mode=reauth&next=%2Fadmin%2Fsso%3Ftab%3D1"
        );
        assert_eq!(returned(Some("ok"), None), Some(Returned::Confirmed));
        assert_eq!(returned(Some("saved"), None), Some(Returned::Saved));
        assert_eq!(
            returned(None, Some("reauth_not_fresh")),
            Some(Returned::Refused("reauth_not_fresh".into()))
        );
        assert_eq!(returned(None, None), None);
        assert_eq!(without_round_trip("/admin/sso?reauth=ok"), "/admin/sso");
        assert_eq!(
            without_round_trip("/x?a=1&reauth_error=oidc_denied&b=2"),
            "/x?a=1&b=2"
        );
        let kept = PendingAction {
            path: "/admin/instance".into(),
            settings: Some(BTreeMap::from([(
                "auth.signup".into(),
                Value::from("closed"),
            )])),
        };
        let json = serde_json::to_value(&kept).unwrap();
        assert_eq!(serde_json::from_value::<PendingAction>(json).unwrap(), kept);
    }

    /// UX3-19: the kept path comes back from the server; only a local path is followed (a
    /// page at `//evil/x` must not send the browser off-site after the proof).
    #[test]
    fn the_replay_goes_to_local_paths_only() {
        assert_eq!(
            round_trip_url_saved("/admin/instance"),
            "/admin/instance?reauth=saved"
        );
        assert_eq!(round_trip_url_saved("/x?a=1"), "/x?a=1&reauth=saved");
        assert_eq!(round_trip_url_saved("//evil.example/x"), "/?reauth=saved");
        assert_eq!(round_trip_url_saved("/\\\\evil.example"), "/?reauth=saved");
        assert_eq!(
            round_trip_url_saved("https://evil.example/"),
            "/?reauth=saved"
        );
    }
}
