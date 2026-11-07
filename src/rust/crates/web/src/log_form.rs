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
    ///
    /// # Errors
    /// See [`occurred_at`].
    pub fn payload(
        &self,
        progress: serde_json::Value,
        today: Day,
        op: String,
    ) -> Result<NewProgressDto, FormError> {
        Ok(NewProgressDto {
            progress,
            occurred_at: occurred_at(self.date, &self.picked, today)?,
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

/// The play sheet (`PlaySheet`): players with scores, the winner, and the rarer fields
/// behind "More".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayForm {
    pub date: DateChip,
    pub picked: String,
    pub players: Vec<Player>,
    /// A winner chosen by hand (overrides the highest score).
    pub winner: Option<String>,
    /// Co-op game: everyone wins or loses together.
    pub coop: bool,
    pub coop_won: bool,
    /// Minutes, as typed.
    pub duration: String,
    pub location: String,
    pub notes: String,
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

fn trimmed(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl PlayForm {
    /// The winner: chosen by hand, else the one player with the highest score (a tie has
    /// no winner); a co-op play has none.
    #[must_use]
    pub fn winner(&self) -> Option<String> {
        if self.coop {
            return None;
        }
        if let Some(w) = &self.winner {
            return Some(w.clone());
        }
        let scored: Vec<(f64, &str)> = self
            .players
            .iter()
            .filter_map(|p| Some((parse_score(&p.score).ok()??, p.companion_id.as_str())))
            .collect();
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
            if self.winner.as_deref() == Some(companion_id) {
                self.winner = None;
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
        let duration_minutes = match trimmed(&self.duration).map(|d| d.parse::<u32>()) {
            None => None,
            Some(Ok(m)) => Some(m),
            Some(Err(_)) => {
                errors.push(FormError::Duration);
                None
            }
        };
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(NewPlayDto {
            started_at,
            duration_minutes,
            location: trimmed(&self.location),
            notes: trimmed(&self.notes),
            players,
            coop: self
                .coop
                .then(|| if self.coop_won { "won" } else { "lost" }.to_string()),
            client_op_id: Some(op),
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
        let p = form.payload(ep.clone(), today(), "op-1".into()).unwrap();
        assert_eq!(p.progress, ep);
        assert_eq!(p.occurred_at.as_deref(), Some("2026-03-09T12:00:00Z"));
        assert_eq!(p.client_op_id.as_deref(), Some("op-1"));
        assert_eq!(p.with, vec!["anna".to_string(), "ben".to_string()]);
        let bad = WatchedForm {
            date: DateChip::Pick,
            ..WatchedForm::default()
        };
        assert_eq!(
            bad.payload(ep, today(), "op".into()),
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
        form.winner = Some("cleo".into());
        assert_eq!(form.winner().as_deref(), Some("cleo"), "chosen by hand");
        form.coop = true;
        assert_eq!(form.winner(), None, "co-op: nobody wins alone");
        let none = PlayForm {
            players: vec![player("me", ""), player("ben", "x")],
            ..PlayForm::default()
        };
        assert_eq!(none.winner(), None);
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
        let coop = PlayForm {
            coop: true,
            coop_won: true,
            ..form.clone()
        };
        let p = coop.payload(today(), "op".into()).unwrap();
        assert_eq!(p.coop.as_deref(), Some("won"));
        assert!(p.players.iter().all(|p| !p.winner));
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
}
