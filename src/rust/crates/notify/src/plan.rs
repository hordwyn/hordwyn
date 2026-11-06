//! Pure planning rules: which subscriptions apply, quiet hours, digest times, rate limits.

use app::ports::Subject;
use chrono::{DateTime, Duration, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::model::{Filter, QuietHours, Subscription};

/// Does an event id match a pattern (`id`, `group.*` or `*`)?
#[must_use]
pub fn event_matches(pattern: &str, event: &str) -> bool {
    pattern == "*"
        || pattern == event
        || pattern
            .strip_suffix(".*")
            .is_some_and(|group| event.split_once('.').is_some_and(|(g, _)| g == group))
}

#[must_use]
pub fn filter_matches(f: &Filter, s: &Subject) -> bool {
    fn any<T: PartialEq>(wanted: &[T], got: Option<&T>) -> bool {
        wanted.is_empty() || got.is_some_and(|g| wanted.contains(g))
    }
    any(&f.kinds, s.kind.as_ref())
        && any(&f.lists, s.list.as_ref())
        && any(&f.items, s.item.as_ref())
        && any(&f.companions, s.companion.as_ref())
}

/// The subscriptions that apply to a user: their own plus the instance defaults they did not
/// override; locked defaults always apply. Disabled ones are dropped after overriding, so
/// an own disabled subscription mutes a default.
#[must_use]
pub fn effective(own: &[Subscription], defaults: &[Subscription]) -> Vec<Subscription> {
    let mut out: Vec<Subscription> = defaults
        .iter()
        .filter(|d| d.locked || !own.iter().any(|o| o.event == d.event))
        .cloned()
        .collect();
    out.extend(
        own.iter()
            .filter(|o| !defaults.iter().any(|d| d.locked && d.event == o.event))
            .cloned(),
    );
    out.retain(|s| s.enabled);
    out
}

/// Subscriptions of `effective` that want this event.
#[must_use]
pub fn matching<'a>(
    subs: &'a [Subscription],
    event: &str,
    subject: &Subject,
) -> Vec<&'a Subscription> {
    subs.iter()
        .filter(|s| event_matches(&s.event, event) && filter_matches(&s.filter, subject))
        .collect()
}

#[must_use]
pub fn valid_timezone(name: &str) -> bool {
    name.parse::<Tz>().is_ok()
}

#[must_use]
pub fn timezone(name: &str) -> Tz {
    name.parse().unwrap_or(Tz::UTC)
}

fn in_window(t: NaiveTime, q: QuietHours) -> bool {
    if q.start <= q.end {
        t >= q.start && t < q.end
    } else {
        t >= q.start || t < q.end
    }
}

/// If `now` falls into the quiet hours (local time in `tz`), the instant they end.
#[must_use]
pub fn quiet_until(now: DateTime<Utc>, tz: Tz, q: QuietHours) -> Option<DateTime<Utc>> {
    let local = now.with_timezone(&tz);
    if !in_window(local.time(), q) {
        return None;
    }
    let mut day = local.date_naive();
    if q.start > q.end && local.time() >= q.start {
        day = day.succ_opt()?;
    }
    // a DST gap at the end time: take the first valid instant after it
    let mut end = day.and_time(q.end);
    for _ in 0..4 {
        if let Some(t) = tz.from_local_datetime(&end).earliest() {
            return Some(t.with_timezone(&Utc));
        }
        end += Duration::minutes(30);
    }
    None
}

/// Next occurrence of a 5-field cron schedule after `now`, evaluated in `tz`.
///
/// # Errors
/// If the schedule does not parse.
pub fn next_digest(cron: &str, tz: Tz, now: DateTime<Utc>) -> Result<DateTime<Utc>, String> {
    let c: croner::Cron = cron
        .parse()
        .map_err(|e: croner::errors::CronError| e.to_string())?;
    c.find_next_occurrence(&now.with_timezone(&tz), false)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| e.to_string())
}

/// # Errors
/// If the schedule does not parse.
pub fn validate_cron(cron: &str) -> Result<(), String> {
    next_digest(cron, Tz::UTC, DateTime::UNIX_EPOCH).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DeliveryMode, Targets};

    fn sub(event: &str, user: bool, enabled: bool, locked: bool) -> Subscription {
        Subscription {
            id: uuid::Uuid::now_v7(),
            user: user.then(|| domain::ids::UserId(uuid::Uuid::nil())),
            event: event.into(),
            filter: Filter::default(),
            targets: Targets::All,
            mode: DeliveryMode::Immediate,
            quiet_hours: None,
            timezone: "UTC".into(),
            rate_limit_per_hour: None,
            enabled,
            locked,
        }
    }

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    #[test]
    fn event_patterns() {
        assert!(event_matches("*", "social.link_request"));
        assert!(event_matches("social.*", "social.link_request"));
        assert!(!event_matches("social.*", "socialite.x"));
        assert!(event_matches("release.episode", "release.episode"));
        assert!(!event_matches("release.episode", "release.chapter"));
    }

    #[test]
    fn filters_match_on_every_given_dimension() {
        let item = domain::ids::ItemId(uuid::Uuid::now_v7());
        let f = Filter {
            kinds: vec!["tv".into()],
            ..Filter::default()
        };
        let tv = Subject {
            kind: Some("tv".into()),
            item: Some(item),
            ..Subject::default()
        };
        assert!(filter_matches(&f, &tv));
        assert!(!filter_matches(&f, &Subject::default()));
        assert!(filter_matches(&Filter::default(), &Subject::default()));
    }

    #[test]
    fn own_subscriptions_override_unlocked_defaults() {
        let default_on = sub("social.*", false, true, false);
        let locked = sub("admin.*", false, true, true);
        let mute = sub("social.*", true, false, false);
        let try_unlock = sub("admin.*", true, false, false);
        let eff = effective(&[mute, try_unlock], &[default_on, locked.clone()]);
        assert_eq!(eff, vec![locked]);
    }

    #[test]
    fn quiet_hours_wrap_midnight_in_the_users_timezone() {
        let q = QuietHours {
            start: NaiveTime::from_hms_opt(22, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(7, 0, 0).unwrap(),
        };
        let berlin = timezone("Europe/Berlin");
        // 22:30 in Berlin (CET, UTC+1) on 2026-01-10 -> until 07:00 next morning
        assert_eq!(
            quiet_until(at("2026-01-10T21:30:00Z"), berlin, q),
            Some(at("2026-01-11T06:00:00Z"))
        );
        // 03:00 local -> until 07:00 the same day
        assert_eq!(
            quiet_until(at("2026-01-11T02:00:00Z"), berlin, q),
            Some(at("2026-01-11T06:00:00Z"))
        );
        assert_eq!(quiet_until(at("2026-01-11T12:00:00Z"), berlin, q), None);
    }

    #[test]
    fn digest_times_follow_the_timezone() {
        let berlin = timezone("Europe/Berlin");
        // every day at 08:00 Berlin = 07:00 UTC in winter, 06:00 UTC in summer
        assert_eq!(
            next_digest("0 8 * * *", berlin, at("2026-01-10T10:00:00Z")).unwrap(),
            at("2026-01-11T07:00:00Z")
        );
        assert_eq!(
            next_digest("0 8 * * *", berlin, at("2026-07-10T05:00:00Z")).unwrap(),
            at("2026-07-10T06:00:00Z")
        );
        assert!(next_digest("not a cron", berlin, at("2026-07-10T05:00:00Z")).is_err());
        assert_eq!(timezone("Mars/Olympus"), Tz::UTC);
    }
}
