//! Generic star rating on a 0..=100 scale (DEC-018), optionally in half-star steps.
//!
//! A `radiogroup` of one radio per step (APG radio group): the checked step is the only tab
//! stop, arrow keys move and select, Home/End jump, Delete/Backspace clears, and clicking the
//! checked step clears it. Each step's accessible name comes from `option_label`, so the app
//! decides how the value reads on the person's scale.

use leptos::prelude::*;

use crate::chip::focus_id;
use crate::class::cn;
use crate::icon::{Icon, IconName};

/// The selectable values for `stars` stars: `stars` (or `2 * stars` with half steps) evenly
/// spaced values ending at 100.
#[must_use]
pub fn rating_steps(stars: u8, half_steps: bool) -> Vec<u8> {
    let n = u32::from(stars.max(1)) * if half_steps { 2 } else { 1 };
    (1..=n)
        .map(|k| u8::try_from(k * 100 / n).unwrap_or(100))
        .collect()
}

/// How much of star `index` (zero-based, of `stars`) is filled for `value`: 0, 50 or 100.
#[must_use]
pub fn star_fill(value: Option<u8>, index: u8, stars: u8) -> u8 {
    let Some(v) = value else { return 0 };
    let scaled = u32::from(v) * u32::from(stars.max(1));
    let diff = scaled.saturating_sub(u32::from(index) * 100);
    match diff {
        100.. => 100,
        50..=99 => 50,
        _ => 0,
    }
}

/// What a key press does to the rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyAction {
    Set(u8),
    Clear,
    Ignore,
}

impl KeyAction {
    const fn value(self) -> Option<u8> {
        match self {
            Self::Set(v) => Some(v),
            Self::Clear | Self::Ignore => None,
        }
    }
}

fn key_step(key: &str, steps: &[u8], current: Option<u8>) -> KeyAction {
    let Some(last) = steps.len().checked_sub(1) else {
        return KeyAction::Ignore;
    };
    let pos = current.and_then(|c| steps.iter().position(|s| *s == c));
    let next = match key {
        "ArrowRight" | "ArrowUp" => pos.map_or(0, |p| (p + 1).min(last)),
        "ArrowLeft" | "ArrowDown" => pos.map_or(0, |p| p.saturating_sub(1)),
        "Home" => 0,
        "End" => last,
        "Delete" | "Backspace" => return KeyAction::Clear,
        _ => return KeyAction::Ignore,
    };
    KeyAction::Set(steps[next])
}

#[component]
pub fn Rating(
    /// Prefix of the step ids (`{id}-{value}`).
    #[prop(into)]
    id: String,
    /// Accessible name of the group, e.g. "Your rating".
    #[prop(into)]
    label: String,
    value: RwSignal<Option<u8>>,
    /// Number of stars (default 5).
    #[prop(optional)]
    stars: Option<u8>,
    #[prop(optional)] half_steps: bool,
    /// Accessible name of the step with this value (1..=100), e.g. "Rate 3.5 of 5".
    option_label: Callback<u8, String>,
    #[prop(optional, into)] on_change: Option<Callback<Option<u8>>>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let stars = stars.unwrap_or(5).max(1);
    let steps = rating_steps(stars, half_steps);
    let base = StoredValue::new(id);
    let set = move |v: Option<u8>| {
        value.set(v);
        if let Some(cb) = on_change {
            cb.run(v);
        }
    };
    let first = steps[0];
    let tab_stop = move |v: u8| match value.get() {
        Some(c) if steps_contains(stars, half_steps, c) => c == v,
        _ => v == first,
    };
    let key_steps = steps.clone();
    let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
        let action = key_step(&ev.key(), &key_steps, value.get_untracked());
        if action == KeyAction::Ignore {
            return;
        }
        ev.prevent_default();
        let next = action.value();
        set(next);
        focus_id(&format!("{}-{}", base.get_value(), next.unwrap_or(first)));
    };
    let cell = if half_steps { "size-12" } else { "size-8" };
    let per_star: usize = if half_steps { 2 } else { 1 };
    let radio = move |v: u8, pos: &'static str| {
        view! {
            <button
                type="button"
                role="radio"
                id=format!("{}-{v}", base.get_value())
                aria-checked=move || if value.get() == Some(v) { "true" } else { "false" }
                aria-label=option_label.run(v)
                tabindex=move || if tab_stop(v) { "0" } else { "-1" }
                on:click=move |_| set(if value.get_untracked() == Some(v) { None } else { Some(v) })
                class=cn(&[
                    "absolute inset-y-0 cursor-pointer rounded-sm focus-visible:outline-none \
                     focus-visible:ring-2 focus-visible:ring-ring",
                    pos,
                ])
            ></button>
        }
    };
    view! {
        <div
            role="radiogroup"
            aria-label=label
            on:keydown=on_keydown
            class=move || cn(&["inline-flex", &class.get().unwrap_or_default()])
        >
            {(0..stars).map(|i| {
                let own = &steps[usize::from(i) * per_star..(usize::from(i) + 1) * per_star];
                let radios = if half_steps {
                    view! { {radio(own[0], "left-0 w-1/2")}{radio(own[1], "right-0 w-1/2")} }.into_any()
                } else {
                    radio(own[0], "inset-x-0").into_any()
                };
                view! {
                    <span class=cn(&["relative inline-flex shrink-0", cell])>
                        <Icon name=IconName::Star class=cn(&[cell, "text-muted-foreground"])/>
                        <span
                            class="pointer-events-none absolute inset-y-0 left-0 overflow-hidden"
                            style:width=move || format!("{}%", star_fill(value.get(), i, stars))
                        >
                            <Icon name=IconName::Star class=cn(&[cell, "fill-current text-primary"])/>
                        </span>
                        {radios}
                    </span>
                }
            }).collect_view()}
        </div>
    }
}

fn steps_contains(stars: u8, half_steps: bool, v: u8) -> bool {
    rating_steps(stars, half_steps).contains(&v)
}

#[cfg(test)]
mod tests {
    use super::{KeyAction, key_step};

    #[test]
    fn keys_move_clamp_and_clear() {
        let s = [20, 40, 60, 80, 100];
        assert_eq!(key_step("ArrowRight", &s, None), KeyAction::Set(20));
        assert_eq!(key_step("ArrowRight", &s, Some(100)), KeyAction::Set(100));
        assert_eq!(key_step("ArrowLeft", &s, Some(20)), KeyAction::Set(20));
        assert_eq!(key_step("ArrowDown", &s, Some(60)), KeyAction::Set(40));
        assert_eq!(key_step("End", &s, None), KeyAction::Set(100));
        assert_eq!(key_step("Delete", &s, Some(60)), KeyAction::Clear);
        assert_eq!(key_step("x", &s, Some(60)), KeyAction::Ignore);
        assert_eq!(key_step("End", &[], None), KeyAction::Ignore);
    }
}
