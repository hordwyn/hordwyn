//! The item page `/items/:id` (design.md 6.3, 3.4, 5.4, 5.5; addendum-1 1.3 Item row): cover
//! with the entry facts card (status, progress, rating open; the rest behind "More facts"),
//! title block, the action bar per progress model with the next step first, the rating /
//! tags / lists row, and tabs (Episodes where the kind has them, About, History, Notes).
//! The sheets, the open tab and the episode question live here, outside the view that a
//! refetch rebuilds.

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
use crate::log_sheet::{
    ItemInfo, PagesInline, PlaySheet, PlayState, PositionInline, SheetTarget, WatchedSheet,
    WatchedState,
};
use crate::next_step::{
    LoggedStep, StepCtx, next_step_label, progress_text, run_step, step_progress,
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
    companions: RwSignal<Vec<CompanionDto>>,
    tags: RwSignal<Option<LibraryRowDto>>,
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
    // of the previous render, whose state was already disposed
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
            && topic == "entity.updated"
            && mentions(&payload, &id())
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
        companions,
        tags: RwSignal::new(None),
        refresh: Callback::new(move |()| refetch()),
    };
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
            <For each=move || data.get().map(|r| (version.get_untracked(), r, kinds.get().and_then(Result::ok))) key=|(v, _, _)| *v
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
    }
}

fn info_of(page: &ItemPageDto, refresh: Callback<()>) -> ItemInfo {
    ItemInfo {
        item_id: page.item.id.clone(),
        title: page.item.title.clone(),
        rating: page.entry.as_ref().and_then(|e| e.rating),
        note: page
            .entry
            .as_ref()
            .and_then(|e| e.notes.clone())
            .unwrap_or_default(),
        finished: page.summary.as_ref().map_or(0, |s| s.finished),
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
    let info = info_of(&page, ui.refresh);
    let model = kind
        .as_ref()
        .map(|k| k.progress_model.clone())
        .unwrap_or_default();
    let loading = item.state == "stub";
    let square = crate::library_state::square_art(&item.kind);
    let play_title = page.next_step.as_ref().map(|s| next_step_label(&i18n, s));
    view! {
        <Title text=item.title.clone()/>
        <article class="grid gap-6 md:grid-cols-[14rem_1fr] md:gap-8">
            <div class="flex flex-col gap-4">
                <Cover title=item.title.clone() image=item.cover.clone() square=square sizes="(min-width: 768px) 224px, 10rem" class="w-40 md:w-full"/>
                {page.entry.is_some().then(|| view! { <FactsCard page=page.clone() kind=kind.clone() ui=ui/> })}
            </div>
            <div class="flex min-w-0 flex-col gap-5">
                <header class="grid gap-1">
                    <div class="flex flex-wrap gap-1.5">
                        <Badge variant=BadgeVariant::Secondary><Icon name=crate::library::kind_icon(&item.kind)/> {kind_label(&item.kind)}</Badge>
                        {item.year.map(|y| view! { <Badge variant=BadgeVariant::Outline>{y.to_string()}</Badge> })}
                    </div>
                    <h1 class="text-2xl font-semibold tracking-tight md:text-3xl">{item.title.clone()}</h1>
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
        {matches!(model.as_str(), "single" | "episodic").then(|| view! {
            <WatchedSheet state=ui.watched info=info.clone() companions=ui.companions/>
        })}
        {(model == "sessions").then(|| view! {
            <PlaySheet state=ui.play info=info.clone() title=play_title.unwrap_or_default()/>
        })}
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
    let progress = progress_text(&i18n, &summary, &step).unwrap_or_else(|| {
        if summary.events == 0 {
            i18n.t("item-not-started")
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
    let item_id = page.item.id.clone();
    Effect::new(move |prev: Option<String>| {
        let now = status.get();
        if prev.as_ref().is_some_and(|p| *p != now) {
            set_status(&ctx, item_id.clone(), now.clone(), ui.refresh);
        }
        now
    });
    let repeats = if summary.sessions > 0 {
        (i18n.t("item-plays"), summary.sessions)
    } else {
        (i18n.t("item-rewatches"), summary.finished.saturating_sub(1))
    };
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
                    <div class=row><dt class="text-muted-foreground">{repeats.0}</dt><dd>{use_i18n().number(f64::from(repeats.1))}</dd></div>
                </dl>
            </Disclosure>
        </section>
    }
    .into_any()
}

fn set_status(ctx: &StepCtx, item_id: String, status: String, refresh: Callback<()>) {
    let word = ctx.i18n.t(&format!("status-{status}"));
    let done = ctx
        .i18n
        .t_args("library-status-changed", &[("status", word.into())]);
    let failed = ctx.i18n.t("error-generic");
    let toasts = ctx.toasts;
    leptos::task::spawn_local(async move {
        match api::set_status(item_id, status).await {
            Ok(_) => toasts.push(done, None, ToastVariant::Default),
            Err(_) => toasts.push(failed, None, ToastVariant::Destructive),
        }
        refresh.run(());
    });
}

/// The overflow menu of the action bar: every other status of the kind.
fn overflow(
    page: &ItemPageDto,
    kind: Option<&KindDto>,
    ui: &ItemUi,
) -> Option<impl IntoView + use<>> {
    let entry = page.entry.as_ref()?;
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let refresh = ui.refresh;
    let items: Vec<MenuItem> = kind
        .map(|k| k.statuses.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|s| *s != entry.status)
        .map(|s| {
            let word = i18n.t(&format!("status-{s}"));
            let (ctx, id) = (ctx.clone(), page.item.id.clone());
            MenuItem {
                label: i18n.t_args("library-mark", &[("status", word.into())]),
                on_select: Callback::new(move |()| {
                    set_status(&ctx, id.clone(), s.clone(), refresh);
                }),
                destructive: false,
            }
        })
        .collect();
    let label = i18n.t_args("item-actions", &[("title", page.item.title.clone().into())]);
    Some(view! {
        <DropdownMenu label=label items=items id="item-actions" trigger_class="h-10 px-3"
            trigger=Box::new(|| view! { <Icon name=IconName::More/> }.into_any())/>
    })
}

/// The action bar (design.md 6.3, 3.4): the next step first, "With..." for a sheet, the
/// overflow menu; pages and positions log inline. Sticky above the tab bar on phones.
#[component]
fn ActionBar(
    page: ItemPageDto,
    kind: Option<KindDto>,
    info: ItemInfo,
    ui: ItemUi,
) -> impl IntoView {
    let i18n = use_i18n();
    let card = "grid gap-3 rounded-lg border border-border bg-card p-4 shadow-sm";
    let Some(step) = page.next_step.clone().filter(|_| page.entry.is_some()) else {
        let busy = RwSignal::new(false);
        let id = page.item.id.clone();
        let refresh = ui.refresh;
        let add = move |_| {
            let id = id.clone();
            busy.set(true);
            leptos::task::spawn_local(async move {
                let _ = api::add_to_library(id).await;
                busy.set(false);
                refresh.run(());
            });
        };
        return view! {
            <section class=card data-testid="action-bar" aria-label=t("item-action-bar")>
                <div><Button size=ButtonSize::Lg loading=busy on:click=add>{t("item-add")}</Button></div>
            </section>
        }
        .into_any();
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
        "page" => view! { <PagesInline info=info.clone() step=step.clone()/> }.into_any(),
        "position" => view! { <PositionInline info=info.clone() step=step.clone()/> }.into_any(),
        "session" => {
            let open = ui.play.open;
            view! {
                <Button size=ButtonSize::Lg on:click=move |_| open.set(true)>
                    <Icon name=IconName::Meeple/> {next_step_label(&i18n, &step)}
                </Button>
            }
            .into_any()
        }
        _ => view! { <StepButtons step=step.clone() info=info.clone() ui=ui/> }.into_any(),
    };
    view! {
        <section class=format!("{card} z-20 max-md:sticky max-md:bottom-[calc(var(--tab-bar-height)+0.5rem)]")
            data-testid="action-bar" data-sticky-action=sticky.then_some("true") aria-label=t("item-action-bar")>
            <div class="flex flex-wrap items-end gap-2">
                {controls}
                {overflow(&page, kind.as_ref(), &ui)}
            </div>
            {line.map(|l| view! { <p class="text-sm text-muted-foreground">{l}</p> })}
        </section>
    }
    .into_any()
}

/// One-click next step and "With..." (single and episodic models).
#[component]
fn StepButtons(step: NextStepDto, info: ItemInfo, ui: ItemUi) -> impl IntoView {
    let i18n = use_i18n();
    let label = next_step_label(&i18n, &step);
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
        label: label.clone(),
        progress: progress.clone(),
    };
    let ctx = StepCtx::current();
    let refresh = info.refresh;
    let target = SheetTarget {
        progress,
        label: label.clone(),
    };
    let sheet = ui.watched.target;
    let with = i18n.t_args("item-with-named", &[("title", info.title.clone().into())]);
    view! {
        <Button size=ButtonSize::Lg loading=busy on:click=move |_| run_step(ctx.clone(), logged.clone(), busy, error, Some(refresh))>
            <Icon name=IconName::Check/> {label.clone()}
        </Button>
        <Button size=ButtonSize::Lg variant=ButtonVariant::Outline attr:aria-label=with
            on:click=move |_| sheet.set(Some(target.clone()))>
            <Icon name=IconName::Users/> {t("item-with")}
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
    let id = page.item.id.clone();
    let rate = Callback::new(move |v: Option<u8>| {
        let id = id.clone();
        leptos::task::spawn_local(async move {
            // the facts card shows the rating too: refresh it (UX1-10)
            if api::rate(id, v).await.is_ok() {
                ui.refresh.run(());
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
            <div class="grid gap-1">
                <span class="text-sm text-muted-foreground">{t("item-rating")}</span>
                <RatingInput value=rating on_change=rate/>
            </div>
            <div class="grid content-start gap-1">
                <span class="text-sm text-muted-foreground">{t("item-tags")}</span>
                <div class="flex flex-wrap items-center gap-2">
                    <TagBadges tags=page.tags.clone()/>
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Sm on:click=move |_| tags.set(Some(row.clone()))>
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

/// Tab values in order: the kind's own first (Episodes), then About, History, Notes.
fn tab_values(page: &ItemPageDto, kind: Option<&KindDto>) -> Vec<&'static str> {
    let mut v = Vec::new();
    if kind.is_some_and(|k| k.progress_model == "episodic") && page.entry.is_some() {
        v.push("episodes");
    }
    v.extend(["about", "history"]);
    if page.entry.is_some() {
        v.push("notes");
    }
    v
}

#[component]
fn ItemTabs(page: ItemPageDto, kind: Option<KindDto>, info: ItemInfo, ui: ItemUi) -> impl IntoView {
    let values = tab_values(&page, kind.as_ref());
    // the kind can arrive after a first render without it: a default chosen then is replaced
    let current = ui.tab.get_untracked();
    if !values.contains(&current.as_str()) || current == ui.auto_tab.get_untracked() {
        ui.tab.set(values[0].to_string());
        ui.auto_tab.set(values[0].to_string());
    }
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
                <TabsTrigger value="about">{t("item-section-about")}</TabsTrigger>
                <TabsTrigger value="history">{t("item-history")} " " <span class="text-muted-foreground">{history_count}</span></TabsTrigger>
                {has_notes.then(|| view! { <TabsTrigger value="notes">{use_i18n().t("item-section-notes")}</TabsTrigger> })}
            </TabsList>
            {has_eps.then(|| {
                let (info, attributes, progress) = (info.clone(), page.item.attributes.clone(), page.progress.clone());
                view! {
                    <TabsContent value="episodes">
                        <EpisodeList info=info.clone() attributes=attributes.clone() progress=progress.clone() next=next ui=ui.episodes open_sheet=open_sheet/>
                    </TabsContent>
                }
            })}
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

/// This item's events, newest first, as activity rows.
#[component]
fn ItemHistory(page: ItemPageDto, refresh: Callback<()>) -> impl IntoView {
    let rows: Vec<ActivityRowDto> = page
        .progress
        .iter()
        .rev()
        .map(|p| ActivityRowDto {
            at: p.occurred_at.clone(),
            row_type: "progress".into(),
            item_id: page.item.id.clone(),
            title: page.item.title.clone(),
            kind: page.item.kind.clone(),
            group: String::new(),
            progress: Some(p.progress.clone()),
            event_id: Some(p.id.clone()),
            session_id: None,
            participants: Vec::new(),
            shared: false,
            owner_name: None,
            // the item page has no recorded-at stamp; Undo stays on the toast and History
            undoable: false,
        })
        .collect();
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
                    <span lang="en">{s.text.clone()}</span> " " <a href=s.url.clone() class="underline" rel="noopener">{s.url.clone()}</a>
                </p>
            }).collect_view()}
        </footer>
    }
}
