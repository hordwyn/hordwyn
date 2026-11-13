//! The Rules tab of Settings > Notifications (design.md 6.8, 5.8; mockup `notifications.html`
//! annotation 4): events x channels with one switch per cell, the delivery in words, instance
//! defaults labelled and locked rows with a lock. The first switch of a new channel offers the
//! instance's suggested events in one click. Rules for one list or person (the `Filter` of a
//! subscription) are listed below the matrix, with "Add a rule for one list, item or person".
//! The logic is in [`crate::notify_rules`].

use contracts::{ChannelDto, CompanionDto, ListDto, SubscriptionDto};
use leptos::prelude::*;
use serde_json::json;
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant,
    FormField, Icon, IconName, Select, SelectOption, Table, TableBody, TableCell, TableHead,
    TableHeader, TableRow,
};

use crate::api;
use crate::i18n::{t, use_i18n};
use crate::notifications::{NotifyState, event_label, frames, notify_error, when_label};
use crate::notify_rules::{RuleRow, filtered, is_new, rows, suggested_off, toggled};
use crate::rest;
use crate::settings::kept;

/// The zone for a rule made here: the browser's, else UTC (the server checks the name).
fn zone() -> String {
    rest::browser_time_zone().unwrap_or_else(|| "UTC".into())
}

/// The first switch of a new channel, waiting for the answer (design.md 5.8).
#[derive(Debug, Clone, PartialEq)]
struct Offer {
    channel: String,
    name: String,
    /// The rule the switch asked for.
    first: SubscriptionDto,
    /// The instance's suggested events that do not reach the channel yet.
    events: Vec<String>,
    /// The switch that asked, when it had the focus ([`switch_selector`]): the answer hands
    /// the focus back to it, also when the view was rendered afresh in between.
    switch: Option<String>,
}

/// Save rules one after the other, then reload the page; the first error is kept.
fn save(state: NotifyState, error: RwSignal<Option<String>>) -> Action<Vec<SubscriptionDto>, ()> {
    Action::new(move |subs: &Vec<SubscriptionDto>| {
        let subs = subs.clone();
        async move {
            error.set(None);
            for s in subs {
                let body = serde_json::to_value(&s).ok();
                if let Err(e) = rest::send("PUT", "/notifications/subscriptions", body).await {
                    error.set(Some(notify_error(&api::error_code(&e))));
                    break;
                }
            }
            state.refresh();
        }
    })
}

/// The heading a row is listed under: its event group, or "several events" for patterns.
fn section(event: &str) -> String {
    if event.contains('*') {
        "patterns".into()
    } else {
        event.split('.').next().unwrap_or_default().to_string()
    }
}

/// What the rules are built from: the events, the rules and the channels without their
/// health. A later read that changes none of them (a new inbox row, a finished delivery)
/// leaves the matrix and the form below it as they are (SEC0-3).
type Base = (
    Vec<contracts::EventTypeDto>,
    Vec<SubscriptionDto>,
    Vec<ChannelDto>,
);

/// The selector of the switch of `event` for the channel `channel`. The matrix is built anew
/// when a rule changes and a switch is disabled while its rule is saved; either takes the
/// focus away from the switch that was pressed, and this is how it is found again.
#[must_use]
pub fn switch_selector(event: &str, channel: &str) -> String {
    format!("[data-nf-switch=\"{}\"]", switch_key(event, channel))
}

/// Event names are dotted words with `*` and channel ids are UUIDs: neither holds a quote.
fn switch_key(event: &str, channel: &str) -> String {
    format!("{event}|{channel}")
}

/// Put the focus back on `selector` on the next frame, unless the person has moved it to
/// another control meanwhile. The page is not scrolled: whoever scrolled on while the rule
/// was saved stays there (review POL-4).
fn refocus_soon(selector: String) {
    request_animation_frame(move || {
        if crate::keyboard::focus_is_nowhere() {
            let _ = crate::keyboard::focus_in_place(&selector);
        }
    });
}

fn own_rules(subscriptions: &[SubscriptionDto]) -> Vec<SubscriptionDto> {
    subscriptions
        .iter()
        .filter(|s| !s.instance_default)
        .cloned()
        .collect()
}

#[component]
pub fn Rules(state: NotifyState) -> impl IntoView {
    let base: Memo<Base> = Memo::new(move |_| {
        state.page.with(|p| {
            (
                p.events.clone(),
                p.subscriptions.clone(),
                frames(&p.channels),
            )
        })
    });
    let error = RwSignal::new(None::<String>);
    let offer = kept("nf-offer", || None::<Offer>);
    let save = save(state, error);
    // the switch that was pressed last, when it had the focus then: it gets the focus back
    // when its rule is saved and once more when the matrix is built from the answer (sec0
    // "left open", v1-polish). Where a click does not focus a button (Safari, Firefox on
    // macOS) nothing is remembered and no focus is put where none was (review POL-4)
    let pressed = StoredValue::new(None::<String>);
    let pending = save.pending();
    Effect::new(move |was: Option<bool>| {
        let now = pending.get();
        if was == Some(true)
            && !now
            && let Some(selector) = pressed.get_value()
        {
            // a refused rule changes nothing: no new matrix will ask for the focus
            if error.get_untracked().is_some() {
                pressed.set_value(None);
            }
            refocus_soon(selector);
        }
        now
    });
    let toggle = Callback::new(move |(row, channel, on): (usize, usize, bool)| {
        let (events, subscriptions, channels) = base.get_untracked();
        let (rows, own) = (rows(&events, &subscriptions), own_rules(&subscriptions));
        let (Some(r), Some(c)) = (rows.get(row), channels.get(channel)) else {
            return;
        };
        let first = toggled(r, &channels, &c.id, on, &zone());
        let selector = switch_selector(&r.event, &c.id);
        let switch = crate::keyboard::focus_is_on(&selector).then_some(selector);
        pressed.set_value(switch.clone());
        if on && is_new(&rows, &own, &c.id) {
            let mut events = suggested_off(&rows, &c.id);
            events.retain(|e| *e != r.event);
            if !events.is_empty() {
                offer.set(Some(Offer {
                    channel: c.id.clone(),
                    name: c.name.clone(),
                    first,
                    events,
                    switch,
                }));
                return;
            }
        }
        save.dispatch(vec![first]);
    });
    let answer = Callback::new(move |all_of_them: bool| {
        let Some(o) = offer.get_untracked() else {
            return;
        };
        offer.set(None);
        // the offer outlives a render of this view (`kept`); the switch is kept with it
        pressed.set_value(o.switch.clone());
        let (events, subscriptions, channels) = base.get_untracked();
        let mut subs = vec![o.first.clone()];
        if all_of_them {
            subs.extend(
                rows(&events, &subscriptions)
                    .iter()
                    .filter(|r| o.events.contains(&r.event))
                    .map(|r| toggled(r, &channels, &o.channel, true, &zone())),
            );
        }
        save.dispatch(subs);
    });
    view! {
        <div class="mt-4 grid gap-4 pb-[var(--tab-bar-height)]">
            <p class="text-sm text-muted-foreground">{t("notify-rules-lead")}</p>
            {move || base.with(|(_, _, channels)| channels.is_empty()).then(|| view! {
                <Alert variant=AlertVariant::Info role=AlertRole::None>{t("notify-rules-no-channels")}</Alert>
            })}
            {move || offer.get().map(|o| view! { <OfferPrompt o=o answer=answer/> })}
            {move || error.get().map(|e| view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{e}</Alert> })}
            {move || {
                let (events, subscriptions, channels) = base.get();
                // a matrix built while a rule is being saved is not the answer to that save
                if !pending.get_untracked()
                    && let Some(selector) = pressed.get_value()
                {
                    pressed.set_value(None);
                    refocus_soon(selector);
                }
                view! {
                    <Matrix rows=rows(&events, &subscriptions) channels=channels toggle=toggle busy=Signal::from(save.pending())/>
                }
            }}
            <FilteredRules base=base state=state/>
        </div>
    }
}

#[component]
fn OfferPrompt(o: Offer, answer: Callback<bool>) -> impl IntoView {
    let i18n = use_i18n();
    let names = o
        .events
        .iter()
        .map(|e| event_label(e))
        .collect::<Vec<_>>()
        .join(", ");
    let text = i18n.t_args(
        "notify-offer-body",
        &[
            ("channel", o.name.clone().into()),
            ("events", names.into()),
            ("count", o.events.len().into()),
        ],
    );
    view! {
        <Alert variant=AlertVariant::Info role=AlertRole::Status
            title=i18n.t_args("notify-offer-title", &[("channel", o.name.into())])
            actions=move || view! {
                <Button size=ButtonSize::Sm on:click=move |_| answer.run(true)>{t("notify-offer-yes")}</Button>
                <Button size=ButtonSize::Sm variant=ButtonVariant::Outline on:click=move |_| answer.run(false)>{t("notify-offer-no")}</Button>
            }>
            {text}
        </Alert>
    }
}

#[component]
fn Matrix(
    rows: Vec<RuleRow>,
    channels: Vec<ChannelDto>,
    toggle: Callback<(usize, usize, bool)>,
    busy: Signal<bool>,
) -> impl IntoView {
    let width = channels.len() + 3;
    let heads = channels
        .iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>()
        .into_iter()
        .map(|n| view! { <TableHead>{n}</TableHead> })
        .collect_view();
    // one row group per section, headed by a `rowgroup` header spanning the table (UX3-26:
    // `colgroup` named columns that do not exist)
    let mut groups: Vec<(String, Vec<(usize, RuleRow)>)> = Vec::new();
    for (ri, r) in rows.into_iter().enumerate() {
        let s = section(&r.event);
        match groups.last_mut() {
            Some((last, members)) if *last == s => members.push((ri, r)),
            _ => groups.push((s, vec![(ri, r)])),
        }
    }
    let body = groups
        .into_iter()
        .map(|(s, members)| {
            let rows = members
                .into_iter()
                .map(|(ri, r)| view! { <MatrixRow r=r index=ri channels=channels.clone() toggle=toggle busy=busy/> })
                .collect_view();
            view! {
                <TableBody>
                    <tr class="border-b bg-muted/50">
                        <th scope="rowgroup" colspan=width.to_string() class="px-2 py-1.5 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                            {t(&format!("notify-section-{s}"))}
                        </th>
                    </tr>
                    {rows}
                </TableBody>
            }
        })
        .collect_view();
    // not `stacked` (DEC-388, WP4-2): the matrix is two-dimensional (events x channels, one
    // switch per cell, section header rows); as cards every switch would need its channel
    // name repeated and the comparison across a row is the point, so it keeps the focusable
    // scroll wrapper (DEC-337; data tables are exempt from WCAG 1.4.10)
    view! {
        <Table attr:data-testid="nf-rules">
            <caption class="sr-only">{t("notify-rules")}</caption>
            <TableHeader>
                <TableRow>
                    <TableHead>{t("notify-event")}</TableHead>
                    <TableHead>{t("notify-inbox")}</TableHead>
                    {heads}
                    <TableHead>{t("notify-when")}</TableHead>
                </TableRow>
            </TableHeader>
            {body}
        </Table>
    }
}

#[component]
fn MatrixRow(
    r: RuleRow,
    index: usize,
    channels: Vec<ChannelDto>,
    toggle: Callback<(usize, usize, bool)>,
    busy: Signal<bool>,
) -> impl IntoView {
    let i18n = use_i18n();
    let label = event_label(&r.event);
    let when = r
        .deciding()
        .filter(|s| s.enabled)
        .map_or_else(|| t("notify-when-inbox-only"), when_label);
    let locked = r.locked();
    let cells = channels
        .iter()
        .enumerate()
        .map(|(ci, c)| {
            let on = r.reaches(&c.id);
            let name = i18n.t_args(
                "notify-switch-named",
                &[("event", label.clone().into()), ("channel", c.name.clone().into())],
            );
            let state = t(if on { "notify-on" } else { "notify-off" });
            let key = switch_key(&r.event, &c.id);
            if locked {
                view! {
                    <TableCell>
                        <span class="inline-flex items-center gap-1 text-sm">
                            <Icon name=IconName::Lock label=t("notify-locked")/>{state}
                        </span>
                    </TableCell>
                }
                .into_any()
            } else {
                view! {
                    <TableCell>
                        <button type="button" role="switch" aria-checked=if on { "true" } else { "false" }
                            aria-label=name disabled=move || busy.get()
                            data-nf-switch=key
                            on:click=move |_| toggle.run((index, ci, !on))
                            class=if on { SWITCH_ON } else { SWITCH_OFF }>
                            <span class=if on { KNOB_ON } else { KNOB_OFF }></span>
                        </button>
                    </TableCell>
                }
                .into_any()
            }
        })
        .collect_view();
    view! {
        <TableRow>
            <th scope="row" class="p-2 text-left align-middle font-normal">
                <span class="block">{label}</span>
                {r.from_default().then(|| view! { <Badge variant=BadgeVariant::Secondary>{t("notify-default")}</Badge> })}
                {locked.then(|| view! { <Badge variant=BadgeVariant::Outline class="ml-1">{t("notify-locked")}</Badge> })}
            </th>
            <TableCell><Icon name=IconName::Check label=t("notify-inbox-always")/></TableCell>
            {cells}
            <TableCell class="text-sm text-muted-foreground">{when}</TableCell>
        </TableRow>
    }
}

const SWITCH_ON: &str = "inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full \
    border-2 border-transparent bg-primary transition-colors focus-visible:outline-none \
    focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-wait disabled:opacity-60";
const SWITCH_OFF: &str = "inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full \
    border-2 border-transparent bg-input transition-colors focus-visible:outline-none \
    focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-wait disabled:opacity-60";
const KNOB_ON: &str =
    "pointer-events-none block size-5 translate-x-5 rounded-full bg-background shadow-lg";
const KNOB_OFF: &str = "pointer-events-none block size-5 rounded-full bg-background shadow-lg";

/// What a filtered rule is about, in words (`List: Films`, `Person: Pia`).
fn filter_label(s: &SubscriptionDto, lists: &[ListDto], people: &[CompanionDto]) -> String {
    let i18n = use_i18n();
    let ids = |k: &str| -> Vec<String> {
        s.filter[k]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };
    let mut parts = Vec::new();
    for id in ids("lists") {
        let name = lists
            .iter()
            .find(|l| l.id == id)
            .map_or(id, |l| l.name.clone());
        parts.push(i18n.t_args("notify-filter-list", &[("name", name.into())]));
    }
    for id in ids("companions") {
        let name = people
            .iter()
            .find(|c| c.id == id)
            .map_or(id, |c| c.name.clone());
        parts.push(i18n.t_args("notify-filter-person", &[("name", name.into())]));
    }
    let items = ids("items").len();
    if items > 0 {
        parts.push(i18n.t_args("notify-filter-items", &[("count", items.into())]));
    }
    parts.join(", ")
}

#[component]
fn FilteredRules(base: Memo<Base>, state: NotifyState) -> impl IntoView {
    let picks = Resource::new(
        || (),
        |()| async {
            let lists = api::lists().await.unwrap_or_default();
            let people = api::companions().await.unwrap_or_default();
            Ok::<_, ServerFnError>((lists, people))
        },
    );
    let remove = Action::new(move |id: &String| {
        let path = format!("/notifications/subscriptions/{id}");
        async move {
            let _ = rest::send("DELETE", &path, None).await;
            state.refresh();
        }
    });
    view! {
        <section class="grid gap-3" aria-labelledby="nf-filtered">
            <h3 id="nf-filtered" class="font-semibold">{t("notify-filtered")}</h3>
            <Suspense fallback=|| ()>
                {move || picks.get().map(|r| {
                    let (lists, people) = r.unwrap_or_default();
                    // the form is built once (the events do not change while the page is
                    // open); the list above it follows the rules in place
                    let events = base.with_untracked(|(events, _, _)| events.clone());
                    let (names, persons) = (lists.clone(), people.clone());
                    view! {
                        {move || base.with(|(_, subscriptions, channels)| {
                            let rules: Vec<SubscriptionDto> =
                                own_rules(subscriptions).into_iter().filter(filtered).collect();
                            view! {
                                {rules.is_empty().then(|| view! { <p class="text-sm text-muted-foreground">{t("notify-filtered-empty")}</p> })}
                                <ul class="grid gap-2" aria-labelledby="nf-filtered">
                                    {rules.into_iter().map(|s| {
                                        let id = s.id.clone().unwrap_or_default();
                                        let what = filter_label(&s, &names, &persons);
                                        let to = targets_label(&s, channels);
                                        let label = event_label(&s.event);
                                        let del = use_i18n().t_args("notify-delete-named", &[("name", label.clone().into())]);
                                        view! {
                                            <li class="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2 text-sm">
                                                <span class="min-w-0 flex-1">
                                                    <span class="font-medium">{label}</span>
                                                    " - "{what}" - "{when_label(&s)}" - "{to}
                                                </span>
                                                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost attr:aria-label=del
                                                    on:click=move |_| { remove.dispatch(id.clone()); }>{t("notify-delete")}</Button>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }
                        })}
                        <AddFiltered events=events lists=lists people=people base=base state=state/>
                    }
                })}
            </Suspense>
        </section>
    }
}

fn targets_label(s: &SubscriptionDto, channels: &[ChannelDto]) -> String {
    match s.targets["type"].as_str() {
        Some("only") => {
            let names: Vec<String> = channels
                .iter()
                .filter(|c| crate::notify_rules::sends_to(s, &c.id))
                .map(|c| c.name.clone())
                .collect();
            if names.is_empty() {
                t("notify-inbox-only")
            } else {
                names.join(", ")
            }
        }
        _ => t("notify-all-channels"),
    }
}

fn opt(value: &str, label: String) -> SelectOption {
    SelectOption {
        value: value.into(),
        label,
    }
}

/// What a filtered rule can be about: a list or a person.
fn subject_options(
    i18n: &crate::i18n::I18n,
    lists: &[ListDto],
    people: &[CompanionDto],
) -> Vec<SelectOption> {
    let mut subjects = vec![opt("", i18n.t("notify-filter-pick"))];
    subjects.extend(lists.iter().map(|l| {
        opt(
            &format!("list:{}", l.id),
            i18n.t_args("notify-filter-list", &[("name", l.name.clone().into())]),
        )
    }));
    subjects.extend(
        people
            .iter()
            .filter(|c| !c.is_self && c.merged_into.is_none())
            .map(|c| {
                opt(
                    &format!("person:{}", c.id),
                    i18n.t_args("notify-filter-person", &[("name", c.name.clone().into())]),
                )
            }),
    );
    subjects
}

/// "Choose an item...": the search palette picks the item (the scoped pick, DEC-344); it
/// joins the subjects as "Item: <title>" and is chosen.
fn item_pick(subject: RwSignal<String>, subjects: RwSignal<Vec<SelectOption>>) -> Callback<()> {
    let scope = expect_context::<crate::palette::PaletteScope>().0;
    let open = expect_context::<crate::palette::PaletteOpen>().0;
    let i18n = use_i18n();
    let done = Callback::new(move |(id, label): (String, String)| {
        let value = format!("item:{id}");
        let text = i18n.t_args("notify-filter-item", &[("name", label.into())]);
        subjects.update(|s| {
            if !s.iter().any(|o| o.value == value) {
                s.push(opt(&value, text));
            }
        });
        subject.set(value);
    });
    Callback::new(move |()| {
        scope.set(Some(crate::palette::PaletteTarget::RuleSubject { done }));
        open.set(true);
    })
}

#[component]
fn AddFiltered(
    events: Vec<contracts::EventTypeDto>,
    lists: Vec<ListDto>,
    people: Vec<CompanionDto>,
    base: Memo<Base>,
    state: NotifyState,
) -> impl IntoView {
    let event = RwSignal::new(events.first().map(|e| e.id.clone()).unwrap_or_default());
    let subject = RwSignal::new(String::new());
    let target = RwSignal::new(String::new());
    let mode = RwSignal::new("immediate".to_string());
    let hour = RwSignal::new("8".to_string());
    let error = RwSignal::new(None::<String>);
    let event_options: Vec<SelectOption> = events
        .iter()
        .map(|e| opt(&e.id, event_label(&e.id)))
        .collect();
    let i18n = use_i18n();
    let subjects = RwSignal::new(subject_options(&i18n, &lists, &people));
    let pick_item = item_pick(subject, subjects);
    // the channels as the latest read has them: a channel added while the form is open
    // can be chosen without the form being built anew
    let all_channels = t("notify-all-channels");
    let targets = Signal::derive(move || {
        let mut targets = vec![opt("", all_channels.clone())];
        base.with(|(_, _, channels)| {
            targets.extend(channels.iter().map(|c| opt(&c.id, c.name.clone())));
        });
        targets
    });
    let modes = vec![
        opt("immediate", t("notify-mode-immediate")),
        opt("digest", t("notify-mode-digest")),
    ];
    let hours: Vec<SelectOption> = (0..24)
        .map(|h| opt(&h.to_string(), i18n.time(h, 0)))
        .collect();
    let add = Action::new(move |(): &()| {
        let filter = match subject.get_untracked().split_once(':') {
            Some(("list", id)) => json!({"lists": [id]}),
            Some(("person", id)) => json!({"companions": [id]}),
            Some(("item", id)) => json!({"items": [id]}),
            _ => serde_json::Value::Null,
        };
        let target = target.get_untracked();
        let body = json!({
            "event": event.get_untracked(),
            "filter": filter,
            "targets": if target.is_empty() { json!({"type": "all"}) } else { json!({"type": "only", "ids": [target]}) },
            "mode": if mode.get_untracked() == "digest" {
                json!({"mode": "digest", "cron": format!("0 {} * * *", hour.get_untracked())})
            } else {
                json!({"mode": "immediate"})
            },
            "timezone": zone(),
        });
        async move {
            if body["filter"].is_null() {
                error.set(Some(t("notify-filter-required")));
                return;
            }
            match rest::send("PUT", "/notifications/subscriptions", Some(body)).await {
                Ok(()) => {
                    error.set(None);
                    // the form stays as it is; only its subject is asked for again
                    subject.set(String::new());
                    state.refresh();
                }
                Err(e) => error.set(Some(notify_error(&api::error_code(&e)))),
            }
        }
    });
    view! {
        <details class="rounded-md border border-border p-3">
            <summary class="cursor-pointer text-sm font-medium">{t("notify-add-filtered")}</summary>
            <form class="mt-3 grid max-w-md gap-3" on:submit=move |ev| { ev.prevent_default(); add.dispatch(()); }>
                <FormField id="nf-r-event" label=t("notify-event")>
                    <Select id="nf-r-event" value=event options=Signal::derive(move || event_options.clone())/>
                </FormField>
                <FormField id="nf-r-for" label=t("notify-filter-for") description=t("notify-filter-items-later")>
                    <Select id="nf-r-for" value=subject options=subjects/>
                </FormField>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm class="w-fit" on:click=move |_| pick_item.run(())>
                    <Icon name=IconName::Search/> {t("notify-filter-pick-item")}
                </Button>
                <FormField id="nf-r-target" label=t("notify-channel")>
                    <Select id="nf-r-target" value=target options=targets/>
                </FormField>
                <FormField id="nf-r-mode" label=t("notify-mode")>
                    <Select id="nf-r-mode" value=mode options=Signal::derive(move || modes.clone())/>
                </FormField>
                {move || (mode.get() == "digest").then(|| {
                    let hours = hours.clone();
                    view! {
                        <FormField id="nf-r-hour" label=t("notify-digest-at")>
                            <Select id="nf-r-hour" value=hour options=Signal::derive(move || hours.clone())/>
                        </FormField>
                    }
                })}
                {move || error.get().map(|e| view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{e}</Alert> })}
                <Button r#type="submit" variant=ButtonVariant::Outline class="w-fit" loading=add.pending()>{t("notify-add-rule")}</Button>
            </form>
        </details>
    }
}
