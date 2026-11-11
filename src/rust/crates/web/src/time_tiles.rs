//! The time-led stats block (owner test OT-17, DEC-542): a hero tile with the total hours
//! and four tiles (movies, shows, books, games) with time or pages first and the count
//! second, from the all-time totals per kind. Home shows it as its stats block, the Stats
//! page at its top. Everything is text (the icon is decoration); no pace, rate or target
//! wording (R-04). The words are built here as plain data and tested; the component only
//! lays them out.

use contracts::{AmountsDto, StatsTotalsDto};
use leptos::prelude::*;
use ui::{Icon, IconName, Tint, cn};

use crate::charts::{Measure, hours_of};
use crate::i18n::{I18n, use_i18n};

/// The kinds the four tiles count; every other kind with activity is named in the
/// "Also" line.
const TILE_KINDS: [&str; 5] = ["movie", "show", "book", "video_game", "board_game"];

/// One of the four tiles: time or pages first, the count second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    /// `movies`, `shows`, `books` or `games` (test id, never shown).
    pub id: &'static str,
    pub label: String,
    /// "312 hours", "18,420 pages".
    pub big: String,
    /// "164 movies watched".
    pub small: String,
    /// The kind's icon and its group's tint (OT-13); the label names the kind, so the
    /// colour never stands alone.
    pub icon: IconName,
    pub tint: Tint,
}

/// The words of the block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banked {
    /// The hero figure: "1,284 hours".
    pub total: String,
    /// The sentence under it.
    pub sentence: String,
    /// Movies, shows, books, games; always these four, in this order.
    pub tiles: Vec<Tile>,
    /// "Also: Audiobook 41 hours, Manga 3,100 pages, Anime 500 episodes"; `None` without
    /// such a kind.
    pub also: Option<String>,
    /// Footnotes: reading is counted in pages (when there are pages), titles without a
    /// known length count as 0.
    pub notes: Vec<String>,
}

/// The amounts of the given kinds together.
fn sum(stats: &StatsTotalsDto, kinds: &[&str]) -> AmountsDto {
    let mut out = AmountsDto::default();
    for k in stats
        .kinds
        .iter()
        .filter(|k| kinds.contains(&k.kind.as_str()))
    {
        out.events += k.amounts.events;
        out.completions += k.amounts.completions;
        out.finished += k.amounts.finished;
        out.episodes += k.amounts.episodes;
        out.pages += k.amounts.pages;
        out.plays += k.amounts.plays;
        out.minutes += k.amounts.minutes;
    }
    out
}

/// The sentence under the hero figure: days and hours from one day on, from the hours the
/// figure shows (review OT2C-1: `hours_of` rounds to the nearest hour, and so does this;
/// full days read without "and 0 hours").
fn hero_sentence(i: &I18n, minutes: u64) -> String {
    let shown = hours_of(minutes);
    if minutes == 0 {
        return i.t("stats-hero-none");
    }
    if shown < 24.0 {
        return i.t("stats-hero-under-day");
    }
    // whole from ten hours on, and far below 2^52
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let hours = shown as u64;
    let (days, rest) = (hours / 24, hours % 24);
    if rest == 0 {
        i.t_args("stats-hero-days-only", &[("days", days.into())])
    } else {
        i.t_args(
            "stats-hero-days",
            &[("days", days.into()), ("hours", rest.into())],
        )
    }
}

/// What the "Also" line says of a kind: its time and its pages, or, for a kind logged
/// without either, its first count that has anything (review OT2C-2: episodes, plays, times
/// watched, completed entries, logged steps). The words are the lower-case forms with the
/// number, since they stand inside a sentence (review OT2C-6).
fn also_amounts(i: &I18n, a: &AmountsDto) -> Vec<String> {
    let counted = |key: &str, n: u64| i.t_args(key, &[("count", n.into())]);
    let mut out = Vec::new();
    if a.minutes > 0 {
        out.push(Measure::Hours.counted(i, a.minutes));
    }
    if a.pages > 0 {
        out.push(counted("stats-also-pages", a.pages));
    }
    if out.is_empty() {
        out.extend(
            [
                ("stats-also-episodes", a.episodes),
                ("stats-also-plays", a.plays),
                ("stats-also-finished", a.finished),
                ("stats-also-completions", a.completions),
                ("stats-also-events", a.events),
            ]
            .into_iter()
            .find(|(_, n)| *n > 0)
            .map(|(key, n)| counted(key, n)),
        );
    }
    out
}

/// "Also: ..." for the kinds outside the four tiles that have any activity.
fn also_line(i: &I18n, stats: &StatsTotalsDto) -> Option<String> {
    let mut parts = Vec::new();
    for k in stats
        .kinds
        .iter()
        .filter(|k| !TILE_KINDS.contains(&k.kind.as_str()))
    {
        let key = format!("kind-{}", k.kind);
        let name = i.t(&key);
        let name = if name == key { k.kind.clone() } else { name };
        for amount in also_amounts(i, &k.amounts) {
            parts.push(i.t_args(
                "stats-also-part",
                &[("kind", name.clone().into()), ("amount", amount.into())],
            ));
        }
    }
    (!parts.is_empty()).then(|| i.t_args("stats-also", &[("parts", parts.join(", ").into())]))
}

/// The words of the block for these totals, in the locale of `i`.
#[must_use]
pub fn banked(i: &I18n, stats: &StatsTotalsDto) -> Banked {
    let movies = sum(stats, &["movie"]);
    let shows = sum(stats, &["show"]);
    let books = sum(stats, &["book"]);
    let games = sum(stats, &["video_game", "board_game"]);
    let tile = |id, label: &str, big, small| Tile {
        id,
        label: i.t(label),
        big,
        small,
        icon: match id {
            "movies" => IconName::KindMovie,
            "shows" => IconName::KindShow,
            "books" => IconName::KindBook,
            _ => IconName::KindVideoGame,
        },
        tint: match id {
            "movies" | "shows" => Tint::Watch,
            "books" => Tint::Read,
            _ => Tint::Play,
        },
    };
    // OT-20: a tile leads with time (books: pages). With counts but no known time it
    // leads with the count and says what is missing, so "0 hours" never stands beside
    // "13 episodes"; an empty tile keeps its zero.
    let led = |id, label: &str, lead: Measure, amount: u64, count: u64, counted: String| {
        if amount == 0 && count > 0 {
            let missing = match id {
                "movies" => "stats-tile-untimed-movies",
                "shows" => "stats-tile-untimed-shows",
                "books" => "stats-tile-untimed-books",
                _ => "stats-tile-untimed-games",
            };
            tile(
                id,
                label,
                counted,
                i.t_args(missing, &[("count", count.into())]),
            )
        } else {
            tile(id, label, lead.counted(i, amount), counted)
        }
    };
    let tiles = vec![
        led(
            "movies",
            "stats-tile-movies",
            Measure::Hours,
            movies.minutes,
            movies.finished,
            i.t_args(
                "stats-tile-movies-count",
                &[("count", movies.finished.into())],
            ),
        ),
        led(
            "shows",
            "stats-tile-shows",
            Measure::Hours,
            shows.minutes,
            shows.episodes,
            Measure::Episodes.counted(i, shows.episodes),
        ),
        led(
            "books",
            "stats-tile-books",
            Measure::Pages,
            books.pages,
            books.completions,
            i.t_args(
                "stats-tile-books-count",
                &[("count", books.completions.into())],
            ),
        ),
        led(
            "games",
            "stats-tile-games",
            Measure::Hours,
            games.minutes,
            games.plays,
            Measure::Plays.counted(i, games.plays),
        ),
    ];

    let total = stats.overall.amounts.minutes;
    let mut notes = Vec::new();
    if stats.overall.amounts.pages > 0 {
        notes.push(i.t("stats-reading-note"));
    }
    notes.push(i.t("stats-untimed-note"));
    Banked {
        total: Measure::Hours.counted(i, total),
        sentence: hero_sentence(i, total),
        tiles,
        also: also_line(i, stats),
        notes,
    }
}

const LABEL: &str = "text-sm font-medium text-muted-foreground";

/// The grid of the hero and of the tiles: the same tracks, so their edges line up.
const GRID: &str = "grid grid-cols-2 gap-3 @2xl:grid-cols-4";

/// The block: the hero tile (a section named by its label), the four tiles as a
/// description list, then the "Also" line and the footnotes. The hero and the tiles sit on
/// one grid that counts its columns by the width of the block itself (a container query,
/// OT-19): four from 42 rem of space, else two, the hero over two of them, so its edges
/// are the tiles' edges and no tile depends on the width of the window. `level` is the heading level
/// of the hero label (3 under Home's "All-time stats", 2 at the top of the Stats page).
#[component]
pub fn TimeBanked(stats: StatsTotalsDto, #[prop(default = 3)] level: u8) -> impl IntoView {
    let i18n = use_i18n();
    let b = banked(&i18n, &stats);
    let label = i18n.t("stats-hero-label");
    let heading = if level == 2 {
        view! { <h2 id="st-hero-title" class=LABEL>{label}</h2> }.into_any()
    } else {
        view! { <h3 id="st-hero-title" class=LABEL>{label}</h3> }.into_any()
    };
    view! {
        <div class="@container flex flex-col gap-3" data-testid="banked">
            <div class=GRID>
            <section aria-labelledby="st-hero-title" data-testid="banked-hero"
                class="col-span-2 flex min-w-0 items-center gap-4 rounded-xl border border-border bg-primary-soft p-4">
                <span class="flex size-16 shrink-0 items-center justify-center rounded-full bg-background text-primary">
                    <Icon name=IconName::PiggyBank class="size-10"/>
                </span>
                <div class="min-w-0">
                    {heading}
                    <p class="text-display tabular-nums" data-testid="banked-total">{b.total}</p>
                    <p class="text-sm text-muted-foreground">{b.sentence}</p>
                </div>
            </section>
            </div>
            <dl class=GRID>
                {b.tiles.into_iter().map(|t| view! {
                    <div class=cn(&["min-w-0 rounded-xl border border-border p-4", t.tint.soft()]) data-testid=format!("banked-tile-{}", t.id)>
                        <dt class="flex items-center gap-1.5 text-sm font-medium text-muted-foreground">
                            <Icon name=t.icon class=t.tint.text()/>
                            {t.label}
                        </dt>
                        <dd class="text-xl font-semibold tabular-nums">{t.big}</dd>
                        <dd class="text-sm text-muted-foreground tabular-nums">{t.small}</dd>
                    </div>
                }).collect_view()}
            </dl>
            {b.also.map(|a| view! { <p class="text-sm" data-testid="banked-also">{a}</p> })}
            <p class="text-xs text-muted-foreground" data-testid="banked-notes">{b.notes.join(" ")}</p>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use contracts::{AmountsDto, KindTotalsDto, StatsTotalsDto, TotalsDto};

    use super::*;

    fn kind(kind: &str, amounts: AmountsDto) -> KindTotalsDto {
        KindTotalsDto {
            kind: kind.into(),
            amounts,
        }
    }

    fn stats(kinds: Vec<KindTotalsDto>) -> StatsTotalsDto {
        let mut amounts = AmountsDto::default();
        for k in &kinds {
            amounts.minutes += k.amounts.minutes;
            amounts.pages += k.amounts.pages;
        }
        StatsTotalsDto {
            overall: TotalsDto {
                group: "all".into(),
                entries: 0,
                planned: 0,
                in_progress: 0,
                completed: 0,
                paused: 0,
                dropped: 0,
                amounts,
            },
            groups: Vec::new(),
            kinds,
        }
    }

    fn full() -> StatsTotalsDto {
        let a = AmountsDto::default;
        stats(vec![
            kind(
                "audiobook",
                AmountsDto {
                    minutes: 2_460,
                    ..a()
                },
            ),
            kind(
                "board_game",
                AmountsDto {
                    plays: 80,
                    minutes: 12_000,
                    ..a()
                },
            ),
            kind(
                "book",
                AmountsDto {
                    pages: 18_420,
                    completions: 61,
                    ..a()
                },
            ),
            kind(
                "manga",
                AmountsDto {
                    pages: 3_100,
                    ..a()
                },
            ),
            kind(
                "movie",
                AmountsDto {
                    finished: 164,
                    minutes: 18_720,
                    ..a()
                },
            ),
            // logged without time or pages: named with its count (review OT2C-2)
            kind("music", AmountsDto { events: 3, ..a() }),
            kind(
                "show",
                AmountsDto {
                    episodes: 1_930,
                    minutes: 44_520,
                    ..a()
                },
            ),
            kind(
                "video_game",
                AmountsDto {
                    plays: 8,
                    minutes: 1_800,
                    ..a()
                },
            ),
        ])
    }

    fn column(b: &Banked, f: impl Fn(&Tile) -> &str) -> Vec<String> {
        b.tiles.iter().map(|t| f(t).to_string()).collect()
    }

    #[test]
    fn the_hero_and_the_four_tiles_lead_with_time() {
        let en = I18n::new("en", BTreeMap::new());
        let b = banked(&en, &full());
        assert_eq!(b.total, "1,325 hours");
        assert_eq!(
            b.sentence,
            "That is 55 days and 5 hours of movies, shows, listening and play."
        );
        assert_eq!(
            b.tiles.iter().map(|t| t.id).collect::<Vec<_>>(),
            ["movies", "shows", "books", "games"]
        );
        assert_eq!(
            column(&b, |t| &t.label),
            ["Movies", "Shows", "Books", "Games"]
        );
        assert_eq!(
            column(&b, |t| &t.big),
            ["312 hours", "742 hours", "18,420 pages", "230 hours"]
        );
        assert_eq!(
            column(&b, |t| &t.small),
            [
                "164 movies watched",
                "1,930 episodes",
                "61 books finished",
                "88 plays"
            ],
            "the games tile adds video and board games"
        );
        assert_eq!(
            b.also.as_deref(),
            Some("Also: Audiobook 41 hours, Manga 3,100 pages, Music 3 logged steps")
        );
        assert_eq!(
            b.notes,
            [
                "Reading is counted in pages.",
                "Titles without a known length count as 0."
            ]
        );
    }

    /// OT-20: a tile with counts but no known time (or pages) leads with the count and
    /// says what is missing; "0 hours" beside "13 episodes" read as wrong.
    #[test]
    fn a_tile_with_counts_but_no_known_time_says_so() {
        let en = I18n::new("en", BTreeMap::new());
        let a = AmountsDto::default;
        let s = stats(vec![
            kind("board_game", AmountsDto { plays: 2, ..a() }),
            kind(
                "book",
                AmountsDto {
                    completions: 3,
                    ..a()
                },
            ),
            kind("movie", AmountsDto { finished: 1, ..a() }),
            kind(
                "show",
                AmountsDto {
                    episodes: 13,
                    ..a()
                },
            ),
        ]);
        let b = banked(&en, &s);
        assert_eq!(
            column(&b, |t| &t.big),
            [
                "One movie watched",
                "13 episodes",
                "3 books finished",
                "2 plays"
            ]
        );
        assert_eq!(
            column(&b, |t| &t.small),
            [
                "length not known yet",
                "runtimes not known yet",
                "no pages logged yet",
                "no play time logged yet"
            ]
        );
        let de = I18n::new("de", BTreeMap::new());
        let b = banked(&de, &s);
        assert_eq!(b.tiles[1].big, "13 Folgen");
        assert_eq!(b.tiles[1].small, "Laufzeiten noch nicht bekannt");
        // with any time the tile is as before, and an empty one keeps its zero
        let timed = banked(
            &en,
            &stats(vec![kind(
                "show",
                AmountsDto {
                    episodes: 13,
                    minutes: 45,
                    ..a()
                },
            )]),
        );
        assert_eq!(timed.tiles[1].big, "45 minutes");
        assert_eq!(timed.tiles[1].small, "13 episodes");
        assert_eq!(timed.tiles[0].big, "0 hours");
        assert_eq!(timed.tiles[0].small, "0 movies watched");
    }

    #[test]
    fn every_tile_has_the_icon_and_the_tint_of_its_group() {
        // OT-13: the tint never stands alone, the tile names its kind and shows its icon
        let en = I18n::new("en", BTreeMap::new());
        let b = banked(&en, &full());
        assert_eq!(
            b.tiles.iter().map(|t| t.tint).collect::<Vec<_>>(),
            [Tint::Watch, Tint::Watch, Tint::Read, Tint::Play]
        );
        assert_eq!(
            b.tiles.iter().map(|t| t.icon).collect::<Vec<_>>(),
            [
                IconName::KindMovie,
                IconName::KindShow,
                IconName::KindBook,
                IconName::KindVideoGame
            ]
        );
    }

    #[test]
    fn the_tiles_read_in_german() {
        let de = I18n::new("de", BTreeMap::new());
        let b = banked(&de, &full());
        assert_eq!(b.total, "1.325 Stunden");
        assert!(
            b.sentence.starts_with("Das sind 55 Tage und 5 Stunden "),
            "{}",
            b.sentence
        );
        assert_eq!(
            column(&b, |t| &t.label),
            ["Filme", "Serien", "B\u{fc}cher", "Spiele"]
        );
        assert_eq!(
            column(&b, |t| &t.big),
            ["312 Stunden", "742 Stunden", "18.420 Seiten", "230 Stunden"]
        );
        assert_eq!(b.tiles[0].small, "164 Filme gesehen");
        let also = b.also.unwrap();
        assert!(also.starts_with("Au\u{df}erdem: "), "{also}");
        assert!(also.contains(" 41 Stunden, "), "{also}");
    }

    #[test]
    fn an_empty_or_small_total_has_its_own_sentence_and_one_of_each_is_singular() {
        let en = I18n::new("en", BTreeMap::new());
        let none = banked(&en, &stats(Vec::new()));
        assert_eq!(none.total, "0 hours");
        assert_eq!(none.sentence, "Nothing with a known length logged yet.");
        assert_eq!(
            column(&none, |t| &t.big),
            ["0 hours", "0 hours", "0 pages", "0 hours"]
        );
        assert_eq!(none.also, None);
        assert_eq!(none.notes, ["Titles without a known length count as 0."]);

        let movie = |minutes| {
            stats(vec![kind(
                "movie",
                AmountsDto {
                    finished: 1,
                    minutes,
                    ..AmountsDto::default()
                },
            )])
        };
        let small = banked(&en, &movie(90));
        assert_eq!(small.total, "1.5 hours");
        assert_eq!(
            small.sentence,
            "Movies, shows, listening and play, added up."
        );
        assert_eq!(small.tiles[0].small, "One movie watched");
        // 25 hours: one day and one hour
        let day = banked(&en, &movie(1_500));
        assert_eq!(
            day.sentence,
            "That is 1 day and 1 hour of movies, shows, listening and play."
        );
        let de = I18n::new("de", BTreeMap::new());
        assert!(
            banked(&de, &movie(1_500))
                .sentence
                .starts_with("Das ist 1 Tag und 1 Stunde "),
            "{}",
            banked(&de, &movie(1_500)).sentence
        );
    }

    fn one_movie(minutes: u64) -> StatsTotalsDto {
        stats(vec![kind(
            "movie",
            AmountsDto {
                finished: 1,
                minutes,
                ..AmountsDto::default()
            },
        )])
    }

    #[test]
    fn the_sentence_uses_the_hours_of_the_figure_and_leaves_out_zero_hours() {
        // review OT2C-1: the figure rounds to the nearest hour, so must the sentence
        let en = I18n::new("en", BTreeMap::new());
        let de = I18n::new("de", BTreeMap::new());
        let days = |n: &str| format!("That is {n} of movies, shows, listening and play.");
        // 47 h 50 min shows as 48 hours: two days, no "and 0 hours"
        for minutes in [2_870, 2_880] {
            let b = banked(&en, &one_movie(minutes));
            assert_eq!(b.total, "48 hours", "{minutes}");
            assert_eq!(b.sentence, days("2 days"), "{minutes}");
            assert_eq!(
                banked(&de, &one_movie(minutes)).sentence,
                "Das sind 2 Tage mit Filmen, Serien, H\u{f6}ren und Spielen.",
                "{minutes}"
            );
        }
        // 1,325.5 hours show as 1,326: 55 days and 6 hours
        let b = banked(&en, &one_movie(79_530));
        assert_eq!(b.total, "1,326 hours");
        assert_eq!(b.sentence, days("55 days and 6 hours"));
        // from 23 h 30 min the figure reads 24 hours: one day
        let b = banked(&en, &one_movie(1_410));
        assert_eq!(b.total, "24 hours");
        assert_eq!(b.sentence, days("1 day"));
        assert_eq!(
            banked(&de, &one_movie(1_439)).sentence,
            "Das ist 1 Tag mit Filmen, Serien, H\u{f6}ren und Spielen."
        );
        let b = banked(&en, &one_movie(1_409));
        assert_eq!(b.total, "23 hours");
        assert_eq!(b.sentence, "Movies, shows, listening and play, added up.");
    }

    #[test]
    fn a_kind_without_time_and_pages_is_named_with_its_count() {
        // review OT2C-2: nothing a person logged disappears from the block
        let a = AmountsDto::default;
        let s = stats(vec![
            kind(
                "anime",
                AmountsDto {
                    events: 500,
                    episodes: 500,
                    finished: 3,
                    ..a()
                },
            ),
            // review OT2C-6: one page reads "1 page" inside the sentence
            kind(
                "manga",
                AmountsDto {
                    events: 1,
                    pages: 1,
                    ..a()
                },
            ),
            kind(
                "music",
                AmountsDto {
                    events: 2,
                    completions: 1,
                    ..a()
                },
            ),
            kind("podcast", AmountsDto { events: 1, ..a() }),
            kind(
                "visual_novel",
                AmountsDto {
                    events: 12,
                    plays: 12,
                    ..a()
                },
            ),
        ]);
        let en = I18n::new("en", BTreeMap::new());
        assert_eq!(
            banked(&en, &s).also.as_deref(),
            Some(
                "Also: Anime 500 episodes, Manga 1 page, Music 1 completed, \
                 Podcast 1 logged step, Visual novel 12 plays"
            )
        );
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(
            banked(&de, &s).also.as_deref(),
            Some(
                "Au\u{df}erdem: Anime 500 Folgen, Manga 1 Seite, Musik 1 abgeschlossen, \
                 Podcast 1 eingetragener Schritt, Visual Novel 12 Partien"
            )
        );
        // a kind that is listed without any amount is not named
        let idle = stats(vec![kind("anime", a())]);
        assert_eq!(banked(&en, &idle).also, None);
    }

    #[test]
    fn time_below_one_hour_reads_in_minutes_never_as_zero_hours() {
        // review OT2C-3
        let en = I18n::new("en", BTreeMap::new());
        let de = I18n::new("de", BTreeMap::new());
        let b = banked(&en, &one_movie(2));
        assert_eq!(b.total, "2 minutes");
        assert_eq!(b.tiles[0].big, "2 minutes");
        assert_eq!(b.sentence, "Movies, shows, listening and play, added up.");
        assert_eq!(banked(&en, &one_movie(1)).total, "1 minute");
        assert_eq!(banked(&en, &one_movie(45)).total, "45 minutes");
        assert_eq!(banked(&en, &one_movie(60)).total, "1 hour");
        assert_eq!(banked(&de, &one_movie(2)).total, "2 Minuten");
        assert_eq!(banked(&de, &one_movie(1)).total, "1 Minute");
        let music = stats(vec![kind(
            "music",
            AmountsDto {
                events: 1,
                minutes: 2,
                ..AmountsDto::default()
            },
        )]);
        assert_eq!(
            banked(&en, &music).also.as_deref(),
            Some("Also: Music 2 minutes")
        );
    }

    #[test]
    fn nothing_in_the_block_talks_about_pace() {
        // R-04: totals only, no rate, target or judgement
        for locale in ["en", "de"] {
            let i = I18n::new(locale, BTreeMap::new());
            let b = banked(&i, &full());
            let mut text = vec![b.total.clone(), b.sentence.clone()];
            text.extend(b.notes.clone());
            text.extend(b.also.clone());
            let text = text.join(" ").to_lowercase();
            for word in [
                "per week",
                "pro woche",
                "behind",
                "ahead",
                "goal",
                "ziel",
                "pace",
                "tempo",
            ] {
                assert!(!text.contains(word), "{locale}: {word} in {text}");
            }
        }
    }
}
