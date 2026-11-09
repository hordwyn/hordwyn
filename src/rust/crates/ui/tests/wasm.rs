//! Interactive behaviour in a real browser (`wasm-bindgen-test`, headless Chromium).
//! Run with `cargo test -p ui --target wasm32-unknown-unknown` (runner configured in
//! `.cargo/config.toml`, browser driver from the Nix shell).
#![cfg(target_arch = "wasm32")]

use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;
use ui::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{HtmlElement, KeyboardEvent, KeyboardEventInit};

wasm_bindgen_test_configure!(run_in_browser);

fn el(id: &str) -> HtmlElement {
    document()
        .get_element_by_id(id)
        .unwrap_or_else(|| panic!("#{id} missing"))
        .dyn_into()
        .unwrap()
}

fn key(target: &HtmlElement, key: &str) {
    let init = KeyboardEventInit::new();
    init.set_key(key);
    init.set_bubbles(true);
    init.set_cancelable(true);
    let ev = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap();
    target.dispatch_event(&ev).unwrap();
}

async fn settle() {
    leptos::task::tick().await;
    leptos::task::tick().await;
}

#[wasm_bindgen_test]
async fn switch_toggles_aria_checked() {
    let checked = RwSignal::new(false);

    leptos::mount::mount_to_body(move || view! { <Switch id="w-switch" checked=checked/> });
    let s = el("w-switch");
    assert_eq!(s.get_attribute("aria-checked").as_deref(), Some("false"));
    s.click();
    settle().await;
    assert!(checked.get_untracked());
    assert_eq!(s.get_attribute("aria-checked").as_deref(), Some("true"));
}

#[wasm_bindgen_test]
async fn tabs_follow_arrow_keys() {
    let active = RwSignal::new("a".to_string());
    leptos::mount::mount_to_body(move || {
        view! {
            <Tabs id="w-tabs" active=active>
                <TabsList label="L">
                    <TabsTrigger value="a">"A"</TabsTrigger>
                    <TabsTrigger value="b">"B"</TabsTrigger>
                </TabsList>
                <TabsContent value="a">"pa"</TabsContent>
                <TabsContent value="b">"pb"</TabsContent>
            </Tabs>
        }
    });
    let a = el("w-tabs-tab-a");
    a.focus().unwrap();
    key(&a, "ArrowRight");
    settle().await;
    assert_eq!(active.get_untracked(), "b");
    assert_eq!(
        el("w-tabs-tab-b").get_attribute("aria-selected").as_deref(),
        Some("true")
    );
    assert!(el("w-tabs-panel-a").has_attribute("hidden"));
    key(&el("w-tabs-tab-b"), "ArrowRight");
    settle().await;
    assert_eq!(active.get_untracked(), "a", "wraps around");
}

#[wasm_bindgen_test]
async fn dialog_closes_on_escape() {
    let open = RwSignal::new(true);
    leptos::mount::mount_to_body(move || {
        view! { <Dialog open=open title="W dialog" close_label="Close"><p>"x"</p></Dialog> }
    });
    settle().await;
    let panel: HtmlElement = document()
        .query_selector("[aria-labelledby=w-dialog-title]")
        .unwrap()
        .expect("dialog rendered")
        .dyn_into()
        .unwrap();
    key(&panel, "Escape");
    settle().await;
    assert!(!open.get_untracked());
    assert!(
        document()
            .query_selector("[aria-labelledby=w-dialog-title]")
            .unwrap()
            .is_none()
    );
}

#[wasm_bindgen_test]
async fn command_selects_active_option_with_enter() {
    let q = RwSignal::new(String::new());
    let chosen = RwSignal::new(String::new());
    let items = Signal::derive(|| {
        ["Alien", "Arrival", "Brazil"]
            .iter()
            .map(|t| CommandItem {
                id: t.to_lowercase(),
                label: (*t).into(),
                group: "g".into(),
                keywords: String::new(),
            })
            .collect::<Vec<_>>()
    });
    leptos::mount::mount_to_body(move || {
        view! {
            <Command id="w-cmd" label="S" placeholder="p" empty_label="none" items=items query=q
                on_select=Callback::new(move |i: CommandItem| chosen.set(i.label))/>
        }
    });
    q.set("a".into());
    settle().await;
    let input = el("w-cmd");
    key(&input, "ArrowDown");
    settle().await;
    assert_eq!(
        input.get_attribute("aria-activedescendant").as_deref(),
        Some("w-cmd-opt-arrival")
    );
    key(&input, "Enter");
    settle().await;
    assert_eq!(chosen.get_untracked(), "Arrival");
}

#[wasm_bindgen_test]
async fn rating_arrow_keys_move_in_half_steps_and_focus_follows() {
    let v = RwSignal::new(Some(60u8));
    let label = Callback::new(|v: u8| format!("{v}"));
    leptos::mount::mount_to_body(move || {
        view! { <Rating id="w-rating" label="R" value=v half_steps=true option_label=label/> }
    });
    let start = el("w-rating-60");
    start.focus().unwrap();
    key(&start, "ArrowRight");
    settle().await;
    assert_eq!(v.get_untracked(), Some(70));
    let now = el("w-rating-70");
    assert_eq!(now.get_attribute("aria-checked").as_deref(), Some("true"));
    assert_eq!(now.get_attribute("tabindex").as_deref(), Some("0"));
    assert_eq!(
        document().active_element().map(|e| e.id()).as_deref(),
        Some("w-rating-70")
    );
    key(&now, "Delete");
    settle().await;
    assert_eq!(v.get_untracked(), None);
    el("w-rating-40").click();
    settle().await;
    assert_eq!(v.get_untracked(), Some(40));
    el("w-rating-40").click();
    settle().await;
    assert_eq!(v.get_untracked(), None, "clicking the checked step clears");
}

#[wasm_bindgen_test]
async fn segmented_radio_follows_arrow_keys() {
    let v = RwSignal::new("a".to_string());
    leptos::mount::mount_to_body(move || {
        let opts = vec![
            SegmentedOption::new("a", "A"),
            SegmentedOption::new("b", "B"),
        ];
        view! { <Segmented id="w-seg" label="S" value=v options=opts/> }
    });
    let a = el("w-seg-a");
    a.focus().unwrap();
    key(&a, "ArrowRight");
    settle().await;
    assert_eq!(v.get_untracked(), "b");
    assert_eq!(
        el("w-seg-b").get_attribute("aria-checked").as_deref(),
        Some("true")
    );
    assert_eq!(
        document().active_element().map(|e| e.id()).as_deref(),
        Some("w-seg-b")
    );
}

#[wasm_bindgen_test]
async fn chip_toggles_aria_pressed() {
    let on = RwSignal::new(false);
    leptos::mount::mount_to_body(move || {
        view! { <Chip pressed=on attr:id="w-chip">"Movies"</Chip> }
    });
    let chip = el("w-chip");
    chip.click();
    settle().await;
    assert!(on.get_untracked());
    assert_eq!(chip.get_attribute("aria-pressed").as_deref(), Some("true"));
}

#[wasm_bindgen_test]
async fn button_disclosure_toggles_and_remembers() {
    let key_name = disclosure_storage_key("wasm", "sec");
    let storage = window().local_storage().unwrap().unwrap();
    storage.set_item(&key_name, "1").unwrap();
    let k = key_name.clone();
    leptos::mount::mount_to_body(move || {
        view! {
            <Disclosure id="w-dis" title="Sec" mode=DisclosureMode::Button storage_key=k.clone()>
                <p>"content"</p>
            </Disclosure>
        }
    });
    settle().await;
    let button: HtmlElement = document()
        .query_selector("#w-dis button")
        .unwrap()
        .expect("header button")
        .dyn_into()
        .unwrap();
    assert_eq!(
        button.get_attribute("aria-expanded").as_deref(),
        Some("true"),
        "restored from localStorage"
    );
    assert!(!el("w-dis-content").has_attribute("hidden"));
    button.click();
    settle().await;
    assert_eq!(
        button.get_attribute("aria-expanded").as_deref(),
        Some("false")
    );
    assert!(el("w-dis-content").has_attribute("hidden"));
    assert_eq!(storage.get_item(&key_name).unwrap().as_deref(), Some("0"));
    storage.remove_item(&key_name).unwrap();
}

/// Addendum-1 D16 (WP4-5): a remembered disclosure reports that it was shown, opened and
/// opened early to the watcher in context, which also knows the open ones at a save; a
/// disclosure without a storage key reports nothing.
#[wasm_bindgen_test]
async fn disclosures_report_shown_opened_and_open_at_save() {
    let seen = RwSignal::new(Vec::<(String, DisclosureEvent)>::new());
    let watch = DisclosureWatch::new(Callback::new(move |e| seen.update(|v| v.push(e))));
    let key_name = disclosure_storage_key("wasm", "report");
    let storage = window().local_storage().unwrap().unwrap();
    storage.remove_item(&key_name).unwrap();
    let k = key_name.clone();
    leptos::mount::mount_to_body(move || {
        provide_context(watch);
        view! {
            <Disclosure id="w-rep" title="Rep" mode=DisclosureMode::Button storage_key=k.clone()>
                <p>"x"</p>
            </Disclosure>
            <Disclosure id="w-plain" title="Plain"><p>"y"</p></Disclosure>
        }
    });
    settle().await;
    assert_eq!(
        seen.get_untracked(),
        vec![(key_name.clone(), DisclosureEvent::Shown)]
    );
    assert_eq!(watch.open_keys().len(), 0);
    let button: HtmlElement = document()
        .query_selector("#w-rep button")
        .unwrap()
        .expect("header button")
        .dyn_into()
        .unwrap();
    button.click();
    settle().await;
    assert_eq!(
        seen.get_untracked(),
        vec![
            (key_name.clone(), DisclosureEvent::Shown),
            (key_name.clone(), DisclosureEvent::Opened),
            (key_name.clone(), DisclosureEvent::OpenedEarly),
        ],
        "opened within 2 s of showing"
    );
    assert_eq!(watch.open_keys(), vec![key_name.clone()]);
    button.click();
    settle().await;
    assert_eq!(seen.get_untracked().len(), 3, "closing reports nothing");
    assert_eq!(watch.open_keys().len(), 0);
    storage.remove_item(&key_name).unwrap();
}

#[wasm_bindgen_test]
async fn error_summary_takes_focus_when_errors_appear() {
    let errors = RwSignal::new(Vec::<ErrorLink>::new());
    leptos::mount::mount_to_body(move || {
        view! {
            <ErrorSummary id="w-errs" title="Problem" errors=errors/>
            <input id="w-errs-field"/>
        }
    });
    settle().await;
    assert!(document().get_element_by_id("w-errs").is_none());
    errors.set(vec![ErrorLink::new("w-errs-field", "Fix this")]);
    settle().await;
    assert_eq!(
        document().active_element().map(|e| e.id()).as_deref(),
        Some("w-errs")
    );
    let link: HtmlElement = document()
        .query_selector("#w-errs a")
        .unwrap()
        .expect("link")
        .dyn_into()
        .unwrap();
    link.click();
    settle().await;
    assert_eq!(
        document().active_element().map(|e| e.id()).as_deref(),
        Some("w-errs-field")
    );
}

fn key_with_shift(target: &HtmlElement, key: &str) {
    let init = KeyboardEventInit::new();
    init.set_key(key);
    init.set_shift_key(true);
    init.set_bubbles(true);
    init.set_cancelable(true);
    let ev = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap();
    target.dispatch_event(&ev).unwrap();
}

fn active_id() -> String {
    document()
        .active_element()
        .map(|e| e.id())
        .unwrap_or_default()
}

/// APG dialog pattern (design.md 8): Tab and Shift+Tab stay inside, Escape returns focus to
/// the control that opened it.
#[wasm_bindgen_test]
async fn dialog_traps_focus_and_returns_it_to_the_opener() {
    let open = RwSignal::new(false);
    leptos::mount::mount_to_body(move || {
        view! {
            <button id="w-d-opener" on:click=move |_| open.set(true)>"Open"</button>
            <Dialog open=open title="W trap" close_label="Close W" class="w-d-panel">
                <input id="w-d-first"/>
                <button id="w-d-middle">"Middle"</button>
            </Dialog>
        }
    });
    let opener = el("w-d-opener");
    opener.focus().unwrap();
    opener.click();
    settle().await;
    let panel: HtmlElement = document()
        .query_selector(".w-d-panel")
        .unwrap()
        .expect("dialog rendered")
        .dyn_into()
        .unwrap();
    assert!(
        document()
            .active_element()
            .is_some_and(|a| a.is_same_node(Some(panel.as_ref()))),
        "focus moves into the dialog"
    );
    let close: HtmlElement = panel
        .query_selector("[aria-label='Close W']")
        .unwrap()
        .unwrap()
        .dyn_into()
        .unwrap();
    close.focus().unwrap();
    key(&close, "Tab");
    assert_eq!(
        active_id(),
        "w-d-first",
        "Tab from the last control wraps to the first"
    );
    key_with_shift(&el("w-d-first"), "Tab");
    assert!(
        document()
            .active_element()
            .is_some_and(|a| a.is_same_node(Some(close.as_ref()))),
        "Shift+Tab from the first control wraps to the last"
    );
    key(&close, "Escape");
    settle().await;
    assert!(!open.get_untracked());
    assert_eq!(active_id(), "w-d-opener", "focus returns to the opener");
}

/// UX2-W3: controls taken out of the Tab order (`tabindex=-1`, the unselected options of a
/// roving radio group) are not the trap's first or last stop, so Shift+Tab from the first
/// reachable control still wraps; UX2-W18: `initial_focus` names the control that gets
/// focus when the dialog opens.
#[wasm_bindgen_test]
async fn dialog_trap_skips_controls_out_of_the_tab_order_and_focuses_the_named_field() {
    let open = RwSignal::new(false);
    leptos::mount::mount_to_body(move || {
        view! {
            <button id="w-r-opener" on:click=move |_| open.set(true)>"Open"</button>
            <Dialog open=open title="W roving" close_label="Close R" initial_focus="w-r-input">
                <button id="w-r-off" tabindex="-1">"Off"</button>
                <button id="w-r-on">"On"</button>
                <input id="w-r-input"/>
            </Dialog>
        }
    });
    let opener = el("w-r-opener");
    opener.focus().unwrap();
    opener.click();
    settle().await;
    assert_eq!(active_id(), "w-r-input", "the named field has focus");
    let on = el("w-r-on");
    on.focus().unwrap();
    key_with_shift(&on, "Tab");
    assert!(
        document()
            .active_element()
            .is_some_and(|a| a.get_attribute("aria-label").as_deref() == Some("Close R")),
        "Shift+Tab from the first reachable control wraps to the last"
    );
    key(&el("w-r-input"), "Escape");
    settle().await;
}

/// An opener that unmounts as the dialog opens (a menu item): focus goes to `return_to`.
#[wasm_bindgen_test]
async fn dialog_returns_focus_to_the_fallback_when_the_opener_is_gone() {
    let open = RwSignal::new(false);
    let menu = RwSignal::new(true);
    leptos::mount::mount_to_body(move || {
        view! {
            <button id="w-f-trigger">"Menu"</button>
            <Show when=move || menu.get()>
                <button id="w-f-item" on:click=move |_| { menu.set(false); open.set(true); }>"Help"</button>
            </Show>
            <Dialog open=open title="W fallback" close_label="Close F" return_to="w-f-trigger">
                <p>"x"</p>
            </Dialog>
        }
    });
    let item = el("w-f-item");
    item.focus().unwrap();
    item.click();
    settle().await;
    assert!(document().get_element_by_id("w-f-item").is_none());
    let panel: HtmlElement = document()
        .query_selector("[aria-labelledby='w-fallback-title']")
        .unwrap()
        .expect("dialog rendered")
        .dyn_into()
        .unwrap();
    key(&panel, "Escape");
    settle().await;
    assert_eq!(active_id(), "w-f-trigger");
}

fn type_into(input: &HtmlElement, text: &str) {
    let i: web_sys::HtmlInputElement = input.clone().dyn_into().unwrap();
    i.set_value(text);
    // the listener sits on the input itself: no bubbling needed
    let ev = web_sys::Event::new("input").unwrap();
    input.dispatch_event(&ev).unwrap();
}

#[wasm_bindgen_test]
async fn combobox_offers_to_create_what_was_typed_and_clears_after_choosing() {
    let selected = RwSignal::new(None::<String>);
    let created = RwSignal::new(String::new());
    let options = Signal::derive(|| {
        [("a", "Anna"), ("b", "Ben")]
            .map(|(value, label)| SelectOption {
                value: value.into(),
                label: label.into(),
            })
            .to_vec()
    });
    leptos::mount::mount_to_body(move || {
        view! {
            <Combobox id="w-combo" label="People" options=options selected=selected empty_label="none"
                clear_on_select=true
                create_label=Callback::new(|t: String| format!("Add {t}"))
                on_create=Callback::new(move |t: String| created.set(t))/>
        }
    });
    let input = el("w-combo");
    type_into(&input, "Zoe");
    settle().await;
    let create = el("w-combo-create");
    assert_eq!(create.get_attribute("role").as_deref(), Some("option"));
    assert_eq!(create.text_content().as_deref(), Some("Add Zoe"));
    key(&input, "Enter");
    settle().await;
    assert_eq!(created.get_untracked(), "Zoe");
    let i: web_sys::HtmlInputElement = input.clone().dyn_into().unwrap();
    assert_eq!(i.value(), "", "cleared for the next name");
    // an existing name is chosen, not created, and the text clears as well
    type_into(&input, "ben");
    settle().await;
    assert!(
        document().get_element_by_id("w-combo-create").is_none(),
        "exact match: no create row"
    );
    key(&input, "Enter");
    settle().await;
    assert_eq!(selected.get_untracked().as_deref(), Some("b"));
    assert_eq!(i.value(), "");
}

#[wasm_bindgen_test]
async fn sheet_asks_before_closing_when_it_may_not_close() {
    let open = RwSignal::new(true);
    let asked = RwSignal::new(0u32);
    let may_close = RwSignal::new(false);
    leptos::mount::mount_to_body(move || {
        view! {
            <Sheet open=open title="W guarded" close_label="Close" side=SheetSide::Adaptive
                can_close=Callback::new(move |()| {
                    asked.update(|n| *n += 1);
                    may_close.get_untracked()
                })>
                <p>"x"</p>
            </Sheet>
        }
    });
    settle().await;
    let panel: HtmlElement = document()
        .query_selector("[aria-labelledby=w-guarded-title]")
        .unwrap()
        .expect("sheet rendered")
        .dyn_into()
        .unwrap();
    key(&panel, "Escape");
    settle().await;
    assert!(open.get_untracked(), "kept open while it may not close");
    assert_eq!(asked.get_untracked(), 1);
    may_close.set(true);
    key(&panel, "Escape");
    settle().await;
    assert!(!open.get_untracked());
}

fn key_with_ctrl(target: &HtmlElement, key: &str) {
    let init = KeyboardEventInit::new();
    init.set_key(key);
    init.set_ctrl_key(true);
    init.set_bubbles(true);
    init.set_cancelable(true);
    let ev = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init).unwrap();
    target.dispatch_event(&ev).unwrap();
}

#[wasm_bindgen_test]
async fn command_ctrl_enter_runs_the_secondary_action_and_the_active_row_shows_hints() {
    let q = RwSignal::new(String::new());
    let chosen = RwSignal::new(String::new());
    let second = RwSignal::new(String::new());
    let items = Signal::derive(|| {
        ["Dune", "Duel"]
            .iter()
            .map(|t| CommandItem {
                id: t.to_lowercase(),
                label: (*t).into(),
                group: "g".into(),
                keywords: String::new(),
            })
            .collect::<Vec<_>>()
    });
    let hints = CommandHints {
        enter_keys: vec!["Enter".into()],
        enter: Signal::derive(|| "Open".to_string()),
        secondary_keys: vec!["Ctrl".into(), "Enter".into()],
        secondary: Signal::derive(|| "Add".to_string()),
    };
    leptos::mount::mount_to_body(move || {
        view! {
            <Command id="w-cmd2" label="S" placeholder="p" empty_label="none" items=items query=q
                on_select=Callback::new(move |i: CommandItem| chosen.set(i.label))
                on_secondary=Callback::new(move |i: CommandItem| second.set(i.label))
                hints=hints.clone()/>
        }
    });
    settle().await;
    let input = el("w-cmd2");
    // only the active row carries the hints
    let active = el("w-cmd2-opt-dune");
    assert!(active.text_content().unwrap_or_default().contains("Add"));
    assert!(
        !el("w-cmd2-opt-duel")
            .text_content()
            .unwrap_or_default()
            .contains("Add")
    );
    assert!(
        el("w-cmd2-hints")
            .text_content()
            .unwrap_or_default()
            .contains("Ctrl")
    );
    key(&input, "ArrowDown");
    settle().await;
    assert!(
        el("w-cmd2-opt-duel")
            .text_content()
            .unwrap_or_default()
            .contains("Open")
    );
    key_with_ctrl(&input, "Enter");
    settle().await;
    assert_eq!(second.get_untracked(), "Duel");
    assert_eq!(chosen.get_untracked(), "", "Ctrl+Enter is not Enter");
    key(&input, "Enter");
    settle().await;
    assert_eq!(chosen.get_untracked(), "Duel");
}

#[wasm_bindgen_test]
fn roving_index_and_chord_run_in_the_browser() {
    // the same rules the pages use, compiled for wasm (DEC: WP4-4 keyboard)
    assert_eq!(roving_target(1, 7, 3, "ArrowDown", false), Some(4));
    assert_eq!(roving_target(4, 7, 3, "ArrowDown", false), Some(4));
    assert_eq!(roving_target(4, 7, 3, "Home", true), Some(0));
    assert_eq!(roving_target(0, 7, 3, "Enter", false), None);
    assert_eq!(columns_from_tops(&[4.0, 4.0, 4.5, 200.0]), 3);
    let mut c = Chord::new('g', &['h', 'l', 'y', 't', 'p'], 1500.0);
    assert_eq!(c.key("g", 0.0), ChordStep::Started);
    assert_eq!(c.key("Shift", 10.0), ChordStep::Ignored);
    assert_eq!(c.key("Y", 900.0), ChordStep::Done('y'));
    assert_eq!(c.key("g", 1000.0), ChordStep::Started);
    assert_eq!(c.key("y", 2600.0), ChordStep::Ignored);
}
