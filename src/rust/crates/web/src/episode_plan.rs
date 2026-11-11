//! The episode list as plain data (design.md 6.3 Episodes tab, 5.4, 3.4 R-07): seasons with
//! their watched episodes, the ranges of the bulk actions ("Mark season watched", "Mark up to
//! here") and the remembered answer to "Also mark the earlier episodes?". The view lives in
//! [`crate::episode_list`].

use std::collections::BTreeSet;

use contracts::ProgressEventDto;

use crate::history_state::Day;

/// `(season, episode)`.
pub type Ep = (u32, u32);

/// One season of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonRow {
    pub number: u32,
    /// Episodes in the season (from the item, else the highest one watched).
    pub count: u32,
    pub watched: BTreeSet<u32>,
}

/// The seasons of an item: its `seasons` attribute (specials, season 0, left out, as the
/// next step does), else its `episode_count` as one season, plus any season only the history
/// knows; watched episodes from the effective history.
#[must_use]
pub fn season_rows(
    attributes: &serde_json::Value,
    progress: &[ProgressEventDto],
) -> Vec<SeasonRow> {
    let mut rows = item_seasons(attributes);
    for p in progress.iter().map(|e| &e.progress) {
        if p["type"] != "episode" {
            continue;
        }
        let (Some(s), Some(e)) = (u32_of(&p["season"]), u32_of(&p["episode"])) else {
            continue;
        };
        if s == 0 || e == 0 {
            continue;
        }
        match rows.iter_mut().find(|r| r.number == s) {
            Some(r) => {
                r.count = r.count.max(e);
                r.watched.insert(e);
            }
            None => rows.push(SeasonRow {
                number: s,
                count: e,
                watched: BTreeSet::from([e]),
            }),
        }
    }
    rows.sort_by_key(|r| r.number);
    rows
}

fn u32_of(v: &serde_json::Value) -> Option<u32> {
    v.as_u64().and_then(|n| u32::try_from(n).ok())
}

fn season(number: u32, count: u32) -> SeasonRow {
    SeasonRow {
        number,
        count,
        watched: BTreeSet::new(),
    }
}

/// The seasons the item knows, regular ones only.
fn item_seasons(attributes: &serde_json::Value) -> Vec<SeasonRow> {
    let mut rows: Vec<SeasonRow> = attributes
        .get("seasons")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| Some((u32_of(s.get("number")?)?, u32_of(s.get("episode_count")?)?)))
        .filter(|(n, c)| *n > 0 && *c > 0)
        .map(|(n, c)| season(n, c))
        .collect();
    if rows.is_empty()
        && let Some(count) = attributes
            .get("episode_count")
            .and_then(u32_of)
            .filter(|c| *c > 0)
    {
        rows.push(season(1, count));
    }
    rows
}

/// Unwatched episodes from the start up to and including `target`, in order ("Mark up to
/// here", "Also mark the earlier ones").
#[must_use]
pub fn unwatched_up_to(rows: &[SeasonRow], target: Ep) -> Vec<Ep> {
    rows.iter()
        .filter(|r| r.number <= target.0)
        .flat_map(|r| {
            let last = if r.number == target.0 {
                target.1.min(r.count)
            } else {
                r.count
            };
            (1..=last)
                .filter(|e| !r.watched.contains(e))
                .map(|e| (r.number, e))
        })
        .collect()
}

/// Unwatched episodes of one season ("Mark season watched").
#[must_use]
pub fn unwatched_in_season(rows: &[SeasonRow], season: u32) -> Vec<Ep> {
    rows.iter()
        .filter(|r| r.number == season)
        .flat_map(|r| {
            (1..=r.count)
                .filter(|e| !r.watched.contains(e))
                .map(|e| (r.number, e))
        })
        .collect()
}

/// Unwatched episodes of every season, in order ("Mark show").
#[must_use]
pub fn unwatched_in_show(rows: &[SeasonRow]) -> Vec<Ep> {
    rows.iter()
        .flat_map(|r| unwatched_in_season(rows, r.number))
        .collect()
}

/// A bulk mark: "Mark up to here" of an episode, "Mark season watched" of a season, "Mark
/// show watched".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bulk {
    UpTo(Ep),
    Season(u32),
    Show,
}

/// The id prefix of an episode row's menu (`ui::DropdownMenu`).
#[must_use]
pub fn row_menu_id((season, episode): Ep) -> String {
    format!("ep-menu-{season}-{episode}")
}

impl Bulk {
    /// A selector for the control that starts this mark: the row's menu button, or the
    /// season's or the show's button (`data-ep-bulk`). The focus returns there when its
    /// question is answered (review OT2A-4).
    #[must_use]
    pub fn asker(self) -> String {
        match self {
            Self::UpTo(ep) => format!("[aria-controls=\"{}-menu\"]", row_menu_id(ep)),
            Self::Season(_) => "[data-ep-bulk=\"season\"]".into(),
            Self::Show => "[data-ep-bulk=\"show\"]".into(),
        }
    }
}

/// What a bulk mark does (OT-15, DEC-538: never a click that does nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BulkPlan {
    /// Log the unwatched episodes it covers; `skipped` more are watched already and stay as
    /// they are (the toast says how many).
    Log { eps: Vec<Ep>, skipped: usize },
    /// Every episode it covers is watched: ask "Log these N again as a rewatch?".
    Again(Vec<Ep>),
    /// It covers no episode (a season the item does not have).
    Nothing,
}

/// Every episode a bulk mark covers, watched or not, in order.
fn covered(rows: &[SeasonRow], bulk: Bulk) -> Vec<Ep> {
    rows.iter()
        .filter_map(|r| {
            let last = match bulk {
                Bulk::UpTo((season, _)) if r.number > season => return None,
                Bulk::UpTo((season, episode)) if r.number == season => episode.min(r.count),
                Bulk::Season(season) if r.number != season => return None,
                Bulk::UpTo(_) | Bulk::Season(_) | Bulk::Show => r.count,
            };
            Some((1..=last).map(move |e| (r.number, e)))
        })
        .flatten()
        .collect()
}

/// The plan of a bulk mark: the unwatched episodes it covers and how many it skips, or the
/// whole range to offer again when all of it is watched.
#[must_use]
pub fn plan_bulk(rows: &[SeasonRow], bulk: Bulk) -> BulkPlan {
    let all = covered(rows, bulk);
    let watched = |(s, e): &Ep| rows.iter().any(|r| r.number == *s && r.watched.contains(e));
    let eps: Vec<Ep> = all.iter().filter(|ep| !watched(ep)).copied().collect();
    if all.is_empty() {
        BulkPlan::Nothing
    } else if eps.is_empty() {
        BulkPlan::Again(all)
    } else {
        BulkPlan::Log {
            skipped: all.len() - eps.len(),
            eps,
        }
    }
}

/// Why a backfill range cannot be used; each names its Fluent message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeError {
    Missing,
    Reversed,
    Future,
}

impl RangeError {
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Missing => "episodes-range-missing",
            Self::Reversed => "episodes-range-reversed",
            Self::Future => "episodes-range-future",
        }
    }
}

/// The days of a backfill range ("Spread over a date range", design.md 5.4 R-09): both
/// `YYYY-MM-DD`, in order, not after today.
///
/// # Errors
/// A missing or malformed day, the end before the start, a day after today.
pub fn parse_range(from: &str, to: &str, today: Day) -> Result<(Day, Day), RangeError> {
    let (Some(a), Some(b)) = (Day::parse(from.trim()), Day::parse(to.trim())) else {
        return Err(RangeError::Missing);
    };
    if b.days() < a.days() {
        return Err(RangeError::Reversed);
    }
    if b.days() > today.days() {
        return Err(RangeError::Future);
    }
    Ok((a, b))
}

/// When each of `n` bulk-marked episodes happened, spread evenly from `from` to `to`
/// (first and last on the range's ends, noon UTC like the log sheet's picked days), oldest
/// first.
#[must_use]
pub fn spread_over(n: usize, from: Day, to: Day) -> Vec<String> {
    let span = to.days() - from.days();
    let steps = i64::try_from(n.saturating_sub(1))
        .unwrap_or(i64::MAX)
        .max(1);
    (0..n)
        .map(|i| {
            let i = i64::try_from(i).unwrap_or(0);
            let day = Day::from_days(from.days() + span * i / steps);
            format!("{}T12:00:00Z", day.iso())
        })
        .collect()
}

/// Release day of each episode the item has a date for (WP4-7).
pub type AirDates = std::collections::BTreeMap<Ep, Day>;

/// The episodes' release days from the item page (bad or missing dates left out).
#[must_use]
pub fn air_dates(episodes: &[contracts::EpisodeDateDto]) -> AirDates {
    episodes
        .iter()
        .filter_map(|e| Some(((e.season, e.episode), Day::parse(e.air_date.as_deref()?)?)))
        .collect()
}

/// How many of `eps` have no release day up to `today` (none known, or still to come).
#[must_use]
pub fn missing_release(eps: &[Ep], dates: &AirDates, today: Day) -> usize {
    eps.iter()
        .filter(|e| dates.get(e).is_none_or(|d| d.days() > today.days()))
        .count()
}

/// "On release dates" (design.md 5.4 R-09): each episode at noon UTC of its release day, in
/// the order given.
///
/// # Errors
/// The number of episodes without a release day up to today; then nothing is logged
/// (episodes still to come are refused).
pub fn release_dates(eps: &[Ep], dates: &AirDates, today: Day) -> Result<Vec<String>, usize> {
    match missing_release(eps, dates, today) {
        0 => Ok(eps
            .iter()
            .filter_map(|e| dates.get(e))
            .map(|d| format!("{}T12:00:00Z", d.iso()))
            .collect()),
        n => Err(n),
    }
}

/// The remembered answer to "Also mark the earlier episodes?" for one show (per person and
/// browser, DEC in docs/DECISIONS.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Previous {
    /// Not answered with "remember": ask.
    #[default]
    Ask,
    /// Always mark the earlier unwatched episodes too.
    All,
    /// Only ever mark the one clicked.
    Only,
}

impl Previous {
    #[must_use]
    pub fn parse(stored: Option<&str>) -> Self {
        match stored {
            Some("all") => Self::All,
            Some("only") => Self::Only,
            _ => Self::Ask,
        }
    }

    /// The stored value; `None` for [`Previous::Ask`] (nothing stored).
    #[must_use]
    pub fn as_stored(self) -> Option<&'static str> {
        match self {
            Self::Ask => None,
            Self::All => Some("all"),
            Self::Only => Some("only"),
        }
    }
}

/// `localStorage` key of the answer for one show.
#[must_use]
pub fn previous_key(item_id: &str) -> String {
    format!("app-episodes-previous:{item_id}")
}

/// What one episode's "Watched" does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Log these episodes now.
    Log(Vec<Ep>),
    /// Ask first: there are `earlier` unwatched episodes before it.
    Ask { earlier: usize },
}

/// The plan for marking `target`: alone when nothing earlier is unwatched, else as the
/// remembered answer says, else ask.
#[must_use]
pub fn plan_single(rows: &[SeasonRow], target: Ep, answer: Previous) -> Plan {
    let watched = rows
        .iter()
        .any(|r| r.number == target.0 && r.watched.contains(&target.1));
    let all = unwatched_up_to(rows, target);
    let earlier = all.iter().filter(|e| **e != target).count();
    if watched || earlier == 0 {
        return Plan::Log(vec![target]);
    }
    match answer {
        Previous::All => Plan::Log(all),
        Previous::Only => Plan::Log(vec![target]),
        Previous::Ask => Plan::Ask { earlier },
    }
}

/// The episodes of the question "Log these N again?" of the bulk mark `asked`, for as long as
/// the rows still say that all of its range is watched: after a refetch with an open episode
/// in it the question is gone (OT1A-2).
#[must_use]
pub fn standing_again(rows: &[SeasonRow], asked: Option<Bulk>) -> Option<Vec<Ep>> {
    match plan_bulk(rows, asked?) {
        BulkPlan::Again(eps) => Some(eps),
        BulkPlan::Log { .. } | BulkPlan::Nothing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review OT2A-4: each bulk mark names the control that asked.
    #[test]
    fn a_bulk_mark_knows_the_control_that_asked() {
        assert_eq!(
            Bulk::UpTo((2, 5)).asker(),
            "[aria-controls=\"ep-menu-2-5-menu\"]"
        );
        assert_eq!(Bulk::Season(3).asker(), "[data-ep-bulk=\"season\"]");
        assert_eq!(Bulk::Show.asker(), "[data-ep-bulk=\"show\"]");
    }

    fn ev(n: u32, progress: serde_json::Value) -> ProgressEventDto {
        ProgressEventDto {
            id: format!("e{n}"),
            item_id: "i".into(),
            occurred_at: "2026-01-01T00:00:00Z".into(),
            progress,
        }
    }

    fn ep(n: u32, s: u32, e: u32) -> ProgressEventDto {
        ev(
            n,
            serde_json::json!({"type": "episode", "season": s, "episode": e}),
        )
    }

    fn rows() -> Vec<SeasonRow> {
        let attrs = serde_json::json!({"seasons": [
            {"number": 0, "episode_count": 3},
            {"number": 2, "episode_count": 4},
            {"number": 1, "episode_count": 3},
        ]});
        season_rows(
            &attrs,
            &[ep(1, 1, 1), ep(2, 1, 3), ep(3, 2, 2), ep(4, 1, 1)],
        )
    }

    #[test]
    fn seasons_come_from_the_item_and_the_history() {
        let r = rows();
        assert_eq!(r.len(), 2, "specials left out: {r:?}");
        assert_eq!((r[0].number, r[0].count), (1, 3));
        assert_eq!(r[0].watched, BTreeSet::from([1, 3]));
        assert_eq!((r[1].number, r[1].count), (2, 4));
        assert_eq!(r[1].watched, BTreeSet::from([2]));
        // without a season list: the episode count is one season
        let one = season_rows(&serde_json::json!({"episode_count": 12}), &[]);
        assert_eq!(one.len(), 1);
        assert_eq!((one[0].number, one[0].count), (1, 12));
        // a season only the history knows, sized by the highest episode watched
        let more = season_rows(&serde_json::json!({}), &[ep(1, 3, 7), ep(2, 3, 2)]);
        assert_eq!(more.len(), 1);
        assert_eq!((more[0].number, more[0].count), (3, 7));
        // other progress types are ignored
        let odd = season_rows(
            &serde_json::json!({}),
            &[ev(1, serde_json::json!({"type": "finished"}))],
        );
        assert_eq!(odd.len(), 0);
    }

    #[test]
    fn marking_up_to_here_takes_every_earlier_gap() {
        let r = rows();
        assert_eq!(unwatched_up_to(&r, (2, 3)), vec![(1, 2), (2, 1), (2, 3)]);
        assert_eq!(unwatched_up_to(&r, (1, 2)), vec![(1, 2)]);
        // a watched target adds nothing for itself
        assert_eq!(unwatched_up_to(&r, (1, 3)), vec![(1, 2)]);
        assert_eq!(unwatched_up_to(&r, (1, 1)), Vec::<Ep>::new());
    }

    #[test]
    fn marking_a_season_takes_its_gaps_only() {
        let r = rows();
        assert_eq!(unwatched_in_season(&r, 1), vec![(1, 2)]);
        assert_eq!(unwatched_in_season(&r, 2), vec![(2, 1), (2, 3), (2, 4)]);
        assert_eq!(unwatched_in_season(&r, 9), Vec::<Ep>::new());
    }

    #[test]
    fn marking_the_show_takes_every_gap() {
        let r = rows();
        assert_eq!(unwatched_in_show(&r), vec![(1, 2), (2, 1), (2, 3), (2, 4)]);
    }

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    #[test]
    fn a_range_spreads_the_episodes_evenly() {
        let today = day("2026-03-10");
        let (a, b) = parse_range("2026-03-01", "2026-03-09", today).unwrap();
        assert_eq!(
            spread_over(5, a, b),
            [
                "2026-03-01T12:00:00Z",
                "2026-03-03T12:00:00Z",
                "2026-03-05T12:00:00Z",
                "2026-03-07T12:00:00Z",
                "2026-03-09T12:00:00Z"
            ]
        );
        // more episodes than days: several on one day, still in order
        let (a, b) = parse_range("2026-02-27", "2026-03-01", today).unwrap();
        let days = spread_over(6, a, b);
        assert_eq!(
            days.first().map(String::as_str),
            Some("2026-02-27T12:00:00Z")
        );
        assert_eq!(
            days.last().map(String::as_str),
            Some("2026-03-01T12:00:00Z")
        );
        assert!(days.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(spread_over(1, a, b), ["2026-02-27T12:00:00Z"]);
        assert_eq!(spread_over(0, a, b).len(), 0);
        assert_eq!(
            parse_range("", "2026-03-01", today),
            Err(RangeError::Missing)
        );
        assert_eq!(
            parse_range("2026-03-05", "2026-03-01", today),
            Err(RangeError::Reversed)
        );
        assert_eq!(
            parse_range("2026-03-05", "2026-03-11", today),
            Err(RangeError::Future)
        );
        assert_eq!(RangeError::Future.key(), "episodes-range-future");
    }

    fn dated(dates: &[(u32, u32, Option<&str>)]) -> AirDates {
        air_dates(
            &dates
                .iter()
                .map(|(s, e, d)| contracts::EpisodeDateDto {
                    season: *s,
                    episode: *e,
                    air_date: d.map(str::to_string),
                })
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn release_dates_log_each_episode_on_its_day_and_refuse_missing_or_future_ones() {
        let today = day("2026-03-10");
        let dates = dated(&[
            (1, 1, Some("2011-04-17")),
            (1, 2, Some("2011-04-24")),
            (1, 3, None),
            (2, 1, Some("2026-03-10")),
            (2, 2, Some("2026-03-11")),
            (2, 3, Some("not a day")),
        ]);
        assert_eq!(
            release_dates(&[(1, 2), (1, 1)], &dates, today),
            Ok(vec![
                "2011-04-24T12:00:00Z".to_string(),
                "2011-04-17T12:00:00Z".to_string()
            ]),
            "one date per episode, in the order given"
        );
        // today counts as released; tomorrow, no date, a bad date and an unknown episode do not
        assert_eq!(
            release_dates(&[(2, 1)], &dates, today),
            Ok(vec!["2026-03-10T12:00:00Z".to_string()])
        );
        assert_eq!(
            release_dates(&[(1, 1), (1, 3), (2, 2), (2, 3), (4, 1)], &dates, today),
            Err(4)
        );
        assert_eq!(missing_release(&[(1, 1), (1, 2)], &dates, today), 0);
        assert_eq!(missing_release(&[(1, 1), (2, 2)], &dates, today), 1);
        assert_eq!(release_dates(&[], &dates, today), Ok(Vec::new()));
    }

    #[test]
    fn one_episode_asks_once_about_the_earlier_ones() {
        let r = rows();
        // nothing earlier is open: log it alone, no question
        assert_eq!(
            plan_single(&r, (1, 2), Previous::Ask),
            Plan::Log(vec![(1, 2)])
        );
        // gaps before it: ask, unless the answer is remembered
        assert_eq!(
            plan_single(&r, (2, 3), Previous::Ask),
            Plan::Ask { earlier: 2 }
        );
        assert_eq!(
            plan_single(&r, (2, 3), Previous::All),
            Plan::Log(vec![(1, 2), (2, 1), (2, 3)])
        );
        assert_eq!(
            plan_single(&r, (2, 3), Previous::Only),
            Plan::Log(vec![(2, 3)])
        );
        // a rewatch of a watched episode logs it again, alone
        assert_eq!(
            plan_single(&r, (2, 2), Previous::Ask),
            Plan::Log(vec![(2, 2)])
        );
    }

    #[test]
    fn the_answer_is_remembered_per_show() {
        assert_eq!(previous_key("abc"), "app-episodes-previous:abc");
        assert_eq!(Previous::parse(Some("all")), Previous::All);
        assert_eq!(Previous::parse(Some("only")), Previous::Only);
        assert_eq!(Previous::parse(Some("junk")), Previous::Ask);
        assert_eq!(Previous::parse(None), Previous::Ask);
        assert_eq!(Previous::All.as_stored(), Some("all"));
        assert_eq!(Previous::Only.as_stored(), Some("only"));
        assert_eq!(Previous::Ask.as_stored(), None);
    }

    /// OT-15 (DEC-538): a fully watched range offers N to log again, a mixed range reports
    /// the skipped count.
    #[test]
    fn a_fully_watched_range_offers_to_log_again_a_mixed_range_reports_the_skipped() {
        // season 1: 1 and 3 of 3 watched; season 2: 2 of 4 watched
        let r = rows();
        assert_eq!(
            plan_bulk(&r, Bulk::UpTo((1, 1))),
            BulkPlan::Again(vec![(1, 1)])
        );
        // a mixed range logs the open ones and counts the rest
        assert_eq!(
            plan_bulk(&r, Bulk::UpTo((1, 3))),
            BulkPlan::Log {
                eps: vec![(1, 2)],
                skipped: 2
            }
        );
        assert_eq!(
            plan_bulk(&r, Bulk::Season(1)),
            BulkPlan::Log {
                eps: vec![(1, 2)],
                skipped: 2
            }
        );
        assert_eq!(
            plan_bulk(&r, Bulk::Show),
            BulkPlan::Log {
                eps: vec![(1, 2), (2, 1), (2, 3), (2, 4)],
                skipped: 3
            }
        );
        // a season the item does not have covers nothing
        assert_eq!(plan_bulk(&r, Bulk::Season(9)), BulkPlan::Nothing);
        let attrs = serde_json::json!({"seasons": [
            {"number": 1, "episode_count": 2},
            {"number": 2, "episode_count": 1},
        ]});
        // nothing watched: nothing skipped
        assert_eq!(
            plan_bulk(&season_rows(&attrs, &[]), Bulk::Season(1)),
            BulkPlan::Log {
                eps: vec![(1, 1), (1, 2)],
                skipped: 0
            }
        );
        // everything watched: each mark offers its whole range again, in order
        let done = season_rows(&attrs, &[ep(1, 1, 1), ep(2, 1, 2), ep(3, 2, 1)]);
        let all = vec![(1, 1), (1, 2), (2, 1)];
        assert_eq!(
            plan_bulk(&done, Bulk::UpTo((2, 1))),
            BulkPlan::Again(all.clone())
        );
        assert_eq!(
            plan_bulk(&done, Bulk::Season(1)),
            BulkPlan::Again(vec![(1, 1), (1, 2)])
        );
        assert_eq!(plan_bulk(&done, Bulk::Show), BulkPlan::Again(all.clone()));
        // the question stands only while the rows still say that all of it is watched
        // (OT1A-2): after a refetch with an open episode, or with no mark asked, it is gone
        assert_eq!(standing_again(&done, Some(Bulk::Show)), Some(all));
        let undone = season_rows(&attrs, &[ep(1, 1, 1), ep(3, 2, 1)]);
        assert_eq!(standing_again(&undone, Some(Bulk::Show)), None);
        assert_eq!(standing_again(&undone, Some(Bulk::Season(1))), None);
        assert_eq!(
            standing_again(&undone, Some(Bulk::Season(2))),
            Some(vec![(2, 1)])
        );
        assert_eq!(standing_again(&done, None), None);
    }
}
