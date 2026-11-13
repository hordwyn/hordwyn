//! The app shell (design.md 3.2, 6.15, 7.3; replaces the old `Header`): skip link, top bar
//! with the five sections, search pill, Add, bell and account menu; the mobile tab bar with
//! the You hub; the offline banner; toaster, search palette and shortcut help mounted once.
//! What the bar shows comes from [`ClientConfig`], filled from the session on the server, so
//! there is no Suspense gap and no "Create account" after sign-in.

use leptos::html;
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use ui::{
    Alert, AlertRole, AlertVariant, Avatar, Button, ButtonLink, ButtonVariant, Dialog, Icon,
    IconName, KeyHint, NavGroup, NavItem, Segmented, SegmentedOption, SideNav, TabBar,
    TabBarAction, ThemeToggle, Toaster, provide_toasts,
};

use crate::app::ClientConfig;
use crate::i18n::{LOCALES, t, use_i18n};
use crate::keyboard::{CHORD_MS, GO_KEYS, Sep, ShortcutsOn, go_target};
use crate::palette::PaletteOpen;
use crate::sign_out::SIGN_OUT_FORM;

/// `localStorage` key of the theme choice (unchanged; read before paint by `THEME_BOOT`).
pub const THEME_KEY: &str = "app-theme";

/// The theme choice: `system`, `light` or `dark`.
#[derive(Clone, Copy)]
pub struct ThemeChoice(pub RwSignal<String>);

/// Whether the keyboard shortcut help is open.
#[derive(Clone, Copy)]
pub struct ShortcutsOpen(pub RwSignal<bool>);

/// The section a path belongs to; its links get `aria-current="page"`. Pages reached from
/// the account menu or the You hub belong to `/you` (the mobile tab): Stats too, which the
/// top bar used to mark as Home (v1-polish P1). The account menu marks its own entry.
#[must_use]
pub fn section_of(path: &str) -> &'static str {
    match path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default()
    {
        "" => "/",
        "library" | "items" => "/library",
        "history" => "/history",
        "lists" | "shared" => "/lists",
        "people" => "/people",
        "you" | "stats" | "settings" | "admin" => "/you",
        _ => "",
    }
}

/// How wide a page may get on a large screen (owner test OT-14, DEC-541). Forms, the wizard
/// and prose keep their own, narrower measure inside either.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageWidth {
    /// Grids and dashboards (Home, Library, Lists, People, Stats): 96 rem, more columns.
    Grid,
    /// Records and streams (an item, History, one list, one person), settings and admin:
    /// 80 rem.
    Record,
}

impl PageWidth {
    #[must_use]
    pub const fn class(self) -> &'static str {
        match self {
            Self::Grid => "max-w-[96rem]",
            Self::Record => "max-w-7xl",
        }
    }
}

/// The width of the page at `path`: the five grid pages are wide, everything below them
/// (`/lists/<id>`, `/people/<id>`) and every other page is a record page.
#[must_use]
pub fn page_width(path: &str) -> PageWidth {
    match path.trim_matches('/') {
        "" | "library" | "lists" | "people" | "stats" => PageWidth::Grid,
        _ => PageWidth::Record,
    }
}

/// The admin entry of the account menu and the You hub leads to Admin > Overview
/// (addendum-1 3.4).
const ADMIN_HREF: &str = "/admin";

/// Notifications live in the settings area (design.md 6.8); the bell links there.
const NOTIFICATIONS_HREF: &str = "/settings/notifications";

/// The entry of the account menu a path belongs to: the page itself, or the area it is in
/// (`/settings/...`, `/admin/...`). Notifications are an entry of their own inside the
/// settings area, with every page below them.
#[must_use]
pub fn account_entry(path: &str) -> Option<&'static str> {
    let path = path.trim_end_matches('/');
    if path
        .strip_prefix(NOTIFICATIONS_HREF)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    {
        return Some(NOTIFICATIONS_HREF);
    }
    match path.trim_start_matches('/').split('/').next()? {
        "stats" => Some("/stats"),
        "settings" => Some("/settings/account"),
        "admin" => Some(ADMIN_HREF),
        _ => None,
    }
}

/// The five sections of the top bar (design.md 3.2, 3.3); all routes exist.
fn sections() -> Vec<(&'static str, &'static str)> {
    vec![
        ("/", "nav-home"),
        ("/library", "nav-library"),
        ("/history", "nav-history"),
        ("/lists", "nav-lists"),
        ("/people", "nav-people"),
    ]
}

/// Account pages that are not top-level sections: notifications, stats, the settings area
/// and, for admins, the admin area.
fn account_links(is_admin: bool) -> Vec<(&'static str, &'static str)> {
    let mut links = vec![
        (NOTIFICATIONS_HREF, "nav-notifications"),
        ("/stats", "nav-stats"),
        ("/settings/account", "nav-settings"),
    ];
    if is_admin {
        links.push((ADMIN_HREF, "shell-admin"));
    }
    links
}

// ---------------------------------------------------------------- browser helpers

#[cfg(feature = "hydrate")]
fn stored_theme() -> Option<String> {
    web_sys::window()?
        .local_storage()
        .ok()??
        .get_item(THEME_KEY)
        .ok()?
        .filter(|v| v == "light" || v == "dark")
}

/// Store the choice (`system` removes the key) and set the `dark` class accordingly.
#[cfg(feature = "hydrate")]
fn apply_theme(choice: &str) {
    let Some(w) = web_sys::window() else {
        return;
    };
    if let Ok(Some(s)) = w.local_storage() {
        let _ = if choice == "system" {
            s.remove_item(THEME_KEY)
        } else {
            s.set_item(THEME_KEY, choice)
        };
    }
    let dark = match choice {
        "dark" => true,
        "light" => false,
        _ => w
            .match_media("(prefers-color-scheme: dark)")
            .ok()
            .flatten()
            .is_some_and(|m| m.matches()),
    };
    if let Some(root) = w.document().and_then(|d| d.document_element()) {
        let _ = if dark {
            root.class_list().add_1("dark")
        } else {
            root.class_list().remove_1("dark")
        };
    }
}

#[cfg(not(feature = "hydrate"))]
fn apply_theme(_: &str) {}

#[cfg(feature = "hydrate")]
fn is_online() -> bool {
    web_sys::window().is_none_or(|w| w.navigator().on_line())
}

/// Is the key press meant for a text field (single-key shortcuts stay off there)?
#[cfg(feature = "hydrate")]
pub(crate) fn typing(ev: &leptos::ev::KeyboardEvent) -> bool {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
        .is_some_and(|el| {
            matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                || el.is_content_editable()
        })
}

#[cfg(not(feature = "hydrate"))]
pub(crate) fn typing(_: &leptos::ev::KeyboardEvent) -> bool {
    false
}

/// Move focus between the menu's items for an arrow, Home or End key; `false` for others.
#[cfg(feature = "hydrate")]
fn focus_menu(menu: NodeRef<html::Div>, key: &str) -> bool {
    use wasm_bindgen::JsCast;
    let Some(menu) = menu.get_untracked() else {
        return false;
    };
    let Ok(list) = menu.query_selector_all("[role^=menuitem]") else {
        return false;
    };
    let items: Vec<web_sys::HtmlElement> = (0..list.length())
        .filter_map(|i| list.item(i))
        .filter_map(|n| n.dyn_into().ok())
        .collect();
    let n = items.len();
    if n == 0 {
        return false;
    }
    let active = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element());
    let at = items.iter().position(|el| {
        active
            .as_ref()
            .is_some_and(|a| a.is_same_node(Some(el.as_ref())))
    });
    let next = match key {
        "Home" => 0,
        "End" => n - 1,
        "ArrowDown" => at.map_or(0, |i| (i + 1) % n),
        "ArrowUp" => at.map_or(n - 1, |i| (i + n - 1) % n),
        _ => return false,
    };
    let _ = items[next].focus();
    true
}

#[cfg(not(feature = "hydrate"))]
fn focus_menu(_: NodeRef<html::Div>, _: &str) -> bool {
    false
}

/// Did the pointer go down outside `container`?
#[cfg(feature = "hydrate")]
fn outside(container: NodeRef<html::Div>, ev: &leptos::ev::PointerEvent) -> bool {
    use wasm_bindgen::JsCast;
    let target = ev.target().and_then(|t| t.dyn_into::<web_sys::Node>().ok());
    container
        .get_untracked()
        .is_some_and(|c| !c.contains(target.as_ref()))
}

#[cfg(not(feature = "hydrate"))]
fn outside(_: NodeRef<html::Div>, _: &leptos::ev::PointerEvent) -> bool {
    false
}

// ---------------------------------------------------------------- shell

fn provide_theme() -> ThemeChoice {
    let choice = RwSignal::new("system".to_string());
    // read the stored choice once in the browser (the server cannot know it)
    Effect::new(move |_| {
        #[cfg(feature = "hydrate")]
        if let Some(v) = stored_theme() {
            choice.set(v);
        }
    });
    // apply later changes (the first run is the initial value, already applied before paint)
    Effect::new(move |prev: Option<()>| {
        let v = choice.get();
        if prev.is_some() {
            apply_theme(&v);
        }
    });
    let theme = ThemeChoice(choice);
    provide_context(theme);
    theme
}

/// The chord's state: the machine, the hint shown while it waits, and the timer that ends it.
#[derive(Clone, Copy)]
struct ChordUi {
    chord: StoredValue<ui::Chord>,
    hint: RwSignal<String>,
    timer: StoredValue<Option<TimeoutHandle>>,
}

impl ChordUi {
    fn new() -> Self {
        Self {
            chord: StoredValue::new(ui::Chord::new('g', GO_KEYS, CHORD_MS)),
            hint: RwSignal::new(String::new()),
            timer: StoredValue::new(None),
        }
    }

    fn stop(self) {
        self.chord.update_value(ui::Chord::cancel);
        self.hint.set(String::new());
        if let Some(h) = self.timer.get_value() {
            h.clear();
        }
        self.timer.set_value(None);
    }

    /// Show the hint and end the chord when its window runs out.
    fn wait(self, text: String) {
        self.hint.set(text);
        if let Some(h) = self.timer.get_value() {
            h.clear();
        }
        let h = set_timeout_with_handle(
            move || self.stop(),
            std::time::Duration::from_secs_f64(CHORD_MS / 1000.0),
        );
        self.timer.set_value(h.ok());
    }
}

/// Global single-key shortcuts (design.md 3.2): `?` help, `N` add, `G then H/L/Y/T/P` go to a
/// section (signed in), with the hint in a polite live region and focus on the new page's
/// heading. Never while typing, in a dialog or menu, with a modifier, or with the shortcuts
/// switched off (Settings > Account, WCAG 2.1.4).
fn shortcut_keys(signed_in: bool, palette: RwSignal<bool>, help: RwSignal<bool>) -> ChordUi {
    let on = expect_context::<ShortcutsOn>().0;
    let ui = ChordUi::new();
    let nav = StoredValue::new(leptos_router::hooks::use_navigate());
    let location = use_location();
    let waiting = use_i18n().t("shortcuts-chord-waiting");
    let _ = window_event_listener(leptos::ev::keydown, move |ev| {
        if !crate::keyboard::plain_key(&ev, on) {
            ui.stop();
            return;
        }
        let key = ev.key();
        let step = if signed_in {
            let mut step = ui::ChordStep::Ignored;
            ui.chord
                .update_value(|c| step = c.key(&key, crate::keyboard::now_ms()));
            step
        } else {
            ui::ChordStep::Ignored
        };
        match step {
            ui::ChordStep::Started => {
                ev.prevent_default();
                ui.wait(waiting.clone());
                return;
            }
            ui::ChordStep::Done(c) => {
                ev.prevent_default();
                ui.stop();
                if let Some((path, _)) = go_target(c) {
                    let same = location.pathname.get_untracked() == path;
                    nav.with_value(|n| n(path, leptos_router::NavigateOptions::default()));
                    crate::keyboard::focus_heading(same);
                }
                return;
            }
            ui::ChordStep::Cancelled => ui.stop(),
            ui::ChordStep::Ignored => {}
        }
        match key.as_str() {
            "?" => {
                ev.prevent_default();
                help.set(true);
            }
            "n" | "N" if signed_in => {
                ev.prevent_default();
                palette.set(true);
            }
            _ => {}
        }
    });
    ui
}

/// Everything around the routes. Must sit inside the `Router`.
#[component]
pub fn AppShell(children: Children) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    let palette = expect_context::<PaletteOpen>().0;
    let help = RwSignal::new(false);
    provide_context(ShortcutsOpen(help));
    let _ = provide_toasts();
    provide_context(crate::reauth::Reauth::new());
    provide_theme();
    let online = provide_online();
    // the offline queue and the funnel (WP4-5) belong to a signed-in person
    let waiting = cfg
        .signed_in
        .then(|| crate::offline::provide_offline(cfg.username.clone(), online).waiting);
    crate::funnel::provide_funnel(cfg.signed_in && cfg.usage_funnel);
    crate::keyboard::provide_shortcuts();
    let chord = shortcut_keys(cfg.signed_in, palette, help);
    let location = use_location();
    let current = Signal::derive(move || section_of(&location.pathname.get()).to_string());
    let width = Signal::derive(move || page_width(&location.pathname.get()));
    let main_class = move || {
        format!(
            "mx-auto w-full {} px-4 pt-6 pb-[calc(1.5rem+var(--tab-bar-height))] focus:outline-none",
            width.get().class()
        )
    };
    view! {
        <a href="#main" class="sr-only focus:not-sr-only focus:fixed focus:left-4 focus:top-4 focus:z-[60] focus:rounded focus:bg-background focus:p-2 focus:ring-2 focus:ring-ring">
            {t("skip-to-content")}
        </a>
        <TopBar current=current/>
        <OfflineBanner online=online waiting=waiting width=width/>
        <main id="main" tabindex="-1" class=main_class>
            {children()}
        </main>
        {cfg.signed_in.then(|| view! { <MobileTabs current=current/> <crate::reauth::ReauthDialog/> <crate::sign_out::SignOutForm/> })}
        <crate::palette::SearchPalette/>
        <ShortcutHelp open=help/>
        <ChordHint hint=chord.hint/>
        <Toaster dismiss_label=i18n.t("shell-dismiss")/>
    }
}

/// The unread count as the bell shows it (design.md 6.8): nothing for none, capped at
/// `9+`.
#[must_use]
pub fn unread_label(n: u32) -> Option<String> {
    match n {
        0 => None,
        1..=9 => Some(n.to_string()),
        _ => Some("9+".into()),
    }
}

/// The count for the unread messages (`shell-notifications`, `shell-unread`): the number up
/// to 9, else `9+` like the bell.
fn unread_arg(n: u32) -> fluent_bundle::FluentValue<'static> {
    if n > 9 { "9+".into() } else { n.into() }
}

const ICON_BTN: &str = "relative inline-flex size-11 shrink-0 items-center justify-center rounded-md \
    hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring \
    min-[47.5rem]:size-9 cursor-pointer";

/// The desktop-only Add button (the tab bar has its own on mobile).
const ADD_BTN: &str = "relative hidden size-9 shrink-0 items-center justify-center rounded-md \
    hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring \
    min-[47.5rem]:inline-flex cursor-pointer";

const NAV_LINK: &str = "rounded-md px-3 py-2 text-sm font-medium text-muted-foreground \
    hover:bg-accent hover:text-accent-foreground focus-visible:outline-none focus-visible:ring-2 \
    focus-visible:ring-ring aria-[current=page]:bg-primary-soft aria-[current=page]:text-primary";

#[component]
fn TopBar(current: Signal<String>) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    view! {
        <header class="sticky top-0 z-30 border-b border-border bg-background/95 backdrop-blur">
            <div class="mx-auto flex h-[var(--top-bar-height)] max-w-[96rem] items-center gap-2 px-4">
                <a href="/" class="mr-2 rounded-md text-lg font-semibold tracking-tight focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                    {cfg.product_name.clone()}
                </a>
                {if cfg.signed_in {
                    view! { <SignedInBar current=current/> }.into_any()
                } else {
                    view! { <SignedOutBar/> }.into_any()
                }}
            </div>
        </header>
    }
}

#[component]
fn SignedOutBar() -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <span class="flex-1"></span>
        <ThemeToggle dark_label=i18n.t("theme-dark") light_label=i18n.t("theme-light") storage_key=THEME_KEY/>
        <a href="/sign-in" class=NAV_LINK>{t("nav-sign-in")}</a>
        <ButtonLink href="/sign-up" class="min-h-11 min-[47.5rem]:min-h-9">{t("nav-sign-up")}</ButtonLink>
    }
}

#[component]
fn SignedInBar(current: Signal<String>) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    let palette = expect_context::<PaletteOpen>().0;
    let bell = i18n.t_args("shell-notifications", &[("count", unread_arg(cfg.unread))]);
    let badge = unread_label(cfg.unread);
    view! {
        <nav aria-label=i18n.t("nav-main") class="hidden flex-1 items-center gap-0.5 min-[47.5rem]:flex">
            {sections().into_iter().map(|(href, key)| view! {
                <a href=href class=NAV_LINK aria-current=move || (current.get() == href).then_some("page")>
                    {t(key)}
                </a>
            }).collect_view()}
        </nav>
        <span class="flex-1 min-[47.5rem]:hidden"></span>
        // 10 rem wide below 52 rem, and narrower when the bar is full: the label is cut then
        // (review OT2F-10: with the German labels the bar was 11 px wider than a window of
        // 760 px)
        <button
            type="button"
            on:click=move |_| palette.set(true)
            class="inline-flex size-11 items-center justify-center gap-2 rounded-md text-sm text-muted-foreground hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer \
                   min-[47.5rem]:h-9 min-[47.5rem]:w-40 min-[47.5rem]:min-w-28 min-[52rem]:w-auto min-[52rem]:min-w-52 min-[47.5rem]:justify-start min-[47.5rem]:border min-[47.5rem]:border-input min-[47.5rem]:px-3"
        >
            <Icon name=IconName::Search/>
            <span class="min-w-0 truncate max-[47.5rem]:sr-only">{t("shell-search")}</span>
            // the key hint from 52 rem: with it the bar was wider than a window of 760 to
            // 790 px (OT-19: the page scrolled sideways there)
            <span class="ml-auto hidden min-[52rem]:inline"><KeyHint keys=vec![t("shell-ctrl"), "K".to_string()]/></span>
        </button>
        <button
            type="button"
            aria-label=i18n.t("shell-add")
            title=i18n.t("shell-add")
            on:click=move |_| palette.set(true)
            class=ADD_BTN
        >
            <Icon name=IconName::Plus class="size-5"/>
        </button>
        <a href=NOTIFICATIONS_HREF aria-label=bell class=ICON_BTN>
            <Icon name=IconName::Bell class="size-5"/>
            {badge.map(|n| view! {
                <span aria-hidden="true" class="absolute right-0.5 top-0.5 min-w-4 rounded-full border border-background bg-destructive px-1 text-center text-[0.625rem] font-semibold leading-4 text-destructive-foreground">{n}</span>
            })}
        </a>
        <div class="hidden min-[47.5rem]:block">
            <AccountMenu/>
        </div>
    }
}

/// An entry of the account menu. The entry of the page one is on (`aria-current="page"`) is
/// heavier and tinted like the current section of the top bar (review POL-1: on Stats,
/// Settings and Admin the top bar marks nothing); with the focus on it, it looks focused and
/// keeps the weight.
const MENU_ITEM: &str = "flex w-full min-h-9 cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 \
    text-left text-sm outline-none hover:bg-accent focus:bg-accent focus:text-accent-foreground \
    aria-[current=page]:bg-primary-soft aria-[current=page]:font-semibold \
    aria-[current=page]:text-primary aria-[current=page]:focus:bg-accent \
    aria-[current=page]:focus:text-accent-foreground";

const SEPARATOR: &str = "my-1 h-px bg-border";

/// Element id of the account menu button (the shortcut help returns focus to it).
const ACCOUNT_TRIGGER: &str = "account-menu-trigger";

/// The account menu (APG menu button): pages, theme (system/light/dark), language, sign out,
/// and help last (addendum-1 D13). Enter, Space and ArrowDown open it on the first item,
/// ArrowUp on the last; Escape, Tab and every item that stays on the page close it with focus
/// back on the button.
#[component]
fn AccountMenu() -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    // the item that takes focus when the menu opens: `Home` (first) or `End` (last)
    let start = StoredValue::new("Home");
    let trigger = NodeRef::<html::Button>::new();
    let container = NodeRef::<html::Div>::new();
    let menu = NodeRef::<html::Div>::new();
    let label = i18n.t_args(
        "shell-account",
        &[("name", cfg.display_name.clone().into())],
    );
    Effect::new(move |_| {
        // tracks the node ref too: the menu mounts after `open` turns true
        if open.get() && menu.get().is_some() {
            focus_menu(menu, start.get_value());
        }
    });
    let _ = window_event_listener(leptos::ev::pointerdown, move |ev| {
        if open.get_untracked() && outside(container, &ev) {
            open.set(false);
        }
    });
    let close = Callback::new(move |()| {
        open.set(false);
        if let Some(t) = trigger.get_untracked() {
            let _ = t.focus();
        }
    });
    let on_key = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "Escape" => {
            ev.prevent_default();
            close.run(());
        }
        // focus goes to the button first, so Tab moves on from there (APG)
        "Tab" => close.run(()),
        k => {
            if focus_menu(menu, k) {
                ev.prevent_default();
            }
        }
    };
    let open_at = move |at: &'static str| {
        start.set_value(at);
        open.set(true);
    };
    let (name, username) = (cfg.display_name.clone(), cfg.username.clone());
    view! {
        <div node_ref=container class="relative">
            <button
                node_ref=trigger
                id=ACCOUNT_TRIGGER
                type="button"
                aria-haspopup="menu"
                aria-expanded=move || if open.get() { "true" } else { "false" }
                aria-controls="account-menu"
                aria-label=label.clone()
                on:click=move |_| {
                    if open.get_untracked() {
                        open.set(false);
                    } else {
                        open_at("Home");
                    }
                }
                on:keydown=move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
                    "ArrowDown" => {
                        ev.prevent_default();
                        open_at("Home");
                    }
                    "ArrowUp" => {
                        ev.prevent_default();
                        open_at("End");
                    }
                    _ => {}
                }
                class=ICON_BTN
            >
                // the button is named by its label; the initials are decoration (WCAG 2.5.3)
                <Avatar name=cfg.display_name.clone() decorative=true/>
            </button>
            <Show when=move || open.get()>
                <div class="absolute right-0 z-50 mt-1 w-64 rounded-md border bg-popover p-1 text-popover-foreground shadow-md">
                    <div data-testid="account-head" class="px-2 py-1.5">
                        <p class="truncate text-sm font-semibold">{name.clone()}</p>
                        <p class="truncate text-small text-muted-foreground">{username.clone()}</p>
                    </div>
                    <div node_ref=menu id="account-menu" role="menu" aria-label=label.clone() on:keydown=on_key>
                        <AccountMenuItems open=open close=close/>
                    </div>
                </div>
            </Show>
        </div>
    }
}

/// The items of the account menu: the account and admin links, theme, language, sign out,
/// shortcuts and help.
#[component]
fn AccountMenuItems(open: RwSignal<bool>, close: Callback<()>) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let theme = expect_context::<ThemeChoice>().0;
    let help = expect_context::<ShortcutsOpen>().0;
    let links = account_links(cfg.is_admin);
    // the entry of the page one is on (Stats has no place in the top bar, v1-polish P1)
    let path = use_location().pathname;
    view! {
        {links.iter().map(|(href, key)| {
            let href = *href;
            let here = move || path.with(|p| account_entry(p) == Some(href)).then_some("page");
            view! {
                <a role="menuitem" tabindex="-1" href=href class=MENU_ITEM aria-current=here on:click=move |_| open.set(false)>
                    {t(key)}
                </a>
            }
        }).collect_view()}
        <div role="separator" class=SEPARATOR></div>
        <ThemeItems theme=theme close=close/>
        <div role="separator" class=SEPARATOR></div>
        <LanguageItems close=close/>
        <div role="separator" class=SEPARATOR></div>
        <button type="submit" form=SIGN_OUT_FORM role="menuitem" tabindex="-1" class=MENU_ITEM>
            <Icon name=IconName::LogOut/>
            {t("nav-sign-out")}
        </button>
        <div role="separator" class=SEPARATOR></div>
        <button type="button" role="menuitem" tabindex="-1" class=MENU_ITEM
            on:click=move |_| { close.run(()); help.set(true); }>
            {t("shell-shortcuts")}
            <KeyHint keys=vec!["?".to_string()] class="ml-auto"/>
        </button>
        {cfg.support_url.clone().map(|url| view! {
            <a role="menuitem" tabindex="-1" href=url target="_blank" rel="noopener" class=MENU_ITEM
                on:click=move |_| close.run(())>
                {t("shell-help")}
            </a>
        })}
    }
}

fn theme_options() -> Vec<(&'static str, &'static str, IconName)> {
    vec![
        ("system", "shell-theme-system", IconName::Settings),
        ("light", "shell-theme-light", IconName::Sun),
        ("dark", "shell-theme-dark", IconName::Moon),
    ]
}

/// The theme as a group of three `menuitemradio`s.
#[component]
fn ThemeItems(theme: RwSignal<String>, close: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div role="group" aria-label=i18n.t("shell-theme")>
            <p aria-hidden="true" class="px-2 pb-1 pt-1.5 text-xs font-medium text-muted-foreground">{t("shell-theme")}</p>
            {theme_options().into_iter().map(|(value, key, icon)| view! {
                <button
                    type="button"
                    role="menuitemradio"
                    tabindex="-1"
                    aria-checked=move || if theme.get() == value { "true" } else { "false" }
                    on:click=move |_| {
                        theme.set(value.to_string());
                        close.run(());
                    }
                    class=MENU_ITEM
                >
                    <Icon name=icon/>
                    <span class="flex-1">{t(key)}</span>
                    {move || (theme.get() == value).then(|| view! { <Icon name=IconName::Check/> })}
                </button>
            }).collect_view()}
        </div>
    }
}

/// The value of the "automatic" language choice (no stored locale).
const LANGUAGE_AUTO: &str = "auto";

/// The language choices (DEC-201): automatic, then every locale named in its own language.
fn language_options() -> Vec<(&'static str, String, Option<&'static str>)> {
    std::iter::once((LANGUAGE_AUTO, t("shell-language-auto"), None))
        .chain(
            LOCALES
                .iter()
                .map(|l| (*l, contracts::locale::locale_name(l).to_string(), Some(*l))),
        )
        .collect()
}

/// Store the language choice ([`LANGUAGE_AUTO`]: none) and reload, so the server renders the page,
/// its `lang` and the hydration config in the new language.
fn language_action() -> Action<String, ()> {
    Action::new(|l: &String| {
        let locale = (l != LANGUAGE_AUTO).then(|| l.clone());
        async move {
            if crate::api::set_locale(locale).await.is_ok() {
                crate::pages::reload();
            }
        }
    })
}

/// The language as a group of `menuitemradio`s; each language is named in itself and
/// marked with its `lang` (WCAG 3.1.2).
#[component]
fn LanguageItems(close: Callback<()>) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    let current = cfg
        .locale_setting
        .clone()
        .unwrap_or_else(|| LANGUAGE_AUTO.to_string());
    let choose = language_action();
    view! {
        <div role="group" aria-label=i18n.t("shell-language")>
            <p aria-hidden="true" class="px-2 pb-1 pt-1.5 text-xs font-medium text-muted-foreground">{t("shell-language")}</p>
            {language_options().into_iter().map(|(value, label, lang)| {
                let checked = value == current;
                view! {
                    <button
                        type="button"
                        role="menuitemradio"
                        tabindex="-1"
                        aria-checked=if checked { "true" } else { "false" }
                        on:click=move |_| {
                            close.run(());
                            if !checked {
                                choose.dispatch(value.to_string());
                            }
                        }
                        class=MENU_ITEM
                    >
                        <span class="flex-1" lang=lang>{label}</span>
                        {checked.then(|| view! { <Icon name=IconName::Check/> })}
                    </button>
                }
            }).collect_view()}
        </div>
    }
}

/// The language on the You hub and Settings > Account: a segmented choice like the theme.
#[component]
pub(crate) fn LanguageChoice() -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    let value = RwSignal::new(
        cfg.locale_setting
            .clone()
            .unwrap_or_else(|| LANGUAGE_AUTO.to_string()),
    );
    let choose = language_action();
    Effect::new(move |prev: Option<String>| {
        let v = value.get();
        // the first run is the stored value; later runs are choices
        if prev.as_ref().is_some_and(|p| *p != v) {
            choose.dispatch(v.clone());
        }
        v
    });
    let options: Vec<SegmentedOption> = language_options()
        .into_iter()
        .map(|(value, label, lang)| {
            let o = SegmentedOption::new(value, label);
            match lang {
                Some(l) => o.lang(l),
                None => o,
            }
        })
        .collect();
    view! { <Segmented id="you-language" label=i18n.t("shell-language") value=value options=options/> }
}

/// The theme as a segmented choice (You hub, Settings > Account).
#[component]
pub(crate) fn ThemeChoiceControl() -> impl IntoView {
    let i18n = use_i18n();
    let theme = expect_context::<ThemeChoice>().0;
    let options: Vec<SegmentedOption> = theme_options()
        .into_iter()
        .map(|(value, key, icon)| SegmentedOption::new(value, i18n.t(key)).icon(icon))
        .collect();
    view! { <Segmented id="you-theme" label=i18n.t("shell-theme") value=theme options=options/> }
}

/// Mobile bottom navigation: Home, Library, Add, History, You (owner ruling, DEC-171).
#[component]
fn MobileTabs(current: Signal<String>) -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    let palette = expect_context::<PaletteOpen>().0;
    let mut you = NavItem::new("/you", i18n.t("nav-you")).icon(IconName::User);
    if cfg.unread > 0 {
        you = you.count(i18n.t_args("shell-unread", &[("count", unread_arg(cfg.unread))]));
    }
    let items = vec![
        NavItem::new("/", i18n.t("nav-home")).icon(IconName::Home),
        NavItem::new("/library", i18n.t("nav-library")).icon(IconName::Grid),
        NavItem::new("/history", i18n.t("nav-history")).icon(IconName::History),
        you,
    ];
    view! {
        <TabBar
            label=i18n.t("nav-main")
            items=items
            current=current
            action=TabBarAction::new(i18n.t("shell-add"), IconName::Plus, Callback::new(move |()| palette.set(true)))
        />
    }
}

/// Whether the browser is online (`navigator.onLine` and its events); forms disable their
/// submit button while offline (design.md 5.2).
#[derive(Clone, Copy)]
pub struct Online(pub RwSignal<bool>);

fn provide_online() -> RwSignal<bool> {
    let online = RwSignal::new(true);
    Effect::new(move |_| {
        #[cfg(feature = "hydrate")]
        online.set(is_online());
    });
    let _ = window_event_listener(leptos::ev::online, move |_| online.set(true));
    let _ = window_event_listener(leptos::ev::offline, move |_| online.set(false));
    provide_context(Online(online));
    online
}

/// Offline banner (design.md 6.15), a polite live region that is always in the page so the
/// change is announced.
#[component]
fn OfflineBanner(
    online: RwSignal<bool>,
    waiting: Option<RwSignal<usize>>,
    /// The page's column: the line sits above it (OT-14).
    width: Signal<PageWidth>,
) -> impl IntoView {
    let count = move || waiting.map_or(0, |w| w.get());
    let has_waiting = move || count() > 0;
    let line = move || {
        (count() > 0).then(|| {
            let text = use_i18n().t_args("offline-waiting", &[("count", count().into())]);
            view! { <span class="block font-medium" data-testid="queue-count">{text}</span> }
        })
    };
    view! {
        <div role="status" class=move || format!("mx-auto w-full {} px-4 empty:hidden", width.get().class())>
            <Show when=move || !online.get() || has_waiting()>
                <Alert variant=AlertVariant::Warning role=AlertRole::None class="mt-4">
                    {move || (!online.get()).then(|| view! { <span class="block">{t("shell-offline")}</span> })}
                    {line}
                </Alert>
            </Show>
        </div>
    }
}

/// `?`: every keyboard shortcut, grouped by where it works (design.md 8), with a note when
/// the single-key ones are switched off.
#[component]
fn ShortcutHelp(open: RwSignal<bool>) -> impl IntoView {
    let i18n = use_i18n();
    let signed_in = expect_context::<ClientConfig>().signed_in;
    let on = expect_context::<ShortcutsOn>().0;
    let key_name = |k: &str| crate::keyboard::key_message(k).map_or_else(|| k.to_string(), t);
    let sections = move || {
        crate::keyboard::help_sections(signed_in)
            .into_iter()
            .map(|(section, rows)| {
                let heading = format!("kb-{}", section.rsplit('-').next().unwrap_or_default());
                let rows = rows
                    .into_iter()
                    .map(|r| {
                        let keys: Vec<String> = r.keys.iter().map(|k| key_name(k)).collect();
                        let sep = match r.sep {
                            Sep::Plus => "+".to_string(),
                            Sep::Then => format!(" {} ", t("shortcuts-then")),
                            Sep::Or => format!(" {} ", t("shortcuts-or")),
                        };
                        view! {
                            <dt><KeyHint keys=keys separator=sep/></dt>
                            <dd>{t(r.what)}</dd>
                        }
                    })
                    .collect_view();
                view! {
                    <section aria-labelledby=heading.clone() class="grid gap-2">
                        <h3 id=heading.clone() class="text-sm font-semibold">{t(section)}</h3>
                        <dl class="grid grid-cols-[auto_1fr] items-center gap-x-4 gap-y-2 text-sm">{rows}</dl>
                    </section>
                }
            })
            .collect_view()
    };
    view! {
        <Dialog open=open title=i18n.t("shell-shortcuts") close_label=i18n.t("dialog-close") return_to=ACCOUNT_TRIGGER
            class="max-h-[90dvh] overflow-y-auto">
            <div class="grid gap-5">
                {move || (!on.get()).then(|| view! { <p class="text-sm text-muted-foreground">{t("shortcuts-off-note")}</p> })}
                {sections}
            </div>
        </Dialog>
    }
}

/// "G pressed: then ..." while a chord waits (design.md 3.2): a polite live region that is
/// always in the page, so the change is announced; the words show above the tab bar. Without
/// words the region has no box (`sr-only`, not `display: none`, which would take it out of
/// the accessibility tree; `empty:hidden` never matched, the reactive text leaves a node
/// behind, so an empty pill showed on every page: review OT2D-13).
#[component]
fn ChordHint(hint: RwSignal<String>) -> impl IntoView {
    view! {
        <div data-float="" class="pointer-events-none fixed inset-x-0 bottom-[calc(var(--tab-bar-height)+1rem)] z-50 flex justify-center px-4">
            <p role="status" data-testid="chord-hint"
                class=move || if hint.with(String::is_empty) {
                    "sr-only"
                } else {
                    "rounded-md border border-border bg-popover px-3 py-1.5 text-sm text-popover-foreground shadow-elevation-2"
                }>
                {move || hint.get()}
            </p>
        </div>
    }
}

/// The You hub (mobile tab, design.md 3.2): people, lists, notifications and the account
/// pages, theme, help and sign out.
#[component]
pub fn You() -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    let i18n = use_i18n();
    if !cfg.signed_in {
        return view! {
            <crate::pages::PageTitle text=i18n.t("nav-you")/>
            <p class="flex gap-4">
                <a href="/sign-in" class="text-primary underline">{t("nav-sign-in")}</a>
                <a href="/sign-up" class="text-primary underline">{t("nav-sign-up")}</a>
            </p>
        }
        .into_any();
    }
    let help = expect_context::<ShortcutsOpen>().0;
    let mut notifications =
        NavItem::new(NOTIFICATIONS_HREF, i18n.t("nav-notifications")).icon(IconName::Bell);
    if cfg.unread > 0 {
        notifications =
            notifications.count(i18n.t_args("shell-unread", &[("count", unread_arg(cfg.unread))]));
    }
    let mut items = vec![
        NavItem::new("/stats", i18n.t("nav-stats")).icon(IconName::History),
        NavItem::new("/people", i18n.t("nav-people")).icon(IconName::Users),
        NavItem::new("/lists", i18n.t("nav-lists")).icon(IconName::List),
        notifications,
    ];
    items.extend(
        account_links(cfg.is_admin)
            .into_iter()
            // notifications and stats are already in the list above, with their icons
            .filter(|(href, _)| *href != NOTIFICATIONS_HREF && *href != "/stats")
            .map(|(href, key)| NavItem::new(href, i18n.t(key))),
    );
    let location = use_location();
    view! {
        <crate::pages::PageTitle text=cfg.display_name.clone()/>
        <div class="grid max-w-md gap-6">
            <SideNav id="you" label=i18n.t("nav-you") groups=vec![NavGroup::new(items)] current=Signal::derive(move || location.pathname.get())/>
            <ThemeChoiceControl/>
            <LanguageChoice/>
            <div class="flex flex-wrap gap-2">
                <Button variant=ButtonVariant::Outline class="min-h-11" on:click=move |_| help.set(true)>
                    {t("shell-shortcuts")}
                </Button>
                {cfg.support_url.clone().map(|url| view! {
                    <ButtonLink href=url variant=ButtonVariant::Outline class="min-h-11" attr:target="_blank" attr:rel="noopener">
                        {t("shell-help")}
                    </ButtonLink>
                })}
            </div>
            <Button r#type="submit" variant=ButtonVariant::Outline class="min-h-11 w-fit" attr:form=SIGN_OUT_FORM>
                    <Icon name=IconName::LogOut/>
                    {t("nav-sign-out")}
            </Button>
        </div>
    }
    .into_any()
}

/// 404 (design.md 6.15), inside the shell.
#[component]
pub fn NotFound() -> impl IntoView {
    let cfg = expect_context::<ClientConfig>();
    view! {
        <section class="mx-auto max-w-xl py-16 text-center">
            <crate::pages::PageTitle text=t("notfound-title")/>
            <p class="text-muted-foreground">{t("notfound-lead")}</p>
            <p class="mt-6 flex justify-center gap-4">
                <a href="/" class="text-primary underline">{t("nav-home")}</a>
                {cfg.signed_in.then(|| view! { <a href="/library" class="text-primary underline">{t("nav-library")}</a> })}
            </p>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bell_caps_its_count() {
        assert_eq!(unread_label(0), None);
        assert_eq!(unread_label(3).as_deref(), Some("3"));
        assert_eq!(unread_label(9).as_deref(), Some("9"));
        assert_eq!(unread_label(10).as_deref(), Some("9+"));
        assert_eq!(unread_label(250).as_deref(), Some("9+"));
    }

    #[test]
    fn paths_map_to_their_section() {
        assert_eq!(section_of("/"), "/");
        assert_eq!(section_of("/library"), "/library");
        assert_eq!(section_of("/items/abc"), "/library");
        assert_eq!(section_of("/history"), "/history");
        assert_eq!(section_of("/shared/lists/x"), "/lists");
        assert_eq!(section_of("/people"), "/people");
        assert_eq!(section_of("/settings/notifications"), "/you");
        assert_eq!(section_of("/admin/users"), "/you");
        // Stats is reached from the account menu and the You hub, like the settings: it is
        // not Home (v1-polish P1)
        assert_eq!(section_of("/stats"), "/you");
        assert_eq!(section_of("/stats/"), "/you");
        assert_eq!(section_of("/notifications"), "");
        assert_eq!(section_of("/sessions"), "");
    }

    #[test]
    fn grid_pages_are_wide_and_record_pages_keep_80_rem() {
        // OT-14, DEC-541: 96 rem for grids and dashboards, 80 rem for records, streams,
        // settings and admin
        for path in [
            "/",
            "/library",
            "/lists",
            "/people",
            "/stats",
            "/library/",
            "/people/",
        ] {
            assert_eq!(page_width(path), PageWidth::Grid, "{path}");
        }
        for path in [
            "/items/abc",
            "/history",
            "/lists/abc",
            "/people/abc",
            "/shared/lists/x",
            "/settings/account",
            "/admin",
            "/you",
            "/sign-in",
            "/setup",
            "/dev/gallery",
        ] {
            assert_eq!(page_width(path), PageWidth::Record, "{path}");
        }
        assert_eq!(PageWidth::Grid.class(), "max-w-[96rem]");
        assert_eq!(PageWidth::Record.class(), "max-w-7xl");
    }

    #[test]
    fn five_sections_and_an_admin_entry_only_for_admins() {
        let hrefs: Vec<_> = sections().into_iter().map(|(h, _)| h).collect();
        assert_eq!(hrefs, ["/", "/library", "/history", "/lists", "/people"]);
        assert!(account_links(true).contains(&("/admin", "shell-admin")));
        assert!(account_links(false).contains(&("/settings/account", "nav-settings")));
        assert!(
            !account_links(false)
                .iter()
                .any(|(_, k)| *k == "shell-admin")
        );
        // the Stats page is reached from the account menu too (design.md 6.1)
        assert!(account_links(false).iter().any(|(h, _)| *h == "/stats"));
        // and the menu marks the entry of the page one is on
        assert_eq!(account_entry("/stats"), Some("/stats"));
        assert_eq!(
            account_entry("/settings/notifications"),
            Some(NOTIFICATIONS_HREF)
        );
        assert_eq!(
            account_entry("/settings/privacy"),
            Some("/settings/account")
        );
        assert_eq!(account_entry("/admin/users"), Some("/admin"));
        assert_eq!(account_entry("/library"), None);
        // a page below Notifications is still Notifications (review POL-16)
        assert_eq!(
            account_entry("/settings/notifications/rules"),
            Some(NOTIFICATIONS_HREF)
        );
        assert_eq!(
            account_entry("/settings/notifications-log"),
            Some("/settings/account")
        );
        for (href, _) in account_links(true) {
            assert_eq!(account_entry(href), Some(href));
        }
    }
}
