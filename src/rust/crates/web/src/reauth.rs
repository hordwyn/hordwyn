//! Re-authentication (OD-12, DEC-191, addendum-1 3.3): when a server function answers
//! `reauth_required`, the page hands its action to [`Reauth::intercept`]; the dialog asks
//! for the password (and the authenticator or a recovery code when enrolled) and, once the
//! server confirms, runs the action again, once. A security dialog is one of the three
//! interruptions D15 allows.

use leptos::prelude::*;
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
        if let Some((r, _)) = self.retry.try_update(Option::take).flatten() {
            r.run(());
        }
    }

    /// The person cancelled: drop the kept action and tell its page.
    pub fn cancel(&self) {
        let pending = self.retry.try_update(Option::take).flatten();
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
            Ok(v) => done.run(Ok(v)),
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

/// Text for a failed proof (`auth-error-<code>`, the sign-in wording).
fn proof_error(e: &ServerFnError) -> String {
    crate::pages::error_text(e)
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
    // what the dialog asks for: loaded each time it opens
    let status = Action::new(|(): &()| admin_api::reauth_status());
    Effect::new(move |_| {
        if r.open.get() {
            password.set(String::new());
            code.set(String::new());
            error.set(None);
            status.dispatch(());
        }
    });
    Effect::new(move |_| {
        if let Some(Ok(s)) = status.value().get() {
            has_totp.set(s.has_totp);
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
    let pending = submit.pending();
    let invalid = Signal::derive(move || error.get().is_some());
    view! {
        <Dialog open=r.open title=i18n.t("reauth-title") description=i18n.t("reauth-lead")
            close_label=i18n.t("dialog-close") initial_focus="ra-password" can_close=Callback::new(move |()| { r.cancel(); true })>
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
                {move || error.get().map(|e| view! {
                    <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{e}</Alert>
                })}
                <div class="flex flex-wrap justify-end gap-2">
                    <Button variant=ButtonVariant::Outline on:click=move |_| r.cancel()>{t("reauth-cancel")}</Button>
                    <Button r#type="submit" loading=pending>{t("reauth-confirm")}</Button>
                </div>
            </form>
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
}
