//! The "Plays" tab of a game page and the win rate of the Stats Play tab (owner test OT-11,
//! design.md 6.3; ot2f): a strip with plays, time played and the person's own win rate, and
//! a table of the plays (date, players, scores, result, duration), newest first. The words
//! are built here as plain data and tested; the components only lay them out.

use contracts::{ItemPageDto, PlayTotalsDto};
use leptos::prelude::*;
use ui::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};

use crate::charts::Measure;
use crate::history::{PlayWords, play_words};
use crate::i18n::{I18n, use_i18n};

/// One play as the table shows it; a cell without anything to say is a dash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayRow {
    pub session_id: String,
    pub date: String,
    pub players: String,
    pub scores: String,
    pub result: String,
    pub duration: String,
}

/// What a cell says when nothing was logged for it.
pub const NOTHING: &str = "-";

/// "45 min", "2 h", "1 h 35 min".
fn duration_text(i: &I18n, minutes: u32) -> String {
    let (hours, rest) = (minutes / 60, minutes % 60);
    if hours == 0 {
        let minutes = i.number(f64::from(minutes));
        return i.t_args("history-play-duration", &[("minutes", minutes.into())]);
    }
    let hours = i.number(f64::from(hours));
    if rest == 0 {
        i.t_args("plays-duration-h", &[("hours", hours.into())])
    } else {
        i.t_args(
            "plays-duration-hm",
            &[
                ("hours", hours.into()),
                ("minutes", rest.to_string().into()),
            ],
        )
    }
}

/// The plays the page carries, newest first: each `session` event of the history with the
/// play read for it (at most 50, `ItemPageDto::plays`). The date is the day of the event
/// as [`I18n::date`] reads it (the History tab's day).
#[must_use]
pub fn play_rows(i: &I18n, page: &ItemPageDto) -> Vec<PlayRow> {
    let or_dash = |text: Option<String>| text.unwrap_or_else(|| NOTHING.to_string());
    page.progress
        .iter()
        .rev()
        .filter_map(|event| {
            let id = event.progress["session_id"].as_str()?;
            let play = page.plays.iter().find(|p| p.session_id == id)?;
            let words: PlayWords = play_words(i, &play.participants, &play.play);
            let players: Vec<String> = play
                .participants
                .iter()
                .map(|p| {
                    if p.is_me {
                        i.t("history-play-you")
                    } else {
                        p.name.clone()
                    }
                })
                .collect();
            Some(PlayRow {
                session_id: id.to_string(),
                date: i.date(&event.occurred_at),
                players: or_dash((!players.is_empty()).then(|| players.join(", "))),
                scores: or_dash(words.scores_bare),
                result: or_dash(words.result),
                duration: or_dash(words.duration_minutes.map(|m| duration_text(i, m))),
            })
        })
        .collect()
}

/// The win rate in words: "67%" and "2 of 3 plays with a result in your own log" (the rate
/// counts the person's own log, not plays that others shared with them; review OT2F-8);
/// `None` before the first play with a result of the person.
#[must_use]
pub fn win_rate(i: &I18n, t: &PlayTotalsDto) -> Option<(String, String)> {
    if t.decided == 0 {
        return None;
    }
    // rounded to a whole percent
    let percent = (u64::from(t.wins) * 100 + u64::from(t.decided) / 2) / u64::from(t.decided);
    Some((
        i.t_args(
            "plays-rate-value",
            &[("percent", percent.to_string().into())],
        ),
        i.t_args(
            "plays-rate-detail",
            &[("wins", t.wins.into()), ("decided", t.decided.into())],
        ),
    ))
}

/// The plays a page counts (review OT2F-2). A play is counted from its history event, as the
/// header of the game ("Played 3 times"), the History tab, the totals row and the tiles
/// count it; `bare` of them have no play read for them (a history that was imported from an
/// export file has the events only), so they are in no time and no rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayCount {
    pub plays: u32,
    pub bare: u32,
}

/// `events`: the plays the history counts (`ItemPageDto::times` of a game, the plays of the
/// Stats totals row); `t`: what was read from the plays themselves.
#[must_use]
pub fn play_count(events: u32, t: &PlayTotalsDto) -> PlayCount {
    let plays = events.max(t.plays);
    PlayCount {
        plays,
        bare: plays - t.plays,
    }
}

/// What the page says about plays without details; `None` when every play has them.
#[must_use]
pub fn bare_note(i: &I18n, count: PlayCount) -> Option<String> {
    (count.bare > 0).then(|| i.t_args("plays-bare", &[("count", count.bare.into())]))
}

/// The strip above the table: label and value of plays (`plays`: every play counted,
/// [`play_count`]), time played and win rate. Time that was never logged and a rate without
/// any result say so instead of reading as zero (OT-20).
#[must_use]
pub fn strip(i: &I18n, t: &PlayTotalsDto, plays: u32) -> Vec<(String, String)> {
    let time = if t.minutes == 0 {
        i.t("plays-time-none")
    } else {
        Measure::Hours.counted(i, t.minutes)
    };
    let rate = win_rate(i, t).map_or_else(|| i.t("plays-rate-none"), |(rate, _)| rate);
    vec![
        (i.t("plays-total"), i.number(f64::from(plays))),
        (i.t("plays-time"), time),
        (i.t("plays-rate"), rate),
    ]
}

/// The strip as a description list; under the win rate, what it is a rate of.
#[component]
pub fn PlayStrip(totals: PlayTotalsDto, plays: u32) -> impl IntoView {
    let i18n = use_i18n();
    let detail = win_rate(&i18n, &totals).map(|(_, detail)| detail);
    let last = 2;
    view! {
        <dl class="grid max-w-2xl grid-cols-2 gap-3 sm:grid-cols-3" data-testid="ip-plays-strip">
            {strip(&i18n, &totals, plays).into_iter().enumerate().map(|(n, (label, value))| {
                let detail = detail.clone().filter(|_| n == last);
                view! {
                    <div class="rounded-xl border border-border p-3">
                        <dt class="text-xs text-muted-foreground">{label}</dt>
                        <dd class="text-lg font-semibold tabular-nums">{value}</dd>
                        {detail.map(|d| view! { <dd class="text-xs text-muted-foreground">{d}</dd> })}
                    </div>
                }
            }).collect_view()}
        </dl>
    }
}

/// The "Plays" tab: the strip, then the plays as a table (cards on a phone), newest first.
#[component]
pub fn PlaysTab(page: ItemPageDto) -> impl IntoView {
    let i18n = use_i18n();
    let rows = play_rows(&i18n, &page);
    let totals = page.play_totals.unwrap_or_default();
    let count = play_count(page.times, &totals);
    if rows.is_empty() && count.plays == 0 {
        return view! { <p class="py-4 text-sm text-muted-foreground">{i18n.t("plays-empty")}</p> }
            .into_any();
    }
    let title = crate::i18n::title_text(&page.item.title);
    let caption = i18n.t_args("plays-caption", &[("title", title.into())]);
    let shown = u32::try_from(rows.len()).unwrap_or(u32::MAX);
    let more = (totals.plays > shown).then(|| {
        i18n.t_args(
            "plays-newest",
            &[("shown", shown.into()), ("count", totals.plays.into())],
        )
    });
    let cols = [
        i18n.t("plays-col-date"),
        i18n.t("plays-col-players"),
        i18n.t("plays-col-scores"),
        i18n.t("plays-col-result"),
        i18n.t("plays-col-duration"),
    ];
    let bare = bare_note(&i18n, count);
    let listed = !rows.is_empty();
    let heads = cols.clone();
    let body = rows
        .into_iter()
        .map(|r| {
            let cells = [r.date, r.players, r.scores, r.result, r.duration];
            let cells = cells
                .into_iter()
                .zip(cols.clone())
                .map(|(text, label)| view! { <TableCell label=label>{text}</TableCell> })
                .collect_view();
            view! { <TableRow>{cells}</TableRow> }
        })
        .collect_view();
    view! {
        <div class="grid gap-4 py-2" data-testid="ip-plays">
            <PlayStrip totals=totals plays=count.plays/>
            {bare.map(|b| view! { <p class="text-sm text-muted-foreground" data-testid="ip-plays-bare">{b}</p> })}
            {listed.then(|| view! {
                <Table stacked=true>
                    <caption class="sr-only">{caption}</caption>
                    <TableHeader>
                        <TableRow>
                            {heads.into_iter().map(|h| view! { <TableHead>{h}</TableHead> }).collect_view()}
                        </TableRow>
                    </TableHeader>
                    <TableBody>{body}</TableBody>
                </Table>
            })}
            {more.map(|m| view! { <p class="text-sm text-muted-foreground" data-testid="ip-plays-more">{m}</p> })}
        </div>
    }
    .into_any()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn page() -> ItemPageDto {
        serde_json::from_value(serde_json::json!({
            "item": {
                "id": "i", "kind": "board_game", "title": "Azul", "original_title": null,
                "year": 2017, "description": null, "cover": null, "external_ids": [],
                "attributes": {}, "state": "full", "provenance": {}
            },
            "entry": null,
            "progress": [
                {"id": "e1", "item_id": "i", "occurred_at": "2026-03-01T20:00:00Z",
                 "progress": {"type": "session", "session_id": "s1"}},
                {"id": "e2", "item_id": "i", "occurred_at": "2026-03-02T20:00:00Z",
                 "progress": {"type": "session", "session_id": "s2"}},
                {"id": "e3", "item_id": "i", "occurred_at": "2026-03-03T20:00:00Z",
                 "progress": {"type": "finished"}},
                {"id": "e4", "item_id": "i", "occurred_at": "2026-03-04T20:00:00Z",
                 "progress": {"type": "session", "session_id": "s4"}},
                {"id": "e5", "item_id": "i", "occurred_at": "2026-03-05T20:00:00Z",
                 "progress": {"type": "session", "session_id": "s5"}}
            ],
            "sources": [],
            "plays": [
                {"session_id": "s4", "play": {"duration_minutes": 120, "coop": "none"},
                 "participants": [
                    {"companion_id": "a", "name": "Me", "is_me": true, "score": null, "winner": false},
                    {"companion_id": "b", "name": "Ben", "is_me": false, "score": null, "winner": false}]},
                {"session_id": "s2", "play": {"duration_minutes": 95, "coop": null},
                 "participants": [
                    {"companion_id": "a", "name": "Me", "is_me": true, "score": 31.0, "winner": false},
                    {"companion_id": "b", "name": "Ben", "is_me": false, "score": 42.5, "winner": true}]},
                {"session_id": "s1", "play": {"duration_minutes": null, "coop": null},
                 "participants": [
                    {"companion_id": "a", "name": "Me", "is_me": true, "score": null, "winner": false}]}
            ]
        }))
        .unwrap()
    }

    /// OT-11: a row per play with the date, who played, the scores, the result and the
    /// duration; newest first; a play the page did not get (older than the newest 50) and
    /// events that are no plays are left out.
    #[test]
    fn the_plays_table_has_a_row_per_play_newest_first() {
        let en = I18n::new("en", BTreeMap::new());
        let rows = play_rows(&en, &page());
        assert_eq!(
            rows.iter()
                .map(|r| r.session_id.as_str())
                .collect::<Vec<_>>(),
            ["s4", "s2", "s1"]
        );
        assert_eq!(
            rows[0],
            PlayRow {
                session_id: "s4".into(),
                date: "Mar 4, 2026".into(),
                players: "You, Ben".into(),
                scores: NOTHING.into(),
                result: "Played together".into(),
                duration: "2 h".into(),
            }
        );
        assert_eq!(rows[1].scores, "You 31, Ben 42.5");
        assert_eq!(rows[1].result, "Ben won");
        assert_eq!(rows[1].duration, "1 h 35 min");
        assert_eq!(
            (
                rows[2].players.as_str(),
                rows[2].result.as_str(),
                rows[2].duration.as_str()
            ),
            ("You", NOTHING, NOTHING)
        );
        let de = I18n::new("de", BTreeMap::new());
        let rows = play_rows(&de, &page());
        assert_eq!(rows[0].date, "04.03.2026");
        assert_eq!(rows[0].result, "Gemeinsam gespielt");
        assert_eq!(rows[1].scores, "Du 31, Ben 42,5");
        assert_eq!(rows[1].duration, "1 Std. 35 Min.");
    }

    /// The strip: plays, time played and the win rate, each saying so when there is nothing
    /// to count (OT-20: never "0 hours" beside plays).
    #[test]
    fn the_strip_counts_plays_time_and_the_win_rate() {
        let en = I18n::new("en", BTreeMap::new());
        let t = PlayTotalsDto {
            plays: 14,
            minutes: 1_860,
            decided: 3,
            wins: 2,
        };
        assert_eq!(
            strip(&en, &t, t.plays),
            [
                ("Plays".to_string(), "14".to_string()),
                ("Time played".to_string(), "31 hours".to_string()),
                ("Win rate".to_string(), "67%".to_string()),
            ]
        );
        assert_eq!(
            win_rate(&en, &t),
            Some((
                "67%".into(),
                "2 of 3 plays with a result in your own log".into()
            ))
        );
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(
            win_rate(&de, &t),
            Some((
                "67\u{a0}%".into(),
                "2 von 3 Partien mit Ergebnis in deinem Protokoll".into()
            ))
        );
        let open = PlayTotalsDto {
            plays: 2,
            minutes: 0,
            decided: 0,
            wins: 0,
        };
        assert_eq!(win_rate(&en, &open), None);
        assert_eq!(
            strip(&en, &open, open.plays),
            [
                ("Plays".to_string(), "2".to_string()),
                ("Time played".to_string(), "not logged".to_string()),
                ("Win rate".to_string(), "no results yet".to_string()),
            ]
        );
        let one = PlayTotalsDto {
            plays: 1,
            minutes: 45,
            decided: 1,
            wins: 1,
        };
        assert_eq!(strip(&en, &one, one.plays)[1].1, "45 minutes");
        assert_eq!(
            win_rate(&en, &one),
            Some((
                "100%".into(),
                "1 of 1 play with a result in your own log".into()
            ))
        );
    }

    /// Review OT2F-2: the tab counts a play as the header and the History tab do, from its
    /// event; plays without details are counted and named, and are in no time and no rate.
    #[test]
    fn plays_without_details_are_counted_and_named() {
        let en = I18n::new("en", BTreeMap::new());
        let none = PlayTotalsDto::default();
        let count = play_count(3, &none);
        assert_eq!(count, PlayCount { plays: 3, bare: 3 });
        assert_eq!(strip(&en, &none, count.plays)[0].1, "3");
        assert_eq!(
            bare_note(&en, count).as_deref(),
            Some(
                "3 plays have no details (players, result, duration). They count as plays, \
                 not in the time or the win rate."
            )
        );
        let one = PlayTotalsDto {
            plays: 1,
            minutes: 60,
            decided: 1,
            wins: 1,
        };
        let count = play_count(2, &one);
        assert_eq!(count, PlayCount { plays: 2, bare: 1 });
        assert_eq!(
            bare_note(&en, count).as_deref(),
            Some(
                "One play has no details (players, result, duration). It counts as a play, \
                 not in the time or the win rate."
            )
        );
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(
            bare_note(&de, count).as_deref(),
            Some(
                "Eine Partie hat keine Details (Mitspielende, Ergebnis, Dauer). Sie z\u{e4}hlt \
                 als Partie, nicht in der Spielzeit oder der Gewinnquote."
            )
        );
        // every play has its details: nothing to say, and never fewer than were read
        let all = play_count(1, &one);
        assert_eq!(all, PlayCount { plays: 1, bare: 0 });
        assert_eq!(bare_note(&en, all), None);
        assert_eq!(play_count(0, &one), PlayCount { plays: 1, bare: 0 });
    }
}
