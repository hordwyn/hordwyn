//! Settings > Account, phase 3 parts (design.md 6.12, addendum-1 4.2 and 4.6; DEC-306,
//! DEC-312): "Linked sign-ins" with Link and Unlink (Unlink is refused while it is the
//! account's only way to sign in, and the page says why) and the time zone, with the
//! browser's zone as a one-click suggestion. Dates and times on Home, History and Stats
//! follow the zone the server knows (DEC-239).

use contracts::admin::{LinkedIdentityDto, OidcButtonDto};
use leptos::prelude::*;
use serde_json::json;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant, FormField, Input,
    field_described_by,
};

use crate::admin_api;
use crate::i18n::{t, use_i18n};
use crate::pages::{go, reload};
use crate::reauth::{gated, use_reauth};
use crate::rest;
use crate::settings::{Outcome, admin_error_with, loaded};
use crate::sign_in::encode;

type Message = RwSignal<Option<Result<String, String>>>;

/// Where the link round trip starts (mode `link`, DEC-312), coming back to this page.
#[must_use]
pub fn link_url(slug: &str) -> String {
    format!(
        "/api/v1/auth/oidc/start?provider={}&mode=link&next={}",
        encode(slug),
        encode("/settings/account")
    )
}

/// Providers the account is not linked to yet (by slug).
#[must_use]
pub fn linkable(providers: &[OidcButtonDto], linked: &[LinkedIdentityDto]) -> Vec<OidcButtonDto> {
    providers
        .iter()
        .filter(|p| {
            !linked
                .iter()
                .any(|l| l.provider_slug.as_deref() == Some(&p.slug))
        })
        .cloned()
        .collect()
}

/// "Linked sign-ins" (addendum-1 4.2): one row per provider with Unlink, and Link for the
/// others. `error` is a failed link round trip (`/settings/account?error=<code>`).
#[component]
pub fn LinkedSignIns(error: Option<String>) -> impl IntoView {
    let ids = Resource::new(|| (), |()| admin_api::my_identities());
    let page = Resource::new(|| (), |()| admin_api::sign_in_page());
    let message: Message = RwSignal::new(None);
    let failed = error.map(|code| {
        let key = format!("signin-error-{code}");
        let text = t(&key);
        if text == key {
            t("signin-error-oidc_failed")
        } else {
            text
        }
    });
    view! {
        <section class="grid max-w-xl gap-3" aria-labelledby="set-linked-h">
            <h3 id="set-linked-h" class="font-semibold">{t("settings-linked")}</h3>
            <p class="text-sm text-muted-foreground">{t("settings-linked-lead")}</p>
            {failed.map(|f| view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{f}</Alert> })}
            <Outcome message=message/>
            {loaded(ids, move |list| {
                let linked = list.clone();
                view! {
                    {list.is_empty().then(|| view! { <p class="text-sm">{t("settings-linked-none")}</p> })}
                    <ul class="grid gap-2">
                        {list.into_iter().map(|l| view! { <LinkedRow l=l ids=ids message=message/> }).collect_view()}
                    </ul>
                    {loaded(page, move |p| view! { <LinkButtons providers=linkable(&p.providers, &linked)/> })}
                }
            })}
        </section>
    }
}

#[component]
fn LinkedRow(
    l: LinkedIdentityDto,
    ids: Resource<Result<Vec<LinkedIdentityDto>, ServerFnError>>,
    message: Message,
) -> impl IntoView {
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let i18n = use_i18n();
    let since = i18n.t_args(
        "settings-linked-since",
        &[("date", i18n.datetime(&l.linked_at).into())],
    );
    let label = i18n.t_args(
        "settings-unlink-named",
        &[("provider", l.provider.clone().into())],
    );
    let path = format!("/auth/me/identities/{}", l.id);
    let unlink = move |_| {
        let path = path.clone();
        let call = move || {
            let path = path.clone();
            async move { rest::send("DELETE", &path, None).await }
        };
        let done = t("settings-unlinked");
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<(), String>| {
                let ok = r.is_ok();
                message.set(Some(r.map(|()| done.clone())));
                if ok {
                    ids.refetch();
                }
            }),
        );
    };
    view! {
        <li data-testid="linked-row" class="flex flex-wrap items-center gap-x-4 gap-y-1 rounded-xl border bg-card px-4 py-3">
            <span class="grid min-w-0 flex-1">
                <span class="font-medium">{l.provider.clone()}</span>
                <span class="text-sm text-muted-foreground">{since}</span>
                {(!l.can_unlink).then(|| view! { <span class="text-sm text-muted-foreground">{t("settings-linked-only")}</span> })}
            </span>
            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=label loading=busy on:click=unlink>
                {t("settings-unlink")}
            </Button>
        </li>
    }
}

/// "Link <provider>": linking needs a fresh proof at the start (DEC-312), so the dialog
/// comes first when the window is closed.
#[component]
fn LinkButtons(providers: Vec<OidcButtonDto>) -> impl IntoView {
    let reauth = use_reauth();
    let i18n = use_i18n();
    view! {
        <div class="flex flex-wrap gap-2">
            {providers.into_iter().map(|p| {
                let url = link_url(&p.slug);
                let label = i18n.t_args("settings-link-provider", &[("provider", p.display_name.into())]);
                let open = move |_| {
                    let url = url.clone();
                    leptos::task::spawn_local(async move {
                        let fresh = admin_api::reauth_status().await.is_ok_and(|s| s.fresh);
                        if fresh {
                            go(&url);
                        } else {
                            reauth.ask(Some(Callback::new(move |()| go(&url))));
                        }
                    });
                };
                view! { <Button variant=ButtonVariant::Outline on:click=open>{label}</Button> }
            }).collect_view()}
        </div>
    }
}

/// The time zone (DEC-306): stored per user, shown as typed; the browser's own zone is one
/// click away. Times render on the server, so a change reloads the page.
#[component]
pub fn TimeZone() -> impl IntoView {
    let user = expect_context::<crate::app::UserResource>();
    view! {
        <Suspense fallback=|| ()>
            {move || user.get().map(|u| {
                let zone = u.ok().flatten().and_then(|u| u.time_zone);
                view! { <TimeZoneForm zone=zone/> }
            })}
        </Suspense>
    }
}

#[component]
fn TimeZoneForm(zone: Option<String>) -> impl IntoView {
    let i18n = use_i18n();
    let value = RwSignal::new(zone.clone().unwrap_or_default());
    let browser = RwSignal::new(None::<String>);
    let zones = RwSignal::new(Vec::<String>::new());
    // the browser's zone and list exist only after hydration
    Effect::new(move |_| {
        browser.set(rest::browser_time_zone());
        zones.set(rest::browser_time_zones());
    });
    let message: Message = RwSignal::new(None);
    let save = Action::new(move |z: &Option<String>| {
        let z = z.clone();
        let texts = use_i18n();
        async move {
            match rest::send("PUT", "/auth/me/time-zone", Some(json!({"time_zone": z}))).await {
                Ok(()) => reload(),
                Err(e) => message.set(Some(Err(admin_error_with(&texts, &e)))),
            }
        }
    });
    let current = zone.as_deref().map_or_else(
        || t("settings-tz-none"),
        |z| i18n.t_args("settings-tz-current", &[("zone", z.to_string().into())]),
    );
    let suggestion = move || {
        let b = browser.get()?;
        (zone.as_deref() != Some(b.as_str())).then_some(b)
    };
    let typed = move || {
        let v = value.get_untracked().trim().to_string();
        (!v.is_empty()).then_some(v)
    };
    view! {
        <section class="grid max-w-xl gap-3" aria-labelledby="set-tz-h">
            <h2 id="set-tz-h" class="text-lg font-semibold">{t("settings-tz")}</h2>
            <p data-testid="tz-current" class="text-sm">{current}</p>
            {move || suggestion().map(|b| {
                let label = use_i18n().t_args("settings-tz-use-browser", &[("zone", b.clone().into())]);
                view! {
                    <Button variant=ButtonVariant::Outline class="w-fit" loading=save.pending()
                        on:click=move |_| { save.dispatch(Some(b.clone())); }>{label}</Button>
                }
            })}
            <form class="grid gap-3" on:submit=move |ev| { ev.prevent_default(); save.dispatch(typed()); }>
                <FormField id="set-tz" label=t("settings-tz-field") description=t("settings-tz-hint")>
                    <Input id="set-tz" value=value autocomplete="off" described_by=field_described_by("set-tz")
                        attr:list="set-tz-zones"/>
                </FormField>
                <datalist id="set-tz-zones">
                    {move || zones.get().into_iter().map(|z| view! { <option value=z></option> }).collect_view()}
                </datalist>
                <Outcome message=message/>
                <div class="flex flex-wrap gap-2">
                    <Button r#type="submit" loading=save.pending()>{t("settings-tz-save")}</Button>
                    <Button variant=ButtonVariant::Ghost on:click=move |_| { save.dispatch(None); }>{t("settings-tz-reset")}</Button>
                </div>
            </form>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(slug: &str) -> OidcButtonDto {
        OidcButtonDto {
            slug: slug.into(),
            display_name: slug.to_uppercase(),
            is_default: false,
        }
    }

    #[test]
    fn only_providers_not_linked_yet_offer_link() {
        let linked = vec![LinkedIdentityDto {
            id: "1".into(),
            provider: "A".into(),
            provider_slug: Some("a".into()),
            issuer: "https://a".into(),
            linked_at: "2026-11-07T10:00:00Z".into(),
            can_unlink: true,
        }];
        let left: Vec<String> = linkable(&[button("a"), button("b")], &linked)
            .into_iter()
            .map(|p| p.slug)
            .collect();
        assert_eq!(left, ["b"]);
        assert_eq!(
            link_url("b"),
            "/api/v1/auth/oidc/start?provider=b&mode=link&next=%2Fsettings%2Faccount"
        );
    }
}
