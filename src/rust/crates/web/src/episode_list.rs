//! The Episodes tab (design.md 6.3, 5.4, 3.4 R-07): season chips with watched/total, "Mark
//! season watched", "Mark show watched", the episode rows with check circles, the next-up
//! row highlighted, and a row menu ("Mark up to here", "Watched with..."). The bulk marks log
//! now, spread over a date range (backfill, R-09) or on the release dates (WP4-7, when every
//! episode of the season shown has one up to today). Marking one episode while earlier ones
//! are open asks once, inline, whether to mark those too; "Remember for this show" keeps the
//! answer in this browser (DEC-231). The ranges come from [`crate::episode_plan`].

use contracts::ProgressEventDto;
use leptos::html;
use leptos::prelude::*;
use ui::{
    Button, ButtonSize, ButtonVariant, Checkbox, Chip, DropdownMenu, Icon, IconName, Label,
    MenuItem, Segmented, SegmentedOption,
};

use crate::episode_plan::{
    AirDates, Bulk, Ep, Plan, Previous, SeasonRow, air_dates, missing_release, parse_range,
    plan_bulk, plan_single, previous_key, release_dates, spread_over, standing_note,
    unwatched_in_season, unwatched_up_to,
};
use crate::i18n::{t, use_i18n};
use crate::log_sheet::{ItemInfo, SheetTarget};
use crate::next_step::{StepCtx, toast_logged};

/// Episode state the page keeps across refetches: the season shown and an open question.
#[derive(Clone, Copy)]
pub struct EpisodeUi {
    pub season: RwSignal<Option<u32>>,
    /// "Also mark the earlier ones?" for this episode.
    pub ask: RwSignal<Option<Ep>>,
    pub busy: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
    /// The bulk mark that logged nothing (every episode it covers is watched, OT-15); the
    /// line shows its message while the rows still say so.
    pub note: RwSignal<Option<Bulk>>,
    /// Dates of the bulk marks: `now`, `range` ("Spread over a date range", R-09) or `release`
    /// ("On release dates", WP4-7).
    pub when: RwSignal<String>,
    pub from: RwSignal<String>,
    pub to: RwSignal<String>,
}

impl Default for EpisodeUi {
    fn default() -> Self {
        Self {
            season: RwSignal::new(None),
            ask: RwSignal::new(None),
            busy: RwSignal::new(false),
            error: RwSignal::new(None),
            note: RwSignal::new(None),
            when: RwSignal::new("now".into()),
            from: RwSignal::new(String::new()),
            to: RwSignal::new(String::new()),
        }
    }
}

impl EpisodeUi {
    /// Another item: what was said and asked about the last one does not follow (OT1A-2),
    /// and neither does its season. The date choice of the bulk marks stays.
    pub fn reset(&self) {
        self.season.set(None);
        self.ask.set(None);
        self.error.set(None);
        self.note.set(None);
    }
}

/// A bulk mark has nothing to log: say so, again when it is the same mark as before. A live
/// region reports changes only, so the line is emptied and filled on the next frame (OT1A-5).
fn say_nothing_to_mark(note: RwSignal<Option<Bulk>>, what: Bulk) {
    note.set(None);
    request_animation_frame(move || {
        let _ = note.try_set(Some(what));
    });
}

/// The remembered answer for a show (browser storage; nothing stored: ask).
fn stored_answer(item_id: &str) -> Previous {
    #[cfg(feature = "hydrate")]
    {
        let stored = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(&previous_key(item_id)).ok().flatten());
        Previous::parse(stored.as_deref())
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = previous_key(item_id);
        Previous::Ask
    }
}

fn remember_answer(item_id: &str, answer: Previous) {
    #[cfg(feature = "hydrate")]
    if let (Some(s), Some(v)) = (
        web_sys::window().and_then(|w| w.local_storage().ok().flatten()),
        answer.as_stored(),
    ) {
        let _ = s.set_item(&previous_key(item_id), v);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (item_id, answer);
}

/// Log `eps` one after the other (at `dates`, one per episode, else now), then one toast
/// with Undo for all of them.
fn log_episodes(
    eps: Vec<Ep>,
    dates: Option<Vec<String>>,
    info: &ItemInfo,
    ctx: StepCtx,
    ui: EpisodeUi,
) {
    if eps.is_empty() {
        return;
    }
    let label = match eps.as_slice() {
        [(s, e)] => ctx.i18n.t_args(
            "next-step-episode",
            &[("season", (*s).into()), ("episode", (*e).into())],
        ),
        _ => ctx
            .i18n
            .t_args("episodes-logged", &[("count", eps.len().into())]),
    };
    let (item_id, title, refresh) = (info.item_id.clone(), info.title.clone(), info.refresh);
    ctx.funnel.intent(crate::funnel::Flow::EpisodeMark);
    ui.busy.set(true);
    ui.error.set(None);
    ui.note.set(None);
    leptos::task::spawn_local(async move {
        let mut ids = Vec::new();
        let mut failed = None;
        let mut queued = Vec::new();
        let mut dates = dates.unwrap_or_default().into_iter();
        for (season, episode) in eps {
            let progress =
                serde_json::json!({"type": "episode", "season": season, "episode": episode});
            let occurred_at = dates.next();
            let op = uuid::Uuid::now_v7().to_string();
            // after one network failure the rest goes straight into the queue (WP4-5)
            if !queued.is_empty() {
                queued.push(offline_log(
                    &item_id,
                    &title,
                    op,
                    progress,
                    occurred_at,
                    ctx.offline,
                ));
                continue;
            }
            let p = contracts::NewProgressDto {
                progress: progress.clone(),
                occurred_at: occurred_at.clone(),
                client_op_id: Some(op.clone()),
                with: Vec::new(),
            };
            match crate::api::log_progress(item_id.clone(), p).await {
                Ok(ev) => ids.push(ev.id),
                Err(e) if crate::offline::network_error(&e) && ctx.offline.is_some() => {
                    queued.push(offline_log(
                        &item_id,
                        &title,
                        op,
                        progress,
                        occurred_at,
                        ctx.offline,
                    ));
                }
                Err(e) => {
                    failed = Some(crate::pages::error_text(&e));
                    break;
                }
            }
        }
        if !ids.is_empty() {
            toast_logged(&ctx, &label, title.clone(), ids, Some(refresh));
        }
        if !queued.is_empty() {
            ctx.save_offline(title, queued, None);
        }
        if let Some(text) = failed {
            ctx.funnel.abandoned_if(crate::funnel::Flow::EpisodeMark);
            ui.error.set(Some(
                ctx.i18n
                    .t_args("library-step-failed", &[("error", text.into())]),
            ));
        } else {
            ctx.funnel.logged();
        }
        ui.busy.set(false);
        refresh.run(());
    });
}

/// An episode log for the offline queue, with its op id, time (now when not backfilled) and
/// the queue's stamp for the status it implies (DEC-455).
fn offline_log(
    item_id: &str,
    title: &str,
    op: String,
    progress: serde_json::Value,
    occurred_at: Option<String>,
    offline: Option<crate::offline::Offline>,
) -> crate::offline::Queued {
    let at = occurred_at.or_else(crate::offline::now_rfc3339);
    let hlc = offline.map(crate::offline::Offline::stamp);
    crate::offline::Queued::with_op_id(
        op,
        crate::offline::log_command(item_id, progress, at, hlc.as_deref()),
        title,
        crate::offline::now_ms(),
    )
}

/// A bulk mark ("Mark season", "Mark show", "Mark up to here"): now, spread over the chosen
/// date range (design.md 5.4, R-09), or each on its release day (refused, with nothing
/// logged, when one of them has none up to today). When every episode it covers is watched
/// already it says so instead of doing nothing (OT-15).
fn bulk(
    what: Bulk,
    rows: StoredValue<Vec<SeasonRow>>,
    info: StoredValue<ItemInfo>,
    ctx: StepCtx,
    ui: EpisodeUi,
    released: StoredValue<AirDates>,
) {
    let Ok(eps) = rows.with_value(|r| plan_bulk(r, what)) else {
        ui.error.set(None);
        say_nothing_to_mark(ui.note, what);
        return;
    };
    ui.note.set(None);
    let dates = if ui.when.get_untracked() == "release" {
        match released.with_value(|d| release_dates(&eps, d, crate::history::today())) {
            Ok(days) => Some(days),
            Err(n) => {
                ui.error.set(Some(
                    ctx.i18n
                        .t_args("episodes-release-refused", &[("count", n.into())]),
                ));
                return;
            }
        }
    } else if ui.when.get_untracked() == "range" {
        let range = parse_range(
            &ui.from.get_untracked(),
            &ui.to.get_untracked(),
            crate::history::today(),
        );
        match range {
            Ok((from, to)) => Some(spread_over(eps.len(), from, to)),
            Err(e) => {
                ui.error.set(Some(ctx.i18n.t(e.key())));
                return;
            }
        }
    } else {
        None
    };
    info.with_value(|i| log_episodes(eps, dates, i, ctx, ui));
}

/// When the bulk marks happened: now, spread over a date range (backfill), or on the release
/// dates, offered only when every open episode of the season shown has one up to today
/// (`offered`); else, for an item with any dates, a hint says how many are missing.
#[component]
fn BulkDates(
    ui: EpisodeUi,
    offered: Memo<bool>,
    missing: Memo<usize>,
    has_dates: bool,
) -> impl IntoView {
    // a season without the dates takes "On release dates" away: fall back to now
    Effect::new(move |_| {
        if !offered.get() && ui.when.get_untracked() == "release" {
            ui.when.set("now".into());
        }
    });
    let segmented = move || {
        let i18n = use_i18n();
        let mut options = vec![
            SegmentedOption::new("now", i18n.t("episodes-when-now")),
            SegmentedOption::new("range", i18n.t("episodes-when-range")),
        ];
        if offered.get() {
            options.push(SegmentedOption::new(
                "release",
                i18n.t("episodes-when-release"),
            ));
        }
        view! { <Segmented id="ep-when" label=t("episodes-when") value=ui.when options=options class="flex-wrap"/> }
    };
    let missing_hint = move || has_dates && !offered.get();
    let release_hint = move || ui.when.get() == "release";
    let max = crate::history::today().iso();
    let field = "flex h-9 w-full rounded-md border border-input bg-background px-3 text-sm \
        focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
    view! {
        <div class="grid gap-2">
            {segmented}
            <Show when=missing_hint>
                <p data-testid="ep-release-missing" class="text-xs text-muted-foreground">
                    {move || use_i18n().t_args("episodes-release-missing", &[("count", missing.get().into())])}
                </p>
            </Show>
            <Show when=release_hint>
                <p class="text-xs text-muted-foreground">{t("episodes-release-hint")}</p>
            </Show>
            <Show when=move || ui.when.get() == "range">
                <div class="flex flex-wrap gap-3">
                    <div class="grid gap-1">
                        <Label r#for="ep-from">{t("episodes-from")}</Label>
                        <input id="ep-from" type="date" max=max.clone() class=field aria-describedby="ep-range-hint"
                            prop:value=move || ui.from.get() on:input=move |ev| ui.from.set(event_target_value(&ev))/>
                    </div>
                    <div class="grid gap-1">
                        <Label r#for="ep-to">{t("episodes-to")}</Label>
                        <input id="ep-to" type="date" max=max.clone() class=field aria-describedby="ep-range-hint"
                            prop:value=move || ui.to.get() on:input=move |ev| ui.to.set(event_target_value(&ev))/>
                    </div>
                </div>
                <p id="ep-range-hint" class="text-xs text-muted-foreground">{t("episodes-range-hint")}</p>
            </Show>
        </div>
    }
}

/// The Episodes tab.
#[component]
pub fn EpisodeList(
    info: ItemInfo,
    attributes: serde_json::Value,
    progress: Vec<ProgressEventDto>,
    /// Release dates of the episodes (WP4-7; empty when the item has none).
    dates: Vec<contracts::EpisodeDateDto>,
    /// The next unwatched episode (highlighted, its season shown first).
    next: Option<Ep>,
    ui: EpisodeUi,
    /// Opens the watched sheet for one episode.
    open_sheet: Callback<SheetTarget>,
) -> impl IntoView {
    let rows = crate::episode_plan::season_rows(&attributes, &progress);
    if rows.is_empty() {
        return view! { <p class="py-4 text-sm text-muted-foreground">{t("episodes-none")}</p> }
            .into_any();
    }
    let default = next.map_or(rows[0].number, |(s, _)| s);
    let shown = Memo::new(move |_| ui.season.get().unwrap_or(default));
    let rows = StoredValue::new(rows);
    let info = StoredValue::new(info);
    let released = StoredValue::new(air_dates(&dates));
    let has_dates = !dates.is_empty();
    let today = crate::history::today();
    let missing = Memo::new(move |_| {
        let open = rows.with_value(|r| unwatched_in_season(r, shown.get()));
        released.with_value(|d| missing_release(&open, d, today))
    });
    let offered = Memo::new(move |_| has_dates && missing.get() == 0);
    let ctx = StepCtx::current();
    let c2 = ctx.clone();
    let mark_season = move |_| {
        let season = Bulk::Season(shown.get_untracked());
        bulk(season, rows, info, c2.clone(), ui, released);
    };
    let c3 = ctx.clone();
    let mark = Callback::new(move |(target, up_to): (Ep, bool)| {
        if up_to {
            bulk(Bulk::UpTo(target), rows, info, c3.clone(), ui, released);
            return;
        }
        let answer = info.with_value(|i| stored_answer(&i.item_id));
        match rows.with_value(|r| plan_single(r, target, answer)) {
            Plan::Log(eps) => info.with_value(|i| log_episodes(eps, None, i, c3.clone(), ui)),
            Plan::Ask { .. } => ui.ask.set(Some(target)),
        }
    });
    let answer = Callback::new(move |(all, remember): (bool, bool)| {
        let Some(target) = ui.ask.get_untracked() else {
            return;
        };
        ui.ask.set(None);
        let choice = if all { Previous::All } else { Previous::Only };
        if remember {
            info.with_value(|i| remember_answer(&i.item_id, choice));
        }
        let eps = rows.with_value(|r| plan_single(r, target, choice));
        if let Plan::Log(eps) = eps {
            info.with_value(|i| log_episodes(eps, None, i, ctx.clone(), ui));
        }
    });
    let c4 = StepCtx::current();
    let mark_show = move |_| bulk(Bulk::Show, rows, info, c4.clone(), ui, released);
    view! {
        <section class="grid gap-3 py-2">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <SeasonChips rows=rows.get_value() shown=shown ui=ui/>
                <div class="flex flex-wrap gap-2">
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm disabled=ui.busy on:click=mark_season>
                        {t("episodes-mark-season")}
                    </Button>
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm disabled=ui.busy on:click=mark_show>
                        {t("episodes-mark-show")}
                    </Button>
                </div>
            </div>
            <BulkDates ui=ui offered=offered missing=missing has_dates=has_dates/>
            <AskPrevious ui=ui rows=rows answer=answer/>
            {move || ui.error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
            // always there, so the text is announced when it appears
            <p role="status" data-testid="episodes-note" class="text-sm empty:hidden">{move || rows.with_value(|r| standing_note(r, ui.note.get())).map(t)}</p>
            {move || rows.with_value(|r| r.iter().find(|s| s.number == shown.get()).cloned()).map(|season| view! {
                <ul class="divide-y divide-border rounded-md border border-border" aria-label=use_i18n().t_args("episodes-season", &[("season", season.number.into())])>
                    {(1..=season.count).map(|e| view! {
                        <EpisodeRow season=season.number episode=e watched=season.watched.contains(&e)
                            next=next == Some((season.number, e)) mark=mark open_sheet=open_sheet busy=ui.busy/>
                    }).collect_view()}
                </ul>
            })}
        </section>
    }
    .into_any()
}

/// One chip per season with watched/total; exactly one is pressed.
#[component]
fn SeasonChips(rows: Vec<SeasonRow>, shown: Memo<u32>, ui: EpisodeUi) -> impl IntoView {
    view! {
        <div role="group" aria-label=t("episodes-seasons") class="flex flex-wrap gap-2">
            {rows.into_iter().map(|r| {
                let n = r.number;
                let pressed = RwSignal::new(shown.get_untracked() == n);
                Effect::new(move |_| pressed.set(shown.get() == n));
                let count = use_i18n().t_args("episodes-count", &[
                    ("watched", r.watched.len().into()),
                    ("total", r.count.into()),
                ]);
                view! {
                    <Chip pressed=pressed count=count on_change=Callback::new(move |_| {
                        // a chip never unpresses itself: the season stays shown
                        pressed.set(true);
                        ui.season.set(Some(n));
                        // a message about another season's mark does not follow
                        ui.note.set(None);
                    })>
                        {use_i18n().t_args("episodes-season", &[("season", n.into())])}
                    </Chip>
                }
            }).collect_view()}
        </div>
    }
}

/// The question before marking one episode with open ones before it (asked once per show
/// when the answer is remembered). Inline, not a dialog (addendum-1 D15).
#[component]
fn AskPrevious(
    ui: EpisodeUi,
    rows: StoredValue<Vec<SeasonRow>>,
    answer: Callback<(bool, bool)>,
) -> impl IntoView {
    let remember = RwSignal::new(false);
    let first = NodeRef::<html::Div>::new();
    Effect::new(move |_| {
        if ui.ask.get().is_some()
            && let Some(el) = first.get()
            && let Ok(Some(b)) = el.query_selector("button")
        {
            use leptos::wasm_bindgen::JsCast;
            if let Ok(b) = b.dyn_into::<leptos::web_sys::HtmlElement>() {
                let _ = b.focus();
            }
        }
    });
    move || {
        ui.ask.get().map(|(s, e)| {
        let i = use_i18n();
        let all = rows.with_value(|r| unwatched_up_to(r, (s, e)));
        let earlier = all.len().saturating_sub(1);
        let question = i.t_args("episodes-ask", &[("count", earlier.into())]);
        view! {
            <div node_ref=first role="group" aria-label=question.clone() data-testid="episodes-ask"
                class="grid gap-3 rounded-md border border-info/40 bg-info-soft p-3">
                <p class="text-sm font-medium">{question.clone()}</p>
                <div class="flex flex-wrap items-center gap-2">
                    <Button size=ButtonSize::Sm on:click=move |_| answer.run((true, remember.get_untracked()))>
                        {use_i18n().t_args("episodes-ask-all", &[("count", all.len().into())])}
                    </Button>
                    <Button size=ButtonSize::Sm variant=ButtonVariant::Outline on:click=move |_| answer.run((false, remember.get_untracked()))>
                        {use_i18n().t_args("episodes-ask-one", &[("season", s.into()), ("episode", e.into())])}
                    </Button>
                    <span class="flex items-center gap-2">
                        <Checkbox id="episodes-remember" checked=remember/>
                        <Label r#for="episodes-remember">{t("episodes-remember")}</Label>
                    </span>
                </div>
            </div>
        }
    })
    }
}

#[component]
fn EpisodeRow(
    season: u32,
    episode: u32,
    watched: bool,
    next: bool,
    mark: Callback<(Ep, bool)>,
    open_sheet: Callback<SheetTarget>,
    busy: RwSignal<bool>,
) -> impl IntoView {
    let i18n = use_i18n();
    let n = [("season", season.into()), ("episode", episode.into())];
    let label = i18n.t_args("next-step-episode", &n);
    let menu = vec![
        MenuItem {
            label: i18n.t("episodes-up-to-here"),
            on_select: Callback::new(move |()| mark.run(((season, episode), true))),
            destructive: false,
        },
        MenuItem {
            label: i18n.t("episodes-with"),
            on_select: {
                let label = label.clone();
                Callback::new(move |()| {
                    open_sheet.run(SheetTarget {
                        progress: serde_json::json!({"type": "episode", "season": season, "episode": episode}),
                        label: label.clone(),
                    });
                })
            },
            destructive: false,
        },
    ];
    let state = if watched {
        i18n.t("episodes-watched")
    } else if next {
        i18n.t("episodes-next")
    } else {
        String::new()
    };
    let look = if next { "bg-accent" } else { "" };
    view! {
        <li data-testid="episode-row" data-next=next.then_some("true")
            class=format!("flex items-center gap-3 px-3 py-2 {look}")>
            {if watched {
                view! { <Icon name=IconName::CircleCheck class="text-success"/> }.into_any()
            } else {
                view! { <Icon name=IconName::Circle class="text-muted-foreground"/> }.into_any()
            }}
            <span class="min-w-0 flex-1">
                <span class="block text-sm font-medium">{use_i18n().t_args("episodes-episode", &[("episode", episode.into())])}</span>
                <span class="block text-xs text-muted-foreground">{state}</span>
            </span>
            <Button size=ButtonSize::Sm variant=if next { ButtonVariant::Default } else { ButtonVariant::Ghost }
                disabled=busy attr:aria-label=use_i18n().t_args(if watched { "episodes-mark-again" } else { "episodes-mark" }, &n)
                on:click=move |_| mark.run(((season, episode), false))>
                {if watched { t("episodes-again") } else { t("episodes-watched") }}
            </Button>
            <DropdownMenu label=use_i18n().t_args("episodes-row-menu", &n) items=menu
                id=format!("ep-menu-{season}-{episode}") trigger_class="size-8 px-0"
                trigger=Box::new(|| view! { <Icon name=IconName::More/> }.into_any())/>
        </li>
    }
}
