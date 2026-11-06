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
        remember(open, key, force_open);
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
