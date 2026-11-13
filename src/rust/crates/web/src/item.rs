//! The item page `/items/:id` (design.md 6.3, 3.4, 5.4, 5.5; addendum-1 1.3 Item row): cover
//! with the entry facts card (status, progress, rating open; the rest behind "More facts"),
//! title block with the status pill and the fact line ("Watched 3 times, last on ...", OT-5,
//! OT-7), the action bar per progress model with the next step first (two actions for an item
//! that is not in the library: "Mark as watched" and "Want to watch", OT-4), the rating /
//! tags / lists row, and tabs (Episodes where the kind has them, About, History, Notes).
//! The state of the log dialogs, the open tab and the episode question live here, outside
//! the view that a refetch rebuilds; the two log dialogs themselves are mounted here once,
//! next to the tag editor and the list picker (review OT2B-1).

use contracts::{
    ActivityRowDto, CompanionDto, ItemPageDto, KindDto, LibraryRowDto, ListDto, NextStepDto,
};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params_map;
use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Disclosure, DisclosureMode,
    DropdownMenu, FormField, Icon, IconName, Label, MenuItem, Select, SelectOption, Skeleton, Tabs,
    TabsContent, TabsList, TabsTrigger, Textarea, ToastVariant,
};

use crate::api;
use crate::episode_list::{EpisodeList, EpisodeUi};
use crate::events::{mentions, use_events};
use crate::history::ActivityRow;
use crate::i18n::{kind_label, t, use_i18n};
use crate::item_words::{
    Facts, RewatchWords, Verb, again_count, fact_line, times_text, want_key, with_button_key,
    with_play_time,
};
use crate::log_sheet::{
    InlineDraft, ItemInfo, PLAY_OPENER, PagesInline, PlaySheet, PlayState, PositionInline,
    SheetTarget, WITH_OPENER, WatchedSheet, WatchedState,
};
use crate::next_step::{
    LoggedStep, StepCtx, log_run_marker, logged_step_label, next_step_label, progress_text,
    rewatch_start, rewatch_stop, run_first_step, run_step, step_progress, toast_added,
};
use crate::pages::{error_text, go, needs_sign_in};
use crate::widgets::{Cover, RatingInput, TagBadges, TagEditor};

type Res<T> = Resource<Result<T, ServerFnError>>;

/// Page state that survives a refetch.
#[derive(Clone, Copy)]
struct ItemUi {
    tab: RwSignal<String>,
    /// The tab last chosen by the page rather than the user; replaced when the kind loads late.
    auto_tab: RwSignal<String>,
    episodes: EpisodeUi,
    watched: WatchedState,
    play: PlayState,
    /// What the log dialogs know about the item: each render of the view writes it, the
    /// dialogs read it when they open and when they save (never while they are open).
    info: RwSignal<Option<ItemInfo>>,
    /// The play dialog's title, the step it logs ("Log a play", "Log a session").
    play_title: RwSignal<String>,
    companions: RwSignal<Vec<CompanionDto>>,
    tags: RwSignal<Option<LibraryRowDto>>,
    /// The list picker (`L`): the item to add.
    pick: RwSignal<Option<Vec<String>>>,
    /// `?log=1` (with `&with=<person>` and `&group=<game group>`): open the log sheet with
    /// these people (and that group) once the page is there (the palette's "Log with ..." and
    /// "Log a play of ...", WP4-4; People's "Log a play with" a group, WP4-6).
    auto_log: RwSignal<Option<(Vec<String>, String)>>,
    /// The inline page or position field's typed text and error (DEC-477).
    inline: InlineDraft,
    refresh: Callback<()>,
}

#[component]
pub fn ItemPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.read().get("id").unwrap_or_default();
    let data = Resource::new(id, api::item_page);
    let kinds = Resource::new(|| (), |()| api::kinds());
    let lists = Resource::new(|| (), |()| api::lists());
    let known = Resource::new(|| (), |()| api::tags());
    // one refetch for a burst of changes (a bulk mark sends an event per episode, then asks
    // for a refresh itself): overlapping refetches rebuilt the view under a live click
    let pending = RwSignal::new(false);
    // each load renders afresh (keyed): rebuilding the old view in place kept event handlers
    // of the previous render, whose state was already disposed; and again once the kinds
    // arrive (after a client-side navigation the item can come first, and without its kind the
    // view has no log sheets, WP4-4)
    let version = RwSignal::new(0u32);
    let refetch = move || {
        if !pending.get_untracked() {
            pending.set(true);
            set_timeout(
                move || {
                    pending.set(false);
                    version.update(|v| *v += 1);
                    data.refetch();
                    lists.refetch();
                },
                std::time::Duration::from_millis(300),
            );
        }
    };
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, payload)) = events.0.get()
            // missed events may have been about this item (SEC0-9)
            && (topic == crate::events::LAGGED
                || (topic == "entity.updated" && mentions(&payload, &id())))
        {
            refetch();
        }
    });
    let companions = RwSignal::new(Vec::new());
    let ui = ItemUi {
        tab: RwSignal::new(String::new()),
        auto_tab: RwSignal::new(String::new()),
        episodes: EpisodeUi::default(),
        watched: WatchedState::new(),
        play: PlayState::new(companions),
        info: RwSignal::new(None),
        play_title: RwSignal::new(String::new()),
        companions,
        tags: RwSignal::new(None),
        pick: RwSignal::new(None),
        auto_log: RwSignal::new(None),
        inline: InlineDraft::new(),
        refresh: Callback::new(move |()| refetch()),
    };
    let (inline, episodes) = (ui.inline, ui.episodes);
    Effect::new(move |prev: Option<String>| {
        let now = id();
        if prev.is_some_and(|p| p != now) {
            inline.clear();
            episodes.reset();
            // a log dialog left open belongs to the item that is gone
            ui.watched.dismiss();
            ui.play.dismiss();
        }
        now
    });
    // before the first item is there (and for the ids of the panel) the plain name
    let plain = use_i18n().t("next-step-log-play");
    let play_title = Signal::derive(move || {
        Some(ui.play_title.get())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| plain.clone())
    });
    item_keys(&ui, data);
    follow_log_query(ui.auto_log);
    let known_tags = Signal::derive(move || {
        known
            .get()
            .and_then(Result::ok)
            .map(|v| v.into_iter().map(|t| t.tag).collect())
            .unwrap_or_default()
    });
    view! {
        // Transition: a refetch keeps the page (and focus) until the new data is there
        <Transition fallback=|| view! { <Skeleton class="h-64 w-full"/> }>
            <For each=move || data.get().map(|r| (version.get_untracked(), r, kinds.get().and_then(Result::ok))) key=item_key
                children=move |(_, r, kinds)| match r {
                Err(e) if needs_sign_in(&e) => { go("/sign-in"); ().into_any() }
                Err(e) => view! { <p role="alert">{error_text(&e)}</p> }.into_any(),
                Ok(page) => {
                    let kind = kinds
                        .and_then(|k| k.kinds.into_iter().find(|k| k.id == page.item.kind));
                    view! { <ItemView page=page kind=kind ui=ui lists=lists/> }.into_any()
                }
                }/>
        </Transition>
        <TagEditor target=ui.tags known=known_tags on_change=ui.refresh/>
        <crate::bulk::ListPicker target=ui.pick on_done=ui.refresh/>
        // mounted once, outside the keyed view: a refetch renders the page afresh under an
        // open dialog and leaves its focus, its scroll position and its fields alone
        <WatchedSheet state=ui.watched info=ui.info companions=ui.companions/>
        <PlaySheet state=ui.play info=ui.info title=play_title/>
    }
}

/// One render of the page: the item's kind when it is known, and the answer.
type Loaded = (
    u32,
    Result<ItemPageDto, ServerFnError>,
    Option<contracts::KindsDto>,
);

/// What makes a render of the page a new one: a refetch (`version`), the kinds arriving, and
/// another item. Without the item a client-side navigation from one item to the next (the
/// palette on an item page) kept showing the first one: the route only changes its id.
fn item_key((version, answer, kinds): &Loaded) -> (u32, bool, Option<String>) {
    let item = answer.as_ref().ok().map(|page| page.item.id.clone());
    (*version, kinds.is_some(), item)
}

/// Follow `?log=1&with=<id>,<id>&group=<id>` into `asked`: the people (none without `with`)
/// and the game group (empty without `group`). Read in an effect: on a client-side navigation
/// the page is built before the query map has the new URL's values (seen in the e2e run).
fn follow_log_query(asked: RwSignal<Option<(Vec<String>, String)>>) {
    let q = leptos_router::hooks::use_query_map();
    Effect::new(move |_| {
        let wanted = q.with(|q| {
            q.get("log").map(|_| {
                let with = q
                    .get("with")
                    .map(|w| {
                        w.split(',')
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                (with, q.get("group").unwrap_or_default())
            })
        });
        if wanted.is_some() {
            asked.set(wanted);
        }
    });
}

/// The item page's keys (design.md 6.3): `W` the next step (or the first field of an inline
/// step, or "Add to library"), `R` the rating, `T` the tags, `L` the list picker.
fn item_keys(ui: &ItemUi, data: Res<ItemPageDto>) {
    let ui = *ui;
    crate::keyboard::page_keys(move |_, key| match key {
        "w" => {
            crate::keyboard::click("[data-action-bar] [data-next-step]")
                || crate::keyboard::focus("[data-action-bar] input")
        }
        "r" => crate::keyboard::focus("#ip-rate [role=radio][tabindex=\"0\"]"),
        "t" => crate::keyboard::click("[data-shortcut=tags]"),
        "l" => {
            let id = data.with_untracked(|d| {
                d.as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .filter(|p| p.entry.is_some())
                    .map(|p| p.item.id.clone())
            });
            id.is_some_and(|id| {
                ui.pick.set(Some(vec![id]));
                true
            })
        }
        _ => false,
    });
}

/// Remember the opened item for the empty palette (this browser only, DEC: WP4-4).
fn remember_recent(page: &ItemPageDto, kind: Option<&KindDto>) {
    let username = expect_context::<crate::app::ClientConfig>().username;
    let item = crate::recent::RecentItem {
        id: page.item.id.clone(),
        label: match page.item.year {
            Some(y) => format!("{} ({y})", crate::i18n::title_text(&page.item.title)),
            None => crate::i18n::title_text(&page.item.title),
        },
        plays: kind.is_some_and(|k| k.progress_model == "sessions"),
        kind: page.item.kind.clone(),
        cover: crate::palette::thumb(&page.item),
    };
    Effect::new(move |_| crate::recent::remember(&username, item.clone()));
}

/// Open the log sheet asked for by `?log=1` once, with the people of `&with=`: the "With..."
/// sheet of a one-click step, or the play sheet.
fn open_asked_sheet(page: &ItemPageDto, model: &str, ui: &ItemUi) {
    let ui = *ui;
    // without the kind (it can arrive after a first render) the sheet is not known yet: the
    // view renders again with it
    if page.entry.is_none() || model.is_empty() {
        return;
    }
    let i18n = use_i18n();
    let step = page.next_step.clone().unwrap_or_default();
    let watched = step_progress(&step).filter(|_| matches!(model, "single" | "episodic"));
    let label = logged_step_label(&i18n, &step);
    let plays = model == "sessions";
    Effect::new(move |_| {
        let Some((with, group)) = ui.auto_log.get() else {
            return;
        };
        ui.auto_log.set(None);
        let (watched, label) = (watched.clone(), label.clone());
        crate::keyboard::when_present("[data-action-bar]", move || {
            if plays {
                ui.play.preset.set(with);
                ui.play.preset_group.set(group);
                ui.play.open.set(true);
            } else if let Some(progress) = watched {
                ui.watched.preset.set(with);
                ui.watched.target.set(Some(SheetTarget { progress, label }));
            }
        });
    });
}

fn info_of(page: &ItemPageDto, kind: Option<&KindDto>, refresh: Callback<()>) -> ItemInfo {
    ItemInfo {
        run_start: crate::item_words::run_start(page),
        verb: kind.and_then(|k| Verb::of_group(&k.group)),
        board_game: page.item.kind == "board_game",
        // the newest play comes first
        last_duration: page.plays.first().and_then(|p| p.play.duration_minutes),
        item_id: page.item.id.clone(),
        title: crate::i18n::title_text(&page.item.title),
        rating: page.entry.as_ref().and_then(|e| e.rating),
        note: page
            .entry
            .as_ref()
            .and_then(|e| e.notes.clone())
            .unwrap_or_default(),
        // what the header counts, for a step that says "again" (review OT2A-1)
        finished: page
            .next_step
            .as_ref()
            .map_or(0, |s| again_count(s, page.times)),
        refresh,
    }
}

#[component]
fn ItemView(
    page: ItemPageDto,
    kind: Option<KindDto>,
    ui: ItemUi,
    lists: Res<Vec<ListDto>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let item = page.item.clone();
    let info = info_of(&page, kind.as_ref(), ui.refresh);
    let model = kind
        .as_ref()
        .map(|k| k.progress_model.clone())
        .unwrap_or_default();
    let loading = item.state == "stub";
    let square = crate::library_state::square_art(&item.kind);
    // the kind badge takes its group's tint (OT-13)
    let tint = kind
        .as_ref()
        .map_or(ui::Tint::Neutral, |k| crate::tint::group_tint(&k.group));
    let play_title = page
        .next_step
        .as_ref()
        .filter(|s| model == "sessions" || s.action == "session")
        .map(|s| next_step_label(&i18n, s))
        .unwrap_or_default();
    // for the log dialogs, which outlive this render
    ui.info.set(Some(info.clone()));
    if ui.play_title.with_untracked(|t| *t != play_title) {
        ui.play_title.set(play_title);
    }
    // the status as a pill, and where the person stands in one line (OT-5, OT-7)
    let pill = page
        .entry
        .as_ref()
        .map(|e| i18n.t(&format!("status-{}", e.status)));
    let fact = page.entry.as_ref().and_then(|e| {
        let facts = Facts {
            verb: info.verb,
            model: &model,
            times: page.times,
            times_at: page.times_at.as_deref(),
            completed: e.status == "completed",
            added_at: &e.added_at,
            summary: &page.summary.clone().unwrap_or_default(),
            step: &page.next_step.clone().unwrap_or_default(),
        };
        // a game's line ends with the time played, when any was logged (ot2f)
        let minutes = page.play_totals.map_or(0, |t| t.minutes);
        fact_line(&i18n, &facts).map(|line| with_play_time(&i18n, line, minutes))
    });
    remember_recent(&page, kind.as_ref());
    open_asked_sheet(&page, &model, &ui);
    view! {
        <Title text=crate::i18n::title_text(&item.title)/>
        <article class="grid gap-6 md:grid-cols-[14rem_1fr] md:gap-8">
            <div class="flex flex-col gap-4">
                <Cover title=crate::i18n::title_text(&item.title) image=item.cover.clone() square=square sizes="(min-width: 768px) 224px, 10rem" class="w-40 md:w-full"/>
                {page.entry.is_some().then(|| view! { <FactsCard page=page.clone() kind=kind.clone() ui=ui/> })}
            </div>
            <div class="flex min-w-0 flex-col gap-5">
                <header class="grid gap-1">
                    <div class="flex flex-wrap gap-1.5">
                        <Badge variant=BadgeVariant::Tint(tint) attr:data-testid="ip-kind"><Icon name=crate::library::kind_icon(&item.kind) class=tint.text()/> {kind_label(&item.kind)}</Badge>
                        {item.year.map(|y| view! { <Badge variant=BadgeVariant::Outline>{y.to_string()}</Badge> })}
                        {pill.map(|p| view! { <Badge attr:data-testid="ip-status">{p}</Badge> })}
                    </div>
                    <h1 class="text-2xl font-semibold tracking-tight md:text-3xl">{crate::i18n::title_text(&item.title)}</h1>
                    {fact.map(|f| view! { <p class="text-sm text-muted-foreground" data-testid="item-fact-line">{f}</p> })}
                    {item.original_title.clone().filter(|o| o != &item.title).map(|o| view! {
                        <p class="text-sm text-muted-foreground">{use_i18n().t_args("item-original-title", &[("title", o.into())])}</p>
                    })}
                </header>
                {loading.then(|| view! { <p class="text-sm text-muted-foreground" aria-live="polite">{t("item-loading-details")}</p> })}
                <ActionBar page=page.clone() kind=kind.clone() info=info.clone() ui=ui/>
                {page.entry.is_some().then(|| view! { <ItemRow page=page.clone() kind=kind.clone() ui=ui lists=lists/> })}
                <ItemTabs page=page.clone() kind=kind.clone() info=info.clone() ui=ui/>
                <Attribution page=page.clone()/>
            </div>
        </article>
    }
}

/// Status, progress and rating; added, started, last logged and repeats behind "More
/// facts" (addendum-1 1.3). The status is editable in place.
#[component]
fn FactsCard(page: ItemPageDto, kind: Option<KindDto>, ui: ItemUi) -> impl IntoView {
    let i18n = use_i18n();
    let Some(entry) = page.entry.clone() else {
        return ().into_any();
    };
    let summary = page.summary.clone().unwrap_or_default();
    let step = page.next_step.clone().unwrap_or_default();
    let verb = kind.as_ref().and_then(|k| Verb::of_group(&k.group));
    let progress = progress_text(&i18n, &summary, &step).unwrap_or_else(|| {
        if summary.events == 0 {
            i18n.t("item-not-started")
        } else if page.times > 0 {
            times_text(&i18n, verb, page.times)
        } else {
            next_step_label(&i18n, &step)
        }
    });
    let rating = entry.rating.map_or_else(
        || i18n.t("item-not-rated"),
        |r| i18n.t_args("item-rated", &[("stars", (f64::from(r) / 20.0).into())]),
    );
    let statuses = kind.map_or_else(|| vec![entry.status.clone()], |k| k.statuses);
    let options: Vec<SelectOption> = statuses
        .iter()
        .map(|s| SelectOption {
            value: s.clone(),
            label: i18n.t(&format!("status-{s}")),
        })
        .collect();
    let status = RwSignal::new(entry.status.clone());
    let ctx = StepCtx::current();
    let item = (
        page.item.id.clone(),
        crate::i18n::title_text(&page.item.title),
    );
    Effect::new(move |prev: Option<String>| {
        let now = status.get();
        if prev.as_ref().is_some_and(|p| *p != now) {
            set_status(&ctx, item.clone(), now.clone(), ui.refresh);
        }
        now
    });
    // "Rewatches: N" is gone (OT-7): the header counts in words, "Watched 3 times"
    let plays = (summary.sessions > 0).then_some(summary.sessions);
    let row = "flex items-baseline justify-between gap-3";
    view! {
        <section class="grid gap-3 rounded-lg border border-border bg-card p-4 text-sm" data-testid="item-facts" aria-label=t("item-facts")>
            <div class="grid gap-1">
                <Label r#for="entry-status">{t("item-status")}</Label>
                <Select id="entry-status" value=status options=Signal::derive(move || options.clone())/>
            </div>
            <div class=row><span class="text-muted-foreground">{t("item-progress")}</span><span class="text-right">{progress}</span></div>
            <div class=row><span class="text-muted-foreground">{t("item-rating")}</span><span>{rating}</span></div>
            <Disclosure id="item-more-facts" title=t("item-more-facts") summary=t("item-more-facts-summary")
                mode=DisclosureMode::Native class="-mx-2 border-0 bg-transparent">
                <dl class="grid gap-1.5">
                    <div class=row><dt class="text-muted-foreground">{t("item-added")}</dt><dd>{use_i18n().date(&entry.added_at)}</dd></div>
                    {summary.first_at.clone().map(|d| view! { <div class=row><dt class="text-muted-foreground">{use_i18n().t("item-started")}</dt><dd>{use_i18n().date(&d)}</dd></div> })}
                    {summary.last_at.clone().map(|d| view! { <div class=row><dt class="text-muted-foreground">{use_i18n().t("item-last")}</dt><dd>{use_i18n().date(&d)}</dd></div> })}
                    {plays.map(|n| view! { <div class=row><dt class="text-muted-foreground">{use_i18n().t("item-plays")}</dt><dd>{use_i18n().number(f64::from(n))}</dd></div> })}
                </dl>
            </Disclosure>
        </section>
    }
    .into_any()
}

/// Change the status; offline it is queued with a client stamp (WP4-5).
fn set_status(
    ctx: &StepCtx,
    (item_id, title): (String, String),
    status: String,
    refresh: Callback<()>,
) {
    let word = ctx.i18n.t(&format!("status-{status}"));
    let done = ctx
        .i18n
        .t_args("library-status-changed", &[("status", word.into())]);
    let failed = ctx.i18n.t("error-generic");
    let (toasts, ctx) = (ctx.toasts, ctx.clone());
    leptos::task::spawn_local(async move {
        match api::set_status(item_id.clone(), status.clone()).await {
            Ok(_) => toasts.push(done, None, ToastVariant::Default),
            Err(e)
                if crate::offline::network_error(&e)
                    && ctx.queue_field(title, |h| {
                        crate::offline::status_command(&item_id, &status, h)
                    }) => {}
            Err(_) => toasts.push(failed, None, ToastVariant::Destructive),
        }
        refresh.run(());
    });
}

/// What the two rewatch items of the overflow menu share.
#[derive(Clone)]
struct RunMenu {
    ctx: StepCtx,
    /// The item's id and title.
    item: (String, String),
    words: RewatchWords,
    error: RwSignal<Option<String>>,
    refresh: Callback<()>,
}

impl RunMenu {
    /// "Start a rewatch" (`start`) or "Stop the rewatch": the marker is logged, and its
    /// toast has Undo ([`log_run_marker`]).
    fn item(&self, start: bool) -> MenuItem {
        let (label, marker, said) = if start {
            (self.words.start, rewatch_start(), self.words.started)
        } else {
            (self.words.stop, rewatch_stop(), self.words.stopped_toast)
        };
        let said = self.ctx.i18n.t(said);
        let me = self.clone();
        MenuItem {
            label: self.ctx.i18n.t(label),
            on_select: Callback::new(move |()| {
                log_run_marker(
                    me.ctx.clone(),
                    me.item.clone(),
                    (marker.clone(), said.clone()),
                    me.error,
                    Some(me.refresh),
                );
            }),
            destructive: false,
        }
    }
}

/// The overflow menu of the action bar: "Start a rewatch" for a Completed show, every other
/// status of the kind, and "Remove from library" (toast with Undo; history stays). While a
/// rewatch runs, Completed reads "Stop the rewatch" and logs the stop marker instead of the
/// plain status write: the run ends there for good (ot2e, DEC-587, DEC-590).
fn overflow(
    page: &ItemPageDto,
    kind: Option<&KindDto>,
    ui: &ItemUi,
) -> Option<impl IntoView + use<>> {
    let entry = page.entry.as_ref()?;
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let refresh = ui.refresh;
    let error = RwSignal::new(None::<String>);
    let (id, title) = (
        page.item.id.clone(),
        crate::i18n::title_text(&page.item.title),
    );
    let run = RunMenu {
        ctx: ctx.clone(),
        item: (id.clone(), title.clone()),
        words: RewatchWords::of(kind.and_then(|k| Verb::of_group(&k.group))),
        error,
        refresh,
    };
    let running = page
        .summary
        .as_ref()
        .and_then(|s| s.rewatch.as_ref())
        .is_some_and(|r| r.running);
    let mut items: Vec<MenuItem> = kind
        .map(|k| k.statuses.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|s| *s != entry.status)
        .map(|s| {
            if running && s == "completed" {
                return run.item(false);
            }
            let word = i18n.t(&format!("status-{s}"));
            let (ctx, item) = (ctx.clone(), (id.clone(), title.clone()));
            MenuItem {
                label: i18n.t_args("library-mark", &[("status", word.into())]),
                on_select: Callback::new(move |()| {
                    set_status(&ctx, item.clone(), s.clone(), refresh);
                }),
                destructive: false,
            }
        })
        .collect();
    // a Completed show can be gone through again as a run: In progress while it runs
    if entry.status == "completed" && kind.is_some_and(|k| k.progress_model == "episodic") {
        items.insert(0, run.item(true));
    }
    // an item no source answers for: its details are the person's to correct (v1-manual)
    if page.details_editable {
        let manual = expect_context::<crate::manual_item::ManualItem>();
        let item = page.item.clone();
        items.push(MenuItem {
            label: i18n.t("item-edit-details"),
            on_select: Callback::new(move |()| manual.edit(item.clone(), refresh)),
            destructive: false,
        });
    }
    items.push(MenuItem {
        label: i18n.t("item-remove"),
        on_select: Callback::new(move |()| {
            crate::bulk::remove_one(&ctx, id.clone(), &title, error, Some(refresh));
        }),
        destructive: true,
    });
    let label = i18n.t_args(
        "item-actions",
        &[("title", crate::i18n::title_text(&page.item.title).into())],
    );
    Some(view! {
        <DropdownMenu label=label items=items id="item-actions" trigger_class="h-10 px-3"
            trigger=Box::new(|| view! { <Icon name=IconName::More/> }.into_any())/>
        {move || error.get().map(|e| view! { <p role="alert" class="w-full text-sm text-destructive-text">{e}</p> })}
    })
}

const BAR_CARD: &str = "grid gap-3 rounded-lg border border-border bg-card p-4 shadow-sm";

/// The action bar of an item that is not in the library (OT-4): two verbs. The first is the
/// item's first step, "Mark as watched" (one click, logged with Undo; the first log adds the
/// item, the toast says so and its Undo takes the log and the add back) or "Log a play" (its
/// dialog); the second, "Want to watch", adds it as Planned and says so in the shared toast
/// with Undo. A kind whose first step needs a field (a page, a
/// position) has the second button only, as the primary one. One of the two runs at a time:
/// while a log or an add is on its way both buttons wait (review OT2A-8: two clicks said
/// "Added as Planned" for an entry that was Completed), and once one went through they keep
/// waiting (`done`) until the refetched page puts the bar of an item in the library in their
/// place (review OT2B-4: a click in between added or logged a second time).
#[component]
fn AddBar(page: ItemPageDto, info: ItemInfo, ui: ItemUi) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let step = page.next_step.clone().unwrap_or_default();
    let (busy, adding, error) = (
        RwSignal::new(false),
        RwSignal::new(false),
        RwSignal::new(None::<String>),
    );
    let refresh = info.refresh;
    // never cleared: the refetch replaces this bar
    let done = RwSignal::new(false);
    let either = Signal::derive(move || busy.get() || adding.get() || done.get());
    let first = match (step_progress(&step), step.action.as_str()) {
        (Some(progress), _) => {
            let (step, label) = (
                LoggedStep {
                    item_id: info.item_id.clone(),
                    title: info.title.clone(),
                    label: logged_step_label(&i18n, &step),
                    progress,
                    verb: info.verb,
                },
                next_step_label(&i18n, &step),
            );
            let ctx = ctx.clone();
            Some(
                view! {
                    <Button size=ButtonSize::Lg loading=busy disabled=either attr:data-next-step=""
                        on:click=move |_| if !either.get_untracked() {
                            run_first_step(ctx.clone(), step.clone(), busy, error, done, Some(refresh));
                        }>
                        <Icon name=IconName::Check/> {label}
                    </Button>
                }
                .into_any(),
            )
        }
        (None, "session") => {
            let open = ui.play.open;
            Some(view! {
                <Button size=ButtonSize::Lg disabled=adding attr:id=PLAY_OPENER attr:data-next-step="" on:click=move |_| open.set(true)>
                    <Icon name=IconName::Meeple/> {next_step_label(&i18n, &step)}
                </Button>
            }.into_any())
        }
        _ => None,
    };
    let (id, title) = (info.item_id.clone(), info.title.clone());
    let add = move |_| {
        if either.get_untracked() {
            return;
        }
        let (id, title, ctx) = (id.clone(), title.clone(), ctx.clone());
        adding.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            match api::add_to_library(id.clone()).await {
                Ok(_) => {
                    done.set(true);
                    toast_added(&ctx, id, title, Some(refresh));
                }
                Err(e) => error.set(Some(
                    ctx.i18n
                        .t_args("library-step-failed", &[("error", error_text(&e).into())]),
                )),
            }
            adding.set(false);
            refresh.run(());
        });
    };
    // alone it is the primary action, and the one the W key runs
    let alone = first.is_none();
    let variant = if alone {
        ButtonVariant::Default
    } else {
        ButtonVariant::Outline
    };
    view! {
        <section class=BAR_CARD data-testid="action-bar" data-action-bar="" aria-label=t("item-action-bar")>
            <div class="flex flex-wrap items-end gap-2">
                {first}
                <Button size=ButtonSize::Lg variant=variant loading=adding disabled=either attr:data-want=""
                    attr:data-next-step=alone.then_some("") on:click=add>
                    {t(want_key(info.verb))}
                </Button>
                {move || error.get().map(|e| view! { <p role="alert" class="basis-full text-sm text-destructive-text">{e}</p> })}
            </div>
        </section>
    }
}

/// The action bar (design.md 6.3, 3.4): the next step first, "Watched with..." for a sheet,
/// the overflow menu; pages and positions log inline. Sticky above the tab bar on phones. An
/// item that is not in the library has [`AddBar`] instead.
#[component]
fn ActionBar(
    page: ItemPageDto,
    kind: Option<KindDto>,
    info: ItemInfo,
    ui: ItemUi,
) -> impl IntoView {
    let i18n = use_i18n();
    let card = BAR_CARD;
    let Some(step) = page.next_step.clone().filter(|_| page.entry.is_some()) else {
        return view! { <AddBar page=page info=info ui=ui/> }.into_any();
    };
    let summary = page.summary.clone().unwrap_or_default();
    let line = progress_text(&i18n, &summary, &step).or_else(|| {
        (summary.sessions > 0)
            .then(|| i18n.t_args("item-plays-count", &[("count", summary.sessions.into())]))
    });
    let sticky = matches!(
        step.action.as_str(),
        "finish" | "episode" | "caught_up" | "session"
    );
    let controls = match step.action.as_str() {
        "page" => {
            view! { <PagesInline info=info.clone() step=step.clone() draft=ui.inline/> }.into_any()
        }
        "position" => {
            view! { <PositionInline info=info.clone() step=step.clone() draft=ui.inline/> }
                .into_any()
        }
        "session" => {
            let open = ui.play.open;
            view! {
                <Button size=ButtonSize::Lg attr:id=PLAY_OPENER attr:data-next-step="" on:click=move |_| open.set(true)>
                    <Icon name=IconName::Meeple/> {next_step_label(&i18n, &step)}
                </Button>
            }
            .into_any()
        }
        _ => view! { <StepButtons step=step.clone() info=info.clone() ui=ui/> }.into_any(),
    };
    view! {
        <section class=format!("{card} z-20 max-md:sticky max-md:bottom-[calc(var(--tab-bar-height)+0.5rem)]")
            data-testid="action-bar" data-action-bar="" data-sticky-action=sticky.then_some("true") aria-label=t("item-action-bar")>
            <div class="flex flex-wrap items-end gap-2">
                {controls}
                {overflow(&page, kind.as_ref(), &ui)}
            </div>
            {line.map(|l| view! { <p class="text-sm text-muted-foreground">{l}</p> })}
        </section>
    }
    .into_any()
}

/// One-click next step and "Watched with..." (single and episodic models). The button is an
/// imperative ("Mark as watched"); the toast and the dialog's title say it in the past tense
/// (OT-5).
#[component]
fn StepButtons(step: NextStepDto, info: ItemInfo, ui: ItemUi) -> impl IntoView {
    let i18n = use_i18n();
    let label = next_step_label(&i18n, &step);
    let done = logged_step_label(&i18n, &step);
    let Some(progress) = step_progress(&step) else {
        return view! {
            <span class="inline-flex h-10 items-center gap-2 text-sm font-medium"><Icon name=IconName::CheckCheck/> {label}</span>
        }
        .into_any();
    };
    let (busy, error) = (RwSignal::new(false), RwSignal::new(None::<String>));
    let logged = LoggedStep {
        item_id: info.item_id.clone(),
        title: info.title.clone(),
        label: done.clone(),
        progress: progress.clone(),
        verb: info.verb,
    };
    let ctx = StepCtx::current();
    let refresh = info.refresh;
    let target = SheetTarget {
        progress,
        label: done,
    };
    let sheet = ui.watched.target;
    let with_text = i18n.t(with_button_key(info.verb));
    let with = i18n.t_args(
        "item-with-named",
        &[
            ("with", with_text.clone().into()),
            ("title", info.title.clone().into()),
        ],
    );
    view! {
        <Button size=ButtonSize::Lg loading=busy attr:data-next-step="" on:click=move |_| run_step(ctx.clone(), logged.clone(), busy, error, Some(refresh))>
            <Icon name=IconName::Check/> {label.clone()}
        </Button>
        <Button size=ButtonSize::Lg variant=ButtonVariant::Outline attr:id=WITH_OPENER attr:aria-label=with
            on:click=move |_| sheet.set(Some(target.clone()))>
            <Icon name=IconName::Users/> {with_text}
        </Button>
        {move || error.get().map(|e| view! { <p role="alert" class="basis-full text-sm text-destructive-text">{e}</p> })}
    }
    .into_any()
}

/// Rating, tags and lists in one row, each editable in place (design.md 6.3, 5.5).
#[component]
fn ItemRow(
    page: ItemPageDto,
    kind: Option<KindDto>,
    ui: ItemUi,
    lists: Res<Vec<ListDto>>,
) -> impl IntoView {
    let Some(entry) = page.entry.clone() else {
        return ().into_any();
    };
    let rating = RwSignal::new(entry.rating);
    let rate_error = RwSignal::new(None::<String>);
    let (id, title) = (
        page.item.id.clone(),
        crate::i18n::title_text(&page.item.title),
    );
    let ctx = StepCtx::current();
    let stored = entry.rating;
    let rate = Callback::new(move |v: Option<u8>| {
        let (id, title, ctx) = (id.clone(), title.clone(), ctx.clone());
        rate_error.set(None);
        leptos::task::spawn_local(async move {
            // the facts card shows the rating too: refresh it (UX1-10)
            let failed = match api::rate(id.clone(), v).await {
                Ok(_) => {
                    ui.refresh.run(());
                    None
                }
                // offline: queued with a client stamp (WP4-5); a full queue says so itself
                Err(e) if crate::offline::network_error(&e) => {
                    if ctx.queue_field(title, |h| crate::offline::rate_command(&id, v, h)) {
                        None
                    } else {
                        Some(crate::pages::error_text(&e))
                    }
                }
                Err(e) => Some(crate::pages::error_text(&e)),
            };
            // never swallowed (UX4-22): the stars go back and the line says why
            if let Some(text) = failed {
                rating.set(stored);
                rate_error.set(Some(
                    ctx.i18n
                        .t_args("library-step-failed", &[("error", text.into())]),
                ));
            }
        });
    });
    let row = LibraryRowDto {
        entry: entry.clone(),
        item: page.item.clone(),
        group: kind.map(|k| k.group).unwrap_or_default(),
        last_activity: entry.added_at.clone(),
        tags: page.tags.clone(),
        summary: page.summary.clone().unwrap_or_default(),
        next_step: page.next_step.clone().unwrap_or_default(),
    };
    let tags = ui.tags;
    let item_id = page.item.id.clone();
    view! {
        <div class="flex flex-wrap gap-x-8 gap-y-4">
            <div class="grid gap-1" id="ip-rate">
                <span class="text-sm text-muted-foreground">{t("item-rating")}</span>
                <RatingInput value=rating on_change=rate/>
                {move || rate_error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
            </div>
            <div class="grid content-start gap-1">
                <span class="text-sm text-muted-foreground">{t("item-tags")}</span>
                <div class="flex flex-wrap items-center gap-2">
                    <TagBadges tags=page.tags.clone()/>
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:data-shortcut="tags" on:click=move |_| tags.set(Some(row.clone()))>
                        <Icon name=IconName::Tag/> {t("item-edit-tags")}
                    </Button>
                </div>
            </div>
            <Suspense fallback=|| ()>
                {move || lists.get().and_then(Result::ok).map(|ls| view! { <ListsCell lists=ls item_id=item_id.clone() refresh=ui.refresh/> })}
            </Suspense>
        </div>
    }
    .into_any()
}

#[component]
fn ListsCell(lists: Vec<ListDto>, item_id: String, refresh: Callback<()>) -> impl IntoView {
    let i18n = use_i18n();
    let (inside, others): (Vec<ListDto>, Vec<ListDto>) = lists
        .into_iter()
        .partition(|l| l.item_ids.contains(&item_id));
    let ctx = StepCtx::current();
    let items: Vec<MenuItem> = others
        .into_iter()
        .map(|l| {
            let (ctx, item, list) = (ctx.clone(), item_id.clone(), l.id.clone());
            let done = i18n.t_args("item-added-to-list", &[("list", l.name.clone().into())]);
            MenuItem {
                label: l.name,
                on_select: Callback::new(move |()| {
                    let (item, list, done, toasts) =
                        (item.clone(), list.clone(), done.clone(), ctx.toasts);
                    leptos::task::spawn_local(async move {
                        if api::add_to_list(list, item).await.is_ok() {
                            toasts.push(done, None, ToastVariant::Default);
                        }
                        refresh.run(());
                    });
                }),
                destructive: false,
            }
        })
        .collect();
    let has_more = !items.is_empty();
    view! {
        <div class="grid content-start gap-1">
            <span class="text-sm text-muted-foreground">{t("item-lists-title")}</span>
            <div class="flex flex-wrap items-center gap-2">
                {inside.into_iter().map(|l| view! { <Badge variant=BadgeVariant::Outline>{l.name}</Badge> }).collect_view()}
                {has_more.then(|| view! {
                    <DropdownMenu label=use_i18n().t("item-lists") items=items id="item-lists" trigger_class="h-8 px-3 text-sm"/>
                })}
            </div>
        </div>
    }
}

/// Tab values in order: the kind's own first (Episodes of a show, Plays of a game), then
/// About, History, Notes.
fn tab_values(page: &ItemPageDto, kind: Option<&KindDto>) -> Vec<&'static str> {
    let mut v = Vec::new();
    if kind.is_some_and(|k| k.progress_model == "sessions") {
        v.push("plays");
    }
    if kind.is_some_and(|k| k.progress_model == "episodic") && page.entry.is_some() {
        v.push("episodes");
    }
    v.extend(["about", "history"]);
    if page.entry.is_some() {
        v.push("notes");
    }
    v
}

/// The plays of a game as its "Plays" tab counts them: as the header does ("Played 3
/// times"), from the history events (review OT2F-2).
fn play_count(page: &ItemPageDto) -> u32 {
    crate::plays_tab::play_count(page.times, &page.play_totals.unwrap_or_default()).plays
}

/// The tab a page opens on: the first, except that a game without a play opens on About
/// (an empty "Plays" tab says nothing about the game).
fn default_tab(page: &ItemPageDto, values: &[&'static str]) -> &'static str {
    let played = values.first() == Some(&"plays") && play_count(page) > 0;
    match values.first() {
        Some(&"plays") if !played => "about",
        Some(first) => first,
        None => "about",
    }
}

#[component]
fn ItemTabs(page: ItemPageDto, kind: Option<KindDto>, info: ItemInfo, ui: ItemUi) -> impl IntoView {
    let values = tab_values(&page, kind.as_ref());
    // the kind can arrive after a first render without it: a default chosen then is replaced
    // (and so a game's first play brings its "Plays" tab forward, unless a tab was chosen)
    let current = ui.tab.get_untracked();
    if !values.contains(&current.as_str()) || current == ui.auto_tab.get_untracked() {
        let first = default_tab(&page, &values);
        ui.tab.set(first.to_string());
        ui.auto_tab.set(first.to_string());
    }
    let has_plays = values.contains(&"plays");
    let play_count = if has_plays { play_count(&page) } else { 0 };
    let plays_page = page.clone();
    let history_count = page.progress.len();
    let next = page
        .next_step
        .as_ref()
        .filter(|s| s.action == "episode")
        .and_then(|s| Some((s.season?, s.episode?)));
    let sheet = ui.watched.target;
    let open_sheet = Callback::new(move |t: SheetTarget| sheet.set(Some(t)));
    let (has_eps, has_notes) = (values.contains(&"episodes"), values.contains(&"notes"));
    let (about_page, about_kind, history_page, refresh) =
        (page.clone(), kind.clone(), page.clone(), ui.refresh);
    view! {
        <Tabs id="item" active=ui.tab>
            <TabsList label=t("item-sections") class="max-w-full justify-start overflow-x-auto">
                {has_eps.then(|| view! { <TabsTrigger value="episodes">{use_i18n().t("item-section-episodes")}</TabsTrigger> })}
                {has_plays.then(|| view! { <TabsTrigger value="plays">{use_i18n().t("item-section-plays")} " " <span class="text-muted-foreground">{play_count}</span></TabsTrigger> })}
                <TabsTrigger value="about">{t("item-section-about")}</TabsTrigger>
                <TabsTrigger value="history">{t("item-history")} " " <span class="text-muted-foreground">{history_count}</span></TabsTrigger>
                {has_notes.then(|| view! { <TabsTrigger value="notes">{use_i18n().t("item-section-notes")}</TabsTrigger> })}
            </TabsList>
            {has_eps.then(|| {
                let (info, attributes, progress, dates) = (info.clone(), page.item.attributes.clone(), page.progress.clone(), page.episodes.clone());
                // while a rewatch runs the list shows the run; a Completed entry starts one
                // with the answer "Log N again" (DEC-587)
                let run = page.summary.as_ref().and_then(|s| s.rewatch.as_ref()).filter(|r| r.running).map(|r| r.start.clone());
                let status = page.entry.as_ref().map(|e| e.status.clone()).unwrap_or_default();
                view! {
                    <TabsContent value="episodes">
                        <EpisodeList info=info.clone() attributes=attributes.clone() progress=progress.clone() dates=dates.clone() next=next ui=ui.episodes open_sheet=open_sheet
                            run=run.clone() status=status.clone()/>
                    </TabsContent>
                }
            })}
            {has_plays.then(|| view! { <TabsContent value="plays"><crate::plays_tab::PlaysTab page=plays_page.clone()/></TabsContent> })}
            <TabsContent value="about"><About page=about_page.clone() kind=about_kind.clone()/></TabsContent>
            <TabsContent value="history"><ItemHistory page=history_page.clone() refresh=refresh/></TabsContent>
            {has_notes.then(|| {
                let (id, note) = (page.item.id.clone(), info.note.clone());
                view! { <TabsContent value="notes"><Note item_id=id.clone() note=note.clone()/></TabsContent> }
            })}
        </Tabs>
    }
}

/// Text of an attribute value: a string, a number, or a list of them.
fn attribute_text(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Array(a) if !a.is_empty() => {
            let parts: Vec<String> = a.iter().filter_map(attribute_text).collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        _ => None,
    }
}

/// Description, the kind's facets with their values, the provider ids.
#[component]
fn About(page: ItemPageDto, kind: Option<KindDto>) -> impl IntoView {
    let i18n = use_i18n();
    let item = page.item;
    let facts: Vec<(String, String)> = kind
        .map(|k| k.facets)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|f| {
            Some((
                i18n.t(&format!("library-facet-{f}")),
                attribute_text(item.attributes.get(&f)?)?,
            ))
        })
        .collect();
    let empty = item.description.is_none() && facts.is_empty();
    view! {
        <div class="grid gap-4 py-2">
            {item.description.clone().map(|d| view! { <p class="max-w-prose">{d}</p> })}
            {(!facts.is_empty()).then(|| view! {
                <dl class="grid max-w-prose grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
                    {facts.into_iter().map(|(k, v)| view! { <dt class="text-muted-foreground">{k}</dt><dd>{v}</dd> }).collect_view()}
                </dl>
            })}
            {empty.then(|| view! { <p class="text-sm text-muted-foreground">{use_i18n().t("item-about-empty")}</p> })}
            {(!item.external_ids.is_empty()).then(|| view! {
                <p class="text-xs text-muted-foreground">
                    {use_i18n().t("item-ids")} " "
                    {item.external_ids.iter().map(|e| format!("{}: {}", e.namespace, e.value)).collect::<Vec<_>>().join(" - ")}
                </p>
            })}
        </div>
    }
}

/// This item's events, newest first, as activity rows. A play's row carries its people, their
/// scores and the duration from `page.plays` (OT-11), found by the event's session.
fn history_rows(page: &ItemPageDto, title: &str) -> Vec<ActivityRowDto> {
    page.progress
        .iter()
        .rev()
        .map(|p| {
            let play = p.progress["session_id"]
                .as_str()
                .and_then(|id| page.plays.iter().find(|x| x.session_id == id));
            ActivityRowDto {
                at: p.occurred_at.clone(),
                // the local day and time are filled by the server for feed rows (WP3-4 uses them)
                local_day: String::new(),
                local_time: String::new(),
                row_type: "progress".into(),
                item_id: page.item.id.clone(),
                title: title.to_string(),
                kind: page.item.kind.clone(),
                group: String::new(),
                progress: Some(p.progress.clone()),
                event_id: Some(p.id.clone()),
                session_id: play.map(|x| x.session_id.clone()),
                participants: play.map(|x| x.participants.clone()).unwrap_or_default(),
                play: play.map(|x| x.play.clone()),
                shared: false,
                owner_name: None,
                // the item page has no recorded-at stamp; Undo stays on the toast and History
                undoable: false,
            }
        })
        .collect()
}

/// The History tab: this item's events.
#[component]
fn ItemHistory(page: ItemPageDto, refresh: Callback<()>) -> impl IntoView {
    let rows = history_rows(&page, &crate::i18n::title_text(&page.item.title));
    if rows.is_empty() {
        return view! { <p class="py-4 text-sm text-muted-foreground">{t("item-history-empty")}</p> }.into_any();
    }
    view! {
        <ul class="grid gap-2 py-2">
            {rows.into_iter().map(|row| view! { <ActivityRow row=row dated=true on_change=refresh/> }).collect_view()}
        </ul>
    }
    .into_any()
}

/// The private note of the entry (design.md 5.5); only the owner sees it.
#[component]
fn Note(item_id: String, note: String) -> impl IntoView {
    let i18n = use_i18n();
    let value = RwSignal::new(note);
    let (busy, status, error) = (
        RwSignal::new(false),
        RwSignal::new(None::<String>),
        RwSignal::new(None::<String>),
    );
    let (saved, failed) = (i18n.t("item-note-saved"), i18n.t("error-generic"));
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (id, text) = (item_id.clone(), value.get_untracked());
        let (saved, failed) = (saved.clone(), failed.clone());
        busy.set(true);
        status.set(None);
        error.set(None);
        leptos::task::spawn_local(async move {
            let note = Some(text.trim().to_string()).filter(|n| !n.is_empty());
            match api::set_entry_note(id, note).await {
                Ok(_) => status.set(Some(saved)),
                Err(_) => error.set(Some(failed)),
            }
            busy.set(false);
        });
    };
    view! {
        <form class="grid max-w-prose gap-3 py-2" on:submit=save>
            <FormField id="entry-note" label=t("item-note") description=t("item-note-hint")
                status=Signal::derive(move || status.get()) error=Signal::derive(move || error.get())>
                <Textarea id="entry-note" value=value rows=6u32 described_by="entry-note-description"/>
            </FormField>
            <div><Button r#type="submit" loading=busy>{t("item-note-save")}</Button></div>
        </form>
    }
}

#[component]
fn Attribution(page: ItemPageDto) -> impl IntoView {
    view! {
        <footer class="mt-2 border-t border-border pt-4 text-xs text-muted-foreground" data-testid="attribution">
            {page.sources.iter().map(|s| view! {
                <p>
                    {use_i18n().t_args("item-source", &[("name", s.name.clone().into())])} " - "
                    // the attribution text is the provider's own wording, in English
                    <span lang="en">{s.text.clone()}</span> " " <a href=s.url.clone() class="text-primary underline" rel="noopener">{s.url.clone()}</a>
                </p>
            }).collect_view()}
        </footer>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// OT-11: the History tab built its rows without people; a play's row now carries the
    /// people, scores and duration of its session, other rows stay as they were.
    #[test]
    fn the_history_rows_of_plays_carry_their_people_and_scores() {
        let page: ItemPageDto = serde_json::from_value(serde_json::json!({
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
                 "progress": {"type": "finished"}}
            ],
            "sources": [],
            "plays": [{
                "session_id": "s2",
                "play": {"duration_minutes": 45, "coop": null},
                "participants": [
                    {"companion_id": "a", "name": "Me", "is_me": true, "score": 31.0, "winner": false},
                    {"companion_id": "b", "name": "Ben", "is_me": false, "score": 42.5, "winner": true}
                ]
            }]
        }))
        .unwrap();
        let rows = history_rows(&page, "Azul");
        // newest first
        let ids: Vec<_> = rows.iter().map(|r| r.event_id.as_deref()).collect();
        assert_eq!(ids, [Some("e3"), Some("e2"), Some("e1")]);
        assert_eq!(rows[0].play, None);
        assert_eq!(rows[0].participants.len(), 0);
        let play = &rows[1];
        assert_eq!(play.session_id.as_deref(), Some("s2"));
        assert_eq!(play.play.as_ref().unwrap().duration_minutes, Some(45));
        assert_eq!(play.participants.len(), 2);
        assert_eq!(play.participants[1].score, Some(42.5));
        assert!(play.participants[1].winner);
        // a play the page did not get (older than the newest 50): the plain row
        assert_eq!(rows[2].play, None);
        assert_eq!(rows[2].session_id, None);
    }
}
