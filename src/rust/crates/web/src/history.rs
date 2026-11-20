//! The History page (design.md 6.6, 3.6; addendum-1 1.3): one stream of everything logged,
//! grouped by day, with the group switcher open and the other filters (kind, person, type,
//! shared with me, year) behind one Filters disclosure, all in the URL ([`HistoryState`]);
//! "Load more" pages with the feed's `next`; a side panel counts what is on screen. The
//! activity row is shared with Home.

use contracts::{ActivityPageDto, ActivityRowDto, CompanionDto, KindsDto};
use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_query_map};
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant,
    Checkbox, Disclosure, DisclosureMode, EmptyState, Icon, IconName, Label, Segmented,
    SegmentedOption, Select, SelectOption, Skeleton, ToastVariant,
};

use crate::api;
use crate::history_state::{
    Day, DayGroup, HISTORY_PAGE, HistoryState, day_heading, group_by_day, side_counts,
};
use crate::i18n::{I18n, kind_label, t, use_i18n};
use crate::next_step::{StepCtx, undo_event};
use crate::pages::{PageTitle, bad_query, error_text, go, needs_sign_in};
use crate::widgets::progress_label;

/// The feed's first page and the History address it was asked for.
type Loaded = (String, Result<ActivityPageDto, ServerFnError>);

/// Today as the server saw it when it rendered the page (so both sides group alike).
pub(crate) fn today() -> Day {
    let cfg = expect_context::<crate::app::ClientConfig>();
    if let Some(d) = Day::parse(&cfg.today) {
        return d;
    }
    #[cfg(not(target_arch = "wasm32"))]
    return Day::today_utc();
    #[cfg(target_arch = "wasm32")]
    Day::from_days(0)
}

/// "Anna, Jonas" for the people other than the viewer.
fn others(row: &ActivityRowDto) -> Vec<String> {
    row.participants
        .iter()
        .filter(|p| !p.is_me)
        .map(|p| p.name.clone())
        .collect()
}

/// What happened, in words: the progress, else a logged session, which a board game calls a
/// play (as the dialog, its toast and the "Plays" tab do; OT-16, v1-polish P3).
/// A session reaches the feed as a row of its own or as the history event `session`.
fn what(i: &I18n, row: &ActivityRowDto) -> String {
    match &row.progress {
        Some(p) if p.get("type").and_then(|t| t.as_str()) != Some("session") => {
            progress_label(i, p)
        }
        _ => i.t(session_key(&row.kind)),
    }
}

/// "Play logged" for a board game, "Session logged" for every other kind with sessions.
fn session_key(kind: &str) -> &'static str {
    if kind == "board_game" {
        "history-play"
    } else {
        "history-session"
    }
}

/// The parts of what a play says beyond its people: the result, the scores, the duration.
/// The activity row joins them into one line ([`play_line`]); the "Plays" tab of a game
/// page puts each into its own column.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayWords {
    /// "Ben won", "You won", "Won together", "Played together" (a co-op play without a
    /// result, OT-18); `None` when no result was logged.
    pub result: Option<String>,
    /// "Scores: You 31, Ben 42.5"; alone "Score: 12".
    pub scores: Option<String>,
    /// The scores without their label: "You 31, Ben 42.5"; alone "12".
    pub scores_bare: Option<String>,
    pub duration_minutes: Option<u32>,
}

/// The words of a play with these people.
#[must_use]
pub fn play_words(
    i: &I18n,
    people: &[contracts::ActivityPersonDto],
    play: &contracts::ActivityPlayDto,
) -> PlayWords {
    let name = |p: &contracts::ActivityPersonDto| {
        if p.is_me {
            i.t("history-play-you")
        } else {
            p.name.clone()
        }
    };
    let winners: Vec<_> = people.iter().filter(|p| p.winner).collect();
    let result = match (play.coop.as_deref(), winners.as_slice()) {
        (Some("won"), _) => Some(i.t("history-play-coop-won")),
        (Some("lost"), _) => Some(i.t("history-play-coop-lost")),
        // any other co-op word is a play without a result
        (Some(_), _) => Some(i.t("history-play-coop-none")),
        (None, []) => None,
        (None, [one]) if one.is_me => Some(i.t("history-play-won-you")),
        (None, many) => {
            let names: Vec<String> = many.iter().map(|p| name(p)).collect();
            Some(i.t_args("history-play-won", &[("names", names.join(", ").into())]))
        }
    };
    let with_score: Vec<_> = people
        .iter()
        .filter_map(|p| p.score.map(|s| (p, i.number(s))))
        .collect();
    let (scores, scores_bare) = match with_score.as_slice() {
        [] => (None, None),
        [(_, score)] if people.len() == 1 => (
            Some(i.t_args("history-play-score", &[("score", score.clone().into())])),
            Some(score.clone()),
        ),
        many => {
            let listed = many
                .iter()
                .map(|(p, score)| format!("{} {score}", name(p)))
                .collect::<Vec<_>>()
                .join(", ");
            (
                Some(i.t_args("history-play-scores", &[("scores", listed.clone().into())])),
                Some(listed),
            )
        }
    };
    PlayWords {
        result,
        scores,
        scores_bare,
        duration_minutes: play.duration_minutes.filter(|m| *m > 0),
    }
}

/// What the row's session says beyond its people (OT-11): who won or how the co-op game
/// ended, the scores, the duration, as "Ben won - Scores: You 31, Ben 42.5 - 45 min". `None`
/// for a row without a session and for a session that says none of it.
fn play_line(i: &I18n, row: &ActivityRowDto) -> Option<String> {
    let words = play_words(i, &row.participants, row.play.as_ref()?);
    let duration = words.duration_minutes.map(|minutes| {
        let minutes = i.number(f64::from(minutes));
        i.t_args("history-play-duration", &[("minutes", minutes.into())])
    });
    let parts: Vec<String> = [words.result, words.scores, duration]
        .into_iter()
        .flatten()
        .collect();
    (!parts.is_empty()).then(|| parts.join(" - "))
}

/// The time of a row: the server's `local_time` in the viewer's zone (DEC-306), unmarked;
/// without it the UTC time of `at`, marked as UTC.
fn clock(i: &I18n, row: &ActivityRowDto) -> Option<String> {
    let hm = |s: &str, at: usize| -> Option<(u32, u32)> {
        Some((
            s.get(at..at + 2)?.parse().ok()?,
            s.get(at + 3..at + 5)?.parse().ok()?,
        ))
    };
    if let Some((h, m)) = hm(&row.local_time, 0) {
        return Some(i.time(h, m));
    }
    let (h, m) = hm(&row.at, 11)?;
    Some(i.t_args("history-time-utc", &[("time", i.time(h, m).into())]))
}

/// Date and time of a row for Home (no day headings there): local when the server sent it.
fn dated_text(i: &I18n, row: &ActivityRowDto) -> String {
    match clock(i, row) {
        Some(time) if !row.local_day.is_empty() => i.t_args(
            "datetime-local",
            &[
                ("date", i.date(&row.local_day).into()),
                ("time", time.into()),
            ],
        ),
        _ => i.datetime(&row.at),
    }
}

/// One activity row as a sentence (Home and History): kind icon, title link, what happened,
/// who was there, when; a shared row says whose it is and offers "Copy to my log"; an own
/// recent row offers Undo. `dated` shows the date with the time (Home has no day headings).
#[component]
pub fn ActivityRow(
    row: ActivityRowDto,
    #[prop(optional)] dated: bool,
    /// Runs after an undo or a copy (refetch).
    #[prop(optional_no_strip, into)]
    on_change: Option<Callback<()>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let names = others(&row);
    let with = (!names.is_empty())
        .then(|| i18n.t_args("history-with", &[("names", names.join(", ").into())]));
    let when = if dated {
        Some(dated_text(&i18n, &row))
    } else {
        clock(&i18n, &row)
    };
    let kind = kind_label(&row.kind);
    let result = play_line(&i18n, &row);
    let by = row
        .owner_name
        .clone()
        .filter(|_| row.shared)
        .map(|n| i18n.t_args("sessions-by", &[("name", n.into())]));
    let undo = row
        .event_id
        .clone()
        .filter(|_| row.undoable && !row.shared)
        .map(|id| {
            let label = i18n.t_args(
                "history-undo-named",
                &[("title", crate::i18n::title_text(&row.title).into())],
            );
            let ctx = ctx.clone();
            view! {
                <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm attr:aria-label=label
                    on:click=move |_| undo_event(&ctx, id.clone(), on_change)>
                    {t("library-undo")}
                </Button>
            }
        });
    // a copied shared row says so in place and offers no second copy (#35); the server
    // knows it (`copied`), so a reload keeps it
    let copy = row.session_id.clone().filter(|_| row.shared).map(|id| {
        let label = i18n.t_args(
            "history-copy-named",
            &[("title", crate::i18n::title_text(&row.title).into())],
        );
        let ctx = ctx.clone();
        let copied = RwSignal::new(row.copied);
        let busy = RwSignal::new(false);
        move || {
            if copied.get() {
                view! {
                    <Badge variant=BadgeVariant::Success attr:data-testid="activity-copied">
                        <Icon name=IconName::Check/> {t("history-copied")}
                    </Badge>
                }
                .into_any()
            } else {
                let (ctx, id, label) = (ctx.clone(), id.clone(), label.clone());
                view! {
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=label
                        attr:aria-disabled=move || busy.get().then_some("true")
                        on:click=move |_| {
                            if !busy.get_untracked() {
                                busy.set(true);
                                copy_to_log(&ctx, id.clone(), copied, busy, on_change);
                            }
                        }>
                        {t("sessions-claim")}
                    </Button>
                }
                .into_any()
            }
        }
    });
    let look = if row.shared {
        "flex flex-wrap items-start gap-3 rounded-md border border-info/40 bg-info-soft px-3 py-2"
    } else {
        "flex flex-wrap items-start gap-3 rounded-md border border-border px-3 py-2"
    };
    view! {
        <li class=look data-testid="activity-row">
            <span class=ui::cn(&["mt-0.5", crate::tint::group_tint(&row.group).text()]) title=kind.clone()>
                <Icon name=crate::library::kind_icon(&row.kind)/>
            </span>
            <span class="min-w-0 flex-1">
                <span class="block">
                    <a href=format!("/items/{}", row.item_id) class="font-medium underline-offset-2 hover:underline">{crate::i18n::title_text(&row.title)}</a>
                    " - " {what(&i18n, &row)}
                    {with.map(|w| view! { " " <span>{w}</span> })}
                </span>
                {result.map(|r| view! { <span class="block text-sm" data-testid="activity-play">{r}</span> })}
                <span class="block text-xs text-muted-foreground">
                    <span class="sr-only">{kind.clone()} " - "</span>
                    {when}
                    {by.map(|b| view! { " - " {b} })}
                </span>
            </span>
            {row.shared.then(|| view! { <Badge variant=BadgeVariant::Info>{t("history-shared")}</Badge> })}
            {copy}
            {undo}
        </li>
    }
}

fn copy_to_log(
    ctx: &StepCtx,
    session_id: String,
    copied: RwSignal<bool>,
    busy: RwSignal<bool>,
    on_change: Option<Callback<()>>,
) {
    let (done, failed) = (ctx.i18n.t("sessions-claimed"), ctx.i18n.t("error-generic"));
    let toasts = ctx.toasts;
    leptos::task::spawn_local(async move {
        let ok = api::claim(session_id).await.is_ok();
        // the row may have been replaced by a refetch meanwhile (DEC-865)
        let _ = busy.try_set(false);
        if ok {
            let _ = copied.try_set(true);
            toasts.push(done, None, ToastVariant::Default);
        } else {
            toasts.push(failed, None, ToastVariant::Destructive);
        }
        if let Some(cb) = on_change {
            cb.run(());
        }
    });
}

/// Pages after the first answer, loaded by "Show more" for one state (`href`). They continue
/// the first answer whose `next` is `after`; a refetched first answer (it brings the loaded
/// range itself, DEC-490) has another `next` and so leaves them out.
#[derive(Clone, Default, PartialEq)]
struct More {
    href: String,
    after: Option<String>,
    rows: Vec<ActivityRowDto>,
    next: Option<String>,
}

/// Rows on screen and the cursor for the next page: the first answer of address `href` and the
/// "Show more" pages that continue it.
fn shown_rows(
    more: &More,
    href: &str,
    first: &ActivityPageDto,
) -> (Vec<ActivityRowDto>, Option<String>) {
    let mut rows = first.rows.clone();
    if more.href == href && first.next.is_some() && more.after == first.next {
        rows.extend(more.rows.iter().cloned());
        (rows, more.next.clone())
    } else {
        (rows, first.next.clone())
    }
}

/// The first answer for `state` with at least `rows` rows (pages of up to the feed's maximum,
/// following `next`): a refresh fetches again what is on screen in one answer (DEC-490).
async fn feed_range(state: HistoryState, rows: usize) -> Result<ActivityPageDto, ServerFnError> {
    // the feed answers at most this many rows per page (`app::MAX_ACTIVITY_PAGE`)
    const MAX_PAGE: usize = 100;
    let ask = |before: Option<String>, left: usize| {
        let mut q = state.query_dto(before);
        q.limit = Some(u32::try_from(left.clamp(1, MAX_PAGE)).unwrap_or(HISTORY_PAGE));
        q
    };
    let mut page = api::activity_feed(ask(None, rows)).await?;
    while page.rows.len() < rows
        && let Some(next) = page.next.clone()
    {
        let more = api::activity_feed(ask(Some(next), rows - page.rows.len())).await?;
        if more.rows.is_empty() {
            page.next = None;
            break;
        }
        page.rows.extend(more.rows);
        page.next = more.next;
    }
    Ok(page)
}

#[derive(Clone, Copy)]
struct Hist {
    state: Memo<HistoryState>,
    go: Callback<HistoryState>,
    first: Resource<Loaded>,
    more: RwSignal<More>,
    /// "Show more" is loading; why it failed.
    loading: RwSignal<bool>,
    more_error: RwSignal<Option<String>>,
    refresh: Callback<()>,
}

impl Hist {
    /// Rows on screen and the cursor for the next page, for the answer of address `href`.
    fn shown(&self, href: &str, first: &ActivityPageDto) -> (Vec<ActivityRowDto>, Option<String>) {
        self.more.with(|m| shown_rows(m, href, first))
    }

    /// Load the page at `next` after the first answer whose `next` is `after`.
    fn load_more(&self, next: String, after: Option<String>) {
        let (state, more, loading, error) = (
            self.state.get_untracked(),
            self.more,
            self.loading,
            self.more_error,
        );
        let href = state.href();
        loading.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let result = api::activity_feed(state.query_dto(Some(next))).await;
            loading.set(false);
            match result {
                Err(e) => error.set(Some(error_text(&e))),
                Ok(page) => more.update(|m| {
                    if m.href != href || m.after != after {
                        *m = More {
                            href: href.clone(),
                            after: after.clone(),
                            ..More::default()
                        };
                    }
                    m.rows.extend(page.rows);
                    m.next = page.next;
                }),
            }
        });
    }
}

#[component]
pub fn History() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_query_map();
    let state = Memo::new(move |_| query.with(|q| HistoryState::from_lookup(|k| q.get(k))));
    let nav = StoredValue::new(use_navigate());
    let go_cb = Callback::new(move |s: HistoryState| {
        let href = s.href();
        nav.with_value(|n| {
            n(
                &href,
                NavigateOptions {
                    scroll: false,
                    ..NavigateOptions::default()
                },
            );
        });
    });
    // the answer carries the address it belongs to (UX3-3): the stream never shows the
    // previous address's result under the new one
    // a refresh asks for as many rows as are on screen for that address (taken by the next
    // fetch); any other fetch asks for one page
    let want = StoredValue::new((String::new(), 0usize));
    let first = Resource::new(
        move || state.get(),
        move |s: HistoryState| {
            let href = s.href();
            let rows = want
                .try_update_value(std::mem::take)
                .filter(|(h, _)| *h == href)
                .map_or(0, |(_, n)| n)
                .max(HISTORY_PAGE as usize);
            async move { (href, feed_range(s, rows).await) }
        },
    );
    let more = RwSignal::new(More::default());
    // an event or an undo fetches again the rows on screen, "Show more" pages included, in one
    // answer: the pages stay and the stream keeps its rendering (DEC-490; it reset the pages)
    let refetch = Callback::new(move |()| {
        if let Some((href, Ok(page))) = first.get_untracked() {
            let n = more.with_untracked(|m| shown_rows(m, &href, &page).0.len());
            want.set_value((href, n));
        }
        first.refetch();
    });
    // paced: an import announces every item it writes (OT-21); the own refreshes (an undo, a
    // change on a row) tell the same pace (LR-5)
    let own = crate::events::refresh_on_updates(crate::offline::refresh_when_online(refetch));
    let refresh = own.wrap(refetch);
    let h = Hist {
        state,
        go: go_cb,
        first,
        more,
        loading: RwSignal::new(false),
        more_error: RwSignal::new(None),
        refresh,
    };
    view! {
        // the H1 carries a text link to the export (DEC-237)
        <div class="flex flex-wrap items-baseline justify-between gap-x-4">
            <PageTitle text=i18n.t("history-title")/>
            <a href="/settings/imports#export" class="mb-6 text-sm text-primary underline" data-testid="hist-export">
                {i18n.t("history-export")}
            </a>
        </div>
        <p class="-mt-4 mb-4 text-sm text-muted-foreground">{i18n.t("history-lead")}</p>
        <Filters h=h/>
        <div class="grid gap-6 lg:grid-cols-[minmax(0,1fr)_16rem]">
            <Stream h=h/>
            <Side h=h/>
        </div>
    }
}

/// Group switcher (open) and the Filters disclosure (kind, person, type, shared, year).
#[component]
fn Filters(h: Hist) -> impl IntoView {
    let i18n = use_i18n();
    let kinds = Resource::new(|| (), |()| api::kinds());
    let group = text_control(
        h,
        |s| Some(s.group.clone().unwrap_or_else(|| "all".into())),
        HistoryState::with_group,
    );
    let active = Signal::derive(move || h.state.with(HistoryState::hidden_filters) > 0);
    let summary = Signal::derive(move || {
        let n = h.state.with(HistoryState::hidden_filters);
        use_i18n().t_args("history-filters-summary", &[("count", n.into())])
    });
    view! {
        <div class="mb-4 flex flex-col gap-3">
            <div class="-mx-4 max-w-[calc(100%+2rem)] overflow-x-auto px-4">
                <Suspense fallback=|| view! { <Skeleton class="h-9 w-72"/> }>
                    {move || kinds.get().and_then(Result::ok).map(|k| {
                        let i = use_i18n();
                        let mut options = vec![SegmentedOption::new("all", i.t("kind-group-all"))];
                        options.extend(k.groups.iter().map(|g| crate::tint::group_option(&g.id, i.t(&g.label_key))));
                        view! { <Segmented id="hist-group" label=i.t("library-group") value=group options=options/> }
                    })}
                </Suspense>
            </div>
            <Disclosure id="hist-filters" title=i18n.t("history-filters") summary=summary mode=DisclosureMode::Button level=2 force_open=active>
                <Suspense fallback=|| view! { <Skeleton class="h-20 w-full"/> }>
                    <FilterFields h=h kinds=kinds group=group/>
                </Suspense>
            </Disclosure>
        </div>
    }
}

/// A select bound to one optional text field of the URL state ("" is "any").
fn text_control(
    h: Hist,
    get: fn(&HistoryState) -> Option<String>,
    set: fn(&HistoryState, Option<String>) -> HistoryState,
) -> RwSignal<String> {
    let value = RwSignal::new(h.state.with_untracked(|s| get(s).unwrap_or_default()));
    bind(
        value,
        h,
        move |s| get(s).unwrap_or_default(),
        move |s, v| set(s, some(v)),
    );
    value
}

/// The fields behind the Filters button.
#[component]
fn FilterFields(
    h: Hist,
    kinds: Resource<Result<KindsDto, ServerFnError>>,
    group: RwSignal<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let people = Resource::new(|| (), |()| api::companions());
    let kind = text_control(
        h,
        |s| s.kind.clone(),
        |s, kind| HistoryState { kind, ..s.clone() },
    );
    let person = text_control(
        h,
        |s| s.person.clone(),
        |s, person| HistoryState {
            person,
            ..s.clone()
        },
    );
    let row_type = text_control(
        h,
        |s| s.row_type.clone(),
        |s, row_type| HistoryState {
            row_type,
            ..s.clone()
        },
    );
    let year = text_control(
        h,
        |s| s.year.map(|y| y.to_string()),
        |s, v| HistoryState {
            year: v.and_then(|v| v.parse().ok()),
            ..s.clone()
        },
    );
    let shared = RwSignal::new(h.state.with_untracked(|s| s.shared));
    bind(
        shared,
        h,
        |s| s.shared,
        |s, shared| HistoryState {
            shared,
            ..s.clone()
        },
    );
    let types = [
        ("", "history-type-all"),
        ("progress", "history-type-progress"),
        ("session", "history-type-session"),
    ]
    .into_iter()
    .map(|(value, key)| SelectOption {
        value: value.into(),
        label: i18n.t(key),
    })
    .collect::<Vec<_>>();
    let kind_options = Signal::derive(move || {
        kind_options(
            &use_i18n(),
            kinds.get().and_then(Result::ok).as_ref(),
            &group.get(),
        )
    });
    let people_options = Signal::derive(move || {
        people_options(
            &use_i18n(),
            &people.get().and_then(Result::ok).unwrap_or_default(),
        )
    });
    view! {
        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            <div class="flex flex-col gap-1">
                <Label r#for="hist-kind">{t("history-filter-kind")}</Label>
                <Select id="hist-kind" value=kind options=kind_options/>
            </div>
            <div class="flex flex-col gap-1">
                <Label r#for="hist-person">{t("history-filter-person")}</Label>
                <Select id="hist-person" value=person options=people_options/>
            </div>
            <div class="flex flex-col gap-1">
                <Label r#for="hist-type">{t("history-filter-type")}</Label>
                <Select id="hist-type" value=row_type options=types/>
            </div>
            <div class="flex flex-col gap-1">
                <Label r#for="hist-year">{t("history-filter-year")}</Label>
                <Select id="hist-year" value=year options=year_options(h)/>
            </div>
            <div class="flex min-h-11 items-center gap-2 self-end">
                <Checkbox id="hist-shared" checked=shared/>
                <Label r#for="hist-shared">{t("history-filter-shared")}</Label>
            </div>
            <div class="flex items-end">
                <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on:click=move |_| h.go.run(HistoryState::default())>
                    {t("library-clear")}
                </Button>
            </div>
        </div>
    }
}

/// "All years" and every year with activity, newest first (plus the year in the URL).
fn year_options(h: Hist) -> Signal<Vec<SelectOption>> {
    let years = Resource::new(
        || (),
        |()| api::stats_series(Some("all".into()), Some("year".into())),
    );
    Signal::derive(move || {
        let i = use_i18n();
        let mut v = vec![SelectOption {
            value: String::new(),
            label: i.t("history-year-all"),
        }];
        let mut ys: Vec<String> = years
            .get()
            .and_then(Result::ok)
            .map(|s| s.points.into_iter().map(|p| p.bucket).collect())
            .unwrap_or_default();
        if let Some(y) = h
            .state
            .with(|s| s.year)
            .map(|y| y.to_string())
            .filter(|y| !ys.contains(y))
        {
            ys.push(y);
        }
        ys.sort_unstable_by(|a, b| b.cmp(a));
        v.extend(ys.into_iter().map(|y| SelectOption {
            value: y.clone(),
            label: y,
        }));
        v
    })
}

fn some(v: String) -> Option<String> {
    (!v.is_empty()).then_some(v)
}

fn kind_options(i: &I18n, kinds: Option<&KindsDto>, group: &str) -> Vec<SelectOption> {
    let mut v = vec![SelectOption {
        value: String::new(),
        label: i.t("history-kind-all"),
    }];
    if let Some(k) = kinds {
        v.extend(
            k.kinds
                .iter()
                .filter(|d| group == "all" || d.group == group)
                .map(|d| SelectOption {
                    value: d.id.clone(),
                    label: kind_label(&d.id),
                }),
        );
    }
    v
}

fn people_options(i: &I18n, people: &[CompanionDto]) -> Vec<SelectOption> {
    let mut v = vec![SelectOption {
        value: String::new(),
        label: i.t("history-person-all"),
    }];
    v.extend(
        people
            .iter()
            .filter(|c| !c.is_self && c.merged_into.is_none())
            .map(|c| SelectOption {
                value: c.id.clone(),
                label: c.name.clone(),
            }),
    );
    v
}

/// Keep a control's signal and the URL in step: the URL sets the control (back button), a
/// change by the user navigates.
fn bind<T: Clone + PartialEq + Send + Sync + 'static>(
    value: RwSignal<T>,
    h: Hist,
    from_state: impl Fn(&HistoryState) -> T + Send + Sync + 'static,
    to_state: impl Fn(&HistoryState, T) -> HistoryState + 'static,
) {
    let from_state = std::sync::Arc::new(from_state);
    let f = from_state.clone();
    Effect::new(move |_| {
        let v = h.state.with(|s| f(s));
        if value.get_untracked() != v {
            value.set(v);
        }
    });
    Effect::new(move |prev: Option<()>| {
        let v = value.get();
        if prev.is_some() {
            let s = h.state.get_untracked();
            if from_state(&s) != v {
                h.go.run(to_state(&s, v));
            }
        }
    });
}

/// The stream: day groups, "Load more", empty and error states; three skeleton rows while
/// loading.
#[component]
fn Stream(h: Hist) -> impl IntoView {
    let i18n = use_i18n();
    // one item per answer; the key changes only with the answer or when the address on
    // screen moves away from it, so "Load more" and other signals never remount the list
    let each = move || {
        h.first.get().map(|(href, r)| {
            let stale = h.state.with(HistoryState::href) != href;
            (stream_key(&href, stale, &r), href, r, stale)
        })
    };
    view! {
        <section aria-labelledby="hist-stream-title" class="min-w-0">
            <h2 id="hist-stream-title" class="sr-only">{i18n.t("history-stream")}</h2>
            <Transition fallback=|| view! { <RowSkeletons/> }>
                <For each=each key=|(k, _, _, _)| k.clone() children=move |(_, href, r, stale)| {
                    stream_body(h, href, r, stale)
                }/>
            </Transition>
        </section>
    }
}

/// The key of one rendering of the stream (UX3-3): the address the answer belongs to,
/// whether the address on screen has moved on (a new one is loading), and the answer itself;
/// for rows only that there are rows, as the day groups follow the latest answer themselves
/// (a refresh keeps "Show more" and its focus, DEC-490).
fn stream_key(href: &str, stale: bool, r: &Result<ActivityPageDto, ServerFnError>) -> String {
    let data = match r {
        Ok(page) if !page.rows.is_empty() => "rows".to_string(),
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    format!("{stale}|{href}|{data}")
}

/// The latest answer with rows for `href`, starting from `first` (browser only: on the server
/// the first answer, as a second resource read under the Transition can cut the SSR stream,
/// DEC-398).
fn latest_page(
    rows: Resource<Loaded>,
    href: String,
    first: ActivityPageDto,
) -> Memo<ActivityPageDto> {
    #[cfg(feature = "hydrate")]
    let page = Memo::new(move |prev: Option<&ActivityPageDto>| match rows.get() {
        Some((h, Ok(p))) if h == href && !p.rows.is_empty() => p,
        _ => prev.cloned().unwrap_or_else(|| first.clone()),
    });
    #[cfg(not(feature = "hydrate"))]
    let page = {
        let _ = (rows, href);
        Memo::new(move |_| first.clone())
    };
    page
}

/// The stream for one answer: skeletons while the next address loads, else the rows, the
/// empty or no-match state, or the error.
fn stream_body(
    h: Hist,
    href: String,
    r: Result<ActivityPageDto, ServerFnError>,
    stale: bool,
) -> AnyView {
    if stale {
        return view! { <RowSkeletons/> }.into_any();
    }
    // "no match" belongs to the address of this answer, not to the one on screen
    let filtered = href != HistoryState::default().href();
    match r {
        Err(e) if needs_sign_in(&e) => {
            go("/sign-in?next=/history");
            ().into_any()
        }
        Err(e) if bad_query(&e) => view! { <NoRows h=h filtered=true/> }.into_any(),
        Err(e) => view! {
            <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=t("history-error")
                actions=move || view! { <Button variant=ButtonVariant::Outline size=ButtonSize::Sm on:click=move |_| h.refresh.run(())>{t("library-retry")}</Button> }>
                {error_text(&e)}
            </Alert>
        }.into_any(),
        Ok(page) if page.rows.is_empty() => view! { <NoRows h=h filtered=filtered/> }.into_any(),
        Ok(page) => view! { <Days h=h href=href page=page/> }.into_any(),
    }
}

/// No rows: "no match" with Clear filters when filtered, else the empty history.
#[component]
fn NoRows(h: Hist, filtered: bool) -> impl IntoView {
    let palette = expect_context::<crate::palette::PaletteOpen>().0;
    if filtered {
        view! {
            <EmptyState title=t("history-no-match") icon=IconName::Filter body=t("history-no-match-body")
                primary=move || view! { <Button variant=ButtonVariant::Outline on:click=move |_| h.go.run(HistoryState::default())>{t("library-clear")}</Button> }/>
        }.into_any()
    } else {
        view! {
            <EmptyState title=t("history-empty") icon=IconName::History body=t("history-empty-body")
                primary=move || view! { <Button on:click=move |_| palette.set(true)><Icon name=IconName::Search/> {t("library-find")}</Button> }/>
        }.into_any()
    }
}

/// The day groups and "Show more". Pages loaded by "Show more" are appended and a refresh
/// brings the rows on screen again: only a day group whose rows change is rendered again (its
/// handlers carry the new data, UX1-2), and the button stays (it keeps the focus, UX3-3,
/// DEC-490).
#[component]
fn Days(h: Hist, href: String, page: ActivityPageDto) -> impl IntoView {
    let today = today();
    let page = latest_page(h.first, href.clone(), page);
    let shown = Memo::new(move |_| page.with(|p| h.shown(&href, p)));
    let groups = Memo::new(move |_| shown.with(|(rows, _)| group_by_day(rows.clone(), today)));
    let next = Memo::new(move |_| shown.with(|(_, next)| next.clone()));
    let after = move || page.with_untracked(|p| p.next.clone());
    view! {
        <div class="flex flex-col gap-6">
            <For each=move || groups.get() key=day_key let:g>
                <DaySection h=h g=g/>
            </For>
            <Show when=move || next.with(Option::is_some)>
                <div class="flex flex-col items-center gap-2">
                    // busy, not disabled, while a page loads: a disabled button loses the
                    // keyboard focus (UX3-3)
                    <Button variant=ButtonVariant::Outline
                        attr:aria-busy=move || h.loading.get().then_some("true")
                        attr:aria-disabled=move || h.loading.get().then_some("true")
                        on:click=move |_| {
                            if h.loading.get_untracked() {
                                return;
                            }
                            if let Some(n) = next.get_untracked() {
                                h.load_more(n, after());
                            }
                        }>{t("library-more")}</Button>
                    {move || h.more_error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
                </div>
            </Show>
        </div>
    }
}

/// A day group is rendered again when its rows change (not only their number).
fn day_key(g: &DayGroup) -> String {
    format!(
        "{}|{}",
        g.day.iso(),
        serde_json::to_string(&g.rows).unwrap_or_default()
    )
}

#[component]
fn DaySection(h: Hist, g: DayGroup) -> impl IntoView {
    let heading = day_heading(&use_i18n(), &g);
    let id = format!("hist-day-{}", g.day.iso());
    let labelled = id.clone();
    view! {
        <section aria-labelledby=labelled>
            <h3 id=id class="mb-2 text-sm font-semibold">{heading}</h3>
            <ul class="flex flex-col gap-2">
                {g.rows.into_iter().map(|row| view! { <ActivityRow row=row on_change=h.refresh/> }).collect_view()}
            </ul>
        </section>
    }
}

#[component]
fn RowSkeletons() -> impl IntoView {
    view! {
        <p class="sr-only" role="status">{t("history-loading")}</p>
        <Skeleton class="mb-2 h-4 w-32"/>
        <div class="flex flex-col gap-2">
            {(0..3).map(|_| view! { <Skeleton class="h-14 w-full"/> }).collect_view()}
        </div>
    }
}

/// Counts for the rows on screen, and the way to all stats.
#[component]
fn Side(h: Hist) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <aside aria-labelledby="hist-side-title" class="flex flex-col gap-3 self-start rounded-xl border border-border p-4" data-testid="history-side">
            <h2 id="hist-side-title" class="text-sm font-semibold">{i18n.t("history-side-title")}</h2>
            <Transition fallback=|| view! { <Skeleton class="h-24 w-full"/> }>
                {move || h.first.get().and_then(|(href, r)| r.ok().map(|f| (href, f))).map(|(href, first)| {
                    let (rows, _) = h.shown(&href, &first);
                    let i = use_i18n();
                    let c = side_counts(&rows);
                    let range = c.range.map(|(a, b)| i.t_args("history-side-range", &[("from", i.date(&a.iso()).into()), ("to", i.date(&b.iso()).into())]));
                    let lines: Vec<(String, u64)> = [
                        ("history-side-rows", c.rows),
                        ("history-side-finished", c.finished),
                        ("history-side-episodes", c.episodes),
                        ("history-side-sessions", c.sessions),
                        ("history-side-shared", c.shared),
                    ]
                    .into_iter()
                    .map(|(k, n)| (i.t(k), n))
                    .chain(c.groups.iter().map(|(g, n)| (i.t(&format!("kind-group-{g}")), *n)))
                    .collect();
                    view! {
                        {range.map(|r| view! { <p class="text-xs text-muted-foreground">{r}</p> })}
                        <dl class="grid grid-cols-[1fr_auto] gap-x-3 gap-y-1 text-sm">
                            {lines.into_iter().map(|(label, n)| view! {
                                <dt class="text-muted-foreground">{label}</dt>
                                <dd class="text-right font-semibold tabular-nums">{crate::home::count(&use_i18n(), n)}</dd>
                            }).collect_view()}
                        </dl>
                    }
                })}
            </Transition>
            <crate::stats_extras::SideCompare/>
            <a href="/stats" class="text-sm font-medium text-primary underline underline-offset-2">{i18n.t("home-all-stats")}</a>
        </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(at: &str, day: &str, time: &str) -> ActivityRowDto {
        let mut r: ActivityRowDto = serde_json::from_value(serde_json::json!({
            "at": at, "row_type": "session", "item_id": "i", "title": "T", "kind": "movie",
            "group": "watch", "participants": [], "shared": false, "undoable": false
        }))
        .unwrap();
        r.local_day = day.into();
        r.local_time = time.into();
        r
    }

    /// UX3-3: the stream is rendered afresh for a new answer or when the address moves away
    /// from it, never for the same answer again (Load more, filters panel, side counts).
    #[test]
    fn the_stream_key_follows_the_answer_and_its_address() {
        let page = |n: usize| -> Result<ActivityPageDto, ServerFnError> {
            Ok(ActivityPageDto {
                rows: (0..n)
                    .map(|_| row("2026-11-06T10:00:00Z", "2026-11-06", "10:00"))
                    .collect(),
                next: None,
            })
        };
        let a = stream_key("/history", false, &page(1));
        assert_eq!(a, stream_key("/history", false, &page(1)));
        // rows that change keep the rendering: the day groups follow them (DEC-490)
        assert_eq!(a, stream_key("/history", false, &page(2)));
        assert_ne!(a, stream_key("/history", false, &page(0)));
        assert_ne!(a, stream_key("/history", true, &page(1)));
        assert_ne!(a, stream_key("/history?group=watch", false, &page(1)));
        let failed: Result<ActivityPageDto, ServerFnError> = Err(ServerFnError::new("internal"));
        assert_ne!(a, stream_key("/history", false, &failed));
    }

    /// DEC-490: the pages of "Show more" hang on the first answer they continue; a refetch
    /// that brings the loaded range in its own answer leaves them out, and the next refresh
    /// fetches as many rows as are on screen.
    #[test]
    fn show_more_pages_belong_to_the_answer_they_continue() {
        let rows = |n: usize| -> Vec<ActivityRowDto> {
            (0..n)
                .map(|_| row("2026-11-06T10:00:00Z", "2026-11-06", "10:00"))
                .collect()
        };
        let first = ActivityPageDto {
            rows: rows(30),
            next: Some("c30".into()),
        };
        let more = More {
            href: "/history".into(),
            after: Some("c30".into()),
            rows: rows(30),
            next: Some("c60".into()),
        };
        let (shown, next) = shown_rows(&more, "/history", &first);
        assert_eq!((shown.len(), next.as_deref()), (60, Some("c60")));
        // another address, or the refetched range (its own `next`): the first answer alone
        let (shown, next) = shown_rows(&more, "/history?group=watch", &first);
        assert_eq!((shown.len(), next.as_deref()), (30, Some("c30")));
        let refetched = ActivityPageDto {
            rows: rows(60),
            next: Some("c60b".into()),
        };
        let (shown, next) = shown_rows(&more, "/history", &refetched);
        assert_eq!((shown.len(), next.as_deref()), (60, Some("c60b")));
        // a first page without a next page has nothing to continue
        let last = ActivityPageDto {
            rows: rows(5),
            next: None,
        };
        let none = More {
            after: None,
            ..more
        };
        assert_eq!(shown_rows(&none, "/history", &last).0.len(), 5);
    }

    /// DEC-306: with the viewer's zone known the times carry no "UTC" marker any more.
    #[test]
    fn times_are_local_without_a_utc_marker() {
        let i = I18n::new("en", std::collections::BTreeMap::new());
        let late = row("2026-11-06T23:30:00Z", "2026-11-07", "00:30");
        assert_eq!(clock(&i, &late).as_deref(), Some("12:30 AM"));
        assert_eq!(dated_text(&i, &late), "Nov 7, 2026, 12:30 AM");
        // an answer without the local fields still says which zone it is
        let old = row("2026-11-06T23:30:00Z", "", "");
        assert_eq!(clock(&i, &old).as_deref(), Some("11:30 PM UTC"));
        let de = I18n::new("de", std::collections::BTreeMap::new());
        assert_eq!(dated_text(&de, &late), "07.11.2026, 00:30");
    }

    fn play_row(people: serde_json::Value, play: serde_json::Value) -> ActivityRowDto {
        let mut r = row("2026-11-06T10:00:00Z", "2026-11-06", "10:00");
        r.participants = serde_json::from_value(people).unwrap();
        r.play = serde_json::from_value(play).unwrap();
        r
    }

    /// A board game's row reads as a play; a video game's stays a session.
    #[test]
    fn a_board_game_row_says_play_logged() {
        let en = I18n::new("en", std::collections::BTreeMap::new());
        let de = I18n::new("de", std::collections::BTreeMap::new());
        let of = |kind: &str| {
            let mut r = row("2026-11-06T10:00:00Z", "2026-11-06", "10:00");
            r.kind = kind.into();
            r
        };
        assert_eq!(what(&en, &of("board_game")), "Play logged");
        assert_eq!(what(&de, &of("board_game")), "Partie eingetragen");
        assert_eq!(what(&en, &of("video_game")), "Session logged");
        assert_eq!(what(&de, &of("video_game")), "Sitzung eingetragen");
        // the row of a session history event carries the progress `session`: the same words
        let event = |kind: &str| {
            let mut r = of(kind);
            r.row_type = "progress".into();
            r.progress = Some(serde_json::json!({"type": "session"}));
            r
        };
        assert_eq!(what(&en, &event("board_game")), "Play logged");
        assert_eq!(what(&de, &event("board_game")), "Partie eingetragen");
        assert_eq!(what(&en, &event("video_game")), "Session logged");
        // a row with another progress says the progress, whatever the kind
        let mut done = of("board_game");
        done.progress = Some(serde_json::json!({"type": "finished"}));
        assert_eq!(
            what(&en, &done),
            progress_label(&en, done.progress.as_ref().unwrap())
        );
    }

    /// OT-11: a play's row says who won, the scores and how long it took (it said only
    /// "Session logged").
    #[test]
    fn a_play_row_says_who_won_the_scores_and_how_long() {
        use serde_json::json;
        let en = I18n::new("en", std::collections::BTreeMap::new());
        let de = I18n::new("de", std::collections::BTreeMap::new());
        let me = |score: Option<f64>, winner: bool| json!({"companion_id": "a", "name": "Row Tester", "is_me": true, "score": score, "winner": winner});
        let ben = |score: Option<f64>, winner: bool| json!({"companion_id": "b", "name": "Ben", "is_me": false, "score": score, "winner": winner});
        let two = play_row(
            json!([me(Some(31.0), false), ben(Some(42.5), true)]),
            json!({"duration_minutes": 45, "coop": null}),
        );
        assert_eq!(
            play_line(&en, &two).as_deref(),
            Some("Ben won - Scores: You 31, Ben 42.5 - 45 min")
        );
        assert_eq!(
            play_line(&de, &two).as_deref(),
            Some("Gewonnen: Ben - Punkte: Du 31, Ben 42,5 - 45 Min.")
        );
        // alone: the score without a name
        let solo = play_row(
            json!([me(Some(12.0), true)]),
            json!({"duration_minutes": null, "coop": null}),
        );
        assert_eq!(
            play_line(&en, &solo).as_deref(),
            Some("You won - Score: 12")
        );
        assert_eq!(
            play_line(&de, &solo).as_deref(),
            Some("Du hast gewonnen - Punkte: 12")
        );
        // a shared win names everyone; a co-op game has one outcome for all
        let tie = play_row(
            json!([me(None, true), ben(None, true)]),
            json!({"duration_minutes": null, "coop": null}),
        );
        assert_eq!(play_line(&en, &tie).as_deref(), Some("You, Ben won"));
        // German: a wording that takes the nominative "Du" (OT1A-4)
        assert_eq!(play_line(&de, &tie).as_deref(), Some("Gewonnen: Du, Ben"));
        let coop = |outcome: &str| {
            play_row(
                json!([me(None, false), ben(None, false)]),
                json!({"duration_minutes": 90, "coop": outcome}),
            )
        };
        assert_eq!(
            play_line(&en, &coop("won")).as_deref(),
            Some("Won together - 90 min")
        );
        assert_eq!(
            play_line(&de, &coop("lost")).as_deref(),
            Some("Gemeinsam verloren - 90 Min.")
        );
        // OT-18: a co-op play without a result is neither won nor lost
        assert_eq!(
            play_line(&en, &coop("none")).as_deref(),
            Some("Played together - 90 min")
        );
        assert_eq!(
            play_line(&de, &coop("none")).as_deref(),
            Some("Gemeinsam gespielt - 90 Min.")
        );
        // nothing known beyond the people: no second line
        let bare = play_row(
            json!([me(None, false), ben(None, false)]),
            json!({"duration_minutes": null, "coop": null}),
        );
        assert_eq!(play_line(&en, &bare), None);
        // a row without a session, and an answer of a server that does not send the fields
        assert_eq!(play_line(&en, &row("2026-11-06T10:00:00Z", "", "")), None);
        let old: ActivityRowDto = serde_json::from_value(json!({
            "at": "2026-11-06T10:00:00Z", "row_type": "session", "item_id": "i", "title": "T",
            "kind": "board_game", "group": "play", "shared": false, "undoable": false,
            "participants": [{"companion_id": "b", "name": "Ben", "is_me": false}]
        }))
        .unwrap();
        assert_eq!(play_line(&en, &old), None);
    }
}
