//! Home (design.md 6.1; addendum-1 1.3; owner ruling DEC-171) and the Stats page. Home shows
//! the greeting, Continue (in-progress entries with their next step), Up next (planned),
//! the last five activity rows with a link to History, and the all-time stats led by time
//! (OT-17, DEC-542: the hero tile and four tiles of `time_tiles`, then the library as one
//! line per group). Stats shows the same block on top, then activity over time per group (a
//! tab each, plus All) by month or year, hours first, each chart with its table, and the
//! entries by status behind a disclosure.

use contracts::{HomeDto, LibraryRowDto, StatsSeriesDto, StatsTotalsDto, TotalsDto, UpNextDto};
use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_query_map};
use ui::{
    Alert, AlertRole, AlertVariant, Button, ButtonLink, ButtonSize, ButtonVariant, Disclosure,
    DisclosureMode, EmptyState, Icon, IconName, ProgressBar, ProgressSize, Segmented,
    SegmentedMode, SegmentedOption, Skeleton, Table, TableBody, TableCaption, TableCell, TableHead,
    TableHeader, TableRow,
};

use crate::api;
use crate::charts::{
    Bar, BarList, Column, ColumnChart, Measure, bucket_label, fill_months, fill_years, measures_led,
};
use crate::history::ActivityRow;
use crate::i18n::{I18n, kind_label, t, use_i18n};
use crate::next_step::{NextStepButton, percent, progress_text};
use crate::pages::{PageTitle, error_text, fresh, go, needs_sign_in};
use crate::widgets::{CoverOf, StableCover, keyed};

/// A count in the locale's digits.
#[allow(clippy::cast_precision_loss)] // counts stay far below 2^52
pub(crate) fn count(i: &I18n, n: u64) -> String {
    i.number(n as f64)
}

/// Name of a kind group (`kind-group-<id>`).
pub(crate) fn group_label(i: &I18n, group: &str) -> String {
    i.t(&format!("kind-group-{group}"))
}

#[component]
pub fn Home() -> impl IntoView {
    // owner bug: no "Create account" once signed in (the shell config knows the session)
    let signed_in = expect_context::<crate::app::ClientConfig>().signed_in;
    if signed_in {
        return view! { <SignedInHome/> }.into_any();
    }
    view! {
        <section class="mx-auto max-w-2xl py-16 text-center">
            <h1 class="text-3xl font-semibold tracking-tight">{t("home-title")}</h1>
            <p class="mt-4 text-muted-foreground">{t("home-lead")}</p>
            <div class="mt-8 flex justify-center gap-4">
                <ButtonLink href="/library">{t("nav-library")}</ButtonLink>
                <ButtonLink href="/sign-up" variant=ButtonVariant::Outline>{t("nav-sign-up")}</ButtonLink>
            </div>
        </section>
    }
    .into_any()
}

#[component]
fn SignedInHome() -> impl IntoView {
    let cfg = expect_context::<crate::app::ClientConfig>();
    let i18n = use_i18n();
    let data = Resource::new(|| (), |()| api::home());
    let shown = StoredValue::new(Shown::default());
    let refetch = Callback::new(move |()| data.refetch());
    // paced: an import announces every item it writes (OT-21); the own refreshes tell the
    // same pace (LR-5)
    let own = crate::events::refresh_on_updates(crate::offline::refresh_when_online(refetch));
    let refresh = own.wrap(refetch);
    let title = i18n.t_args(
        "home-greeting",
        &[("name", cfg.display_name.clone().into())],
    );
    let product = cfg.product_name.clone();
    let tab = i18n.t("nav-home");
    continue_keys();
    view! {
        <leptos_meta::Title text=format!("{tab} - {product}")/>
        <h1 class="mb-6 text-2xl font-semibold tracking-tight">{title}</h1>
        {cfg.is_admin.then(|| view! { <crate::nudge::AdminNudge/> })}
        <Transition fallback=|| view! { <HomeSkeleton/> }>
            <For each=move || data.get().and_then(|r| shown.try_update_value(|s| s.next(r))) key=|(k, _)| *k
                children=move |(_, r)| match r {
                Err(e) if needs_sign_in(&e) => {
                    go("/sign-in?next=/");
                    ().into_any()
                }
                Err(e) => view! {
                    <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=t("home-error")
                        actions=move || view! { <Button variant=ButtonVariant::Outline size=ButtonSize::Sm on:click=move |_| refresh.run(())>{t("library-retry")}</Button> }>
                        {error_text(&e)}
                    </Alert>
                }.into_any(),
                Ok(home) if home.library_total == 0 => view! { <HomeEmpty/> }.into_any(),
                Ok(first) => {
                    let home = latest_home(data, first);
                    view! { <HomeSections home=home refresh=refresh/> }.into_any()
                }
            }/>
        </Transition>
    }
}

/// The latest answer with rows, for the sections on screen (they outlive refetches, OT-21). In
/// the browser it follows the resource and keeps the last one while an error or an empty
/// library is the answer (the view is replaced then, or the answer never reached the server);
/// on the server nothing refetches, so it is the first answer (a second resource read under
/// the Transition made the server panic now and then, DEC-398).
fn latest_home(data: Resource<Result<HomeDto, ServerFnError>>, first: HomeDto) -> Memo<HomeDto> {
    #[cfg(feature = "hydrate")]
    let home = Memo::new(move |prev: Option<&HomeDto>| match data.get() {
        Some(Ok(h)) if h.library_total > 0 => h,
        _ => prev.cloned().unwrap_or_else(|| first.clone()),
    });
    #[cfg(not(feature = "hydrate"))]
    let home = {
        let _ = data;
        Memo::new(move |_| first.clone())
    };
    home
}

/// What Home shows of its answers. The sections stay on screen from one answer with rows to
/// the next: their cards are keyed, so a card whose row did not change keeps its focus, its
/// open menu and its cover, and a changed row renders its acting parts afresh (handlers with
/// the new data, UX1-2; OT-21: every answer used to render all of Home afresh, and during an
/// import every cover was loaded again several times a second). An error, the empty library
/// and the way back from them render afresh. A refetch that never reached the server (offline,
/// the connection dropped) keeps the last answer instead of "Home could not be loaded" (DEC-486).
#[derive(Default)]
struct Shown {
    /// The answer on screen and its render key.
    last: Option<(Result<HomeDto, ServerFnError>, u32)>,
    /// The last answer that loaded.
    good: Option<HomeDto>,
}

impl Shown {
    /// The render key and the answer to show for the resource's next answer.
    fn next(
        &mut self,
        answer: Result<HomeDto, ServerFnError>,
    ) -> (u32, Result<HomeDto, ServerFnError>) {
        let answer = match (answer, &self.good) {
            (Err(e), Some(good)) if crate::offline::network_error(&e) => Ok(good.clone()),
            (other, _) => other,
        };
        if let Ok(home) = &answer {
            self.good = Some(home.clone());
        }
        let rows = |a: &Result<HomeDto, ServerFnError>| matches!(a, Ok(h) if h.library_total > 0);
        let key = match &self.last {
            Some((same, key)) if *same == answer => *key,
            Some((was, key)) if rows(was) && rows(&answer) => *key,
            Some((_, key)) => key.wrapping_add(1),
            None => 0,
        };
        self.last = Some((answer.clone(), key));
        (key, answer)
    }
}

#[component]
fn HomeEmpty() -> impl IntoView {
    let palette = expect_context::<crate::palette::PaletteOpen>().0;
    view! {
        <EmptyState title=t("home-empty") icon=IconName::Library body=t("home-empty-body") level=2
            primary=move || view! { <Button on:click=move |_| palette.set(true)><Icon name=IconName::Search/> {t("library-find")}</Button> }/>
    }
}

/// Skeletons shaped like the sections (design.md 6: never a page-wide spinner).
#[component]
fn HomeSkeleton() -> impl IntoView {
    view! {
        <p class="sr-only" role="status">{t("home-loading")}</p>
        <div class="flex flex-col gap-8">
            <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                {(0..3).map(|_| view! { <Skeleton class="h-24 w-full"/> }).collect_view()}
            </div>
            <div class="flex gap-4">
                {(0..5).map(|_| view! { <Skeleton class="aspect-[2/3] w-28"/> }).collect_view()}
            </div>
            <Skeleton class="h-40 w-full"/>
        </div>
    }
}

/// A section with a heading and an optional link on the right.
#[component]
fn Section(
    #[prop(into)] id: String,
    #[prop(into)] title: String,
    #[prop(optional)] link: Option<(&'static str, String)>,
    children: Children,
) -> impl IntoView {
    let heading = format!("{id}-title");
    let labelled = heading.clone();
    view! {
        <section aria-labelledby=labelled class="flex flex-col gap-3">
            <div class="flex items-baseline justify-between gap-3">
                <h2 id=heading class="text-lg font-semibold">{title}</h2>
                {link.map(|(href, text)| view! { <a href=href class="text-sm font-medium text-primary underline underline-offset-2">{text}</a> })}
            </div>
            {children()}
        </section>
    }
}

#[component]
fn HomeSections(home: Memo<HomeDto>, refresh: Callback<()>) -> impl IntoView {
    let continue_rows = Memo::new(move |_| home.with(|h| h.continue_rows.clone()));
    let up_next = Memo::new(move |_| home.with(|h| h.up_next.clone()));
    let activity = Memo::new(move |_| home.with(|h| h.activity.rows.clone()));
    let stats = Memo::new(move |_| home.with(|h| h.stats.clone()));
    let any_continue = move || continue_rows.with(|r| !r.is_empty());
    let any_up_next = move || up_next.with(|r| !r.is_empty());
    let any_activity = move || activity.with(|r| !r.is_empty());
    // rows and stats that changed are rendered afresh, not rebuilt in place (UX1-2)
    let renders = StoredValue::new(0u32);
    view! {
        <div class="flex flex-col gap-10">
            <Show when=any_continue>
                <Section id="hm-continue" title=t("home-continue") link=("/library?status=in_progress", t("home-all-in-progress"))>
                    <ul class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
                        {keyed(continue_rows.into(), |r: &LibraryRowDto| r.entry.id.clone(), move |row| view! { <ContinueCard row=row on_done=refresh/> })}
                    </ul>
                </Section>
            </Show>
            <Show when=any_up_next>
                <Section id="hm-up-next" title=t("home-up-next") link=("/library?status=planned", t("home-all-planned"))>
                    <ul class="-mx-4 flex snap-x snap-mandatory gap-4 overflow-x-auto px-4 pb-2">
                        {keyed(up_next.into(), |u: &UpNextDto| u.row.entry.id.clone(), |up| view! { <UpNextCard up=up/> })}
                    </ul>
                </Section>
            </Show>
            <Show when=any_activity>
                <Section id="hm-activity" title=t("home-activity") link=("/history", t("home-all-history"))>
                    <ul class="flex flex-col gap-2">
                        <For each=move || Some((fresh(renders), activity.get())) key=|(k, _)| *k
                            children=move |(_, rows)| rows.into_iter().map(|row| view! { <ActivityRow row=row dated=true on_change=refresh/> }).collect_view()/>
                    </ul>
                </Section>
            </Show>
            <Section id="hm-stats" title=t("home-stats") link=("/stats", t("home-all-stats"))>
                <For each=move || Some((fresh(renders), stats.get())) key=|(k, _)| *k
                    children=move |(_, stats)| view! { <HomeStats stats=stats/> }/>
            </Section>
        </div>
    }
}

/// `J`/`K` (design.md 6.1): focus the next or previous Continue card's next step (its title
/// when the step is a label only), so Enter runs it; stops at the ends.
fn continue_keys() {
    crate::keyboard::page_keys(|_, key| {
        let by = match key {
            "j" => 1,
            "k" => -1,
            _ => return false,
        };
        move_continue(by)
    });
}

#[cfg(feature = "hydrate")]
fn move_continue(by: isize) -> bool {
    use wasm_bindgen::JsCast;
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return false;
    };
    let Ok(cards) = doc.query_selector_all("[data-continue-card]") else {
        return false;
    };
    let cards: Vec<web_sys::Element> = (0..cards.length())
        .filter_map(|i| cards.item(i))
        .filter_map(|n| n.dyn_into().ok())
        .collect();
    if cards.is_empty() {
        return false;
    }
    let focused = crate::keyboard::focused();
    let at = cards.iter().position(|c| {
        focused
            .as_ref()
            .is_some_and(|f| c.contains(Some(f.as_ref())))
    });
    let last = cards.len() - 1;
    let to = match at {
        None => 0,
        Some(i) => i.saturating_add_signed(by).min(last),
    };
    let target = cards[to]
        .query_selector("[data-next-step]")
        .ok()
        .flatten()
        .or_else(|| cards[to].query_selector("a[data-title]").ok().flatten())
        .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok());
    target.is_some_and(|el| el.focus().is_ok())
}

#[cfg(not(feature = "hydrate"))]
fn move_continue(_: isize) -> bool {
    false
}

/// An in-progress entry: small cover, title, progress in words and as a bar, one button. The
/// card and its cover stay while the entry is listed; the rest is rendered afresh when the row
/// changes (OT-21).
#[component]
fn ContinueCard(row: Memo<Option<LibraryRowDto>>, on_done: Callback<()>) -> impl IntoView {
    let href = row.with_untracked(|r| {
        r.as_ref()
            .map(|r| format!("/items/{}", r.item.id))
            .unwrap_or_default()
    });
    let cover = Signal::derive(move || row.with(|r| r.as_ref().map(|r| CoverOf::of(&r.item))));
    let renders = StoredValue::new(0u32);
    view! {
        <li class="flex gap-3 rounded-xl border border-border p-3" data-testid="continue-card" data-continue-card="">
            <a href=href class="w-14 shrink-0 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring" tabindex="-1" aria-hidden="true">
                <StableCover of=cover sizes="56px"/>
            </a>
            <For each=move || row.get().map(|r| (fresh(renders), r)) key=|(k, _)| *k
                children=move |(_, r)| view! { <ContinueBody row=r on_done=on_done/> }/>
        </li>
    }
}

/// The words and the button of a [`ContinueCard`].
#[component]
fn ContinueBody(row: LibraryRowDto, on_done: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    let item = row.item.clone();
    let progress = progress_text(&i18n, &row.summary, &row.next_step);
    let bar = row.summary.fraction.zip(progress.clone()).map(|(f, text)| {
        let value = Signal::derive(move || percent(f));
        view! { <ProgressBar label=t("library-progress") value=value value_text=text size=ProgressSize::Thin/> }
    });
    let meta = match &progress {
        Some(p) => format!("{} - {p}", kind_label(&item.kind)),
        None => kind_label(&item.kind),
    };
    view! {
        <div class="flex min-w-0 flex-1 flex-col gap-1.5">
            <a href=format!("/items/{}", item.id) data-title="" class="truncate font-medium hover:underline">{crate::i18n::title_text(&item.title)}</a>
            <span class="text-xs text-muted-foreground">{meta}</span>
            {bar}
            <div class="mt-auto">
                <NextStepButton item_id=item.id.clone() title=crate::i18n::title_text(&item.title) step=row.next_step.clone() on_done=on_done/>
            </div>
        </div>
    }
}

/// A planned entry. Nothing on it acts, so its words follow the row and the card stays.
#[component]
fn UpNextCard(up: Memo<Option<UpNextDto>>) -> impl IntoView {
    let href = up.with_untracked(|u| {
        u.as_ref()
            .map(|u| format!("/items/{}", u.row.item.id))
            .unwrap_or_default()
    });
    let cover = Signal::derive(move || up.with(|u| u.as_ref().map(|u| CoverOf::of(&u.row.item))));
    let title = move || {
        up.with(|u| {
            u.as_ref()
                .map(|u| crate::i18n::title_text(&u.row.item.title))
                .unwrap_or_default()
        })
    };
    let meta = move || {
        up.with(|u| {
            u.as_ref().map(|u| {
                let reason = match u.reason.as_str() {
                    "next_in_series" => t("home-reason-next-in-series"),
                    _ => t("home-reason-planned"),
                };
                format!("{} - {reason}", kind_label(&u.row.item.kind))
            })
        })
    };
    view! {
        <li class="w-28 shrink-0 snap-start sm:w-32" data-testid="up-next-card">
            <a href=href class="group flex flex-col gap-1.5 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <StableCover of=cover sizes="128px"/>
                <span class="line-clamp-2 text-sm font-medium group-hover:underline">{title}</span>
            </a>
            <span class="text-xs text-muted-foreground">{meta}</span>
        </li>
    }
}

/// The status bars of one group's totals.
fn status_bars(i: &I18n, g: &TotalsDto) -> Vec<Bar> {
    [
        ("status-planned", g.planned),
        ("status-in_progress", g.in_progress),
        ("status-completed", g.completed),
        ("status-paused", g.paused),
        ("status-dropped", g.dropped),
    ]
    .into_iter()
    .map(|(k, v)| Bar {
        label: i.t(k),
        value: v,
        text: count(i, v),
    })
    .collect()
}

/// "Watch: 1 planned, 1 in progress, 0 completed, 0 paused, 0 dropped", every word from the
/// catalogue (German keeps its own forms).
fn totals_summary(i: &I18n, g: &TotalsDto) -> String {
    let parts: Vec<String> = [
        ("stats-totals-part-planned", g.planned),
        ("stats-totals-part-in_progress", g.in_progress),
        ("stats-totals-part-completed", g.completed),
        ("stats-totals-part-paused", g.paused),
        ("stats-totals-part-dropped", g.dropped),
    ]
    .into_iter()
    .map(|(k, v)| i.t_args(k, &[("count", count(i, v).into())]))
    .collect();
    i.t_args(
        "stats-totals-summary",
        &[
            ("group", group_label(i, &g.group).into()),
            ("parts", parts.join(", ").into()),
        ],
    )
}

/// "Watch: 3 entries, 1 in progress, 2 planned": a group's library in one line, the
/// entries first, then the statuses that have any (OT-17: the status counts are text on
/// Home and sit below the time tiles).
fn library_line(i: &I18n, g: &TotalsDto) -> String {
    let mut parts = vec![i.t_args("stats-totals-part-entries", &[("count", g.entries.into())])];
    parts.extend(
        [
            ("stats-totals-part-completed", g.completed),
            ("stats-totals-part-in_progress", g.in_progress),
            ("stats-totals-part-planned", g.planned),
            ("stats-totals-part-paused", g.paused),
            ("stats-totals-part-dropped", g.dropped),
        ]
        .into_iter()
        .filter(|(_, v)| *v > 0)
        .map(|(k, v)| i.t_args(k, &[("count", count(i, v).into())])),
    );
    i.t_args(
        "stats-totals-summary",
        &[
            ("group", group_label(i, &g.group).into()),
            ("parts", parts.join(", ").into()),
        ],
    )
}

/// Home's stats block (OT-17, DEC-542): the time tiles lead, the library follows as one
/// line per group with entries. No chart here; the status bars are on the Stats page.
#[component]
fn HomeStats(stats: StatsTotalsDto) -> impl IntoView {
    let i18n = use_i18n();
    let lines: Vec<(String, String)> = stats
        .groups
        .iter()
        .filter(|g| g.entries > 0)
        .map(|g| (g.group.clone(), library_line(&i18n, g)))
        .collect();
    view! {
        <div class="flex flex-col gap-4">
            <crate::time_tiles::TimeBanked stats=stats/>
            <div class="flex flex-col gap-1">
                <h3 class="text-sm font-semibold">{i18n.t("stats-library-heading")}</h3>
                <ul class="text-sm text-muted-foreground">
                    {lines.into_iter().map(|(group, line)| view! {
                        <li class="flex items-baseline gap-2" data-testid=format!("totals-line-{group}")>
                            <span aria-hidden="true" class=ui::cn(&["size-2 shrink-0 rounded-full", crate::tint::group_tint(&group).solid()])></span>
                            <span>{line}</span>
                        </li>
                    }).collect_view()}
                </ul>
            </div>
        </div>
    }
}

/// The status bars of a group's library behind a closed disclosure (OT-17: the status
/// counts moved below the time figures; addendum-1 D4), with the numbers as a line of text.
#[component]
fn LibraryByStatus(totals: TotalsDto) -> impl IntoView {
    let i18n = use_i18n();
    let summary = totals_summary(&i18n, &totals);
    let line = library_line(&i18n, &totals);
    view! {
        <Disclosure id="st-library" title=i18n.t("stats-library-by-status") mode=DisclosureMode::Native>
            <div class="flex flex-col gap-3">
                <BarList summary=summary bars=status_bars(&i18n, &totals) tint=crate::tint::chart_tint(&totals.group)/>
                <p class="text-sm text-muted-foreground" data-testid="st-library-line">{line}</p>
            </div>
        </Disclosure>
    }
}

/// The totals of the Stats tab's group (`all`: overall).
fn tab_totals(t: StatsTotalsDto, group: &str) -> Option<TotalsDto> {
    if group == "all" {
        Some(t.overall)
    } else {
        t.groups.into_iter().find(|x| x.group == group)
    }
}

/// The Stats page's URL state: `group` (`all` or a group id) and `bucket` (`month`, `year`).
fn stats_href(group: &str, bucket: &str) -> String {
    let mut q = Vec::new();
    if group != "all" {
        q.push(format!("group={}", crate::library_state::encode(group)));
    }
    if bucket == "year" {
        q.push("bucket=year".to_string());
    }
    if q.is_empty() {
        "/stats".into()
    } else {
        format!("/stats?{}", q.join("&"))
    }
}

/// Months shown in the month view (DEC-229).
const MONTHS: usize = 24;

/// The Stats page's state in the URL: the `group` and `bucket` of the query as memos, and
/// the signals of the two controls, kept in step with them both ways (a control navigates
/// without scrolling; Back and Forward set the controls).
type UrlState = (
    Memo<String>,
    Memo<String>,
    RwSignal<String>,
    RwSignal<String>,
);

fn stats_url_state() -> UrlState {
    let query = use_query_map();
    let group_q = Memo::new(move |_| {
        query
            .with(|q| q.get("group"))
            .filter(|g| !g.is_empty())
            .unwrap_or_else(|| "all".into())
    });
    let bucket_q = Memo::new(move |_| match query.with(|q| q.get("bucket")).as_deref() {
        Some("year") => "year".to_string(),
        _ => "month".to_string(),
    });
    let nav = StoredValue::new(use_navigate());
    let to = move |g: String, b: String| {
        let href = stats_href(&g, &b);
        nav.with_value(|n| {
            n(
                &href,
                NavigateOptions {
                    scroll: false,
                    ..NavigateOptions::default()
                },
            );
        });
    };
    let group = RwSignal::new(group_q.get_untracked());
    let bucket = RwSignal::new(bucket_q.get_untracked());
    Effect::new(move |_| {
        let (g, b) = (group_q.get(), bucket_q.get());
        if group.get_untracked() != g {
            group.set(g);
        }
        if bucket.get_untracked() != b {
            bucket.set(b);
        }
    });
    Effect::new(move |prev: Option<()>| {
        let (g, b) = (group.get(), bucket.get());
        if prev.is_some() && (g != group_q.get_untracked() || b != bucket_q.get_untracked()) {
            to(g, b);
        }
    });
    (group_q, bucket_q, group, bucket)
}

#[component]
pub fn Stats() -> impl IntoView {
    let i18n = use_i18n();
    let (group_q, bucket_q, group, bucket) = stats_url_state();
    let kinds = Resource::new(|| (), |()| api::kinds());
    let totals = Resource::new(|| (), |()| api::stats_totals());
    let series = Resource::new(
        move || (group_q.get(), bucket_q.get()),
        |(g, b)| api::stats_series(Some(g), Some(b)),
    );
    let buckets = vec![
        SegmentedOption::new("month", i18n.t("stats-by-month")),
        SegmentedOption::new("year", i18n.t("stats-by-year")),
    ];
    let labelled = move || format!("st-group-{}", group_q.get());
    // the plays of the Play tab's totals row: "Your results" counts the same (review OT2F-2)
    let row_plays = Signal::derive(move || {
        totals
            .get()
            .and_then(Result::ok)
            .and_then(|t| tab_totals(t, "play"))
            .map(|x| x.amounts.plays)
    });
    view! {
        <PageTitle text=i18n.t("stats-title")/>
        <Suspense fallback=|| view! { <Skeleton class="mb-6 h-48 w-full"/> }>
            {move || totals.get().and_then(Result::ok).map(|t| view! {
                <div class="mb-6"><crate::time_tiles::TimeBanked stats=t level=2/></div>
            })}
        </Suspense>
        <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div class="-mx-4 max-w-[calc(100%+2rem)] overflow-x-auto px-4">
                <Suspense fallback=|| view! { <Skeleton class="h-9 w-72"/> }>
                    {move || kinds.get().and_then(Result::ok).map(|k| {
                        let i = use_i18n();
                        let mut options = vec![SegmentedOption::new("all", i.t("kind-group-all")).controls("st-panel")];
                        options.extend(k.groups.iter().map(|g| crate::tint::group_option(&g.id, i.t(&g.label_key)).controls("st-panel")));
                        view! { <Segmented id="st-group" label=i.t("stats-groups") value=group options=options mode=SegmentedMode::Tabs/> }
                    })}
                </Suspense>
            </div>
            <Segmented id="st-bucket" label=i18n.t("stats-bucket") value=bucket options=buckets/>
        </div>
        <section id="st-panel" role="tabpanel" aria-labelledby=labelled class="flex flex-col gap-6">
            <Suspense fallback=|| view! { <Skeleton class="h-16 w-full"/> }>
                {move || totals.get().map(|r| match r {
                    Err(e) if needs_sign_in(&e) => {
                        go("/sign-in?next=/stats");
                        ().into_any()
                    }
                    Err(e) => view! { <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=t("stats-error")>{error_text(&e)}</Alert> }.into_any(),
                    Ok(t) => tab_totals(t, &group_q.get()).map(|x| view! { <TotalsLine totals=x/> }).into_any(),
                })}
            </Suspense>
            <crate::stats_extras::StatsCompare group=group_q plays=row_plays/>
            <Transition fallback=|| view! { <Skeleton class="h-48 w-full"/> }>
                {move || series.get().map(|r| match r {
                    Err(e) => view! {
                        <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=t("stats-error")
                            actions=move || view! { <Button variant=ButtonVariant::Outline size=ButtonSize::Sm on:click=move |_| series.refetch()>{t("library-retry")}</Button> }>
                            {error_text(&e)}
                        </Alert>
                    }.into_any(),
                    Ok(s) => {
                        // the charts lead as the totals row does: hours after the counts
                        // when the tab has no time at all (review OT2C-5)
                        let timed = totals.get().and_then(Result::ok)
                            .and_then(|t| tab_totals(t, &s.group))
                            .is_none_or(|x| x.amounts.minutes > 0);
                        view! { <SeriesCharts series=s timed=timed/> }.into_any()
                    }
                })}
            </Transition>
            <Suspense>
                {move || totals.get().and_then(Result::ok)
                    .and_then(|t| tab_totals(t, &group_q.get()))
                    .filter(|x| x.entries > 0)
                    .map(|x| view! { <LibraryByStatus totals=x/> })}
            </Suspense>
        </section>
        <div class="mt-8"><crate::stats_extras::Goals/></div>
    }
}

/// The group's all-time amounts in one row, time first (OT-17), or the counts first when
/// the group has no time at all (review OT2C-5). The entries by status are in
/// [`LibraryByStatus`], below the charts.
#[component]
fn TotalsLine(totals: TotalsDto) -> impl IntoView {
    let i18n = use_i18n();
    let items: Vec<(String, String)> = measures_led(&totals.group, totals.amounts.minutes > 0)
        .into_iter()
        .map(|m| {
            (
                i18n.t(m.label_key()),
                m.text(&i18n, m.value(&totals.amounts)),
            )
        })
        .collect();
    view! {
        <dl class="grid grid-cols-2 gap-3 sm:grid-cols-3" aria-label=i18n.t("stats-totals") data-testid="st-totals">
            {items.into_iter().map(|(label, n)| view! {
                <div class="rounded-xl border border-border p-3">
                    <dt class="text-xs text-muted-foreground">{label}</dt>
                    <dd class="text-xl font-semibold tabular-nums">{n}</dd>
                </div>
            }).collect_view()}
        </dl>
    }
}

/// One column chart per measure of the group, each with its table behind a disclosure.
/// `timed`: the group has time at all (else the hours chart comes last).
#[component]
fn SeriesCharts(series: StatsSeriesDto, timed: bool) -> impl IntoView {
    let i18n = use_i18n();
    let points = if series.bucket == "year" {
        fill_years(&series.points)
    } else {
        fill_months(&series.points, MONTHS)
    };
    if points.is_empty() {
        return view! { <EmptyState title=i18n.t("stats-empty") icon=IconName::History body=i18n.t("stats-empty-body") level=2/> }.into_any();
    }
    let group = group_label(&i18n, &series.group);
    let tint = crate::tint::chart_tint(&series.group);
    view! {
        <div class="grid gap-6 xl:grid-cols-2">
            {measures_led(&series.group, timed).into_iter().map(|m| view! {
                <MeasureChart measure=m points=points.clone() group=group.clone() bucket=series.bucket.clone() tint=tint/>
            }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn MeasureChart(
    measure: Measure,
    points: Vec<contracts::SeriesPointDto>,
    group: String,
    bucket: String,
    tint: ui::Tint,
) -> impl IntoView {
    let i18n = use_i18n();
    let name = i18n.t(measure.label_key());
    let id = format!(
        "st-{}",
        measure.label_key().trim_start_matches("stats-measure-")
    );
    let rows: Vec<(String, u64)> = points
        .iter()
        .map(|p| (bucket_label(&i18n, &p.bucket), measure.value(&p.amounts)))
        .collect();
    let total: u64 = rows.iter().map(|(_, v)| v).sum();
    let summary = chart_summary(&i18n, &name, group, &rows, measure);
    let columns: Vec<Column> = rows
        .iter()
        .map(|(label, v)| Column {
            label: label.clone(),
            value: *v,
            title: i18n.t_args(
                "chart-value",
                &[
                    ("label", label.clone().into()),
                    ("value", measure.counted(&i18n, *v).into()),
                ],
            ),
        })
        .collect();
    // the table shows what the chart shows: hours for time, the count otherwise
    let table: Vec<(String, String)> = rows
        .iter()
        .map(|(label, v)| (label.clone(), measure.text(&i18n, *v)))
        .collect();
    let head = i18n.t(if bucket == "year" {
        "stats-col-year"
    } else {
        "stats-col-month"
    });
    let (fig_label, cap_id) = (format!("{id}-title"), format!("{id}-title"));
    let caption_name = name.clone();
    view! {
        <figure class="flex flex-col gap-3 rounded-xl border border-border p-4" aria-labelledby=fig_label>
            <figcaption id=cap_id class="flex items-baseline justify-between gap-2">
                <span class="font-semibold">{caption_name}</span>
                <span class="text-sm text-muted-foreground tabular-nums">{measure.text(&i18n, total)}</span>
            </figcaption>
            <ColumnChart summary=summary columns=columns unit=measure.unit() tint=tint/>
            <MeasureTable id=format!("{id}-table") name=name head=head rows=table/>
        </figure>
    }
}

/// The chart's sentence: total, range and the peak bucket (an empty series has no peak:
/// "none yet", not "0 in total, most in <last month>").
fn chart_summary(
    i18n: &I18n,
    name: &str,
    group: String,
    rows: &[(String, u64)],
    measure: Measure,
) -> String {
    let total: u64 = rows.iter().map(|(_, v)| v).sum();
    let peak = rows.iter().max_by_key(|(_, v)| *v).cloned();
    i18n.t_args(
        if total == 0 {
            "stats-chart-summary-empty"
        } else {
            "stats-chart-summary"
        },
        &[
            ("measure", name.to_string().into()),
            ("group", group.into()),
            ("total", measure.text(i18n, total).into()),
            (
                "from",
                rows.first().map(|r| r.0.clone()).unwrap_or_default().into(),
            ),
            (
                "to",
                rows.last().map(|r| r.0.clone()).unwrap_or_default().into(),
            ),
            (
                "peak",
                peak.as_ref()
                    .map(|p| p.0.clone())
                    .unwrap_or_default()
                    .into(),
            ),
        ],
    )
}

/// A chart's numbers as a table behind "Show as a table", newest first.
#[component]
fn MeasureTable(
    id: String,
    name: String,
    head: String,
    rows: Vec<(String, String)>,
) -> impl IntoView {
    let i18n = use_i18n();
    let caption = name.clone();
    view! {
        <Disclosure id=id title=i18n.t("stats-show-table") mode=DisclosureMode::Native class="border-0">
            <Table>
                <TableCaption class="sr-only">{caption}</TableCaption>
                <TableHeader>
                    <TableRow>
                        <TableHead attr:scope="col">{head}</TableHead>
                        <TableHead attr:scope="col" class="text-right">{name}</TableHead>
                    </TableRow>
                </TableHeader>
                <TableBody>
                    {rows.into_iter().rev().map(|(label, v)| view! {
                        <TableRow>
                            <TableCell>{label}</TableCell>
                            <TableCell class="text-right tabular-nums">{v}</TableCell>
                        </TableRow>
                    }).collect_view()}
                </TableBody>
            </Table>
        </Disclosure>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn the_totals_sentence_comes_from_the_catalogue_in_each_locale() {
        // UX1-8: no lower-casing of translated status words in code
        let g = TotalsDto {
            group: "watch".into(),
            entries: 3,
            planned: 2,
            in_progress: 1,
            completed: 0,
            paused: 0,
            dropped: 0,
            amounts: contracts::AmountsDto::default(),
        };
        let en = I18n::new("en", BTreeMap::new());
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(
            totals_summary(&en, &g),
            "Watch: 2 planned, 1 in progress, 0 completed, 0 paused, 0 dropped"
        );
        assert!(
            totals_summary(&de, &g)
                .ends_with(": 2 geplant, 1 begonnen, 0 abgeschlossen, 0 pausiert, 0 abgebrochen"),
            "{}",
            totals_summary(&de, &g)
        );
    }

    #[test]
    fn the_chart_sentence_gives_the_total_as_the_chart_shows_it() {
        // hours for time; one or two minutes are "under 0.1", never "0" (review OT2C-3)
        let en = I18n::new("en", BTreeMap::new());
        let rows = |v: u64| vec![("Jan 2026".to_string(), v), ("Feb 2026".to_string(), 0)];
        let say = |v, m| chart_summary(&en, "Hours", "Listen".into(), &rows(v), m);
        assert_eq!(
            say(90, Measure::Hours),
            "Hours in Listen, Jan 2026 to Feb 2026: 1.5 in total, most in Jan 2026."
        );
        assert_eq!(
            say(2, Measure::Hours),
            "Hours in Listen, Jan 2026 to Feb 2026: under 0.1 in total, most in Jan 2026."
        );
        assert_eq!(
            say(0, Measure::Hours),
            "Hours in Listen, Jan 2026 to Feb 2026: none yet."
        );
        assert_eq!(
            say(1_200, Measure::Pages),
            "Hours in Listen, Jan 2026 to Feb 2026: 1,200 in total, most in Jan 2026."
        );
    }

    #[test]
    fn the_library_line_names_the_entries_and_the_statuses_that_have_any() {
        // OT-17: the status counts are one line per group on Home, without a chart
        let mut g = TotalsDto {
            group: "watch".into(),
            entries: 3,
            planned: 2,
            in_progress: 1,
            completed: 0,
            paused: 0,
            dropped: 0,
            amounts: contracts::AmountsDto::default(),
        };
        let en = I18n::new("en", BTreeMap::new());
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(
            library_line(&en, &g),
            "Watch: 3 entries, 1 in progress, 2 planned"
        );
        assert!(
            library_line(&de, &g).ends_with(": 3 Eintr\u{e4}ge, 1 begonnen, 2 geplant"),
            "{}",
            library_line(&de, &g)
        );
        g.group = "read".into();
        (g.entries, g.planned, g.in_progress, g.completed) = (1, 0, 0, 1);
        assert_eq!(library_line(&en, &g), "Read: 1 entry, 1 completed");
    }

    fn answer(total: u64) -> HomeDto {
        let none = TotalsDto {
            group: "all".into(),
            entries: total,
            planned: 0,
            in_progress: 0,
            completed: 0,
            paused: 0,
            dropped: 0,
            amounts: contracts::AmountsDto::default(),
        };
        HomeDto {
            library_total: total,
            continue_rows: Vec::new(),
            up_next: Vec::new(),
            activity: contracts::ActivityPageDto {
                rows: Vec::new(),
                next: None,
            },
            stats: contracts::StatsTotalsDto {
                overall: none,
                groups: Vec::new(),
                kinds: Vec::new(),
            },
        }
    }

    #[test]
    fn the_sections_outlive_answers_with_rows_and_offline_keeps_the_last_one() {
        // J/K lost the focus and the next step was detached when every refetch rendered Home
        // afresh; offline, a failed refetch replaced Home with "Home could not be loaded";
        // OT-21: during an import every answer differs, and rendering Home afresh for each
        // loaded every cover again
        let mut shown = Shown::default();
        let (first, r) = shown.next(Ok(answer(2)));
        assert_eq!(r, Ok(answer(2)));
        let (same, _) = shown.next(Ok(answer(2)));
        assert_eq!(same, first, "the same answer keeps its render");
        let offline = ServerFnError::Request("Failed to fetch".into());
        let (key, r) = shown.next(Err(offline.clone()));
        assert_eq!(
            (key, r),
            (first, Ok(answer(2))),
            "offline: the last answer stays"
        );
        let (changed, r) = shown.next(Ok(answer(3)));
        assert_eq!(
            changed, first,
            "a new answer with rows goes to the sections on screen (keyed cards)"
        );
        assert_eq!(r, Ok(answer(3)));
        let (empty, _) = shown.next(Ok(answer(0)));
        assert_ne!(empty, changed, "the empty library is another view");
        let (changed, _) = shown.next(Ok(answer(3)));
        assert_ne!(changed, empty, "and so is the way back");
        let refused = ServerFnError::ServerError("unauthorized".into());
        let (key, r) = shown.next(Err(refused.clone()));
        assert_ne!(key, changed);
        assert_eq!(
            r,
            Err(refused),
            "an answer of the server is shown, errors too"
        );
        let mut cold = Shown::default();
        assert_eq!(
            cold.next(Err(offline.clone())).1,
            Err(offline),
            "nothing to keep yet"
        );
    }
}
