//! Settings > Notifications (design.md 6.8, 5.8; mockup `notifications.html`): four tabs.
//! Inbox (grouped by the day in the reader's time zone, "Mark all read"), Channels (cards
//! with health from the delivery log, "Add and send a test"), Rules (events x channels, see
//! [`crate::notify_rules`]) and Log (deliveries with the error to unfold). Reads come from
//! the page's server function; changes go to the REST routes (`web::rest`, DEC-327).

use std::collections::HashMap;

use contracts::{
    ChannelDto, ChannelHealthDto, DeliveryDto, InboxItemDto, NotificationPageDto, SubscriptionDto,
};
use leptos::prelude::*;
use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, IconName, Table, TableBody,
    TableCell, TableHead, TableHeader, TableRow, Tabs, TabsContent, TabsList, TabsTrigger,
};

use crate::api;
use crate::history_state::{Day, DayGroup, DayLabel, day_heading};
use crate::i18n::{t, use_i18n};
use crate::pages::PageTitle;
use crate::rest;
use crate::settings::{Lead, loaded};

pub(crate) use crate::notify_channels::Channels;
pub(crate) use crate::notify_rules_view::Rules;

/// The text for an error code of these pages: `notify-error-<code>`, else the generic one.
pub(crate) fn notify_error(code: &str) -> String {
    let key = format!("notify-error-{code}");
    let text = t(&key);
    if text == key {
        t("error-generic")
    } else {
        text
    }
}

/// The hour of a daily digest schedule as the form writes it (`0 <hour> * * *`).
fn digest_hour(cron: &str) -> Option<u32> {
    let mut parts = cron.split_whitespace();
    let (min, hour) = (parts.next()?, parts.next()?);
    let rest: Vec<_> = parts.collect();
    (min == "0" && rest == ["*", "*", "*"])
        .then(|| hour.parse().ok())
        .flatten()
        .filter(|h| *h < 24)
}

pub(crate) fn mode_label(s: &SubscriptionDto) -> String {
    match s.mode["mode"].as_str() {
        Some("digest") => {
            let cron = s.mode["cron"].as_str().unwrap_or_default();
            let i18n = use_i18n();
            // other schedules (set through the API) show as written
            let time = digest_hour(cron).map_or_else(|| cron.to_string(), |h| i18n.time(h, 0));
            i18n.t_args("notify-mode-digest-at", &[("time", time.into())])
        }
        _ => t("notify-mode-immediate"),
    }
}

/// `22:00:00` -> the locale's short time.
fn clock(i18n: &crate::i18n::I18n, s: &str) -> Option<String> {
    let mut parts = s.split(':');
    let h = parts.next()?.parse().ok()?;
    let m = parts.next()?.parse().ok()?;
    Some(i18n.time(h, m))
}

/// The delivery of a rule in words: the mode, and the quiet hours if it has some.
pub(crate) fn when_label(s: &SubscriptionDto) -> String {
    let mode = mode_label(s);
    let i18n = use_i18n();
    let quiet = s.quiet_hours.as_ref().and_then(|q| {
        Some((
            clock(&i18n, q["start"].as_str()?)?,
            clock(&i18n, q["end"].as_str()?)?,
        ))
    });
    match quiet {
        Some((from, to)) => i18n.t_args(
            "notify-when-quiet",
            &[
                ("mode", mode.into()),
                ("from", from.into()),
                ("to", to.into()),
            ],
        ),
        None => mode,
    }
}

/// Name of an event or event pattern of a subscription (`*`, `release.*`, `release.episode`);
/// events without a label show their id.
pub(crate) fn event_label(id: &str) -> String {
    if id == "*" {
        return t("notify-all-events");
    }
    let key = match id.strip_suffix(".*") {
        Some(group) => format!("notify-group-{group}"),
        None => format!("notify-event-{}", id.replace('.', "-")),
    };
    let text = t(&key);
    if text == key { id.to_string() } else { text }
}

/// Which tab opens first: the inbox when something is unread, else the channels.
fn first_tab(p: &NotificationPageDto) -> String {
    if p.inbox.iter().any(|i| !i.read) {
        "inbox".into()
    } else {
        "channels".into()
    }
}

/// The channels without what a delivery changes on them (the health) and without a secret
/// that is shown once: what the rules are built from, so that a finished delivery does not
/// build them anew.
pub(crate) fn frames(channels: &[ChannelDto]) -> Vec<ChannelDto> {
    channels
        .iter()
        .map(|c| ChannelDto {
            secret: None,
            health: ChannelHealthDto::default(),
            ..c.clone()
        })
        .collect()
}

/// What a later read changed in the health of the channels that are shown: `(name, failing)`
/// for each channel whose last delivery now failed or now got through. It is said in the
/// page's live region, because the card changes on its own (WCAG 4.1.3). A channel that is
/// new, or that only waits, says nothing.
pub(crate) fn health_news(shown: &[ChannelDto], fresh: &[ChannelDto]) -> Vec<(String, bool)> {
    fresh
        .iter()
        .filter_map(|c| {
            let before = shown.iter().find(|s| s.id == c.id)?;
            let failing = match c.health.state.as_str() {
                "failing" => true,
                "working" => false,
                _ => return None,
            };
            (before.health.state != c.health.state).then(|| (c.name.clone(), failing))
        })
        .collect()
}

/// The state of the page. The page is built once; every later read (after a change made on
/// it, for a new inbox row, for a finished delivery, when the event stream has opened) is
/// shown in place through [`Self::page`], so a form someone is filling in, an unfolded error
/// and the focus stay where they are.
#[derive(Clone, Copy)]
pub(crate) struct NotifyState {
    /// What the page shows: the read that began last among those that were answered.
    pub page: RwSignal<NotificationPageDto>,
    /// The last "Send test" result per channel id: `Err` carries the channel's answer.
    pub tests: RwSignal<HashMap<String, Result<(), String>>>,
    /// A generated webhook signing secret, shown once on its card: `(channel id, secret)`.
    pub secret: RwSignal<Option<(String, String)>>,
    pub tab: RwSignal<String>,
    /// The sentence of the live region: what a later read changed in a channel's health.
    pub news: RwSignal<String>,
    /// Reads are numbered as they begin; `shown` is the number of the one on the page. Of
    /// two answers the one whose read began later is shown.
    reads: StoredValue<u64>,
    shown: StoredValue<Option<u64>>,
    /// How often the event stream had opened when the latest read began (0 for the data
    /// that came with the page from the server).
    read_at: StoredValue<u32>,
    opens: Option<crate::events::StreamOpens>,
    i18n: StoredValue<std::sync::Arc<crate::i18n::I18n>>,
}

impl NotifyState {
    fn new() -> Self {
        Self {
            page: RwSignal::new(NotificationPageDto {
                inbox: Vec::new(),
                channels: Vec::new(),
                subscriptions: Vec::new(),
                events: Vec::new(),
                deliveries: Vec::new(),
            }),
            tests: RwSignal::new(HashMap::new()),
            secret: RwSignal::new(None),
            tab: RwSignal::new(String::new()),
            news: RwSignal::new(String::new()),
            reads: StoredValue::new(0),
            shown: StoredValue::new(None),
            read_at: StoredValue::new(0),
            opens: use_context(),
            i18n: StoredValue::new(use_i18n()),
        }
    }

    /// A read begins: its number (`None`: the page is gone).
    fn begin(self) -> Option<u64> {
        let n = self.reads.try_update_value(|n| {
            *n += 1;
            *n
        })?;
        let opens = self
            .opens
            .and_then(|o| o.0.try_get_untracked())
            .unwrap_or(0);
        self.read_at.try_update_value(|at| *at = opens);
        Some(n)
    }

    /// Read `n` was answered with `fresh`: show it, unless a read that began later is shown
    /// already. The first answer also chooses the tab; a later one says what it changed in
    /// the health of a channel.
    fn show(self, n: u64, fresh: NotificationPageDto) {
        let Some(shown) = self.shown.try_get_value() else {
            return;
        };
        if shown.is_some_and(|s| s >= n) {
            return;
        }
        self.shown.set_value(Some(n));
        if shown.is_some() {
            let news = self
                .page
                .with_untracked(|p| health_news(&p.channels, &fresh.channels));
            if !news.is_empty() {
                let said = self.i18n.with_value(|i| {
                    let say = |(name, failing): &(String, bool)| {
                        let key = if *failing {
                            "notify-live-failing"
                        } else {
                            "notify-live-working"
                        };
                        i.t_args(key, &[("channel", name.clone().into())])
                    };
                    news.iter().map(say).collect::<Vec<_>>().join(" ")
                });
                self.news.set(said);
            }
        }
        if self.tab.with_untracked(String::is_empty) {
            self.tab.set(first_tab(&fresh));
        }
        if self.page.with_untracked(|p| *p != fresh) {
            self.page.set(fresh);
        }
    }

    /// Read the page again and show the answer in place. An answer that fails changes
    /// nothing: the next change or announcement reads again.
    pub fn refresh(self) {
        leptos::task::spawn_local(async move {
            let Some(n) = self.begin() else {
                return;
            };
            if let Ok(fresh) = api::notifications().await {
                self.show(n, fresh);
            }
        });
    }
}

#[component]
pub fn Notifications() -> impl IntoView {
    let state = NotifyState::new();
    // the number of the read the page's resource began (0: its data came with the page)
    let page_read = StoredValue::new(0_u64);
    let data = Resource::new(
        || (),
        move |()| {
            page_read.set_value(state.begin().unwrap_or(0));
            api::notifications()
        },
    );
    // An inbox row and the outcome of a delivery are written by the server on its own time
    // (the outcome by the worker, after the inbox row was announced), and what is written
    // between the read of a freshly loaded page and the opening of its event stream is
    // announced to nobody. All of it is read again here, paced, and shown in place.
    crate::events::refresh_on_topics(
        &["notification.new", "notification.delivery"],
        state.read_at,
        Callback::new(move |()| state.refresh()),
    );
    view! {
        <PageTitle text=t("notify-title")/>
        <Lead text=t("notify-lead")/>
        // says what changed on its own while the page is open (WCAG 4.1.3); not an alert,
        // and not the card's own text, which is also there on a normal page load
        <p role="status" class="sr-only" data-testid="nf-live">{move || state.news.get()}</p>
        {loaded(data, move |p| {
            state.show(page_read.get_value(), p);
            view! { <Page state=state/> }
        })}
    }
}

#[component]
fn Page(state: NotifyState) -> impl IntoView {
    let unread = Memo::new(move |_| {
        state
            .page
            .with(|p| p.inbox.iter().filter(|i| !i.read).count())
    });
    let i18n = use_i18n();
    view! {
        <Tabs id="nf" active=state.tab>
            <TabsList label=t("notify-tabs") class="max-w-full justify-start overflow-x-auto">
                <TabsTrigger value="inbox">
                    {t("notify-inbox")}
                    {move || {
                        let n = unread.get();
                        (n > 0).then(|| {
                            let text = i18n.t_args("shell-unread", &[("count", n.into())]);
                            view! {
                                <span class="ml-1.5 rounded-full bg-muted px-1.5 text-xs" aria-hidden="true">{n}</span>
                                <span class="sr-only">{format!(", {text}")}</span>
                            }
                        })
                    }}
                </TabsTrigger>
                <TabsTrigger value="channels">{t("notify-channels")}</TabsTrigger>
                <TabsTrigger value="rules">{t("notify-rules")}</TabsTrigger>
                <TabsTrigger value="log">{t("notify-log")}</TabsTrigger>
            </TabsList>
            <TabsContent value="inbox"><Inbox state=state/></TabsContent>
            <TabsContent value="channels"><Channels state=state/></TabsContent>
            <TabsContent value="rules"><Rules state=state/></TabsContent>
            <TabsContent value="log"><Log state=state/></TabsContent>
        </Tabs>
    }
}

/// The inbox rows grouped by their local day (newest first), with the heading of each day.
fn by_day(rows: Vec<InboxItemDto>, today: Day) -> Vec<(Day, DayLabel, Vec<InboxItemDto>)> {
    let mut groups: Vec<(Day, DayLabel, Vec<InboxItemDto>)> = Vec::new();
    for r in rows {
        let Some(day) = Day::parse(&r.local_day).or_else(|| Day::parse(&r.created_at)) else {
            continue;
        };
        match groups.last_mut() {
            Some((d, _, rows)) if *d == day => rows.push(r),
            _ => {
                let label = match today.days() - day.days() {
                    0 => DayLabel::Today,
                    1 => DayLabel::Yesterday,
                    _ => DayLabel::Date {
                        same_year: day.year == today.year,
                    },
                };
                groups.push((day, label, vec![r]));
            }
        }
    }
    groups
}

type Days = Vec<(Day, DayLabel, Vec<InboxItemDto>)>;

#[component]
fn Inbox(state: NotifyState) -> impl IntoView {
    let failed = RwSignal::new(false);
    let all_read = Action::new(move |(): &()| async move {
        let r = rest::send("POST", "/notifications/inbox/read-all", None).await;
        failed.set(r.is_err());
        state.refresh();
    });
    let read = Action::new(move |id: &String| {
        let path = format!("/notifications/inbox/{id}/read");
        async move {
            let _ = rest::send("POST", &path, None).await;
            state.refresh();
        }
    });
    let days: Memo<Days> = Memo::new(move |_| {
        state
            .page
            .with(|p| by_day(p.inbox.clone(), crate::history::today()))
    });
    let none = Memo::new(move |_| days.with(Vec::is_empty));
    let unread = Memo::new(move |_| state.page.with(|p| p.inbox.iter().any(|i| !i.read)));
    view! {
        {move || if none.get() {
            view! {
                <EmptyState title=t("notify-inbox-empty") icon=IconName::Inbox level=2
                    body=t("notify-inbox-empty-body")
                    primary=move || view! {
                        <Button variant=ButtonVariant::Outline on:click=move |_| state.tab.set("channels".into())>
                            {t("notify-add-channel")}
                        </Button>
                    }/>
            }
            .into_any()
        } else {
            view! {
                <div class="mt-4 grid max-w-2xl gap-6 pb-[var(--tab-bar-height)]">
                    {move || unread.get().then(|| view! {
                        <div class="flex flex-wrap items-center justify-end gap-2">
                            {move || failed.get().then(|| view! { <p role="alert" class="text-sm text-destructive-text">{t("error-generic")}</p> })}
                            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm loading=all_read.pending()
                                on:click=move |_| { all_read.dispatch(()); }>{t("notify-mark-all-read")}</Button>
                        </div>
                    })}
                    <InboxDays days=days read=read/>
                </div>
            }
            .into_any()
        }}
    }
}

/// The days of the inbox and their rows, each keyed: a row that arrives or is marked read
/// changes that row only, the others keep their nodes (and the focus on one of them).
#[component]
fn InboxDays(days: Memo<Days>, read: Action<String, ()>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <For each=move || days.get()
            key=|(day, label, _)| format!("{}{label:?}", day.iso())
            children=move |(day, label, _)| {
                let heading = day_heading(&i18n, &DayGroup { day, label, rows: Vec::new() });
                let id = format!("nf-day-{}", day.iso());
                let rows = Memo::new(move |_| {
                    days.with(|all| {
                        all.iter()
                            .find(|(d, _, _)| *d == day)
                            .map(|(_, _, rows)| rows.clone())
                            .unwrap_or_default()
                    })
                });
                view! {
                    <section aria-labelledby=id.clone()>
                        <h2 id=id.clone() class="mb-2 text-sm font-medium text-muted-foreground">{heading}</h2>
                        <ul class="grid gap-1" aria-labelledby=id.clone()>
                            <For each=move || rows.get() key=|i| (i.id.clone(), i.read)
                                children=move |i| view! { <InboxRow i=i read=read/> }/>
                        </ul>
                    </section>
                }
            }/>
    }
}

#[component]
fn InboxRow(i: InboxItemDto, read: Action<String, ()>) -> impl IntoView {
    let i18n = use_i18n();
    let time = clock(&i18n, &i.local_time).unwrap_or_default();
    let id = i.id.clone();
    let mark = i18n.t_args(
        "notify-mark-read-named",
        &[("title", i.title.clone().into())],
    );
    let title = match i.link.clone() {
        Some(l) => {
            view! { <a class="underline-offset-2 hover:underline" href=l>{i.title.clone()}</a> }
                .into_any()
        }
        None => view! { <span>{i.title.clone()}</span> }.into_any(),
    };
    view! {
        <li class="flex items-start gap-3 rounded-md border border-border px-3 py-2" data-read=if i.read { "true" } else { "false" }>
            <span aria-hidden="true" class=if i.read { "mt-2 size-2 shrink-0 rounded-full" } else { "mt-2 size-2 shrink-0 rounded-full bg-primary" }></span>
            <div class="min-w-0 flex-1">
                <p class=if i.read { "text-muted-foreground" } else { "font-semibold" }>
                    {(!i.read).then(|| view! { <span class="sr-only">{t("notify-unread")}" "</span> })}
                    {title}
                </p>
                <p class="text-sm">{i.body.clone()}</p>
                <p class="text-xs text-muted-foreground">{time}</p>
            </div>
            {(!i.read).then(|| view! {
                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost attr:aria-label=mark
                    on:click=move |_| { read.dispatch(id.clone()); }>{t("notify-mark-read")}</Button>
            })}
        </li>
    }
}

#[component]
fn Log(state: NotifyState) -> impl IntoView {
    // the rows follow every later read in place, keyed by the delivery: a row keeps its
    // nodes while its state changes, so an unfolded error stays unfolded and the focus
    // stays on it (SEC0-5)
    let rows = Memo::new(move |_| state.page.with(|p| p.deliveries.clone()));
    let none = Memo::new(move |_| rows.with(Vec::is_empty));
    view! {
        <Show when=move || !none.get()
            fallback=|| view! { <p class="mt-4 text-sm text-muted-foreground">{t("notify-log-empty")}</p> }>
            <div class="mt-4 pb-[var(--tab-bar-height)]">
                <Table stacked=true attr:data-testid="nf-log">
                    <caption class="sr-only">{t("notify-log")}</caption>
                    <TableHeader>
                        <TableRow>
                            <TableHead>{t("notify-log-when")}</TableHead>
                            <TableHead>{t("notify-event")}</TableHead>
                            <TableHead>{t("notify-channel")}</TableHead>
                            <TableHead>{t("notify-log-state")}</TableHead>
                        </TableRow>
                    </TableHeader>
                    <TableBody>
                        <For each=move || rows.get() key=|d| d.id.clone()
                            children=move |d| view! { <LogRow first=d rows=rows state=state/> }/>
                    </TableBody>
                </Table>
            </div>
        </Show>
    }
}

/// One delivery. `first` is the row as it was when it joined the log; what an attempt
/// changes (its state, the error) is read from `rows` and changes inside the same nodes.
#[component]
fn LogRow(first: DeliveryDto, rows: Memo<Vec<DeliveryDto>>, state: NotifyState) -> impl IntoView {
    let i18n = use_i18n();
    let when = match (
        Day::parse(&first.local_day),
        clock(&i18n, &first.local_time),
    ) {
        (Some(day), Some(time)) => format!("{} {time}", i18n.date(&day.iso())),
        _ => i18n.datetime(&first.created_at),
    };
    let (id, event, title) = (first.id.clone(), first.event.clone(), first.title.clone());
    let channel_id = first.channel_id.clone();
    // the name of its channel if that still exists
    let channel = Memo::new(move |_| {
        state.page.with(|p| {
            p.channels
                .iter()
                .find(|c| c.id == channel_id)
                .map(|c| c.name.clone())
        })
    });
    let now = Memo::new(move |_| {
        rows.with(|all| all.iter().find(|d| d.id == first.id).cloned())
            .unwrap_or_else(|| first.clone())
    });
    let outcome = Memo::new(move |_| now.with(|d| d.state.clone()));
    let error = Memo::new(move |_| now.with(|d| d.last_error.clone()));
    view! {
        // the id names the row for a reader that looks at it twice: the log follows
        // deliveries that finish while it is shown, so a row's place in it is not fixed
        <TableRow attr:data-delivery=id>
            <TableCell label=t("notify-log-when") class="whitespace-nowrap max-[40rem]:whitespace-normal">{when}</TableCell>
            <TableCell label=t("notify-event")>
                <span class="block">{event_label(&event)}</span>
                <span class="block text-xs text-muted-foreground">{title}</span>
            </TableCell>
            <TableCell label=t("notify-channel")>
                {move || channel.get().unwrap_or_else(|| t("notify-channel-gone"))}
            </TableCell>
            <TableCell label=t("notify-log-state")>
                {move || {
                    let state = outcome.get();
                    let variant = match state.as_str() {
                        "failed" => BadgeVariant::Destructive,
                        "sent" => BadgeVariant::Success,
                        _ => BadgeVariant::Secondary,
                    };
                    view! { <Badge variant=variant>{t(&format!("notify-state-{state}"))}</Badge> }
                }}
                // what the channel answered, as it said it (not translated)
                <Show when=move || error.with(Option::is_some)>
                    <details class="mt-1 text-xs">
                        <summary class="cursor-pointer">{t("notify-log-show-error")}</summary>
                        <p lang="en" class="mt-1 max-w-sm break-words text-destructive-text">
                            {move || error.get().unwrap_or_default()}
                        </p>
                    </details>
                </Show>
            </TableCell>
        </TableRow>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(day: &str, at: &str) -> InboxItemDto {
        InboxItemDto {
            id: at.into(),
            event: "admin.signup".into(),
            title: String::new(),
            body: String::new(),
            link: None,
            created_at: at.into(),
            local_day: day.into(),
            local_time: "23:30".into(),
            read: false,
        }
    }

    #[test]
    fn the_inbox_groups_by_the_local_day() {
        let today = Day::parse("2026-11-07").unwrap();
        // UTC says the 8th, the reader's zone the 7th: grouped by the zone
        let groups = by_day(
            vec![
                row("2026-11-07", "2026-11-08T01:00:00Z"),
                row("2026-11-07", "2026-11-07T09:00:00Z"),
                row("2026-11-06", "2026-11-06T09:00:00Z"),
                row("", "2025-12-24T09:00:00Z"),
            ],
            today,
        );
        let shape: Vec<_> = groups.iter().map(|(_, l, r)| (*l, r.len())).collect();
        assert_eq!(
            shape,
            vec![
                (DayLabel::Today, 2),
                (DayLabel::Yesterday, 1),
                (DayLabel::Date { same_year: false }, 1),
            ]
        );
    }

    fn channel(id: &str, state: &str) -> ChannelDto {
        ChannelDto {
            id: id.into(),
            name: format!("Channel {id}"),
            kind: "webhook".into(),
            enabled: true,
            config: serde_json::json!({"url": "https://hooks.example.org/in"}),
            secret: Some("shown once".into()),
            health: ChannelHealthDto {
                state: state.into(),
                at: None,
                error: (state == "failing").then(|| "refused".to_string()),
            },
        }
    }

    /// SEC0-2: a later read says which channel's last delivery now failed or now got
    /// through; a state that did not change, a channel that only waits and a channel that
    /// is new say nothing.
    #[test]
    fn a_later_read_says_whose_health_changed() {
        let shown = [
            channel("a", "never"),
            channel("b", "failing"),
            channel("c", "working"),
            channel("d", "never"),
        ];
        let fresh = [
            channel("a", "failing"),
            channel("b", "working"),
            channel("c", "working"),
            channel("d", "waiting"),
            channel("e", "failing"),
        ];
        assert_eq!(
            health_news(&shown, &fresh),
            vec![
                ("Channel a".to_string(), true),
                ("Channel b".to_string(), false)
            ]
        );
        assert_eq!(health_news(&shown, &shown).len(), 0);
    }

    /// SEC0-3: the rules are built from the channels without their health, so a delivery
    /// that finishes does not build the matrix anew under someone using it.
    #[test]
    fn the_rules_do_not_see_what_a_delivery_changes() {
        let before = [channel("a", "never")];
        let after = [channel("a", "failing")];
        assert_eq!(frames(&before), frames(&after));
        assert_eq!(frames(&after)[0].secret, None);
        let mut renamed = channel("a", "never");
        renamed.name = "Phone".into();
        assert_ne!(frames(&before), frames(&[renamed]));
    }

    #[test]
    fn digest_hours_are_read_from_the_form_schedule_only() {
        assert_eq!(digest_hour("0 8 * * *"), Some(8));
        assert_eq!(digest_hour("30 8 * * *"), None);
        assert_eq!(digest_hour("0 25 * * *"), None);
    }
}
