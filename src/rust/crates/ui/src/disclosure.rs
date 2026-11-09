//! Progressive disclosure (addendum-1 1.2, rules D3 and D5-D7).
//!
//! - `DisclosureMode::Native`: `<details>`/`<summary>` for static content.
//! - `DisclosureMode::Button`: the APG disclosure pattern for content with form controls or
//!   lazy loading: a `<button aria-expanded aria-controls>` inside a heading of the caller's
//!   level; collapsed content is `hidden`.
//!
//! The summary line shows the title, a one-line contents hint and an optional status slot
//! (D5). `force_open` opens the section (errors, required empty fields). With `storage_key`
//! the open state is remembered in `localStorage` (D6; build the key with
//! [`disclosure_storage_key`]). Sections inside a [`DisclosureGroup`] get "Expand all /
//! Collapse all" once there are four or more (D6).

use leptos::prelude::*;

use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::class::cn;
use crate::heading::heading;
use crate::icon::{Icon, IconName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisclosureMode {
    #[default]
    Native,
    Button,
}

/// `localStorage` key for a remembered disclosure: `app-disclosure:<page>:<section>`.
#[must_use]
pub fn disclosure_storage_key(page: &str, section: &str) -> String {
    format!("app-disclosure:{page}:{section}")
}

#[derive(Clone, Copy)]
struct GroupCtx {
    opens: RwSignal<Vec<RwSignal<bool>>>,
}

fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

/// Restore the remembered state once (browser only), then write every change back.
fn remember(open: RwSignal<bool>, key: String, force_open: Signal<bool>) {
    Effect::new(move |prev: Option<()>| {
        let is_open = open.get();
        let Some(s) = storage() else { return };
        if prev.is_none() {
            match s.get_item(&key).ok().flatten().as_deref() {
                Some("1") => open.set(true),
                Some("0") if !force_open.get_untracked() => open.set(false),
                _ => {}
            }
        } else {
            let _ = s.set_item(&key, if is_open { "1" } else { "0" });
        }
    });
}

#[component]
fn SummaryLine(
    open: RwSignal<bool>,
    title: String,
    summary: MaybeProp<String>,
    status: Option<ViewFn>,
) -> impl IntoView {
    view! {
        {move || if open.get() {
            view! { <Icon name=IconName::ChevronDown/> }
        } else {
            view! { <Icon name=IconName::ChevronRight/> }
        }}
        <span class="flex flex-1 flex-wrap items-baseline gap-x-2">
            <span class="font-semibold">{title}</span>
            {move || summary.get().map(|s| view! { <span class="text-sm font-normal text-muted-foreground">{s}</span> })}
        </span>
        {status.map(|s| view! { <span class="shrink-0 font-normal">{s.run()}</span> })}
    }
}

const HEADER: &str = "flex w-full cursor-pointer items-center gap-2 rounded-md px-4 py-3 text-left \
    focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

#[component]
pub fn Disclosure(
    /// Element id; the content panel is `{id}-content`.
    #[prop(into)]
    id: String,
    #[prop(into)] title: String,
    /// One-line contents hint shown next to the title (D5).
    #[prop(into, optional)]
    summary: MaybeProp<String>,
    /// Status slot (a `StatusDot` or badge), visible while closed (D5).
    #[prop(optional, into)]
    status: Option<ViewFn>,
    #[prop(optional)] mode: DisclosureMode,
    /// Heading level of the header in `Button` mode (default 3).
    #[prop(optional)]
    level: Option<u8>,
    /// Open state; created closed when not given.
    #[prop(optional)]
    open: Option<RwSignal<bool>>,
    #[prop(into, optional)] storage_key: MaybeProp<String>,
    #[prop(into, optional)] force_open: Signal<bool>,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let open = open.unwrap_or_else(|| RwSignal::new(false));
    if force_open.get_untracked() {
        open.set(true);
    }
    Effect::new(move |_| {
        if force_open.get() {
            open.set(true);
        }
    });
    if let Some(key) = storage_key.get_untracked() {
        // after `remember`, so its restore is in place before the first report
        remember(open, key.clone(), force_open);
        if let Some(watch) = use_context::<DisclosureWatch>() {
            watched(watch, key, open);
        }
    }
    if let Some(group) = use_context::<GroupCtx>() {
        group.opens.update(|v| v.push(open));
    }
    let content_id = format!("{id}-content");
    let wrapper = move || {
        cn(&[
            "rounded-xl border bg-card text-card-foreground",
            &class.get().unwrap_or_default(),
        ])
    };
    let line = view! { <SummaryLine open=open title=title summary=summary status=status/> };
    match mode {
        DisclosureMode::Native => view! {
            <details
                id=id
                open=move || open.get()
                on:toggle=move |ev| {
                    let now = event_target::<web_sys::HtmlDetailsElement>(&ev).open();
                    if now != open.get_untracked() {
                        open.set(now);
                    }
                }
                class=wrapper
            >
                <summary class=cn(&[HEADER, "list-none [&::-webkit-details-marker]:hidden"])>{line}</summary>
                <div id=content_id class="px-4 pb-4">{children()}</div>
            </details>
        }
        .into_any(),
        DisclosureMode::Button => {
            let button = view! {
                <button
                    type="button"
                    aria-expanded=move || if open.get() { "true" } else { "false" }
                    aria-controls=content_id.clone()
                    on:click=move |_| open.update(|o| *o = !*o)
                    class=HEADER
                >
                    {line}
                </button>
            };
            view! {
                <div id=id class=wrapper>
                    {heading(level.unwrap_or(3), None, "text-base".into(), button)}
                    <div id=content_id hidden=move || !open.get() class="px-4 pb-4">{children()}</div>
                </div>
            }
            .into_any()
        }
    }
}

/// Container for several disclosures. From `threshold` (default 4) sections on it shows one
/// toggle button: `expand_label` while any section is closed, else `collapse_label`.
#[component]
pub fn DisclosureGroup(
    #[prop(into)] expand_label: String,
    #[prop(into)] collapse_label: String,
    #[prop(optional)] threshold: Option<usize>,
    #[prop(into, optional)] class: MaybeProp<String>,
    children: Children,
) -> impl IntoView {
    let opens = RwSignal::new(Vec::<RwSignal<bool>>::new());
    provide_context(GroupCtx { opens });
    // render the sections first so they are registered before the toggle is evaluated
    let sections = children();
    let threshold = threshold.unwrap_or(4);
    let all_open = move || opens.with(|v| v.iter().all(|o| o.get()));
    let labels = StoredValue::new((expand_label, collapse_label));
    view! {
        <div class=move || cn(&["grid gap-3", &class.get().unwrap_or_default()])>
            {move || (opens.with(Vec::len) >= threshold).then(|| view! {
                <div class="flex justify-end">
                    <Button
                        variant=ButtonVariant::Ghost
                        size=ButtonSize::Sm
                        on:click=move |_| {
                            opens.with_untracked(|v| {
                                let target = !v.iter().all(|o| o.get_untracked());
                                for o in v {
                                    o.set(target);
                                }
                            });
                        }
                    >
                        {move || labels.with_value(|(e, c)| if all_open() { c.clone() } else { e.clone() })}
                    </Button>
                </div>
            })}
            {sections}
        </div>
    }
}

/// What a remembered disclosure reports (addendum-1 D16): it was shown on a page view, the
/// person opened it, and opened it within [`OPENED_EARLY_MS`] of it showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisclosureEvent {
    Shown,
    Opened,
    OpenedEarly,
}

/// "Opened early": within this long after the disclosure showed (D16: 2 s of page load).
pub const OPENED_EARLY_MS: f64 = 2_000.0;

/// The app's watcher in context (WP4-5): every disclosure with a `storage_key` reports its
/// [`DisclosureEvent`]s with the key, and the watcher knows which of them are open (the open
/// sections at the moment of a save). Without a watcher nothing is reported.
#[derive(Clone, Copy)]
pub struct DisclosureWatch {
    report: Callback<(String, DisclosureEvent)>,
    mounted: StoredValue<Vec<(u64, String, RwSignal<bool>)>>,
    next: StoredValue<u64>,
}

impl DisclosureWatch {
    #[must_use]
    pub fn new(report: Callback<(String, DisclosureEvent)>) -> Self {
        Self {
            report,
            mounted: StoredValue::new(Vec::new()),
            next: StoredValue::new(0),
        }
    }

    /// Storage keys of the disclosures open now, once each.
    #[must_use]
    pub fn open_keys(&self) -> Vec<String> {
        let keys: Vec<String> = self
            .mounted
            .try_with_value(|m| {
                m.iter()
                    .filter(|(_, _, open)| open.try_get_untracked().unwrap_or(false))
                    .map(|(_, k, _)| k.clone())
                    .collect()
            })
            .unwrap_or_default();
        // once each, in mount order, also when two of them are not adjacent
        let mut seen = std::collections::HashSet::new();
        keys.into_iter()
            .filter(|k| seen.insert(k.clone()))
            .collect()
    }

    fn register(&self, key: String, open: RwSignal<bool>) -> u64 {
        let id = self.next.try_update_value(|n| {
            *n += 1;
            *n
        });
        let id = id.unwrap_or_default();
        self.mounted.try_update_value(|m| m.push((id, key, open)));
        id
    }

    fn unregister(&self, id: u64) {
        self.mounted
            .try_update_value(|m| m.retain(|(i, _, _)| *i != id));
    }
}

fn now_ms() -> f64 {
    window().performance().map_or(0.0, |p| p.now())
}

/// Report shown/opened for one remembered disclosure (browser only: effects do not run on
/// the server). A state restored from `localStorage` before the first run is not "opened".
fn watched(watch: DisclosureWatch, key: String, open: RwSignal<bool>) {
    let id = watch.register(key.clone(), open);
    on_cleanup(move || watch.unregister(id));
    Effect::new(move |prev: Option<(bool, f64)>| {
        let is_open = open.get();
        let Some((was_open, shown_at)) = prev else {
            watch.report.run((key.clone(), DisclosureEvent::Shown));
            return (is_open, now_ms());
        };
        if is_open && !was_open {
            watch.report.run((key.clone(), DisclosureEvent::Opened));
            if now_ms() - shown_at <= OPENED_EARLY_MS {
                watch
                    .report
                    .run((key.clone(), DisclosureEvent::OpenedEarly));
            }
        }
        (is_open, shown_at)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_keys_name_each_key_once_even_when_not_adjacent() {
        // UX4-22: `dedup` on an unsorted list counted a shared key twice
        let owner = Owner::new();
        owner.with(|| {
            let watch = DisclosureWatch::new(Callback::new(|_| ()));
            for key in ["a", "b", "a"] {
                watch.register(key.into(), RwSignal::new(true));
            }
            watch.register("c".into(), RwSignal::new(false));
            assert_eq!(watch.open_keys(), ["a", "b"]);
        });
    }
}
