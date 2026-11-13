//! The log sheets' form state as plain data (design.md 5.4, 7.3; addendum-1 1.3 "5.4 Log
//! sheets"): the date chips, validation and the payloads the sheets send. No view code, so it
//! is tested natively; the sheets in [`crate::log_sheet`] only bind it to controls.

use contracts::{NewPlayDto, NewProgressDto, PlayerDto};

use crate::history_state::Day;

/// The date chips of a log sheet: today (now) and yesterday are one tap, any other day is
/// picked (design.md 3.4 "Dates").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateChip {
    #[default]
    Today,
    Yesterday,
    Pick,
}

/// Why a sheet cannot be saved yet; each names its Fluent message (`log-error-*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    DateMissing,
    DateInFuture,
    /// A day before the start of the rewatch run that is running (the start's timestamp):
    /// the view would be logged and not counted in the run (review OT2E-1, DEC-591).
    BeforeRun(String),
    /// The score of this companion is not a number.
    Score(String),
    Duration,
    Page,
    /// More pages than the item has.
    PageBeyond(u32),
    Position,
}

impl FormError {
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::DateMissing => "log-error-date-missing",
            Self::DateInFuture => "log-error-date-future",
            Self::BeforeRun(_) => "log-error-date-before-run",
            Self::Score(_) => "log-error-score",
            Self::Duration => "log-error-duration",
            Self::Page => "log-error-page",
            Self::PageBeyond(_) => "log-error-page-beyond",
            Self::Position => "log-error-position",
        }
    }
}

/// Noon UTC of a day: inside that calendar day in every time zone from UTC-11 to UTC+11.
fn noon(day: Day) -> String {
    format!("{}T12:00:00Z", day.iso())
}

/// When it happened, as RFC 3339; `None` means now (the server's default).
///
/// # Errors
/// A picked day that is missing or after today.
pub fn occurred_at(chip: DateChip, picked: &str, today: Day) -> Result<Option<String>, FormError> {
    match chip {
        DateChip::Today => Ok(None),
        DateChip::Yesterday => Ok(Some(noon(Day::from_days(today.days() - 1)))),
        DateChip::Pick => {
            let day = Day::parse(picked.trim()).ok_or(FormError::DateMissing)?;
            match day.days().cmp(&today.days()) {
                std::cmp::Ordering::Greater => Err(FormError::DateInFuture),
                std::cmp::Ordering::Equal => Ok(None),
                std::cmp::Ordering::Less => Ok(Some(noon(day))),
            }
        }
    }
}

/// A chosen moment (`at`, see [`occurred_at`]) for a view logged while a rewatch run is
/// running since `run_start` (an RFC 3339 timestamp in UTC, as the server sends it). The
/// run counts what was logged after its start, so a day before the start is refused (the
/// server refuses it too, DEC-591). On the day of the start noon may lie before the start:
/// then the view gets the start's own moment, which is the earliest one the run takes.
///
/// # Errors
/// [`FormError::BeforeRun`] for a day before the start.
pub fn within_run(
    at: Option<String>,
    run_start: Option<&str>,
) -> Result<Option<String>, FormError> {
    let (Some(at), Some(start)) = (at.as_deref(), run_start) else {
        return Ok(at);
    };
    let (Some(day), Some(start_day)) = (Day::parse(at), Day::parse(start)) else {
        return Ok(Some(at.to_string()));
    };
    match day.cmp(&start_day) {
        std::cmp::Ordering::Less => Err(FormError::BeforeRun(start.to_string())),
        // both are UTC and start with the same day: the times compare as text
        std::cmp::Ordering::Equal if at.get(11..19) < start.get(11..19) => {
            Ok(Some(start.to_string()))
        }
        _ => Ok(Some(at.to_string())),
    }
}

/// The "watched with..." sheet (`WatchedSheet`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WatchedForm {
    pub date: DateChip,
    /// `YYYY-MM-DD` of the "Pick a date" field.
    pub picked: String,
    /// Companion ids (not the viewer's own companion).
    pub with: Vec<String>,
    pub rating: Option<u8>,
    pub note: String,
}

impl WatchedForm {
    /// The `log_progress` body for `progress` (`{"type": "finished"}`, an episode).
    /// `run_start`: when the rewatch run that is running started ([`within_run`]).
    ///
    /// # Errors
    /// See [`occurred_at`] and [`within_run`].
    pub fn payload(
        &self,
        progress: serde_json::Value,
        today: Day,
        op: String,
        run_start: Option<&str>,
    ) -> Result<NewProgressDto, FormError> {
        let at = occurred_at(self.date, &self.picked, today)?;
        Ok(NewProgressDto {
            progress,
            occurred_at: within_run(at, run_start)?,
            client_op_id: Some(op),
            with: self.with.clone(),
        })
    }
}

/// One player row of the play sheet.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Player {
    pub companion_id: String,
    pub name: String,
    /// As typed; empty means no score.
    pub score: String,
}

/// Who won a competitive play, as the play dialog holds it (review OT2F-4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WinnerChoice {
    /// Not chosen by hand: the one highest score of at least two scores.
    #[default]
    Auto,
    /// Chosen by hand: the play has no winner.
    Nobody,
    /// Chosen by hand: this player (a companion id).
    Player(String),
}

/// The play sheet (`PlaySheet`): players with scores, the winner, and the rarer fields
/// behind "More".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayForm {
    pub date: DateChip,
    pub picked: String,
    pub players: Vec<Player>,
    /// Who won, as the dialog holds it.
    pub winner: WinnerChoice,
    /// Co-op game: everyone wins or loses together, or the play has no such end (OT-18).
    pub coop: bool,
    /// How a co-op play ended; only read when `coop` is set.
    pub coop_result: CoopResult,
    /// As typed: minutes or `h:mm` ([`parse_duration`]).
    pub duration: String,
    pub location: String,
    pub notes: String,
    /// The game group chosen with a chip (its id); empty: none.
    pub group: String,
}

/// How a co-op play ended. Many co-op games are played without winning or losing, so that
/// is where the choice starts (OT-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoopResult {
    #[default]
    NoResult,
    Won,
    Lost,
}

impl CoopResult {
    /// The word the API takes (`NewPlayDto::coop`) and the value of the form's choice.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoResult => "none",
            Self::Won => "won",
            Self::Lost => "lost",
        }
    }

    /// The result of a choice value; anything unknown is no result.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "won" => Self::Won,
            "lost" => Self::Lost,
            _ => Self::NoResult,
        }
    }
}

/// The players a group chip sets: you first, then the members (each once).
#[must_use]
pub fn group_players(me: Option<&str>, members: &[String]) -> Vec<String> {
    let mut out: Vec<String> = me.map(String::from).into_iter().collect();
    for m in members {
        if !out.contains(m) {
            out.push(m.clone());
        }
    }
    out
}

/// A score that is not a (finite) number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotANumber;

/// A typed score: empty is none, a comma works as the decimal mark (German keyboards).
///
/// # Errors
/// Not a number.
pub fn parse_score(s: &str) -> Result<Option<f64>, NotANumber> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    match s.replace(',', ".").parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(Some(v)),
        _ => Err(NotANumber),
    }
}

/// A typed duration in minutes: `90` or `1:30` (hours, then two digits of minutes below
/// 60); empty is none (OT-16), and so is zero: `0` typed over a prefilled duration clears
/// it, a play of no minutes is a play without a duration (review OT2A-14).
///
/// # Errors
/// [`FormError::Duration`] for anything else; `1:5` too, which could be 1:05 or 1:50.
pub fn parse_duration(s: &str) -> Result<Option<u32>, FormError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    let minutes = match s.split_once(':') {
        None if digits(s) => s.parse::<u32>().ok(),
        Some((h, m)) if digits(h) && digits(m) && m.len() == 2 => h
            .parse::<u32>()
            .ok()
            .zip(m.parse::<u32>().ok().filter(|m| *m < 60))
            .and_then(|(h, m)| h.checked_mul(60)?.checked_add(m)),
        _ => None,
    };
    minutes
        .map(|m| (m > 0).then_some(m))
        .ok_or(FormError::Duration)
}

/// Minutes the way they are typed: `45` below an hour, `1:30` from an hour on (the prefill
/// from the last play).
#[must_use]
pub fn format_duration(minutes: u32) -> String {
    if minutes < 60 {
        minutes.to_string()
    } else {
        format!("{}:{:02}", minutes / 60, minutes % 60)
    }
}

fn trimmed(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl PlayForm {
    /// The winner as a save sends it: the player chosen by hand while they are at the table
    /// (review OT2F-5), nobody when that was chosen, else the one player with the highest
    /// score. A tie has no winner, and neither has one score alone: a solo play, or the only
    /// score typed, is no result unless the person marks it (review OT2F-4). A co-op play
    /// has none.
    #[must_use]
    pub fn winner(&self) -> Option<String> {
        if self.coop {
            return None;
        }
        match &self.winner {
            WinnerChoice::Player(w) if self.players.iter().any(|p| p.companion_id == *w) => {
                return Some(w.clone());
            }
            WinnerChoice::Nobody => return None,
            WinnerChoice::Player(_) | WinnerChoice::Auto => {}
        }
        let scored: Vec<(f64, &str)> = self
            .players
            .iter()
            .filter_map(|p| Some((parse_score(&p.score).ok()??, p.companion_id.as_str())))
            .collect();
        if scored.len() < 2 {
            return None;
        }
        let best = scored
            .iter()
            .map(|(s, _)| *s)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut top = scored
            .iter()
            .filter(|(s, _)| (*s - best).abs() < f64::EPSILON);
        match (top.next(), top.next()) {
            (Some((_, id)), None) => Some((*id).to_string()),
            _ => None,
        }
    }

    /// Add a player once (the picker toggles).
    pub fn toggle_player(&mut self, companion_id: &str, name: &str) {
        if let Some(at) = self
            .players
            .iter()
            .position(|p| p.companion_id == companion_id)
        {
            self.players.remove(at);
            if self.winner == WinnerChoice::Player(companion_id.into()) {
                self.winner = WinnerChoice::Auto;
            }
        } else {
            self.players.push(Player {
                companion_id: companion_id.into(),
                name: name.into(),
                score: String::new(),
            });
        }
    }

    /// The `log_play` body.
    ///
    /// # Errors
    /// Every field that is wrong, in the order of the sheet.
    pub fn payload(&self, today: Day, op: String) -> Result<NewPlayDto, Vec<FormError>> {
        let mut errors = Vec::new();
        let started_at = occurred_at(self.date, &self.picked, today)
            .map_err(|e| errors.push(e))
            .unwrap_or_default();
        let winner = self.winner();
        let mut players = Vec::new();
        for p in &self.players {
            match parse_score(&p.score) {
                Ok(score) => players.push(PlayerDto {
                    companion_id: p.companion_id.clone(),
                    score,
                    winner: winner.as_deref() == Some(p.companion_id.as_str()),
                }),
                Err(NotANumber) => errors.push(FormError::Score(p.companion_id.clone())),
            }
        }
        let duration_minutes = parse_duration(&self.duration)
            .map_err(|e| errors.push(e))
            .unwrap_or_default();
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(NewPlayDto {
            started_at,
            duration_minutes,
            location: trimmed(&self.location),
            notes: trimmed(&self.notes),
            players,
            coop: self.coop.then(|| self.coop_result.as_str().to_string()),
            client_op_id: Some(op),
            game_group_id: trimmed(&self.group),
        })
    }
}

/// The paged progress for a typed page; with a known total the page cannot exceed it.
///
/// # Errors
/// Not a page number, or beyond the last page.
pub fn page_progress(input: &str, pages: Option<u32>) -> Result<serde_json::Value, FormError> {
    let page = input
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|p| *p > 0)
        .ok_or(FormError::Page)?;
    if let Some(total) = pages.filter(|t| page > *t) {
        return Err(FormError::PageBeyond(total));
    }
    Ok(serde_json::json!({"type": "paged", "page": page}))
}

/// What the pages control knows about the page total of its item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PageTotal {
    /// The total is a source's estimate (the median over the editions of a work), not a
    /// number somebody counted or set.
    pub estimated: bool,
    /// The viewer may change it (`ItemPageDto::page_total_editable`).
    pub can_edit: bool,
}

/// The sources whose page total is an estimate: Open Library gives the median number of
/// pages over the editions of a work (docs/connectors/openlibrary.md).
const ESTIMATING_SOURCES: [&str; 1] = ["openlibrary"];

/// [`PageTotal`] of an item: an estimate when the field came from a source that estimates,
/// never when a person set it.
#[must_use]
pub fn page_total(item: &contracts::CatalogItemDto, can_edit: bool) -> PageTotal {
    let estimated = item
        .provenance
        .get("attributes.page_count")
        .is_some_and(|source| ESTIMATING_SOURCES.contains(&source.as_str()));
    PageTotal {
        estimated,
        can_edit,
    }
}

/// A typed page total: a whole number from 1 to 100000.
#[must_use]
pub fn parse_page_total(input: &str) -> Option<u32> {
    input
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|p| (1..=100_000).contains(p))
}

/// The page "Finished" logs before it completes the entry: the last one, when the total is
/// known and the reader is not there yet, so that finishing counts the pages that were left.
/// `None`: nothing to log (no total, or already on the last page).
#[must_use]
pub fn finish_page(at: Option<u32>, pages: Option<u32>) -> Option<u32> {
    pages.filter(|total| *total > 0 && at != Some(*total))
}

/// `h:mm:ss`, `mm:ss` or plain minutes, in seconds.
#[must_use]
pub fn parse_position(s: &str) -> Option<u64> {
    let parts: Vec<u64> = s
        .trim()
        .split(':')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<_>>()?;
    let sixty = |v: u64| (v < 60).then_some(v);
    match parts.as_slice() {
        [m] => Some(m * 60),
        [m, sec] => Some(m * 60 + sixty(*sec)?),
        [h, m, sec] => Some(h * 3600 + sixty(*m)? * 60 + sixty(*sec)?),
        _ => None,
    }
}

/// Seconds as `h:mm:ss`.
#[must_use]
pub fn format_position(seconds: u64) -> String {
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// The timed progress for a typed position; with a known total it cannot exceed it.
///
/// # Errors
/// Not a position, or beyond the end.
pub fn position_progress(input: &str, total: Option<u64>) -> Result<serde_json::Value, FormError> {
    let seconds = parse_position(input).ok_or(FormError::Position)?;
    if total.is_some_and(|t| seconds > t) {
        return Err(FormError::Position);
    }
    Ok(serde_json::json!({"type": "position", "seconds": seconds}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> Day {
        Day::parse("2026-03-10").unwrap()
    }

    #[test]
    fn date_chips_map_to_the_logged_moment() {
        assert_eq!(occurred_at(DateChip::Today, "", today()), Ok(None));
        assert_eq!(
            occurred_at(DateChip::Yesterday, "", today()),
            Ok(Some("2026-03-09T12:00:00Z".into()))
        );
        // across a month border
        let first = Day::parse("2026-03-01").unwrap();
        assert_eq!(
            occurred_at(DateChip::Yesterday, "", first),
            Ok(Some("2026-02-28T12:00:00Z".into()))
        );
        assert_eq!(
            occurred_at(DateChip::Pick, "2025-12-24", today()),
            Ok(Some("2025-12-24T12:00:00Z".into()))
        );
        // picking today is the same as "Today": now
        assert_eq!(occurred_at(DateChip::Pick, "2026-03-10", today()), Ok(None));
        assert_eq!(
            occurred_at(DateChip::Pick, "", today()),
            Err(FormError::DateMissing)
        );
        assert_eq!(
            occurred_at(DateChip::Pick, "24.12.2025", today()),
            Err(FormError::DateMissing)
        );
        assert_eq!(
            occurred_at(DateChip::Pick, "2026-03-11", today()),
            Err(FormError::DateInFuture)
        );
    }

    /// Review OT2E-1: while a rewatch run is running, a view dated before its start would
    /// be logged and not counted. The dialog refuses the day and names the day of the start.
    #[test]
    fn a_day_before_the_start_of_a_running_rewatch_is_refused() {
        let ep = serde_json::json!({"type": "episode", "season": 1, "episode": 1});
        let at = |date, picked: &str, start: Option<&str>| {
            WatchedForm {
                date,
                picked: picked.into(),
                ..WatchedForm::default()
            }
            .payload(ep.clone(), today(), "op".into(), start)
            .map(|p| p.occurred_at)
        };
        // started yesterday afternoon
        let start = "2026-03-09T15:30:00.250+00:00";
        assert_eq!(at(DateChip::Today, "", Some(start)), Ok(None));
        assert_eq!(
            at(DateChip::Pick, "2026-03-08", Some(start)),
            Err(FormError::BeforeRun(start.into()))
        );
        // the day of the start: noon would sort before the start, so its own moment
        assert_eq!(
            at(DateChip::Yesterday, "", Some(start)),
            Ok(Some(start.into()))
        );
        // started in the morning of that day: noon is after it
        let morning = "2026-03-09T08:05:00+00:00";
        assert_eq!(
            at(DateChip::Yesterday, "", Some(morning)),
            Ok(Some("2026-03-09T12:00:00Z".into()))
        );
        // started today: yesterday is before it
        let now = "2026-03-10T07:00:00+00:00";
        assert_eq!(
            at(DateChip::Yesterday, "", Some(now)),
            Err(FormError::BeforeRun(now.into()))
        );
        // no run: every past day is fine
        assert_eq!(
            at(DateChip::Pick, "2026-03-08", None),
            Ok(Some("2026-03-08T12:00:00Z".into()))
        );
        assert_eq!(
            FormError::BeforeRun(String::new()).key(),
            "log-error-date-before-run"
        );
    }

    #[test]
    fn errors_name_their_message() {
        assert_eq!(FormError::DateMissing.key(), "log-error-date-missing");
        assert_eq!(FormError::DateInFuture.key(), "log-error-date-future");
        assert_eq!(FormError::Score("c".into()).key(), "log-error-score");
        assert_eq!(FormError::Duration.key(), "log-error-duration");
        assert_eq!(FormError::Page.key(), "log-error-page");
        assert_eq!(FormError::PageBeyond(3).key(), "log-error-page-beyond");
        assert_eq!(FormError::Position.key(), "log-error-position");
    }

    #[test]
    fn the_watched_sheet_sends_the_day_and_the_people() {
        let form = WatchedForm {
            date: DateChip::Yesterday,
            with: vec!["anna".into(), "ben".into()],
            rating: Some(80),
            note: "great".into(),
            ..WatchedForm::default()
        };
        let ep = serde_json::json!({"type": "episode", "season": 2, "episode": 5});
        let p = form
            .payload(ep.clone(), today(), "op-1".into(), None)
            .unwrap();
        assert_eq!(p.progress, ep);
        assert_eq!(p.occurred_at.as_deref(), Some("2026-03-09T12:00:00Z"));
        assert_eq!(p.client_op_id.as_deref(), Some("op-1"));
        assert_eq!(p.with, vec!["anna".to_string(), "ben".to_string()]);
        let bad = WatchedForm {
            date: DateChip::Pick,
            ..WatchedForm::default()
        };
        assert_eq!(
            bad.payload(ep, today(), "op".into(), None),
            Err(FormError::DateMissing)
        );
    }

    fn player(id: &str, score: &str) -> Player {
        Player {
            companion_id: id.into(),
            name: id.to_uppercase(),
            score: score.into(),
        }
    }

    #[test]
    fn scores_take_a_comma_or_a_point() {
        assert_eq!(parse_score(""), Ok(None));
        assert_eq!(parse_score(" 42 "), Ok(Some(42.0)));
        assert_eq!(parse_score("12,5"), Ok(Some(12.5)));
        assert_eq!(parse_score("-3.5"), Ok(Some(-3.5)));
        assert_eq!(parse_score("lots"), Err(NotANumber));
        assert_eq!(parse_score("inf"), Err(NotANumber));
    }

    #[test]
    fn the_highest_score_wins_unless_tied_chosen_or_co_op() {
        let mut form = PlayForm {
            players: vec![player("me", "31"), player("ben", "42"), player("cleo", "")],
            ..PlayForm::default()
        };
        assert_eq!(form.winner().as_deref(), Some("ben"));
        form.players[0].score = "42".into();
        assert_eq!(form.winner(), None, "a tie has no derived winner");
        form.winner = WinnerChoice::Player("cleo".into());
        assert_eq!(form.winner().as_deref(), Some("cleo"), "chosen by hand");
        form.coop = true;
        assert_eq!(form.winner(), None, "co-op: nobody wins alone");
        let none = PlayForm {
            players: vec![player("me", ""), player("ben", "x")],
            ..PlayForm::default()
        };
        assert_eq!(none.winner(), None);
    }

    /// Review OT2F-4: one score alone is no result (a solo play, or the only score typed),
    /// unless the person marks the winner; and a scored play can be saved without one.
    #[test]
    fn a_single_score_names_no_winner_and_nobody_can_be_chosen() {
        let day = || Day::parse("2026-03-07").unwrap();
        let mut solo = PlayForm {
            players: vec![player("me", "120")],
            ..PlayForm::default()
        };
        assert_eq!(solo.winner(), None, "a solo play has no result by itself");
        let sent = solo.payload(day(), "op".into()).unwrap();
        assert!(sent.players.iter().all(|p| !p.winner));
        assert_eq!(sent.players[0].score, Some(120.0));
        solo.winner = WinnerChoice::Player("me".into());
        assert_eq!(solo.winner().as_deref(), Some("me"), "marked won by hand");
        let one_typed = PlayForm {
            players: vec![player("me", "50"), player("ben", "")],
            ..PlayForm::default()
        };
        assert_eq!(one_typed.winner(), None, "one score of two players");
        let mut scored = PlayForm {
            players: vec![player("me", "31"), player("ben", "42")],
            ..PlayForm::default()
        };
        assert_eq!(scored.winner().as_deref(), Some("ben"));
        scored.winner = WinnerChoice::Nobody;
        assert_eq!(scored.winner(), None, "no winner, chosen by hand");
        let sent = scored.payload(day(), "op".into()).unwrap();
        assert!(sent.players.iter().all(|p| !p.winner));
        assert_eq!(sent.players[1].score, Some(42.0));
    }

    /// Review OT2F-5: a winner chosen by hand who is no longer at the table is no choice;
    /// the highest score decides again.
    #[test]
    fn a_chosen_winner_who_left_the_table_is_no_choice() {
        let form = PlayForm {
            players: vec![player("me", "50"), player("cleo", "20")],
            winner: WinnerChoice::Player("ben".into()),
            ..PlayForm::default()
        };
        assert_eq!(form.winner().as_deref(), Some("me"));
        let sent = form
            .payload(Day::parse("2026-03-07").unwrap(), "op".into())
            .unwrap();
        assert_eq!(
            sent.players.iter().map(|p| p.winner).collect::<Vec<_>>(),
            [true, false]
        );
    }

    #[test]
    fn a_group_chip_sets_you_and_the_members_and_the_group_is_sent() {
        let members = vec!["ann".to_string(), "me".to_string(), "ben".to_string()];
        assert_eq!(group_players(Some("me"), &members), ["me", "ann", "ben"]);
        assert_eq!(group_players(None, &members[..1]), ["ann"]);
        let form = PlayForm {
            players: vec![player("me", "")],
            group: "g1".into(),
            ..PlayForm::default()
        };
        let p = form
            .payload(Day::parse("2026-03-07").unwrap(), "op".into())
            .unwrap();
        assert_eq!(p.game_group_id.as_deref(), Some("g1"));
        let none = PlayForm::default()
            .payload(Day::parse("2026-03-07").unwrap(), "op".into())
            .unwrap();
        assert_eq!(none.game_group_id, None);
    }

    #[test]
    fn players_toggle_once() {
        let mut form = PlayForm::default();
        form.toggle_player("ben", "Ben");
        form.toggle_player("cleo", "Cleo");
        assert_eq!(form.players.len(), 2);
        assert_eq!(form.players[0].name, "Ben");
        form.toggle_player("ben", "Ben");
        assert_eq!(form.players.len(), 1);
        assert_eq!(form.players[0].companion_id, "cleo");
    }

    #[test]
    fn the_play_sheet_sends_players_scores_and_the_winner() {
        let form = PlayForm {
            date: DateChip::Pick,
            picked: "2026-03-01".into(),
            players: vec![player("me", "31"), player("ben", "42,5")],
            duration: "45".into(),
            location: " Cafe ".into(),
            ..PlayForm::default()
        };
        let p = form.payload(today(), "op-2".into()).unwrap();
        assert_eq!(p.started_at.as_deref(), Some("2026-03-01T12:00:00Z"));
        assert_eq!(p.client_op_id.as_deref(), Some("op-2"));
        assert_eq!(p.duration_minutes, Some(45));
        assert_eq!(p.location.as_deref(), Some("Cafe"));
        assert_eq!(p.notes, None);
        assert_eq!(p.coop, None);
        assert_eq!(
            p.players,
            vec![
                PlayerDto {
                    companion_id: "me".into(),
                    score: Some(31.0),
                    winner: false
                },
                PlayerDto {
                    companion_id: "ben".into(),
                    score: Some(42.5),
                    winner: true
                },
            ]
        );
        // co-op: one outcome for everyone and no winner; it starts as "no result" (OT-18)
        let coop = PlayForm {
            coop: true,
            ..form.clone()
        };
        let p = coop.payload(today(), "op".into()).unwrap();
        assert_eq!(p.coop.as_deref(), Some("none"));
        assert!(p.players.iter().all(|p| !p.winner));
        for (result, word) in [(CoopResult::Won, "won"), (CoopResult::Lost, "lost")] {
            let p = PlayForm {
                coop_result: result,
                ..coop.clone()
            }
            .payload(today(), "op".into())
            .unwrap();
            assert_eq!(p.coop.as_deref(), Some(word));
            assert_eq!(CoopResult::parse(word), result);
        }
        assert_eq!(CoopResult::parse(""), CoopResult::NoResult);
        // the result of a play that is not co-op is not sent
        let p = PlayForm {
            coop_result: CoopResult::Won,
            ..form.clone()
        }
        .payload(today(), "op".into())
        .unwrap();
        assert_eq!(p.coop, None);
        let bad = PlayForm {
            date: DateChip::Pick,
            picked: "2026-04-01".into(),
            players: vec![player("me", "a lot")],
            duration: "an hour".into(),
            ..PlayForm::default()
        };
        assert_eq!(
            bad.payload(today(), "op".into()),
            Err(vec![
                FormError::DateInFuture,
                FormError::Score("me".into()),
                FormError::Duration
            ])
        );
    }

    #[test]
    fn pages_are_whole_numbers_within_the_book() {
        assert_eq!(
            page_progress(" 212 ", Some(410)),
            Ok(serde_json::json!({"type": "paged", "page": 212}))
        );
        assert_eq!(
            page_progress("500", None),
            Ok(serde_json::json!({"type": "paged", "page": 500}))
        );
        assert_eq!(page_progress("0", None), Err(FormError::Page));
        assert_eq!(page_progress("twelve", None), Err(FormError::Page));
        assert_eq!(
            page_progress("411", Some(410)),
            Err(FormError::PageBeyond(410))
        );
    }

    /// v1-books 3.1.3 (G16): Finished with a total logs the last page; without one it does not.
    #[test]
    fn finished_logs_the_last_page_when_the_total_is_known() {
        assert_eq!(finish_page(Some(212), Some(410)), Some(410));
        assert_eq!(finish_page(None, Some(410)), Some(410), "never opened");
        assert_eq!(finish_page(Some(410), Some(410)), None, "already there");
        assert_eq!(
            finish_page(Some(212), None),
            None,
            "no total: only the status"
        );
        assert_eq!(finish_page(None, None), None);
        assert_eq!(
            finish_page(Some(3), Some(0)),
            None,
            "a total of 0 is no total"
        );
    }

    #[test]
    fn a_page_total_is_a_number_from_1_to_100000() {
        assert_eq!(parse_page_total(" 577 "), Some(577));
        assert_eq!(parse_page_total("100000"), Some(100_000));
        for bad in ["0", "100001", "", "12.5", "-3", "many"] {
            assert_eq!(parse_page_total(bad), None, "{bad}");
        }
    }

    #[test]
    fn only_a_total_from_an_estimating_source_is_an_estimate() {
        let item = |source: Option<&str>| -> contracts::CatalogItemDto {
            let mut provenance = serde_json::Map::new();
            if let Some(s) = source {
                provenance.insert("attributes.page_count".into(), s.into());
            }
            serde_json::from_value(serde_json::json!({
                "id": "i", "kind": "book", "title": "Dune", "original_title": null,
                "year": 1965, "description": null, "cover": null, "external_ids": [],
                "attributes": {"page_count": 608}, "state": "full", "provenance": provenance
            }))
            .unwrap()
        };
        assert!(page_total(&item(Some("openlibrary")), true).estimated);
        assert!(
            !page_total(&item(Some("user")), true).estimated,
            "a person set it"
        );
        assert!(
            !page_total(&item(None), true).estimated,
            "an import brought it"
        );
        assert!(page_total(&item(None), true).can_edit);
        assert!(!page_total(&item(Some("openlibrary")), false).can_edit);
    }

    #[test]
    fn positions_read_hours_minutes_and_seconds() {
        assert_eq!(parse_position("4:12:30"), Some(4 * 3600 + 12 * 60 + 30));
        assert_eq!(parse_position("12:30"), Some(12 * 60 + 30));
        assert_eq!(parse_position(" 45 "), Some(45 * 60), "plain minutes");
        assert_eq!(parse_position("1:75:00"), None);
        assert_eq!(parse_position(""), None);
        assert_eq!(parse_position("soon"), None);
        assert_eq!(format_position(4 * 3600 + 12 * 60 + 30), "4:12:30");
        assert_eq!(format_position(59), "0:00:59");
        assert_eq!(
            position_progress("1:00:00", Some(7200)),
            Ok(serde_json::json!({"type": "position", "seconds": 3600}))
        );
        assert_eq!(
            position_progress("3:00:00", Some(7200)),
            Err(FormError::Position)
        );
        assert_eq!(position_progress("x", None), Err(FormError::Position));
    }

    #[test]
    fn a_sheet_is_dirty_once_anything_changed() {
        let start = WatchedForm::default();
        let mut now = start.clone();
        assert_eq!(now, start);
        now.with.push("anna".into());
        assert_ne!(now, start);
    }

    /// OT-16: the play dialog takes "1:30" or "90".
    #[test]
    fn a_duration_is_hours_and_minutes_or_plain_minutes() {
        assert_eq!(parse_duration("90"), Ok(Some(90)));
        assert_eq!(parse_duration(" 1:30 "), Ok(Some(90)));
        assert_eq!(parse_duration("0:45"), Ok(Some(45)));
        assert_eq!(parse_duration("12:05"), Ok(Some(725)));
        // OT2A-14: zero is "no duration" (typed to clear the prefill), not a play of 0 minutes
        assert_eq!(parse_duration("0"), Ok(None));
        assert_eq!(parse_duration("0:00"), Ok(None));
        // empty is no duration
        assert_eq!(parse_duration(""), Ok(None));
        assert_eq!(parse_duration("   "), Ok(None));
        // "1:5" could be 1:05 or 1:50: refused, as are minutes from 60 on and anything else
        for bad in [
            "1:5", "1:60", "1:234", ":30", "1:", "1:30:00", "ninety", "1.5", "-5", "1h 30",
        ] {
            assert_eq!(parse_duration(bad), Err(FormError::Duration), "{bad}");
        }
        // shown again the way it is typed
        assert_eq!(format_duration(45), "45");
        assert_eq!(format_duration(60), "1:00");
        assert_eq!(format_duration(90), "1:30");
        assert_eq!(format_duration(725), "12:05");
        for m in [1, 59, 60, 61, 600] {
            assert_eq!(parse_duration(&format_duration(m)), Ok(Some(m)));
        }
    }

    #[test]
    fn a_play_takes_its_duration_as_hours_and_minutes() {
        let form = PlayForm {
            duration: "1:30".into(),
            ..PlayForm::default()
        };
        let play = form.payload(today(), "op".into()).unwrap();
        assert_eq!(play.duration_minutes, Some(90));
        let bad = PlayForm {
            duration: "1:5".into(),
            ..PlayForm::default()
        };
        assert_eq!(
            bad.payload(today(), "op".into()).unwrap_err(),
            vec![FormError::Duration]
        );
    }
}
