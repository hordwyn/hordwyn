//! Domain widgets (D-3): poster card with thumbhash placeholder, rating input, companion picker.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::{CatalogItemDto, CompanionDto, ImageDto};
use leptos::prelude::*;
use leptos_router::components::A;
use ui::Rating;

use crate::i18n::{t, use_i18n};

/// Decode a base64 thumbhash into a tiny BMP data URL (no image codec needed; identical on
/// server and client, so hydration matches).
#[must_use]
pub fn thumbhash_data_url(b64: &str) -> Option<String> {
    let hash = STANDARD.decode(b64).ok()?;
    let (width, height, rgba) = thumbhash::thumb_hash_to_rgba(&hash).ok()?;
    let row = (width * 3).div_ceil(4) * 4;
    let size = 54 + row * height;
    let mut b = Vec::with_capacity(size);
    let le32 = |v: usize| u32::try_from(v).unwrap_or(0).to_le_bytes();
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&le32(size));
    b.extend_from_slice(&[0, 0, 0, 0]);
    b.extend_from_slice(&le32(54));
    b.extend_from_slice(&le32(40));
    b.extend_from_slice(&le32(width));
    b.extend_from_slice(&le32(height));
    b.extend_from_slice(&[1, 0, 24, 0]);
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&le32(row * height));
    b.extend_from_slice(&[0; 16]);
    for y in (0..height).rev() {
        for x in 0..width {
            let px = &rgba[(y * width + x) * 4..(y * width + x) * 4 + 4];
            // premultiply onto white so transparent areas stay light
            let blend = |c: u8| {
                let alpha = u16::from(px[3]);
                u8::try_from((u16::from(c) * alpha + 255 * (255 - alpha)) / 255).unwrap_or(u8::MAX)
            };
            b.extend_from_slice(&[blend(px[2]), blend(px[1]), blend(px[0])]);
        }
        b.resize(b.len() + (row - width * 3), 0);
    }
    Some(format!("data:image/bmp;base64,{}", STANDARD.encode(b)))
}

/// `srcset` for the image variants of one format.
fn srcset(img: &ImageDto, mime: &str) -> String {
    img.variants
        .iter()
        .filter(|v| v.mime == mime)
        .map(|v| format!("{} {}w", v.url, v.width))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Cover with an instant placeholder (thumbhash, else the dominant colour).
#[component]
pub fn Cover(
    #[prop(into)] title: String,
    image: Option<ImageDto>,
    #[prop(into, optional)] class: String,
    /// CSS `sizes` hint for `srcset`.
    #[prop(into, optional)]
    sizes: Option<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let alt = i18n.t_args("item-cover-alt", &[("title", title.clone().into())]);
    let base = format!("relative overflow-hidden rounded-md bg-muted aspect-[2/3] {class}");
    match image {
        None => view! {
            <div class=base role="img" aria-label=alt>
                <span class="absolute inset-0 flex items-center justify-center p-2 text-center text-xs text-muted-foreground">{title}</span>
            </div>
        }
        .into_any(),
        Some(img) => {
            let placeholder = img.thumbhash.as_deref().and_then(thumbhash_data_url);
            let style = match (&placeholder, &img.dominant_color) {
                (Some(url), _) => format!("background-image:url('{url}');background-size:cover"),
                (None, Some(c)) => format!("background-color:{c}"),
                (None, None) => String::new(),
            };
            let avif = srcset(&img, "image/avif");
            let webp = srcset(&img, "image/webp");
            let sizes = sizes.unwrap_or_else(|| "(min-width: 768px) 200px, 45vw".into());
            view! {
                <div class=base style=style>
                    <picture>
                        {(!avif.is_empty()).then(|| view! { <source type="image/avif" srcset=avif.clone() sizes=sizes.clone()/> })}
                        {(!webp.is_empty()).then(|| view! { <source type="image/webp" srcset=webp.clone() sizes=sizes.clone()/> })}
                        <img
                            src=img.url.clone()
                            alt=alt
                            loading="lazy"
                            decoding="async"
                            referrerpolicy="no-referrer"
                            class="absolute inset-0 h-full w-full object-cover"
                        />
                    </picture>
                </div>
            }
            .into_any()
        }
    }
}

/// Grid card linking to the item page.
#[component]
pub fn PosterCard(
    item: CatalogItemDto,
    #[prop(optional, into)] status: Option<String>,
) -> impl IntoView {
    let href = format!("/items/{}", item.id);
    let kind = crate::i18n::kind_label(&item.kind);
    let status = status.map(|s| t(&format!("status-{s}")));
    view! {
        <li class="flex flex-col gap-2">
            <A href=href attr:class="group flex flex-col gap-2 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <Cover title=item.title.clone() image=item.cover.clone()/>
                <span class="line-clamp-2 text-sm font-medium group-hover:underline">{item.title.clone()}</span>
            </A>
            <span class="text-xs text-muted-foreground">
                {kind}
                {item.year.map(|y| format!(" - {y}"))}
                {status.map(|s| format!(" - {s}"))}
            </span>
        </li>
    }
}

/// The person's rating as five stars in half steps over `ui::Rating`, stored as 0..=100
/// (DEC-018): 3.5 stars = 70. This wrapper only maps the scale and the translated labels.
#[component]
pub fn RatingInput(value: RwSignal<Option<u8>>, on_change: Callback<Option<u8>>) -> impl IntoView {
    let i18n = use_i18n();
    let label = i18n.t("item-rating");
    let option_label = Callback::new(move |v: u8| {
        i18n.t_args("item-rate", &[("stars", (f64::from(v) / 20.0).into())])
    });
    view! {
        <Rating
            id="item-rating"
            label=label
            value=value
            stars=5
            half_steps=true
            option_label=option_label
            on_change=on_change
        />
    }
}

/// A logged progress event in words (`{"type": "episode", "season": 2, "episode": 5}` ->
/// "Season 2, episode 5"); unknown types show their name.
#[must_use]
pub fn progress_label(i: &crate::i18n::I18n, p: &serde_json::Value) -> String {
    let n = |field: &str| p[field].as_u64();
    let num = |v: u64| fluent_bundle::FluentValue::from(v);
    match p["type"].as_str().unwrap_or_default() {
        "finished" => i.t("progress-finished"),
        "episode" => i.t_args(
            "progress-episode",
            &[
                ("season", num(n("season").unwrap_or(0))),
                ("episode", num(n("episode").unwrap_or(0))),
            ],
        ),
        "paged" => match (n("page"), n("chapter"), n("volume")) {
            (Some(v), _, _) => i.t_args("progress-page", &[("page", num(v))]),
            (None, Some(v), _) => i.t_args("progress-chapter", &[("chapter", num(v))]),
            (None, None, v) => i.t_args("progress-volume", &[("volume", num(v.unwrap_or(0)))]),
        },
        "position" => {
            let s = n("seconds").unwrap_or(0);
            let time = format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60);
            i.t_args("progress-position", &[("time", time.into())])
        }
        "session" => i.t("progress-session"),
        other => other.to_string(),
    }
}

/// Multi-select of the user's companions (excluding the self-companion).
#[component]
pub fn CompanionPicker(
    companions: Vec<CompanionDto>,
    selected: RwSignal<Vec<String>>,
) -> impl IntoView {
    let list: Vec<CompanionDto> = companions
        .into_iter()
        .filter(|c| !c.is_self && c.merged_into.is_none())
        .collect();
    view! {
        <fieldset class="flex flex-wrap gap-3">
            <legend class="mb-1 text-sm font-medium">{t("item-mark-done-with")}</legend>
            {list
                .into_iter()
                .map(|c| {
                    let id = c.id.clone();
                    let input_id = format!("with-{}", c.id);
                    let for_id = input_id.clone();
                    let id2 = id.clone();
                    view! {
                        <label for=for_id class="flex items-center gap-2 text-sm">
                            <input
                                id=input_id
                                type="checkbox"
                                class="h-4 w-4"
                                prop:checked=move || selected.get().contains(&id)
                                on:change=move |ev| {
                                    let on = event_target_checked(&ev);
                                    selected.update(|s| {
                                        s.retain(|x| x != &id2);
                                        if on {
                                            s.push(id2.clone());
                                        }
                                    });
                                }
                            />
                            {c.name.clone()}
                        </label>
                    }
                })
                .collect_view()}
        </fieldset>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::i18n::I18n;

    #[test]
    fn progress_reads_as_words_in_each_locale() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        let ep = serde_json::json!({"type": "episode", "season": 2, "episode": 5});
        assert_eq!(super::progress_label(&en, &ep), "Season 2, episode 5");
        assert_eq!(super::progress_label(&de, &ep), "Staffel 2, Folge 5");
        let done = serde_json::json!({"type": "finished"});
        assert_eq!(super::progress_label(&de, &done), "Abgeschlossen");
        let pos = serde_json::json!({"type": "position", "seconds": 3725});
        assert_eq!(super::progress_label(&en, &pos), "At 1:02:05");
        let page = serde_json::json!({"type": "paged", "page": 1212});
        assert_eq!(super::progress_label(&de, &page), "Seite 1.212");
    }

    #[test]
    fn thumbhash_becomes_a_bmp_data_url() {
        // a real thumbhash (from an image encoded by the assets pipeline)
        let rgba = vec![200u8; 16 * 16 * 4];
        let hash = thumbhash::rgba_to_thumb_hash(16, 16, &rgba);
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, hash);
        let url = super::thumbhash_data_url(&b64).unwrap();
        assert!(url.starts_with("data:image/bmp;base64,Qk"));
        assert!(super::thumbhash_data_url("not base64!").is_none());
    }
}
