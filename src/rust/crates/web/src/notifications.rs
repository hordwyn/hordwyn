//! Settings > Notifications (design.md 6.8, 5.8; mockup `notifications.html`): four tabs.
//! Inbox (grouped by the day in the reader's time zone, "Mark all read"), Channels (cards
//! with health from the delivery log, "Add and send a test"), Rules (events x channels, see
//! [`crate::notify_rules`]) and Log (deliveries with the error to unfold). Reads come from
//! the page's server function; changes go to the REST routes (`web::rest`, DEC-327).

use std::collections::HashMap;

use contracts::{InboxItemDto, NotificationPageDto, SubscriptionDto};
use leptos::prelude::*;
use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, IconName, Table, TableBody,
    TableCell, TableHead, TableHeader, TableRow, Tabs, TabsContent, TabsList, TabsTrigger,
};

use crate::api;
use crate::events::use_events;
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

/// Page-level state that outlives a refetch.
#[derive(Clone, Copy)]
pub(crate) struct NotifyState {
    pub data: Resource<Result<NotificationPageDto, ServerFnError>>,
    /// The last "Send test" result per channel id: `Err` carries the channel's answer.
    pub tests: RwSignal<HashMap<String, Result<(), String>>>,
    /// A generated webhook signing secret, shown once on its card: `(channel id, secret)`.
    pub secret: RwSignal<Option<(String, String)>>,
    pub tab: RwSignal<String>,
}

#[component]
pub fn Notifications() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::notifications());
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && topic == "notification.new"
        {
            data.refetch();
        }
    });
    let state = NotifyState {
        data,
        tests: RwSignal::new(HashMap::new()),
        secret: RwSignal::new(None),
        tab: RwSignal::new(String::new()),
    };
    view! {
        <PageTitle text=t("notify-title")/>
        <Lead text=t("notify-lead")/>
        {loaded(data, move |p| view! { <Page p=p state=state/> })}
    }
}

#[component]
fn Page(p: NotificationPageDto, state: NotifyState) -> impl IntoView {
    // the tab is page state: a refetch (a new inbox row, a saved rule) keeps it
    if state.tab.get_untracked().is_empty() {
        state.tab.set(first_tab(&p));
    }
    let unread = p.inbox.iter().filter(|i| !i.read).count();
    let i18n = use_i18n();
    let unread_text = i18n.t_args("shell-unread", &[("count", unread.into())]);
    let (p1, p2, p3, p4) = (p.clone(), p.clone(), p.clone(), p);
    view! {
        <Tabs id="nf" active=state.tab>
            <TabsList label=t("notify-tabs") class="max-w-full justify-start overflow-x-auto">
                <TabsTrigger value="inbox">
                    {t("notify-inbox")}
                    {(unread > 0).then(|| view! {
                        <span class="ml-1.5 rounded-full bg-muted px-1.5 text-xs" aria-hidden="true">{unread}</span>
                        <span class="sr-only">{format!(", {unread_text}")}</span>
                    })}
                </TabsTrigger>
                <TabsTrigger value="channels">{t("notify-channels")}</TabsTrigger>
                <TabsTrigger value="rules">{t("notify-rules")}</TabsTrigger>
                <TabsTrigger value="log">{t("notify-log")}</TabsTrigger>
            </TabsList>
            <TabsContent value="inbox"><Inbox p=p1.clone() state=state/></TabsContent>
            <TabsContent value="channels"><Channels p=p2.clone() state=state/></TabsContent>
            <TabsContent value="rules"><Rules p=p3.clone() state=state/></TabsContent>
            <TabsContent value="log"><Log p=p4.clone()/></TabsContent>
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

#[component]
fn Inbox(p: NotificationPageDto, state: NotifyState) -> impl IntoView {
    let unread = p.inbox.iter().any(|i| !i.read);
    let failed = RwSignal::new(false);
    let all_read = Action::new(move |(): &()| async move {
        let r = rest::send("POST", "/notifications/inbox/read-all", None).await;
        failed.set(r.is_err());
        state.data.refetch();
    });
    let read = Action::new(move |id: &String| {
        let path = format!("/notifications/inbox/{id}/read");
        async move {
            let _ = rest::send("POST", &path, None).await;
            state.data.refetch();
        }
    });
    if p.inbox.is_empty() {
        return view! {
            <EmptyState title=t("notify-inbox-empty") icon=IconName::Inbox level=2
                body=t("notify-inbox-empty-body")
                primary=move || view! {
                    <Button variant=ButtonVariant::Outline on:click=move |_| state.tab.set("channels".into())>
                        {t("notify-add-channel")}
                    </Button>
                }/>
        }
        .into_any();
    }
    let i18n = use_i18n();
    let groups = by_day(p.inbox, crate::history::today());
    view! {
        <div class="mt-4 grid max-w-2xl gap-6 pb-[var(--tab-bar-height)]">
            {unread.then(|| view! {
                <div class="flex flex-wrap items-center justify-end gap-2">
                    {move || failed.get().then(|| view! { <p role="alert" class="text-sm text-destructive-text">{t("error-generic")}</p> })}
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm loading=all_read.pending()
                        on:click=move |_| { all_read.dispatch(()); }>{t("notify-mark-all-read")}</Button>
                </div>
            })}
            {groups.into_iter().map(|(day, label, rows)| {
                let heading = day_heading(&i18n, &DayGroup { day, label, rows: Vec::new() });
                let id = format!("nf-day-{}", day.iso());
                view! {
                    <section aria-labelledby=id.clone()>
                        <h2 id=id.clone() class="mb-2 text-sm font-medium text-muted-foreground">{heading}</h2>
                        <ul class="grid gap-1" aria-labelledby=id.clone()>
                            {rows.into_iter().map(|i| view! { <InboxRow i=i read=read/> }).collect_view()}
                        </ul>
                    </section>
                }
            }).collect_view()}
        </div>
    }
    .into_any()
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
fn Log(p: NotificationPageDto) -> impl IntoView {
    if p.deliveries.is_empty() {
        return view! { <p class="mt-4 text-sm text-muted-foreground">{t("notify-log-empty")}</p> }
            .into_any();
    }
    let i18n = use_i18n();
    let channels = p.channels;
    view! {
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
                    {p.deliveries.into_iter().map(|d| {
                        let when = match (Day::parse(&d.local_day), clock(&i18n, &d.local_time)) {
                            (Some(day), Some(time)) => format!("{} {time}", i18n.date(&day.iso())),
                            _ => i18n.datetime(&d.created_at),
                        };
                        let channel = channels.iter().find(|c| c.id == d.channel_id)
                            .map_or_else(|| t("notify-channel-gone"), |c| c.name.clone());
                        let variant = match d.state.as_str() {
                            "failed" => BadgeVariant::Destructive,
                            "sent" => BadgeVariant::Success,
                            _ => BadgeVariant::Secondary,
                        };
                        view! {
                            <TableRow>
                                <TableCell label=t("notify-log-when") class="whitespace-nowrap max-[40rem]:whitespace-normal">{when}</TableCell>
                                <TableCell label=t("notify-event")>
                                    <span class="block">{event_label(&d.event)}</span>
                                    <span class="block text-xs text-muted-foreground">{d.title.clone()}</span>
                                </TableCell>
                                <TableCell label=t("notify-channel")>{channel}</TableCell>
                                <TableCell label=t("notify-log-state")>
                                    <Badge variant=variant>{t(&format!("notify-state-{}", d.state))}</Badge>
                                    // what the channel answered, as it said it (not translated)
                                    {d.last_error.clone().map(|e| view! {
                                        <details class="mt-1 text-xs">
                                            <summary class="cursor-pointer">{t("notify-log-show-error")}</summary>
                                            <p lang="en" class="mt-1 max-w-sm break-words text-destructive-text">{e}</p>
                                        </details>
                                    })}
                                </TableCell>
                            </TableRow>
                        }
                    }).collect_view()}
                </TableBody>
            </Table>
        </div>
    }
    .into_any()
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

    #[test]
    fn digest_hours_are_read_from_the_form_schedule_only() {
        assert_eq!(digest_hour("0 8 * * *"), Some(8));
        assert_eq!(digest_hour("30 8 * * *"), None);
        assert_eq!(digest_hour("0 25 * * *"), None);
    }
}
