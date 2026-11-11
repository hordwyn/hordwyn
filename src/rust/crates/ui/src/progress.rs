//! Determinate progress bar (`role="progressbar"`).

use leptos::prelude::*;

use crate::class::cn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgressSize {
    /// 4 px, for poster edges and dense rows.
    Thin,
    #[default]
    Regular,
}

/// Percentage of `value` in `0..=max`, clamped; `max == 0` counts as empty.
#[must_use]
pub fn percent(value: u32, max: u32) -> u32 {
    if max == 0 {
        return 0;
    }
    let p = u64::from(value.min(max)) * 100 / u64::from(max);
    u32::try_from(p).unwrap_or(100)
}

/// Bar with `aria-valuenow`/`aria-valuemax` (default max 100). `label` names it;
/// `value_text` is the human reading ("4 of 10 episodes") announced instead of the number.
#[component]
pub fn ProgressBar(
    #[prop(into)] label: String,
    #[prop(into)] value: Signal<u32>,
    #[prop(optional, into)] max: Option<u32>,
    #[prop(into, optional)] value_text: MaybeProp<String>,
    #[prop(optional)] size: ProgressSize,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let max = max.unwrap_or(100);
    view! {
        <div
            role="progressbar"
            aria-label=label
            aria-valuemin="0"
            aria-valuemax=max.to_string()
            aria-valuenow=move || value.get().min(max).to_string()
            aria-valuetext=move || value_text.get()
            class=move || cn(&[
                "w-full overflow-hidden rounded-full bg-muted",
                match size {
                    ProgressSize::Thin => "h-1",
                    ProgressSize::Regular => "h-2",
                },
                &class.get().unwrap_or_default(),
            ])
        >
            <div
                class="h-full rounded-full bg-primary transition-[width]"
                style:width=move || format!("{}%", percent(value.get(), max))
            ></div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn percent_is_clamped() {
        assert_eq!(super::percent(5, 10), 50);
        assert_eq!(super::percent(15, 10), 100);
        assert_eq!(super::percent(1, 0), 0);
        assert_eq!(super::percent(1, 3), 33);
    }
}
