//! The History page's state in the URL and its pure helpers (design.md 6.6): filters
//! (`group`, `kind`, `person`, `type`, `shared`, `year`), day groups with Today and
//! Yesterday, and the side counts for the rows on screen. Days are the viewer's days: the
//! server sends each row's `local_day` and Today in the user's time zone (DEC-306), so the
//! server and the browser agree on them without a time zone database in the browser.

use contracts::{ActivityQueryDto, ActivityRowDto};

use crate::i18n::I18n;
use crate::library_state::encode;

/// Rows per request; "Load more" asks for the next page.
pub const HISTORY_PAGE: u32 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HistoryState {
    /// `None`: all groups.
    pub group: Option<String>,
    pub kind: Option<String>,
    /// Companion id.
    pub person: Option<String>,
    /// `progress` or `session`.
    pub row_type: Option<String>,
    /// Only sessions shared with me.
    pub shared: bool,
    pub year: Option<i32>,
}

fn clean(v: Option<String>) -> Option<String> {
    v.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

impl HistoryState {
    /// From the query parameters (first value of each); unknown values are ignored.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            group: clean(get("group"))
                .filter(|g| crate::library_state::GROUP_IDS.contains(&g.as_str())),
            kind: clean(get("kind")),
            person: clean(get("person")).filter(|p| uuid::Uuid::parse_str(p).is_ok()),
            row_type: clean(get("type")).filter(|t| t == "progress" || t == "session"),
            shared: clean(get("shared")).is_some_and(|s| s == "1" || s == "true"),
            year: clean(get("year"))
                .and_then(|y| y.parse().ok())
                .filter(|y| (1000..=9999).contains(y)),
        }
    }

    #[must_use]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self::from_lookup(|k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| (*v).to_string())
        })
    }

    /// The query string in a fixed order, without the `?`; empty for the default state.
    #[must_use]
    pub fn query(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (name, value) in [
            ("group", &self.group),
            ("kind", &self.kind),
            ("person", &self.person),
            ("type", &self.row_type),
        ] {
            if let Some(v) = value {
                parts.push(format!("{name}={}", encode(v)));
            }
        }
        if self.shared {
            parts.push("shared=1".into());
        }
        if let Some(y) = self.year {
            parts.push(format!("year={y}"));
        }
        parts.join("&")
    }

    #[must_use]
    pub fn href(&self) -> String {
        let q = self.query();
        if q.is_empty() {
            "/history".into()
        } else {
            format!("/history?{q}")
        }
    }

    /// The server query for one page.
    #[must_use]
    pub fn query_dto(&self, before: Option<String>) -> ActivityQueryDto {
        ActivityQueryDto {
            before,
            limit: Some(HISTORY_PAGE),
            group: self.group.clone(),
            kind: self.kind.clone(),
            person: self.person.clone(),
            row_type: self.row_type.clone(),
            shared: self.shared,
            year: self.year,
        }
    }

    /// Filters behind the Filters button that are set (the group switcher is open).
    #[must_use]
    pub fn hidden_filters(&self) -> usize {
        [
            self.kind.is_some(),
            self.person.is_some(),
            self.row_type.is_some(),
            self.shared,
            self.year.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count()
    }

    /// Another group drops a kind of the old group.
    #[must_use]
    pub fn with_group(&self, group: Option<String>) -> Self {
        let group = group.filter(|g| g != "all");
        Self {
            kind: if group == self.group {
                self.kind.clone()
            } else {
                None
            },
            group,
            ..self.clone()
        }
    }
}

/// A calendar day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Day {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Day {
    /// The day of a server timestamp (`2026-11-06T21:05:00Z` or `2026-11-06`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (y, m, d) = (s.get(0..4)?, s.get(5..7)?, s.get(8..10)?);
        if s.get(4..5) != Some("-") || s.get(7..8) != Some("-") {
            return None;
        }
        let day = Self {
            year: y.parse().ok()?,
            month: m.parse().ok().filter(|m| (1..=12).contains(m))?,
            day: d.parse().ok().filter(|d| (1..=31).contains(d))?,
        };
        (Self::from_days(day.days()) == day).then_some(day)
    }

    /// Days since 1970-01-01 (proleptic Gregorian; H. Hinnant's `days_from_civil`).
    #[must_use]
    pub fn days(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.month);
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The inverse of [`Day::days`] (`civil_from_days`).
    #[must_use]
    pub fn from_days(days: i64) -> Self {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        Self {
            year: i32::try_from(year).unwrap_or(1970),
            month: u32::try_from(month).unwrap_or(1),
            day: u32::try_from(day).unwrap_or(1),
        }
    }

    /// ISO weekday: 1 Monday .. 7 Sunday (1970-01-01 was a Thursday).
    #[must_use]
    pub fn weekday(self) -> u32 {
        u32::try_from((self.days() + 3).rem_euclid(7) + 1).unwrap_or(1)
    }

    /// `2026-11-06`.
    #[must_use]
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Today in UTC from the clock (server side; the browser reads the server's answer).
    #[must_use]
    pub fn today_utc() -> Self {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Self::from_days(i64::try_from(secs / 86_400).unwrap_or(0))
    }
}

/// How a day group is headed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayLabel {
    Today,
    Yesterday,
    /// Another day; `same_year` with today (the heading may leave the year out).
    Date {
        same_year: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DayGroup {
    pub day: Day,
    pub label: DayLabel,
    pub rows: Vec<ActivityRowDto>,
}

/// The day of a row in the viewer's time zone: the server's `local_day` (DEC-306), else the
/// UTC day of `at` (older servers, tests).
#[must_use]
pub fn row_day(r: &ActivityRowDto) -> Option<Day> {
    Day::parse(&r.local_day).or_else(|| Day::parse(&r.at))
}

/// Consecutive rows of the same day, in feed order (newest first).
#[must_use]
pub fn group_by_day(rows: Vec<ActivityRowDto>, today: Day) -> Vec<DayGroup> {
    let mut groups: Vec<DayGroup> = Vec::new();
    for r in rows {
        let Some(day) = row_day(&r) else {
            continue;
        };
        match groups.last_mut() {
            Some(g) if g.day == day => g.rows.push(r),
            _ => {
                let label = match today.days() - day.days() {
                    0 => DayLabel::Today,
                    1 => DayLabel::Yesterday,
                    _ => DayLabel::Date {
                        same_year: day.year == today.year,
                    },
                };
                groups.push(DayGroup {
                    day,
                    label,
                    rows: vec![r],
                });
            }
        }
    }
    groups
}

/// Counts for the rows on screen (the side panel).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SideCounts {
    pub rows: u64,
    pub finished: u64,
    pub episodes: u64,
    pub pages_rows: u64,
    pub sessions: u64,
    pub shared: u64,
    /// `(group, rows)` in the order groups first appear.
    pub groups: Vec<(String, u64)>,
    /// Oldest and newest day on screen.
    pub range: Option<(Day, Day)>,
}

#[must_use]
pub fn side_counts(rows: &[ActivityRowDto]) -> SideCounts {
    let mut c = SideCounts::default();
    for r in rows {
        c.rows += 1;
        match r.progress.as_ref().and_then(|p| p["type"].as_str()) {
            Some("finished") => c.finished += 1,
            Some("episode") => c.episodes += 1,
            Some("paged") => c.pages_rows += 1,
            _ => {}
        }
        if r.row_type == "session" {
            c.sessions += 1;
        }
        if r.shared {
            c.shared += 1;
        }
        match c.groups.iter_mut().find(|(g, _)| *g == r.group) {
            Some((_, n)) => *n += 1,
            None => c.groups.push((r.group.clone(), 1)),
        }
        if let Some(d) = row_day(r) {
            c.range = Some(c.range.map_or((d, d), |(a, b)| (a.min(d), b.max(d))));
        }
    }
    c
}

/// The heading of a day group: Today, Yesterday, else the weekday and the date (the year
/// only when it is not this year), in the locale's format.
#[must_use]
pub fn day_heading(i: &I18n, group: &DayGroup) -> String {
    let iso = group.day.iso();
    match group.label {
        DayLabel::Today => i.t("history-today"),
        DayLabel::Yesterday => i.t("history-yesterday"),
        DayLabel::Date { same_year } => {
            let weekday = match group.day.weekday() {
                1 => "weekday-monday",
                2 => "weekday-tuesday",
                3 => "weekday-wednesday",
                4 => "weekday-thursday",
                5 => "weekday-friday",
                6 => "weekday-saturday",
                _ => "weekday-sunday",
            };
            let date = if same_year {
                i.day_month(&iso)
            } else {
                i.date(&iso)
            };
            i.t_args(
                "history-day",
                &[("weekday", i.t(weekday).into()), ("date", date.into())],
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(at: &str, group: &str, progress: Option<serde_json::Value>) -> ActivityRowDto {
        ActivityRowDto {
            at: at.into(),
            local_day: String::new(),
            local_time: String::new(),
            row_type: if progress.is_some() {
                "progress"
            } else {
                "session"
            }
            .into(),
            item_id: "i".into(),
            title: "T".into(),
            kind: "movie".into(),
            group: group.into(),
            progress,
            event_id: None,
            session_id: None,
            participants: vec![],
            play: None,
            shared: false,
            owner_name: None,
            undoable: false,
        }
    }

    /// DEC-306: a play at 23:30 UTC on 6 Nov is on 7 Nov in Berlin; the server says so.
    #[test]
    fn rows_group_by_the_viewers_local_day() {
        let mut late = row("2026-11-06T23:30:00Z", "watch", None);
        late.local_day = "2026-11-07".into();
        late.local_time = "00:30".into();
        let today = Day::parse("2026-11-07").unwrap();
        let groups = group_by_day(
            vec![late, row("2026-11-06T10:00:00Z", "watch", None)],
            today,
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].label, DayLabel::Today);
        assert_eq!(groups[1].label, DayLabel::Yesterday);
    }

    #[test]
    fn the_url_round_trips_in_a_fixed_order() {
        let s = HistoryState::from_pairs(&[
            ("year", "2025"),
            ("shared", "1"),
            ("type", "session"),
            ("person", "01900000-0000-7000-8000-0000000000c1"),
            ("kind", "movie"),
            ("group", "watch"),
        ]);
        assert_eq!(
            s.query(),
            "group=watch&kind=movie&person=01900000-0000-7000-8000-0000000000c1&type=session&shared=1&year=2025"
        );
        assert_eq!(
            s.href(),
            "/history?group=watch&kind=movie&person=01900000-0000-7000-8000-0000000000c1&type=session&shared=1&year=2025"
        );
        let back = HistoryState::from_pairs(&[
            ("group", "watch"),
            ("kind", "movie"),
            ("person", "01900000-0000-7000-8000-0000000000c1"),
            ("type", "session"),
            ("shared", "1"),
            ("year", "2025"),
        ]);
        assert_eq!(back, s);
        assert_eq!(s.hidden_filters(), 5);
        assert_eq!(HistoryState::default().href(), "/history");
        // unknown and empty values are ignored; `all` is the default group
        let odd = HistoryState::from_pairs(&[
            ("group", "all"),
            ("type", "nonsense"),
            ("year", "soon"),
            ("shared", "no"),
            ("kind", " "),
        ]);
        assert_eq!(odd, HistoryState::default());
        assert_eq!(
            HistoryState::from_pairs(&[("kind", "a b")]).query(),
            "kind=a%20b"
        );
    }

    #[test]
    fn the_state_becomes_the_server_query() {
        let s = HistoryState::from_pairs(&[("group", "read"), ("shared", "1"), ("year", "2026")]);
        let q = s.query_dto(Some("cursor".into()));
        assert_eq!(q.group.as_deref(), Some("read"));
        assert!(q.shared);
        assert_eq!(q.year, Some(2026));
        assert_eq!(q.before.as_deref(), Some("cursor"));
        assert_eq!(q.limit, Some(HISTORY_PAGE));
        let g = HistoryState::from_pairs(&[("group", "watch"), ("kind", "movie")])
            .with_group(Some("read".into()));
        assert_eq!(g.kind, None, "a kind of another group is dropped");
        assert_eq!(
            HistoryState::default().with_group(Some("all".into())).group,
            None
        );
    }

    #[test]
    fn calendar_maths() {
        let d = Day::parse("2026-11-06T21:05:00Z").unwrap();
        assert_eq!(
            d,
            Day {
                year: 2026,
                month: 11,
                day: 6
            }
        );
        assert_eq!(d.weekday(), 5, "a Friday");
        assert_eq!(Day::parse("2026-11-01").unwrap().weekday(), 7, "a Sunday");
        assert_eq!(Day::from_days(d.days()), d);
        assert_eq!(Day::from_days(0).iso(), "1970-01-01");
        assert_eq!(Day::from_days(d.days() - 6).iso(), "2026-10-31");
        assert_eq!(
            Day::from_days(Day::parse("2024-03-01").unwrap().days() - 1).iso(),
            "2024-02-29"
        );
        assert_eq!(Day::parse("nonsense"), None);
        assert_eq!(Day::parse("2026-13-01"), None);
    }

    #[test]
    fn rows_group_by_day_with_today_and_yesterday() {
        let today = Day::parse("2026-11-07").unwrap();
        let rows = vec![
            row("2026-11-07T09:00:00Z", "watch", None),
            row("2026-11-07T08:00:00Z", "read", None),
            row("2026-11-06T23:59:00Z", "watch", None),
            row("2026-11-01T10:00:00Z", "play", None),
            row("2025-12-31T10:00:00Z", "play", None),
        ];
        let groups = group_by_day(rows, today);
        let shape: Vec<(String, DayLabel, usize)> = groups
            .iter()
            .map(|g| (g.day.iso(), g.label, g.rows.len()))
            .collect();
        assert_eq!(
            shape,
            [
                ("2026-11-07".to_string(), DayLabel::Today, 2),
                ("2026-11-06".to_string(), DayLabel::Yesterday, 1),
                (
                    "2026-11-01".to_string(),
                    DayLabel::Date { same_year: true },
                    1
                ),
                (
                    "2025-12-31".to_string(),
                    DayLabel::Date { same_year: false },
                    1
                ),
            ]
        );
        assert_eq!(group_by_day(vec![], today).len(), 0);
    }

    #[test]
    fn the_side_panel_counts_what_is_on_screen() {
        let mut shared = row("2026-11-01T10:00:00Z", "play", None);
        shared.shared = true;
        let rows = vec![
            row(
                "2026-11-07T09:00:00Z",
                "watch",
                Some(serde_json::json!({"type": "episode", "season": 1, "episode": 2})),
            ),
            row(
                "2026-11-06T09:00:00Z",
                "watch",
                Some(serde_json::json!({"type": "finished"})),
            ),
            row(
                "2026-11-05T09:00:00Z",
                "read",
                Some(serde_json::json!({"type": "paged", "page": 40})),
            ),
            shared,
        ];
        let c = side_counts(&rows);
        assert_eq!(c.rows, 4);
        assert_eq!(c.episodes, 1);
        assert_eq!(c.finished, 1);
        assert_eq!(c.pages_rows, 1);
        assert_eq!(c.sessions, 1);
        assert_eq!(c.shared, 1);
        assert_eq!(
            c.groups,
            [
                ("watch".to_string(), 2),
                ("read".to_string(), 1),
                ("play".to_string(), 1)
            ]
        );
        let (from, to) = c.range.unwrap();
        assert_eq!(
            (from.iso(), to.iso()),
            ("2026-11-01".into(), "2026-11-07".into())
        );
        assert_eq!(side_counts(&[]), SideCounts::default());
    }

    #[test]
    fn day_headings_in_each_locale() {
        use std::collections::BTreeMap;
        let today = Day::parse("2026-11-07").unwrap();
        let rows = vec![
            row("2026-11-07T09:00:00Z", "watch", None),
            row("2026-11-06T09:00:00Z", "watch", None),
            row("2026-11-01T09:00:00Z", "watch", None),
            row("2025-12-31T09:00:00Z", "watch", None),
        ];
        let groups = group_by_day(rows, today);
        for (locale, want) in [
            (
                "en",
                [
                    "Today",
                    "Yesterday",
                    "Sunday, Nov 1",
                    "Wednesday, Dec 31, 2025",
                ],
            ),
            (
                "de",
                [
                    "Heute",
                    "Gestern",
                    "Sonntag, 01.11.",
                    "Mittwoch, 31.12.2025",
                ],
            ),
        ] {
            let i = I18n::new(locale, BTreeMap::new());
            let got: Vec<String> = groups.iter().map(|g| day_heading(&i, g)).collect();
            assert_eq!(got, want, "{locale}");
        }
    }

    #[test]
    fn unknown_groups_and_people_in_a_shared_url_are_ignored() {
        // UX1-7
        let s = HistoryState::from_pairs(&[("group", "foo"), ("person", "junk")]);
        assert_eq!((s.group, s.person), (None, None));
        let id = "01900000-0000-7000-8000-000000000001";
        let s = HistoryState::from_pairs(&[("group", "play"), ("person", id)]);
        assert_eq!(s.group.as_deref(), Some("play"));
        assert_eq!(s.person.as_deref(), Some(id));
    }
}
