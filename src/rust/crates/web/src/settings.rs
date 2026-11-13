//! The settings area (design.md 3.3, 3.7, 6.12; addendum-1 3.4): one side nav with "You"
//! and, for admins, "Instance (admin)"; every page has the anatomy H1, lead, attention
//! block, open set, disclosures. The personal pages live here: Account, Notifications,
//! Integrations, Data sources and Privacy (moved from their old top-level paths without
//! redirects, DEC-171).

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use contracts::admin::{SourceStatusDto, UserCredentialDto};
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use serde::Serialize;
use serde::de::DeserializeOwned;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant, CopyField, Disclosure,
    DisclosureMode, FormField, Input, NavGroup, NavItem, SecretField, SideNav, Skeleton, StatusDot,
    disclosure_storage_key, field_described_by,
};

use crate::admin_api;
use crate::admin_state::{ADMIN_PAGES, SETTINGS_PAGES, ask_admin_for, source_state};
use crate::app::ClientConfig;
use crate::i18n::{kind_label, t, use_i18n};
use crate::pages::{PageTitle, fresh};
use crate::reauth::use_reauth;

/// Classes that turn the side nav into a horizontal scroller below 47.5rem (addendum-1 3.4).
const NAV_CLASS: &str = "max-[47.5rem]:flex max-[47.5rem]:items-center max-[47.5rem]:gap-2 \
    max-[47.5rem]:overflow-x-auto max-[47.5rem]:pb-2 max-[47.5rem]:[&_ul]:flex \
    max-[47.5rem]:[&_li]:shrink-0 max-[47.5rem]:[&_a]:whitespace-nowrap \
    max-[47.5rem]:[&_p]:whitespace-nowrap";

/// The side nav's column from 47.5rem: sticky below the top bar, scrolling on its own when
/// it is taller than the window.
const STICKY_NAV: &str = "min-w-0 min-[47.5rem]:sticky \
    min-[47.5rem]:top-[calc(var(--top-bar-height)+1rem)] min-[47.5rem]:self-start \
    min-[47.5rem]:max-h-[calc(100dvh-var(--top-bar-height)-2rem)] min-[47.5rem]:overflow-y-auto";

/// The side nav groups: "You" for everyone, the admin pages for admins only.
#[must_use]
pub fn nav_groups(is_admin: bool) -> Vec<NavGroup> {
    let i18n = use_i18n();
    let items = |pages: &[(&str, &str)]| -> Vec<NavItem> {
        pages
            .iter()
            .map(|(href, key)| NavItem::new(*href, i18n.t(key)))
            .collect()
    };
    let mut groups =
        vec![NavGroup::new(items(&SETTINGS_PAGES)).heading(i18n.t("settings-group-you"))];
    if is_admin {
        groups.push(NavGroup::new(items(&ADMIN_PAGES)).heading(i18n.t("settings-group-admin")));
    }
    groups
}

/// Side nav plus the page.
#[component]
pub fn SettingsLayout(children: Children) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let location = use_location();
    view! {
        <div class="grid gap-6 min-[47.5rem]:grid-cols-[13rem_minmax(0,1fr)]">
            // on wide screens the nav stays in view below the sticky top bar instead of
            // scrolling under it (a link half under the bar is too small a target, WCAG 2.5.8)
            <div class=STICKY_NAV>
                <SideNav id="set-nav" label=t("settings-nav-label") groups=nav_groups(cfg.is_admin)
                    current=Signal::derive(move || location.pathname.get()) class=NAV_CLASS/>
            </div>
            <div class="min-w-0">{children()}</div>
        </div>
    }
}

/// The page's one-sentence lead (design.md 3.7).
#[component]
pub fn Lead(#[prop(into)] text: String) -> impl IntoView {
    view! { <p class="-mt-4 mb-6 max-w-2xl text-sm text-muted-foreground">{text}</p> }
}

/// Text for an error of the phase 2 functions: `admin-error-<code>`, else the sign-in
/// wording (`auth-error-<code>`), else the generic sentence.
#[must_use]
pub fn admin_error(e: &ServerFnError) -> String {
    admin_error_with(&use_i18n(), e)
}

/// [`admin_error`] with the messages at hand (inside tasks that run without the page's
/// context).
#[must_use]
pub fn admin_error_with(i18n: &crate::i18n::I18n, e: &ServerFnError) -> String {
    let code = crate::api::error_code(e);
    for key in [format!("admin-error-{code}"), format!("auth-error-{code}")] {
        let text = i18n.t(&key);
        if text != key {
            return text;
        }
    }
    i18n.t("error-generic")
}

/// State that outlives one render of a [`loaded`] value: every refetch renders the value
/// afresh (UX1-2), which would drop an outcome line ("Saved.") or an open card set just
/// before the refetch. Owned by the `loaded` call, provided to each render.
#[derive(Clone)]
pub struct Kept {
    owner: Owner,
    slots: StoredValue<HashMap<String, Box<dyn Any + Send + Sync>>>,
    /// The menus of [`kept_menu`], each with "asked for by the latest render".
    menus: StoredValue<HashMap<String, (KeptMenu, bool)>>,
    /// Render the next answer afresh even when it equals the last one ([`render_again`]).
    again: StoredValue<bool>,
}

impl Kept {
    /// Slots owned by the current owner (the one around `loaded`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            owner: Owner::current().unwrap_or_default(),
            slots: StoredValue::new(HashMap::new()),
            menus: StoredValue::new(HashMap::new()),
            again: StoredValue::new(false),
        }
    }

    /// A render starts: no menu is asked for yet.
    fn begin_render(&self) {
        self.menus
            .update_value(|m| m.values_mut().for_each(|(_, asked)| *asked = false));
    }

    /// After a render: close the menus it did not ask for (their card or page left the
    /// answer, or the person page shows another person), so that one coming back later does
    /// not open by itself (RACE-6).
    fn close_menus_not_asked(&self) {
        self.menus.with_value(|m| {
            for (menu, asked) in m.values() {
                if !asked {
                    menu.close();
                }
            }
        });
    }
}

/// A handle that makes the next answer of the nearest [`loaded`] render afresh even when it
/// equals the one on screen (nothing outside `loaded`): for the callback of a save that the
/// server may store other than typed (`045` stored as 45, a normalised value), so that the
/// fields show what is stored (RACE-5). Call it before the refetch.
pub fn render_again() -> impl Fn() + Copy + Send + Sync + 'static {
    let again = use_context::<Kept>().map(|k| k.again);
    move || {
        if let Some(a) = again {
            a.set_value(true);
        }
    }
}

/// The open state of a menu and its focused item, kept across the renders of a [`loaded`]
/// value (LR-4, RACE-2): a menu a different answer renders again stays open, with the focus
/// on the same item.
#[derive(Clone, Copy)]
pub struct KeptMenu {
    pub open: RwSignal<bool>,
    pub active: RwSignal<usize>,
}

impl KeptMenu {
    /// A menu of its own, closed (outside `loaded`, or kept by its page).
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            active: RwSignal::new(0),
        }
    }

    pub fn close(self) {
        self.open.set(false);
        self.active.set(0);
    }
}

impl Default for KeptMenu {
    fn default() -> Self {
        Self::new()
    }
}

/// The menu named `key` (with the row id in lists) of the nearest [`loaded`] render; a menu
/// that a render does not ask for is closed after it (RACE-6).
pub fn kept_menu(key: &str) -> KeptMenu {
    let menu = KeptMenu {
        open: kept(&format!("{key}-open"), || false),
        active: kept(&format!("{key}-active"), || 0_usize),
    };
    if let Some(k) = use_context::<Kept>() {
        k.menus.update_value(|m| {
            m.insert(key.to_string(), (menu, true));
        });
    }
    menu
}

impl Default for Kept {
    fn default() -> Self {
        Self::new()
    }
}

/// The signal named `key` of the nearest [`loaded`] render, created with `init` the first
/// time; a plain new signal outside `loaded`. Keys are per page (add the row id in lists).
pub fn kept<T: Send + Sync + 'static>(key: &str, init: impl FnOnce() -> T) -> RwSignal<T> {
    let Some(k) = use_context::<Kept>() else {
        return RwSignal::new(init());
    };
    let found = k.slots.with_value(|m| {
        m.get(key)
            .and_then(|b| b.downcast_ref::<RwSignal<T>>())
            .copied()
    });
    if let Some(s) = found {
        return s;
    }
    let s = k.owner.with(|| RwSignal::new(init()));
    k.slots.update_value(|m| {
        m.insert(key.to_string(), Box::new(s));
    });
    s
}

/// The key of a [`loaded`] render for `answer`, given the last `Ok` value shown and its key
/// (`last`, updated here): the same key for an `Ok` equal to that value, so the render stays
/// as it is; a new key from `next` for another value and for an error (LR-3). After an error
/// the next value is rendered anew whatever it is.
pub(crate) fn render_key<T: Clone + PartialEq, E>(
    last: &mut Option<(u32, T)>,
    answer: &Result<T, E>,
    mut next: impl FnMut() -> u32,
) -> u32 {
    match (answer, last.as_ref()) {
        (Ok(v), Some((k, old))) if v == old => *k,
        (Ok(v), _) => {
            let k = next();
            *last = Some((k, v.clone()));
            k
        }
        (Err(_), _) => {
            *last = None;
            next()
        }
    }
}

/// Render a resource inside a `Transition`, each different value afresh (UX1-2) while an
/// equal one leaves the render as it is (LR-3: the echo of an own action and events about
/// other things keep cards, menus, focus and typed text), with an inline error and Retry.
/// State that must survive a different value comes from [`kept`] and [`kept_menu`]; a caller
/// whose save the server may store other than typed asks for the next render with
/// [`render_again`].
pub fn loaded<T, V>(
    data: Resource<Result<T, ServerFnError>>,
    render: impl Fn(T) -> V + Send + Sync + 'static,
) -> impl IntoView
where
    T: Clone + PartialEq + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: IntoView + 'static,
{
    let renders = StoredValue::new(0_u32);
    let last = StoredValue::new(None::<(u32, T)>);
    let render = Arc::new(render);
    let keep = Kept::new();
    let again = keep.again;
    let key_of = move |r: &Result<T, ServerFnError>| {
        let mut key = 0;
        let afresh = again.try_update_value(std::mem::take).unwrap_or(false);
        last.update_value(|shown| {
            if afresh {
                *shown = None;
            }
            key = render_key(shown, r, || fresh(renders));
        });
        key
    };
    view! {
        <Transition fallback=|| view! { <Skeleton class="h-32 w-full"/> }>
            <For each=move || data.get().map(|r| (key_of(&r), r)) key=|(k, _)| *k
                children=move |(_, r)| match r {
                    Ok(v) => {
                        keep.begin_render();
                        provide_context(keep.clone());
                        let view = render(v).into_any();
                        // after the render and the views it builds when it is mounted
                        let swept = keep.clone();
                        Effect::new(move |_| swept.close_menus_not_asked());
                        view
                    }
                    Err(e) => view! {
                        <Alert variant=AlertVariant::Destructive role=AlertRole::Alert
                            actions=move || view! {
                                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm
                                    on:click=move |_| data.refetch()>{t("library-retry")}</Button>
                            }>
                            {admin_error(&e)}
                        </Alert>
                    }.into_any(),
                }/>
        </Transition>
    }
}

// ---------------------------------------------------------------- the moved pages

#[component]
pub fn SettingsNotifications() -> impl IntoView {
    view! { <SettingsLayout><crate::notifications::Notifications/></SettingsLayout> }
}

#[component]
pub fn SettingsIntegrations() -> impl IntoView {
    view! { <SettingsLayout><crate::integrations::Integrations/></SettingsLayout> }
}

#[component]
pub fn SettingsPrivacy() -> impl IntoView {
    // what the logging funnel counts, in one line (WP4-5)
    let usage = if expect_context::<ClientConfig>().usage_funnel {
        t("privacy-usage-on")
    } else {
        t("privacy-usage-off")
    };
    view! {
        <SettingsLayout>
            <PageTitle text=t("privacy-title")/>
            <Lead text=t("settings-privacy-lead")/>
            <crate::sharing::Privacy/>
            <p class="mt-6 max-w-2xl text-sm text-muted-foreground" data-testid="usage-line">{usage}</p>
        </SettingsLayout>
    }
}

// ---------------------------------------------------------------- account

#[component]
pub fn SettingsAccount() -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    // a failed link round trip comes back here (`?error=<code>`, DEC-311): open Sign-in
    let link_error = leptos_router::hooks::use_query_map().with_untracked(|q| q.get("error"));
    let force = link_error.is_some();
    let user = expect_context::<crate::app::UserResource>();
    let email = move || {
        user.get()
            .and_then(Result::ok)
            .flatten()
            .and_then(|u| u.email)
            .unwrap_or_else(|| t("settings-account-no-email"))
    };
    view! {
        <SettingsLayout>
            <PageTitle text=t("settings-nav-account")/>
            <Lead text=t("settings-account-lead")/>
            <dl class="mb-6 grid max-w-xl grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
                <dt class="font-medium">{t("settings-account-display-name")}</dt>
                <dd>{cfg.display_name.clone()}</dd>
                <dt class="font-medium">{t("settings-account-username")}</dt>
                <dd>
                    <span class="font-mono">{cfg.username.clone()}</span>
                    <span class="block text-muted-foreground">{t("settings-account-username-hint")}</span>
                </dd>
                <dt class="font-medium">{t("settings-account-email")}</dt>
                <dd><Suspense fallback=|| ()>{email}</Suspense></dd>
            </dl>
            <div class="mb-6 grid max-w-xl gap-4">
                <crate::shell::ThemeChoiceControl/>
                <crate::shell::LanguageChoice/>
                <ShortcutsSwitch/>
            </div>
            <div class="mb-6"><crate::account::TimeZone/></div>
            <Disclosure id="set-signin" title=t("settings-signin") summary=t("settings-signin-summary")
                mode=DisclosureMode::Button level=2 storage_key=disclosure_storage_key("account", "signin")
                force_open=force class="max-w-2xl">
                <div class="grid gap-8">
                    <PasswordForm/>
                    <Authenticator/>
                    <crate::account::LinkedSignIns error=link_error/>
                </div>
            </Disclosure>
        </SettingsLayout>
    }
}

/// "Keyboard shortcuts" on or off (WCAG 2.1.4 character key shortcuts; remembered in this
/// browser, WP4-4). Ctrl/Cmd+K and keys inside a widget stay.
#[component]
fn ShortcutsSwitch() -> impl IntoView {
    let on = expect_context::<crate::keyboard::ShortcutsOn>().0;
    view! {
        <div class="grid gap-1">
            <div class="flex items-center gap-3">
                <ui::Switch id="set-shortcuts" checked=on/>
                <label for="set-shortcuts" class="text-sm font-medium">{t("settings-shortcuts")}</label>
            </div>
            <p class="text-sm text-muted-foreground">{t("settings-shortcuts-hint")}</p>
        </div>
    }
}

#[component]
fn PasswordForm() -> impl IntoView {
    let current = RwSignal::new(String::new());
    let new = RwSignal::new(String::new());
    let message = RwSignal::new(None::<Result<String, String>>);
    let i18n = use_i18n();
    let save = Action::new(move |(): &()| {
        let (c, n) = (current.get_untracked(), new.get_untracked());
        let i18n = i18n.clone();
        async move {
            let r = admin_api::change_password(c, n).await;
            message.set(Some(match r {
                Ok(()) => {
                    current.set(String::new());
                    new.set(String::new());
                    Ok(i18n.t("settings-password-changed"))
                }
                Err(e) => Err(admin_error(&e)),
            }));
        }
    });
    view! {
        <form class="grid max-w-md gap-4" on:submit=move |ev| { ev.prevent_default(); save.dispatch(()); }>
            <h3 class="font-semibold">{t("settings-password")}</h3>
            <FormField id="set-pw-current" label=t("settings-password-current")>
                <Input id="set-pw-current" r#type="password" value=current autocomplete="current-password"
                    described_by=field_described_by("set-pw-current")/>
            </FormField>
            <FormField id="set-pw-new" label=t("settings-password-new") description=t("settings-password-rule")>
                <Input id="set-pw-new" r#type="password" value=new autocomplete="new-password"
                    described_by=field_described_by("set-pw-new")/>
            </FormField>
            <Outcome message=message/>
            <Button r#type="submit" class="w-fit" loading=save.pending()>{t("settings-password-save")}</Button>
        </form>
    }
}

/// An inline outcome: `Ok` as a polite status, `Err` as an alert (D15: no dialogs).
#[component]
pub fn Outcome(message: RwSignal<Option<Result<String, String>>>) -> impl IntoView {
    view! {
        {move || message.get().map(|m| match m {
            Ok(text) => view! { <Alert variant=AlertVariant::Success>{text}</Alert> }.into_any(),
            Err(text) => view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{text}</Alert> }.into_any(),
        })}
    }
}

/// TOTP: start (gated, DEC-191), confirm with a first code, then the recovery codes once
/// (OD-08).
#[component]
fn Authenticator() -> impl IntoView {
    let reauth = use_reauth();
    let status = Resource::new(move || reauth.version.get(), |_| admin_api::reauth_status());
    let enrolment = RwSignal::new(None::<contracts::admin::TotpEnrolmentDto>);
    let codes = RwSignal::new(None::<Vec<String>>);
    let message = RwSignal::new(None::<Result<String, String>>);
    let code = RwSignal::new(String::new());
    let begin = Action::new(move |(): &()| async move {
        match admin_api::totp_begin().await {
            Ok(e) => {
                message.set(None);
                enrolment.set(Some(e));
            }
            Err(e) => {
                let retry = Callback::new(move |()| {
                    begin_again(enrolment, message);
                });
                if !reauth.intercept(&e, retry) {
                    message.set(Some(Err(admin_error(&e))));
                }
            }
        }
    });
    let confirm = Action::new(move |(): &()| {
        let c = code.get_untracked();
        async move {
            match admin_api::totp_confirm(c).await {
                Ok(r) => {
                    enrolment.set(None);
                    code.set(String::new());
                    codes.set(Some(r.codes));
                    message.set(None);
                    status.refetch();
                }
                Err(e) => message.set(Some(Err(admin_error(&e)))),
            }
        }
    });
    let on = move || {
        status
            .get()
            .and_then(Result::ok)
            .is_some_and(|s| s.has_totp)
    };
    view! {
        <section class="grid max-w-xl gap-4" aria-labelledby="set-totp-h">
            <h3 id="set-totp-h" class="font-semibold">{t("settings-totp")}</h3>
            <Suspense fallback=|| ()>
                <p class="text-sm">{move || if on() { t("settings-totp-on") } else { t("settings-totp-off") }}</p>
            </Suspense>
            <Outcome message=message/>
            {move || codes.get().map(|c| view! { <RecoveryCodes codes=c/> })}
            {move || match enrolment.get() {
                None => view! {
                    <Button variant=ButtonVariant::Outline class="w-fit" loading=begin.pending()
                        on:click=move |_| { begin.dispatch(()); }>
                        {t("settings-totp-start")}
                    </Button>
                }.into_any(),
                Some(e) => view! {
                    <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); confirm.dispatch(()); }>
                        <p class="text-sm">{t("settings-totp-scan")}</p>
                        <FormField id="set-totp-secret" label=t("settings-totp-secret")>
                            <CopyField id="set-totp-secret" value=Signal::stored(e.secret_base32.clone())
                                copy_label=t("settings-copy") copied_label=t("settings-copied")/>
                        </FormField>
                        <FormField id="set-totp-url" label=t("settings-totp-link")>
                            <CopyField id="set-totp-url" value=Signal::stored(e.otpauth_url.clone())
                                copy_label=t("settings-copy") copied_label=t("settings-copied")/>
                        </FormField>
                        <FormField id="set-totp-code" label=t("settings-totp-code")>
                            <Input id="set-totp-code" value=code inputmode="numeric" autocomplete="one-time-code"
                                described_by=field_described_by("set-totp-code")/>
                        </FormField>
                        <Button r#type="submit" class="w-fit" loading=confirm.pending()>{t("settings-totp-confirm")}</Button>
                    </form>
                }.into_any(),
            }}
        </section>
    }
}

/// The retry after the re-auth dialog: the same call again.
fn begin_again(
    enrolment: RwSignal<Option<contracts::admin::TotpEnrolmentDto>>,
    message: RwSignal<Option<Result<String, String>>>,
) {
    let i18n = use_i18n();
    leptos::task::spawn_local(async move {
        match admin_api::totp_begin().await {
            Ok(e) => enrolment.set(Some(e)),
            Err(e) => message.set(Some(Err(admin_error_with(&i18n, &e)))),
        }
    });
}

#[component]
fn RecoveryCodes(codes: Vec<String>) -> impl IntoView {
    let all = codes.join("\n");
    view! {
        <section class="grid gap-3 rounded-lg border border-warning/35 bg-warning-soft p-4" aria-labelledby="set-codes-h">
            <h4 id="set-codes-h" class="font-semibold">{t("settings-codes-title")}</h4>
            <p class="text-sm">{t("settings-codes-lead")}</p>
            <ol data-testid="recovery-codes" class="grid grid-cols-2 gap-1 font-mono text-sm">
                {codes.into_iter().map(|c| view! { <li>{c}</li> }).collect_view()}
            </ol>
            <FormField id="set-codes-all" label=t("settings-codes-copy")>
                <CopyField id="set-codes-all" value=Signal::stored(all.replace('\n', " "))
                    copy_label=t("settings-copy") copied_label=t("settings-copied")
                    shown_once=t("settings-codes-once")/>
            </FormField>
        </section>
    }
}

// ---------------------------------------------------------------- data sources (member)

/// Settings > Data sources for everyone (design.md 6.10): what works, what needs their own
/// account, what is missing with "Ask admin"; the attribution list last.
#[component]
pub fn SettingsSources() -> impl IntoView {
    let status = Resource::new(|| (), |()| admin_api::source_status());
    let mine = Resource::new(|| (), |()| admin_api::my_credentials());
    let attribution = Resource::new(|| (), |()| crate::api::sources());
    view! {
        <SettingsLayout>
            <PageTitle text=t("nav-sources")/>
            <Lead text=t("settings-sources-lead")/>
            {loaded(status, move |page| {
                let admin = page.is_admin;
                view! {
                    <GroupsWithout groups=page.groups_without_source.clone() admin=admin/>
                    <ul data-testid="member-sources" class="mb-8 grid max-w-3xl gap-3" aria-label=t("nav-sources")>
                        {page.sources.into_iter().map(|s| view! { <MemberSource s=s admin=admin mine=mine/> }).collect_view()}
                    </ul>
                }
            })}
            <section id="attribution" aria-labelledby="set-attr-h" class="max-w-3xl">
                <h2 id="set-attr-h" class="mb-2 text-lg font-semibold">{t("sources-title")}</h2>
                <p class="mb-4 text-sm text-muted-foreground">{t("sources-lead")}</p>
                <Suspense fallback=|| ()>
                    {move || attribution.get().and_then(Result::ok).map(|list| view! {
                        <ul class="grid gap-3">
                            {list.into_iter().map(|s| view! {
                                <li class="rounded-md border border-border p-3 text-sm">
                                    <p class="font-medium">{s.name.clone()}</p>
                                    <p lang="en">{s.text.clone()}</p>
                                    <p class="text-muted-foreground"><span lang="en">{s.licence.clone()}</span> " - " <a class="text-primary underline" href=s.url.clone() rel="noopener">{s.url.clone()}</a></p>
                                </li>
                            }).collect_view()}
                        </ul>
                    })}
                </Suspense>
            </section>
        </SettingsLayout>
    }
}

/// "Ask admin" for a source or a whole kind group; the answer is inline.
fn ask_action() -> Action<(Option<String>, Option<String>), Result<(), String>> {
    Action::new(|(source, group): &(Option<String>, Option<String>)| {
        let (source, group) = (source.clone(), group.clone());
        async move {
            admin_api::ask_admin(source, group)
                .await
                .map_err(|e| admin_error(&e))
        }
    })
}

#[component]
pub(crate) fn AskAdmin(
    source: Option<String>,
    group: Option<String>,
    #[prop(into)] name: String,
) -> impl IntoView {
    let ask = ask_action();
    let label = use_i18n().t_args("settings-ask-admin-named", &[("name", name.into())]);
    view! {
        {move || match ask.value().get() {
            Some(Ok(())) => view! { <p role="status" class="text-sm text-success">{t("settings-asked")}</p> }.into_any(),
            other => {
                let (source, group) = (source.clone(), group.clone());
                view! {
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=label.clone()
                        loading=ask.pending() on:click=move |_| { ask.dispatch((source.clone(), group.clone())); }>
                        {t("settings-ask-admin")}
                    </Button>
                    {other.and_then(Result::err).map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
                }.into_any()
            }
        }}
    }
}

#[component]
fn GroupsWithout(groups: Vec<String>, admin: bool) -> impl IntoView {
    (!groups.is_empty()).then(|| {
        view! {
            <Alert variant=AlertVariant::Info role=AlertRole::None class="mb-6 max-w-3xl" title=t("settings-sources-missing")>
                <ul class="grid gap-2">
                    {groups.into_iter().map(|g| {
                        let name = t(&format!("kind-group-{g}"));
                        let line = use_i18n().t_args("settings-sources-no-source", &[("group", name.clone().into())]);
                        view! {
                            <li class="flex flex-wrap items-center gap-2">
                                <span class="flex-1">{line}</span>
                                {if admin {
                                    view! { <a class="text-sm underline" href="/admin/sources">{t("settings-sources-set-up")}</a> }.into_any()
                                } else {
                                    view! { <AskAdmin source=None group=Some(g.clone()) name=name/> }.into_any()
                                }}
                            </li>
                        }
                    }).collect_view()}
                </ul>
            </Alert>
        }
    })
}

#[component]
fn MemberSource(
    s: SourceStatusDto,
    admin: bool,
    mine: Resource<Result<Vec<UserCredentialDto>, ServerFnError>>,
) -> impl IntoView {
    let state = s.state.clone();
    let label = t(&format!("source-state-{}", s.state));
    let kinds = s
        .kinds
        .iter()
        .map(|k| kind_label(k))
        .collect::<Vec<_>>()
        .join(", ");
    let ask = ask_admin_for(&s.state);
    let has_fields = !s.user_fields.is_empty();
    let id = s.id.clone();
    // lives here, not in the credential form: a refetch of `mine` renders that form afresh
    let message = RwSignal::new(None::<Result<String, String>>);
    view! {
        <li class="grid gap-3 rounded-xl border bg-card p-4">
            <div class="flex flex-wrap items-center gap-x-4 gap-y-1">
                <span class="font-semibold">{s.name.clone()}</span>
                <StatusDot state=source_state(&state) label=label/>
                <span class="basis-full text-sm text-muted-foreground">{kinds}</span>
            </div>
            {(ask && !has_fields).then(|| if admin {
                view! { <a class="w-fit text-sm text-primary underline" href="/admin/sources">{t("settings-sources-set-up")}</a> }.into_any()
            } else {
                view! { <AskAdmin source=Some(id.clone()) group=None name=s.name.clone()/> }.into_any()
            })}
            {has_fields.then(|| view! {
                <Suspense fallback=|| ()>
                    {move || mine.get().and_then(Result::ok).and_then(|list| list.into_iter().find(|c| c.source == id)).map(|c| view! {
                        <MyCredential c=c mine=mine message=message/>
                    })}
                </Suspense>
            })}
        </li>
    }
}

/// The member's own credential for a source (`TheTVDB` PIN): write-only (addendum-1 2.3).
#[component]
fn MyCredential(
    c: UserCredentialDto,
    mine: Resource<Result<Vec<UserCredentialDto>, ServerFnError>>,
    message: RwSignal<Option<Result<String, String>>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let editing = RwSignal::new(false);
    let values: Vec<(String, RwSignal<String>)> = c
        .fields
        .iter()
        .map(|f| (f.clone(), RwSignal::new(String::new())))
        .collect();
    let stored = c.set.then(|| {
        if c.locked {
            i18n.t("settings-credential-locked")
        } else {
            let when = c
                .set_at
                .as_deref()
                .map(|d| i18n.date(d))
                .unwrap_or_default();
            i18n.t_args("settings-credential-set", &[("date", when.into())])
        }
    });
    let source = c.source.clone();
    let fields = values.clone();
    let save = Action::new(move |(): &()| {
        let body: BTreeMap<String, String> = fields
            .iter()
            .map(|(k, v)| (k.clone(), v.get_untracked()))
            .collect();
        let source = source.clone();
        async move {
            match admin_api::set_my_credential(source, body).await {
                Ok(_) => {
                    editing.set(false);
                    message.set(Some(Ok(t("settings-credential-saved"))));
                    mine.refetch();
                }
                Err(e) => message.set(Some(Err(admin_error(&e)))),
            }
        }
    });
    let source = c.source.clone();
    let clear = Action::new(move |(): &()| {
        let source = source.clone();
        async move {
            match admin_api::clear_my_credential(source).await {
                Ok(_) => mine.refetch(),
                Err(e) => message.set(Some(Err(admin_error(&e)))),
            }
        }
    });
    let is_set = c.set;
    let show_save = move || !is_set || editing.get();
    view! {
        <form class="grid max-w-md gap-3" on:submit=move |ev| { ev.prevent_default(); save.dispatch(()); }>
            {values.iter().map(|(f, v)| {
                let field_id = format!("set-cred-{}-{f}", c.source);
                let label = t(&format!("admin-source-field-{f}"));
                let replace = i18n.t_args("settings-replace-named", &[("field", label.clone().into())]);
                view! {
                    <SecretField id=field_id label=label value=*v editing=editing
                        stored=stored.clone() replace_label=t("settings-replace") replace_name=replace cancel_label=t("reauth-cancel")
                        description=t("settings-credential-hint")/>
                }
            }).collect_view()}
            <Outcome message=message/>
            <div class="flex flex-wrap gap-2">
                <Show when=show_save>
                    <Button r#type="submit" size=ButtonSize::Sm loading=save.pending()>{t("settings-credential-save")}</Button>
                </Show>
                {is_set.then(|| view! {
                    <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on:click=move |_| { clear.dispatch(()); }>
                        {t("settings-credential-clear")}
                    </Button>
                })}
            </div>
        </form>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use super::*;
    use crate::i18n::I18n;

    /// LR-3: an equal answer keeps the render (its key), another value or an error gets a
    /// new one, and an answer after an error is rendered anew even when it equals the last
    /// value.
    #[test]
    fn an_equal_answer_keeps_the_key_of_its_render() {
        let mut n = 0_u32;
        let mut next = || {
            n += 1;
            n
        };
        let mut last = None::<(u32, Vec<&str>)>;
        let ok = |v: &[&'static str]| Ok::<_, String>(v.to_vec());
        let first = render_key(&mut last, &ok(&["Zed"]), &mut next);
        assert_eq!(render_key(&mut last, &ok(&["Zed"]), &mut next), first);
        let other = render_key(&mut last, &ok(&["Zed", "Yan"]), &mut next);
        assert_ne!(other, first);
        assert_eq!(
            render_key(&mut last, &ok(&["Zed", "Yan"]), &mut next),
            other
        );
        let failed = render_key(&mut last, &Err("down".to_string()), &mut next);
        assert_ne!(failed, other);
        let again = render_key(&mut last, &ok(&["Zed", "Yan"]), &mut next);
        assert_ne!(again, failed);
        assert_ne!(again, other);
    }

    /// The e2e failure of "a gated change ... is retried once confirmed": the save's
    /// `done` sets "Saved." and refetches, and the fresh render of the value (UX1-2) must
    /// find the same message instead of a new, empty one.
    #[test]
    fn kept_state_survives_a_fresh_render_of_the_loaded_value() {
        let owner = Owner::new();
        owner.with(|| {
            let keep = Kept::new();
            let first = Owner::new();
            first.with(|| {
                provide_context(keep.clone());
                kept("sessions", || None::<String>).set(Some("Saved.".into()));
            });
            first.cleanup();
            let second = Owner::new();
            let again = second.with(|| {
                provide_context(keep.clone());
                kept("sessions", || None::<String>)
            });
            assert_eq!(again.get_untracked().as_deref(), Some("Saved."));
            // another key starts afresh; without a `loaded` around, state is plain
            let other = second.with(|| kept("secrets", || 1_u8));
            assert_eq!(other.get_untracked(), 1);
            assert_eq!(kept("x", || 2_u8).get_untracked(), 2);
        });
    }

    #[test]
    fn members_see_their_pages_and_admins_also_the_instance_group() {
        let owner = Owner::new();
        owner.with(|| {
            provide_context(Arc::new(I18n::new("en", BTreeMap::new())));
            let member = nav_groups(false);
            assert_eq!(member.len(), 1);
            assert_eq!(member[0].heading.as_deref(), Some("You"));
            assert_eq!(member[0].items.len(), 6);
            assert_eq!(member[0].items[0].label, "Account");
            let admin = nav_groups(true);
            assert_eq!(admin.len(), 2);
            assert_eq!(admin[1].heading.as_deref(), Some("Instance (admin)"));
            let labels: Vec<_> = admin[1].items.iter().map(|i| i.label.as_str()).collect();
            assert_eq!(
                labels,
                [
                    "Overview",
                    "Instance",
                    "Sign-in and SSO",
                    "Users",
                    "Data sources",
                    "Notification defaults",
                    "Storage and jobs",
                    "Audit log"
                ]
            );
        });
    }

    #[test]
    fn phase_2_errors_use_their_own_words_then_the_sign_in_words() {
        let i18n = I18n::new("en", BTreeMap::new());
        let e = ServerFnError::new("last_admin");
        assert_eq!(
            admin_error_with(&i18n, &e),
            "This is the last admin. Make someone else admin first."
        );
        let e = ServerFnError::new("invalid_credentials");
        assert_eq!(admin_error_with(&i18n, &e), "Wrong username or password.");
        let e = ServerFnError::new("no_such_code");
        assert_eq!(
            admin_error_with(&i18n, &e),
            "Something went wrong. Please try again."
        );
    }
}
