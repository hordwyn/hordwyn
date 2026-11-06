//! Sharing and companion flows (D-9): companions with link, invite, unlink and merge; link
//! requests addressed to the user; the privacy switch; sessions logged and shared with the
//! user ("copy to my log"); the public page of a link-shared list.

use contracts::{CompanionDto, LoggedSessionDto};
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, FormField, Input, Label, Select,
    SelectOption, Skeleton,
};

use crate::api;
use crate::events::use_events;
use crate::i18n::{t, use_i18n};
use crate::pages::{PageTitle, go, needs_sign_in};

/// Error text for sharing actions (`sharing-error-<code>`).
fn share_error(e: &ServerFnError) -> String {
    let key = format!("sharing-error-{}", api::error_code(e));
    let text = t(&key);
    if text == key {
        t("error-generic")
    } else {
        text
    }
}

/// Absolute URL of an app path, for links people copy and send.
pub(crate) fn absolute(path: &str) -> String {
    #[cfg(feature = "hydrate")]
    if let Some(origin) = web_sys::window().and_then(|w| w.location().origin().ok()) {
        return format!("{origin}{path}");
    }
    path.to_string()
}

/// Refetch on any `entity.updated` event (companions, sessions and links change elsewhere).
fn refetch_on_update(refetch: impl Fn() + Send + Sync + 'static) {
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && topic == "entity.updated"
        {
            refetch();
        }
    });
}

fn options(cs: &[CompanionDto], with_self: bool) -> Vec<SelectOption> {
    std::iter::once(SelectOption {
        value: String::new(),
        label: t("companions-choose"),
    })
    .chain(
        cs.iter()
            .filter(|c| c.merged_into.is_none() && (with_self || !c.is_self))
            .map(|c| SelectOption {
                value: c.id.clone(),
                label: c.name.clone(),
            }),
    )
    .collect()
}

#[component]
pub fn Companions() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::companions());
    let requests = Resource::new(|| (), |()| api::link_requests());
    refetch_on_update(move || data.refetch());
    refetch_on_update(move || requests.refetch());
    let message = RwSignal::new(None::<String>);
    let report = move |r: Result<(), ServerFnError>| match r {
        Ok(()) => message.set(None),
        Err(e) => message.set(Some(share_error(&e))),
    };

    let name = RwSignal::new(String::new());
    let add = Action::new(move |(): &()| {
        let n = name.get_untracked();
        async move {
            let r = api::create_companion(n)
                .await
                .map(|_| name.set(String::new()));
            report(r);
            data.refetch();
        }
    });
    // companion id -> invite URL, shown once
    let invite_url = RwSignal::new(None::<(String, String)>);
    let invite = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            match api::invite(id.clone()).await {
                Ok(i) => {
                    invite_url.set(Some((id, absolute(&i.path))));
                    message.set(None);
                }
                Err(e) => message.set(Some(share_error(&e))),
            }
        }
    });
    let unlink = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            report(api::unlink(id).await.map(|_| ()));
            data.refetch();
        }
    });
    let link_companion = RwSignal::new(String::new());
    let link_user = RwSignal::new(String::new());
    let request = Action::new(move |(): &()| {
        let (c, u) = (link_companion.get_untracked(), link_user.get_untracked());
        async move {
            match api::request_link(c, u).await {
                Ok(_) => {
                    link_user.set(String::new());
                    message.set(Some(t("companions-link-sent")));
                }
                Err(e) => message.set(Some(share_error(&e))),
            }
        }
    });
    let keep = RwSignal::new(String::new());
    let remove = RwSignal::new(String::new());
    let merge = Action::new(move |(): &()| {
        let (k, r) = (keep.get_untracked(), remove.get_untracked());
        async move {
            report(api::merge(k, r).await.map(|_| ()));
            remove.set(String::new());
            data.refetch();
        }
    });
    let answer = Action::new(move |(id, accept): &(String, bool)| {
        let (id, accept) = (id.clone(), *accept);
        async move {
            report(api::answer_link(id, accept).await);
            requests.refetch();
            data.refetch();
        }
    });
    let all = Signal::derive(move || data.get().and_then(Result::ok).unwrap_or_default());
    let pick_all = Signal::derive(move || options(&all.get(), false));

    view! {
        <PageTitle text=t("companions-title")/>
        <p class="mb-4 text-sm text-muted-foreground">{t("companions-lead")}</p>
        <p role="status" class="mb-4 text-sm">{move || message.get()}</p>

        <Suspense fallback=|| ()>
            {move || requests.get().and_then(Result::ok).filter(|r| !r.is_empty()).map(|list| view! {
                <section class="mb-6 rounded-md border border-border p-4" aria-labelledby="requests-title">
                    <h2 id="requests-title" class="mb-2 font-medium">{t("companions-requests")}</h2>
                    <ul class="flex flex-col gap-2">
                        {list.into_iter().map(|r| {
                            let text = use_i18n().t_args("companions-request-text", &[
                                ("from", r.from_name.clone().into()),
                                ("name", r.companion_name.clone().into()),
                            ]);
                            let (a, d) = (r.id.clone(), r.id.clone());
                            view! {
                                <li class="flex flex-wrap items-center gap-2">
                                    <span class="flex-1">{text}</span>
                                    <Button size=ButtonSize::Sm on:click=move |_| { answer.dispatch((a.clone(), true)); }>{t("companions-accept")}</Button>
                                    <Button size=ButtonSize::Sm variant=ButtonVariant::Outline on:click=move |_| { answer.dispatch((d.clone(), false)); }>{t("companions-decline")}</Button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                </section>
            })}
        </Suspense>

        <form class="mb-6 flex max-w-md items-end gap-2" on:submit=move |ev| { ev.prevent_default(); add.dispatch(()); }>
            <FormField id="companion-name" label=t("companions-new")>
                <Input id="companion-name" value=name/>
            </FormField>
            <Button r#type="submit">{t("companions-add")}</Button>
        </form>

        <Suspense fallback=|| view! { <Skeleton class="h-24 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{share_error(&e)}</p> }.into_any(),
                Ok(list) => view! {
                    <ul class="mb-8 flex flex-col gap-2" aria-label=t("companions-title")>
                        {list.into_iter().filter(|c| c.merged_into.is_none()).map(|c| {
                            let (inv_id, un_id, row_id) = (c.id.clone(), c.id.clone(), c.id.clone());
                            let can_invite = !c.is_self && c.linked_user.is_none();
                            let can_unlink = !c.is_self && c.linked_user.is_some();
                            view! {
                                <li class="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2">
                                    <span class="flex-1">
                                        {c.name.clone()} {c.is_self.then(|| format!(" {}", t("companions-you")))}
                                    </span>
                                    {can_unlink.then(|| view! { <Badge variant=BadgeVariant::Secondary>{t("companions-linked")}</Badge> })}
                                    {can_invite.then(|| view! {
                                        <Button size=ButtonSize::Sm variant=ButtonVariant::Outline
                                            attr:aria-label=format!("{} {}", t("companions-invite"), c.name)
                                            on:click=move |_| { invite.dispatch(inv_id.clone()); }>
                                            {t("companions-invite")}
                                        </Button>
                                    })}
                                    {can_unlink.then(|| view! {
                                        <Button size=ButtonSize::Sm variant=ButtonVariant::Outline
                                            attr:aria-label=format!("{} {}", t("companions-unlink"), c.name)
                                            on:click=move |_| { unlink.dispatch(un_id.clone()); }>
                                            {t("companions-unlink")}
                                        </Button>
                                    })}
                                    {move || invite_url.get().filter(|(id, _)| *id == row_id).map(|(_, url)| view! {
                                        <p class="basis-full text-sm">
                                            {t("companions-invite-link")} " "
                                            <output class="break-all font-mono" data-testid="invite-link">{url}</output>
                                        </p>
                                    })}
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any(),
            })}
        </Suspense>

        // the pickers read `data`: inside a transition so hydration waits for it
        <Transition fallback=|| ()>
        <section class="mb-8 max-w-md" aria-labelledby="link-title">
            <h2 id="link-title" class="mb-1 font-medium">{t("companions-link-title")}</h2>
            <p class="mb-2 text-sm text-muted-foreground">{t("companions-link-lead")}</p>
            <form class="grid gap-2" on:submit=move |ev| { ev.prevent_default(); request.dispatch(()); }>
                <FormField id="link-companion" label=t("companions-companion")>
                    <Select id="link-companion" value=link_companion options=pick_all/>
                </FormField>
                <FormField id="link-username" label=t("auth-username")>
                    <Input id="link-username" value=link_user/>
                </FormField>
                <Button r#type="submit" variant=ButtonVariant::Outline>{t("companions-link-submit")}</Button>
            </form>
        </section>

        <section class="mb-8 max-w-md" aria-labelledby="merge-title">
            <h2 id="merge-title" class="mb-1 font-medium">{t("companions-merge-title")}</h2>
            <p class="mb-2 text-sm text-muted-foreground">{t("companions-merge-lead")}</p>
            <form class="grid gap-2" on:submit=move |ev| { ev.prevent_default(); merge.dispatch(()); }>
                <FormField id="merge-keep" label=t("companions-merge-keep")>
                    <Select id="merge-keep" value=keep options=pick_all/>
                </FormField>
                <FormField id="merge-remove" label=t("companions-merge-remove")>
                    <Select id="merge-remove" value=remove options=pick_all/>
                </FormField>
                <Button r#type="submit" variant=ButtonVariant::Outline>{t("companions-merge-submit")}</Button>
            </form>
        </section>
        </Transition>

        <Privacy/>
    }
}

/// Per-user privacy: whether linked participants see the sessions you log.
#[component]
fn Privacy() -> impl IntoView {
    let user = expect_context::<crate::app::UserResource>();
    let on = RwSignal::new(true);
    Effect::new(move |_| {
        if let Some(Ok(Some(u))) = user.get() {
            on.set(u.share_sessions);
        }
    });
    let save = Action::new(move |v: &bool| {
        let v = *v;
        async move {
            if let Ok(u) = api::set_privacy(v).await {
                on.set(u.share_sessions);
            }
        }
    });
    view! {
        <section class="max-w-md" aria-labelledby="privacy-title">
            <h2 id="privacy-title" class="mb-2 font-medium">{t("privacy-title")}</h2>
            <div class="flex items-center gap-3">
                <input id="share-sessions" type="checkbox" class="size-4 rounded border-input accent-primary"
                    prop:checked=move || on.get()
                    on:change=move |ev| { let v = event_target_checked(&ev); on.set(v); save.dispatch(v); }/>
                <Label r#for="share-sessions">{t("privacy-share-sessions")}</Label>
            </div>
        </section>
    }
}

fn session_row(s: LoggedSessionDto, claim: Option<Action<String, ()>>) -> impl IntoView {
    let date = s.started_at.get(..10).unwrap_or_default().to_string();
    let names = s
        .participants
        .iter()
        .map(|p| {
            let mut n = if p.is_me {
                t("companions-you-short")
            } else {
                p.name.clone()
            };
            if p.winner {
                n.push_str(" *");
            }
            n
        })
        .collect::<Vec<_>>()
        .join(", ");
    let by = (!s.mine)
        .then(|| use_i18n().t_args("sessions-by", &[("name", s.owner_name.clone().into())]));
    let id = s.id.clone();
    view! {
        <li class="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2">
            <span class="flex-1">
                <A href=format!("/items/{}", s.item_id) attr:class="font-medium underline-offset-2 hover:underline">{s.item_title.clone()}</A>
                <span class="ml-2 text-sm text-muted-foreground">{date}</span>
                <span class="block text-sm">{names}</span>
                {by.map(|b| view! { <span class="block text-xs text-muted-foreground">{b}</span> })}
            </span>
            {claim.map(|a| view! {
                <Button size=ButtonSize::Sm variant=ButtonVariant::Outline on:click=move |_| { a.dispatch(id.clone()); }>
                    {t("sessions-claim")}
                </Button>
            })}
        </li>
    }
}

/// History of sessions: the user's own, and those of others the user took part in.
#[component]
pub fn Sessions() -> impl IntoView {
    let mine = Resource::new(|| (), |()| api::sessions(false));
    let shared = Resource::new(|| (), |()| api::sessions(true));
    refetch_on_update(move || mine.refetch());
    refetch_on_update(move || shared.refetch());
    let message = RwSignal::new(None::<String>);
    let claim = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            match api::claim(id).await {
                Ok(_) => message.set(Some(t("sessions-claimed"))),
                Err(e) => message.set(Some(share_error(&e))),
            }
            mine.refetch();
        }
    });
    view! {
        <PageTitle text=t("sessions-title")/>
        <p role="status" class="mb-4 text-sm">{move || message.get()}</p>
        <section class="mb-8" aria-labelledby="shared-title">
            <h2 id="shared-title" class="mb-2 font-medium">{t("sessions-shared")}</h2>
            <Suspense fallback=|| view! { <Skeleton class="h-16 w-full"/> }>
                {move || shared.get().map(|r| match r {
                    Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                    Err(e) => view! { <p role="alert">{share_error(&e)}</p> }.into_any(),
                    Ok(list) if list.is_empty() => view! { <p class="text-sm text-muted-foreground">{t("sessions-shared-empty")}</p> }.into_any(),
                    Ok(list) => view! {
                        <ul class="flex flex-col gap-2" aria-labelledby="shared-title">
                            {list.into_iter().map(|s| session_row(s, Some(claim))).collect_view()}
                        </ul>
                    }.into_any(),
                })}
            </Suspense>
        </section>
        <section aria-labelledby="mine-title">
            <h2 id="mine-title" class="mb-2 font-medium">{t("sessions-mine")}</h2>
            <Suspense fallback=|| view! { <Skeleton class="h-16 w-full"/> }>
                {move || mine.get().and_then(Result::ok).map(|list| if list.is_empty() {
                    view! { <p class="text-sm text-muted-foreground">{t("sessions-mine-empty")}</p> }.into_any()
                } else {
                    view! {
                        <ul class="flex flex-col gap-2" aria-labelledby="mine-title">
                            {list.into_iter().map(|s| session_row(s, None)).collect_view()}
                        </ul>
                    }.into_any()
                })}
            </Suspense>
        </section>
    }
}

/// Public page of a link-shared list (no account needed).
#[component]
pub fn SharedList() -> impl IntoView {
    let params = use_params_map();
    let data = Resource::new(
        move || params.with(|p| p.get("token").unwrap_or_default()),
        api::shared_list,
    );
    view! {
        <Suspense fallback=|| view! { <Skeleton class="h-24 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(_) => view! { <p role="alert">{t("shared-list-missing")}</p> }.into_any(),
                Ok(l) => {
                    let by = use_i18n().t_args("sessions-by", &[("name", l.owner_name.clone().into())]);
                    view! {
                        <PageTitle text=l.name.clone()/>
                        <p class="mb-4 text-sm text-muted-foreground">{by}</p>
                        {l.description.clone().map(|d| view! { <p class="mb-4">{d}</p> })}
                        <ul class="flex flex-col gap-1" aria-label=l.name.clone()>
                            {l.items.into_iter().map(|i| view! {
                                <li>{i.title.clone()} {i.year.map(|y| format!(" ({y})"))}</li>
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }
            })}
        </Suspense>
    }
}
