//! Notifications (D-17): the inbox, channels, subscriptions (own and, for admins, instance
//! defaults), the delivery log and admin text overrides. Shared by REST and the web app.

use axum::http::StatusCode;
use contracts::{
    ChannelDto, DeliveryDto, EventTypeDto, InboxItemDto, NewChannelDto, SubscriptionDto,
    TemplateOverrideDto,
};
use domain::user::Actor;
use notify::catalogue::{CATALOGUE, EventAudience};
use notify::model::{Channel, ChannelConfig, Delivery, InboxItem, Subscription};
use uuid::Uuid;

use crate::error::ApiResult;
use crate::ops::parse_id;
use crate::{ApiError, ApiState, NotifyApi};

/// Inbox page size.
pub const INBOX_LIMIT: u32 = 100;

fn api(s: &ApiState) -> ApiResult<&NotifyApi> {
    s.notify.as_ref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "notifications are not enabled",
        )
    })
}

#[allow(clippy::needless_pass_by_value)]
fn port(e: app::ports::PortError) -> ApiError {
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", e.0)
}

fn invalid(field: &str, e: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        "invalid",
        format!("{field}: {e}"),
    )
}

fn forbidden() -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, "forbidden", "forbidden")
}

fn not_found(what: &str) -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "not_found",
        format!("{what} not found"),
    )
}

#[must_use]
pub fn catalogue(a: Actor) -> Vec<EventTypeDto> {
    CATALOGUE
        .iter()
        .filter(|e| a.is_admin() || e.audience == EventAudience::User)
        .map(|e| EventTypeDto {
            id: e.id.into(),
            audience: match e.audience {
                EventAudience::User => "user".into(),
                EventAudience::Admin => "admin".into(),
            },
            args: e.args.iter().map(|a| (*a).to_string()).collect(),
        })
        .collect()
}

fn inbox_dto(i: &InboxItem) -> InboxItemDto {
    InboxItemDto {
        id: i.id.to_string(),
        event: i.event.clone(),
        title: i.title.clone(),
        body: i.body.clone(),
        link: i.link.clone(),
        created_at: i.created_at.to_rfc3339(),
        read: i.read_at.is_some(),
    }
}

/// # Errors
/// Notifications disabled, storage errors.
pub async fn inbox(s: &ApiState, a: Actor) -> ApiResult<Vec<InboxItemDto>> {
    let na = api(s)?;
    Ok(na
        .engine
        .store()
        .inbox(a.user_id, INBOX_LIMIT)
        .await
        .map_err(port)?
        .iter()
        .map(inbox_dto)
        .collect())
}

/// # Errors
/// Notifications disabled, invalid id, storage errors.
pub async fn mark_read(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    let na = api(s)?;
    na.engine
        .store()
        .mark_read(a.user_id, parse_id(id, |u| u)?, s.app.clock.now())
        .await
        .map_err(port)
}

fn channel_dto(c: &Channel, secret: Option<String>) -> ChannelDto {
    ChannelDto {
        id: c.id.to_string(),
        name: c.name.clone(),
        kind: c.config.kind().into(),
        enabled: c.enabled,
        config: serde_json::to_value(c.config.redacted()).unwrap_or_default(),
        secret,
    }
}

/// # Errors
/// Notifications disabled, storage errors.
pub async fn channels(s: &ApiState, a: Actor) -> ApiResult<Vec<ChannelDto>> {
    let na = api(s)?;
    Ok(na
        .engine
        .store()
        .channels(a.user_id)
        .await
        .map_err(port)?
        .iter()
        .map(|c| channel_dto(c, None))
        .collect())
}

fn parse_config(v: serde_json::Value) -> ApiResult<ChannelConfig> {
    serde_json::from_value(v).map_err(|e| invalid("config", e))
}

fn check_name(name: &str) -> ApiResult<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 100 {
        return Err(invalid("name", "1 to 100 characters"));
    }
    Ok(n.to_string())
}

/// Create a channel. A webhook without a secret gets a generated one, returned once.
///
/// # Errors
/// Notifications disabled, invalid config.
pub async fn create_channel(s: &ApiState, a: Actor, b: NewChannelDto) -> ApiResult<ChannelDto> {
    let na = api(s)?;
    let mut config = parse_config(b.config)?;
    let mut shown = None;
    if let ChannelConfig::Webhook { secret, .. } = &mut config
        && secret.trim().is_empty()
    {
        *secret = format!("whsec_{}", s.app.ids.new_token());
        shown = Some(secret.clone());
    }
    let channel = Channel {
        id: s.app.ids.new_id(),
        user: a.user_id,
        name: check_name(&b.name)?,
        config,
        enabled: b.enabled,
        created_at: s.app.clock.now(),
    };
    na.engine
        .store()
        .put_channel(&channel)
        .await
        .map_err(port)?;
    Ok(channel_dto(&channel, shown))
}

async fn own_channel(na: &NotifyApi, a: Actor, id: &str) -> ApiResult<Channel> {
    let c = na
        .engine
        .store()
        .channel(parse_id(id, |u| u)?)
        .await
        .map_err(port)?
        .ok_or_else(|| not_found("channel"))?;
    if c.user == a.user_id {
        Ok(c)
    } else {
        Err(not_found("channel"))
    }
}

/// Update a channel; redacted secrets sent back unchanged are kept.
///
/// # Errors
/// Notifications disabled, unknown channel, invalid config.
pub async fn update_channel(
    s: &ApiState,
    a: Actor,
    id: &str,
    b: NewChannelDto,
) -> ApiResult<ChannelDto> {
    let na = api(s)?;
    let mut channel = own_channel(na, a, id).await?;
    channel.config = parse_config(b.config)?.keep_secrets_of(&channel.config);
    channel.name = check_name(&b.name)?;
    channel.enabled = b.enabled;
    na.engine
        .store()
        .put_channel(&channel)
        .await
        .map_err(port)?;
    Ok(channel_dto(&channel, None))
}

/// # Errors
/// Notifications disabled, unknown channel.
pub async fn delete_channel(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    let na = api(s)?;
    let c = own_channel(na, a, id).await?;
    na.engine.store().delete_channel(c.id).await.map_err(port)
}

/// Send a test message now.
///
/// # Errors
/// Notifications disabled, unknown channel, or what the channel answered (`channel_error`).
pub async fn test_channel(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    let na = api(s)?;
    let c = own_channel(na, a, id).await?;
    na.deliverer
        .test(&c)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, "channel_error", e.to_string()))
}

fn sub_dto(sub: &Subscription) -> SubscriptionDto {
    let v = |x: serde_json::Result<serde_json::Value>| x.unwrap_or_default();
    SubscriptionDto {
        id: Some(sub.id.to_string()),
        event: sub.event.clone(),
        filter: v(serde_json::to_value(&sub.filter)),
        targets: v(serde_json::to_value(&sub.targets)),
        mode: v(serde_json::to_value(&sub.mode)),
        quiet_hours: sub.quiet_hours.map(|q| v(serde_json::to_value(q))),
        timezone: sub.timezone.clone(),
        rate_limit_per_hour: sub.rate_limit_per_hour,
        enabled: sub.enabled,
        locked: sub.locked,
        instance_default: sub.user.is_none(),
    }
}

/// The caller's subscriptions followed by the instance defaults.
///
/// # Errors
/// Notifications disabled, storage errors.
pub async fn subscriptions(s: &ApiState, a: Actor) -> ApiResult<Vec<SubscriptionDto>> {
    let na = api(s)?;
    let store = na.engine.store();
    let mut out: Vec<_> = store
        .subscriptions(a.user_id)
        .await
        .map_err(port)?
        .iter()
        .map(sub_dto)
        .collect();
    out.extend(store.defaults().await.map_err(port)?.iter().map(sub_dto));
    Ok(out)
}

fn parse_sub(
    b: SubscriptionDto,
    id: Uuid,
    user: Option<domain::ids::UserId>,
) -> ApiResult<Subscription> {
    let ev = b.event.trim();
    let known = ev == "*"
        || CATALOGUE.iter().any(|e| {
            e.id == ev
                || ev
                    .strip_suffix(".*")
                    .is_some_and(|g| e.id.starts_with(&format!("{g}.")))
        });
    if !known {
        return Err(invalid("event", "unknown event or group"));
    }
    let filter = if b.filter.is_null() {
        notify::model::Filter::default()
    } else {
        serde_json::from_value(b.filter).map_err(|e| invalid("filter", e))?
    };
    let mode: notify::model::DeliveryMode =
        serde_json::from_value(b.mode).map_err(|e| invalid("mode", e))?;
    if let notify::model::DeliveryMode::Digest { cron } = &mode {
        notify::plan::validate_cron(cron).map_err(|e| invalid("mode.cron", e))?;
    }
    if !notify::plan::valid_timezone(&b.timezone) {
        return Err(invalid("timezone", "unknown IANA timezone"));
    }
    Ok(Subscription {
        id,
        user,
        event: ev.to_string(),
        filter,
        targets: serde_json::from_value(b.targets).map_err(|e| invalid("targets", e))?,
        mode,
        quiet_hours: b
            .quiet_hours
            .filter(|q| !q.is_null())
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| invalid("quiet_hours", e))?,
        timezone: b.timezone,
        rate_limit_per_hour: b.rate_limit_per_hour,
        enabled: b.enabled,
        locked: user.is_none() && b.locked,
    })
}

/// Create or update a subscription. `instance_default: true` (admins only) edits the
/// instance defaults.
///
/// # Errors
/// Notifications disabled, invalid fields, not the owner, not an admin.
pub async fn put_subscription(
    s: &ApiState,
    a: Actor,
    b: SubscriptionDto,
) -> ApiResult<SubscriptionDto> {
    let na = api(s)?;
    let store = na.engine.store();
    let default = b.instance_default;
    if default && !a.is_admin() {
        return Err(forbidden());
    }
    let user = (!default).then_some(a.user_id);
    let id = match &b.id {
        Some(id) => {
            let id = parse_id(id, |u| u)?;
            let old = store
                .subscription(id)
                .await
                .map_err(port)?
                .ok_or_else(|| not_found("subscription"))?;
            if old.user != user {
                return Err(not_found("subscription"));
            }
            id
        }
        None => s.app.ids.new_id(),
    };
    let sub = parse_sub(b, id, user)?;
    store.put_subscription(&sub).await.map_err(port)?;
    Ok(sub_dto(&sub))
}

/// # Errors
/// Notifications disabled, unknown subscription, not allowed.
pub async fn delete_subscription(s: &ApiState, a: Actor, id: &str) -> ApiResult<()> {
    let na = api(s)?;
    let store = na.engine.store();
    let id = parse_id(id, |u| u)?;
    let sub = store
        .subscription(id)
        .await
        .map_err(port)?
        .ok_or_else(|| not_found("subscription"))?;
    let allowed = match sub.user {
        Some(u) => u == a.user_id,
        None => a.is_admin(),
    };
    if !allowed {
        return Err(not_found("subscription"));
    }
    store.delete_subscription(id).await.map_err(port)
}

fn delivery_dto(d: &Delivery) -> DeliveryDto {
    DeliveryDto {
        id: d.id.to_string(),
        channel_id: d.channel.to_string(),
        event: d.event.clone(),
        title: d.title.clone(),
        state: d.state.as_str().into(),
        attempts: d.attempts,
        last_error: d.last_error.clone(),
        due_at: d.due_at.to_rfc3339(),
        created_at: d.created_at.to_rfc3339(),
        sent_at: d.sent_at.map(|t| t.to_rfc3339()),
    }
}

/// The caller's delivery log, newest first.
///
/// # Errors
/// Notifications disabled, storage errors.
pub async fn deliveries(s: &ApiState, a: Actor) -> ApiResult<Vec<DeliveryDto>> {
    let na = api(s)?;
    Ok(na
        .engine
        .store()
        .deliveries(a.user_id, INBOX_LIMIT)
        .await
        .map_err(port)?
        .iter()
        .map(delivery_dto)
        .collect())
}

/// Admin text overrides.
///
/// # Errors
/// Not an admin, notifications disabled.
pub async fn templates(s: &ApiState, a: Actor) -> ApiResult<Vec<TemplateOverrideDto>> {
    if !a.is_admin() {
        return Err(forbidden());
    }
    let na = api(s)?;
    let o = na.engine.store().template_overrides().await.map_err(port)?;
    Ok(o.into_iter()
        .flat_map(|(locale, keys)| {
            keys.into_iter()
                .map(move |(key, text)| TemplateOverrideDto {
                    locale: locale.clone(),
                    key,
                    text: Some(text),
                })
        })
        .collect())
}

/// Set or remove an admin text override; takes effect immediately.
///
/// # Errors
/// Not an admin, unknown locale, notifications disabled.
pub async fn set_template(s: &ApiState, a: Actor, b: TemplateOverrideDto) -> ApiResult<()> {
    if !a.is_admin() {
        return Err(forbidden());
    }
    if !notify::template::LOCALES.contains(&b.locale.as_str()) {
        return Err(invalid("locale", "unsupported locale"));
    }
    let na = api(s)?;
    na.engine
        .store()
        .set_template_override(&b.locale, &b.key, b.text.as_deref())
        .await
        .map_err(port)?;
    na.engine.reload_templates().await.map_err(port)
}

/// The notifications page in one call.
///
/// # Errors
/// Notifications disabled, storage errors.
pub async fn page(s: &ApiState, a: Actor) -> ApiResult<contracts::NotificationPageDto> {
    Ok(contracts::NotificationPageDto {
        inbox: inbox(s, a).await?,
        channels: channels(s, a).await?,
        subscriptions: subscriptions(s, a).await?,
        events: catalogue(a),
        deliveries: deliveries(s, a).await?,
    })
}
