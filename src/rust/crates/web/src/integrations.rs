//! Settings > Integrations (design.md 6.9, 5.9, R-38; mockup `integrations.html`): senders
//! (media servers, scripts) post what was watched to the watched webhook with their own token.
//! Each sender is a card with its health from the last call and what it last recorded; a
//! refused call is explained on the card with the field and the body it sent (secrets
//! redacted), never in a toast. "Connect a sender" is a three-step panel that listens for the
//! sender's first call over SSE (`integration.call`). Reads come from server functions,
//! changes go to the REST routes (`web::rest`, DEC-327).

use contracts::{IntegrationCreatedDto, IntegrationDto, IntegrationTestDto};
use leptos::prelude::*;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant, CopyField, DropdownMenu,
    EmptyState, FormField, IconName, Input, MenuItem, StatusDot, StatusState, Steps,
    field_described_by,
};

use crate::api;
use crate::events::use_events;
use crate::i18n::{t, use_i18n};
use crate::pages::PageTitle;
use crate::rest;
use crate::settings::{Lead, kept_menu, loaded};

/// The sample the panel's "Send a test call from here" posts, also shown as the body shape.
const SAMPLE: &str = r#"{"kind": "movie", "title": "Heat", "year": 1995,
 "ids": {"tmdb": "949", "imdb": "tt0113277"},
 "watched_at": "2026-01-01T21:00:00Z", "source": "Jellyfin"}"#;

/// The episode body shape shown beside the token (design.md 5.9).
const EPISODE_SHAPE: &str = r#"{
  "kind": "episode",
  "series": { "title": "...", "ids": { "tmdb": "..." } },
  "season": 2, "episode": 5,
  "watched_at": "...", "source": "..."
}"#;

/// The fields the webhook knows (refusals list them).
const FIELDS: &str = "kind, title, year, ids, series, season, episode, watched_at, source, id";

/// Seconds without a call after which the panel suggests checking the token (DEC, plan WP3-5).
pub const CHECK_TOKEN_AFTER_SECS: u32 = 60;

/// One `integration.call` event as the page reads it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Call {
    pub integration_id: String,
    pub status: u16,
    #[serde(default)]
    pub field: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub test: bool,
    #[serde(default)]
    pub title: Option<String>,
}

/// The call in an SSE event's data (the whole `AppEvent`).
#[must_use]
pub fn call_of(data: &str) -> Option<Call> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    serde_json::from_value(v.get("payload")?.clone()).ok()
}

/// What the "Wait for the first call" step shows.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Waiting {
    Listening,
    /// Nothing came in [`CHECK_TOKEN_AFTER_SECS`].
    Quiet,
    Accepted(String),
    Refused {
        field: Option<String>,
        error: String,
    },
}

#[component]
pub fn Integrations() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::integrations());
    let events = use_events();
    let panel = RwSignal::new(false);
    let created = RwSignal::new(None::<IntegrationCreatedDto>);
    let waiting = RwSignal::new(Waiting::Listening);
    Effect::new(move |_| {
        let Some((topic, payload)) = events.0.get() else {
            return;
        };
        if topic != "integration.call" {
            return;
        }
        data.refetch();
        let Some(call) = call_of(&payload) else {
            return;
        };
        let mine = created
            .get_untracked()
            .is_some_and(|c| c.integration.id == call.integration_id);
        if !mine || call.test {
            return;
        }
        let now = waiting.get_untracked();
        match (call.status, now) {
            (_, Waiting::Accepted(_)) => {}
            (200, _) => waiting.set(Waiting::Accepted(call.title.unwrap_or_default())),
            _ => waiting.set(Waiting::Refused {
                field: call.field,
                error: call.error.unwrap_or_default(),
            }),
        }
    });
    crate::events::refresh_when_lagged(Callback::new(move |()| data.refetch()));
    let open = move || {
        created.set(None);
        waiting.set(Waiting::Listening);
        panel.set(true);
    };
    view! {
        <PageTitle text=t("integrations-title")/>
        <Lead text=t("integrations-lead")/>
        <div class="grid max-w-3xl gap-4 pb-[var(--tab-bar-height)]">
            <Show when=move || !panel.get()>
                <Button class="w-fit" on:click=move |_| open()>{t("integrations-connect")}</Button>
            </Show>
            <Show when=move || panel.get()>
                <ConnectPanel panel=panel created=created waiting=waiting data=data/>
            </Show>
            {loaded(data, move |list: Vec<IntegrationDto>| {
                if list.is_empty() {
                    return view! {
                        <EmptyState title=t("integrations-empty") icon=IconName::Link level=2
                            body=t("integrations-empty-body")/>
                    }.into_any();
                }
                view! {
                    <ul class="grid gap-3" aria-label=t("integrations-senders")>
                        {list.into_iter().map(|i| view! { <SenderCard i=i data=data/> }).collect_view()}
                    </ul>
                }.into_any()
            })}
        </div>
    }
}

#[component]
fn SenderCard(
    i: IntegrationDto,
    data: Resource<Result<Vec<IntegrationDto>, ServerFnError>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let confirm = RwSignal::new(false);
    let id = StoredValue::new(i.id.clone());
    let revoke = Action::new(move |(): &()| {
        let path = format!("/integrations/{}", id.get_value());
        async move {
            let _ = rest::send("DELETE", &path, None).await;
            data.refetch();
        }
    });
    let when = i
        .last_seen_at
        .as_deref()
        .map(|s| i18n.datetime(s))
        .unwrap_or_default();
    let (dot, words) = match i.last_status {
        None => (StatusState::Off, t("integrations-never-called")),
        Some(200) => (StatusState::Ok, t("integrations-working")),
        Some(_) => (StatusState::Bad, t("integrations-refused")),
    };
    let detail = match (i.last_status, &i.last_item) {
        (None, _) => None,
        (Some(200), Some(item)) => Some(i18n.t_args(
            "integrations-last-recorded",
            &[
                ("when", when.clone().into()),
                ("title", item.title.clone().into()),
            ],
        )),
        (Some(status), _) => Some(i18n.t_args(
            "integrations-last-call-status",
            &[("when", when.clone().into()), ("status", status.into())],
        )),
    };
    let created = i18n.t_args(
        "integrations-created-on",
        &[("when", i18n.date(&i.created_at).into())],
    );
    let refused = i.last_status.is_some_and(|s| s != 200);
    let menu = vec![MenuItem {
        label: t("integrations-revoke"),
        on_select: Callback::new(move |()| confirm.set(true)),
        destructive: true,
    }];
    let menu_label = i18n.t_args(
        "integrations-menu-named",
        &[("name", i.name.clone().into())],
    );
    let kept_open = kept_menu(&format!("ig-menu-{}", i.id));
    view! {
        <li class="grid gap-3 rounded-xl border border-border bg-card p-4 shadow-elevation-1" data-testid=format!("ig-card-{}", i.id)>
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="min-w-0">
                    <h3 class="font-semibold">{i.name.clone()}</h3>
                    <p class="text-sm text-muted-foreground">{created}</p>
                </div>
                <div class="flex items-center gap-2">
                    <StatusDot state=dot label=words/>
                    <DropdownMenu label=menu_label id=format!("ig-menu-{}", i.id) items=menu
                        open=kept_open.open active=kept_open.active active_label=kept_open.active_label/>
                </div>
            </div>
            {detail.map(|d| view! { <p class="break-words text-sm">{d}</p> })}
            {refused.then(|| view! { <Refusal i=i.clone()/> })}
            {i.last_item.clone().filter(|_| refused).map(|item| view! {
                <p class="text-sm text-muted-foreground">
                    {use_i18n().t_args("integrations-earlier-recorded", &[("title", item.title.into())])}
                </p>
            })}
            <crate::admin::ConfirmDialog open=confirm
                title=i18n.t_args("integrations-revoke-title", &[("name", i.name.clone().into())])
                body=t("integrations-revoke-body") confirm=t("integrations-revoke")
                on_confirm=Callback::new(move |()| { revoke.dispatch(()); })/>
        </li>
    }
}

/// The persistent explanation of a refused call (R-38): the field, the reason, what the
/// webhook expects, and the body it got (redacted).
#[component]
fn Refusal(i: IntegrationDto) -> impl IntoView {
    let i18n = use_i18n();
    let field = i.last_error_field.clone();
    let lead = match &field {
        Some(f) => i18n.t_args(
            "integrations-refusal-field",
            &[("name", i.name.clone().into()), ("field", f.clone().into())],
        ),
        None => i18n.t_args("integrations-refusal", &[("name", i.name.clone().into())]),
    };
    view! {
        <Alert variant=AlertVariant::Destructive role=AlertRole::None title=t("integrations-refused-title")>
            <p>{lead}</p>
            // the server's reason, as it said it (not translated)
            {i.last_error.clone().map(|e| view! { <p lang="en" class="mt-1 break-words font-mono text-xs">{e}</p> })}
            <p class="mt-1">{i18n.t_args("integrations-refusal-expected", &[("fields", FIELDS.into())])}</p>
            {i.last_payload.clone().map(|body| view! {
                <details class="mt-2">
                    <summary class="cursor-pointer">{t("integrations-show-payload")}</summary>
                    <pre class="mt-1 max-h-64 overflow-auto whitespace-pre-wrap break-all rounded bg-muted p-2 font-mono text-xs text-foreground" tabindex="0">{body}</pre>
                </details>
            })}
        </Alert>
    }
}

#[component]
fn ConnectPanel(
    panel: RwSignal<bool>,
    created: RwSignal<Option<IntegrationCreatedDto>>,
    waiting: RwSignal<Waiting>,
    data: Resource<Result<Vec<IntegrationDto>, ServerFnError>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let name = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let create = Action::new(move |(): &()| {
        let body = serde_json::json!({"name": name.get_untracked()});
        async move {
            match rest::call_dto::<IntegrationCreatedDto>("POST", "/integrations", Some(body)).await
            {
                Ok(Some(c)) => {
                    error.set(None);
                    created.set(Some(c));
                    waiting.set(Waiting::Listening);
                    start_quiet_timer(created, waiting);
                    data.refetch();
                }
                Ok(None) => error.set(Some(t("error-generic"))),
                Err(e) => error.set(Some(create_error(&use_i18n(), &e.code))),
            }
        }
    });
    let step = Signal::derive(move || match (created.get().is_some(), waiting.get()) {
        (false, _) => 0,
        (true, Waiting::Accepted(_)) => 3,
        (true, _) => 2,
    });
    let steps = Signal::derive(|| {
        vec![
            t("integrations-step-name"),
            t("integrations-step-copy"),
            t("integrations-step-wait"),
        ]
    });
    view! {
        <section class="grid gap-4 rounded-xl border border-primary p-4" aria-labelledby="ig-connect">
            <h2 id="ig-connect" class="text-lg font-semibold">{t("integrations-connect")}</h2>
            <p class="-mt-3 text-sm text-muted-foreground">{t("integrations-connect-lead")}</p>
            <Steps label=t("integrations-steps") steps=steps current=step done_label=i18n.t("integrations-step-done")/>
            {move || match created.get() {
                None => view! {
                    <form class="grid max-w-md gap-3" on:submit=move |ev| { ev.prevent_default(); create.dispatch(()); }>
                        <FormField id="ig-name" label=t("integrations-name") description=t("integrations-name-hint")
                            error=Signal::derive(move || error.get())>
                            <Input id="ig-name" value=name described_by=field_described_by("ig-name")
                                invalid=Signal::derive(move || error.get().is_some())/>
                        </FormField>
                        <div class="flex flex-wrap gap-2">
                            <Button r#type="submit" loading=create.pending()>{t("integrations-create")}</Button>
                            <Button variant=ButtonVariant::Ghost on:click=move |_| panel.set(false)>{t("integrations-cancel")}</Button>
                        </div>
                    </form>
                }.into_any(),
                Some(c) => view! { <TokenAndWait c=c waiting=waiting panel=panel/> }.into_any(),
            }}
        </section>
    }
}

/// After [`CHECK_TOKEN_AFTER_SECS`] without a call, say so (browser only).
fn start_quiet_timer(created: RwSignal<Option<IntegrationCreatedDto>>, waiting: RwSignal<Waiting>) {
    let id = created.get_untracked().map(|c| c.integration.id);
    set_timeout(
        move || {
            let same = created.get_untracked().map(|c| c.integration.id) == id;
            if same && waiting.get_untracked() == Waiting::Listening {
                waiting.set(Waiting::Quiet);
            }
        },
        std::time::Duration::from_secs(u64::from(CHECK_TOKEN_AFTER_SECS)),
    );
}

#[component]
fn TokenAndWait(
    c: IntegrationCreatedDto,
    waiting: RwSignal<Waiting>,
    panel: RwSignal<bool>,
) -> impl IntoView {
    let i18n = use_i18n();
    let token = c.token.clone();
    let url = c.webhook_url.clone();
    let id = c.integration.id.clone();
    let tested = RwSignal::new(None::<Result<IntegrationTestDto, String>>);
    let test = Action::new(move |(): &()| {
        let path = format!("/integrations/{id}/test");
        let body = serde_json::from_str::<serde_json::Value>(SAMPLE).ok();
        async move {
            let r = rest::call_dto::<IntegrationTestDto>("POST", &path, body).await;
            tested.set(Some(match r {
                Ok(Some(t)) => Ok(t),
                Ok(None) => Err(String::new()),
                Err(e) => Err(e.message),
            }));
        }
    });
    let curl = format!(
        "curl -X POST {url} -H 'Authorization: Bearer {token}' -H 'Content-Type: application/json' -d '{}'",
        SAMPLE.replace('\n', "")
    );
    view! {
        <div class="grid gap-4 min-[47.5rem]:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <div class="grid content-start gap-3">
                <div class="grid gap-1">
                    <label class="text-sm font-medium" for="ig-token">{i18n.t_args("integrations-token-for", &[("name", c.integration.name.clone().into())])}</label>
                    <CopyField id="ig-token" value=Signal::derive(move || token.clone()) copy_label=t("settings-copy")
                        copied_label=t("settings-copied") shown_once=t("integrations-token-once")/>
                </div>
                <div class="grid gap-1">
                    <label class="text-sm font-medium" for="ig-url">{t("integrations-url")}</label>
                    <CopyField id="ig-url" value=Signal::derive(move || url.clone()) copy_label=t("settings-copy")
                        copied_label=t("settings-copied")/>
                </div>
            </div>
            <div class="grid content-start gap-2 text-sm">
                <h3 class="font-semibold">{t("integrations-help-title")}</h3>
                <ol class="list-decimal space-y-1 pl-5">
                    <li>{t("integrations-help-1")}</li>
                    <li>{t("integrations-help-2")}</li>
                    <li>{t("integrations-help-3")}</li>
                </ol>
                <pre class="overflow-x-auto rounded bg-muted p-2 font-mono text-xs" tabindex="0" aria-label=t("integrations-shape")>{EPISODE_SHAPE}</pre>
                <details>
                    <summary class="cursor-pointer">{t("integrations-example")}</summary>
                    <pre class="mt-1 overflow-x-auto whitespace-pre-wrap break-all rounded bg-muted p-2 font-mono text-xs" tabindex="0">{curl}</pre>
                </details>
            </div>
        </div>
        <WaitBox waiting=waiting/>
        <div class="flex flex-wrap items-center gap-2">
            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm loading=test.pending()
                on:click=move |_| { test.dispatch(()); }>{t("integrations-send-test")}</Button>
            <Button size=ButtonSize::Sm on:click=move |_| panel.set(false)>{t("integrations-done")}</Button>
        </div>
        {move || tested.get().map(|r| match r {
            Ok(t) => {
                let i18n = use_i18n();
                let text = match t.matched {
                    Some(m) => i18n.t_args("integrations-test-matched", &[("title", m.title.into())]),
                    None => i18n.t_args("integrations-test-new", &[("title", t.title.into())]),
                };
                view! { <Alert variant=AlertVariant::Success title=i18n.t("integrations-test-ok")>{text}</Alert> }.into_any()
            }
            Err(e) => view! {
                <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=use_i18n().t("integrations-test-refused")>
                    <span lang="en">{e}</span>
                </Alert>
            }.into_any(),
        })}
    }
}

#[component]
fn WaitBox(waiting: RwSignal<Waiting>) -> impl IntoView {
    view! {
        <div role="status" data-testid="ig-wait" class="rounded-md border border-border p-3 text-sm"
            data-state=move || match waiting.get() {
                Waiting::Listening => "listening",
                Waiting::Quiet => "quiet",
                Waiting::Accepted(_) => "accepted",
                Waiting::Refused { .. } => "refused",
            }>
            {move || match waiting.get() {
                Waiting::Listening => view! {
                    <p class="font-medium">{t("integrations-waiting")}</p>
                    <p class="text-muted-foreground">{t("integrations-waiting-body")}</p>
                }.into_any(),
                Waiting::Quiet => view! {
                    <p class="font-medium">{t("integrations-quiet")}</p>
                    <p class="text-muted-foreground">{t("integrations-quiet-body")}</p>
                }.into_any(),
                Waiting::Accepted(title) => view! {
                    <Alert variant=AlertVariant::Success role=AlertRole::None title=t("integrations-accepted")>
                        {use_i18n().t_args("integrations-accepted-body", &[("title", title.into())])}
                    </Alert>
                }.into_any(),
                Waiting::Refused { field, error } => {
                    let i18n = use_i18n();
                    let lead = field.map_or_else(
                        || i18n.t("integrations-wait-refused"),
                        |f| i18n.t_args("integrations-wait-refused-field", &[("field", f.into())]),
                    );
                    view! {
                        <Alert variant=AlertVariant::Destructive role=AlertRole::None title=t("integrations-refused-title")>
                            <p>{lead}</p>
                            <p lang="en" class="mt-1 break-words font-mono text-xs">{error}</p>
                            <p class="mt-1">{i18n.t_args("integrations-refusal-expected", &[("fields", FIELDS.into())])}</p>
                        </Alert>
                    }.into_any()
                }
            }}
        </div>
    }
}

/// The error line of "Create sender", by the server's code (UX3-21): a refused name says
/// what a name may be; anything else says what it is.
fn create_error(i: &crate::i18n::I18n, code: &str) -> String {
    if matches!(code, "invalid" | "bad_request") {
        return i.t("integrations-name-invalid");
    }
    let key = format!("auth-error-{code}");
    let text = i.t(&key);
    if text == key {
        i.t("error-generic")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_are_read_from_the_event_payload() {
        let data = r#"{"topic":"integration.call","entity":null,"audience":[],
            "payload":{"integration_id":"i1","status":422,"field":"season","error":"x","test":false,
            "item_id":null,"title":null,"at":"2026-11-07T10:00:00Z"}}"#;
        let c = call_of(data).unwrap();
        assert_eq!(c.integration_id, "i1");
        assert_eq!(c.status, 422);
        assert_eq!(c.field.as_deref(), Some("season"));
        assert!(!c.test);
        assert_eq!(call_of("not json"), None);
        assert!(serde_json::from_str::<serde_json::Value>(SAMPLE).is_ok());
    }

    /// UX3-21: only a refused name says "name invalid"; a throttle, a lost session or a
    /// server error say what they are.
    #[test]
    fn create_errors_are_mapped_by_code() {
        let i = crate::i18n::I18n::new("en", std::collections::BTreeMap::new());
        let text = |code: &str| create_error(&i, code);
        assert_eq!(text("invalid"), i.t("integrations-name-invalid"));
        assert_eq!(text("bad_request"), i.t("integrations-name-invalid"));
        assert_eq!(text("unauthorized"), i.t("auth-error-unauthorized"));
        assert_eq!(text("internal"), i.t("error-generic"));
    }
}
