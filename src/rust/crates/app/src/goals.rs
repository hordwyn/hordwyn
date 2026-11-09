//! Opt-in yearly goals (design.md 9, research R-04) and "this year so far against the same
//! period last year" (design.md 6.1, 6.6). Goals are private: every read and change is the
//! owner's own. Their progress is the year's sum from the statistics cache (WP4-1).

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, TimeZone};
use domain::Timestamp;
use domain::goal::{Goal, GoalMetric};
use domain::ids::GoalId;
use domain::kind::KindGroup;
use domain::user::Actor;

use crate::error::OptionExt;
use crate::ports::RepoError;
use crate::stats::Amounts;
use crate::time_zone::Tz;
use crate::{App, AppError, AppResult};

/// Input for [`App::set_goal`] and [`App::update_goal`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewGoal {
    pub year: i32,
    /// `None`: every group.
    pub group: Option<KindGroup>,
    pub metric: GoalMetric,
    pub target: u32,
}

/// A goal and how much of it is done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalProgress {
    pub goal: Goal,
    pub done: u64,
}

/// This year's amounts so far and last year's over the same days.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComparePair {
    pub this: Amounts,
    pub last: Amounts,
}

/// "This year so far vs the same period last year": January 1 to today in the user's zone,
/// and January 1 to the same day a year earlier (February 28 for a February 29).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearCompare {
    pub year: i32,
    /// Today in the user's zone (the last day counted this year).
    pub through: NaiveDate,
    /// The last day counted last year.
    pub last_through: NaiveDate,
    pub overall: ComparePair,
    /// Watch, Read, Listen and Play always; Other when it has activity.
    pub groups: Vec<(KindGroup, ComparePair)>,
}

/// The measure of a goal in an amount (minutes from seconds).
fn measure(m: GoalMetric, a: &Amounts, seconds: u64) -> u64 {
    match m {
        GoalMetric::Completions => a.completions,
        GoalMetric::Finished => a.finished,
        GoalMetric::Episodes => a.episodes,
        GoalMetric::Pages => a.pages,
        GoalMetric::Plays => a.plays,
        GoalMetric::Minutes => seconds / 60,
    }
}

/// The same day one year earlier; February 29 becomes February 28.
#[must_use]
pub fn same_day_last_year(d: NaiveDate) -> NaiveDate {
    let y = d.year() - 1;
    NaiveDate::from_ymd_opt(y, d.month(), d.day())
        .or_else(|| NaiveDate::from_ymd_opt(y, d.month(), d.day() - 1))
        .unwrap_or(d)
}

/// The instant a local day starts (the earlier instant of an ambiguous midnight, the first
/// valid hour of a skipped one).
fn day_start(d: NaiveDate, zone: Tz) -> Timestamp {
    let mut t = d.and_hms_opt(0, 0, 0).unwrap_or_default();
    for _ in 0..3 {
        if let Some(x) = zone.from_local_datetime(&t).earliest() {
            return x.with_timezone(&chrono::Utc);
        }
        t += chrono::Duration::hours(1);
    }
    chrono::Utc.from_utc_datetime(&t)
}

fn first_of_month(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

impl App {
    /// The sums per group of the months `from_month..=to_month` (`2026-01`).
    fn month_sums(
        &self,
        cached: &[crate::MonthAmounts],
        months: &(String, String),
    ) -> BTreeMap<KindGroup, (Amounts, u64)> {
        let mut out: BTreeMap<KindGroup, (Amounts, u64)> = BTreeMap::new();
        for m in cached
            .iter()
            .filter(|m| m.month >= months.0 && m.month <= months.1)
        {
            let (a, s) = out.entry(self.group_of(&m.kind)).or_default();
            a.add(&m.amounts);
            *s += m.seconds;
        }
        out
    }

    /// Every goal of the actor with its progress, newest year first, then by group and
    /// measure.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn goals(&self, actor: Actor) -> AppResult<Vec<GoalProgress>> {
        let goals = self.repos.users.goals(actor.user_id).await?;
        let zone = self.user_zone(actor.user_id).await?;
        let all = self
            .repos
            .progress
            .month_amounts(actor.user_id, zone.name())
            .await?;
        let mut out: Vec<GoalProgress> = goals
            .into_iter()
            .map(|goal| {
                let prefix = format!("{:04}-", goal.year);
                let (mut a, mut secs) = (Amounts::default(), 0);
                for m in all.iter().filter(|m| {
                    m.month.starts_with(&prefix)
                        && goal.group.is_none_or(|g| g == self.group_of(&m.kind))
                }) {
                    a.add(&m.amounts);
                    secs += m.seconds;
                }
                GoalProgress {
                    done: measure(goal.metric, &a, secs),
                    goal,
                }
            })
            .collect();
        out.sort_by_key(|g| {
            (
                std::cmp::Reverse(g.goal.year),
                g.goal.group.map_or(0, |g| {
                    1 + KindGroup::ALL.iter().position(|x| *x == g).unwrap_or(0)
                }),
                g.goal.metric,
            )
        });
        Ok(out)
    }

    async fn progress_of(&self, actor: Actor, id: GoalId) -> AppResult<GoalProgress> {
        self.goals(actor)
            .await?
            .into_iter()
            .find(|g| g.goal.id == id)
            .found("goal")
    }

    /// The actor's goal `id` (another user's goal is not found: goals are private).
    async fn own_goal(&self, actor: Actor, id: GoalId) -> AppResult<Goal> {
        self.repos
            .users
            .goal(id)
            .await?
            .filter(|g| g.user_id == actor.user_id)
            .found("goal")
    }

    async fn store_goal(&self, goal: &Goal) -> AppResult<()> {
        goal.validate()?;
        let taken = self
            .repos
            .users
            .goals(goal.user_id)
            .await?
            .iter()
            .any(|g| g.id != goal.id && g.same_slot(goal));
        if taken {
            return Err(AppError::Conflict(
                "there is a goal for this year, group and measure".into(),
            ));
        }
        self.repos
            .users
            .upsert_goal(goal)
            .await
            .map_err(|e| match e {
                RepoError::Duplicate(m) => AppError::Conflict(m),
                e @ RepoError::Backend(_) => e.into(),
            })
    }

    /// Set a goal for a year, a group (or all) and a measure.
    ///
    /// # Errors
    /// Invalid year or target; a goal for the same year, group and measure exists.
    pub async fn set_goal(&self, actor: Actor, new: NewGoal) -> AppResult<GoalProgress> {
        let goal = Goal {
            id: GoalId(self.uuid()),
            user_id: actor.user_id,
            year: new.year,
            group: new.group,
            metric: new.metric,
            target: new.target,
            created_at: self.now(),
        };
        self.store_goal(&goal).await?;
        self.progress_of(actor, goal.id).await
    }

    /// Change a goal.
    ///
    /// # Errors
    /// Not the actor's goal; as [`Self::set_goal`].
    pub async fn update_goal(
        &self,
        actor: Actor,
        id: GoalId,
        new: NewGoal,
    ) -> AppResult<GoalProgress> {
        let mut goal = self.own_goal(actor, id).await?;
        goal.year = new.year;
        goal.group = new.group;
        goal.metric = new.metric;
        goal.target = new.target;
        self.store_goal(&goal).await?;
        self.progress_of(actor, id).await
    }

    /// # Errors
    /// Not the actor's goal.
    pub async fn delete_goal(&self, actor: Actor, id: GoalId) -> AppResult<()> {
        let goal = self.own_goal(actor, id).await?;
        self.repos.users.delete_goal(goal.id).await?;
        Ok(())
    }

    /// The amounts per group from January 1 of `through`'s year to the end of `through`:
    /// whole months from the cache, the current month from its events.
    async fn year_to_date(
        &self,
        actor: Actor,
        (zone, cached): (Tz, &[crate::MonthAmounts]),
        through: NaiveDate,
    ) -> AppResult<BTreeMap<KindGroup, Amounts>> {
        let month = first_of_month(through);
        let mut sums = if through.month() > 1 {
            let before = month.pred_opt().unwrap_or(month);
            self.month_sums(
                cached,
                &(
                    format!("{:04}-01", through.year()),
                    before.format("%Y-%m").to_string(),
                ),
            )
        } else {
            BTreeMap::new()
        };
        let next = through.succ_opt().unwrap_or(through);
        for k in self
            .repos
            .progress
            .window_amounts(actor.user_id, day_start(month, zone), day_start(next, zone))
            .await?
        {
            let (a, s) = sums.entry(self.group_of(&k.kind)).or_default();
            a.add(&k.amounts);
            *s += k.seconds;
        }
        Ok(sums
            .into_iter()
            .map(|(g, (mut a, secs))| {
                a.minutes = secs / 60;
                (g, a)
            })
            .collect())
    }

    /// This year so far against the same period last year, overall and per group.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn year_compare(&self, actor: Actor) -> AppResult<YearCompare> {
        let zone = self.user_zone(actor.user_id).await?;
        let through = self.now().with_timezone(&zone).date_naive();
        let last_through = same_day_last_year(through);
        let cached = self
            .repos
            .progress
            .month_amounts(actor.user_id, zone.name())
            .await?;
        let this = self.year_to_date(actor, (zone, &cached), through).await?;
        let last = self
            .year_to_date(actor, (zone, &cached), last_through)
            .await?;
        let mut overall = ComparePair::default();
        let mut groups = Vec::new();
        for g in KindGroup::ALL {
            let pair = ComparePair {
                this: this.get(&g).copied().unwrap_or_default(),
                last: last.get(&g).copied().unwrap_or_default(),
            };
            if g == KindGroup::Other && pair == ComparePair::default() {
                continue;
            }
            overall.this.add(&pair.this);
            overall.last.add(&pair.last);
            groups.push((g, pair));
        }
        Ok(YearCompare {
            year: through.year(),
            through,
            last_through,
            overall,
            groups,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_days_map_to_february_28() {
        let d = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(same_day_last_year(d(2028, 2, 29)), d(2027, 2, 28));
        assert_eq!(same_day_last_year(d(2029, 2, 28)), d(2028, 2, 28));
        assert_eq!(same_day_last_year(d(2027, 1, 1)), d(2026, 1, 1));
    }
}
