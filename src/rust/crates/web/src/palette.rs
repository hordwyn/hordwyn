//! Ctrl-K search palette (D-6.6, design.md 5.3, 6.4): local results first, then each
//! connector's results as its `search.results` event arrives for the current search id. The
//! footer shows the source status (dots with words: which groups have no source, which
//! sources fail) and an empty search ends in a card that says why (design.md 4.4), with "Ask
//! admin" for members and "Set up" for admins. Below two characters it lists the recently
//! opened items and suggested commands; Ctrl/Cmd+Enter adds a result as Planned (toast with
//! Undo) or runs the next step of an entry, and the palette stays open (WP4-4). A
//! [`PaletteTarget`] makes choosing a result do something else for one opening.

use contracts::admin::{SkippedSourceDto, SourceStatusPageDto};
use contracts::{CatalogItemDto, SearchResultsEventDto};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use ui::{Command, CommandHints, CommandItem, Dialog, Segmented, SegmentedOption, StatusDot};

use crate::admin_api;
use crate::admin_state::source_state;
use crate::api;
use crate::events::use_events;
use crate::i18n::{I18n, t, use_i18n};
use crate::next_step::{
    LoggedStep, StepCtx, logged_step_label, run_step, step_progress, toast_added,
};
use crate::recent::RecentItem;
use crate::settings::AskAdmin;
use crate::source_hints::{
    FooterItem, HintAction, NoResults, SET_UP_HREF, footer_items, no_results,
};

/// Whether the palette is open (shared with the header button).
#[derive(Clone, Copy)]
pub struct PaletteOpen(pub RwSignal<bool>);

/// An unmatched import row the palette is matching (Settings > Imports, design.md 6.11):
/// choosing a result matches the row with it and replays the row; `done` hears the outcome
/// (`None` on success, else the message). The palette opens with the row's title as query.
#[derive(Clone)]
pub struct MatchTarget {
    pub import_id: String,
    pub source_id: String,
    pub title: String,
    pub done: Callback<Option<String>>,
}

/// What choosing a result does instead of opening it, for one opening of the palette (the
/// scoped pick; DEC-344, DEC-353).
#[derive(Clone)]
pub enum PaletteTarget {
    /// Add it to this list: the list page's "Add" (design.md 6.5).
    List { id: String, name: String },
    /// Match an unmatched import row with it.
    Match(MatchTarget),
    /// Make it the subject of a notification rule ("Add a rule for one item"): `done` gets
    /// the item id and its label.
    RuleSubject { done: Callback<(String, String)> },
    /// Log it with this person (a People card): the item page opens its log sheet with them.
    LogWith { companion_id: String, name: String },
    /// Log a play of it as this game group (People > Game groups, WP4-6): the item page opens
    /// its play sheet with the group chosen and its members as players.
    LogWithGroup { group_id: String, name: String },
}

/// The palette's target, if any; cleared when the palette closes.
#[derive(Clone, Copy)]
pub struct PaletteScope(pub RwSignal<Option<PaletteTarget>>);

/// Match the row with the chosen item (`POST /imports/{id}/match`).
fn match_row(m: MatchTarget, item: CommandItem) {
    leptos::task::spawn_local(async move {
        let path = format!("/imports/{}/match", m.import_id);
        let body = serde_json::json!({"source_id": m.source_id, "item_id": item.id});
        let r = crate::rest::send("POST", &path, Some(body)).await;
        m.done
            .run(r.err().map(|e| crate::settings::admin_error(&e)));
    });
}

/// Add the chosen item to the list the palette is scoped to; the answer is a toast.
fn add_to_list(
    (toasts, i18n): (ui::Toasts, std::sync::Arc<I18n>),
    list: String,
    name: String,
    item: CommandItem,
) {
    leptos::task::spawn_local(async move {
        let path = format!("/lists/{list}/items");
        let body = serde_json::json!({"item_id": item.id});
        match crate::rest::send("POST", &path, Some(body)).await {
            Ok(()) => toasts.push(
                i18n.t_args(
                    "lists-added",
                    &[("title", item.label.into()), ("name", name.into())],
                ),
                None,
                ui::ToastVariant::Default,
            ),
            Err(e) => toasts.push(
                crate::sharing::share_error(&e),
                None,
                ui::ToastVariant::Destructive,
            ),
        }
    });
}

/// The line above the input naming what choosing a result does (test id, text).
fn target_line(i18n: &I18n, target: &PaletteTarget) -> (&'static str, String) {
    match target {
        PaletteTarget::List { name, .. } => (
            "palette-target",
            i18n.t_args("lists-add-target", &[("name", name.clone().into())]),
        ),
        PaletteTarget::Match(m) => (
            "palette-match",
            i18n.t_args("imports-match-target", &[("title", m.title.clone().into())]),
        ),
        PaletteTarget::RuleSubject { .. } => ("palette-target", i18n.t("search-target-rule")),
        PaletteTarget::LogWith { name, .. } => (
            "palette-target",
            i18n.t_args("search-target-log-with", &[("name", name.clone().into())]),
        ),
        PaletteTarget::LogWithGroup { name, .. } => (
            "palette-target",
            i18n.t_args("search-target-log-group", &[("name", name.clone().into())]),
        ),
    }
}

/// Ids of the suggested commands (item ids are UUIDs, so they never collide).
const CMD_LISTS: &str = "cmd:lists";
const CMD_NEW_LIST: &str = "cmd:new-list";
const CMD_IMPORT: &str = "cmd:import";
const CMD_PLAY: &str = "cmd:play:";

/// Where a suggested command goes; `None` for an item.
#[must_use]
pub fn command_href(id: &str) -> Option<String> {
    match id {
        CMD_LISTS => Some("/lists".into()),
        CMD_NEW_LIST => Some("/lists?new=1".into()),
        CMD_IMPORT => Some("/settings/imports".into()),
        _ => id
            .strip_prefix(CMD_PLAY)
            .map(|item| format!("/items/{item}?log=1")),
    }
}

/// What the palette lists below two characters (design.md 5.3): the recent items, then the
/// suggested commands (not while a target is set: they would leave it). "Log a play of ..."
/// names the newest recent item whose kind logs plays. Signed out nothing (UX4-15): the
/// commands and recent items belong to an account.
#[must_use]
pub fn idle_items(
    i18n: &I18n,
    recent: &[RecentItem],
    commands: bool,
    signed_in: bool,
) -> Vec<CommandItem> {
    if !signed_in {
        return Vec::new();
    }
    let row = |id: String, label: String, group: &str| CommandItem {
        id,
        label,
        group: group.to_string(),
        keywords: String::new(),
    };
    let recent_group = i18n.t("search-recent");
    let mut out: Vec<CommandItem> = recent
        .iter()
        .map(|r| row(r.id.clone(), r.label.clone(), &recent_group))
        .collect();
    if commands {
        let g = i18n.t("search-suggested");
        out.push(row(CMD_LISTS.into(), i18n.t("search-cmd-lists"), &g));
        out.push(row(CMD_NEW_LIST.into(), i18n.t("search-cmd-new-list"), &g));
        if let Some(game) = recent.iter().find(|r| r.plays) {
            let label = i18n.t_args(
                "search-cmd-log-play",
                &[("title", game.label.clone().into())],
            );
            out.push(row(format!("{CMD_PLAY}{}", game.id), label, &g));
        }
        out.push(row(CMD_IMPORT.into(), i18n.t("search-cmd-import"), &g));
    }
    out
}

/// The quick path (design.md 5.3, 3.4): a result not in the library is added as Planned (the
/// same toast with Undo as the item page's "Want to watch", OT-4); an
/// entry runs its one-click next step; a step that needs input opens the item page. The
/// palette stays open except for that last case; errors show in the palette.
fn quick(
    ctx: StepCtx,
    item: CommandItem,
    (busy, error): (RwSignal<bool>, RwSignal<Option<String>>),
    open_page: Callback<String>,
) {
    error.set(None);
    leptos::task::spawn_local(async move {
        let failed = |e: &ServerFnError| {
            ctx.i18n.t_args(
                "library-step-failed",
                &[("error", crate::pages::error_text(e).into())],
            )
        };
        match api::item_page(item.id.clone()).await {
            Ok(page) if page.entry.is_some() => {
                let step = page.next_step.unwrap_or_default();
                match step_progress(&step) {
                    Some(progress) => {
                        let logged = LoggedStep {
                            item_id: item.id,
                            title: page.item.title,
                            label: logged_step_label(&ctx.i18n, &step),
                            progress,
                            verb: None,
                        };
                        run_step(ctx.clone(), logged, busy, error, None);
                    }
                    None => open_page.run(item.id),
                }
            }
            Ok(_) => match api::add_to_library(item.id.clone()).await {
                Ok(_) => {
                    ctx.funnel.logged();
                    toast_added(&ctx, item.id, item.label, None);
                }
                Err(e) => error.set(Some(failed(&e))),
            },
            Err(e) => error.set(Some(failed(&e))),
        }
    });
}

/// The palette fills the screen below 47.5rem (design.md 6.4, UX2-W13).
const FULL_SCREEN_ON_PHONES: &str = "max-[47.5rem]:inset-0 max-[47.5rem]:left-0 max-[47.5rem]:top-0 max-[47.5rem]:h-dvh max-[47.5rem]:max-w-none \
    max-[47.5rem]:translate-x-0 max-[47.5rem]:translate-y-0 max-[47.5rem]:rounded-none \
    max-[47.5rem]:content-start max-[47.5rem]:overflow-y-auto";

/// The scope segments of the palette (design.md 6.4): every kind, or one kind group.
const SCOPES: [&str; 5] = ["all", "watch", "read", "listen", "play"];

/// The `group` a search in `scope` sends (none for All).
fn scope_group(scope: &str) -> Option<String> {
    (scope != SCOPES[0]).then(|| scope.to_string())
}

fn to_item(i: &CatalogItemDto, group: &str) -> CommandItem {
    let kind = crate::i18n::kind_label(&i.kind);
    CommandItem {
        id: i.id.clone(),
        label: match i.year {
            Some(y) => format!("{} ({y})", crate::i18n::title_text(&i.title)),
            None => crate::i18n::title_text(&i.title),
        },
        group: format!("{group} - {kind}"),
        keywords: i.original_title.clone().unwrap_or_default(),
    }
}

/// Which search the streamed results belong to. The server starts asking the sources before
/// it answers the search (`api::ops::search`), so a fast source's `search.results` event can
/// arrive before the palette knows the search's id: while an answer is on its way such
/// results are held, and the answer takes the ones of its search.
#[derive(Debug, Default)]
struct Stream {
    /// The search on screen.
    current: Option<String>,
    /// A search was sent and has not answered yet.
    awaiting: bool,
    held: Vec<SearchResultsEventDto>,
}

impl Stream {
    /// Results held for one answer at most (a search asks a handful of sources per kind).
    const MAX_HELD: usize = 32;

    /// A search was sent: what was held for an older one is of no use any more.
    fn asked(&mut self) {
        self.awaiting = true;
        self.held.clear();
    }

    /// The search answered with its id: the results that came before it, in their order.
    fn answered(&mut self, id: String) -> Vec<SearchResultsEventDto> {
        self.awaiting = false;
        let early = std::mem::take(&mut self.held)
            .into_iter()
            .filter(|r| r.search_id == id)
            .collect();
        self.current = Some(id);
        early
    }

    /// The search failed: nothing more to wait for.
    fn failed(&mut self) {
        self.awaiting = false;
        self.held.clear();
    }

    /// The field was cleared: no search on screen.
    fn cleared(&mut self) {
        *self = Self::default();
    }

    /// A `search.results` event: `Some` when it belongs to the search on screen; one that
    /// may belong to the search in flight is held.
    fn arrived(&mut self, r: SearchResultsEventDto) -> Option<SearchResultsEventDto> {
        if self.current.as_deref() == Some(r.search_id.as_str()) {
            return Some(r);
        }
        if self.awaiting && self.held.len() < Self::MAX_HELD {
            self.held.push(r);
        }
        None
    }
}

/// Whether the list is filtered by the typed text. The idle list (commands, recent items)
/// always is. Search results were matched by the server (typos, other fields) and are shown
/// as they came, but only while they answer the text in the field (`answered`: the query of
/// the answer on screen): rows of an earlier query are filtered as typed until the new
/// answer is there, so Enter never takes a row the text does not name (OT1A-1).
fn filter_as_typed(query: &str, answered: Option<&str>) -> bool {
    query.trim().chars().count() < 2 || answered != Some(query)
}

/// One source's results join the list (an item already listed stays where it is).
fn add_results(
    r: &SearchResultsEventDto,
    waiting: RwSignal<Vec<String>>,
    items: RwSignal<Vec<CommandItem>>,
) {
    waiting.update(|w| w.retain(|c| c != &r.connector));
    items.update(|list| {
        for i in &r.items {
            if !list.iter().any(|x| x.id == i.id) {
                list.push(to_item(i, &r.connector));
            }
        }
    });
}

/// Streamed connector results of the current search join the list as they arrive.
fn stream_results(
    events: crate::events::LastEvent,
    stream: StoredValue<Stream>,
    waiting: RwSignal<Vec<String>>,
    items: RwSignal<Vec<CommandItem>>,
) {
    Effect::new(move |_| {
        let Some((topic, data)) = events.0.get() else {
            return;
        };
        if topic != "search.results" {
            return;
        }
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(&data) else {
            return;
        };
        let Ok(r) = serde_json::from_value::<SearchResultsEventDto>(ev["payload"].clone()) else {
            return;
        };
        let mut shown = None;
        stream.update_value(|s| shown = s.arrived(r));
        if let Some(r) = shown {
            add_results(&r, waiting, items);
        }
    });
}

/// The answer of the current search: what was asked, skipped and not served (the reason
/// lines of design.md 4.4).
#[derive(Debug, Clone, Default, PartialEq)]
struct Asked {
    query: String,
    pending: Vec<String>,
    skipped: Vec<SkippedSourceDto>,
    groups_without_source: Vec<String>,
}

/// The footer's source status, fetched when the palette opens (signed-in only).
fn footer_status(open: RwSignal<bool>) -> Resource<Option<SourceStatusPageDto>> {
    let signed_in = expect_context::<crate::app::ClientConfig>().signed_in;
    Resource::new(
        move || open.get() && signed_in,
        |load| async move {
            if load {
                admin_api::source_status().await.ok()
            } else {
                None
            }
        },
    )
}

/// Ctrl-K / Cmd-K anywhere opens the palette.
fn open_on_ctrl_k(open: RwSignal<bool>) {
    let _ = window_event_listener(leptos::ev::keydown, move |ev| {
        if (ev.ctrl_key() || ev.meta_key()) && ev.key().eq_ignore_ascii_case("k") {
            ev.prevent_default();
            open.set(true);
        }
    });
}

/// The palette's search state.
#[derive(Clone, Copy)]
struct State {
    query: RwSignal<String>,
    scope: RwSignal<String>,
    items: RwSignal<Vec<CommandItem>>,
    stream: StoredValue<Stream>,
    waiting: RwSignal<Vec<String>>,
    asked: RwSignal<Option<Asked>>,
}

/// Search 250 ms after the last keystroke or scope change; a newer search wins.
fn debounced_search(s: State) {
    let State {
        query,
        scope,
        items,
        stream,
        waiting,
        asked,
    } = s;
    let generation = StoredValue::new(0u64);
    Effect::new(move |_| {
        let q = query.get();
        let group = scope_group(&scope.get());
        let g = generation.get_value() + 1;
        generation.set_value(g);
        if q.trim().chars().count() < 2 {
            items.set(vec![]);
            stream.update_value(Stream::cleared);
            waiting.set(vec![]);
            asked.set(None);
            return;
        }
        set_timeout(
            move || {
                if generation.get_value() != g {
                    return;
                }
                stream.update_value(Stream::asked);
                leptos::task::spawn_local(async move {
                    let answer = crate::api::search(q.clone(), None, group).await;
                    if generation.get_value() != g {
                        return;
                    }
                    let Ok(r) = answer else {
                        stream.update_value(Stream::failed);
                        return;
                    };
                    let local = t("search-local");
                    items.set(r.items.iter().map(|i| to_item(i, &local)).collect());
                    waiting.set(r.pending.clone());
                    asked.set(Some(Asked {
                        query: q,
                        pending: r.pending,
                        skipped: r.skipped,
                        groups_without_source: r.groups_without_source,
                    }));
                    // results that were faster than this answer
                    let mut early = Vec::new();
                    stream.update_value(|s| early = s.answered(r.search_id));
                    for e in &early {
                        add_results(e, waiting, items);
                    }
                });
            },
            std::time::Duration::from_millis(250),
        );
    });
}

/// The palette as a funnel flow (WP4-5): opening it is the intent; closing it without a
/// choice abandons the flow, a choice carries it on to the page it opens.
fn funnel_flow(open: RwSignal<bool>) -> StoredValue<bool> {
    let funnel = crate::funnel::Funnel::current();
    let chose = StoredValue::new(false);
    Effect::new(move |prev: Option<bool>| {
        let now = open.get();
        if now && prev != Some(true) {
            chose.set_value(false);
            funnel.intent(crate::funnel::Flow::Palette);
        } else if !now && prev == Some(true) && !chose.get_value() {
            funnel.abandoned_if(crate::funnel::Flow::Palette);
        }
        now
    });
    chose
}

/// Choosing a result opens its page, runs a suggested command, or does what the target says.
/// The target lasts one opening of the palette.
fn select_handler(
    open: RwSignal<bool>,
    query: RwSignal<String>,
    i18n: std::sync::Arc<I18n>,
    chose: StoredValue<bool>,
) -> Callback<CommandItem> {
    let scope = expect_context::<PaletteScope>().0;
    let navigate = use_navigate();
    Effect::new(move |_| {
        if !open.get() {
            scope.set(None);
        }
    });
    let notify = (ui::use_toasts(), i18n);
    Callback::new(move |item: CommandItem| {
        // the funnel flow goes on to the chosen page (WP4-5)
        chose.set_value(true);
        open.set(false);
        query.set(String::new());
        let go = |href: &str| navigate(href, leptos_router::NavigateOptions::default());
        if let Some(href) = command_href(&item.id) {
            go(&href);
            return;
        }
        match scope.get_untracked() {
            Some(PaletteTarget::Match(m)) => match_row(m, item),
            Some(PaletteTarget::List { id, name }) => add_to_list(notify.clone(), id, name, item),
            Some(PaletteTarget::RuleSubject { done }) => done.run((item.id, item.label)),
            Some(PaletteTarget::LogWith { companion_id, .. }) => {
                go(&format!("/items/{}?log=1&with={companion_id}", item.id));
            }
            Some(PaletteTarget::LogWithGroup { group_id, .. }) => {
                go(&format!("/items/{}?log=1&group={group_id}", item.id));
            }
            None => go(&format!("/items/{}", item.id)),
        }
    })
}

/// Ctrl/Cmd+Enter: the quick path for a result; with a target or on a command it is the same
/// as Enter.
fn secondary_handler(
    open: RwSignal<bool>,
    select: Callback<CommandItem>,
    (busy, error): (RwSignal<bool>, RwSignal<Option<String>>),
) -> Callback<CommandItem> {
    let scope = expect_context::<PaletteScope>().0;
    let ctx = StepCtx::current();
    let navigate = use_navigate();
    let open_page = Callback::new(move |id: String| {
        open.set(false);
        navigate(
            &format!("/items/{id}"),
            leptos_router::NavigateOptions::default(),
        );
    });
    Callback::new(move |item: CommandItem| {
        if scope.with_untracked(Option::is_some) || command_href(&item.id).is_some() {
            select.run(item);
        } else {
            quick(ctx.clone(), item, (busy, error), open_page);
        }
    })
}

/// The key hints of the active row: Enter opens (or chooses, with a target), Ctrl+Enter is
/// the quick path (none with a target).
fn hints(i18n: &I18n, scope: RwSignal<Option<PaletteTarget>>) -> CommandHints {
    let (open, choose, quick) = (
        i18n.t("search-hint-open"),
        i18n.t("search-hint-choose"),
        i18n.t("search-hint-quick"),
    );
    let enter = i18n.t("shortcuts-key-enter");
    CommandHints {
        enter_keys: vec![enter.clone()],
        enter: Signal::derive(move || {
            if scope.with(Option::is_some) {
                choose.clone()
            } else {
                open.clone()
            }
        }),
        secondary_keys: vec![i18n.t("shell-ctrl"), enter],
        secondary: Signal::derive(move || {
            if scope.with(Option::is_some) {
                String::new()
            } else {
                quick.clone()
            }
        }),
    }
}

/// The recent items are read when the palette opens (they change on every item page).
fn recent_on_open(open: RwSignal<bool>) -> RwSignal<Vec<RecentItem>> {
    let username = expect_context::<crate::app::ClientConfig>().username;
    let recent = RwSignal::new(Vec::new());
    Effect::new(move |_| {
        if open.get() {
            recent.set(crate::recent::load(&username));
        }
    });
    recent
}

/// The "Asking X..." line and the list's empty text (nothing yet, searching, or none when
/// the no-results card explains it).
fn status_texts(
    i18n: &std::sync::Arc<I18n>,
    s: State,
    empty_answer: Memo<Option<Asked>>,
) -> (Signal<String>, Signal<String>) {
    let (i1, i2) = (i18n.clone(), i18n.clone());
    let waiting = s.waiting;
    let asked = s.asked;
    let waiting_text = Signal::derive(move || {
        let w = waiting.get();
        if w.is_empty() {
            String::new()
        } else {
            i1.t_args("search-waiting", &[("sources", w.join(", ").into())])
        }
    });
    let empty = Signal::derive(move || {
        if empty_answer.with(Option::is_some) {
            String::new()
        } else if asked.with(Option::is_some) {
            i2.t("search-searching")
        } else {
            i2.t("search-empty")
        }
    });
    (waiting_text, empty)
}

#[component]
pub fn SearchPalette() -> impl IntoView {
    let open = expect_context::<PaletteOpen>().0;
    let scope = expect_context::<PaletteScope>().0;
    let i18n = use_i18n();
    let signed_in = expect_context::<crate::app::ClientConfig>().signed_in;
    let s = State {
        query: RwSignal::new(String::new()),
        scope: RwSignal::new(SCOPES[0].to_string()),
        items: RwSignal::new(Vec::<CommandItem>::new()),
        stream: StoredValue::new(Stream::default()),
        waiting: RwSignal::new(Vec::<String>::new()),
        asked: RwSignal::new(None::<Asked>),
    };
    let State {
        query,
        items,
        waiting,
        asked,
        ..
    } = s;
    let status = footer_status(open);
    open_on_ctrl_k(open);
    debounced_search(s);
    stream_results(use_events(), s.stream, waiting, items);
    let recent = recent_on_open(open);
    let (busy, quick_error) = (RwSignal::new(false), RwSignal::new(None::<String>));
    Effect::new(move |_| {
        if !open.get() {
            quick_error.set(None);
        }
    });
    let chose = funnel_flow(open);
    let on_select = select_handler(open, query, i18n.clone(), chose);
    let on_secondary = secondary_handler(open, on_select, (busy, quick_error));
    // matching an import row: search for its title
    Effect::new(move |_| {
        if let Some(PaletteTarget::Match(m)) = scope.get() {
            query.set(m.title);
        }
    });
    let i2 = i18n.clone();
    let shown = Memo::new(move |_| {
        if query.with(|q| q.trim().chars().count() < 2) {
            recent.with(|r| idle_items(&i2, r, scope.with(Option::is_none), signed_in))
        } else {
            items.get()
        }
    });
    // the no-results card shows once every asked source has answered with nothing
    let empty_answer = Memo::new(move |_| {
        asked
            .get()
            .filter(|_| waiting.with(Vec::is_empty) && items.with(Vec::is_empty))
    });
    let (waiting_text, empty) = status_texts(&i18n, s, empty_answer);
    let (label, placeholder) = (i18n.t("search-label"), i18n.t("search-placeholder"));
    let scopes: Vec<SegmentedOption> = SCOPES
        .iter()
        .map(|s| crate::tint::group_option(s, i18n.t(&format!("kind-group-{s}"))))
        .collect();
    let hints = hints(&i18n, scope);
    let filter = Signal::derive(move || {
        let answered = asked.with(|a| a.as_ref().map(|a| a.query.clone()));
        query.with(|q| filter_as_typed(q, answered.as_deref()))
    });
    view! {
        <Dialog open=open title=i18n.t("search-title") close_label=i18n.t("dialog-close") class=FULL_SCREEN_ON_PHONES initial_focus="search">
            {move || scope.get().map(|target| {
                let (id, text) = target_line(&use_i18n(), &target);
                view! { <p class="mb-2 text-sm" data-testid=id>{text}</p> }
            })}
            // a strip that scrolls sideways when the five scopes are wider than the dialog (a
            // long locale on a 360 px phone); without it they widened the whole dialog (OT2D-1)
            <div class="-mx-6 mb-3 max-w-[calc(100%+3rem)] overflow-x-auto px-6">
                <Segmented id="palette-scope" label=i18n.t("search-scope") value=s.scope options=scopes.clone()/>
            </div>
            <Command
                id="search"
                label=label.clone()
                placeholder=placeholder.clone()
                empty_label=empty
                items=Signal::derive(move || shown.get())
                query=query
                on_select=on_select
                on_secondary=on_secondary
                hints=hints.clone()
                filter=filter
            />
            {move || quick_error.get().map(|e| view! { <p role="alert" class="mt-2 text-sm text-destructive-text">{e}</p> })}
            // the reason card is announced when it appears (UX2-W8): the live region stays put
            <div role="status">
                <Suspense fallback=|| ()>
                    {move || empty_answer.get().map(|a| view! { <NoResultsCard asked=a status=status.get().flatten()/> })}
                </Suspense>
            </div>
            <Suspense fallback=|| ()>
                {move || {
                    let shown = empty_answer.with(Option::is_some);
                    status.get().flatten().map(|s| view! { <SourceFooter status=s actions=!shown/> })
                }}
            </Suspense>
            <p class="mt-2 text-xs text-muted-foreground" aria-live="polite">{waiting_text}</p>
        </Dialog>
    }
}

/// A source's display name from the status list (its id when unknown).
fn source_name(status: Option<&SourceStatusPageDto>, id: &str) -> String {
    status
        .and_then(|s| s.sources.iter().find(|x| x.id == id))
        .map_or_else(|| id.to_string(), |x| x.name.clone())
}

#[component]
fn HintActionView(action: HintAction, #[prop(into)] name: String) -> impl IntoView {
    match action {
        HintAction::SetUp => view! {
            <a class="text-sm font-medium text-primary underline" href=SET_UP_HREF>{t("settings-sources-set-up")}</a>
        }
        .into_any(),
        HintAction::AskGroup(g) => view! { <AskAdmin source=None group=Some(g) name=name/> }.into_any(),
    }
}

/// Why the search shows nothing (design.md 5.3, 6.4): never an empty list without a reason.
#[component]
fn NoResultsCard(asked: Asked, status: Option<SourceStatusPageDto>) -> impl IntoView {
    let i18n = use_i18n();
    let admin = status.as_ref().is_some_and(|s| s.is_admin);
    let names: Vec<String> = asked
        .pending
        .iter()
        .map(|id| source_name(status.as_ref(), id))
        .collect();
    let reason = no_results(admin, &names, &asked.skipped, &asked.groups_without_source);
    let title = i18n.t_args(
        "search-none-title",
        &[("query", asked.query.clone().into())],
    );
    let searched = |list: &[String]| {
        if list.is_empty() {
            i18n.t("search-none-library")
        } else {
            i18n.t_args(
                "search-none-searched",
                &[("sources", list.join(", ").into())],
            )
        }
    };
    let body = match reason {
        NoResults::NotSetUp { action } => {
            let lead = t(if admin { "search-unset-admin" } else { "search-unset-member" });
            let group = match &action {
                Some(HintAction::AskGroup(g)) => t(&format!("kind-group-{g}")),
                _ => String::new(),
            };
            return view! {
                <section data-testid="no-results" aria-labelledby="palette-none-h" class="mt-3 grid gap-2 rounded-xl border bg-card p-4">
                    <h3 id="palette-none-h" class="font-semibold">{t("search-unset-title")}</h3>
                    <p class="text-sm text-muted-foreground">{lead}</p>
                    {action.map(|a| view! { <div><HintActionView action=a name=group/></div> })}
                </section>
            }
            .into_any();
        }
        NoResults::NothingFound { searched: list } => view! {
            <p class="text-sm text-muted-foreground">{searched(&list)} " " {t("search-none-spelling")}</p>
        }
        .into_any(),
        NoResults::Explained { searched: list, groups, skipped } => {
            let lines = groups.into_iter().map(|(g, action)| {
                let name = t(&format!("kind-group-{g}"));
                let line = i18n.t_args("search-none-group", &[("group", name.clone().into())]);
                view! {
                    <li class="flex flex-wrap items-center gap-x-3 gap-y-1">
                        <span>{line}</span>
                        <HintActionView action=action name=name/>
                    </li>
                }
            }).collect_view();
            let skipped = skipped.into_iter().map(|s| {
                let state = t(&format!("source-state-{}", s.state));
                let line = i18n.t_args("search-none-skipped", &[("source", s.name.into()), ("state", state.into())]);
                view! { <li>{line}</li> }
            }).collect_view();
            view! {
                <p class="text-sm text-muted-foreground">{searched(&list)}</p>
                <ul class="grid gap-1 text-sm">{lines}{skipped}</ul>
            }
            .into_any()
        }
    };
    view! {
        <section data-testid="no-results" aria-labelledby="palette-none-h" class="mt-3 grid gap-2 rounded-xl border bg-card p-4">
            <h3 id="palette-none-h" class="font-semibold">{title}</h3>
            {body}
        </section>
    }
    .into_any()
}

/// Source status dots with words (design.md 6.4), each row with its one action unless the
/// no-results card above already offers it.
#[component]
fn SourceFooter(status: SourceStatusPageDto, actions: bool) -> impl IntoView {
    let i18n = use_i18n();
    let rows = footer_items(&status);
    (!rows.is_empty()).then(|| {
        let rows = rows.into_iter().map(|row| match row {
            FooterItem::Source { name, state, action, .. } => {
                // a word for every state, working too (colour is never the only cue, UX2-W4)
                let word = t(&format!("source-state-{state}"));
                let label = i18n.t_args("search-footer-source", &[("source", name.clone().into()), ("state", word.into())]);
                let dot = source_state(&state);
                view! {
                    <li class="flex flex-wrap items-center gap-x-2">
                        <StatusDot state=dot label=label/>
                        {action.filter(|_| actions).map(|a| view! { <HintActionView action=a name=name/> })}
                    </li>
                }
            }
            FooterItem::Group { group, action } => {
                let name = t(&format!("kind-group-{group}"));
                let label = i18n.t_args("search-footer-group", &[("group", name.clone().into())]);
                view! {
                    <li class="flex flex-wrap items-center gap-x-2">
                        <StatusDot state=ui::StatusState::Off label=label/>
                        {action.filter(|_| actions).map(|a| view! { <HintActionView action=a name=name/> })}
                    </li>
                }
            }
        }).collect_view();
        view! {
            <footer class="mt-3 border-t pt-3">
                <ul aria-label=t("search-footer-label") class="flex flex-wrap gap-x-4 gap-y-2">{rows}</ul>
            </footer>
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_searches_every_kind_and_a_segment_its_group() {
        assert_eq!(SCOPES[0], "all", "All is the first segment");
        assert_eq!(scope_group("all"), None);
        assert_eq!(scope_group("play"), Some("play".to_string()));
    }
    #[test]
    fn commands_go_to_their_pages_and_items_are_not_commands() {
        assert_eq!(command_href("cmd:lists").as_deref(), Some("/lists"));
        assert_eq!(
            command_href("cmd:new-list").as_deref(),
            Some("/lists?new=1")
        );
        assert_eq!(
            command_href("cmd:import").as_deref(),
            Some("/settings/imports")
        );
        assert_eq!(
            command_href("cmd:play:abc").as_deref(),
            Some("/items/abc?log=1")
        );
        assert_eq!(command_href("0190b6a4-0000-7000-8000-000000000001"), None);
    }

    #[test]
    fn the_empty_palette_lists_recent_items_then_the_suggestions() {
        let en = I18n::new("en", std::collections::BTreeMap::new());
        let recent = vec![
            RecentItem {
                id: "m".into(),
                label: "Dune (2021)".into(),
                plays: false,
            },
            RecentItem {
                id: "g".into(),
                label: "Azul (2017)".into(),
                plays: true,
            },
        ];
        let rows = idle_items(&en, &recent, true, true);
        let labels: Vec<_> = rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Dune (2021)",
                "Azul (2017)",
                "Go to Lists",
                "New list",
                "Log a play of Azul (2017)",
                "Import a file"
            ]
        );
        assert_eq!(rows[0].group, "Recent");
        assert_eq!(rows[2].group, "Suggestions");
        assert_eq!(rows[4].id, "cmd:play:g");
        // with a target only the recent items: a command would leave the target
        assert_eq!(idle_items(&en, &recent, false, true).len(), 2);
        // no game opened lately: no "Log a play"
        assert_eq!(idle_items(&en, &recent[..1], true, true).len(), 4);
        // signed out: no commands and no recent items (UX4-15)
        assert_eq!(idle_items(&en, &recent, true, false).len(), 0);
    }

    /// OT1A-1: server results are shown as they came only while they answer the text in the
    /// field; rows of an earlier query are filtered as typed until the new answer is there.
    #[test]
    fn results_of_another_query_are_filtered_as_typed() {
        // the idle list (commands, recent items) is always filtered
        assert!(filter_as_typed("", None));
        assert!(filter_as_typed("d", Some("dune")));
        // typed on, the search has not answered (or failed): the old rows are stale
        assert!(filter_as_typed("dune messiah", Some("dune")));
        assert!(filter_as_typed("du", None));
        // the answer on screen is the one of this text: typo-tolerant hits stay
        assert!(!filter_as_typed("dune messiah", Some("dune messiah")));
        assert!(!filter_as_typed("fihgt clbu", Some("fihgt clbu")));
    }

    fn batch(search: &str, connector: &str) -> SearchResultsEventDto {
        SearchResultsEventDto {
            search_id: search.into(),
            connector: connector.into(),
            items: Vec::new(),
            error: None,
        }
    }

    /// The server starts asking the sources before it answers the search, so a fast source's
    /// results can arrive before the palette knows the search's id: they wait for the answer.
    #[test]
    fn a_batch_that_beats_the_answer_waits_for_it() {
        let mut s = Stream::default();
        s.asked();
        assert_eq!(s.arrived(batch("s1", "tmdb")), None, "held, not dropped");
        assert_eq!(s.arrived(batch("s0", "igdb")), None);
        // the answer names the search: its early batches join, the stale one is gone
        let early = s.answered("s1".into());
        assert_eq!(early, vec![batch("s1", "tmdb")]);
        // from now on its batches go straight to the list
        assert_eq!(s.arrived(batch("s1", "igdb")), Some(batch("s1", "igdb")));
        assert_eq!(s.arrived(batch("s0", "tmdb")), None, "an older search");
        assert_eq!(s.answered("s1".into()).len(), 0, "nothing is shown twice");
    }

    #[test]
    fn nothing_is_held_without_a_search_in_flight_and_a_new_search_starts_clean() {
        let mut s = Stream::default();
        assert_eq!(s.arrived(batch("s1", "tmdb")), None);
        assert_eq!(s.answered("s1".into()).len(), 0, "it was dropped");
        // while the next search is on its way the shown one still takes its batches
        s.asked();
        assert_eq!(s.arrived(batch("s1", "igdb")), Some(batch("s1", "igdb")));
        assert_eq!(s.arrived(batch("s2", "tmdb")), None);
        // a newer search, or a cleared field, forgets what was held
        s.asked();
        assert_eq!(s.answered("s2".into()).len(), 0);
        s.asked();
        assert_eq!(s.arrived(batch("s3", "tmdb")), None);
        s.cleared();
        assert_eq!(s.arrived(batch("s3", "igdb")), None);
        assert_eq!(s.answered("s3".into()).len(), 0);
        // a search that cannot be held for ever: the buffer is bounded
        s.asked();
        for n in 0..(Stream::MAX_HELD + 5) {
            let _ = s.arrived(batch("s4", &format!("c{n}")));
        }
        assert_eq!(s.answered("s4".into()).len(), Stream::MAX_HELD);
    }
}
