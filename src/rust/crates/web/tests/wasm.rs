//! Page widgets in a real browser (`wasm-bindgen-test`, headless Chromium): what stays and
//! what is built again when the data of a page is fetched again (OT-21, OT1B-12). Run with
//! `cargo test -p web --target wasm32-unknown-unknown --features hydrate --test wasm`.
#![cfg(target_arch = "wasm32")]

use std::collections::BTreeMap;
use std::sync::Arc;

use contracts::{ImageDto, ImageVariantDto};
use leptos::prelude::*;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web::i18n::I18n;
use web::widgets::{CoverOf, StableCover, keyed};

wasm_bindgen_test_configure!(run_in_browser);

async fn settle() {
    leptos::task::tick().await;
    leptos::task::tick().await;
}

fn one(selector: &str) -> Option<web_sys::Element> {
    document().query_selector(selector).unwrap()
}

fn count(selector: &str) -> u32 {
    document().query_selector_all(selector).unwrap().length()
}

fn cover(title: &str, image: Option<ImageDto>) -> CoverOf {
    CoverOf {
        title: title.into(),
        image,
        square: false,
    }
}

fn picture(url: &str, variants: Vec<ImageVariantDto>) -> ImageDto {
    ImageDto {
        url: url.into(),
        variants,
        thumbhash: None,
        dominant_color: None,
        width: None,
        height: None,
    }
}

/// A cover keeps its `img` for an answer that says the same and follows its data when the
/// picture or the title changes: the stored variants take the place of the provider's
/// address on an open page.
#[wasm_bindgen_test]
async fn a_cover_stays_for_equal_data_and_changes_with_its_picture() {
    let provider = picture("https://image.example/t/p/w780/heat.jpg", vec![]);
    let of = RwSignal::new(Some(cover("Heat", Some(provider.clone()))));
    leptos::mount::mount_to_body(move || {
        provide_context(Arc::new(I18n::new("en", BTreeMap::new())));
        view! { <div id="c1"><StableCover of=of sizes="10px"/></div> }
    });
    settle().await;
    let img = one("#c1 img").expect("the cover");
    assert_eq!(
        img.get_attribute("src").as_deref(),
        Some("https://image.example/t/p/w780/heat.jpg")
    );
    assert_eq!(img.get_attribute("alt").as_deref(), Some("Cover of Heat"));
    img.set_attribute("data-kept", "1").unwrap();
    // the page was fetched again and nothing about the cover changed
    of.set(Some(cover("Heat", Some(provider))));
    settle().await;
    assert!(
        one("#c1 img")
            .expect("the cover")
            .has_attribute("data-kept"),
        "the same element for the same data"
    );
    // the asset job stored the cover: variants on our own host
    let stored = picture(
        "/assets/h/card.webp",
        vec![ImageVariantDto {
            url: "/assets/h/card.webp".into(),
            width: 400,
            mime: "image/webp".into(),
        }],
    );
    of.set(Some(cover("Heat", Some(stored.clone()))));
    settle().await;
    let img = one("#c1 img").expect("the cover");
    // (the renderer may patch the element in place; what counts is what it shows)
    assert_eq!(
        img.get_attribute("src").as_deref(),
        Some("/assets/h/card.webp")
    );
    let source = one("#c1 source[type='image/webp']").expect("the stored variant");
    assert_eq!(
        source.get_attribute("srcset").as_deref(),
        Some("/assets/h/card.webp 400w")
    );
    // another title: the name of the picture follows
    of.set(Some(cover("Heat 2", Some(stored))));
    settle().await;
    assert_eq!(
        one("#c1 img").unwrap().get_attribute("alt").as_deref(),
        Some("Cover of Heat 2")
    );
    // the cover is gone at the source: the box with the title
    of.set(Some(cover("Heat 2", None)));
    settle().await;
    assert_eq!(count("#c1 img"), 0);
    let boxed = one("#c1 [role='img']").expect("the box");
    assert_eq!(boxed.text_content().unwrap_or_default().trim(), "Heat 2");
    of.set(None);
    settle().await;
    assert_eq!(count("#c1 [role='img']"), 0);
}

/// A keyed row keeps its shell while its key is listed, shows the new data of its row, and
/// leaves the page with its key.
#[wasm_bindgen_test]
async fn a_keyed_row_keeps_its_shell_and_leaves_with_its_key() {
    let rows = RwSignal::new(vec![("a".to_string(), 1u32), ("b".to_string(), 1)]);
    leptos::mount::mount_to_body(move || {
        let list = keyed(
            rows.into(),
            |r: &(String, u32)| r.0.clone(),
            |row: Memo<Option<(String, u32)>>| {
                let key = row.get_untracked().map(|r| r.0).unwrap_or_default();
                view! {
                    <li data-key=key>{move || row.get().map(|r| r.1.to_string())}</li>
                }
            },
        );
        view! { <ul id="k1">{list}</ul> }
    });
    settle().await;
    assert_eq!(count("#k1 li"), 2);
    let a = one("#k1 li[data-key='a']").expect("row a");
    a.set_attribute("data-kept", "1").unwrap();
    // a's data changes, a row joins before it
    rows.set(vec![
        ("c".to_string(), 7),
        ("a".to_string(), 2),
        ("b".to_string(), 1),
    ]);
    settle().await;
    let a = one("#k1 li[data-key='a']").expect("row a");
    assert!(a.has_attribute("data-kept"), "the shell stayed");
    assert_eq!(a.text_content().as_deref(), Some("2"), "with the new data");
    assert_eq!(
        one("#k1 li").unwrap().get_attribute("data-key").as_deref(),
        Some("c"),
        "rows stand in the order of the list"
    );
    // a leaves
    rows.set(vec![("c".to_string(), 7), ("b".to_string(), 1)]);
    settle().await;
    assert_eq!(count("#k1 li"), 2);
    assert!(one("#k1 li[data-key='a']").is_none(), "the card is gone");
}
