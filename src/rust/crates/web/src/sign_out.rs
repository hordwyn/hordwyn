//! Signing out (UX4-6, UX4-22): the account menu and the You hub submit one form, mounted once
//! by the shell, that posts to the logout route (it works before hydration too). Hydrated, the
//! submit first sends the offline queue when the browser is online; when nothing is left the
//! browser's recent items are forgotten and the form goes out. When changes are still waiting
//! (offline, or the push failed) a dialog says how many and lets the person stay signed in or
//! sign out and discard them, so another person on this browser never finds them.

use leptos::html;
use leptos::prelude::*;
use ui::{Button, ButtonVariant, Dialog};

use crate::app::ClientConfig;
use crate::i18n::use_i18n;

/// Where the account menu and the You hub sign out: a form post, so the server ends the
/// session first and then sends the browser to the provider's end-session page or straight
/// to `/signed-out` (addendum-1 4.2 "Logout", DEC-318). Works before hydration too.
pub const SIGN_OUT_ACTION: &str = "/api/v1/auth/logout";

/// The id of the sign-out form (the menu item and the You hub button are its submit buttons).
pub const SIGN_OUT_FORM: &str = "sign-out-form";

/// What sign-out does with a queue of `waiting` commands, after a flush that left `left`
/// (`None`: no flush was possible, offline or one already running).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    /// Nothing waits: forget this browser's data and sign out.
    Now,
    /// Ask: this many changes would be lost.
    Ask(usize),
}

/// The decision after the flush attempt.
#[must_use]
pub fn leave_after(left: Option<usize>, waiting: usize) -> Leave {
    match left {
        Some(0) => Leave::Now,
        Some(n) => Leave::Ask(n),
        None if waiting == 0 => Leave::Now,
        None => Leave::Ask(waiting),
    }
}

/// The hidden form and its "not synced yet" dialog (signed in only).
#[component]
pub fn SignOutForm() -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let offline = use_context::<crate::offline::Offline>();
    let is_offline = crate::offline::is_offline();
    let (i18n, toasts) = (use_i18n(), ui::use_toasts());
    let asked = RwSignal::new(0_usize);
    let open = RwSignal::new(false);
    let form = NodeRef::<html::Form>::new();
    let user = StoredValue::new(cfg.username);
    let leave = move || {
        crate::recent::forget(&user.get_value());
        if let Some(f) = form.get_untracked() {
            let _ = f.submit();
        }
    };
    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        let Some(o) = offline else {
            crate::recent::forget(&user.get_value());
            return;
        };
        if o.waiting.get_untracked() == 0 {
            crate::recent::forget(&user.get_value());
            return;
        }
        ev.prevent_default();
        let i18n = i18n.clone();
        let online = !is_offline.get_untracked();
        leptos::task::spawn_local(async move {
            let left = if online {
                o.send(&i18n, toasts).await
            } else {
                None
            };
            match leave_after(left, o.waiting.get_untracked()) {
                Leave::Now => leave(),
                Leave::Ask(n) => {
                    asked.set(n);
                    open.set(true);
                }
            }
        });
    };
    let discard = move |_| {
        open.set(false);
        if let Some(o) = offline {
            o.discard();
        }
        leave();
    };
    view! {
        <form id=SIGN_OUT_FORM method="post" action=SIGN_OUT_ACTION hidden node_ref=form on:submit=on_submit></form>
        {move || {
            let i = use_i18n();
            let title = i.t_args("signout-waiting-title", &[("count", asked.get().into())]);
            view! {
                <Dialog open=open title=title description=i.t("signout-waiting-body") close_label=i.t("dialog-close")>
                    <div class="flex flex-wrap justify-end gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| open.set(false)>{crate::i18n::t("signout-stay")}</Button>
                        <Button variant=ButtonVariant::Destructive on:click=discard>{crate::i18n::t("signout-discard")}</Button>
                    </div>
                </Dialog>
            }
        }}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_out_asks_only_while_changes_wait() {
        assert_eq!(
            leave_after(Some(0), 3),
            Leave::Now,
            "the flush sent everything"
        );
        assert_eq!(
            leave_after(Some(2), 3),
            Leave::Ask(2),
            "two were kept for later"
        );
        assert_eq!(leave_after(None, 3), Leave::Ask(3), "offline: not sent");
        assert_eq!(leave_after(None, 0), Leave::Now);
    }
}
