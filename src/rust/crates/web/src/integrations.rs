//! Integrations (M14): tokens for the watched webhook that media servers and scripts post to.

use contracts::IntegrationCreatedDto;
use leptos::prelude::*;
use ui::{Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, FormField, Input, Skeleton};

use crate::api;
use crate::i18n::t;
use crate::pages::{PageTitle, error_text, go, needs_sign_in};

/// An example body for the webhook, shown with a new token.
const EXAMPLE: &str = r#"{"kind": "movie", "title": "Heat", "year": 1995,
 "ids": {"tmdb": "949", "imdb": "tt0113277"},
 "watched_at": "2026-01-01T21:00:00Z", "source": "Jellyfin"}"#;

#[component]
pub fn Integrations() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::integrations());
    let name = RwSignal::new(String::new());
    let created = RwSignal::new(None::<IntegrationCreatedDto>);
    let message = RwSignal::new(None::<String>);
    let add = Action::new(move |(): &()| {
        let n = name.get_untracked();
        async move {
            match api::create_integration(n).await {
                Ok(c) => {
                    name.set(String::new());
                    created.set(Some(c));
                    message.set(None);
                }
                Err(e) => message.set(Some(error_text(&e))),
            }
            data.refetch();
        }
    });
    let remove = Action::new(move |id: &String| {
        let id = id.clone();
        async move {
            let _ = api::delete_integration(id).await;
            data.refetch();
        }
    });
    view! {
        <PageTitle text=t("integrations-title")/>
        <p class="mb-4 max-w-2xl text-sm text-muted-foreground">{t("integrations-lead")}</p>
        <p role="status" class="mb-4 text-sm">{move || message.get()}</p>
        <form class="mb-6 flex max-w-md items-end gap-2" on:submit=move |ev| { ev.prevent_default(); add.dispatch(()); }>
            <FormField id="integration-name" label=t("integrations-name")>
                <Input id="integration-name" value=name/>
            </FormField>
            <Button r#type="submit">{t("integrations-add")}</Button>
        </form>
        {move || created.get().map(|c| {
            let curl = format!(
                "curl -X POST {} -H 'Authorization: Bearer {}' -H 'Content-Type: application/json' -d '{}'",
                c.webhook_url, c.token, EXAMPLE.replace('\n', "")
            );
            view! {
                <section class="mb-6 max-w-2xl rounded-md border border-primary p-4 text-sm" aria-labelledby="new-token">
                    <h2 id="new-token" class="mb-2 font-medium">{t("integrations-created")}</h2>
                    <p>{t("integrations-url")} " " <output class="break-all font-mono" data-testid="webhook-url">{c.webhook_url.clone()}</output></p>
                    <p>{t("integrations-token")} " " <output class="break-all font-mono" data-testid="webhook-token">{c.token.clone()}</output></p>
                    <p class="mt-2">{t("integrations-example")}</p>
                    <pre class="mt-1 overflow-x-auto rounded bg-muted p-2 font-mono text-xs">{curl}</pre>
                </section>
            }
        })}
        <Suspense fallback=|| view! { <Skeleton class="h-16 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{error_text(&e)}</p> }.into_any(),
                Ok(list) if list.is_empty() => view! { <p class="text-sm text-muted-foreground">{t("integrations-empty")}</p> }.into_any(),
                Ok(list) => view! {
                    <ul class="flex max-w-2xl flex-col gap-2" aria-label=t("integrations-title")>
                        {list.into_iter().map(|i| {
                            let id = i.id.clone();
                            let seen = i.last_seen_at.as_deref().map_or_else(
                                || t("integrations-never"),
                                |s| s.get(..16).unwrap_or_default().replace('T', " "),
                            );
                            let label = format!("{} {}", t("integrations-revoke"), i.name);
                            view! {
                                <li class="flex flex-wrap items-center gap-2 rounded-md border border-border px-3 py-2">
                                    <span class="flex-1">
                                        <span class="font-medium">{i.name.clone()}</span>
                                        <span class="ml-2 text-sm text-muted-foreground">{t("integrations-last")} " " {seen}</span>
                                    </span>
                                    {i.last_error.clone().map(|e| view! { <Badge variant=BadgeVariant::Destructive attr:title=e>{t("integrations-error")}</Badge> })}
                                    <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost attr:aria-label=label
                                        on:click=move |_| { remove.dispatch(id.clone()); }>{t("integrations-revoke")}</Button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                }.into_any(),
            })}
        </Suspense>
    }
}
