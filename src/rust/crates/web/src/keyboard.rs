//! The keyboard map (design.md 3.2, 6.1-6.4, 8; WP4-4): the "Keyboard shortcuts" switch
//! (WCAG 2.1.4, remembered in this browser), the rule for when a single key is a shortcut,
//! page key handlers that end with the page, the `G then ...` chord targets, focus on the new
//! page's heading after a chord, and the rows of the `?` help. The chord and roving-index
//! rules themselves are in `ui::keys`.

use leptos::prelude::*;

/// `localStorage` key of the switch (`off` when turned off; absent means on).
pub const SHORTCUTS_KEY: &str = "app-shortcuts";

/// How long the second key of a chord may wait (design.md 3.2; phase 4 plan: 1.5 s).
pub const CHORD_MS: f64 = 1500.0;

/// The second keys of `G then ...`, in the order of the help.
pub const GO_KEYS: &[char] = &['h', 'l', 'y', 't', 'p'];

/// Where `G then <key>` goes, with the message naming the place.
#[must_use]
pub fn go_target(key: char) -> Option<(&'static str, &'static str)> {
    match key {
        'h' => Some(("/", "nav-home")),
        'l' => Some(("/library", "nav-library")),
        'y' => Some(("/history", "nav-history")),
        't' => Some(("/lists", "nav-lists")),
        'p' => Some(("/people", "nav-people")),
        _ => None,
    }
}

/// Whether single-key shortcuts are on (Settings > Account).
#[derive(Clone, Copy)]
pub struct ShortcutsOn(pub RwSignal<bool>);

#[cfg(feature = "hydrate")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// The switch as context: read once in the browser, every later change stored.
pub fn provide_shortcuts() -> RwSignal<bool> {
    let on = RwSignal::new(true);
    Effect::new(move |prev: Option<()>| {
        let now = on.get();
        #[cfg(feature = "hydrate")]
        if let Some(s) = storage() {
            if prev.is_none() {
                if s.get_item(SHORTCUTS_KEY).ok().flatten().as_deref() == Some("off") {
                    on.set(false);
                }
            } else if now {
                let _ = s.remove_item(SHORTCUTS_KEY);
            } else {
                let _ = s.set_item(SHORTCUTS_KEY, "off");
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (now, prev);
    });
    provide_context(ShortcutsOn(on));
    on
}

/// Milliseconds since the epoch (the chord window).
#[must_use]
pub fn now_ms() -> f64 {
    #[cfg(feature = "hydrate")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        0.0
    }
}

/// Is a dialog or a menu open (keys belong to it then)?
#[cfg(feature = "hydrate")]
fn layer_open() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| {
            d.query_selector("[role=dialog],[role=alertdialog],[role=menu]")
                .ok()
        })
        .flatten()
        .is_some()
}

#[cfg(not(feature = "hydrate"))]
fn layer_open() -> bool {
    false
}

/// What decides whether a key press may be a single-key shortcut.
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)] // one flag per condition of the rule, read once
pub struct KeyState {
    pub shortcuts_on: bool,
    pub modifier: bool,
    pub handled: bool,
    /// Part of an IME composition (UX4-15): the key belongs to the input method.
    pub composing: bool,
    pub typing: bool,
    pub layer_open: bool,
}

impl KeyState {
    /// Shortcuts on, no Ctrl, Cmd or Alt, nobody handled it, not composing, not in a text
    /// field, no dialog or menu open.
    #[must_use]
    pub fn plain(self) -> bool {
        self.shortcuts_on
            && !self.modifier
            && !self.handled
            && !self.composing
            && !self.typing
            && !self.layer_open
    }
}

/// A key press that may be a single-key shortcut ([`KeyState::plain`]).
#[must_use]
pub fn plain_key(ev: &leptos::ev::KeyboardEvent, on: RwSignal<bool>) -> bool {
    KeyState {
        shortcuts_on: on.get_untracked(),
        modifier: ev.ctrl_key() || ev.meta_key() || ev.alt_key(),
        handled: ev.default_prevented(),
        composing: ev.is_composing(),
        typing: crate::shell::typing(ev),
        layer_open: layer_open(),
    }
    .plain()
}

/// Run `handler` for every plain key press (see [`plain_key`]) while the calling component
/// lives; `handler` gets the key in lower case and answers whether it used it (the event's
/// default is then prevented).
pub fn page_keys(handler: impl Fn(&leptos::ev::KeyboardEvent, &str) -> bool + 'static) {
    let on = expect_context::<ShortcutsOn>().0;
    let h = window_event_listener(leptos::ev::keydown, move |ev| {
        if plain_key(&ev, on) && handler(&ev, &ev.key().to_lowercase()) {
            ev.prevent_default();
        }
    });
    on_cleanup(move || h.remove());
}

/// The element that has the focus.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn focused() -> Option<web_sys::Element> {
    web_sys::window()?.document()?.active_element()
}

/// The first element matching `selector`, as an `HtmlElement`.
#[cfg(feature = "hydrate")]
#[must_use]
pub fn element(selector: &str) -> Option<web_sys::HtmlElement> {
    use wasm_bindgen::JsCast;
    web_sys::window()?
        .document()?
        .query_selector(selector)
        .ok()??
        .dyn_into()
        .ok()
}

/// Click the first element matching `selector`; `false` when there is none.
#[must_use]
pub fn click(selector: &str) -> bool {
    #[cfg(feature = "hydrate")]
    if let Some(el) = element(selector) {
        el.click();
        return true;
    }
    let _ = selector;
    false
}

/// Focus the first element matching `selector`; `false` when there is none.
#[must_use]
pub fn focus(selector: &str) -> bool {
    #[cfg(feature = "hydrate")]
    if let Some(el) = element(selector) {
        return el.focus().is_ok();
    }
    let _ = selector;
    false
}

/// Run `f` once an element matching `selector` is in the document (a page that a Transition
/// still renders off screen is not): tries every 50 ms, at most 3 s. Opening a dialog from a
/// view that is not mounted yet left it detached.
pub fn when_present(selector: &'static str, f: impl FnOnce() + 'static) {
    #[cfg(feature = "hydrate")]
    present_attempt(selector, Box::new(f), 0);
    #[cfg(not(feature = "hydrate"))]
    let _ = (selector, f);
}

#[cfg(feature = "hydrate")]
fn present_attempt(selector: &'static str, f: Box<dyn FnOnce()>, tries: u32) {
    if element(selector).is_some() {
        f();
    } else if tries < 60 {
        set_timeout(
            move || present_attempt(selector, f, tries + 1),
            std::time::Duration::from_millis(50),
        );
    }
}

/// One look for the new page's heading; tries again every 50 ms, at most 60 times.
#[cfg(feature = "hydrate")]
fn heading_attempt(before: Option<web_sys::HtmlElement>, same: bool, tries: u32) {
    let fresh = element("main h1").filter(|h| {
        same || before
            .as_ref()
            .is_none_or(|b| !b.is_same_node(Some(h.as_ref())))
    });
    if let Some(h) = fresh {
        if !h.has_attribute("tabindex") {
            let _ = h.set_attribute("tabindex", "-1");
        }
        let _ = h.focus();
    } else if tries < 60 {
        set_timeout(
            move || heading_attempt(before, same, tries + 1),
            std::time::Duration::from_millis(50),
        );
    }
}

/// After a chord: focus the heading of the page that replaces the one shown (the same one
/// when the chord went to the page already shown), so a screen reader starts at the new
/// page's title. Waits up to 3 s for the page to render.
pub fn focus_heading(same_page: bool) {
    #[cfg(feature = "hydrate")]
    heading_attempt(element("main h1"), same_page, 0);
    #[cfg(not(feature = "hydrate"))]
    let _ = same_page;
}

/// How the keys of a help row go together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sep {
    /// Pressed together (`Ctrl+K`).
    Plus,
    /// One after the other (`G then L`).
    Then,
    /// Either of them (`J or K`).
    Or,
}

/// One row of the help: the keys, how they go together, and the message saying what they
/// do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpRow {
    pub keys: Vec<&'static str>,
    pub sep: Sep,
    pub what: &'static str,
}

const fn row(keys: Vec<&'static str>, what: &'static str) -> HelpRow {
    HelpRow {
        keys,
        sep: Sep::Plus,
        what,
    }
}

const fn either(keys: Vec<&'static str>, what: &'static str) -> HelpRow {
    HelpRow {
        keys,
        sep: Sep::Or,
        what,
    }
}

/// Every shortcut, by where it works (design.md 8: "listed under `?`"). Keys that start with
/// `key:` are translated key names (`key:ctrl` -> `shell-ctrl`, others `shortcuts-key-*`).
/// Signed out only search, help and close work (UX4-15): `N`, the chords and the pages'
/// keys need an account.
#[must_use]
pub fn help_sections(signed_in: bool) -> Vec<(&'static str, Vec<HelpRow>)> {
    if !signed_in {
        return vec![
            (
                "shortcuts-everywhere",
                vec![
                    row(vec!["key:ctrl", "K"], "shell-key-search"),
                    row(vec!["?"], "shell-key-help"),
                    row(vec!["key:esc"], "shell-key-close"),
                ],
            ),
            (
                "shortcuts-search",
                vec![row(vec!["key:enter"], "shortcuts-search-open")],
            ),
        ];
    }
    let mut everywhere = vec![
        row(vec!["key:ctrl", "K"], "shell-key-search"),
        row(vec!["N"], "shell-key-add"),
    ];
    everywhere.extend(GO_KEYS.iter().filter_map(|k| {
        let (_, _) = go_target(*k)?;
        Some(HelpRow {
            keys: vec!["G", go_letter(*k)],
            sep: Sep::Then,
            what: go_message(*k),
        })
    }));
    everywhere.extend([
        row(vec!["?"], "shell-key-help"),
        row(vec!["key:esc"], "shell-key-close"),
    ]);
    vec![
        ("shortcuts-everywhere", everywhere),
        (
            "shortcuts-home",
            vec![
                either(vec!["J", "K"], "shortcuts-home-move"),
                row(vec!["key:enter"], "shortcuts-home-run"),
            ],
        ),
        (
            "shortcuts-library",
            vec![
                row(vec!["key:arrows"], "shortcuts-library-move"),
                either(vec!["key:home", "key:end"], "shortcuts-library-row"),
                row(vec!["key:ctrl", "key:home"], "shortcuts-library-first"),
                row(vec!["key:enter"], "shortcuts-library-open"),
                row(vec!["W"], "shortcuts-library-step"),
                row(vec!["T"], "shortcuts-library-tags"),
                row(vec!["L"], "shortcuts-library-list"),
                row(vec!["/"], "shortcuts-library-filter"),
            ],
        ),
        (
            "shortcuts-item",
            vec![
                row(vec!["W"], "shortcuts-item-step"),
                row(vec!["R"], "shortcuts-item-rating"),
                row(vec!["T"], "shortcuts-item-tags"),
                row(vec!["L"], "shortcuts-item-list"),
            ],
        ),
        (
            "shortcuts-search",
            vec![
                row(vec!["key:enter"], "shortcuts-search-open"),
                row(vec!["key:ctrl", "key:enter"], "shortcuts-search-quick"),
            ],
        ),
    ]
}

fn go_letter(k: char) -> &'static str {
    match k {
        'h' => "H",
        'l' => "L",
        'y' => "Y",
        't' => "T",
        _ => "P",
    }
}

fn go_message(k: char) -> &'static str {
    match k {
        'h' => "shortcuts-go-home",
        'l' => "shortcuts-go-library",
        'y' => "shortcuts-go-history",
        't' => "shortcuts-go-lists",
        _ => "shortcuts-go-people",
    }
}

/// The message key of a key name in a help row.
#[must_use]
pub fn key_message(key: &str) -> Option<&'static str> {
    match key.strip_prefix("key:")? {
        "ctrl" => Some("shell-ctrl"),
        "esc" => Some("shell-key-esc"),
        "enter" => Some("shortcuts-key-enter"),
        "home" => Some("shortcuts-key-home"),
        "end" => Some("shortcuts-key-end"),
        "arrows" => Some("shortcuts-key-arrows"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chord_has_a_place_and_a_help_row() {
        let rows: Vec<HelpRow> = help_sections(true)
            .into_iter()
            .flat_map(|(_, r)| r)
            .collect();
        for k in GO_KEYS {
            let (path, _) = go_target(*k).unwrap();
            assert!(path.starts_with('/'));
            assert!(
                rows.iter().any(|r| r.sep == Sep::Then
                    && r.keys == ["G", go_letter(*k)]
                    && r.what == go_message(*k)),
                "{k}"
            );
        }
        assert_eq!(go_target('x'), None);
        // the five sections of the top bar, in its order
        let paths: Vec<_> = GO_KEYS.iter().map(|k| go_target(*k).unwrap().0).collect();
        assert_eq!(paths, ["/", "/library", "/history", "/lists", "/people"]);
    }

    #[test]
    fn the_help_lists_the_page_shortcuts_and_names_its_keys() {
        let sections = help_sections(true);
        let names: Vec<_> = sections.iter().map(|(s, _)| *s).collect();
        assert_eq!(
            names,
            [
                "shortcuts-everywhere",
                "shortcuts-home",
                "shortcuts-library",
                "shortcuts-item",
                "shortcuts-search"
            ]
        );
        let all: Vec<&'static str> = sections
            .iter()
            .flat_map(|(_, r)| r.iter().flat_map(|r| r.keys.clone()))
            .collect();
        for k in ["W", "R", "T", "L", "J", "N", "?", "/"] {
            assert!(all.contains(&k), "{k}");
        }
        assert_eq!(key_message("key:ctrl"), Some("shell-ctrl"));
        assert_eq!(key_message("key:enter"), Some("shortcuts-key-enter"));
        // every key name in the help has a message
        for (_, rows) in help_sections(true).into_iter().chain(help_sections(false)) {
            for k in rows.iter().flat_map(|r| r.keys.clone()) {
                assert!(!k.starts_with("key:") || key_message(k).is_some(), "{k}");
            }
        }
        assert_eq!(key_message("W"), None);
    }

    #[test]
    fn signed_out_the_help_lists_what_works_signed_out() {
        let rows: Vec<HelpRow> = help_sections(false)
            .into_iter()
            .flat_map(|(_, r)| r)
            .collect();
        assert!(rows.iter().any(|r| r.keys == ["key:ctrl", "K"]));
        assert!(rows.iter().any(|r| r.keys == ["?"]));
        for gone in ["N", "G", "W", "J"] {
            assert!(!rows.iter().any(|r| r.keys.contains(&gone)), "{gone}");
        }
        assert!(
            !rows.iter().any(|r| r.what == "shortcuts-search-quick"),
            "quick add needs an account"
        );
    }

    #[test]
    fn a_key_is_a_shortcut_only_when_nothing_else_owns_it() {
        let on = KeyState {
            shortcuts_on: true,
            ..KeyState::default()
        };
        assert!(on.plain());
        assert!(!KeyState::default().plain(), "switched off");
        for k in [
            KeyState {
                modifier: true,
                ..on
            },
            KeyState {
                handled: true,
                ..on
            },
            KeyState {
                composing: true,
                ..on
            },
            KeyState { typing: true, ..on },
            KeyState {
                layer_open: true,
                ..on
            },
        ] {
            assert!(!k.plain(), "{k:?}");
        }
    }
}
