//! The Episodes tab (design.md 6.3, 5.4, 3.4 R-07): season chips with watched/total, "Mark
//! season watched", the episode rows with check circles, the next-up row highlighted, and a
//! row menu ("Mark up to here", "Watched with..."). Marking one episode while earlier ones
//! are open asks once, inline, whether to mark those too; "Remember for this show" keeps the
//! answer in this browser (DEC-231). The ranges come from [`crate::episode_plan`].

use contracts::ProgressEventDto;
use leptos::html;
use leptos::prelude::*;
use ui::{
    Button, ButtonSize, ButtonVariant, Checkbox, Chip, DropdownMenu, Icon, IconName, Label,
    MenuItem,
};

use crate::episode_plan::{
    Ep, Plan, Previous, SeasonRow, plan_single, previous_key, unwatched_in_season, unwatched_up_to,
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
}

impl Default for EpisodeUi {
    fn default() -> Self {
        Self {
            season: RwSignal::new(None),
            ask: RwSignal::new(None),
            busy: RwSignal::new(false),
            error: RwSignal::new(None),
        }
    }
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

/// Log `eps` one after the other, then one toast with Undo for all of them.
fn log_episodes(eps: Vec<Ep>, info: &ItemInfo, ctx: StepCtx, ui: EpisodeUi) {
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
    ui.busy.set(true);
    ui.error.set(None);
    leptos::task::spawn_local(async move {
        let mut ids = Vec::new();
        let mut failed = None;
        for (season, episode) in eps {
            let p = contracts::NewProgressDto {
                progress: serde_json::json!({"type": "episode", "season": season, "episode": episode}),
                occurred_at: None,
                client_op_id: Some(uuid::Uuid::now_v7().to_string()),
                with: Vec::new(),
            };
            match crate::api::log_progress(item_id.clone(), p).await {
                Ok(ev) => ids.push(ev.id),
                Err(e) => {
                    failed = Some(crate::pages::error_text(&e));
                    break;
                }
            }
        }
        if !ids.is_empty() {
            toast_logged(&ctx, &label, title, ids, Some(refresh));
        }
        if let Some(text) = failed {
            ui.error.set(Some(
                ctx.i18n
                    .t_args("library-step-failed", &[("error", text.into())]),
            ));
        }
        ui.busy.set(false);
        refresh.run(());
    });
}

/// The Episodes tab.
#[component]
pub fn EpisodeList(
    info: ItemInfo,
    attributes: serde_json::Value,
    progress: Vec<ProgressEventDto>,
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
    let ctx = StepCtx::current();
    let c2 = ctx.clone();
    let mark_season = move |_| {
        let eps = rows.with_value(|r| unwatched_in_season(r, shown.get_untracked()));
        info.with_value(|i| log_episodes(eps, i, c2.clone(), ui));
    };
    let c3 = ctx.clone();
    let mark = Callback::new(move |(target, up_to): (Ep, bool)| {
        let plan = if up_to {
            Plan::Log(rows.with_value(|r| unwatched_up_to(r, target)))
        } else {
            let answer = info.with_value(|i| stored_answer(&i.item_id));
            rows.with_value(|r| plan_single(r, target, answer))
        };
        match plan {
            Plan::Log(eps) => info.with_value(|i| log_episodes(eps, i, c3.clone(), ui)),
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
            info.with_value(|i| log_episodes(eps, i, ctx.clone(), ui));
        }
    });
    view! {
        <section class="grid gap-3 py-2">
            <div class="flex flex-wrap items-center justify-between gap-2">
                <SeasonChips rows=rows.get_value() shown=shown ui=ui/>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm disabled=ui.busy on:click=mark_season>
                    {t("episodes-mark-season")}
                </Button>
            </div>
            <AskPrevious ui=ui rows=rows answer=answer/>
            {move || ui.error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
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
