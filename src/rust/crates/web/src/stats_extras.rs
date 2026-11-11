//! WP4-6 on the Stats page and the History side panel (design.md 6.1, 6.6, 9; research R-04):
//! "this year so far" against the same days last year, the people seen most often, and the
//! opt-in yearly goals. Goals are private and live on the Stats page only (never on Home);
//! their progress is "7 of 12 in 2026" as text and a bar, never "behind" or "ahead".

use contracts::{CompareDto, GoalDto, NewGoalDto, OftenWithDto, PlayTotalsDto, StatsCompareDto};
use leptos::prelude::*;
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonSize, ButtonVariant, Dialog, Disclosure,
    DisclosureMode, FormField, Input, Label, ProgressBar, Select, SelectOption, Skeleton,
    disclosure_storage_key,
};

use crate::admin::ConfirmDialog;
use crate::api;
use crate::charts::{Measure, measures_for, measures_led};
use crate::home::{count, group_label};
use crate::i18n::{I18n, t, use_i18n};
use crate::pages::error_text;
use crate::plays_tab::win_rate;
use crate::rest;

/// The goal measures in the order the form offers them.
pub const GOAL_METRICS: [&str; 6] = [
    "finished",
    "episodes",
    "pages",
    "completions",
    "plays",
    "minutes",
];

/// The chart measure of a goal measure (for its name).
#[must_use]
pub fn goal_measure(metric: &str) -> Measure {
    match metric {
        "completions" => Measure::Completions,
        "finished" => Measure::Finished,
        "episodes" => Measure::Episodes,
        "pages" => Measure::Pages,
        "plays" => Measure::Plays,
        _ => Measure::Minutes,
    }
}

/// One line of the comparison: the measure's name, this year so far, the same days last year.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareLine {
    pub label: String,
    /// What the two numbers count; time is shown in hours (`Measure::text`).
    pub measure: Measure,
    pub this: u64,
    pub last: u64,
}

/// The comparison lines of a group (`all` or a group id): its chart measures, for the group
/// itself, or for `all` each group's first measure that has anything in either year (time
/// leads, OT-17; a group without any time shows its first count).
#[must_use]
pub fn compare_lines(i18n: &I18n, c: &[CompareDto], group: &str) -> Vec<CompareLine> {
    let line = |g: &CompareDto, m: Measure, prefix: bool| {
        let name = i18n.t(m.label_key());
        CompareLine {
            label: if prefix {
                format!("{}: {name}", group_label(i18n, &g.group))
            } else {
                name
            },
            measure: m,
            this: m.value(&g.this),
            last: m.value(&g.last),
        }
    };
    if group == "all" {
        return c
            .iter()
            .filter(|g| g.group != "all")
            .filter_map(|g| {
                measures_for(&g.group)
                    .into_iter()
                    .map(|m| line(g, m, true))
                    .find(|l| l.this > 0 || l.last > 0)
            })
            .collect();
    }
    c.iter()
        .find(|g| g.group == group)
        .map(|g| {
            // hours after the counts when neither year has any time (review OT2C-5)
            measures_led(group, g.this.minutes > 0 || g.last.minutes > 0)
                .into_iter()
                .map(|m| line(g, m, false))
                .collect()
        })
        .unwrap_or_default()
}

/// "This year so far" for the Stats tab's group.
#[component]
pub fn YearSoFar(data: StatsCompareDto, #[prop(into)] group: Signal<String>) -> impl IntoView {
    let i18n = use_i18n();
    let c = data.compare;
    let lead = i18n.t_args(
        "stats-compare-lead",
        &[
            ("through", i18n.date(&c.through).into()),
            ("last", i18n.date(&c.last_through).into()),
        ],
    );
    let groups = StoredValue::new(c.groups);
    let (year, last_year) = (c.year, c.year - 1);
    view! {
        <section aria-labelledby="st-compare-h" class="grid gap-2 rounded-xl border border-border p-4" data-testid="st-compare">
            <h2 id="st-compare-h" class="font-semibold">{i18n.t_args("stats-compare-title", &[("year", year.to_string().into())])}</h2>
            <p class="text-sm text-muted-foreground">{lead}</p>
            {move || {
                let i = use_i18n();
                let lines = groups.with_value(|g| compare_lines(&i, g, &group.get()));
                if lines.is_empty() {
                    return view! { <p class="text-sm">{i.t("stats-compare-none")}</p> }.into_any();
                }
                view! { <CompareList lines=lines year=year last_year=last_year/> }.into_any()
            }}
        </section>
    }
}

/// The lines as a description list: the measure, then both numbers in words.
#[component]
pub fn CompareList(lines: Vec<CompareLine>, year: i32, last_year: i32) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <dl class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-1 text-sm">
            {lines.into_iter().map(|l| {
                let value = i18n.t_args("stats-compare-value", &[
                    ("this", l.measure.text(&i18n, l.this).into()),
                    ("last", l.measure.text(&i18n, l.last).into()),
                    ("year", year.to_string().into()),
                    ("last_year", last_year.to_string().into()),
                ]);
                view! {
                    <dt class="text-muted-foreground">{l.label}</dt>
                    <dd class="text-right tabular-nums">{value}</dd>
                }
            }).collect_view()}
        </dl>
    }
}

/// "Most often with": people by sessions together, each linking to their page.
#[component]
pub fn OftenWith(people: Vec<OftenWithDto>, #[prop(optional)] level: Option<u8>) -> impl IntoView {
    let i18n = use_i18n();
    let title = i18n.t("stats-often-with");
    let heading = if level == Some(3) {
        view! { <h3 id="st-often-h" class="text-sm font-semibold">{title}</h3> }.into_any()
    } else {
        view! { <h2 id="st-often-h" class="font-semibold">{title}</h2> }.into_any()
    };
    let body = if people.is_empty() {
        view! { <p class="text-sm text-muted-foreground">{i18n.t("stats-often-none")}</p> }
            .into_any()
    } else {
        view! {
            <ol class="grid gap-1 text-sm">
                {people.into_iter().map(|p| {
                    let n = use_i18n().t_args("stats-often-together", &[("count", p.together.into())]);
                    view! {
                        <li class="flex justify-between gap-3">
                            <a href=format!("/people/{}", p.id) class="truncate underline-offset-2 hover:underline">{p.name}</a>
                            <span class="tabular-nums text-muted-foreground">{n}</span>
                        </li>
                    }
                }).collect_view()}
            </ol>
        }
        .into_any()
    };
    view! {
        <section aria-labelledby="st-often-h" class="grid gap-2" data-testid="st-often">
            {heading}
            {body}
        </section>
    }
}

/// The words of "Your results" (review OT2F-2): the rate, what it is a rate of, the plays
/// that have their details but no result of the person (`open`), and the plays the totals
/// row counts without any details (`bare`). Decided, open and bare plays add up to the
/// plays of the totals row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultWords {
    pub rate: String,
    pub detail: Option<String>,
    pub open: Option<String>,
    pub bare: Option<String>,
}

/// `counted`: the plays the totals row above counts (from the history events).
#[must_use]
pub fn result_words(i: &I18n, totals: &PlayTotalsDto, counted: u32) -> ResultWords {
    let count = crate::plays_tab::play_count(counted, totals);
    let (rate, detail) = match win_rate(i, totals) {
        Some((rate, detail)) => (rate, Some(detail)),
        None => (i.t("plays-rate-none"), None),
    };
    let open = totals.plays.saturating_sub(totals.decided);
    ResultWords {
        rate,
        detail,
        open: (open > 0).then(|| i.t_args("stats-play-open", &[("count", open.into())])),
        bare: crate::plays_tab::bare_note(i, count),
    }
}

/// The person's own results over every game, all time (ot2f, OT-11): the win rate, what it
/// is a rate of, and how many plays it leaves out (no result logged, a co-op play without
/// one, a play they logged without taking part, or a play without details).
#[component]
pub fn PlayResults(totals: PlayTotalsDto, counted: u32) -> impl IntoView {
    let i18n = use_i18n();
    let words = result_words(&i18n, &totals, counted);
    let line = "text-sm text-muted-foreground";
    view! {
        <section aria-labelledby="st-results-h" class="grid gap-2 rounded-xl border border-border p-4" data-testid="st-results">
            <h2 id="st-results-h" class="font-semibold">{i18n.t("stats-play-results")}</h2>
            <dl class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-3 gap-y-1 text-sm">
                <dt class="text-muted-foreground">{i18n.t("plays-rate")}</dt>
                <dd class="text-right font-semibold tabular-nums" data-testid="st-win-rate">{words.rate}</dd>
            </dl>
            {words.detail.map(|d| view! { <p class=line>{d}</p> })}
            {words.open.map(|o| view! { <p class=line>{o}</p> })}
            {words.bare.map(|b| view! { <p class=line data-testid="st-plays-bare">{b}</p> })}
        </section>
    }
}

/// The Stats page's comparison and people, from one load. `plays`: the plays the totals row
/// of the Play tab counts, once it is known; "Your results" counts the same plays.
#[component]
pub fn StatsCompare(
    #[prop(into)] group: Signal<String>,
    #[prop(into)] plays: Signal<Option<u64>>,
) -> impl IntoView {
    let data = Resource::new(|| (), |()| api::stats_compare());
    view! {
        <Suspense fallback=|| view! { <Skeleton class="h-24 w-full"/> }>
            {move || data.get().and_then(Result::ok).map(|d| {
                let people = d.often_with.clone();
                // the Play tab: the win rate after the hours and plays of the totals row (OT-11)
                let play = d.play;
                let counted = move || {
                    let row = plays.get().map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX));
                    row.max(play.plays)
                };
                let results = move || group.get() == "play" && counted() > 0;
                view! {
                    <Show when=results>{move || view! { <PlayResults totals=play counted=counted()/> }}</Show>
                    <div class="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
                        <YearSoFar data=d group=group/>
                        <div class="rounded-xl border border-border p-4"><OftenWith people=people/></div>
                    </div>
                }
            })}
        </Suspense>
    }
}

/// The History side panel's part: the overall comparison and the three people seen most.
#[component]
pub fn SideCompare() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::stats_compare());
    view! {
        <Suspense fallback=|| view! { <Skeleton class="h-16 w-full"/> }>
            {move || data.get().and_then(Result::ok).map(|d| {
                let i = use_i18n();
                let lines = compare_lines(&i, &d.compare.groups, "all");
                let year = d.compare.year;
                let mut people = d.often_with;
                people.truncate(3);
                view! {
                    <section aria-labelledby="hist-compare-h" class="grid gap-2 border-t border-border pt-3" data-testid="history-compare">
                        <h3 id="hist-compare-h" class="text-sm font-semibold">{i.t_args("stats-compare-title", &[("year", year.to_string().into())])}</h3>
                        {if lines.is_empty() {
                            view! { <p class="text-sm">{i.t("stats-compare-none")}</p> }.into_any()
                        } else {
                            view! { <CompareList lines=lines year=year last_year=year - 1/> }.into_any()
                        }}
                    </section>
                    <div class="border-t border-border pt-3"><OftenWith people=people level=3/></div>
                }
            })}
        </Suspense>
    }
}

/// The goal's heading: "Read: Pages" or "Everything: Plays".
fn goal_title(i18n: &I18n, g: &GoalDto) -> String {
    i18n.t_args(
        "goals-title",
        &[
            ("group", group_label(i18n, &g.group).into()),
            (
                "measure",
                i18n.t(goal_measure(&g.metric).label_key()).into(),
            ),
        ],
    )
}

/// The opt-in goals: the list (when there are any) and "Set a goal" behind a disclosure.
#[component]
pub fn Goals() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::goals());
    let refresh = Callback::new(move |()| data.refetch());
    let editing = RwSignal::new(None::<GoalDto>);
    let removing = RwSignal::new(None::<GoalDto>);
    view! {
        <section aria-labelledby="st-goals-h" class="grid gap-3" data-testid="st-goals">
            <h2 id="st-goals-h" class="text-lg font-semibold">{t("goals-heading")}</h2>
            <p class="text-sm text-muted-foreground">{t("goals-lead")}</p>
            <Transition fallback=|| view! { <Skeleton class="h-16 w-full"/> }>
                {move || data.get().and_then(Result::ok).map(|goals| view! {
                    <GoalList goals=goals editing=editing removing=removing/>
                })}
            </Transition>
            <Disclosure id="st-goal-new" title=t("goals-set") summary=t("goals-set-summary")
                mode=DisclosureMode::Button level=3 storage_key=disclosure_storage_key("stats", "goal")>
                <GoalForm editing=None done=refresh/>
            </Disclosure>
        </section>
        <EditGoal editing=editing done=refresh/>
        <DeleteGoal removing=removing done=refresh/>
    }
}

#[component]
fn GoalList(
    goals: Vec<GoalDto>,
    editing: RwSignal<Option<GoalDto>>,
    removing: RwSignal<Option<GoalDto>>,
) -> impl IntoView {
    (!goals.is_empty()).then(|| {
        view! {
            <ul class="grid gap-3 md:grid-cols-2">
                {goals.into_iter().map(|g| view! { <GoalCard g=g editing=editing removing=removing/> }).collect_view()}
            </ul>
        }
    })
}

#[component]
fn GoalCard(
    g: GoalDto,
    editing: RwSignal<Option<GoalDto>>,
    removing: RwSignal<Option<GoalDto>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let title = goal_title(&i18n, &g);
    let text = i18n.t_args(
        "goals-progress",
        &[
            ("done", count(&i18n, g.done).into()),
            ("target", count(&i18n, u64::from(g.target)).into()),
            ("year", g.year.to_string().into()),
        ],
    );
    let value = u32::try_from(g.done.min(u64::from(g.target))).unwrap_or(g.target);
    let label = i18n.t_args(
        "goals-bar",
        &[
            ("title", title.clone().into()),
            ("year", g.year.to_string().into()),
        ],
    );
    let edit_label = i18n.t_args("goals-edit-named", &[("title", title.clone().into())]);
    let delete_label = i18n.t_args("goals-delete-named", &[("title", title.clone().into())]);
    let (for_edit, for_delete) = (g.clone(), g.clone());
    view! {
        <li class="grid gap-2 rounded-xl border border-border p-4" data-testid="st-goal">
            <h3 class="font-semibold">{title}</h3>
            <p class="text-sm tabular-nums">{text.clone()}</p>
            <ProgressBar label=label value=Signal::derive(move || value) max=g.target.max(1) value_text=text/>
            <div class="flex flex-wrap gap-2">
                <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=edit_label
                    on:click=move |_| editing.set(Some(for_edit.clone()))>{t("goals-edit")}</Button>
                <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost attr:aria-label=delete_label
                    on:click=move |_| removing.set(Some(for_delete.clone()))>{t("goals-delete")}</Button>
            </div>
        </li>
    }
}

/// The year, group, measure and target; saves a new goal, or `editing`'s changes.
#[component]
fn GoalForm(editing: Option<GoalDto>, done: Callback<()>) -> impl IntoView {
    let today_year: i32 = crate::history::today()
        .iso()
        .get(..4)
        .and_then(|y| y.parse().ok())
        .unwrap_or(2026);
    let start = editing.clone();
    let year = RwSignal::new(start.as_ref().map_or(today_year, |g| g.year).to_string());
    let group = RwSignal::new(start.as_ref().map_or("all".into(), |g| g.group.clone()));
    let metric = RwSignal::new(
        start
            .as_ref()
            .map_or("finished".into(), |g| g.metric.clone()),
    );
    let target = RwSignal::new(
        start
            .as_ref()
            .map(|g| g.target.to_string())
            .unwrap_or_default(),
    );
    let error = RwSignal::new(None::<String>);
    let prefix = if editing.is_some() {
        "st-goal-edit"
    } else {
        "st-goal"
    };
    let id = editing.map(|g| g.id);
    let save = Action::new(move |(): &()| {
        let body = goal_body(
            &year.get_untracked(),
            &group.get_untracked(),
            &metric.get_untracked(),
            &target.get_untracked(),
        );
        let id = id.clone();
        async move {
            let Some(body) = body else {
                error.set(Some(t("goals-target-invalid")));
                return;
            };
            let r = match id {
                Some(id) => {
                    rest::call::<GoalDto>("PUT", &format!("/stats/goals/{id}"), Some(body)).await
                }
                None => rest::call::<GoalDto>("POST", "/stats/goals", Some(body)).await,
            };
            match r {
                Ok(_) => {
                    error.set(None);
                    target.set(String::new());
                    done.run(());
                }
                Err(e) => error.set(Some(goal_error(&e))),
            }
        }
    });
    view! {
        <form class="grid gap-3 pt-2 sm:grid-cols-2" novalidate on:submit=move |ev| { ev.prevent_default(); save.dispatch(()); }>
            <GoalFields prefix=prefix year=year group=group metric=metric target=target years=(today_year, today_year + 1)/>
            {move || error.get().map(|e| view! { <div class="sm:col-span-2"><Alert variant=AlertVariant::Destructive role=AlertRole::Alert>{e}</Alert></div> })}
            <Button r#type="submit" class="w-fit" loading=save.pending()>{t("goals-save")}</Button>
        </form>
    }
}

/// The body of a goal from the typed fields; `None` for a target that is not a whole number
/// of at least 1.
#[must_use]
pub fn goal_body(year: &str, group: &str, metric: &str, target: &str) -> Option<serde_json::Value> {
    let target: u32 = target.trim().parse().ok().filter(|n| *n > 0)?;
    let year: i32 = year.parse().ok()?;
    serde_json::to_value(NewGoalDto {
        year,
        group: group.into(),
        metric: metric.into(),
        target,
    })
    .ok()
}

fn goal_error(e: &ServerFnError) -> String {
    match api::error_code(e).as_str() {
        "conflict" => t("goals-error-conflict"),
        "invalid" | "bad_request" => t("goals-target-invalid"),
        _ => error_text(e),
    }
}

#[component]
fn GoalFields(
    prefix: &'static str,
    year: RwSignal<String>,
    group: RwSignal<String>,
    metric: RwSignal<String>,
    target: RwSignal<String>,
    years: (i32, i32),
) -> impl IntoView {
    let i18n = use_i18n();
    let mut year_options: Vec<SelectOption> = [years.0, years.1]
        .into_iter()
        .map(|y| SelectOption {
            value: y.to_string(),
            label: y.to_string(),
        })
        .collect();
    let current = year.get_untracked();
    if !year_options.iter().any(|o| o.value == current) {
        year_options.insert(
            0,
            SelectOption {
                value: current.clone(),
                label: current,
            },
        );
    }
    let groups: Vec<SelectOption> = ["all", "watch", "read", "listen", "play"]
        .into_iter()
        .map(|g| SelectOption {
            value: g.into(),
            label: group_label(&i18n, g),
        })
        .collect();
    let metrics: Vec<SelectOption> = GOAL_METRICS
        .into_iter()
        .map(|m| SelectOption {
            value: m.into(),
            label: i18n.t(goal_measure(m).label_key()),
        })
        .collect();
    let (y, g, m, n) = (
        format!("{prefix}-year"),
        format!("{prefix}-group"),
        format!("{prefix}-metric"),
        format!("{prefix}-target"),
    );
    view! {
        <div class="grid gap-1">
            <Label r#for=y.clone()>{t("goals-year")}</Label>
            <Select id=y value=year options=year_options/>
        </div>
        <div class="grid gap-1">
            <Label r#for=g.clone()>{t("goals-group")}</Label>
            <Select id=g value=group options=groups/>
        </div>
        <div class="grid gap-1">
            <Label r#for=m.clone()>{t("goals-metric")}</Label>
            <Select id=m value=metric options=metrics/>
        </div>
        <FormField id=n.clone() label=t("goals-target") description=t("goals-target-hint")>
            <Input id=n value=target inputmode="numeric" autocomplete="off" class="max-w-32"/>
        </FormField>
    }
}

#[component]
fn EditGoal(editing: RwSignal<Option<GoalDto>>, done: Callback<()>) -> impl IntoView {
    let open = RwSignal::new(false);
    // each side changes the other only when it differs: two effects setting each other
    // unconditionally loop for ever
    Effect::new(move |_| {
        let want = editing.with(Option::is_some);
        if open.get_untracked() != want {
            open.set(want);
        }
    });
    Effect::new(move |_| {
        if !open.get() && editing.with_untracked(Option::is_some) {
            editing.set(None);
        }
    });
    let closed = Callback::new(move |()| {
        open.set(false);
        done.run(());
    });
    move || {
        editing.get().map(|g| {
            let i18n = use_i18n();
            let title = i18n.t_args("goals-edit-named", &[("title", goal_title(&i18n, &g).into())]);
            view! {
                <Dialog open=open title=title close_label=i18n.t("dialog-close") initial_focus="st-goal-edit-target">
                    <GoalForm editing=Some(g.clone()) done=closed/>
                </Dialog>
            }
        })
    }
}

#[component]
fn DeleteGoal(removing: RwSignal<Option<GoalDto>>, done: Callback<()>) -> impl IntoView {
    let open = RwSignal::new(false);
    Effect::new(move |_| open.set(removing.with(Option::is_some)));
    let i18n = use_i18n();
    let i2 = i18n.clone();
    let title = Signal::derive(move || {
        removing.with(|g| {
            g.as_ref()
                .map(|g| {
                    i2.t_args(
                        "goals-delete-title",
                        &[("title", goal_title(&i2, g).into())],
                    )
                })
                .unwrap_or_default()
        })
    });
    let body = Signal::derive(move || i18n.t("goals-delete-body"));
    let run = Action::new(move |(): &()| {
        let id = removing.get_untracked().map(|g| g.id).unwrap_or_default();
        async move {
            let _ = rest::send("DELETE", &format!("/stats/goals/{id}"), None).await;
            removing.set(None);
            done.run(());
        }
    });
    view! {
        <ConfirmDialog open=open title=title body=body confirm=t("goals-delete")
            on_confirm=Callback::new(move |()| { run.dispatch(()); })/>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use contracts::AmountsDto;

    use super::*;

    fn amounts(finished: u64, pages: u64) -> AmountsDto {
        AmountsDto {
            finished,
            pages,
            ..AmountsDto::default()
        }
    }

    /// Review OT2F-2: "Your results" counts the plays of the totals row: decided, open (with
    /// details, without a result of the person) and bare (without details) add up to it.
    #[test]
    fn the_results_count_the_plays_of_the_totals_row() {
        let en = I18n::new("en", BTreeMap::new());
        let t = PlayTotalsDto {
            plays: 4,
            minutes: 240,
            decided: 3,
            wins: 2,
        };
        let all = result_words(&en, &t, 4);
        assert_eq!(all.rate, "67%");
        assert_eq!(
            all.detail.as_deref(),
            Some("2 of 3 plays with a result in your own log")
        );
        assert_eq!(
            all.open.as_deref(),
            Some("One play is not counted (no result, or without you).")
        );
        assert_eq!(all.bare, None);
        // the totals row counts three plays more than were read with their details
        let more = result_words(&en, &t, 7);
        assert_eq!(more.open, all.open);
        assert_eq!(
            more.bare.as_deref(),
            Some(
                "3 plays have no details (players, result, duration). They count as plays, \
                 not in the time or the win rate."
            )
        );
        // an imported history alone: no rate, nothing "not counted", the plays are named
        let bare = result_words(&en, &PlayTotalsDto::default(), 3);
        assert_eq!(bare.rate, "no results yet");
        assert_eq!((bare.detail, bare.open), (None, None));
        assert!(
            bare.bare
                .is_some_and(|b| b.starts_with("3 plays have no details"))
        );
    }

    #[test]
    fn the_comparison_names_each_group_and_skips_empty_ones() {
        let en = I18n::new("en", BTreeMap::new());
        let c = vec![
            CompareDto {
                group: "all".into(),
                this: amounts(3, 40),
                last: amounts(1, 0),
            },
            CompareDto {
                group: "watch".into(),
                this: amounts(3, 0),
                last: amounts(1, 0),
            },
            CompareDto {
                group: "read".into(),
                this: amounts(0, 40),
                last: amounts(0, 0),
            },
            CompareDto {
                group: "listen".into(),
                this: amounts(0, 0),
                last: amounts(0, 0),
            },
        ];
        let all = compare_lines(&en, &c, "all");
        assert_eq!(all.len(), 2, "listen has nothing in either year");
        assert_eq!(
            (all[0].label.as_str(), all[0].this, all[0].last),
            ("Watch: Watched", 3, 1)
        );
        assert_eq!((all[1].label.as_str(), all[1].this), ("Read: Pages", 40));
        let read = compare_lines(&en, &c, "read");
        assert_eq!(read[0].label, "Pages");
        assert_eq!(compare_lines(&en, &c, "play").len(), 0);
    }

    #[test]
    fn the_comparison_leads_with_hours_when_there_are_any() {
        // OT-17: time first; a group without any time falls back to its first count
        let en = I18n::new("en", BTreeMap::new());
        let timed = |finished, minutes| AmountsDto {
            finished,
            minutes,
            ..AmountsDto::default()
        };
        let c = vec![CompareDto {
            group: "watch".into(),
            this: timed(3, 300),
            last: timed(1, 0),
        }];
        let all = compare_lines(&en, &c, "all");
        assert_eq!(all.len(), 1);
        assert_eq!(
            (all[0].label.as_str(), all[0].measure, all[0].this),
            ("Watch: Hours", Measure::Hours, 300)
        );
        assert_eq!(all[0].measure.text(&en, all[0].this), "5");
        let labels: Vec<String> = compare_lines(&en, &c, "watch")
            .into_iter()
            .map(|l| l.label)
            .collect();
        assert_eq!(labels, ["Hours", "Watched", "Episodes"]);
        let untimed = vec![CompareDto {
            group: "watch".into(),
            this: timed(3, 0),
            last: timed(1, 0),
        }];
        assert_eq!(
            compare_lines(&en, &untimed, "all")[0].label,
            "Watch: Watched"
        );
    }

    #[test]
    fn a_group_without_time_in_either_year_compares_its_counts_first() {
        // review OT2C-5: no "Hours - 0 in 2026, 0 in 2025" on top of the group's own tab
        let en = I18n::new("en", BTreeMap::new());
        let c = vec![CompareDto {
            group: "watch".into(),
            this: AmountsDto {
                finished: 3,
                ..AmountsDto::default()
            },
            last: AmountsDto::default(),
        }];
        let labels: Vec<String> = compare_lines(&en, &c, "watch")
            .into_iter()
            .map(|l| l.label)
            .collect();
        assert_eq!(labels, ["Watched", "Episodes", "Hours"]);
    }

    #[test]
    fn a_goal_body_needs_a_whole_target_of_at_least_one() {
        let b = goal_body("2026", "read", "pages", " 500 ").unwrap();
        assert_eq!(b["target"], 500);
        assert_eq!(b["group"], "read");
        assert_eq!(goal_body("2026", "all", "plays", "0"), None);
        assert_eq!(goal_body("2026", "all", "plays", "1.5"), None);
        assert_eq!(goal_body("2026", "all", "plays", ""), None);
    }
}
