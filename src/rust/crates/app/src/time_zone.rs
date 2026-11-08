//! The per-user time zone (DEC-239, DEC-211, DEC-230): days, "today", times and the month
//! buckets of the statistics follow the user's IANA zone; without one, UTC.

use chrono::{DateTime, NaiveDate, Utc};
use domain::ids::UserId;
use domain::user::{Actor, User};

pub use chrono_tz::Tz;

use crate::{App, AppResult};

/// An IANA zone name (`Europe/Berlin`), exactly as the tz database spells it.
#[must_use]
pub fn parse_zone(name: &str) -> Option<Tz> {
    name.parse::<Tz>().ok().filter(|z| z.name() == name)
}

/// The zone of `user`: their choice, else UTC.
#[must_use]
pub fn zone_of(user: &User) -> Tz {
    user.time_zone
        .as_deref()
        .and_then(parse_zone)
        .unwrap_or(Tz::UTC)
}

/// The local day of `t` in `zone` (`2026-02-01`).
#[must_use]
pub fn local_day(t: DateTime<Utc>, zone: Tz) -> String {
    t.with_timezone(&zone).format("%Y-%m-%d").to_string()
}

/// The local time of `t` in `zone` (`00:30`).
#[must_use]
pub fn local_time(t: DateTime<Utc>, zone: Tz) -> String {
    t.with_timezone(&zone).format("%H:%M").to_string()
}

impl App {
    /// The zone of a user (UTC for an unknown one).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn user_zone(&self, user: UserId) -> AppResult<Tz> {
        Ok(self.user_opt(user).await?.as_ref().map_or(Tz::UTC, zone_of))
    }

    /// Today in the user's zone (`ClientConfig.today`, "Today" headings).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn today_for(&self, user: UserId) -> AppResult<NaiveDate> {
        let zone = self.user_zone(user).await?;
        Ok(self.clock.now().with_timezone(&zone).date_naive())
    }

    /// Set the caller's zone (`None`: back to the default). The caller checks the name with
    /// [`parse_zone`].
    ///
    /// # Errors
    /// Storage errors.
    pub async fn set_time_zone(&self, actor: Actor, zone: Option<Tz>) -> AppResult<User> {
        let mut user = self.user(actor.user_id).await?;
        user.time_zone = zone.map(|z| z.name().to_string());
        self.repos.users.update(&user).await?;
        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_names_and_local_days() {
        let berlin = parse_zone("Europe/Berlin").unwrap();
        assert_eq!(parse_zone("Mars/Olympus"), None);
        assert_eq!(parse_zone("europe/berlin"), None, "exact spelling only");
        let t = DateTime::parse_from_rfc3339("2026-01-31T23:30:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(local_day(t, Tz::UTC), "2026-01-31");
        assert_eq!(
            (local_day(t, berlin), local_time(t, berlin)),
            ("2026-02-01".to_string(), "00:30".to_string())
        );
        // summer time: UTC+2
        let s = DateTime::parse_from_rfc3339("2026-07-01T22:15:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(local_time(s, berlin), "00:15");
    }
}
