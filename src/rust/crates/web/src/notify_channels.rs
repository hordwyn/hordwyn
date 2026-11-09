//! The Channels tab of Settings > Notifications (design.md 5.8, 6.8): one card per channel
//! with its health from the delivery log and "Send test" (the result inline on the card, never
//! a toast, R-38), and the inline "Add a channel" form: type, name, the type's fields with
//! their constraints as hints and errors on leaving a field, secrets as password fields, then
//! "Add and send a test".

use contracts::{ChannelDto, NotificationPageDto};
use leptos::prelude::*;
use serde_json::{Value, json};
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant,
    CopyField, FormField, Input, Segmented, SegmentedOption, StatusDot, StatusState, Switch,
    field_described_by,
};

use crate::i18n::{t, use_i18n};
use crate::notifications::{NotifyState, notify_error};
use crate::rest;

/// Channel types the form offers (web push needs the browser and is set up there).
pub const KINDS: [&str; 7] = [
    "ntfy", "gotify", "discord", "matrix", "email", "webhook", "apprise",
];

/// What a field holds, for its check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Url,
    Text,
    Email,
    /// ntfy topics: letters, digits, `-` and `_`.
    Topic,
}

/// One field of a channel type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    /// Key in the channel config.
    pub key: &'static str,
    /// Label message.
    pub label: &'static str,
    /// Hint message (the constraint, said before input).
    pub hint: Option<&'static str>,
    pub shape: Shape,
    /// Shown as a password field: stored encrypted, never shown again.
    pub secret: bool,
    pub optional: bool,
}

const fn field(key: &'static str, label: &'static str, shape: Shape) -> FieldSpec {
    FieldSpec {
        key,
        label,
        hint: None,
        shape,
        secret: false,
        optional: false,
    }
}

impl FieldSpec {
    const fn hint(mut self, hint: &'static str) -> Self {
        self.hint = Some(hint);
        self
    }
    const fn secret(mut self) -> Self {
        self.secret = true;
        self
    }
    const fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

/// The fields of a channel type (at most three).
#[must_use]
pub fn fields(kind: &str) -> Vec<FieldSpec> {
    let server = field("server", "notify-f-server", Shape::Url).hint("notify-hint-url");
    match kind {
        "ntfy" => vec![
            server,
            field("topic", "notify-f-topic", Shape::Topic).hint("notify-hint-topic"),
            field("token", "notify-f-token-optional", Shape::Text)
                .secret()
                .optional(),
        ],
        "gotify" => vec![
            server,
            field("token", "notify-f-app-token", Shape::Text).secret(),
        ],
        "discord" => vec![
            field("webhook_url", "notify-f-webhook-url", Shape::Url)
                .hint("notify-hint-discord")
                .secret(),
        ],
        "matrix" => vec![
            field("homeserver", "notify-f-homeserver", Shape::Url).hint("notify-hint-url"),
            field("room_id", "notify-f-room", Shape::Text).hint("notify-hint-room"),
            field("access_token", "notify-f-access-token", Shape::Text).secret(),
        ],
        "apprise" => vec![
            server,
            field("urls", "notify-f-apprise-urls", Shape::Text).secret(),
        ],
        "webhook" => vec![
            field("url", "notify-f-url", Shape::Url).hint("notify-hint-webhook"),
            field("secret", "notify-f-secret-optional", Shape::Text)
                .secret()
                .optional(),
        ],
        _ => vec![field("to", "notify-f-email", Shape::Email)],
    }
}

/// The problem with a value, as a message key (`None`: fine).
#[must_use]
pub fn check(spec: &FieldSpec, value: &str) -> Option<&'static str> {
    let v = value.trim();
    if v.is_empty() {
        return (!spec.optional).then_some("notify-check-required");
    }
    match spec.shape {
        Shape::Url if !(v.starts_with("https://") || v.starts_with("http://")) => {
            Some("notify-check-url")
        }
        Shape::Email if !v.contains('@') => Some("notify-check-email"),
        Shape::Topic
            if v.len() > 64
                || !v
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') =>
        {
            Some("notify-check-topic")
        }
        _ => None,
    }
}

/// The channel config from the form values (in the order of [`fields`]).
#[must_use]
pub fn config(kind: &str, values: &[String]) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("kind".into(), kind.into());
    for (spec, v) in fields(kind).iter().zip(values) {
        let v = v.trim();
        let value = if v.is_empty() && spec.optional && spec.key == "token" {
            Value::Null
        } else {
            v.into()
        };
        m.insert(spec.key.into(), value);
    }
    Value::Object(m)
}

/// The non-secret part of a channel's config in one line (`ntfy.sh / topic`).
fn summary(c: &ChannelDto) -> String {
    let s = |k: &str| c.config[k].as_str().unwrap_or_default().to_string();
    match c.kind.as_str() {
        "ntfy" => format!("{} / {}", s("server"), s("topic")),
        "gotify" | "apprise" => s("server"),
        "matrix" => format!("{} / {}", s("homeserver"), s("room_id")),
        "webhook" => s("url"),
        "email" => s("to"),
        _ => String::new(),
    }
}

/// Health of a channel as a dot and words.
fn health(c: &ChannelDto) -> (StatusState, String) {
    let i18n = use_i18n();
    let when = || {
        c.health
            .at
            .as_deref()
            .map(|a| i18n.datetime(a))
            .unwrap_or_default()
    };
    match c.health.state.as_str() {
        "working" => (
            StatusState::Ok,
            i18n.t_args("notify-health-working", &[("when", when().into())]),
        ),
        "failing" => (
            StatusState::Bad,
            i18n.t_args("notify-health-failing", &[("when", when().into())]),
        ),
        "waiting" => (StatusState::Warn, i18n.t("notify-health-waiting")),
        _ => (StatusState::Off, i18n.t("notify-health-never")),
    }
}

/// Send a test to a channel and keep the result for its card.
async fn run_test(state: NotifyState, id: String) {
    let r = rest::call_dto::<Value>("POST", &format!("/notifications/channels/{id}/test"), None)
        .await
        .map(|_| ())
        .map_err(|e| {
            if e.message.is_empty() {
                notify_error(&e.code)
            } else {
                e.message
            }
        });
    state.tests.update(|m| {
        m.insert(id, r);
    });
}

#[component]
pub fn Channels(p: NotificationPageDto, state: NotifyState) -> impl IntoView {
    let test = Action::new(move |id: &String| run_test(state, id.clone()));
    let remove = Action::new(move |id: &String| {
        let path = format!("/notifications/channels/{id}");
        async move {
            let _ = rest::send("DELETE", &path, None).await;
            state.data.refetch();
        }
    });
    view! {
        <div class="mt-4 grid max-w-3xl gap-4 pb-[var(--tab-bar-height)]">
            <p class="text-sm text-muted-foreground">{t("notify-channels-lead")}</p>
            {(!p.channels.is_empty()).then(|| view! {
                <ul class="grid gap-3" aria-label=t("notify-channels")>
                    {p.channels.into_iter().map(|c| view! { <ChannelCard c=c state=state test=test remove=remove/> }).collect_view()}
                </ul>
            })}
            <AddChannel state=state test=test/>
        </div>
    }
}

#[component]
fn ChannelCard(
    c: ChannelDto,
    state: NotifyState,
    test: Action<String, ()>,
    remove: Action<String, ()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let (dot, words) = health(&c);
    let id = StoredValue::new(c.id.clone());
    let name = c.name.clone();
    let confirm = RwSignal::new(false);
    let editing = RwSignal::new(false);
    let edit_c = c.clone();
    let testing = Signal::derive(move || {
        test.pending().get() && test.input().get().as_deref() == Some(id.get_value().as_str())
    });
    let result = move || state.tests.with(|m| m.get(&id.get_value()).cloned());
    let secret = move || {
        state
            .secret
            .get()
            .filter(|(cid, _)| *cid == id.get_value())
            .map(|(_, s)| s)
    };
    let failing = (c.health.state == "failing").then(|| c.health.error.clone().unwrap_or_default());
    let card_id = format!("nf-ch-{}", c.id);
    view! {
        <li class="grid gap-3 rounded-xl border border-border bg-card p-4 shadow-elevation-1" data-testid=card_id.clone()>
            <div class="flex flex-wrap items-start justify-between gap-2">
                <div class="min-w-0">
                    <h3 class="font-semibold">{c.name.clone()}</h3>
                    <p class="break-words text-sm text-muted-foreground">
                        {t(&format!("notify-kind-{}", c.kind))}
                        {let s = summary(&c); (!s.is_empty()).then(|| format!(" - {s}"))}
                    </p>
                </div>
                <div class="flex items-center gap-2">
                    {(!c.enabled).then(|| view! { <Badge variant=BadgeVariant::Destructive>{t("notify-disabled")}</Badge> })}
                    <StatusDot state=dot label=words/>
                </div>
            </div>
            {failing.map(|e| view! {
                <Alert variant=AlertVariant::Destructive role=AlertRole::None title=t("notify-failing-title")>
                    <span lang="en" class="break-words">{e}</span>
                    " "{t("notify-failing-hint")}
                </Alert>
            })}
            {move || secret().map(|s| view! {
                <div class="grid gap-1">
                    <label class="text-sm font-medium" for="nf-secret">{t("notify-secret-label")}</label>
                    <CopyField id="nf-secret" value=Signal::derive(move || s.clone())
                        copy_label=t("settings-copy") copied_label=t("settings-copied") shown_once=t("notify-secret-once")/>
                </div>
            })}
            {move || result().map(|r| match r {
                Ok(()) => view! { <Alert variant=AlertVariant::Success>{t("notify-test-delivered")}</Alert> }.into_any(),
                Err(e) => view! {
                    <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=t("notify-test-failed")>
                        <span lang="en" class="break-words">{e}</span>
                    </Alert>
                }.into_any(),
            })}
            <div class="flex flex-wrap gap-2">
                <Button size=ButtonSize::Sm variant=ButtonVariant::Outline loading=testing
                    attr:aria-label=i18n.t_args("notify-test-named", &[("name", name.clone().into())])
                    on:click=move |_| { test.dispatch(id.get_value()); }>{t("notify-test")}</Button>
                <Button size=ButtonSize::Sm variant=ButtonVariant::Outline
                    attr:aria-label=i18n.t_args("notify-edit-named", &[("name", name.clone().into())])
                    attr:aria-expanded=move || if editing.get() { "true" } else { "false" }
                    on:click=move |_| editing.update(|e| *e = !*e)>{t("notify-edit")}</Button>
                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost
                    attr:aria-label=i18n.t_args("notify-delete-named", &[("name", name.clone().into())])
                    on:click=move |_| confirm.set(true)>{t("notify-delete")}</Button>
            </div>
            <Show when=move || editing.get()>
                <EditChannel c=edit_c.clone() state=state editing=editing/>
            </Show>
            <crate::admin::ConfirmDialog open=confirm
                title=i18n.t_args("notify-delete-title", &[("name", name.clone().into())])
                body=t("notify-delete-body") confirm=t("notify-delete")
                on_confirm=Callback::new(move |()| { remove.dispatch(id.get_value()); })/>
        </li>
    }
}

/// The config of an edited channel (design.md 5.8, DEC-344): the kind, the fields as typed,
/// and for a secret left blank the stored value's placeholder, which the server keeps
/// (secrets are write-only: the form never shows them).
#[must_use]
pub fn edited_config(c: &ChannelDto, values: &[String]) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("kind".into(), c.kind.clone().into());
    for (spec, v) in fields(&c.kind).iter().zip(values) {
        let v = v.trim();
        let value = if spec.secret && v.is_empty() {
            c.config[spec.key].clone()
        } else if v.is_empty() && spec.optional && spec.key == "token" {
            Value::Null
        } else {
            v.into()
        };
        m.insert(spec.key.into(), value);
    }
    Value::Object(m)
}

/// The problem with an edited value: a blank secret keeps the stored one.
fn check_edit(spec: &FieldSpec, value: &str) -> Option<&'static str> {
    if spec.secret && value.trim().is_empty() {
        None
    } else {
        check(spec, value)
    }
}

/// Edit a channel in place: its name, its non-secret fields (filled in), its secrets (blank
/// keeps them) and whether it is on; `PUT /notifications/channels/{id}`.
#[component]
fn EditChannel(c: ChannelDto, state: NotifyState, editing: RwSignal<bool>) -> impl IntoView {
    let specs = fields(&c.kind);
    let name = RwSignal::new(c.name.clone());
    let enabled = RwSignal::new(c.enabled);
    let values: Vec<RwSignal<String>> = specs
        .iter()
        .map(|s| {
            let shown = if s.secret {
                ""
            } else {
                c.config[s.key].as_str().unwrap_or_default()
            };
            RwSignal::new(shown.to_string())
        })
        .collect();
    let errors: Vec<RwSignal<Option<&'static str>>> =
        (0..=specs.len()).map(|_| RwSignal::new(None)).collect();
    let failed = RwSignal::new(None::<String>);
    let (vals, errs, sp) = (values.clone(), errors.clone(), specs.clone());
    let check_all = move || {
        errs[0].set(
            name.get_untracked()
                .trim()
                .is_empty()
                .then_some("notify-check-required"),
        );
        for (i, spec) in sp.iter().enumerate() {
            errs[i + 1].set(check_edit(spec, &vals[i].get_untracked()));
        }
        errs.iter().all(|e| e.get_untracked().is_none())
    };
    let (channel, vals) = (c.clone(), values.clone());
    let save = Action::new(move |(): &()| {
        let typed: Vec<String> = vals.iter().map(|v| v.get_untracked()).collect();
        let body = json!({
            "name": name.get_untracked(),
            "config": edited_config(&channel, &typed),
            "enabled": enabled.get_untracked(),
        });
        let path = format!("/notifications/channels/{}", channel.id);
        async move {
            match rest::call_dto::<ChannelDto>("PUT", &path, Some(body)).await {
                Ok(_) => {
                    editing.set(false);
                    state.data.refetch();
                }
                Err(e) => failed.set(Some(notify_error(&e.code))),
            }
        }
    });
    let prefix = format!("nf-e-{}", c.id);
    let name_id = format!("{prefix}-name");
    let on_id = format!("{prefix}-on");
    let name_error = errors[0];
    view! {
        <form class="grid max-w-md gap-4" novalidate=true on:submit=move |ev| {
            ev.prevent_default();
            if check_all() { save.dispatch(()); }
        }>
            <FormField id=name_id.clone() label=t("notify-channel-name")
                error=Signal::derive(move || name_error.get().map(t))>
                <Input id=name_id.clone() value=name described_by=field_described_by(&name_id)
                    invalid=Signal::derive(move || name_error.get().is_some())/>
            </FormField>
            {specs.into_iter().enumerate().map(|(i, spec)| view! {
                <ChannelField spec=spec value=values[i] error=errors[i + 1] id_prefix=prefix.clone() keep=true/>
            }).collect_view()}
            <div class="flex items-center gap-3">
                <Switch id=on_id.clone() checked=enabled/>
                <label for=on_id class="text-sm font-medium">{t("notify-channel-enabled")}</label>
            </div>
            {move || failed.get().map(|m| view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{m}</Alert> })}
            <div class="flex flex-wrap gap-2">
                <Button r#type="submit" loading=save.pending()>{t("notify-channel-save")}</Button>
                <Button variant=ButtonVariant::Ghost on:click=move |_| editing.set(false)>{t("notify-channel-cancel")}</Button>
            </div>
        </form>
    }
}

#[component]
fn AddChannel(state: NotifyState, test: Action<String, ()>) -> impl IntoView {
    let kind = RwSignal::new("ntfy".to_string());
    let name = RwSignal::new(String::new());
    let values: [RwSignal<String>; 3] = std::array::from_fn(|_| RwSignal::new(String::new()));
    let errors: [RwSignal<Option<&'static str>>; 4] = std::array::from_fn(|_| RwSignal::new(None));
    let failed = RwSignal::new(None::<String>);
    // a new type starts with empty fields and no errors
    Effect::new(move |_| {
        kind.track();
        for v in values {
            v.set(String::new());
        }
        for e in errors.iter().skip(1) {
            e.set(None);
        }
    });
    let check_all = move || {
        let k = kind.get_untracked();
        errors[0].set(
            name.get_untracked()
                .trim()
                .is_empty()
                .then_some("notify-check-required"),
        );
        for (i, spec) in fields(&k).iter().enumerate() {
            errors[i + 1].set(check(spec, &values[i].get_untracked()));
        }
        errors.iter().all(|e| e.get_untracked().is_none())
    };
    let add = Action::new(move |(): &()| {
        let k = kind.get_untracked();
        let body = json!({
            "name": name.get_untracked(),
            "config": config(&k, &values.map(|v| v.get_untracked())),
            "enabled": true,
        });
        async move {
            match rest::call_dto::<ChannelDto>("POST", "/notifications/channels", Some(body)).await
            {
                Ok(Some(c)) => {
                    failed.set(None);
                    name.set(String::new());
                    for v in values {
                        v.set(String::new());
                    }
                    if let Some(s) = c.secret.clone() {
                        state.secret.set(Some((c.id.clone(), s)));
                    }
                    run_test(state, c.id.clone()).await;
                    state.data.refetch();
                }
                Ok(None) => failed.set(Some(t("error-generic"))),
                Err(e) => failed.set(Some(notify_error(&e.code))),
            }
        }
    });
    let busy = Signal::derive(move || add.pending().get() || test.pending().get());
    let kinds = kind_options();
    view! {
        <section class="grid gap-4 rounded-xl border border-border p-4" aria-labelledby="nf-add">
            <h3 id="nf-add" class="font-semibold">{t("notify-new-channel")}</h3>
            <div class="grid gap-2">
                <span class="text-sm font-medium" aria-hidden="true">{t("notify-kind")}</span>
                <Segmented id="nf-kind" label=t("notify-kind") value=kind options=kinds class="flex-wrap"/>
            </div>
            <form class="grid max-w-md gap-4" novalidate=true on:submit=move |ev| {
                ev.prevent_default();
                if check_all() { add.dispatch(()); }
            }>
                <div on:focusout=move |_| errors[0].set(name.get_untracked().trim().is_empty().then_some("notify-check-required"))>
                    <FormField id="nf-name" label=t("notify-channel-name") description=t("notify-hint-name")
                        error=Signal::derive(move || errors[0].get().map(t))>
                        <Input id="nf-name" value=name described_by=field_described_by("nf-name")
                            invalid=Signal::derive(move || errors[0].get().is_some())/>
                    </FormField>
                </div>
                {move || fields(&kind.get()).into_iter().enumerate().map(|(i, spec)| view! {
                    <ChannelField spec=spec value=values[i] error=errors[i + 1]/>
                }).collect_view()}
                {move || failed.get().map(|m| view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{m}</Alert> })}
                <Button r#type="submit" class="w-fit" loading=busy>{t("notify-add-and-test")}</Button>
            </form>
        </section>
    }
}

/// The channel types as segmented options.
fn kind_options() -> Vec<SegmentedOption> {
    KINDS
        .iter()
        .map(|k| SegmentedOption {
            value: (*k).into(),
            label: t(&format!("notify-kind-{k}")),
            icon: None,
            controls: None,
            lang: None,
        })
        .collect()
}

/// One field of a new channel, checked when it loses the focus.
#[component]
fn ChannelField(
    spec: FieldSpec,
    value: RwSignal<String>,
    error: RwSignal<Option<&'static str>>,
    /// Element id prefix (default `nf-f`; each edit form has its own).
    #[prop(into, optional)]
    id_prefix: Option<String>,
    /// Editing a stored channel: a blank secret keeps the stored one.
    #[prop(optional)]
    keep: bool,
) -> impl IntoView {
    let id = format!(
        "{}-{}",
        id_prefix.unwrap_or_else(|| "nf-f".into()),
        spec.key
    );
    let secret_hint = if keep {
        "notify-hint-secret-keep"
    } else {
        "notify-hint-secret"
    };
    let hint = match (spec.hint, spec.secret) {
        (Some(h), true) => Some(format!("{} {}", t(h), t(secret_hint))),
        (Some(h), false) => Some(t(h)),
        (None, true) => Some(t(secret_hint)),
        (None, false) => None,
    };
    let (secret, shape, label) = (spec.secret, spec.shape, spec.label);
    view! {
        <div on:focusout=move |_| error.set(if keep { check_edit(&spec, &value.get_untracked()) } else { check(&spec, &value.get_untracked()) })>
            <FormField id=id.clone() label=t(label) description=hint
                error=Signal::derive(move || error.get().map(t))>
                <Input id=id.clone() value=value described_by=field_described_by(&id)
                    r#type=if secret { "password" } else { "text" }
                    autocomplete=if secret { "new-password" } else { "off" }
                    inputmode=match shape { Shape::Url => "url", Shape::Email => "email", _ => "text" }
                    invalid=Signal::derive(move || error.get().is_some())/>
            </FormField>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_checked_before_sending() {
        let ntfy = fields("ntfy");
        assert_eq!(ntfy.len(), 3);
        assert_eq!(check(&ntfy[0], "ntfy.sh"), Some("notify-check-url"));
        assert_eq!(check(&ntfy[0], "https://ntfy.sh"), None);
        assert_eq!(check(&ntfy[1], "my topic"), Some("notify-check-topic"));
        assert_eq!(check(&ntfy[1], "my-topic"), None);
        assert_eq!(check(&ntfy[2], ""), None, "the token is optional");
        assert_eq!(
            check(&fields("email")[0], "nobody"),
            Some("notify-check-email")
        );
        assert!(fields("gotify")[1].secret);
        assert!(KINDS.iter().all(|k| fields(k).len() <= 3));
    }

    #[test]
    fn the_config_carries_the_kind_and_drops_an_empty_optional_token() {
        let c = config(
            "ntfy",
            &["https://ntfy.sh".into(), " t ".into(), String::new()],
        );
        assert_eq!(
            c,
            json!({"kind": "ntfy", "server": "https://ntfy.sh", "topic": "t", "token": null})
        );
        let w = config("webhook", &["https://h.example.org".into(), String::new()]);
        assert_eq!(w["secret"], "", "an empty secret asks the server for one");
    }
    #[test]
    fn an_edit_keeps_blank_secrets_and_sends_typed_ones() {
        let ch = |config: Value| ChannelDto {
            id: "c".into(),
            name: "Phone".into(),
            kind: "ntfy".into(),
            enabled: true,
            config,
            secret: None,
            health: contracts::ChannelHealthDto::default(),
        };
        let c = ch(
            json!({"kind": "ntfy", "server": "https://ntfy.sh", "topic": "a", "token": "********"}),
        );
        assert_eq!(
            edited_config(&c, &["https://ntfy.sh".into(), "b".into(), String::new()]),
            json!({"kind": "ntfy", "server": "https://ntfy.sh", "topic": "b", "token": "********"})
        );
        assert_eq!(
            edited_config(&c, &["https://ntfy.sh".into(), "b".into(), " new ".into()])["token"],
            "new"
        );
        // no stored token: a blank one stays absent
        let none =
            ch(json!({"kind": "ntfy", "server": "https://ntfy.sh", "topic": "a", "token": null}));
        assert_eq!(
            edited_config(
                &none,
                &["https://ntfy.sh".into(), "a".into(), String::new()]
            )["token"],
            Value::Null
        );
        let spec = fields("webhook")[1];
        assert_eq!(
            check_edit(&spec, ""),
            None,
            "a blank secret keeps the stored one"
        );
        assert_eq!(
            check_edit(&fields("webhook")[0], ""),
            Some("notify-check-required")
        );
    }
}
