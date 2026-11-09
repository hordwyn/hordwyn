//! Health probes (addendum-1 3.4, DEC-285; WP4-2): the `admin.probes` job computes the eight
//! Overview rows every 60 s on the background lane and stores them (`health_probes`); the
//! Overview only reads the table, so the page never waits on a probe. A row older than
//! [`STALE_SECS`] says "not checked recently". The admin's own changes (mail settings and
//! test, a provider saved or tested, a source key or switch) re-run the affected row at once
//! from stored fields, so the page follows them without a third-party call.

use auth::admin::ProbeRow;
use chrono::{DateTime, Utc};
use contracts::admin::HealthRowDto;
use domain::ids::UserId;
use domain::user::{Actor, Role};
use uuid::Uuid;

use super::{admin, jobs_row, mail, oidc, pe, row, secrets_row, sources_row, storage_row, ts};
use crate::ApiState;
use crate::error::ApiResult;

/// The rows in Overview order.
pub const PROBE_IDS: [&str; 8] = [
    "database",
    "search",
    "storage",
    "jobs",
    "mail",
    "data_sources",
    "identity_provider",
    "secrets",
];

/// A row older than this is reported as not checked recently (plan default: 3 minutes).
pub const STALE_SECS: i64 = 180;

/// The probe job looks with the admin vocabulary and no credentials of its own.
fn system() -> Actor {
    Actor {
        user_id: UserId(Uuid::nil()),
        role: Role::Admin,
    }
}

/// Compute one row now.
async fn compute(s: &ApiState, id: &str) -> ApiResult<HealthRowDto> {
    let a = admin(s)?;
    Ok(match id {
        "database" => {
            if a.store.ping().await.is_ok() {
                row("database", "ok", "reachable", &[])
            } else {
                row("database", "error", "unreachable", &[])
            }
        }
        "search" => match a.health.search_backend.as_str() {
            "postgres_fallback" => row("search", "warning", "fallback", &[]),
            b => row("search", "ok", "backend", &[("backend", b.to_string())]),
        },
        "storage" => storage_row(a).await,
        "jobs" => jobs_row(a).await,
        "mail" => mail::health_row(a, &a.store.settings().await.map_err(pe)?),
        "data_sources" => sources_row(s, system()).await?,
        "identity_provider" => oidc::health_row(s).await,
        _ => secrets_row(s).await?,
    })
}

fn stored(h: HealthRowDto, at: DateTime<Utc>) -> ProbeRow {
    ProbeRow {
        id: h.id,
        status: h.status,
        detail: h.detail,
        args: h.args,
        checked_at: at,
    }
}

/// The `admin.probes` job: compute and store every row; returns how many were written.
///
/// # Errors
/// Admin area off, storage errors.
pub async fn run_probes(s: &ApiState) -> ApiResult<usize> {
    let a = admin(s)?;
    let mut rows = Vec::with_capacity(PROBE_IDS.len());
    for id in PROBE_IDS {
        let r = compute(s, id).await?;
        rows.push(stored(r, s.app.clock.now()));
    }
    a.store.put_probes(&rows).await.map_err(pe)?;
    Ok(rows.len())
}

/// Re-run one row after a change the admin made (stored fields only); a failure is logged.
pub(crate) async fn refresh_probe(s: &ApiState, id: &str) {
    let Ok(a) = admin(s) else {
        return;
    };
    let written = match compute(s, id).await {
        Ok(r) => a
            .store
            .put_probes(&[stored(r, s.app.clock.now())])
            .await
            .map_err(pe),
        Err(e) => Err(e),
    };
    if let Err(e) = written {
        tracing::warn!(error = %e.message, probe = id, "refreshing a health row failed");
    }
}

/// `YYYY-MM-DDTHH:MM` of `t` in `zone`.
fn local(t: DateTime<Utc>, zone: app::time_zone::Tz) -> String {
    format!(
        "{}T{}",
        app::time_zone::local_day(t, zone),
        app::time_zone::local_time(t, zone)
    )
}

/// The stored rows in Overview order, each as stored (a missing one as `unknown`), with
/// local times for `zone` and the stale flag.
///
/// # Errors
/// Admin area off, storage errors.
pub(crate) async fn stored_rows(
    s: &ApiState,
    zone: app::time_zone::Tz,
) -> ApiResult<Vec<HealthRowDto>> {
    let a = admin(s)?;
    let now = s.app.clock.now();
    let mut all = a.store.probes().await.map_err(pe)?;
    Ok(PROBE_IDS
        .iter()
        .map(|id| {
            let Some(p) = all
                .iter()
                .position(|p| p.id == *id)
                .map(|i| all.swap_remove(i))
            else {
                return row(id, "unknown", "not_checked", &[]);
            };
            let mut args = p.args;
            let at_local = args
                .get("at")
                .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
                .map(|t| local(t.with_timezone(&Utc), zone));
            if let Some(l) = at_local {
                args.insert("at_local".into(), l);
            }
            HealthRowDto {
                id: p.id,
                status: p.status,
                detail: p.detail,
                args,
                checked_at: Some(ts(p.checked_at)),
                checked_local: Some(local(p.checked_at, zone)),
                stale: (now - p.checked_at).num_seconds() > STALE_SECS,
            }
        })
        .collect())
}
