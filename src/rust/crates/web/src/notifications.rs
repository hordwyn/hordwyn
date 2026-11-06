//! Notifications (D-17): the inbox, channels, subscriptions and the delivery log.

use contracts::{ChannelDto, NewChannelDto, NotificationPageDto, SubscriptionDto};
use leptos::prelude::*;
use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, FormField, Input, Select, SelectOption,
    Skeleton,
};

use crate::api;
use crate::events::use_events;
use crate::i18n::{t, use_i18n};
use crate::pages::{PageTitle, go, needs_sign_in};

fn notify_error(e: &ServerFnError) -> String {
    let key = format!("notify-error-{}", api::error_code(e));
    let text = t(&key);
    if text == key {
        t("error-generic")
    } else {
        text
    }
}

fn opt(value: &str, label: String) -> SelectOption {
    SelectOption {
        value: value.into(),
        label,
    }
}

/// Channel kinds the form offers (web push needs the browser and is set up there).
const KINDS: [&str; 7] = [
    "ntfy", "gotify", "discord", "matrix", "apprise", "webhook", "email",
];

/// The channel config from the three generic form fields.
fn config(kind: &str, a: &str, b: &str, c: &str) -> serde_json::Value {
    let o = |s: &str| (!s.trim().is_empty()).then(|| s.trim().to_string());
    match kind {
        "ntfy" => serde_json::json!({"kind": "ntfy", "server": a, "topic": b, "token": o(c)}),
        "gotify" => serde_json::json!({"kind": "gotify", "server": a, "token": c}),
        "discord" => serde_json::json!({"kind": "discord", "webhook_url": a}),
        "matrix" => serde_json::json!({
            "kind": "matrix", "homeserver": a, "room_id": b, "access_token": c,
        }),
        "apprise" => serde_json::json!({"kind": "apprise", "server": a, "urls": b}),
        "webhook" => serde_json::json!({"kind": "webhook", "url": a, "secret": c}),
        _ => serde_json::json!({"kind": "email", "to": a}),
    }
}

/// Which of the three fields a kind uses (label keys; `None` hides the field).
fn fields(kind: &str) -> [Option<&'static str>; 3] {
    match kind {
        "ntfy" => [
            Some("notify-f-server"),
            Some("notify-f-topic"),
            Some("notify-f-token-optional"),
        ],
        "gotify" => [Some("notify-f-server"), None, Some("notify-f-app-token")],
        "discord" => [Some("notify-f-webhook-url"), None, None],
        "matrix" => [
            Some("notify-f-homeserver"),
            Some("notify-f-room"),
            Some("notify-f-access-token"),
        ],
        "apprise" => [Some("notify-f-server"), Some("notify-f-apprise-urls"), None],
        "webhook" => [Some("notify-f-url"), None, Some("notify-f-secret-optional")],
        _ => [Some("notify-f-email"), None, None],
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

fn mode_label(s: &SubscriptionDto) -> String {
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

/// Name of an event or event pattern of a subscription (`*`, `release.*`, `release.episode`);
/// events without a label show their id.
fn event_label(id: &str) -> String {
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

fn targets_label(s: &SubscriptionDto, channels: &[ChannelDto]) -> String {
    match s.targets["type"].as_str() {
        Some("only") => s.targets["ids"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|id| channels.iter().find(|c| Some(c.id.as_str()) == id.as_str()))
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
            .join(", "),
        _ => t("notify-all-channels"),
    }
}

#[component]
pub fn Notifications() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::notifications());
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && (topic == "notification.new" || topic == "entity.updated")
        {
            data.refetch();
        }
    });
    let message = RwSignal::new(None::<String>);
    let report = move |r: Result<String, ServerFnError>| match r {
        Ok(m) => message.set(Some(m)),
        Err(e) => message.set(Some(notify_error(&e))),
    };

    let read = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            let _ = api::mark_read(id).await;
            data.refetch();
        }
    });

    // new channel
    let kind = RwSignal::new("ntfy".to_string());
    let name = RwSignal::new(String::new());
    let (f1, f2, f3) = (
        RwSignal::new(String::new()),
        RwSignal::new(String::new()),
        RwSignal::new(String::new()),
    );
    let add_channel = Action::new(move |(): &()| {
        let k = kind.get_untracked();
        let b = NewChannelDto {
            name: name.get_untracked(),
            config: config(
                &k,
                &f1.get_untracked(),
                &f2.get_untracked(),
                &f3.get_untracked(),
            ),
            enabled: true,
        };
        async move {
            let r = api::create_channel(b).await.map(|c| {
                for s in [name, f1, f2, f3] {
                    s.set(String::new());
                }
                match c.secret {
                    Some(secret) => use_i18n_args("notify-secret-created", "secret", secret),
                    None => t("notify-channel-added"),
                }
            });
            report(r);
            data.refetch();
        }
    });
    let test = Action::new(move |id: &String| {
        let id = id.clone();
        async move { report(api::test_channel(id).await.map(|()| t("notify-test-sent"))) }
    });
    let remove_channel = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            report(api::delete_channel(id).await.map(|()| t("notify-deleted")));
            data.refetch();
        }
    });

    // new subscription
    let event = RwSignal::new("*".to_string());
    let target = RwSignal::new(String::new());
    let mode = RwSignal::new("immediate".to_string());
    let digest_hour = RwSignal::new("8".to_string());
    let quiet_from = RwSignal::new(String::new());
    let quiet_to = RwSignal::new(String::new());
    let tz = RwSignal::new("UTC".to_string());
    let add_sub = Action::new(move |(): &()| {
        let quiet = match (quiet_from.get_untracked(), quiet_to.get_untracked()) {
            (a, b) if !a.is_empty() && !b.is_empty() => Some(serde_json::json!({
                "start": format!("{a}:00"), "end": format!("{b}:00"),
            })),
            _ => None,
        };
        let target = target.get_untracked();
        let b = SubscriptionDto {
            id: None,
            event: event.get_untracked(),
            filter: serde_json::Value::Null,
            targets: if target.is_empty() {
                serde_json::json!({"type": "all"})
            } else {
                serde_json::json!({"type": "only", "ids": [target]})
            },
            mode: if mode.get_untracked() == "digest" {
                serde_json::json!({"mode": "digest", "cron": format!("0 {} * * *", digest_hour.get_untracked())})
            } else {
                serde_json::json!({"mode": "immediate"})
            },
            quiet_hours: quiet,
            timezone: tz.get_untracked(),
            rate_limit_per_hour: None,
            enabled: true,
            locked: false,
            instance_default: false,
        };
        async move {
            report(
                api::put_subscription(b)
                    .await
                    .map(|_| t("notify-subscribed")),
            );
            data.refetch();
        }
    });
    let remove_sub = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            report(
                api::delete_subscription(id)
                    .await
                    .map(|()| t("notify-deleted")),
            );
            data.refetch();
        }
    });

    let page = Signal::derive(move || data.get().and_then(Result::ok));
    let event_options = Signal::derive(move || {
        let mut v = vec![opt("*", t("notify-all-events"))];
        if let Some(p) = page.get() {
            let mut groups: Vec<String> = p
                .events
                .iter()
                .filter_map(|e| e.id.split_once('.').map(|(g, _)| g.to_string()))
                .collect();
            groups.dedup();
            v.extend(
                groups
                    .iter()
                    .map(|g| opt(&format!("{g}.*"), t(&format!("notify-group-{g}")))),
            );
            v.extend(p.events.iter().map(|e| opt(&e.id, event_label(&e.id))));
        }
        v
    });
    let target_options = Signal::derive(move || {
        let mut v = vec![opt("", t("notify-all-channels"))];
        if let Some(p) = page.get() {
            v.extend(p.channels.iter().map(|c| opt(&c.id, c.name.clone())));
        }
        v
    });
    let kind_options = Signal::derive(|| {
        KINDS
            .iter()
            .map(|k| opt(k, t(&format!("notify-kind-{k}"))))
            .collect::<Vec<_>>()
    });
    let mode_options = Signal::derive(|| {
        vec![
            opt("immediate", t("notify-mode-immediate")),
            opt("digest", t("notify-mode-digest")),
        ]
    });
    let hour_options = Signal::derive(|| {
        let i18n = use_i18n();
        (0..24)
            .map(|h| opt(&h.to_string(), i18n.time(h, 0)))
            .collect::<Vec<_>>()
    });

    view! {
        <PageTitle text=t("notify-title")/>
        <p role="status" class="mb-4 text-sm">{move || message.get()}</p>
        <Transition fallback=|| view! { <Skeleton class="h-48 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{notify_error(&e)}</p> }.into_any(),
                Ok(p) => view! { <Inbox p=p.clone() read=read/> <Channels p=p.clone() test=test remove=remove_channel/> <Subscriptions p=p.clone() remove=remove_sub/> <Log p=p/> }.into_any(),
            })}
        </Transition>

        <section class="mt-8 max-w-md" aria-labelledby="new-channel">
            <h2 id="new-channel" class="mb-2 font-medium">{t("notify-new-channel")}</h2>
            <form class="grid gap-2" on:submit=move |ev| { ev.prevent_default(); add_channel.dispatch(()); }>
                <FormField id="channel-kind" label=t("notify-kind")>
                    <Select id="channel-kind" value=kind options=kind_options/>
                </FormField>
                <FormField id="channel-name" label=t("notify-channel-name")>
                    <Input id="channel-name" value=name/>
                </FormField>
                {move || {
                    let [a, b, c] = fields(&kind.get());
                    [(a, f1, "channel-f1"), (b, f2, "channel-f2"), (c, f3, "channel-f3")]
                        .into_iter()
                        .filter_map(|(label, sig, id)| label.map(|l| view! {
                            <FormField id=id label=t(l)><Input id=id value=sig/></FormField>
                        }))
                        .collect_view()
                }}
                <Button r#type="submit" variant=ButtonVariant::Outline>{t("notify-add-channel")}</Button>
            </form>
        </section>

        // the pickers read the page resource: inside a transition so hydration waits for it
        <Transition fallback=|| ()>
        <section class="mt-8 max-w-md" aria-labelledby="new-sub">
            <h2 id="new-sub" class="mb-2 font-medium">{t("notify-new-subscription")}</h2>
            <form class="grid gap-2" on:submit=move |ev| { ev.prevent_default(); add_sub.dispatch(()); }>
                <FormField id="sub-event" label=t("notify-event")>
                    <Select id="sub-event" value=event options=event_options/>
                </FormField>
                <FormField id="sub-target" label=t("notify-channel")>
                    <Select id="sub-target" value=target options=target_options/>
                </FormField>
                <FormField id="sub-mode" label=t("notify-mode")>
                    <Select id="sub-mode" value=mode options=mode_options/>
                </FormField>
                {move || (mode.get() == "digest").then(|| view! {
                    <FormField id="sub-hour" label=t("notify-digest-at")>
                        <Select id="sub-hour" value=digest_hour options=hour_options/>
                    </FormField>
                })}
                <div class="grid grid-cols-2 gap-2">
                    <FormField id="quiet-from" label=t("notify-quiet-from")>
                        <Input id="quiet-from" r#type="time" value=quiet_from/>
                    </FormField>
                    <FormField id="quiet-to" label=t("notify-quiet-to")>
                        <Input id="quiet-to" r#type="time" value=quiet_to/>
                    </FormField>
                </div>
                <FormField id="sub-tz" label=t("notify-timezone")>
                    <Input id="sub-tz" value=tz/>
                </FormField>
                <Button r#type="submit" variant=ButtonVariant::Outline>{t("notify-subscribe")}</Button>
            </form>
        </section>
        </Transition>
    }
}

fn use_i18n_args(key: &str, name: &str, value: String) -> String {
    crate::i18n::use_i18n().t_args(key, &[(name, value.into())])
}

#[component]
fn Inbox(p: NotificationPageDto, read: Action<String, ()>) -> impl IntoView {
    view! {
        <section class="mb-8" aria-labelledby="inbox-title">
            <h2 id="inbox-title" class="mb-2 font-medium">{t("notify-inbox")}</h2>
            {if p.inbox.is_empty() {
                view! { <p class="text-sm text-muted-foreground">{t("notify-inbox-empty")}</p> }.into_any()
            } else {
                view! {
                    <ul class="flex flex-col gap-2" aria-labelledby="inbox-title">
                        {p.inbox.into_iter().map(|i| {
                            let id = i.id.clone();
                            view! {
                                <li class=if i.read { "rounded-md border border-border px-3 py-2 text-muted-foreground" } else { "rounded-md border border-primary px-3 py-2" }>
                                    <p class="font-medium">
                                        {match i.link.clone() {
                                            Some(l) => view! { <a class="underline-offset-2 hover:underline" href=l>{i.title.clone()}</a> }.into_any(),
                                            None => view! { <span>{i.title.clone()}</span> }.into_any(),
                                        }}
                                    </p>
                                    <p class="text-sm">{i.body.clone()}</p>
                                    <p class="text-xs text-muted-foreground">{use_i18n().datetime(&i.created_at)}</p>
                                    {(!i.read).then(|| view! {
                                        <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost
                                            attr:aria-label=use_i18n().t_args("notify-mark-read-named", &[("title", i.title.clone().into())])
                                            on:click=move |_| { read.dispatch(id.clone()); }>{t("notify-mark-read")}</Button>
                                    })}
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}
        </section>
    }
}

#[component]
fn Channels(
    p: NotificationPageDto,
    test: Action<String, ()>,
    remove: Action<String, ()>,
) -> impl IntoView {
    view! {
        <section class="mb-8" aria-labelledby="channels-title">
            <h2 id="channels-title" class="mb-2 font-medium">{t("notify-channels")}</h2>
            <p class="mb-2 text-sm text-muted-foreground">{t("notify-channels-lead")}</p>
            <ul class="flex flex-col gap-2" aria-labelledby="channels-title">
                {p.channels.into_iter().map(|c| {
                    let (a, b) = (c.id.clone(), c.id.clone());
                    view! {
                        <li class="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2">
                            <span class="flex-1">{c.name.clone()} <span class="ml-2 text-sm text-muted-foreground">{t(&format!("notify-kind-{}", c.kind))}</span></span>
                            {(!c.enabled).then(|| view! { <Badge variant=BadgeVariant::Destructive>{t("notify-disabled")}</Badge> })}
                            <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=use_i18n().t_args("notify-test-named", &[("name", c.name.clone().into())])
                                on:click=move |_| { test.dispatch(a.clone()); }>{t("notify-test")}</Button>
                            <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost attr:aria-label=use_i18n().t_args("notify-delete-named", &[("name", c.name.clone().into())])
                                on:click=move |_| { remove.dispatch(b.clone()); }>{t("notify-delete")}</Button>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </section>
    }
}

#[component]
fn Subscriptions(p: NotificationPageDto, remove: Action<String, ()>) -> impl IntoView {
    let channels = p.channels.clone();
    view! {
        <section class="mb-8" aria-labelledby="subs-title">
            <h2 id="subs-title" class="mb-2 font-medium">{t("notify-subscriptions")}</h2>
            {p.subscriptions.is_empty().then(|| view! { <p class="text-sm text-muted-foreground">{t("notify-subscriptions-empty")}</p> })}
            <ul class="flex flex-col gap-2" aria-labelledby="subs-title">
                {p.subscriptions.into_iter().map(|s| {
                    let id = s.id.clone().unwrap_or_default();
                    let detail = format!("{} - {}", mode_label(&s), targets_label(&s, &channels));
                    let removable = !s.instance_default;
                    let event = event_label(&s.event);
                    let label = use_i18n().t_args("notify-delete-named", &[("name", event.clone().into())]);
                    view! {
                        <li class="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2">
                            <span class="flex-1">
                                <span class="text-sm font-medium">{event}</span>
                                <span class="ml-2 text-sm text-muted-foreground">{detail}</span>
                            </span>
                            {s.instance_default.then(|| view! { <Badge variant=BadgeVariant::Secondary>{t("notify-default")}</Badge> })}
                            {(!s.enabled).then(|| view! { <Badge variant=BadgeVariant::Outline>{t("notify-muted")}</Badge> })}
                            {removable.then(|| view! {
                                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost attr:aria-label=label
                                    on:click=move |_| { remove.dispatch(id.clone()); }>{t("notify-delete")}</Button>
                            })}
                        </li>
                    }
                }).collect_view()}
            </ul>
        </section>
    }
}

#[component]
fn Log(p: NotificationPageDto) -> impl IntoView {
    view! {
        <section aria-labelledby="log-title">
            <h2 id="log-title" class="mb-2 font-medium">{t("notify-log")}</h2>
            {p.deliveries.is_empty().then(|| view! { <p class="text-sm text-muted-foreground">{t("notify-log-empty")}</p> })}
            <ul class="flex flex-col gap-1 text-sm" aria-labelledby="log-title">
                {p.deliveries.into_iter().map(|d| view! {
                    <li class="flex flex-wrap gap-2">
                        <span class="text-muted-foreground">{use_i18n().datetime(&d.created_at)}</span>
                        <span class="flex-1">{d.title.clone()}</span>
                        <Badge variant=if d.state == "failed" { BadgeVariant::Destructive } else { BadgeVariant::Secondary }>{t(&format!("notify-state-{}", d.state))}</Badge>
                        // what the channel answered, as it said it (not translated)
                        {d.last_error.clone().map(|e| view! { <span lang="en" class="basis-full text-xs text-destructive">{e}</span> })}
                    </li>
                }).collect_view()}
            </ul>
        </section>
    }
}
