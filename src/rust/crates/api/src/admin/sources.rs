//! Data sources (design.md 4.2 and 4.4): status from the credential state and the governed
//! client's counters, credentials (instance and per user, encrypted), Test, licence
//! records, "Ask admin" and the reason lines of a search.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use app::ports::{Audience, HttpError, Lane, Notification};
use auth::admin::{LicenceRecord, StoredCredential};
use axum::http::StatusCode;
use catalog_core::{
    Capability, Connector, ConnectorError, CredentialState, Ctx, SearchRequest, SourceCounters,
};
use contracts::admin::{
    AdminSourceDto, AskAdminDto, CredentialInputDto, CredentialMetaDto, LicenceInputDto,
    SkippedSourceDto, SourceStatusDto, SourceStatusPageDto, SourceTestDto, SourceTestInputDto,
    UserCredentialDto,
};
use domain::user::Actor;
use serde_json::json;
use zeroize::Zeroizing;

use super::{admin, audit, invalid, pe, require_admin, ts};
use crate::error::ApiResult;
use crate::{ApiError, ApiState};

fn unknown_source() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "unknown_source",
        "unknown data source",
    )
}

/// Kind id -> group id of the enabled kinds.
fn kind_groups(s: &ApiState) -> BTreeMap<String, String> {
    s.app
        .kinds
        .all()
        .iter()
        .map(|d| (d.id.to_string(), d.group.as_str().to_string()))
        .collect()
}

fn groups_of(kinds: &[String], map: &BTreeMap<String, String>) -> Vec<String> {
    let mut g: Vec<String> = kinds.iter().filter_map(|k| map.get(k).cloned()).collect();
    g.sort();
    g.dedup();
    g
}

fn counters(s: &ApiState, id: &str) -> SourceCounters {
    s.registry
        .stats()
        .map(|st| st.counters(id))
        .unwrap_or_default()
}

/// The admin vocabulary of design.md 4.4.
fn admin_state(state: CredentialState, c: &SourceCounters) -> &'static str {
    match state {
        CredentialState::Missing => "not_set_up",
        CredentialState::Locked => "key_unavailable",
        CredentialState::Invalid => "key_rejected",
        CredentialState::NotNeeded | CredentialState::Env | CredentialState::Db => {
            if c.circuit_open_until.is_some() {
                "circuit_open"
            } else if c
                .last_rate_limited_at
                .is_some_and(|r| c.last_ok_at.is_none_or(|ok| r > ok))
            {
                "rate_limited"
            } else {
                "working"
            }
        }
    }
}

/// What a member sees for the same state (no details, design.md 4.4).
fn member_state(admin: &str, needs_account: bool) -> &'static str {
    match admin {
        "not_set_up" => "not_set_up",
        "key_unavailable" | "key_rejected" => "unavailable",
        "rate_limited" => "slow",
        "circuit_open" => "retrying",
        _ if needs_account => "needs_account",
        _ => "working",
    }
}

fn status_dto(
    s: &ApiState,
    c: &Arc<dyn Connector>,
    actor: Actor,
    groups: &BTreeMap<String, String>,
    user_set: bool,
) -> SourceStatusDto {
    let info = c.info();
    let spec = c.credentials();
    let state = s.registry.credential(&info.id);
    let cnt = counters(s, &info.id);
    let a_state = admin_state(state, &cnt);
    let hosted = s.admin.as_ref().is_some_and(super::AdminApi::hosted);
    let needs_account = hosted && !spec.user_fields.is_empty() && !user_set;
    let mut dto = SourceStatusDto {
        id: info.id.clone(),
        name: info.name.clone(),
        kinds: info.kinds.clone(),
        groups: groups_of(&info.kinds, groups),
        state: a_state.into(),
        credential: None,
        env_var: None,
        calls_24h: 0,
        rate_limited_24h: 0,
        last_ok_at: None,
        last_error_at: None,
        last_error: None,
        last_auth_status: None,
        retry_at: None,
        user_fields: spec.user_fields.iter().map(|f| (*f).to_string()).collect(),
        user_credential_set: user_set,
    };
    if actor.is_admin() {
        dto.credential = Some(state.as_str().into());
        dto.env_var = s
            .admin
            .as_ref()
            .and_then(|a| a.config.source_env.get(&info.id).cloned())
            .filter(|_| state == CredentialState::Env || state == CredentialState::Invalid);
        dto.calls_24h = cnt.calls_24h;
        dto.rate_limited_24h = cnt.rate_limited_24h;
        dto.last_ok_at = cnt.last_ok_at.map(ts);
        dto.last_error_at = cnt.last_error_at.map(ts);
        dto.last_error = cnt.last_error;
        dto.last_auth_status = cnt.last_auth_status;
        dto.retry_at = cnt.circuit_open_until.map(ts);
    } else {
        dto.state = member_state(a_state, needs_account).into();
    }
    dto
}

fn registered(s: &ApiState) -> Vec<Arc<dyn Connector>> {
    s.registry
        .infos()
        .iter()
        .filter_map(|i| s.registry.get(&i.id))
        .collect()
}

async fn user_set_sources(s: &ApiState, actor: Actor) -> ApiResult<Vec<String>> {
    let Some(a) = s.admin.as_ref() else {
        return Ok(vec![]);
    };
    Ok(a.store
        .user_credentials(actor.user_id)
        .await
        .map_err(pe)?
        .into_iter()
        .filter(|c| c.fields.is_ok())
        .map(|c| c.connector)
        .collect())
}

/// Every source with its state, and the kind groups with no working source.
///
/// # Errors
/// Storage errors.
pub async fn source_status(s: &ApiState, actor: Actor) -> ApiResult<SourceStatusPageDto> {
    let groups = kind_groups(s);
    let mine = user_set_sources(s, actor).await?;
    let mut sources: Vec<SourceStatusDto> = registered(s)
        .iter()
        .map(|c| status_dto(s, c, actor, &groups, mine.contains(&c.info().id)))
        .collect();
    if actor.is_admin()
        && let Some(a) = s.admin.as_ref()
    {
        for (id, name, kinds) in &a.config.disabled_sources {
            sources.push(disabled_dto(id, name, kinds, &groups));
        }
    }
    let mut served: Vec<String> = sources
        .iter()
        .filter(|x| {
            matches!(
                x.state.as_str(),
                "working" | "rate_limited" | "slow" | "needs_account"
            )
        })
        .flat_map(|x| x.groups.clone())
        .collect();
    served.sort();
    served.dedup();
    let mut without: Vec<String> = groups.values().cloned().collect();
    without.sort();
    without.dedup();
    without.retain(|g| !served.contains(g));
    let setup_open = match s.admin.as_ref() {
        Some(a) if actor.is_admin() => a
            .store
            .settings()
            .await
            .map_err(pe)?
            .setup_completed_at
            .is_none(),
        _ => false,
    };
    Ok(SourceStatusPageDto {
        sources,
        groups_without_source: without,
        is_admin: actor.is_admin(),
        setup_open,
    })
}

fn disabled_dto(
    id: &str,
    name: &str,
    kinds: &[String],
    groups: &BTreeMap<String, String>,
) -> SourceStatusDto {
    SourceStatusDto {
        id: id.into(),
        name: name.into(),
        kinds: kinds.to_vec(),
        groups: groups_of(kinds, groups),
        state: "disabled".into(),
        credential: None,
        env_var: None,
        calls_24h: 0,
        rate_limited_24h: 0,
        last_ok_at: None,
        last_error_at: None,
        last_error: None,
        last_auth_status: None,
        retry_at: None,
        user_fields: vec![],
        user_credential_set: false,
    }
}

/// The reason lines of a search over `kinds`: skipped sources and groups without any
/// working source (design.md 4.4).
#[must_use]
pub fn search_reasons(
    s: &ApiState,
    actor: Actor,
    kinds: &[String],
) -> (Vec<SkippedSourceDto>, Vec<String>) {
    let groups = kind_groups(s);
    let skipped: Vec<SkippedSourceDto> = s
        .registry
        .skipped(kinds, Capability::Search)
        .into_iter()
        .filter_map(|(id, state)| {
            let c = s.registry.get(&id)?;
            let a = admin_state(state, &SourceCounters::default());
            Some(SkippedSourceDto {
                id,
                name: c.info().name.clone(),
                state: if actor.is_admin() {
                    a.into()
                } else {
                    member_state(a, false).into()
                },
            })
        })
        .collect();
    let mut without: Vec<String> = kinds
        .iter()
        .filter(|k| s.registry.for_kind(k, Capability::Search).is_empty())
        .filter_map(|k| groups.get(k).cloned())
        .collect();
    without.sort();
    without.dedup();
    // a group is "without source" only when none of its queried kinds has one
    without.retain(|g| {
        kinds
            .iter()
            .filter(|k| groups.get(*k) == Some(g))
            .all(|k| s.registry.for_kind(k, Capability::Search).is_empty())
    });
    (skipped, without)
}

/// "Ask admin": notify the admins that a member wants a source (deduplicated per source
/// per day, event `admin.source_requested`).
///
/// # Errors
/// Neither a known source nor a known group.
pub async fn ask_admin(s: &ApiState, actor: Actor, b: AskAdminDto) -> ApiResult<()> {
    let (key, label) = match (b.source.as_deref(), b.group.as_deref()) {
        (Some(id), _) => {
            let c = s.registry.get(id).ok_or_else(unknown_source)?;
            (format!("source:{id}"), c.info().name.clone())
        }
        (None, Some(g)) => {
            let group = crate::browse::parse_group(g)?;
            (
                format!("group:{}", group.as_str()),
                group.as_str().to_string(),
            )
        }
        (None, None) => return Err(ApiError::bad_request("name a source or a group")),
    };
    let user = s.app.user(actor.user_id).await?;
    let day = s.app.clock.now().format("%Y-%m-%d");
    s.app
        .notifications
        .emit(
            Notification::new(
                "admin.source_requested",
                Audience::Admins,
                format!("admin.source_requested:{key}:{day}"),
            )
            .arg("source", label)
            .arg("user", user.display_name)
            .link("/admin/sources"),
        )
        .await;
    Ok(())
}

// ---------------------------------------------------------------- credentials

/// The connector's key string from stored fields (`a:b` in spec order).
fn key_string(fields: &BTreeMap<String, String>, names: &[&str]) -> Option<Zeroizing<String>> {
    let parts: Vec<&str> = names
        .iter()
        .map(|n| fields.get(*n).map(String::as_str))
        .collect::<Option<_>>()?;
    Some(Zeroizing::new(parts.join(":")))
}

/// Check the fields against the spec: every field present and non-empty, no others.
fn check_fields(
    fields: &BTreeMap<String, String>,
    names: &[&str],
) -> ApiResult<BTreeMap<String, String>> {
    if let Some(k) = fields.keys().find(|k| !names.contains(&k.as_str())) {
        return Err(invalid("unknown_field", format!("unknown field `{k}`")));
    }
    let mut out = BTreeMap::new();
    for n in names {
        let v = fields.get(*n).map(|v| v.trim()).unwrap_or_default();
        if v.is_empty() {
            return Err(invalid("missing_field", format!("`{n}` is required")));
        }
        out.insert((*n).to_string(), v.to_string());
    }
    Ok(out)
}

/// Load the stored instance credentials into the registry (start-up): environment keys
/// stay in charge; a stored key that cannot be decrypted is `locked`.
///
/// # Errors
/// Storage errors.
pub async fn load_credentials(s: &ApiState) -> ApiResult<()> {
    let a = admin(s)?;
    for c in a.store.instance_credentials().await.map_err(pe)? {
        apply_credential(s, &c.connector, Some(&c));
    }
    Ok(())
}

fn apply_credential(s: &ApiState, id: &str, stored: Option<&StoredCredential>) {
    let Some(conn) = s.registry.get(id) else {
        return;
    };
    let env = s
        .admin
        .as_ref()
        .is_some_and(|a| a.config.source_env.contains_key(id));
    if env {
        return;
    }
    match stored.map(|c| &c.fields) {
        Some(Ok(fields)) => {
            let key = key_string(fields, conn.credentials().instance_fields);
            s.registry.set_key(
                id,
                key.as_deref().cloned(),
                if key.is_some() {
                    CredentialState::Db
                } else {
                    CredentialState::Missing
                },
            );
        }
        Some(Err(_)) => s.registry.set_key(id, None, CredentialState::Locked),
        None => s.registry.set_key(id, None, CredentialState::Missing),
    }
}

async fn admin_source(
    s: &ApiState,
    c: &Arc<dyn Connector>,
    actor: Actor,
) -> ApiResult<AdminSourceDto> {
    let a = admin(s)?;
    let info = c.info();
    let stored = a.store.credential(&info.id, None).await.map_err(pe)?;
    let env_var = a.config.source_env.get(&info.id).cloned();
    let set_by = match stored.as_ref().and_then(|c| c.created_by) {
        Some(u) => s.app.user_opt(u).await?.map(|u| u.username),
        None => None,
    };
    let credential = CredentialMetaDto {
        source: match (&env_var, stored.as_ref().map(|c| c.fields.is_ok())) {
            (Some(_), _) => "env",
            (None, Some(true)) => "db",
            (None, Some(false)) => "locked",
            (None, None) => "missing",
        }
        .into(),
        shadowed: env_var.is_some() && stored.is_some(),
        env_var,
        set_at: stored.as_ref().map(|c| ts(c.created_at)),
        set_by,
        last_tested_at: stored.as_ref().and_then(|c| c.last_tested_at).map(ts),
        last_result: stored.as_ref().and_then(|c| c.last_result.clone()),
    };
    let licence = a
        .store
        .licences()
        .await
        .map_err(pe)?
        .into_iter()
        .find(|l| l.connector == info.id);
    let groups = kind_groups(s);
    Ok(AdminSourceDto {
        status: status_dto(s, c, actor, &groups, false),
        credential,
        fields: c
            .credentials()
            .instance_fields
            .iter()
            .map(|f| (*f).to_string())
            .collect(),
        attribution: info.attribution.text.clone(),
        terms_url: info.attribution.url.clone(),
        agreement_on_file: licence.as_ref().map(|l| l.agreement_on_file),
        licence_warning: a.hosted() && licence.is_none_or(|l| !l.agreement_on_file),
    })
}

/// Does any source lack an agreement on file (hosted banner)?
///
/// # Errors
/// Storage errors.
pub(crate) async fn licence_missing(s: &ApiState) -> ApiResult<bool> {
    let a = admin(s)?;
    let on_file: Vec<String> = a
        .store
        .licences()
        .await
        .map_err(pe)?
        .into_iter()
        .filter(|l| l.agreement_on_file)
        .map(|l| l.connector)
        .collect();
    Ok(s.registry.infos().iter().any(|i| !on_file.contains(&i.id)))
}

fn connector(s: &ApiState, id: &str) -> ApiResult<Arc<dyn Connector>> {
    s.registry.get(id).ok_or_else(unknown_source)
}

/// Admin > Data sources.
///
/// # Errors
/// Not an admin, storage errors.
pub async fn sources(s: &ApiState, actor: Actor) -> ApiResult<Vec<AdminSourceDto>> {
    require_admin(actor)?;
    let mut out = Vec::new();
    for c in registered(s) {
        out.push(admin_source(s, &c, actor).await?);
    }
    Ok(out)
}

/// Replace the instance credential (encrypted; takes effect at once unless the environment
/// sets the key). Not gated (research 4.2), audited with purpose and key version.
///
/// # Errors
/// Not an admin, unknown source, `missing_field`, `unknown_field`.
pub async fn set_source_credential(
    s: &ApiState,
    actor: Actor,
    id: &str,
    b: CredentialInputDto,
) -> ApiResult<AdminSourceDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let c = connector(s, id)?;
    let spec = c.credentials();
    if spec.instance_fields.is_empty() {
        return Err(invalid("unknown_field", "this source needs no key"));
    }
    let fields = check_fields(&b.fields, spec.instance_fields)?;
    let stored = a
        .store
        .put_credential(id, None, &fields, Some(actor.user_id), s.app.clock.now())
        .await
        .map_err(pe)?;
    apply_credential(s, id, Some(&stored));
    let detail = json!({"purpose": auth::admin::CREDENTIAL_PURPOSE, "key_version": a.keys.active});
    audit(
        s,
        Some(actor),
        "secret.replaced",
        Some(("source", id.to_string())),
        detail,
    )
    .await;
    admin_source(s, &c, actor).await
}

/// Remove the stored instance credential.
///
/// # Errors
/// Not an admin, unknown source.
pub async fn clear_source_credential(
    s: &ApiState,
    actor: Actor,
    id: &str,
) -> ApiResult<AdminSourceDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let c = connector(s, id)?;
    a.store.delete_credential(id, None).await.map_err(pe)?;
    apply_credential(s, id, None);
    let detail = json!({"purpose": auth::admin::CREDENTIAL_PURPOSE});
    audit(
        s,
        Some(actor),
        "secret.cleared",
        Some(("source", id.to_string())),
        detail,
    )
    .await;
    admin_source(s, &c, actor).await
}

/// Record whether an agreement with the provider is on file (hosted mode).
///
/// # Errors
/// Not an admin, unknown source.
pub async fn set_source_licence(
    s: &ApiState,
    actor: Actor,
    id: &str,
    b: LicenceInputDto,
) -> ApiResult<AdminSourceDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let c = connector(s, id)?;
    a.store
        .put_licence(&LicenceRecord {
            connector: id.to_string(),
            agreement_on_file: b.agreement_on_file,
            recorded_by: Some(actor.user_id),
            recorded_at: s.app.clock.now(),
        })
        .await
        .map_err(pe)?;
    let detail = json!({"agreement_on_file": b.agreement_on_file});
    audit(
        s,
        Some(actor),
        "licence.recorded",
        Some(("source", id.to_string())),
        detail,
    )
    .await;
    admin_source(s, &c, actor).await
}

fn test_result(
    r: &Result<Vec<catalog_core::Snapshot>, ConnectorError>,
) -> (&'static str, Option<u16>) {
    match r {
        Ok(_) => ("ok", None),
        Err(ConnectorError::Status {
            status: s @ (401 | 403),
        }) => ("key_rejected", Some(*s)),
        Err(ConnectorError::Status { status }) => ("failed", Some(*status)),
        Err(ConnectorError::MissingCredentials(_)) => ("missing_key", None),
        Err(ConnectorError::Http(HttpError::RateLimited { .. })) => ("rate_limited", Some(429)),
        Err(ConnectorError::Http(HttpError::CircuitOpen)) => ("circuit_open", None),
        Err(ConnectorError::Http(HttpError::Network(_) | HttpError::Blocked(_))) => {
            ("unreachable", None)
        }
        Err(_) => ("failed", None),
    }
}

/// Test a source: the given fields (before saving) or the stored key; one search, time and
/// one sample title. A failed candidate does not mark the live key as rejected.
///
/// # Errors
/// Not an admin, unknown source, bad fields.
pub async fn test_source(
    s: &ApiState,
    actor: Actor,
    id: &str,
    input: SourceTestInputDto,
) -> ApiResult<SourceTestDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let c = connector(s, id)?;
    let spec = c.credentials();
    let (probe, candidate) = match &input.fields {
        Some(f) => {
            let fields = check_fields(f, spec.instance_fields)?;
            let key = key_string(&fields, spec.instance_fields).map(|k| k.to_string());
            (c.probe(key).unwrap_or_else(|| c.clone()), true)
        }
        None => (c.clone(), false),
    };
    let Some(search) = probe.as_search() else {
        return Err(invalid("unknown_field", "this source cannot be tested"));
    };
    let req = SearchRequest {
        kind: probe.info().kinds.first().cloned().unwrap_or_default(),
        query: "star".into(),
        language: None,
        limit: 1,
    };
    // a candidate's rejection is never recorded for the live key (UX2-S5), so concurrent
    // searches keep using it and a real rejection of the live key is not overwritten
    let mut ctx = Ctx::new(a.http.clone(), Lane::Interactive);
    if candidate {
        ctx = ctx.for_candidate_key();
    }
    let started = Instant::now();
    let answer = search.search(&ctx, &req).await;
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let (result, http_status) = test_result(&answer);
    if !candidate && let Some(stored) = a.store.credential(id, None).await.map_err(pe)? {
        a.store
            .record_test(stored.id, s.app.clock.now(), result)
            .await
            .map_err(pe)?;
    }
    Ok(SourceTestDto {
        ok: answer.is_ok(),
        result: result.into(),
        elapsed_ms,
        sample_title: answer
            .ok()
            .and_then(|v| v.into_iter().next())
            .map(|x| x.title),
        http_status,
    })
}

// ---------------------------------------------------------------- per-user credentials

/// Settings > Data sources (members): sources that ask for the member's own credential.
///
/// # Errors
/// Storage errors.
pub async fn my_credentials(s: &ApiState, actor: Actor) -> ApiResult<Vec<UserCredentialDto>> {
    let a = admin(s)?;
    let mine = a.store.user_credentials(actor.user_id).await.map_err(pe)?;
    Ok(registered(s)
        .iter()
        .filter(|c| !c.credentials().user_fields.is_empty())
        .map(|c| {
            let id = &c.info().id;
            let stored = mine.iter().find(|m| &m.connector == id);
            UserCredentialDto {
                source: id.clone(),
                name: c.info().name.clone(),
                fields: c
                    .credentials()
                    .user_fields
                    .iter()
                    .map(|f| (*f).to_string())
                    .collect(),
                set: stored.is_some_and(|m| m.fields.is_ok()),
                locked: stored.is_some_and(|m| m.fields.is_err()),
                set_at: stored.map(|m| ts(m.created_at)),
            }
        })
        .collect())
}

/// The member's own credentials as connector keys, for their searches (`TheTVDB` PIN,
/// design.md 4.1; DEC-246 follow-up). Credentials that cannot be decrypted are left out
/// (the instance key applies then). Without the admin store: none.
pub async fn user_keys(s: &ApiState, actor: Actor) -> catalog_core::search::UserKeys {
    let mut out = catalog_core::search::UserKeys::new();
    let Some(a) = s.admin.as_ref() else {
        return out;
    };
    let mine = match a.store.user_credentials(actor.user_id).await {
        Ok(list) => list,
        Err(e) => {
            tracing::warn!(error = %e, "member credentials unreadable");
            return out;
        }
    };
    for c in registered(s) {
        let names = c.credentials().user_fields;
        if names.is_empty() {
            continue;
        }
        let id = &c.info().id;
        if let Some(Ok(fields)) = mine.iter().find(|m| &m.connector == id).map(|m| &m.fields)
            && let Some(key) = key_string(fields, names)
        {
            out.insert(id.clone(), key);
        }
    }
    out
}

/// Store the member's own credential (never shown to admins).
///
/// # Errors
/// Unknown source, a source without per-user fields, bad fields.
pub async fn set_my_credential(
    s: &ApiState,
    actor: Actor,
    id: &str,
    b: CredentialInputDto,
) -> ApiResult<Vec<UserCredentialDto>> {
    let a = admin(s)?;
    let c = connector(s, id)?;
    let names = c.credentials().user_fields;
    if names.is_empty() {
        return Err(invalid(
            "unknown_field",
            "this source has no personal credential",
        ));
    }
    let fields = check_fields(&b.fields, names)?;
    a.store
        .put_credential(
            id,
            Some(actor.user_id),
            &fields,
            Some(actor.user_id),
            s.app.clock.now(),
        )
        .await
        .map_err(pe)?;
    my_credentials(s, actor).await
}

/// # Errors
/// Unknown source, storage errors.
pub async fn clear_my_credential(
    s: &ApiState,
    actor: Actor,
    id: &str,
) -> ApiResult<Vec<UserCredentialDto>> {
    let a = admin(s)?;
    connector(s, id)?;
    a.store
        .delete_credential(id, Some(actor.user_id))
        .await
        .map_err(pe)?;
    my_credentials(s, actor).await
}
