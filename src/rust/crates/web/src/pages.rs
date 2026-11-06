//! Pages. Data comes from server functions; `entity.updated` events refetch (no polling).

use contracts::{ItemPageDto, NewProgressDto};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use ui::{Button, ButtonVariant, FormField, Input, Skeleton};

use crate::api;
use crate::events::{mentions, use_events};
use crate::i18n::{t, use_i18n};
use crate::widgets::{CompanionPicker, Cover, PosterCard, RatingInput};

/// Full page load after sign-in/out so the server renders with the new cookie.
fn go(path: &str) {
    #[cfg(feature = "hydrate")]
    if let Some(w) = web_sys::window() {
        let _ = w.location().assign(path);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = path;
}

fn error_text(e: &ServerFnError) -> String {
    let code = api::error_code(e);
    let key = format!("auth-error-{code}");
    let text = t(&key);
    if text == key {
        t("error-generic")
    } else {
        text
    }
}

#[component]
fn PageTitle(#[prop(into)] text: String) -> impl IntoView {
    let product = expect_context::<crate::app::ClientConfig>().product_name;
    view! {
        <Title text=format!("{text} - {product}")/>
        <h1 class="mb-6 text-2xl font-semibold tracking-tight">{text}</h1>
    }
}

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <section class="mx-auto max-w-2xl py-16 text-center">
            <h1 class="text-3xl font-semibold tracking-tight">{t("home-title")}</h1>
            <p class="mt-4 text-muted-foreground">{t("home-lead")}</p>
            <div class="mt-8 flex justify-center gap-4">
                <A href="/library" attr:class="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground">{t("nav-library")}</A>
                <A href="/sign-up" attr:class="rounded-md border border-input px-4 py-2 text-sm font-medium">{t("nav-sign-up")}</A>
            </div>
        </section>
    }
}

#[component]
pub fn SignIn() -> impl IntoView {
    let i18n = use_i18n();
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let totp = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let need_totp = RwSignal::new(false);
    let submit = Action::new(move |(): &()| {
        let (u, p, c) = (
            username.get_untracked(),
            password.get_untracked(),
            totp.get_untracked(),
        );
        async move {
            match api::sign_in(u, p, Some(c)).await {
                Ok(_) => go("/library"),
                Err(e) => {
                    if api::error_code(&e) == "totp_required" {
                        need_totp.set(true);
                    }
                    error.set(Some(error_text(&e)));
                }
            }
        }
    });
    let s = |k: &str| i18n.t(k);
    let (title, l_user, l_pass, l_submit, no_acc, l_signup) = (
        s("auth-sign-in-title"),
        s("auth-username"),
        s("auth-password"),
        s("auth-sign-in-submit"),
        s("auth-no-account"),
        s("nav-sign-up"),
    );
    view! {
        <PageTitle text=title/>
        <form class="grid max-w-sm gap-4" on:submit=move |ev| { ev.prevent_default(); submit.dispatch(()); }>
            <FormField id="username" label=l_user>
                <Input id="username" value=username/>
            </FormField>
            <FormField id="password" label=l_pass>
                <Input id="password" r#type="password" value=password/>
            </FormField>
            <Show when=move || need_totp.get()>
                <FormField id="totp" label=t("auth-totp")>
                    <Input id="totp" value=totp/>
                </FormField>
            </Show>
            <p role="alert" class="text-sm text-destructive">{move || error.get()}</p>
            <Button r#type="submit" disabled=Signal::derive(move || submit.pending().get())>{l_submit}</Button>
            <p class="text-sm text-muted-foreground">
                {no_acc} " " <A href="/sign-up" attr:class="underline">{l_signup}</A>
            </p>
        </form>
    }
}

#[component]
pub fn SignUp() -> impl IntoView {
    let i18n = use_i18n();
    let username = RwSignal::new(String::new());
    let display = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let submit = Action::new(move |(): &()| {
        let (u, d, e, p) = (
            username.get_untracked(),
            display.get_untracked(),
            email.get_untracked(),
            password.get_untracked(),
        );
        async move {
            let d = if d.trim().is_empty() { u.clone() } else { d };
            match api::sign_up(u, d, p, Some(e)).await {
                Ok(_) => go("/library"),
                Err(e) => error.set(Some(error_text(&e))),
            }
        }
    });
    let s = |k: &str| i18n.t(k);
    let (title, l_user, l_display, l_email, l_pass, l_submit, have, l_signin) = (
        s("auth-sign-up-title"),
        s("auth-username"),
        s("auth-display-name"),
        s("auth-email"),
        s("auth-password"),
        s("auth-sign-up-submit"),
        s("auth-have-account"),
        s("nav-sign-in"),
    );
    view! {
        <PageTitle text=title/>
        <form class="grid max-w-sm gap-4" on:submit=move |ev| { ev.prevent_default(); submit.dispatch(()); }>
            <FormField id="username" label=l_user>
                <Input id="username" value=username/>
            </FormField>
            <FormField id="display_name" label=l_display>
                <Input id="display_name" value=display/>
            </FormField>
            <FormField id="email" label=l_email>
                <Input id="email" r#type="email" value=email/>
            </FormField>
            <FormField id="password" label=l_pass>
                <Input id="password" r#type="password" value=password/>
            </FormField>
            <p role="alert" class="text-sm text-destructive">{move || error.get()}</p>
            <Button r#type="submit" disabled=Signal::derive(move || submit.pending().get())>{l_submit}</Button>
            <p class="text-sm text-muted-foreground">
                {have} " " <A href="/sign-in" attr:class="underline">{l_signin}</A>
            </p>
        </form>
    }
}

/// Redirect to sign-in when a server function says the caller is not signed in.
fn needs_sign_in(e: &ServerFnError) -> bool {
    api::error_code(e) == "unauthorized"
}

#[component]
pub fn Library() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(|| (), |()| api::library());
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && topic == "entity.updated"
        {
            data.refetch();
        }
    });
    view! {
        <PageTitle text=i18n.t("library-title")/>
        <Suspense fallback=|| view! { <Skeleton class="h-48 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{error_text(&e)}</p> }.into_any(),
                Ok(list) if list.is_empty() => view! { <p class="text-muted-foreground">{t("library-empty")}</p> }.into_any(),
                Ok(list) => {
                    let count = use_i18n().t_args("library-count", &[("count", list.len().into())]);
                    view! {
                        <p class="mb-4 text-sm text-muted-foreground">{count}</p>
                        <ul class="grid grid-cols-2 gap-4 sm:grid-cols-3 md:grid-cols-5" aria-label=t("library-title")>
                            {list.into_iter().map(|li| view! {
                                <PosterCard item=li.item status=li.entry.status/>
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }
            })}
        </Suspense>
    }
}

#[component]
pub fn ItemPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();
    let data = Resource::new(id, api::item_page);
    let companions = Resource::new(|| (), |()| api::companions());
    let lists = Resource::new(|| (), |()| api::lists());
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, payload)) = events.0.get()
            && topic == "entity.updated"
            && mentions(&payload, &id())
        {
            data.refetch();
        }
    });
    // Actions and form state live here, not in `ItemView`: a refetch (for example after
    // `entity.updated`) rebuilds the view, and must not drop a pending click or the
    // companions the user ticked.
    let state = ItemState {
        with: RwSignal::new(Vec::new()),
        logged: RwSignal::new(false),
        add: Action::new(move |item: &String| {
            let item = item.clone();
            async move {
                let _ = api::add_to_library(item).await;
                data.refetch();
            }
        }),
        done: Action::new(move |(item, with): &(String, Vec<String>)| {
            let (item, with) = (item.clone(), with.clone());
            async move {
                let p = NewProgressDto {
                    progress: serde_json::json!({"type": "finished"}),
                    occurred_at: None,
                    client_op_id: Some(uuid::Uuid::now_v7().to_string()),
                    with,
                };
                let ok = api::log_progress(item, p).await.is_ok();
                data.refetch();
                ok
            }
        }),
        add_list: Action::new(move |(list, item): &(String, String)| {
            let (list, item) = (list.clone(), item.clone());
            async move {
                let _ = api::add_to_list(list, item).await;
                lists.refetch();
            }
        }),
    };
    Effect::new(move |_| {
        if state.done.value().get() == Some(true) {
            state.logged.set(true);
        }
    });
    view! {
        <Suspense fallback=|| view! { <Skeleton class="h-64 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{error_text(&e)}</p> }.into_any(),
                Ok(page) => view! {
                    <ItemView page=page state=state companions=companions lists=lists/>
                }.into_any(),
            })}
        </Suspense>
    }
}

#[derive(Clone, Copy)]
struct ItemState {
    with: RwSignal<Vec<String>>,
    logged: RwSignal<bool>,
    add: Action<String, ()>,
    done: Action<(String, Vec<String>), bool>,
    add_list: Action<(String, String), ()>,
}

#[component]
fn ItemView(
    page: ItemPageDto,
    state: ItemState,
    companions: Resource<Result<Vec<contracts::CompanionDto>, ServerFnError>>,
    lists: Resource<Result<Vec<contracts::ListDto>, ServerFnError>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let item = page.item.clone();
    let item_id = item.id.clone();
    let rating = RwSignal::new(page.entry.as_ref().and_then(|e| e.rating));
    let ItemState {
        with,
        logged,
        add,
        done,
        add_list,
    } = state;
    let rate = {
        let id = item_id.clone();
        Callback::new(move |v: Option<u8>| {
            let id = id.clone();
            leptos::task::spawn_local(async move {
                let _ = api::rate(id, v).await;
            });
        })
    };
    let item_id_for_lists = item_id.clone();
    let in_library = page.entry.is_some();
    let status = page
        .entry
        .as_ref()
        .map(|e| t(&format!("status-{}", e.status)));
    // partial items already show search data; only bare stubs are "loading"
    let loading = item.state == "stub";
    let kind = crate::i18n::kind_label(&item.kind);
    view! {
        <Title text=item.title.clone()/>
        <article class="grid gap-8 md:grid-cols-[220px_1fr]">
            <Cover title=item.title.clone() image=item.cover.clone() sizes="220px"/>
            <div class="flex flex-col gap-4">
                <div>
                    <h1 class="text-2xl font-semibold tracking-tight">{item.title.clone()}</h1>
                    <p class="text-sm text-muted-foreground">
                        {kind}{item.year.map(|y| format!(" - {y}"))}
                        {item.original_title.clone().filter(|o| o != &item.title).map(|o| format!(" - {o}"))}
                    </p>
                </div>
                {loading.then(|| view! { <p class="text-sm text-muted-foreground" aria-live="polite">{t("item-loading-details")}</p> })}
                {item.description.clone().map(|d| view! { <p class="max-w-prose">{d}</p> })}
                <div class="flex flex-wrap items-center gap-3">
                    {if in_library {
                        view! { <span class="rounded-md bg-muted px-3 py-1.5 text-sm">{t("item-in-library")} {status.map(|s| format!(" - {s}"))}</span> }.into_any()
                    } else {
                        { let id = item_id.clone(); view! { <Button on:click=move |_| { add.dispatch(id.clone()); }>{t("item-add")}</Button> }.into_any() }
                    }}
                </div>
                <section class="flex flex-col gap-3 rounded-md border border-border p-4">
                    <Suspense fallback=|| ()>
                        {move || companions.get().and_then(Result::ok).map(|cs| view! { <CompanionPicker companions=cs selected=with/> })}
                    </Suspense>
                    <div class="flex items-center gap-3">
                        <Button variant=ButtonVariant::Secondary on:click={ let id = item_id.clone(); move |_| { done.dispatch((id.clone(), with.get_untracked())); } }>{t("item-mark-done")}</Button>
                        <span aria-live="polite" class="text-sm text-muted-foreground">{move || logged.get().then(|| t("item-done-logged"))}</span>
                    </div>
                </section>
                {in_library.then(|| view! {
                    <section class="flex flex-col gap-2">
                        <h2 class="text-sm font-medium">{t("item-rating")}</h2>
                        <RatingInput value=rating on_change=rate/>
                    </section>
                })}
                <Suspense fallback=|| ()>
                    {move || lists.get().and_then(Result::ok).filter(|l| !l.is_empty()).map(|ls| view! {
                        <section class="flex flex-col gap-2">
                            <h2 class="text-sm font-medium">{t("item-lists")}</h2>
                            <div class="flex flex-wrap gap-2">
                                {ls.into_iter().map(|l| {
                                    let lid = l.id.clone();
                                    view! {
                                        <Button variant=ButtonVariant::Outline on:click={ let item = item_id_for_lists.clone(); move |_| { add_list.dispatch((lid.clone(), item.clone())); } }>{l.name.clone()}</Button>
                                    }
                                }).collect_view()}
                            </div>
                        </section>
                    })}
                </Suspense>
                <section class="flex flex-col gap-2">
                    <h2 class="text-sm font-medium">{t("item-history")}</h2>
                    {if page.progress.is_empty() {
                        view! { <p class="text-sm text-muted-foreground">{t("item-history-empty")}</p> }.into_any()
                    } else {
                        view! {
                            <ul class="text-sm">
                                {page.progress.iter().map(|p| view! {
                                    <li>{p.occurred_at.get(..10).unwrap_or_default().to_string()} " - " {p.progress["type"].as_str().unwrap_or_default().to_string()}</li>
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }}
                </section>
                <footer class="mt-4 border-t border-border pt-4 text-xs text-muted-foreground" data-testid="attribution">
                    {page.sources.iter().map(|s| view! {
                        <p>
                            {i18n.t_args("item-source", &[("name", s.name.clone().into())])} " - "
                            {s.text.clone()} " " <a href=s.url.clone() class="underline" rel="noopener">{s.url.clone()}</a>
                        </p>
                    }).collect_view()}
                </footer>
            </div>
        </article>
    }
}

#[component]
pub fn Companions() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(|| (), |()| api::companions());
    let name = RwSignal::new(String::new());
    let add = Action::new(move |(): &()| {
        let n = name.get_untracked();
        async move {
            if api::create_companion(n).await.is_ok() {
                name.set(String::new());
            }
            data.refetch();
        }
    });
    view! {
        <PageTitle text=i18n.t("companions-title")/>
        <p class="mb-4 text-sm text-muted-foreground">{i18n.t("companions-lead")}</p>
        <form class="mb-6 flex max-w-md items-end gap-2" on:submit=move |ev| { ev.prevent_default(); add.dispatch(()); }>
            <FormField id="companion-name" label=i18n.t("companions-new")>
                <Input id="companion-name" value=name/>
            </FormField>
            <Button r#type="submit">{i18n.t("companions-add")}</Button>
        </form>
        <Suspense fallback=|| view! { <Skeleton class="h-24 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{error_text(&e)}</p> }.into_any(),
                Ok(list) => view! {
                    <ul class="flex flex-col gap-2" aria-label=t("companions-title")>
                        {list.into_iter().filter(|c| c.merged_into.is_none()).map(|c| view! {
                            <li class="rounded-md border border-border px-3 py-2">
                                {c.name.clone()} {c.is_self.then(|| format!(" {}", t("companions-you")))}
                            </li>
                        }).collect_view()}
                    </ul>
                }.into_any(),
            })}
        </Suspense>
    }
}

#[component]
pub fn Lists() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(|| (), |()| api::lists());
    let name = RwSignal::new(String::new());
    let add = Action::new(move |(): &()| {
        let n = name.get_untracked();
        async move {
            if api::create_list(n).await.is_ok() {
                name.set(String::new());
            }
            data.refetch();
        }
    });
    view! {
        <PageTitle text=i18n.t("lists-title")/>
        <form class="mb-6 flex max-w-md items-end gap-2" on:submit=move |ev| { ev.prevent_default(); add.dispatch(()); }>
            <FormField id="list-name" label=i18n.t("lists-new")>
                <Input id="list-name" value=name/>
            </FormField>
            <Button r#type="submit">{i18n.t("lists-create")}</Button>
        </form>
        <Suspense fallback=|| view! { <Skeleton class="h-24 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{error_text(&e)}</p> }.into_any(),
                Ok(list) if list.is_empty() => view! { <p class="text-muted-foreground">{t("lists-empty")}</p> }.into_any(),
                Ok(list) => view! {
                    <ul class="flex flex-col gap-2">
                        {list.into_iter().map(|l| {
                            let count = use_i18n().t_args("lists-items", &[("count", l.item_ids.len().into())]);
                            view! {
                                <li class="rounded-md border border-border px-3 py-2">
                                    <span class="font-medium">{l.name.clone()}</span>
                                    <span class="ml-2 text-sm text-muted-foreground">{count}</span>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any(),
            })}
        </Suspense>
    }
}

#[component]
pub fn Sources() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(|| (), |()| api::sources());
    view! {
        <PageTitle text=i18n.t("sources-title")/>
        <p class="mb-4 text-sm text-muted-foreground">{i18n.t("sources-lead")}</p>
        <Suspense fallback=|| ()>
            {move || data.get().and_then(Result::ok).map(|list| view! {
                <ul class="flex flex-col gap-3">
                    {list.into_iter().map(|s| view! {
                        <li class="rounded-md border border-border p-3 text-sm">
                            <p class="font-medium">{s.name.clone()}</p>
                            <p>{s.text.clone()}</p>
                            <p class="text-muted-foreground">{s.licence.clone()} " - " <a class="underline" href=s.url.clone() rel="noopener">{s.url.clone()}</a></p>
                        </li>
                    }).collect_view()}
                </ul>
            })}
        </Suspense>
    }
}
