//! Hand-built inline SVG charts with text equivalents (design.md 6.1, 8; DEC-229): a bar
//! list for all-time totals (Home) and a column chart for activity over time (Stats). Each
//! chart is one series in one ink colour, labelled directly, so nothing relies on colour
//! (WCAG 1.4.1); the SVG is an image with a summary as its name, and a list or table carries
//! the numbers. The maths is pure and tested here; the components only draw it.

use contracts::{AmountsDto, SeriesPointDto};

use leptos::prelude::*;

use crate::i18n::I18n;

/// What a chart counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    Events,
    Completions,
    Finished,
    Episodes,
    Pages,
    Plays,
    Minutes,
}

impl Measure {
    #[must_use]
    pub fn value(self, a: &AmountsDto) -> u64 {
        match self {
            Self::Events => a.events,
            Self::Completions => a.completions,
            Self::Finished => a.finished,
            Self::Episodes => a.episodes,
            Self::Pages => a.pages,
            Self::Plays => a.plays,
            Self::Minutes => a.minutes,
        }
    }

    /// Fluent key of the measure's name (a chart title, a table column).
    #[must_use]
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Events => "stats-measure-events",
            Self::Completions => "stats-measure-completions",
            Self::Finished => "stats-measure-finished",
            Self::Episodes => "stats-measure-episodes",
            Self::Pages => "stats-measure-pages",
            Self::Plays => "stats-measure-plays",
            Self::Minutes => "stats-measure-minutes",
        }
    }

    /// Fluent key of "{ $count } <measure>" (plural rules), for summaries and bar labels.
    #[must_use]
    pub fn count_key(self) -> &'static str {
        match self {
            Self::Events => "stats-count-events",
            Self::Completions => "stats-count-completions",
            Self::Finished => "stats-count-finished",
            Self::Episodes => "stats-count-episodes",
            Self::Pages => "stats-count-pages",
            Self::Plays => "stats-count-plays",
            Self::Minutes => "stats-count-minutes",
        }
    }
}

/// The measures worth a chart for a group (`all` or a group id), most telling first.
#[must_use]
pub fn measures_for(group: &str) -> Vec<Measure> {
    use Measure::{Completions, Episodes, Events, Finished, Minutes, Pages, Plays};
    match group {
        "watch" => vec![Finished, Episodes, Minutes],
        "read" => vec![Pages, Completions],
        "listen" => vec![Minutes, Completions],
        "play" => vec![Plays, Minutes],
        _ => vec![Events, Completions],
    }
}

/// Axis ticks from 0 to a "nice" top: the smallest step of 1, 2 or 5 times a power of ten
/// that reaches `max` in at most `target` steps. An empty chart still gets the axis 0..1.
#[must_use]
pub fn nice_ticks(max: u64, target: usize) -> Vec<u64> {
    let target = u64::try_from(target.max(1)).unwrap_or(1);
    let max = max.max(1);
    let mut magnitude = 1u64;
    let step = 'found: loop {
        for f in [1, 2, 5] {
            let s = f * magnitude;
            if max.div_ceil(s) <= target {
                break 'found s;
            }
        }
        magnitude = magnitude.saturating_mul(10);
    };
    let steps = max.div_ceil(step);
    (0..=steps).map(|i| i * step).collect()
}

/// `value` as a percentage (0..=100) of `top`; 0 for an empty scale.
#[must_use]
#[allow(clippy::cast_precision_loss)] // counts stay far below 2^52
pub fn pct(value: u64, top: u64) -> f64 {
    if top == 0 {
        return 0.0;
    }
    (value as f64 / top as f64 * 100.0).clamp(0.0, 100.0)
}

/// Show every `n`-th axis label so that at most `max_labels` are drawn.
#[must_use]
pub fn label_stride(n: usize, max_labels: usize) -> usize {
    n.div_ceil(max_labels.max(1)).max(1)
}

fn month_index(bucket: &str) -> Option<i64> {
    let (y, m) = bucket.split_once('-')?;
    let (y, m): (i64, i64) = (y.parse().ok()?, m.parse().ok()?);
    (1..=12).contains(&m).then_some(y * 12 + m - 1)
}

fn filled(
    points: &[SeriesPointDto],
    index: impl Fn(&str) -> Option<i64>,
    name: impl Fn(i64) -> String,
) -> Vec<SeriesPointDto> {
    let known: Vec<(i64, &SeriesPointDto)> = points
        .iter()
        .filter_map(|p| Some((index(&p.bucket)?, p)))
        .collect();
    let (Some(first), Some(last)) = (
        known.iter().map(|(i, _)| *i).min(),
        known.iter().map(|(i, _)| *i).max(),
    ) else {
        return Vec::new();
    };
    (first..=last)
        .map(|i| {
            known.iter().find(|(k, _)| *k == i).map_or_else(
                || SeriesPointDto {
                    bucket: name(i),
                    amounts: AmountsDto::default(),
                },
                |(_, p)| (*p).clone(),
            )
        })
        .collect()
}

/// Months from the first to the last point with the empty ones filled in (the server leaves
/// buckets without activity out), at most the last `keep`.
#[must_use]
pub fn fill_months(points: &[SeriesPointDto], keep: usize) -> Vec<SeriesPointDto> {
    let mut all = filled(points, month_index, |i| {
        format!("{:04}-{:02}", i.div_euclid(12), i.rem_euclid(12) + 1)
    });
    let skip = all.len().saturating_sub(keep);
    all.drain(..skip);
    all
}

/// Years from the first to the last point, empty ones filled in.
#[must_use]
pub fn fill_years(points: &[SeriesPointDto]) -> Vec<SeriesPointDto> {
    filled(points, |b| b.parse().ok(), |i| format!("{i:04}"))
}

/// An axis or table label for a bucket: `2026-03` -> "Mar 2026" in the locale, a year as is.
#[must_use]
pub fn bucket_label(i: &I18n, bucket: &str) -> String {
    match month_index(bucket) {
        Some(idx) => {
            let key = format!("month-short-{}", idx.rem_euclid(12) + 1);
            i.t_args(
                "stats-bucket-month",
                &[
                    ("month", i.t(&key).into()),
                    ("year", idx.div_euclid(12).to_string().into()),
                ],
            )
        }
        None => bucket.to_string(),
    }
}

/// One bar of a [`BarList`].
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub label: String,
    pub value: u64,
    /// The value as shown (locale digits).
    pub text: String,
}

/// One column of a [`ColumnChart`].
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Axis label (shown for every `label_stride`-th column).
    pub label: String,
    pub value: u64,
    /// Tooltip: "Mar 2026: 12 episodes".
    pub title: String,
}

const ROW: u32 = 34;

/// Horizontal bars, one per row, label left and value right above each bar; all bars share
/// the scale of the largest. `summary` is the image's accessible name (the text equivalent
/// in one sentence); the caller shows the numbers as a list too.
#[component]
pub fn BarList(#[prop(into)] summary: String, bars: Vec<Bar>) -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let top = bars.iter().map(|b| b.value).max().unwrap_or(0);
    let height = ROW * u32::try_from(bars.len()).unwrap_or(0);
    view! {
        <svg role="img" aria-label=summary width="100%" height=height class="block overflow-visible text-foreground">
            {bars.into_iter().enumerate().map(|(i, b)| {
                let y = ROW * u32::try_from(i).unwrap_or(0);
                let width = format!("{:.2}%", pct(b.value, top));
                view! {
                    <g>
                        <title>{i18n.t_args("chart-value", &[("label", b.label.clone().into()), ("value", b.text.clone().into())])}</title>
                        <text x="0" y=y + 13 class="fill-current text-[0.8125rem]">{b.label}</text>
                        <text x="100%" y=y + 13 text-anchor="end" class="fill-current text-[0.8125rem] font-semibold tabular-nums">{b.text}</text>
                        <rect x="0" y=y + 19 width="100%" height="8" rx="2" class="fill-muted"/>
                        {(b.value > 0).then(|| view! { <rect x="0" y=y + 19 width=width height="8" rx="2" class="fill-primary"/> })}
                    </g>
                }
            }).collect_view()}
        </svg>
    }
}

const PLOT_TOP: f64 = 12.0;
const PLOT_BOTTOM: f64 = 150.0;
const AXIS_LABELS: usize = 8;

/// Vertical columns over time with a value axis (gridlines and tick labels in a gutter of
/// their own) and every n-th time label; each column has a tooltip and a hit area as wide
/// as its slot. `summary` names the image; the caller renders the table equivalent.
#[component]
pub fn ColumnChart(#[prop(into)] summary: String, columns: Vec<Column>) -> impl IntoView {
    let i18n = crate::i18n::use_i18n();
    let max = columns.iter().map(|c| c.value).max().unwrap_or(0);
    let ticks = nice_ticks(max, 4);
    let top = ticks.last().copied().unwrap_or(1);
    let span = PLOT_BOTTOM - PLOT_TOP;
    let y_of = move |v: u64| PLOT_BOTTOM - pct(v, top) / 100.0 * span;
    #[allow(clippy::cast_precision_loss)] // a handful of columns
    let slot = 100.0 / columns.len().max(1) as f64;
    let stride = label_stride(columns.len(), AXIS_LABELS);
    #[allow(clippy::cast_precision_loss)]
    let tick_labels = ticks
        .iter()
        .map(|t| (y_of(*t), i18n.number(*t as f64)))
        .collect::<Vec<_>>();
    let grid = ticks.iter().map(|t| y_of(*t)).collect::<Vec<_>>();
    view! {
        <div class="grid grid-cols-[2.75rem_1fr] text-foreground">
            <svg aria-hidden="true" width="100%" height="172" class="block overflow-visible">
                {tick_labels.into_iter().map(|(y, text)| view! {
                    <text x="100%" dx="-6" y=y + 4.0 text-anchor="end" class="fill-muted-foreground text-caption tabular-nums">{text}</text>
                }).collect_view()}
            </svg>
            <svg role="img" aria-label=summary width="100%" height="172" class="block overflow-visible">
                {grid.into_iter().map(|y| view! {
                    <line x1="0" x2="100%" y1=y y2=y class="stroke-border" stroke-width="1"/>
                }).collect_view()}
                {columns.into_iter().enumerate().map(|(i, c)| {
                    #[allow(clippy::cast_precision_loss)]
                    let left = i as f64 * slot;
                    let y = y_of(c.value);
                    let label = (i % stride == 0).then(|| view! {
                        <text x=format!("{:.3}%", left + slot / 2.0) y="168" text-anchor="middle" class="fill-muted-foreground text-caption">{c.label.clone()}</text>
                    });
                    view! {
                        <g>
                            <title>{c.title}</title>
                            <rect x=format!("{left:.3}%") y="0" width=format!("{slot:.3}%") height=PLOT_BOTTOM class="fill-transparent"/>
                            {(c.value > 0).then(|| view! {
                                <rect x=format!("{:.3}%", left + slot * 0.15) y=y width=format!("{:.3}%", slot * 0.7)
                                    height=PLOT_BOTTOM - y rx="2" class="fill-primary"/>
                            })}
                            {label}
                        </g>
                    }
                }).collect_view()}
                <line x1="0" x2="100%" y1=PLOT_BOTTOM y2=PLOT_BOTTOM class="stroke-muted-foreground" stroke-width="1"/>
            </svg>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(bucket: &str, events: u64) -> SeriesPointDto {
        SeriesPointDto {
            bucket: bucket.into(),
            amounts: AmountsDto {
                events,
                ..AmountsDto::default()
            },
        }
    }

    #[test]
    fn ticks_end_on_a_nice_number_at_or_above_the_maximum() {
        assert_eq!(nice_ticks(0, 4), [0, 1]);
        assert_eq!(nice_ticks(1, 4), [0, 1]);
        assert_eq!(nice_ticks(3, 4), [0, 1, 2, 3]);
        assert_eq!(nice_ticks(7, 4), [0, 2, 4, 6, 8]);
        assert_eq!(nice_ticks(10, 4), [0, 5, 10]);
        assert_eq!(nice_ticks(38, 4), [0, 10, 20, 30, 40]);
        assert_eq!(nice_ticks(214, 4), [0, 100, 200, 300]);
        assert_eq!(nice_ticks(3480, 4), [0, 1000, 2000, 3000, 4000]);
        for max in [5, 13, 99, 101, 999, 12_345] {
            let t = nice_ticks(max, 4);
            assert!(*t.last().unwrap() >= max, "{max}: {t:?}");
            assert!(t.len() <= 7, "{max}: {t:?}");
        }
    }

    #[test]
    fn bars_scale_to_the_top_tick() {
        assert!((pct(20, 40) - 50.0).abs() < f64::EPSILON);
        assert!((pct(40, 40) - 100.0).abs() < f64::EPSILON);
        assert!((pct(50, 40) - 100.0).abs() < f64::EPSILON, "clamped");
        assert!(pct(3, 0).abs() < f64::EPSILON);
    }

    #[test]
    fn crowded_axes_skip_labels() {
        assert_eq!(label_stride(6, 8), 1);
        assert_eq!(label_stride(8, 8), 1);
        assert_eq!(label_stride(12, 8), 2);
        assert_eq!(label_stride(24, 8), 3);
        assert_eq!(label_stride(0, 8), 1);
    }

    #[test]
    fn missing_months_and_years_are_filled_in() {
        let p = [point("2025-11", 2), point("2026-02", 5)];
        let months: Vec<(String, u64)> = fill_months(&p, 24)
            .into_iter()
            .map(|p| (p.bucket, p.amounts.events))
            .collect();
        assert_eq!(
            months,
            [
                ("2025-11".to_string(), 2),
                ("2025-12".to_string(), 0),
                ("2026-01".to_string(), 0),
                ("2026-02".to_string(), 5)
            ]
        );
        let last2: Vec<String> = fill_months(&p, 2).into_iter().map(|p| p.bucket).collect();
        assert_eq!(last2, ["2026-01", "2026-02"]);
        assert_eq!(fill_months(&[], 24).len(), 0);
        let years: Vec<String> = fill_years(&[point("2023", 1), point("2026", 1)])
            .into_iter()
            .map(|p| p.bucket)
            .collect();
        assert_eq!(years, ["2023", "2024", "2025", "2026"]);
    }

    #[test]
    fn measures_read_their_amount_and_fit_the_group() {
        let a = AmountsDto {
            events: 1,
            completions: 2,
            finished: 3,
            episodes: 4,
            pages: 5,
            plays: 6,
            minutes: 7,
        };
        assert_eq!(Measure::Episodes.value(&a), 4);
        assert_eq!(Measure::Minutes.value(&a), 7);
        assert_eq!(measures_for("watch")[0], Measure::Finished);
        assert!(measures_for("watch").contains(&Measure::Episodes));
        assert!(measures_for("read").contains(&Measure::Pages));
        assert_eq!(measures_for("play")[0], Measure::Plays);
        assert_eq!(measures_for("all")[0], Measure::Events);
        assert_eq!(measures_for("unknown"), measures_for("all"));
        assert_eq!(Measure::Pages.label_key(), "stats-measure-pages");
        assert_eq!(Measure::Pages.count_key(), "stats-count-pages");
    }

    #[test]
    fn buckets_read_in_the_locale() {
        use std::collections::BTreeMap;
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        assert_eq!(bucket_label(&en, "2026-03"), "Mar 2026");
        assert_eq!(bucket_label(&de, "2026-03"), "M\u{e4}rz 2026");
        assert_eq!(bucket_label(&de, "2026"), "2026");
        assert_eq!(bucket_label(&en, "odd"), "odd");
    }
}
