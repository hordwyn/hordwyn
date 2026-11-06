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
        .query_selector("[role=dialog]")
        .unwrap()
        .expect("dialog rendered")
        .dyn_into()
        .unwrap();
    key(&panel, "Escape");
    settle().await;
    assert!(!open.get_untracked());
    assert!(
        document()
            .query_selector("[role=dialog]")
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
