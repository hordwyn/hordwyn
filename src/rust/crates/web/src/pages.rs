//! Pages. Data comes from server functions; `entity.updated` events refetch (no polling).

use leptos::prelude::*;
use leptos_meta::Title;
use ui::{Button, ButtonVariant, FormField, Input, Skeleton};

use crate::api;
pub use crate::auth_pages::{SignIn, SignUp};
use crate::i18n::{t, use_i18n};

/// Full page load after sign-in/out so the server renders with the new cookie.
pub(crate) fn go(path: &str) {
    #[cfg(feature = "hydrate")]
    if let Some(w) = web_sys::window() {
        let _ = w.location().assign(path);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = path;
}

/// Reload the current page from the server (after a change the server renders, such as the
/// language).
pub(crate) fn reload() {
    #[cfg(feature = "hydrate")]
    if let Some(w) = web_sys::window() {
        let _ = w.location().reload();
    }
}

pub(crate) fn error_text(e: &ServerFnError) -> String {
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
pub(crate) fn PageTitle(#[prop(into)] text: String) -> impl IntoView {
    let product = expect_context::<crate::app::ClientConfig>().product_name;
    view! {
        <Title text=format!("{text} - {product}")/>
        <h1 class="mb-6 text-2xl font-semibold tracking-tight">{text}</h1>
    }
}

/// Redirect to sign-in when a server function says the caller is not signed in.
pub(crate) fn needs_sign_in(e: &ServerFnError) -> bool {
    api::error_code(e) == "unauthorized"
}

/// The server refused the query as malformed (400) or invalid (422). Filters come from a
/// URL that may be old or hand-edited, so the pages show "no match" with "Clear filters"
/// instead of an error whose Retry cannot help (UX1-7).
pub(crate) fn bad_query(e: &ServerFnError) -> bool {
    matches!(api::error_code(e).as_str(), "bad_request" | "invalid")
}

/// A key that changes on every call. The data pages render each new value of a resource
/// afresh, `<For each=move || data.get().map(|d| (fresh(n), d)) key=|(k, _)| *k ...>`,
/// instead of rebuilding the previous render in place, which kept event handlers whose
/// state was already disposed (found on the item page; UX1-2 applies it to every list).
pub(crate) fn fresh(counter: StoredValue<u32>) -> u32 {
    counter.update_value(|v| *v = v.wrapping_add(1));
    counter.get_value()
}

#[component]
pub fn Lists() -> impl IntoView {
    let i18n = use_i18n();
    let data = Resource::new(|| (), |()| api::lists());
    // list id -> public URL, after "share by link"
    let shared = RwSignal::new(None::<(String, String)>);
    let share = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            if let Ok(l) = api::share_list(id.clone()).await {
                shared.set(Some((id, crate::sharing::absolute(&l.path))));
            }
            data.refetch();
        }
    });
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
                            let (share_id, row_id) = (l.id.clone(), l.id.clone());
                            let label = use_i18n().t_args("lists-share-named", &[("name", l.name.clone().into())]);
                            view! {
                                <li class="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2">
                                    <span class="flex-1">
                                        <span class="font-medium">{l.name.clone()}</span>
                                        <span class="ml-2 text-sm text-muted-foreground">{count}</span>
                                    </span>
                                    <Button size=ui::ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=label
                                        on:click=move |_| { share.dispatch(share_id.clone()); }>
                                        {t("lists-share")}
                                    </Button>
                                    {move || shared.get().filter(|(id, _)| *id == row_id).map(|(_, url)| view! {
                                        <p class="basis-full text-sm">
                                            {t("lists-share-link")} " "
                                            <output class="break-all font-mono" data-testid="share-link">{url}</output>
                                        </p>
                                    })}
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any(),
            })}
        </Suspense>
    }
}
