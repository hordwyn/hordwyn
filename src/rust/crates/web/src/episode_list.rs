//! The Episodes tab (design.md 6.3, 5.4, 3.4 R-07): season chips with watched/total, "Mark
//! season watched", "Mark show watched", the episode rows with check circles, the next-up
//! row highlighted, and a row menu ("Mark up to here", "Watched with..."). The bulk marks log
//! now, spread over a date range (backfill, R-09) or on the release dates (WP4-7, when every
//! episode of the season shown has one up to today). Marking one episode while earlier ones
//! are open asks once, inline, whether to mark those too; "Remember for this show" keeps the
//! answer in this browser (DEC-231). A bulk mark on a range that is watched already asks,
//! inline as well, "Log these N again as a rewatch?"; a mixed range says in its toast how
//! many it skipped (OT-15, DEC-538). On a Completed show that answer starts a rewatch run;
//! while one is running the rows show the run (what it has reached is watched, the rest is
//! open again) and every mark is logged now (ot2e, DEC-587). The ranges come from
//! [`crate::episode_plan`].
//!
//! A refetch of the page rebuilds this list. What must outlive that sits in [`EpisodeUi`]:
//! the open question, the open row menu (it stays open under the pointer) and where the
//! focus goes once the list is back (review OT2A-4, OT2A-5).

use contracts::ProgressEventDto;
use leptos::html;
use leptos::prelude::*;
use ui::{
    Button, ButtonSize, ButtonVariant, Checkbox, Chip, DropdownMenu, Icon, IconName, Label,
    MenuItem, Segmented, SegmentedOption,
};

use crate::episode_plan::{
    AirDates, Bulk, BulkPlan, Ep, Plan, Previous, SeasonRow, again_starts_run, air_dates,
    date_choice_shown, missing_release, parse_range, plan_bulk, plan_single, previous_key,
    release_dates, spread_over, standing_again, unwatched_in_season, unwatched_up_to,
};
use crate::i18n::{t, use_i18n};
use crate::item_words::{Verb, again_ask_key, skipped_key, with_button_key};
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
    /// The bulk mark whose episodes are all watched (OT-15): "Log these N again as a
    /// rewatch?" is asked while the rows still say so.
    pub again: RwSignal<Option<Bulk>>,
    /// The question was asked just now and takes the focus once; a rebuild of the list under
    /// a standing question does not take it again.
    pub ask_focus: RwSignal<bool>,
    /// The control (a selector) that gets the focus when the list is back after "Log N
    /// again": the one that asked.
    pub focus_back: RwSignal<Option<String>>,
    /// The episode whose row menu is open.
    pub menu: RwSignal<Option<Ep>>,
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
            again: RwSignal::new(None),
            ask_focus: RwSignal::new(false),
            focus_back: RwSignal::new(None),
            menu: RwSignal::new(None),
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
        self.again.set(None);
        self.ask_focus.set(false);
        self.focus_back.set(None);
        self.menu.set(None);
    }
}

/// Focus the first element matching `selector` on the next frame (after the list is in the
/// document).
fn focus_soon(selector: String) {
    request_animation_frame(move || {
        let _ = crate::keyboard::focus(&selector);
    });
}

/// The words of the toast of a mark: "Watched S1 E2" or "5 episodes", and its second line,
/// the title or what a mixed range left alone.
fn mark_words(
    eps: &[Ep],
    skipped: usize,
    info: &ItemInfo,
    i18n: &crate::i18n::I18n,
) -> (String, String) {
    let label = match eps {
        [(s, e)] => i18n.t_args(
            "logged-step-episode",
            &[("season", (*s).into()), ("episode", (*e).into())],
        ),
        _ => i18n.t_args("episodes-logged", &[("count", eps.len().into())]),
    };
    let said = if skipped > 0 {
        i18n.t_args(skipped_key(info.verb), &[("count", skipped.into())])
    } else {
        info.title.clone()
    };
    (label, said)
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

/// What became of one log of a mark.
enum Sent {
    Logged(ProgressEventDto),
    /// The network failed: it is in the offline queue now (WP4-5).
    Queued(crate::offline::Queued),
    Failed(String),
}

/// Send one log of a mark; `queue` (an earlier one met a network failure) puts it straight
/// into the offline queue.
async fn send_log(
    (item_id, title): (&str, &str),
    progress: serde_json::Value,
    occurred_at: Option<String>,
    queue: bool,
    offline: Option<crate::offline::Offline>,
) -> Sent {
    let op = uuid::Uuid::now_v7().to_string();
    if queue {
        return Sent::Queued(crate::offline::queued_log(
            item_id,
            title,
            op,
            progress,
            occurred_at,
            offline,
        ));
    }
    let p = contracts::NewProgressDto {
        progress: progress.clone(),
        occurred_at: occurred_at.clone(),
        client_op_id: Some(op.clone()),
        with: Vec::new(),
    };
    match crate::api::log_progress(item_id.to_string(), p).await {
        Ok(ev) => Sent::Logged(ev),
        Err(e) if crate::offline::network_error(&e) && offline.is_some() => Sent::Queued(
            crate::offline::queued_log(item_id, title, op, progress, occurred_at, offline),
        ),
        Err(e) => Sent::Failed(crate::pages::error_text(&e)),
    }
}

/// How the episodes of a mark are logged: when, and what the toast says besides.
#[derive(Default)]
struct MarkOptions {
    /// One date per episode; `None`: now.
    dates: Option<Vec<String>>,
    /// Episodes of a bulk mark left alone as already watched (the toast says how many).
    skipped: usize,
    /// "Log N again" on a Completed entry: the start of a rewatch run is logged first
    /// (DEC-538, DEC-587), so the entry is In progress and the episodes are the run's.
    start_run: bool,
}

/// Log `eps` one after the other (see [`MarkOptions`]), then one toast with Undo for all of
/// them (a run started with them is taken back too); the toast says when the last of them
/// finished a rewatch run.
fn log_episodes(eps: Vec<Ep>, how: MarkOptions, info: &ItemInfo, ctx: StepCtx, ui: EpisodeUi) {
    let MarkOptions {
        dates,
        skipped,
        start_run,
    } = how;
    if eps.is_empty() {
        return;
    }
    let (label, said) = mark_words(&eps, skipped, info, &ctx.i18n);
    let (item_id, title, refresh) = (info.item_id.clone(), info.title.clone(), info.refresh);
    let verb = info.verb;
    ctx.funnel.intent(crate::funnel::Flow::EpisodeMark);
    ui.busy.set(true);
    ui.error.set(None);
    ui.again.set(None);
    leptos::task::spawn_local(async move {
        let mut logged = Vec::new();
        let mut failed = None;
        let mut queued = Vec::new();
        let mut dates = dates.unwrap_or_default().into_iter();
        // the start of the run first (now), then the episodes
        let start = start_run.then(|| (crate::next_step::rewatch_start(), None));
        let episodes: Vec<_> = eps
            .into_iter()
            .map(|(season, episode)| {
                let progress =
                    serde_json::json!({"type": "episode", "season": season, "episode": episode});
                (progress, dates.next())
            })
            .collect();
        for (progress, occurred_at) in start.into_iter().chain(episodes) {
            // after one network failure the rest goes straight into the queue (WP4-5)
            let queue = !queued.is_empty();
            match send_log(
                (&item_id, &title),
                progress,
                occurred_at,
                queue,
                ctx.offline,
            )
            .await
            {
                Sent::Logged(ev) => logged.push(ev),
                Sent::Queued(q) => queued.push(q),
                Sent::Failed(text) => {
                    failed = Some(text);
                    break;
                }
            }
        }
        if !logged.is_empty() {
            let events: Vec<&ProgressEventDto> = logged.iter().collect();
            let line = crate::next_step::said_title(&ctx.i18n, verb, said.clone(), &events);
            let ids = logged.iter().map(|e| e.id.clone()).collect();
            toast_logged(&ctx, &label, line, ids, Some(refresh));
        }
        // offline the toast says what a mixed range skipped as well (OT2A-11)
        if !queued.is_empty() {
            ctx.save_offline(said, queued, None);
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

/// What a bulk mark needs besides its range: the rows, the item, the contexts and the
/// episodes' release days.
#[derive(Clone)]
struct BulkCtx {
    rows: StoredValue<Vec<SeasonRow>>,
    info: StoredValue<ItemInfo>,
    ctx: StepCtx,
    ui: EpisodeUi,
    released: StoredValue<AirDates>,
    /// A rewatch run is running: the rows show the run, and its marks are logged now (a
    /// backfilled date would sort before the run's start and not count in it; DEC-587).
    in_run: bool,
    /// The entry is Completed: "Log N again" starts a rewatch run.
    start_on_again: bool,
}

/// When the episodes of a bulk mark happened, as the date choice says: now (`None`), spread
/// over the chosen date range (design.md 5.4, R-09), or each on its release day.
///
/// # Errors
/// The message for the person: a bad range, or episodes without a release day up to today.
fn bulk_dates(eps: &[Ep], b: &BulkCtx) -> Result<Option<Vec<String>>, String> {
    let today = crate::history::today();
    if b.in_run {
        return Ok(None);
    }
    match b.ui.when.get_untracked().as_str() {
        "release" => b
            .released
            .with_value(|d| release_dates(eps, d, today))
            .map(Some)
            .map_err(|n| {
                b.ctx
                    .i18n
                    .t_args("episodes-release-refused", &[("count", n.into())])
            }),
        "range" => parse_range(&b.ui.from.get_untracked(), &b.ui.to.get_untracked(), today)
            .map(|(from, to)| Some(spread_over(eps.len(), from, to)))
            .map_err(|e| b.ctx.i18n.t(e.key())),
        _ => Ok(None),
    }
}

/// Log the episodes of a bulk mark on the chosen dates (nothing is logged when the dates
/// cannot be used; the reason shows inline).
fn log_bulk(eps: Vec<Ep>, skipped: usize, b: &BulkCtx) {
    match bulk_dates(&eps, b) {
        Ok(dates) => {
            let how = MarkOptions {
                dates,
                skipped,
                start_run: false,
            };
            b.info
                .with_value(|i| log_episodes(eps, how, i, b.ctx.clone(), b.ui));
        }
        Err(text) => b.ui.error.set(Some(text)),
    }
}

/// A bulk mark ("Mark season", "Mark show", "Mark up to here"): the unwatched episodes of
/// its range are logged (the toast says how many watched ones were skipped). When all of the
/// range is watched already it asks whether to log it again (OT-15, DEC-538).
fn bulk(what: Bulk, b: &BulkCtx) {
    b.ui.error.set(None);
    match b.rows.with_value(|r| plan_bulk(r, what)) {
        BulkPlan::Nothing => b.ui.again.set(None),
        // set again for the same mark too: the question takes the focus once more
        BulkPlan::Again(_) => {
            b.ui.ask_focus.set(true);
            b.ui.again.set(Some(what));
        }
        BulkPlan::Log { eps, skipped } => {
            b.ui.again.set(None);
            log_bulk(eps, skipped, b);
        }
    }
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
    /// The start event of the rewatch run that is running, if one is (DEC-587): then the
    /// rows show the run, not every view.
    run: Option<String>,
    /// The entry's status: on a Completed one "Log N again" starts a rewatch run.
    status: String,
) -> impl IntoView {
    let rows = crate::episode_plan::season_rows_in(&attributes, &progress, run.as_deref());
    if rows.is_empty() {
        return view! { <p class="py-4 text-sm text-muted-foreground">{t("episodes-none")}</p> }
            .into_any();
    }
    let default = next.map_or(rows[0].number, |(s, _)| s);
    let shown = Memo::new(move |_| ui.season.get().unwrap_or(default));
    let rows = StoredValue::new(rows);
    let b = BulkCtx {
        rows,
        info: StoredValue::new(info),
        ctx: StepCtx::current(),
        ui,
        released: StoredValue::new(air_dates(&dates)),
        in_run: run.is_some(),
        start_on_again: again_starts_run(&status),
    };
    let (mark, answer) = single_marks(&b);
    focus_after_log(ui);
    let verb = b.info.with_value(|i| i.verb);
    view! {
        <section class="grid gap-3 py-2">
            <BulkBar bulk=b.clone() shown=shown/>
            <DateChoice bulk=b.clone() shown=shown has_dates=!dates.is_empty()/>
            <AskPrevious ui=ui rows=rows answer=answer/>
            <AskAgain bulk=b/>
            {move || ui.error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
            {move || rows.with_value(|r| r.iter().find(|s| s.number == shown.get()).cloned()).map(|season| view! {
                <ul class="divide-y divide-border rounded-md border border-border" aria-label=use_i18n().t_args("episodes-season", &[("season", season.number.into())])>
                    {(1..=season.count).map(|e| view! {
                        <EpisodeRow season=season.number episode=e watched=season.watched.contains(&e)
                            next=next == Some((season.number, e)) mark=mark open_sheet=open_sheet ui=ui verb=verb/>
                    }).collect_view()}
                </ul>
            })}
        </section>
    }
    .into_any()
}

/// Marks one episode, or (the flag) every open one up to it.
type MarkOne = Callback<(Ep, bool)>;

/// The marks of one episode: the row's button and "Mark up to here" (`mark`; the flag says
/// which), and the answer to "Also mark the earlier ones?" (`answer`: all of them, and
/// whether to remember it for this show).
fn single_marks(b: &BulkCtx) -> (MarkOne, Callback<(bool, bool)>) {
    let (rows, info, ui) = (b.rows, b.info, b.ui);
    let (up_to, ctx) = (b.clone(), b.ctx.clone());
    let mark = Callback::new(move |(target, all_before): (Ep, bool)| {
        if all_before {
            bulk(Bulk::UpTo(target), &up_to);
            return;
        }
        let answer = info.with_value(|i| stored_answer(&i.item_id));
        match rows.with_value(|r| plan_single(r, target, answer)) {
            Plan::Log(eps) => {
                info.with_value(|i| log_episodes(eps, MarkOptions::default(), i, ctx.clone(), ui));
            }
            Plan::Ask { .. } => ui.ask.set(Some(target)),
        }
    });
    let ctx = b.ctx.clone();
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
            info.with_value(|i| log_episodes(eps, MarkOptions::default(), i, ctx.clone(), ui));
        }
    });
    (mark, answer)
}

/// The head of the tab: the season chips, "Mark season watched" (the season shown) and
/// "Mark show watched".
#[component]
fn BulkBar(bulk: BulkCtx, shown: Memo<u32>) -> impl IntoView {
    let ui = bulk.ui;
    let (season, show) = (bulk.clone(), bulk.clone());
    let mark_season = move |_| self::bulk(Bulk::Season(shown.get_untracked()), &season);
    let mark_show = move |_| self::bulk(Bulk::Show, &show);
    view! {
        <div class="flex flex-wrap items-center justify-between gap-2">
            <SeasonChips rows=bulk.rows.get_value() shown=shown ui=ui/>
            <div class="flex flex-wrap gap-2">
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm disabled=ui.busy attr:data-ep-bulk="season" on:click=mark_season>
                    {t("episodes-mark-season")}
                </Button>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm disabled=ui.busy attr:data-ep-bulk="show" on:click=mark_show>
                    {t("episodes-mark-show")}
                </Button>
            </div>
        </div>
    }
}

/// The date choice of the bulk marks ([`BulkDates`]), where it has a say
/// ([`date_choice_shown`]): not while a rewatch run is running, and not while "Log N again"
/// is asked on a Completed entry (that answer starts a run, dated now; review OT2E-11).
#[component]
fn DateChoice(bulk: BulkCtx, shown: Memo<u32>, has_dates: bool) -> impl IntoView {
    let BulkCtx {
        rows,
        released,
        ui,
        in_run,
        start_on_again,
        ..
    } = bulk;
    let today = crate::history::today();
    let missing = Memo::new(move |_| {
        let open = rows.with_value(|r| unwatched_in_season(r, shown.get()));
        released.with_value(|d| missing_release(&open, d, today))
    });
    let offered = Memo::new(move |_| has_dates && missing.get() == 0);
    let visible = move || date_choice_shown(in_run, start_on_again, ui.again.get().is_some());
    view! {
        <Show when=visible>
            <BulkDates ui=ui offered=offered missing=missing has_dates=has_dates/>
        </Show>
    }
}

/// Once the list is back after "Log N again" (the refetch rebuilt it), the focus goes to the
/// control that asked; the answer's button is gone with the question (2.4.3). A row menu
/// that is open keeps the focus it takes itself.
fn focus_after_log(ui: EpisodeUi) {
    Effect::new(move |_| {
        if ui.busy.get_untracked() {
            return;
        }
        if let Some(selector) = ui.focus_back.get_untracked() {
            ui.focus_back.set(None);
            if ui.menu.get_untracked().is_none() {
                focus_soon(selector);
            }
        }
    });
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
                        // a question about another season's mark does not follow
                        ui.again.set(None);
                        ui.menu.set(None);
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

/// "Log these N again as a rewatch?" after a bulk mark on a range that is watched already
/// (OT-15, DEC-538). Inline, not a dialog (addendum-1 D15); the first button takes the focus
/// when the question is asked, so it is announced (once: a refetch that rebuilds the list
/// under a standing question does not pull the focus back to it, review OT2A-4). "Cancel"
/// gives the focus back to the control that asked; after "Log N again" it goes there once
/// the list is back. On a Completed entry the answer starts a rewatch run (the start, then
/// the episodes, all now; DEC-587); on any other status it logs extra views, which is what
/// "Again" on a row does, and while a run is running those are the run's.
#[component]
fn AskAgain(bulk: BulkCtx) -> impl IntoView {
    let ui = bulk.ui;
    let rows = bulk.rows;
    let verb = bulk.info.with_value(|i| i.verb);
    let first = NodeRef::<html::Div>::new();
    Effect::new(move |_| {
        if ui.again.get().is_some()
            && ui.ask_focus.get_untracked()
            && let Some(el) = first.get()
            && let Ok(Some(b)) = el.query_selector("button")
        {
            use leptos::wasm_bindgen::JsCast;
            ui.ask_focus.set(false);
            if let Ok(b) = b.dyn_into::<leptos::web_sys::HtmlElement>() {
                let _ = b.focus();
            }
        }
    });
    move || {
        let asked = ui.again.get()?;
        let eps = rows.with_value(|r| standing_again(r, Some(asked)))?;
        let i = use_i18n();
        let count = eps.len();
        let question = i.t_args(again_ask_key(verb), &[("count", count.into())]);
        // the run this answer starts is dated now: say so, the date choice is hidden
        let dated_now = bulk
            .start_on_again
            .then(|| i.t_args("episodes-again-now", &[("count", count.into())]));
        let (all, cancel) = (
            i.t_args("episodes-again-all", &[("count", count.into())]),
            i.t("episodes-again-cancel"),
        );
        let bulk = bulk.clone();
        // on a Completed entry the answer starts a rewatch run, which has its own dates:
        // now, whatever the date choice says (review OT2A-10). Else the question stays while
        // the date choice cannot be used (its reason shows inline)
        let log = move |_| {
            let dates = if bulk.start_on_again {
                Ok(None)
            } else {
                bulk_dates(&eps, &bulk)
            };
            match dates {
                Ok(dates) => {
                    ui.focus_back.set(Some(asked.asker()));
                    let how = MarkOptions {
                        dates,
                        skipped: 0,
                        start_run: bulk.start_on_again,
                    };
                    bulk.info
                        .with_value(|i| log_episodes(eps.clone(), how, i, bulk.ctx.clone(), ui));
                }
                Err(text) => ui.error.set(Some(text)),
            }
        };
        let cancel_it = move |_| {
            // first the focus, then the group goes (with the button that had it)
            let _ = crate::keyboard::focus(&asked.asker());
            ui.again.set(None);
        };
        Some(view! {
            <div node_ref=first role="group" aria-label=question.clone() data-testid="episodes-again"
                class="grid gap-3 rounded-md border border-info/40 bg-info-soft p-3">
                <p class="text-sm font-medium">{question.clone()}</p>
                {dated_now.map(|d| view! { <p data-testid="episodes-again-now" class="text-sm text-muted-foreground">{d}</p> })}
                <div class="flex flex-wrap items-center gap-2">
                    <Button size=ButtonSize::Sm disabled=ui.busy on:click=log>
                        {all}
                    </Button>
                    <Button size=ButtonSize::Sm variant=ButtonVariant::Outline on:click=cancel_it>
                        {cancel}
                    </Button>
                </div>
            </div>
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
    ui: EpisodeUi,
    verb: Option<Verb>,
) -> impl IntoView {
    let i18n = use_i18n();
    let busy = ui.busy;
    // the menu's open state lives in `ui`: a refetch rebuilds the row, and a menu the
    // person has open stays open (review OT2A-5)
    let here = (season, episode);
    let open = RwSignal::new(ui.menu.get_untracked() == Some(here));
    Effect::new(move |_| {
        if open.get() {
            ui.menu.set(Some(here));
        } else if ui.menu.get_untracked() == Some(here) {
            ui.menu.set(None);
        }
    });
    let n = [("season", season.into()), ("episode", episode.into())];
    // the words of the toast and of the dialog's title: the past tense (OT-5)
    let label = i18n.t_args("logged-step-episode", &n);
    let menu = vec![
        MenuItem {
            label: i18n.t("episodes-up-to-here"),
            on_select: Callback::new(move |()| mark.run(((season, episode), true))),
            destructive: false,
        },
        MenuItem {
            label: i18n.t(with_button_key(verb)),
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
    let look = if next { "bg-primary-soft" } else { "" };
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
                id=crate::episode_plan::row_menu_id(here) open=open trigger_class="size-8 px-0"
                trigger=Box::new(|| view! { <Icon name=IconName::More/> }.into_any())/>
        </li>
    }
}
