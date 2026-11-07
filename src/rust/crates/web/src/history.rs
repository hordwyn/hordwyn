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
use crate::events::use_events;
use crate::history_state::{Day, HistoryState, day_heading, group_by_day, side_counts};
use crate::i18n::{I18n, kind_label, t, use_i18n};
use crate::next_step::{StepCtx, undo_event};
use crate::pages::{PageTitle, bad_query, error_text, fresh, go, needs_sign_in};
use crate::widgets::progress_label;

type Res<T> = Resource<Result<T, ServerFnError>>;

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

/// What happened, in words: the progress, else a logged session.
fn what(i: &I18n, row: &ActivityRowDto) -> String {
    row.progress
        .as_ref()
        .map_or_else(|| i.t("history-session"), |p| progress_label(i, p))
}

/// The `HH:MM` (UTC) of a timestamp.
fn clock(i: &I18n, at: &str) -> Option<String> {
    let (h, m) = (at.get(11..13)?.parse().ok()?, at.get(14..16)?.parse().ok()?);
    Some(i.t_args("history-time-utc", &[("time", i.time(h, m).into())]))
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
        Some(i18n.datetime(&row.at))
    } else {
        clock(&i18n, &row.at)
    };
    let kind = kind_label(&row.kind);
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
            let label = i18n.t_args("history-undo-named", &[("title", row.title.clone().into())]);
            let ctx = ctx.clone();
            view! {
                <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm attr:aria-label=label
                    on:click=move |_| undo_event(&ctx, id.clone(), on_change)>
                    {t("library-undo")}
                </Button>
            }
        });
    let copy = row.session_id.clone().filter(|_| row.shared).map(|id| {
        let label = i18n.t_args("history-copy-named", &[("title", row.title.clone().into())]);
        let ctx = ctx.clone();
        view! {
            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=label
                on:click=move |_| copy_to_log(&ctx, id.clone(), on_change)>
                {t("sessions-claim")}
            </Button>
        }
    });
    let look = if row.shared {
        "flex flex-wrap items-start gap-3 rounded-md border border-info/40 bg-info-soft px-3 py-2"
    } else {
        "flex flex-wrap items-start gap-3 rounded-md border border-border px-3 py-2"
    };
    view! {
        <li class=look data-testid="activity-row">
            <span class="mt-0.5 text-muted-foreground" title=kind.clone()>
                <Icon name=crate::library::kind_icon(&row.kind)/>
            </span>
            <span class="min-w-0 flex-1">
                <span class="block">
                    <a href=format!("/items/{}", row.item_id) class="font-medium underline-offset-2 hover:underline">{row.title.clone()}</a>
                    " - " {what(&i18n, &row)}
                    {with.map(|w| view! { " " <span>{w}</span> })}
                </span>
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

fn copy_to_log(ctx: &StepCtx, session_id: String, on_change: Option<Callback<()>>) {
    let (done, failed) = (ctx.i18n.t("sessions-claimed"), ctx.i18n.t("error-generic"));
    let toasts = ctx.toasts;
    leptos::task::spawn_local(async move {
        match api::claim(session_id).await {
            Ok(_) => toasts.push(done, None, ToastVariant::Default),
            Err(_) => toasts.push(failed, None, ToastVariant::Destructive),
        }
        if let Some(cb) = on_change {
            cb.run(());
        }
    });
}

/// Pages after the first, loaded by "Load more" for one state (`href`).
#[derive(Clone, Default, PartialEq)]
struct More {
    href: String,
    rows: Vec<ActivityRowDto>,
    next: Option<String>,
}

#[derive(Clone, Copy)]
struct Hist {
    state: Memo<HistoryState>,
    go: Callback<HistoryState>,
    first: Res<ActivityPageDto>,
    more: RwSignal<More>,
    /// "Show more" is loading; why it failed.
    loading: RwSignal<bool>,
    more_error: RwSignal<Option<String>>,
    refresh: Callback<()>,
}

impl Hist {
    /// Rows on screen and the cursor for the next page.
    fn shown(&self, first: &ActivityPageDto) -> (Vec<ActivityRowDto>, Option<String>) {
        let href = self.state.with(HistoryState::href);
        let mut rows = first.rows.clone();
        let more = self.more.get();
        if more.href == href {
            rows.extend(more.rows);
            (rows, more.next)
        } else {
            (rows, first.next.clone())
        }
    }

    fn load_more(&self, next: String) {
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
                    if m.href != href {
                        *m = More {
                            href: href.clone(),
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
    let first = Resource::new(move || state.get().query_dto(None), api::activity_feed);
    let more = RwSignal::new(More::default());
    let refresh = Callback::new(move |()| {
        more.set(More::default());
        first.refetch();
    });
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && topic == "entity.updated"
        {
            refresh.run(());
        }
    });
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
        <PageTitle text=i18n.t("history-title")/>
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
                        options.extend(k.groups.iter().map(|g| SegmentedOption::new(g.id.clone(), i.t(&g.label_key))));
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
    let renders = StoredValue::new(0u32);
    let each = move || {
        h.first.get().map(|r| {
            let shown = r.as_ref().ok().map(|f| h.shown(f));
            let filtered = h.state.with(|s| *s != HistoryState::default());
            (fresh(renders), r, shown, filtered)
        })
    };
    view! {
        <section aria-labelledby="hist-stream-title" class="min-w-0">
            <h2 id="hist-stream-title" class="sr-only">{i18n.t("history-stream")}</h2>
            <Transition fallback=|| view! { <RowSkeletons/> }>
                <For each=each key=|(k, _, _, _)| *k children=move |(_, r, shown, filtered)| match (r, shown) {
                    (Err(e), _) if needs_sign_in(&e) => {
                        go("/sign-in?next=/history");
                        ().into_any()
                    }
                    (Err(e), _) if bad_query(&e) => view! { <NoRows h=h filtered=true/> }.into_any(),
                    (Err(e), _) => view! {
                        <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=t("history-error")
                            actions=move || view! { <Button variant=ButtonVariant::Outline size=ButtonSize::Sm on:click=move |_| h.refresh.run(())>{t("library-retry")}</Button> }>
                            {error_text(&e)}
                        </Alert>
                    }.into_any(),
                    (Ok(_), Some((rows, _))) if rows.is_empty() => view! { <NoRows h=h filtered=filtered/> }.into_any(),
                    (Ok(_), shown) => {
                        let (rows, next) = shown.unwrap_or_default();
                        view! { <Days h=h rows=rows next=next/> }.into_any()
                    }
                }/>
            </Transition>
        </section>
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

/// The day groups and "Show more".
#[component]
fn Days(h: Hist, rows: Vec<ActivityRowDto>, next: Option<String>) -> impl IntoView {
    let groups = group_by_day(rows, today());
    view! {
        <div class="flex flex-col gap-6">
            {groups.into_iter().map(|g| {
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
            }).collect_view()}
            {next.map(|n| view! {
                <div class="flex flex-col items-center gap-2">
                    <Button variant=ButtonVariant::Outline loading=h.loading on:click=move |_| h.load_more(n.clone())>{t("library-more")}</Button>
                    {move || h.more_error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
                </div>
            })}
        </div>
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
            <Suspense fallback=|| view! { <Skeleton class="h-24 w-full"/> }>
                {move || h.first.get().and_then(Result::ok).map(|first| {
                    let (rows, _) = h.shown(&first);
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
            </Suspense>
            <a href="/stats" class="text-sm font-medium underline underline-offset-2">{i18n.t("home-all-stats")}</a>
        </aside>
    }
}
