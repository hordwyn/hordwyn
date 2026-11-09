//! The logging funnel on the server (WP4-5, design.md 9, research R-54, addendum-1 D16). The
//! browser's beacon (`POST /usage/funnel`, signed in, at most [`BEACONS_PER_MINUTE`] per
//! person and minute) carries fixed words and buckets only (`contracts::usage`); it is checked
//! against those words and added to `usage_funnel_daily (day, flow, subject, outcome,
//! taps_bucket, ms_bucket, n)`, which has no user and no item column. The instance setting
//! `usage.funnel` (on by default, env override, editable on Admin > Storage and jobs) turns
//! the counting off: beacons are then answered and dropped. A daily job writes one `metrics`
//! event per flow of the day before (DEC-060) and deletes days past [`RETENTION_DAYS`].
//! Admins read the totals on Admin > Storage and jobs (`GET /admin/usage`, CSV).

use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use auth::admin::{UsageKey, UsageRow};
use axum::http::StatusCode;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use contracts::usage::{
    DISCLOSURE_FLOW, DISCLOSURE_OUTCOMES, DISCLOSURE_SUBJECTS, FUNNEL_FLOWS, FUNNEL_OUTCOMES,
    FunnelEventDto, MAX_BEACON_EVENTS, RETENTION_DAYS, TAPS_BUCKETS, TIME_BUCKETS, UsageBeaconDto,
    UsageReportDto, UsageRowDto,
};
use domain::ids::UserId;
use domain::user::Actor;

use crate::admin::{admin, csv_field, effective_value, pe, require_admin};
use crate::error::ApiResult;
use crate::{ApiError, ApiState};

/// The instance setting.
pub const SETTING: &str = "usage.funnel";
/// Beacons one person may send per minute (the browser sends one every few seconds at most).
pub const BEACONS_PER_MINUTE: u32 = 30;
/// Days the admin report covers unless asked otherwise.
pub const REPORT_DAYS: u32 = 28;
const LIMITER_CAPACITY: usize = 10_000;

/// Beacons per person in the current minute, in this process' memory only (never stored, so
/// the counts on disk stay without users), and the setting `usage.funnel` as read last: kept a
/// minute, so a beacon does not read the instance settings each time; an admin change on this
/// process forgets it at once, other processes follow within the minute (UX4-14).
#[derive(Default)]
pub struct Limiter {
    beacons: Mutex<HashMap<UserId, (u32, DateTime<Utc>)>>,
    setting: Mutex<Option<(bool, DateTime<Utc>)>>,
}

impl Limiter {
    /// The setting as read less than a minute before `now`.
    fn cached_setting(&self, now: DateTime<Utc>) -> Option<bool> {
        let s = self
            .setting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        s.filter(|(_, at)| now - *at < Duration::minutes(1))
            .map(|(on, _)| on)
    }

    fn keep_setting(&self, on: bool, now: DateTime<Utc>) {
        *self
            .setting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((on, now));
    }

    /// Read the setting afresh next time (an admin changed it).
    pub fn forget_setting(&self) {
        *self
            .setting
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// `Err(seconds)` until the person may send again.
    fn allow(&self, user: UserId, now: DateTime<Utc>) -> Result<(), u64> {
        let minute = Duration::minutes(1);
        let mut m = self
            .beacons
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if m.len() >= LIMITER_CAPACITY && !m.contains_key(&user) {
            m.retain(|_, (_, since)| now - *since < minute);
        }
        let e = m.entry(user).or_insert((0, now));
        if now - e.1 >= minute {
            *e = (0, now);
        }
        if e.0 >= BEACONS_PER_MINUTE {
            let left = (e.1 + minute - now).num_seconds().max(1);
            return Err(u64::try_from(left).unwrap_or(60));
        }
        e.0 += 1;
        Ok(())
    }
}

/// Is counting on (the effective `usage.funnel`)? Off without the admin area; a storage
/// error counts as off.
pub async fn enabled(s: &ApiState) -> bool {
    let Some(a) = s.admin.as_ref() else {
        return false;
    };
    match a.store.settings().await {
        Ok(st) => effective_value(a, &st, SETTING).as_bool().unwrap_or(true),
        Err(e) => {
            tracing::warn!(error = %e, "reading usage.funnel failed; not counting");
            false
        }
    }
}

/// [`enabled`], read at most once a minute per process.
async fn enabled_cached(s: &ApiState, a: &crate::admin::AdminApi, now: DateTime<Utc>) -> bool {
    if let Some(on) = a.usage_limiter.cached_setting(now) {
        return on;
    }
    let on = enabled(s).await;
    a.usage_limiter.keep_setting(on, now);
    on
}

fn refused(msg: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid", msg)
}

/// A disclosure subject: one of [`DISCLOSURE_SUBJECTS`] (UX4-14), each `page/section` of 1 to
/// 32 lower-case letters or hyphens (an id or a title cannot pass).
fn subject_ok(s: &str) -> bool {
    if !DISCLOSURE_SUBJECTS.contains(&s) {
        return false;
    }
    let word = |w: &str| {
        (1..=32).contains(&w.len())
            && w.starts_with(|c: char| c.is_ascii_lowercase())
            && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
    };
    s.split_once('/')
        .is_some_and(|(page, section)| word(page) && word(section))
}

/// Check one event against the fixed words.
fn key(e: &FunnelEventDto) -> ApiResult<UsageKey> {
    let one_of = |v: Option<&String>, allowed: &[&str], what: &str| {
        v.filter(|v| allowed.contains(&v.as_str()))
            .cloned()
            .ok_or_else(|| refused(format!("{what}: use one of {}", allowed.join(", "))))
    };
    if e.flow == DISCLOSURE_FLOW {
        if e.taps.is_some() || e.time.is_some() {
            return Err(refused("a disclosure has no taps or time"));
        }
        let subject = e.subject.clone().filter(|s| subject_ok(s)).ok_or_else(|| {
            refused(format!(
                "subject: use one of {}",
                DISCLOSURE_SUBJECTS.join(", ")
            ))
        })?;
        return Ok(UsageKey {
            flow: e.flow.clone(),
            subject,
            outcome: one_of(Some(&e.outcome), &DISCLOSURE_OUTCOMES, "outcome")?,
            taps_bucket: String::new(),
            ms_bucket: String::new(),
        });
    }
    if !FUNNEL_FLOWS.contains(&e.flow.as_str()) {
        return Err(refused(format!(
            "flow: use one of {}, {DISCLOSURE_FLOW}",
            FUNNEL_FLOWS.join(", ")
        )));
    }
    if e.subject.is_some() {
        return Err(refused("only a disclosure has a subject"));
    }
    Ok(UsageKey {
        flow: e.flow.clone(),
        subject: String::new(),
        outcome: one_of(Some(&e.outcome), &FUNNEL_OUTCOMES, "outcome")?,
        taps_bucket: one_of(e.taps.as_ref(), &TAPS_BUCKETS, "taps")?,
        ms_bucket: one_of(e.time.as_ref(), &TIME_BUCKETS, "time")?,
    })
}

/// `POST /usage/funnel`: add the beacon's events to today's totals (UTC, the server's day).
///
/// # Errors
/// `too_many_attempts` (429) over the rate, `invalid` (422) for a beacon with an unknown word,
/// a subject that is not `page/section` or more than [`MAX_BEACON_EVENTS`] events; storage
/// errors.
pub async fn record(s: &ApiState, actor: Actor, b: UsageBeaconDto) -> ApiResult<()> {
    let a = admin(s)?;
    let now = s.app.clock.now();
    if let Err(secs) = a.usage_limiter.allow(actor.user_id, now) {
        let mut e = ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "too_many_attempts",
            "too many usage beacons",
        );
        e.retry_after_secs = Some(secs);
        return Err(e);
    }
    if b.events.len() > MAX_BEACON_EVENTS {
        return Err(refused(format!(
            "at most {MAX_BEACON_EVENTS} events per beacon"
        )));
    }
    let mut counts: BTreeMap<UsageKey, u64> = BTreeMap::new();
    for e in &b.events {
        *counts.entry(key(e)?).or_default() += 1;
    }
    if counts.is_empty() || !enabled_cached(s, a, now).await {
        return Ok(());
    }
    let counts: Vec<(UsageKey, u64)> = counts.into_iter().collect();
    a.store
        .usage_add(now.date_naive(), &counts)
        .await
        .map_err(pe)
}

fn row_dto(r: UsageRow) -> UsageRowDto {
    UsageRowDto {
        day: r.day.format("%Y-%m-%d").to_string(),
        flow: r.key.flow,
        subject: r.key.subject,
        outcome: r.key.outcome,
        taps_bucket: r.key.taps_bucket,
        ms_bucket: r.key.ms_bucket,
        n: r.n,
    }
}

/// `GET /admin/usage`: the totals of the last `days` (default [`REPORT_DAYS`], at most the
/// retention), newest day first, and whether counting is on.
///
/// # Errors
/// Not an admin; storage errors.
pub async fn report(s: &ApiState, actor: Actor, days: Option<u32>) -> ApiResult<UsageReportDto> {
    require_admin(actor)?;
    let a = admin(s)?;
    let days = days.unwrap_or(REPORT_DAYS).clamp(1, RETENTION_DAYS);
    let today = s.app.clock.now().date_naive();
    let since = today - Duration::days(i64::from(days) - 1);
    let rows = a.store.usage_rows(since).await.map_err(pe)?;
    Ok(UsageReportDto {
        enabled: enabled(s).await,
        retention_days: RETENTION_DAYS,
        days,
        rows: rows.into_iter().map(row_dto).collect(),
    })
}

/// The same rows as CSV (cells through the shared formula guard).
///
/// # Errors
/// As [`report`].
pub async fn report_csv(s: &ApiState, actor: Actor, days: Option<u32>) -> ApiResult<String> {
    let r = report(s, actor, days).await?;
    let mut out = String::from("day,flow,subject,outcome,taps_bucket,ms_bucket,n\n");
    for row in r.rows {
        let cols = [
            row.day,
            row.flow,
            row.subject,
            row.outcome,
            row.taps_bucket,
            row.ms_bucket,
            row.n.to_string(),
        ];
        let line: Vec<String> = cols.iter().map(|c| csv_field(c)).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    Ok(out)
}

/// One flow's (or one disclosure's) totals of a day, outcomes by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowTotals {
    pub flow: String,
    /// The disclosure's `page/section`; empty for the other flows.
    pub subject: String,
    pub outcomes: Vec<(String, u64)>,
}

/// What the daily job did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DailyReport {
    pub flows: Vec<FlowTotals>,
    /// Rows deleted past the retention.
    pub deleted: u64,
}

/// The daily job (`usage.funnel_daily`): one `metrics` event per flow of the day before
/// `today` (DEC-060), then delete the days before `today - RETENTION_DAYS`.
///
/// # Errors
/// Storage errors.
pub async fn daily(s: &ApiState, today: NaiveDate) -> ApiResult<DailyReport> {
    let a = admin(s)?;
    let day = today - Duration::days(1);
    let mut flows: BTreeMap<(String, String), BTreeMap<String, u64>> = BTreeMap::new();
    for r in a.store.usage_rows(day).await.map_err(pe)? {
        if r.day == day {
            *flows
                .entry((r.key.flow, r.key.subject))
                .or_default()
                .entry(r.key.outcome)
                .or_default() += r.n;
        }
    }
    let flows: Vec<FlowTotals> = flows
        .into_iter()
        .map(|((flow, subject), outcomes)| FlowTotals {
            flow,
            subject,
            outcomes: outcomes.into_iter().collect(),
        })
        .collect();
    for f in &flows {
        let total: u64 = f.outcomes.iter().map(|(_, n)| n).sum();
        let outcomes: Vec<String> = f.outcomes.iter().map(|(o, n)| format!("{o}={n}")).collect();
        tracing::info!(
            target: "metrics",
            metric = "usage.funnel",
            day = %day,
            flow = %f.flow,
            subject = %f.subject,
            total,
            outcomes = %outcomes.join(" "),
        );
    }
    let before = today - Duration::days(i64::from(RETENTION_DAYS));
    let deleted = a.store.usage_purge(before).await.map_err(pe)?;
    if deleted > 0 {
        tracing::info!(
            count = deleted,
            "deleted usage totals past the retention period"
        );
    }
    Ok(DailyReport { flows, deleted })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subjects_are_two_plain_words() {
        assert!(subject_ok("item/log-more"));
        assert!(subject_ok("instance/advanced"));
        for bad in [
            "item",
            "item/",
            "/x",
            "item/log/more",
            "Item/x",
            "item/1",
            "item/0190a1b2-c3d4",
            "a/b c",
            &format!("a/{}", "b".repeat(33)),
        ] {
            assert!(!subject_ok(bad), "{bad}");
        }
    }

    #[test]
    fn the_limit_is_per_person_and_minute() {
        let l = Limiter::default();
        let (anna, ben) = (
            UserId(uuid::Uuid::from_u128(1)),
            UserId(uuid::Uuid::from_u128(2)),
        );
        let t0 = DateTime::<Utc>::from_timestamp(1_000_000, 0).unwrap();
        for _ in 0..BEACONS_PER_MINUTE {
            assert!(l.allow(anna, t0).is_ok());
        }
        assert_eq!(l.allow(anna, t0 + Duration::seconds(20)), Err(40));
        assert!(l.allow(ben, t0).is_ok());
        assert!(l.allow(anna, t0 + Duration::seconds(61)).is_ok());
    }

    #[test]
    fn subjects_are_the_known_sections_only() {
        // UX4-14: free words would let one person create rows without bound
        for s in DISCLOSURE_SUBJECTS {
            assert!(subject_ok(s), "{s}");
        }
        assert!(!subject_ok("made/up"));
        assert!(
            !subject_ok("gallery/mail"),
            "the dev gallery is not measured"
        );
    }

    #[test]
    fn the_setting_is_read_once_a_minute_until_it_changes() {
        let l = Limiter::default();
        let t0 = DateTime::<Utc>::from_timestamp(1_000_000, 0).unwrap();
        assert_eq!(l.cached_setting(t0), None, "nothing read yet");
        l.keep_setting(false, t0);
        assert_eq!(l.cached_setting(t0 + Duration::seconds(59)), Some(false));
        assert_eq!(
            l.cached_setting(t0 + Duration::seconds(60)),
            None,
            "a minute old"
        );
        l.keep_setting(true, t0);
        l.forget_setting();
        assert_eq!(
            l.cached_setting(t0),
            None,
            "an admin change reads it afresh"
        );
    }
}
