//! The words of the item page that depend on the kind's verb (OT-4 to OT-7): "Want to
//! watch", "Watched 3 times, last on ...", "Watched with...", and the fact line under the
//! title. Plain text functions, tested natively in en and de.

use contracts::{LibraryEntryDto, NextStepDto, ProgressSummaryDto};

use crate::i18n::I18n;
use crate::next_step::progress_text;

/// What one does with an item, by its kind group: the verb of the labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Watch,
    Read,
    Listen,
    Play,
}

impl Verb {
    /// The verb of a kind group (`watch`, `read`, `listen`, `play`); `None` for `other` and
    /// while the kind is not known.
    #[must_use]
    pub fn of_group(group: &str) -> Option<Self> {
        match group {
            "watch" => Some(Self::Watch),
            "read" => Some(Self::Read),
            "listen" => Some(Self::Listen),
            "play" => Some(Self::Play),
            _ => None,
        }
    }
}

/// "Watched once" / "Watched 3 times" for the verb (plural forms per language, OT-7).
#[must_use]
pub fn times_text(i: &I18n, verb: Option<Verb>, times: u32) -> String {
    let key = match verb {
        Some(Verb::Watch) => "item-times-watch",
        Some(Verb::Read) => "item-times-read",
        Some(Verb::Listen) => "item-times-listen",
        Some(Verb::Play) => "item-times-play",
        None => "item-times",
    };
    i.t_args(key, &[("count", times.into())])
}

/// The second button of an item that is not in the library: "Want to watch" (adds it as
/// Planned); "Add to library" for a kind without a verb (OT-4).
#[must_use]
pub fn want_key(verb: Option<Verb>) -> &'static str {
    match verb {
        Some(Verb::Watch) => "item-want-watch",
        Some(Verb::Read) => "item-want-read",
        Some(Verb::Listen) => "item-want-listen",
        Some(Verb::Play) => "item-want-play",
        None => "item-add",
    }
}

/// The button that opens the log dialog with people: "Watched with..." (OT-6).
#[must_use]
pub fn with_button_key(verb: Option<Verb>) -> &'static str {
    match verb {
        Some(Verb::Watch) => "item-with-watch",
        Some(Verb::Read) => "item-with-read",
        Some(Verb::Listen) => "item-with-listen",
        Some(Verb::Play) => "item-with-play",
        None => "item-with",
    }
}

/// The legend of the people picker in a log dialog: "Watched with" (OT-6).
#[must_use]
pub fn with_legend_key(verb: Option<Verb>) -> &'static str {
    match verb {
        Some(Verb::Watch) => "log-with-watch",
        Some(Verb::Read) => "log-with-read",
        Some(Verb::Listen) => "log-with-listen",
        Some(Verb::Play) => "log-with-play",
        None => "log-people",
    }
}

/// The Fluent keys of a rewatch run for a verb (ot2e): the menu items, the toast of the start
/// and the lines under the title. A podcast is listened to again; every other kind reads like a
/// show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RewatchWords {
    /// "Start a rewatch" (overflow menu of a Completed show).
    pub start: &'static str,
    /// "Stop the rewatch" (sets Completed).
    pub stop: &'static str,
    /// Toast after the start.
    pub started: &'static str,
    /// Toast after the stop.
    pub stopped_toast: &'static str,
    /// Second line of the toast of the episode that finished the run (`title`).
    pub finished: &'static str,

    before: &'static str,
    line: &'static str,
    line_open: &'static str,
    stopped: &'static str,
    stopped_open: &'static str,
}

impl RewatchWords {
    #[must_use]
    pub fn of(verb: Option<Verb>) -> Self {
        if verb == Some(Verb::Listen) {
            Self {
                start: "item-rewatch-start-listen",
                stop: "item-rewatch-stop-listen",
                started: "item-rewatch-started-listen",
                stopped_toast: "item-rewatch-stopped-toast-listen",
                finished: "item-rewatch-finished-listen",

                before: "item-rewatch-before-listen",
                line: "item-rewatch-line-listen",
                line_open: "item-rewatch-line-open-listen",
                stopped: "item-rewatch-stopped-listen",
                stopped_open: "item-rewatch-stopped-open-listen",
            }
        } else {
            Self {
                start: "item-rewatch-start",
                stop: "item-rewatch-stop",
                started: "item-rewatch-started",
                stopped_toast: "item-rewatch-stopped-toast",
                finished: "item-rewatch-finished",

                before: "item-rewatch-before",
                line: "item-rewatch-line",
                line_open: "item-rewatch-line-open",
                stopped: "item-rewatch-stopped",
                stopped_open: "item-rewatch-stopped-open",
            }
        }
    }
}

/// When the rewatch run that is running started: the timestamp of its start event in the
/// page's history; `None` without a running run. A view logged in the dialog cannot be dated
/// before it (review OT2E-1).
#[must_use]
pub fn run_start(page: &contracts::ItemPageDto) -> Option<String> {
    let summary = page.summary.as_ref()?;
    let run = summary.rewatch.as_ref().filter(|r| r.running)?;
    let start = page.progress.iter().find(|e| e.id == run.start)?;
    Some(start.occurred_at.clone())
}

/// What the fact line under the title is made of.
#[derive(Debug, Clone, Copy)]
pub struct Facts<'a> {
    pub verb: Option<Verb>,
    /// The kind's progress model (`single`, `episodic`, ...).
    pub model: &'a str,
    /// Whole passes (`ItemPageDto::times`).
    pub times: u32,
    /// The entry's status is Completed.
    pub completed: bool,
    /// RFC 3339: the day of the pass the count names (`ItemPageDto::times_at`), when it has
    /// one.
    pub times_at: Option<&'a str>,
    /// RFC 3339, when the entry was added.
    pub added_at: &'a str,
    pub summary: &'a ProgressSummaryDto,
    pub step: &'a NextStepDto,
}

/// The line under the title of an item in the library (OT-5, OT-7): "Watched 3 times, last
/// on <date>" once it was watched, else where the person stands ("12 of 24 episodes", "Up to
/// date: ...", "Page 120 of 240"), else "Added on <date>" while nothing is logged. `None`
/// when there is nothing to say in numbers. While a rewatch run is running the line is the
/// run ("Rewatch: 5 of 24 episodes. Watched once before."); a stopped one follows the count
/// ("Watched once; rewatch stopped after 5 of 24 episodes"; ot2e, DEC-587).
///
/// A show is counted only once its entry is Completed (ux-owner-test.md 1.2): with every
/// known episode watched it is "Up to date" until the person says it is finished; the app
/// cannot know whether more will air (DEC-207, review OT2A-2).
#[must_use]
pub fn fact_line(i: &I18n, f: &Facts<'_>) -> Option<String> {
    let run = f.summary.rewatch.as_ref();
    if let Some(run) = run.filter(|r| r.running) {
        return Some(running_line(i, f, run.done));
    }
    if f.times > 0 && (f.completed || f.model != "episodic") {
        let times = times_text(i, f.verb, f.times);
        if let Some(run) = run.filter(|_| f.completed) {
            return Some(stopped_line(i, f, times, run.done));
        }
        // the day of the pass the count names, not of the newest view (review OT2A-7)
        let Some(last) = f.times_at else {
            return Some(times);
        };
        let key = match (f.times, f.model) {
            (1, "episodic") => "item-times-finished-on",
            (1, _) => "item-times-on",
            _ => "item-times-last-on",
        };
        return Some(i.t_args(
            key,
            &[("times", times.into()), ("date", i.date(last).into())],
        ));
    }
    if f.summary.events == 0 {
        return Some(i.t_args("item-added-on", &[("date", i.date(f.added_at).into())]));
    }
    let progress = progress_text(i, f.summary, f.step)?;
    Some(if f.step.action == "caught_up" {
        i.t_args("item-fact-up-to-date", &[("progress", progress.into())])
    } else {
        progress
    })
}

/// A game's fact line with the time played after it: "Played 14 times, last on Nov 2, 2026,
/// 31 hours in total" (ot2f; ux-owner-test.md 2.4). Without any logged time the line stays
/// as it is.
#[must_use]
pub fn with_play_time(i: &I18n, line: String, minutes: u64) -> String {
    if minutes == 0 {
        return line;
    }
    let time = crate::charts::Measure::Hours.counted(i, minutes);
    i.t_args(
        "item-times-total",
        &[("line", line.into()), ("time", time.into())],
    )
}

/// "Rewatch: 5 of 24 episodes. Watched once before." (a run starts on a Completed entry, so
/// there is at least one time before it).
fn running_line(i: &I18n, f: &Facts<'_>, done: u32) -> String {
    let words = RewatchWords::of(f.verb);
    let before = i.t_args(words.before, &[("count", f.times.max(1).into())]);
    match f.summary.episodes_total {
        Some(total) => i.t_args(
            words.line,
            &[
                ("done", done.into()),
                ("total", total.into()),
                ("before", before.into()),
            ],
        ),
        None => i.t_args(
            words.line_open,
            &[("done", done.into()), ("before", before.into())],
        ),
    }
}

/// "Watched once; rewatch stopped after 5 of 24 episodes".
fn stopped_line(i: &I18n, f: &Facts<'_>, times: String, done: u32) -> String {
    let words = RewatchWords::of(f.verb);
    match f.summary.episodes_total {
        Some(total) => i.t_args(
            words.stopped,
            &[
                ("times", times.into()),
                ("done", done.into()),
                ("total", total.into()),
            ],
        ),
        None => i.t_args(
            words.stopped_open,
            &[("times", times.into()), ("done", done.into())],
        ),
    }
}

/// How often the item was gone through before, for the dialog line of a step that says
/// "again" (OT2A-1): the header's count; 0 for every other step.
#[must_use]
pub fn again_count(step: &NextStepDto, times: u32) -> u32 {
    if step.action == "finish" && step.again {
        times
    } else {
        0
    }
}

/// Is the entry still as an add left it: Planned, nothing logged, not rated, no note, no
/// tag? Only then the Undo of a first log takes the entry away again (review OT2A-3); what
/// the person did to it meanwhile is never removed with it.
#[must_use]
pub fn entry_untouched(entry: &LibraryEntryDto, events: usize, tags: usize) -> bool {
    entry.status == "planned"
        && events == 0
        && tags == 0
        && entry.rating.is_none()
        && entry.notes.as_deref().is_none_or(str::is_empty)
}

/// What the Undo of the add toast does with the entry as it is by then (review OT2A-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddUndo {
    /// Still as the add left it: remove it.
    Remove,
    /// Changed meanwhile (a watch, a status, a rating, a note, a tag): it stays, and the
    /// toast says so.
    Keep,
    /// Already removed: nothing to take back.
    Gone,
}

/// The Undo of "Added to your library as Planned" for the entry as it is now: `events` logged
/// and `tags` given since count as changes ([`entry_untouched`]).
#[must_use]
pub fn add_undo(entry: Option<&LibraryEntryDto>, events: usize, tags: usize) -> AddUndo {
    match entry {
        None => AddUndo::Gone,
        Some(e) if entry_untouched(e, events, tags) => AddUndo::Remove,
        Some(_) => AddUndo::Keep,
    }
}

/// The Fluent keys of the Episodes tab that name what one does with an episode (v1-polish P3):
/// a podcast's episodes are listened to, every other kind reads like a show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeWords {
    /// The state of a row and the button of an open one: "Watched".
    pub done: &'static str,
    /// The button's name: "Mark S1 E1 watched" (`season`, `episode`).
    pub mark: &'static str,
    /// The same on a row that is done: "Watch S1 E1 again".
    pub mark_again: &'static str,
    /// "Mark season watched".
    pub mark_season: &'static str,
    /// "Mark show watched".
    pub mark_show: &'static str,
    /// "Also mark the N earlier episodes as watched?" (`count`).
    pub ask: &'static str,
    /// "Remember for this show".
    pub remember: &'static str,
    /// The hint of the date choice "Over a range", which names the bulk buttons.
    pub range_hint: &'static str,
    /// The same for "On release dates".
    pub release_hint: &'static str,
}

impl EpisodeWords {
    #[must_use]
    pub fn of(verb: Option<Verb>) -> Self {
        if verb == Some(Verb::Listen) {
            return Self {
                done: "episodes-watched-listen",
                mark: "episodes-mark-listen",
                mark_again: "episodes-mark-again-listen",
                mark_season: "episodes-mark-season-listen",
                mark_show: "episodes-mark-show-listen",
                ask: "episodes-ask-listen",
                remember: "episodes-remember-listen",
                range_hint: "episodes-range-hint-listen",
                release_hint: "episodes-release-hint-listen",
            };
        }
        Self {
            done: "episodes-watched",
            mark: "episodes-mark",
            mark_again: "episodes-mark-again",
            mark_season: "episodes-mark-season",
            mark_show: "episodes-mark-show",
            ask: "episodes-ask",
            remember: "episodes-remember",
            range_hint: "episodes-range-hint",
            release_hint: "episodes-release-hint",
        }
    }
}

/// The step the toast of a saved play dialog names ("Logged: Play"): a board game is played
/// in plays, a video game in sessions (v1-polish P3; the dialog's own words differ the same
/// way, OT-16).
#[must_use]
pub fn play_step_key(board_game: bool) -> &'static str {
    if board_game {
        "log-play-step"
    } else {
        "log-session-step"
    }
}

/// The bulk question on episodes that are all watched (OT-15). Where the answer starts a
/// rewatch run (`starts_run`: the entry is Completed, DEC-589) it reads "Log them again as a
/// rewatch?"; on any other status the answer logs further views and the question says so
/// (v1-polish P6). A podcast says "listened to" and names neither (OT2A-9).
#[must_use]
pub fn again_ask_key(verb: Option<Verb>, starts_run: bool) -> &'static str {
    match verb {
        Some(Verb::Listen) => "episodes-again-ask-listen",
        _ if starts_run => "episodes-again-ask",
        _ => "episodes-again-ask-views",
    }
}

/// "2 already watched, not logged again" of a mixed bulk mark, per verb.
#[must_use]
pub fn skipped_key(verb: Option<Verb>) -> &'static str {
    match verb {
        Some(Verb::Listen) => "episodes-skipped-listen",
        _ => "episodes-skipped",
    }
}

/// The line of the log dialog of an item finished before (OT-7): "Watched once before. This
/// will be watch number 2."; `None` for a first time.
#[must_use]
pub fn again_line(i: &I18n, verb: Option<Verb>, finished: u32) -> Option<String> {
    let key = match verb {
        Some(Verb::Watch) => "log-rewatch-watch",
        Some(Verb::Read) => "log-rewatch-read",
        Some(Verb::Listen) => "log-rewatch-listen",
        Some(Verb::Play) => "log-rewatch-play",
        None => "log-rewatch",
    };
    (finished > 0).then(|| {
        i.t_args(
            key,
            &[
                ("count", finished.into()),
                ("next", finished.saturating_add(1).into()),
            ],
        )
    })
}

/// The facts of a board game in words, each with its label: "Players" "2-4 players",
/// "Playing time" "60-90 min", "Age" "from 10 years" (v1-plan 4b point 7), instead of the
/// raw numbers of the attributes. Facts the source does not have are left out.
#[must_use]
pub fn game_facts(i: &I18n, attributes: &serde_json::Value) -> Vec<(String, String)> {
    let number = |key: &str| attributes.get(key).and_then(serde_json::Value::as_u64);
    let mut out = Vec::new();
    let players = range_text(
        i,
        number("players_min"),
        number("players_max"),
        [
            "game-players",
            "game-players-range",
            "game-players-from",
            "game-players-up-to",
        ],
    );
    if let Some(p) = players {
        out.push((i.t("game-fact-players"), p));
    }
    let time = range_text(
        i,
        number("playtime_min_minutes"),
        number("playtime_max_minutes"),
        [
            "game-minutes",
            "game-minutes-range",
            "game-minutes-from",
            "game-minutes-up-to",
        ],
    );
    if let Some(t) = time {
        out.push((i.t("game-fact-time"), t));
    }
    if let Some(age) = number("min_age") {
        out.push((
            i.t("game-fact-age"),
            i.t_args("game-age", &[("age", age.into())]),
        ));
    }
    out
}

/// A count or a range in words with the keys `[one value, range, from, up to]`.
fn range_text(i: &I18n, min: Option<u64>, max: Option<u64>, keys: [&str; 4]) -> Option<String> {
    let [one, range, from, up_to] = keys;
    Some(match (min, max) {
        (Some(a), Some(b)) if a == b => i.t_args(one, &[("count", a.into())]),
        (Some(a), Some(b)) => {
            i.t_args(range, &[("min", a.min(b).into()), ("max", a.max(b).into())])
        }
        (Some(a), None) => i.t_args(from, &[("min", a.into())]),
        (None, Some(b)) => i.t_args(up_to, &[("max", b.into())]),
        (None, None) => return None,
    })
}

/// Links to the pages of the item elsewhere: "Wikidata" (where its data can be corrected),
/// by its external id, and "BoardGameGeek" (a plain link; nothing is fetched from there), by
/// the `bgg_id` attribute (a reference, not a matching key, LB-36). An identifier that is
/// not one of the source's is never made into a link.
#[must_use]
pub fn outbound_links(
    ids: &[contracts::ExternalIdDto],
    attributes: &serde_json::Value,
) -> Vec<(&'static str, String)> {
    let digits = |s: &str| !s.is_empty() && s.len() <= 12 && s.chars().all(|c| c.is_ascii_digit());
    let mut out = Vec::new();
    for e in ids {
        if e.namespace == "wikidata" && e.value.strip_prefix('Q').is_some_and(digits) {
            out.push((
                "Wikidata",
                format!("https://www.wikidata.org/wiki/{}", e.value),
            ));
        }
    }
    if let Some(bgg) = attributes
        .get("bgg_id")
        .and_then(serde_json::Value::as_u64)
        .filter(|b| (1..=999_999_999_999).contains(b))
    {
        out.push((
            "BoardGameGeek",
            format!("https://boardgamegeek.com/boardgame/{bgg}"),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use contracts::{LibraryEntryDto, NextStepDto, ProgressSummaryDto};

    use super::*;
    use crate::i18n::I18n;

    fn both() -> (I18n, I18n) {
        (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        )
    }

    #[test]
    fn the_verb_comes_from_the_kind_group() {
        assert_eq!(Verb::of_group("watch"), Some(Verb::Watch));
        assert_eq!(Verb::of_group("read"), Some(Verb::Read));
        assert_eq!(Verb::of_group("listen"), Some(Verb::Listen));
        assert_eq!(Verb::of_group("play"), Some(Verb::Play));
        assert_eq!(Verb::of_group("other"), None);
        assert_eq!(Verb::of_group(""), None);
    }

    /// OT-7: counts are "times", with the plural forms of each language.
    #[test]
    fn times_are_counted_in_words_per_verb() {
        let (en, de) = both();
        let w = Some(Verb::Watch);
        assert_eq!(times_text(&en, w, 1), "Watched once");
        assert_eq!(times_text(&en, w, 3), "Watched 3 times");
        assert_eq!(times_text(&de, w, 1), "Einmal gesehen");
        assert_eq!(times_text(&de, w, 3), "3-mal gesehen");
        assert_eq!(times_text(&en, Some(Verb::Read), 1), "Read once");
        assert_eq!(times_text(&en, Some(Verb::Read), 2), "Read 2 times");
        assert_eq!(times_text(&en, Some(Verb::Listen), 1), "Listened once");
        assert_eq!(times_text(&en, Some(Verb::Listen), 4), "Listened 4 times");
        assert_eq!(times_text(&en, Some(Verb::Play), 1), "Played once");
        assert_eq!(times_text(&en, Some(Verb::Play), 14), "Played 14 times");
        assert_eq!(times_text(&de, Some(Verb::Read), 2), "2-mal gelesen");
        assert_eq!(times_text(&de, Some(Verb::Listen), 1), "Einmal geh\u{f6}rt");
        assert_eq!(times_text(&de, Some(Verb::Play), 14), "14-mal gespielt");
        // a kind without a verb of its own
        assert_eq!(times_text(&en, None, 1), "Finished once");
        assert_eq!(times_text(&en, None, 2), "Finished 2 times");
        assert_eq!(times_text(&de, None, 2), "2-mal beendet");
    }

    fn facts<'a>(
        summary: &'a ProgressSummaryDto,
        step: &'a NextStepDto,
        model: &'a str,
        times: u32,
    ) -> Facts<'a> {
        Facts {
            verb: Some(Verb::Watch),
            model,
            times,
            completed: times > 0,
            times_at: summary.last_at.as_deref(),
            added_at: "2026-11-10T09:00:00Z",
            summary,
            step,
        }
    }

    fn step(action: &str) -> NextStepDto {
        NextStepDto {
            action: action.into(),
            ..NextStepDto::default()
        }
    }

    /// OT-5, OT-7: the line under the title says where the person stands, in one sentence.
    #[test]
    fn the_fact_line_reads_per_situation() {
        let (en, de) = both();
        let none = ProgressSummaryDto {
            fraction: Some(0.0),
            ..ProgressSummaryDto::default()
        };
        let finish = step("finish");
        // added, nothing logged
        let planned = facts(&none, &finish, "single", 0);
        assert_eq!(
            fact_line(&en, &planned).as_deref(),
            Some("Added on Nov 10, 2026")
        );
        assert_eq!(
            fact_line(&de, &planned).as_deref(),
            Some("Hinzugef\u{fc}gt am 10.11.2026")
        );
        // watched once, then three times
        let seen = ProgressSummaryDto {
            events: 1,
            finished: 1,
            last_at: Some("2026-11-02T20:00:00Z".into()),
            fraction: Some(1.0),
            ..ProgressSummaryDto::default()
        };
        let once = facts(&seen, &finish, "single", 1);
        assert_eq!(
            fact_line(&en, &once).as_deref(),
            Some("Watched once, on Nov 2, 2026")
        );
        assert_eq!(
            fact_line(&de, &once).as_deref(),
            Some("Einmal gesehen, am 02.11.2026")
        );
        let thrice = facts(&seen, &finish, "single", 3);
        assert_eq!(
            fact_line(&en, &thrice).as_deref(),
            Some("Watched 3 times, last on Nov 2, 2026")
        );
        assert_eq!(
            fact_line(&de, &thrice).as_deref(),
            Some("3-mal gesehen, zuletzt am 02.11.2026")
        );
        // completed by hand, nothing logged: no date to name
        let by_hand = facts(&none, &finish, "single", 1);
        assert_eq!(fact_line(&en, &by_hand).as_deref(), Some("Watched once"));
    }

    /// The line of a show (ux-owner-test.md 1.2): its progress, "Up to date" with every known
    /// episode watched, the count only once it is Completed (review OT2A-2).
    #[test]
    fn the_fact_line_follows_the_progress_model() {
        let (en, de) = both();
        let running = ProgressSummaryDto {
            events: 12,
            episodes_watched: 12,
            episodes_total: Some(24),
            last_at: Some("2026-11-02T20:00:00Z".into()),
            fraction: Some(0.5),
            ..ProgressSummaryDto::default()
        };
        let episode = step("episode");
        assert_eq!(
            fact_line(&en, &facts(&running, &episode, "episodic", 0)).as_deref(),
            Some("12 of 24 episodes")
        );
        let all = ProgressSummaryDto {
            events: 24,
            episodes_watched: 24,
            fraction: Some(1.0),
            ..running.clone()
        };
        let caught_up = step("caught_up");
        // completed: the count, with the day of the last one (ux-owner-test.md 1.2, row
        // "Completed")
        assert_eq!(
            fact_line(&en, &facts(&all, &caught_up, "episodic", 1)).as_deref(),
            Some("Watched once, finished on Nov 2, 2026")
        );
        assert_eq!(
            fact_line(&de, &facts(&all, &caught_up, "episodic", 1)).as_deref(),
            Some("Einmal gesehen, beendet am 02.11.2026")
        );
        // OT2A-2, row "All watched, not completed": every known episode is watched (one
        // whole pass), but the person has not said the show is finished, and the app cannot
        // know (a season may still air): up to date, not "Watched once"
        let open = |times| Facts {
            completed: false,
            ..facts(&all, &caught_up, "episodic", times)
        };
        assert_eq!(
            fact_line(&en, &open(1)).as_deref(),
            Some("Up to date: 24 of 24 episodes")
        );
        assert_eq!(
            fact_line(&de, &open(1)).as_deref(),
            Some("Auf dem neuesten Stand: 24 von 24 Folgen")
        );
        assert_eq!(
            fact_line(&en, &open(0)).as_deref(),
            Some("Up to date: 24 of 24 episodes")
        );
        // not completed and episodes open again (a new one became known): the plain progress
        let behind = Facts {
            completed: false,
            ..facts(&running, &episode, "episodic", 1)
        };
        assert_eq!(
            fact_line(&en, &behind).as_deref(),
            Some("12 of 24 episodes")
        );
        // a movie counts whatever its status says (a rewatch does not move it)
        let seen = ProgressSummaryDto {
            events: 1,
            finished: 1,
            last_at: Some("2026-11-02T20:00:00Z".into()),
            ..ProgressSummaryDto::default()
        };
        let finish = step("finish");
        let movie = Facts {
            completed: false,
            ..facts(&seen, &finish, "single", 1)
        };
        assert_eq!(
            fact_line(&en, &movie).as_deref(),
            Some("Watched once, on Nov 2, 2026")
        );
    }

    /// ot2e (DEC-537 model A, ux-owner-test.md 1.2): a running rewatch says how far it is
    /// and what was before; a stopped one keeps the count and says where it stopped.
    #[test]
    fn the_fact_line_of_a_rewatch() {
        let (en, de) = both();
        let run = |done, running| contracts::RewatchDto {
            start: "e9".into(),
            done,
            running,
        };
        let running = ProgressSummaryDto {
            events: 30,
            episodes_watched: 24,
            episodes_total: Some(24),
            last_at: Some("2026-11-09T20:00:00Z".into()),
            fraction: Some(5.0 / 24.0),
            rewatch: Some(run(5, true)),
            ..ProgressSummaryDto::default()
        };
        let episode = step("episode");
        let now = Facts {
            completed: false,
            ..facts(&running, &episode, "episodic", 1)
        };
        assert_eq!(
            fact_line(&en, &now).as_deref(),
            Some("Rewatch: 5 of 24 episodes. Watched once before.")
        );
        assert_eq!(
            fact_line(&de, &now).as_deref(),
            Some("Wiederholung: 5 von 24 Folgen. Davor einmal gesehen.")
        );
        let third = Facts { times: 2, ..now };
        assert_eq!(
            fact_line(&en, &third).as_deref(),
            Some("Rewatch: 5 of 24 episodes. Watched 2 times before.")
        );
        // a podcast is listened to
        let heard = Facts {
            verb: Some(Verb::Listen),
            ..now
        };
        assert_eq!(
            fact_line(&en, &heard).as_deref(),
            Some("Listening again: 5 of 24 episodes. Listened once before.")
        );
        // an item that does not know its episodes
        let open = ProgressSummaryDto {
            episodes_total: None,
            fraction: None,
            ..running.clone()
        };
        let open_now = Facts {
            completed: false,
            ..facts(&open, &episode, "episodic", 1)
        };
        assert_eq!(
            fact_line(&en, &open_now).as_deref(),
            Some("Rewatch: 5 episodes. Watched once before.")
        );

        // stopped: Completed again, the count, and where the rewatch stopped (no day)
        let stopped = ProgressSummaryDto {
            rewatch: Some(run(5, false)),
            fraction: Some(1.0),
            ..running.clone()
        };
        let caught_up = step("caught_up");
        let after = facts(&stopped, &caught_up, "episodic", 1);
        assert_eq!(
            fact_line(&en, &after).as_deref(),
            Some("Watched once; rewatch stopped after 5 of 24 episodes")
        );
        assert_eq!(
            fact_line(&de, &after).as_deref(),
            Some("Einmal gesehen; Wiederholung nach 5 von 24 Folgen abgebrochen")
        );
        let one = ProgressSummaryDto {
            rewatch: Some(run(1, false)),
            episodes_total: None,
            ..stopped.clone()
        };
        assert_eq!(
            fact_line(&en, &facts(&one, &caught_up, "episodic", 1)).as_deref(),
            Some("Watched once; rewatch stopped after one episode")
        );
    }

    /// Review OT2A-7: the day of the count line is the day of the pass it names, not of the
    /// newest view; a pass without a day has none.
    #[test]
    fn the_count_line_names_the_day_of_its_pass() {
        let (en, _) = both();
        let all = ProgressSummaryDto {
            events: 25,
            episodes_watched: 24,
            episodes_total: Some(24),
            // one favourite episode again, last week
            last_at: Some("2026-11-02T20:00:00Z".into()),
            fraction: Some(1.0),
            ..ProgressSummaryDto::default()
        };
        let caught_up = step("caught_up");
        let show = Facts {
            times_at: Some("2025-03-09T20:00:00Z"),
            ..facts(&all, &caught_up, "episodic", 1)
        };
        assert_eq!(
            fact_line(&en, &show).as_deref(),
            Some("Watched once, finished on Mar 9, 2025")
        );
        let by_hand = Facts {
            times_at: None,
            ..show
        };
        assert_eq!(fact_line(&en, &by_hand).as_deref(), Some("Watched once"));
    }

    /// The words of the menu and the toasts of a run, per verb.
    #[test]
    fn a_rewatch_is_named_per_verb() {
        let (en, de) = both();
        let watch = RewatchWords::of(Some(Verb::Watch));
        let listen = RewatchWords::of(Some(Verb::Listen));
        assert_eq!(en.t(watch.start), "Start a rewatch");
        assert_eq!(en.t(watch.stop), "Stop the rewatch");
        assert_eq!(en.t(watch.started), "Rewatch started");
        assert_eq!(de.t(watch.start), "Nochmal von vorn schauen");
        assert_eq!(en.t(listen.start), "Start listening again");
        // review OT2E-10: "Stop listening again" read as "once more, stop listening"
        assert_eq!(en.t(listen.stop), "Stop this listen-through");
        // a stop is a marker with its own toast (DEC-590)
        assert_eq!(en.t(watch.stopped_toast), "Rewatch stopped");
        assert_eq!(en.t(listen.stopped_toast), "Stopped listening again");
        // review OT2E-10: in German a finished pass is "beendet", a stopped run "abgebrochen"
        assert_eq!(de.t(watch.stop), "Wiederholung abbrechen");
        assert_eq!(de.t(listen.stop), "Erneutes H\u{f6}ren abbrechen");
        assert_eq!(de.t(watch.stopped_toast), "Wiederholung abgebrochen");
        assert_eq!(
            de.t(listen.stopped_toast),
            "Erneutes H\u{f6}ren abgebrochen"
        );
        let title = [("title", "Dark".into())];
        assert_eq!(en.t_args(watch.finished, &title), "Rewatch finished: Dark");
        assert_eq!(
            en.t_args(listen.finished, &title),
            "Finished listening again: Dark"
        );
        assert_eq!(
            de.t_args(watch.finished, &title),
            "Wiederholung beendet: Dark"
        );
        assert_eq!(
            de.t_args(listen.finished, &title),
            "Erneutes H\u{f6}ren beendet: Dark"
        );

        // a kind without a verb of its own reads like a show
        assert_eq!(RewatchWords::of(None), watch);
    }

    /// The same line for a game and a book.
    #[test]
    fn the_fact_line_of_plays_and_pages() {
        let (en, _) = both();
        // plays
        let played = ProgressSummaryDto {
            events: 2,
            sessions: 2,
            last_at: Some("2026-11-02T20:00:00Z".into()),
            ..ProgressSummaryDto::default()
        };
        let session = step("session");
        let game = Facts {
            verb: Some(Verb::Play),
            ..facts(&played, &session, "sessions", 2)
        };
        assert_eq!(
            fact_line(&en, &game).as_deref(),
            Some("Played 2 times, last on Nov 2, 2026")
        );
        // with logged time the line ends with the total (ot2f)
        let line = fact_line(&en, &game).unwrap();
        assert_eq!(
            with_play_time(&en, line.clone(), 1_860),
            "Played 2 times, last on Nov 2, 2026, 31 hours in total"
        );
        assert_eq!(
            with_play_time(&en, line.clone(), 45).rsplit(", ").next(),
            Some("45 minutes in total")
        );
        assert_eq!(with_play_time(&en, line.clone(), 0), line);
        let de = both().1;
        let de_line = fact_line(&de, &game).unwrap();
        assert!(
            with_play_time(&de, de_line, 1_860).ends_with(", insgesamt 31 Stunden"),
            "de"
        );

        // a book on its way: the page
        let reading = ProgressSummaryDto {
            events: 1,
            page: Some(120),
            fraction: Some(0.5),
            ..ProgressSummaryDto::default()
        };
        let page = NextStepDto {
            page: Some(120),
            pages: Some(240),
            ..step("page")
        };
        let book = Facts {
            verb: Some(Verb::Read),
            ..facts(&reading, &page, "paged", 0)
        };
        assert_eq!(fact_line(&en, &book).as_deref(), Some("Page 120 of 240"));
        // logged, but nothing to say in numbers
        let bare = ProgressSummaryDto {
            events: 1,
            ..ProgressSummaryDto::default()
        };
        assert_eq!(fact_line(&en, &facts(&bare, &page, "paged", 0)), None);
    }

    /// OT-7: the sheet says which watch this will be.
    #[test]
    fn the_sheet_names_the_number_of_the_next_watch() {
        let (en, de) = both();
        assert_eq!(again_line(&en, Some(Verb::Watch), 0), None);
        assert_eq!(
            again_line(&en, Some(Verb::Watch), 1).as_deref(),
            Some("Watched once before. This will be watch number 2.")
        );
        assert_eq!(
            again_line(&en, Some(Verb::Watch), 2).as_deref(),
            Some("Watched 2 times before. This will be watch number 3.")
        );
        assert_eq!(
            again_line(&de, Some(Verb::Watch), 1).as_deref(),
            Some("Schon einmal gesehen. Das wird Nummer 2.")
        );
        assert_eq!(
            again_line(&de, Some(Verb::Watch), 2).as_deref(),
            Some("Schon 2-mal gesehen. Das wird Nummer 3.")
        );
        // other verbs: no "watch number"
        assert_eq!(
            again_line(&en, Some(Verb::Listen), 1).as_deref(),
            Some("Listened once before. This will be number 2.")
        );
        assert_eq!(
            again_line(&en, None, 1).as_deref(),
            Some("Finished once before. This will be number 2.")
        );
    }

    /// OT2A-1: the dialog line counts what the header counts, and only for a step that
    /// says "again".
    #[test]
    fn the_dialog_line_counts_like_the_header() {
        let again = NextStepDto {
            again: true,
            ..step("finish")
        };
        assert_eq!(again_count(&again, 2), 2);
        // completed by hand, nothing logged: the log is that first watch
        assert_eq!(again_count(&step("finish"), 1), 0);
        // one episode of a show watched through once is not "watch number 2" of the show
        assert_eq!(again_count(&step("episode"), 1), 0);
        assert_eq!(again_count(&step("caught_up"), 1), 0);
    }

    /// OT2A-9: the bulk question, the skipped line and the row menu say "listened" for a
    /// podcast.
    #[test]
    fn the_bulk_question_follows_the_verb() {
        let (en, de) = both();
        let ask = |i: &I18n, v, n: u32| i.t_args(again_ask_key(v, true), &[("count", n.into())]);
        let skipped = |i: &I18n, v, n: u32| i.t_args(skipped_key(v), &[("count", n.into())]);
        let l = Some(Verb::Listen);
        assert_eq!(
            ask(&en, l, 10),
            "These 10 episodes are already listened to. Log them again?"
        );
        assert_eq!(
            ask(&en, l, 1),
            "This episode is already listened to. Log it again?"
        );
        assert_eq!(
            ask(&de, l, 10),
            "Diese 10 Folgen sind schon geh\u{f6}rt. Nochmal eintragen?"
        );
        assert_eq!(
            ask(&de, l, 1),
            "Diese Folge ist schon geh\u{f6}rt. Nochmal eintragen?"
        );
        assert_eq!(
            skipped(&en, l, 2),
            "2 already listened to, not logged again"
        );
        assert_eq!(
            skipped(&en, l, 1),
            "One already listened to, not logged again"
        );
        assert_eq!(
            skipped(&de, l, 2),
            "2 schon geh\u{f6}rt, nicht erneut eingetragen"
        );
        // every other verb keeps the watch words
        let w = Some(Verb::Watch);
        assert_eq!(
            ask(&en, w, 10),
            "These 10 episodes are already watched. Log them again as a rewatch?"
        );
        assert_eq!(skipped(&en, None, 2), "2 already watched, not logged again");
        assert_eq!(en.t(with_button_key(l)), "Listened with...");
    }

    /// OT2A-3: Undo of a first log removes the entry only while nothing else was done to it.
    #[test]
    fn only_an_untouched_entry_goes_with_the_undo() {
        let entry = LibraryEntryDto {
            id: "e".into(),
            item_id: "i".into(),
            kind: "movie".into(),
            status: "planned".into(),
            rating: None,
            notes: None,
            added_at: "2026-11-10T09:00:00Z".into(),
        };
        assert!(entry_untouched(&entry, 0, 0));
        let empty_note = LibraryEntryDto {
            notes: Some(String::new()),
            ..entry.clone()
        };
        assert!(entry_untouched(&empty_note, 0, 0));
        // another log, a tag, a rating, a note or a status of its own: it stays
        assert!(!entry_untouched(&entry, 1, 0));
        assert!(!entry_untouched(&entry, 0, 1));
        let rated = LibraryEntryDto {
            rating: Some(80),
            ..entry.clone()
        };
        assert!(!entry_untouched(&rated, 0, 0));
        let noted = LibraryEntryDto {
            notes: Some("with Ben".into()),
            ..entry.clone()
        };
        assert!(!entry_untouched(&noted, 0, 0));
        let completed = LibraryEntryDto {
            status: "completed".into(),
            ..entry
        };
        assert!(!entry_untouched(&completed, 0, 0));
    }

    /// OT2A-8: the Undo of "Added to your library as Planned" removes the entry only while
    /// it is as the add left it; a watch logged or a rating given meanwhile keeps it, and
    /// the toast says so in both languages.
    #[test]
    fn the_undo_of_an_add_keeps_a_changed_entry() {
        let entry = LibraryEntryDto {
            id: "e".into(),
            item_id: "i".into(),
            kind: "movie".into(),
            status: "planned".into(),
            rating: None,
            notes: None,
            added_at: "2026-11-10T09:00:00Z".into(),
        };
        assert_eq!(add_undo(Some(&entry), 0, 0), AddUndo::Remove);
        assert_eq!(add_undo(Some(&entry), 1, 0), AddUndo::Keep);
        let completed = LibraryEntryDto {
            status: "completed".into(),
            ..entry.clone()
        };
        assert_eq!(add_undo(Some(&completed), 1, 0), AddUndo::Keep);
        let rated = LibraryEntryDto {
            rating: Some(80),
            ..entry
        };
        assert_eq!(add_undo(Some(&rated), 0, 0), AddUndo::Keep);
        // removed by hand meanwhile: nothing is left to take back
        assert_eq!(add_undo(None, 0, 0), AddUndo::Gone);
        let (en, de) = both();
        assert_eq!(
            en.t("item-added-kept"),
            "Kept in your library: it has changed since you added it."
        );
        assert_eq!(
            de.t("item-added-kept"),
            "Bleibt in der Bibliothek: Der Eintrag wurde seit dem Hinzuf\u{fc}gen ge\u{e4}ndert."
        );
        assert_eq!(
            (en.t("log-cancel"), de.t("log-cancel")),
            ("Cancel".into(), "Abbrechen".into())
        );
    }

    /// OT2A-3: the first log of an item outside the library says that it was added.
    #[test]
    fn the_first_log_says_that_the_item_was_added() {
        let (en, de) = both();
        let said = |i: &I18n| i.t_args("item-logged-added", &[("title", "Heat".into())]);
        assert_eq!(said(&en), "Heat, added to your library");
        assert_eq!(said(&de), "Heat, in die Bibliothek aufgenommen");
    }

    /// OT-4, OT-6: the second button and the "with" family per verb, in both languages.
    #[test]
    fn want_and_with_follow_the_verb() {
        let (en, de) = both();
        let texts = |i: &I18n, v: Option<Verb>| {
            (
                i.t(want_key(v)),
                i.t(with_button_key(v)),
                i.t(with_legend_key(v)),
            )
        };
        assert_eq!(
            texts(&en, Some(Verb::Watch)),
            (
                "Want to watch".into(),
                "Watched with...".into(),
                "Watched with".into()
            )
        );
        assert_eq!(
            texts(&en, Some(Verb::Listen)),
            (
                "Want to listen".into(),
                "Listened with...".into(),
                "Listened with".into()
            )
        );
        assert_eq!(
            texts(&en, Some(Verb::Read)),
            (
                "Want to read".into(),
                "Read with...".into(),
                "Read with".into()
            )
        );
        assert_eq!(
            texts(&en, Some(Verb::Play)),
            (
                "Want to play".into(),
                "Played with...".into(),
                "Played with".into()
            )
        );
        // no verb: the plain add, the plain "with"
        assert_eq!(
            texts(&en, None),
            ("Add to library".into(), "With...".into(), "With".into())
        );
        assert_eq!(
            texts(&de, Some(Verb::Watch)),
            (
                "M\u{f6}chte ich sehen".into(),
                "Gesehen mit...".into(),
                "Gesehen mit".into()
            )
        );
        assert_eq!(
            texts(&de, Some(Verb::Play)),
            (
                "M\u{f6}chte ich spielen".into(),
                "Gespielt mit...".into(),
                "Gespielt mit".into()
            )
        );
        assert_eq!(
            en.t("item-added-planned"),
            "Added to your library as Planned"
        );
        assert_eq!(
            de.t("item-added-planned"),
            "Als \u{201e}Geplant\u{201c} in die Bibliothek aufgenommen"
        );
    }

    /// v1-polish P3: the episodes of a podcast are listened to, in every label of the tab.
    #[test]
    fn a_podcasts_episode_rows_say_listened() {
        let (en, de) = both();
        let n = [("season", 1.into()), ("episode", 2.into())];
        let texts = |i: &I18n, v| {
            let w = EpisodeWords::of(v);
            [
                i.t(w.done),
                i.t_args(w.mark, &n),
                i.t_args(w.mark_again, &n),
                i.t(w.mark_season),
                i.t(w.mark_show),
                i.t_args(w.ask, &[("count", 1.into())]),
                i.t_args(w.ask, &[("count", 3.into())]),
                i.t(w.remember),
            ]
        };
        assert_eq!(
            texts(&en, Some(Verb::Listen)),
            [
                "Listened",
                "Mark S1 E2 listened",
                "Listen to S1 E2 again",
                "Mark season listened",
                "Mark podcast listened",
                "Also mark the earlier episode as listened?",
                "Also mark the 3 earlier episodes as listened?",
                "Remember for this podcast",
            ]
        );
        assert_eq!(
            texts(&de, Some(Verb::Listen)),
            [
                "Geh\u{f6}rt",
                "S1 E2 als geh\u{f6}rt markieren",
                "S1 E2 nochmal h\u{f6}ren",
                "Staffel als geh\u{f6}rt markieren",
                "Podcast als geh\u{f6}rt markieren",
                "Auch die fr\u{fc}here Folge als geh\u{f6}rt markieren?",
                "Auch die 3 fr\u{fc}heren Folgen als geh\u{f6}rt markieren?",
                "F\u{fc}r diesen Podcast merken",
            ]
        );
        // a show keeps the words the owner approved
        for v in [Some(Verb::Watch), None] {
            assert_eq!(
                texts(&en, v),
                [
                    "Watched",
                    "Mark S1 E2 watched",
                    "Watch S1 E2 again",
                    "Mark season watched",
                    "Mark show watched",
                    "Also mark the earlier episode as watched?",
                    "Also mark the 3 earlier episodes as watched?",
                    "Remember for this show",
                ]
            );
        }
        assert_eq!(texts(&de, Some(Verb::Watch))[0], "Gesehen");
    }

    /// Review POL-6: the hints of the date choice name the bulk buttons; a podcast has no
    /// button "Mark show".
    #[test]
    fn the_date_hints_of_a_podcast_name_its_own_buttons() {
        let (en, de) = both();
        let hints = |i: &I18n, v| {
            let w = EpisodeWords::of(v);
            [i.t(w.range_hint), i.t(w.release_hint)]
        };
        assert_eq!(
            hints(&en, Some(Verb::Listen)),
            [
                "Mark season, Mark podcast and Mark up to here spread the episodes evenly from the first to the last day.",
                "Mark season, Mark podcast and Mark up to here log each episode on the day it was released.",
            ]
        );
        assert_eq!(
            hints(&de, Some(Verb::Listen)),
            [
                "Staffel, Podcast und bis hier markieren verteilen die Folgen gleichm\u{e4}\u{df}ig vom ersten bis zum letzten Tag.",
                "Staffel, Podcast und bis hier markieren tragen jede Folge an dem Tag ein, an dem sie erschienen ist.",
            ]
        );
        for v in [Some(Verb::Watch), None] {
            assert_eq!(
                hints(&en, v),
                [
                    "Mark season, Mark show and Mark up to here spread the episodes evenly from the first to the last day.",
                    "Mark season, Mark show and Mark up to here log each episode on the day it was released.",
                ]
            );
        }
    }

    /// v1-polish P3: the toast of a video game session does not say "Play".
    #[test]
    fn the_toast_of_a_session_names_a_session_and_of_a_board_game_a_play() {
        let (en, de) = both();
        let toast = |i: &I18n, board| {
            i.t_args(
                "library-logged",
                &[("step", i.t(play_step_key(board)).into())],
            )
        };
        assert_eq!(toast(&en, false), "Logged: Session");
        assert_eq!(toast(&en, true), "Logged: Play");
        assert_eq!(toast(&de, false), "Eingetragen: Sitzung");
        assert_eq!(toast(&de, true), "Eingetragen: Partie");
    }

    /// v1-polish P6: "as a rewatch" only where the answer starts a rewatch run.
    #[test]
    fn the_bulk_question_names_a_rewatch_only_where_one_starts() {
        let (en, de) = both();
        let ask =
            |i: &I18n, v, run, n: u32| i.t_args(again_ask_key(v, run), &[("count", n.into())]);
        let w = Some(Verb::Watch);
        assert_eq!(
            ask(&en, w, true, 5),
            "These 5 episodes are already watched. Log them again as a rewatch?"
        );
        assert_eq!(
            ask(&en, w, false, 5),
            "These 5 episodes are already watched. Log them again as further views?"
        );
        assert_eq!(
            ask(&en, w, false, 1),
            "This episode is already watched. Log it again as a further view?"
        );
        // (review POL-2: no "Nochmal als weitere Male")
        assert_eq!(
            ask(&de, w, false, 5),
            "Diese 5 Folgen sind schon gesehen. Alle ein weiteres Mal eintragen?"
        );
        assert_eq!(
            ask(&de, w, false, 1),
            "Diese Folge ist schon gesehen. Ein weiteres Mal eintragen?"
        );
        // a podcast names neither, with or without a run
        let l = Some(Verb::Listen);
        assert_eq!(ask(&en, l, false, 2), ask(&en, l, true, 2));
        assert_eq!(
            ask(&en, l, false, 2),
            "These 2 episodes are already listened to. Log them again?"
        );
    }

    fn facts_of(i: &I18n, attributes: &serde_json::Value) -> Vec<String> {
        game_facts(i, attributes)
            .into_iter()
            .map(|(label, text)| format!("{label}: {text}"))
            .collect()
    }

    /// v1-plan 4b point 7: players, time and age of a board game in words, en and de.
    #[test]
    fn a_board_game_reads_players_time_and_age_in_words() {
        let (en, de) = both();
        let catan = serde_json::json!({"players_min": 3, "players_max": 4,
            "playtime_min_minutes": 60, "playtime_max_minutes": 90, "min_age": 10,
            "bgg_id": 13});
        assert_eq!(
            facts_of(&en, &catan),
            [
                "Players: 3-4 players",
                "Playing time: 60-90 min",
                "Age: from 10 years"
            ]
        );
        assert_eq!(
            facts_of(&de, &catan),
            [
                "Mitspielende: 3-4 Personen",
                "Spieldauer: 60-90 Min.",
                "Alter: ab 10 Jahren"
            ]
        );
        let duel = serde_json::json!({"players_min": 2, "players_max": 2,
            "playtime_min_minutes": 60, "playtime_max_minutes": 60});
        assert_eq!(
            facts_of(&en, &duel),
            ["Players: 2 players", "Playing time: 60 min"]
        );
        assert_eq!(
            facts_of(&de, &duel),
            ["Mitspielende: 2 Personen", "Spieldauer: 60 Min."]
        );
        let solo = serde_json::json!({"players_min": 1, "players_max": 1});
        assert_eq!(facts_of(&en, &solo), ["Players: 1 player"]);
        assert_eq!(facts_of(&de, &solo), ["Mitspielende: 1 Person"]);
    }

    #[test]
    fn half_a_range_says_which_half_and_nothing_says_nothing() {
        let (en, de) = both();
        let from = serde_json::json!({"players_min": 2});
        assert_eq!(facts_of(&en, &from), ["Players: from 2 players"]);
        assert_eq!(facts_of(&de, &from), ["Mitspielende: ab 2 Personen"]);
        let up_to = serde_json::json!({"players_max": 6});
        assert_eq!(facts_of(&en, &up_to), ["Players: up to 6 players"]);
        assert_eq!(facts_of(&de, &up_to), ["Mitspielende: bis 6 Personen"]);
        // a range written the wrong way round reads the right way round
        let swapped = serde_json::json!({"playtime_min_minutes": 90, "playtime_max_minutes": 30});
        assert_eq!(facts_of(&en, &swapped), ["Playing time: 30-90 min"]);
        assert_eq!(game_facts(&en, &serde_json::json!({})).len(), 0);
        assert_eq!(
            game_facts(&en, &serde_json::json!({"players_min": "two"})).len(),
            0
        );
    }

    #[test]
    fn a_game_links_to_wikidata_and_boardgamegeek() {
        let id = |namespace: &str, value: &str| contracts::ExternalIdDto {
            namespace: namespace.into(),
            value: value.into(),
        };
        // the BGG id is a reference in the attributes, not a matching key (WD-3)
        let links = outbound_links(
            &[id("wikidata", "Q17271")],
            &serde_json::json!({"bgg_id": 13, "gtins": ["0688623905027"]}),
        );
        assert_eq!(
            links,
            [
                (
                    "Wikidata",
                    "https://www.wikidata.org/wiki/Q17271".to_string()
                ),
                (
                    "BoardGameGeek",
                    "https://boardgamegeek.com/boardgame/13".to_string()
                ),
            ]
        );
        // what is no id of the source is never made into a link
        let odd = outbound_links(&[id("wikidata", "Q1/../x")], &serde_json::json!({}));
        assert_eq!(odd.len(), 0);
        for bgg in [
            serde_json::json!({"bgg_id": "13?x=1"}),
            serde_json::json!({"bgg_id": 0}),
            serde_json::json!({"bgg_id": -13}),
            serde_json::json!({"bgg_id": 1_000_000_000_000_u64}),
        ] {
            assert_eq!(outbound_links(&[], &bgg).len(), 0, "{bgg}");
        }
    }
}
