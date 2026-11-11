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

/// The bulk question "These N episodes are already watched. Log them again as a rewatch?"
/// (OT-15); a podcast says "listened to" (OT2A-9).
#[must_use]
pub fn again_ask_key(verb: Option<Verb>) -> &'static str {
    match verb {
        Some(Verb::Listen) => "episodes-again-ask-listen",
        _ => "episodes-again-ask",
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
        let ask = |i: &I18n, v, n: u32| i.t_args(again_ask_key(v), &[("count", n.into())]);
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
}
