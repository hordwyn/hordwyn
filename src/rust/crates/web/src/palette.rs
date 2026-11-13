//! Ctrl-K search palette (D-6.6, design.md 5.3, 6.4): local results first, then each
//! connector's results as its `search.results` event arrives for the current search id. The
//! footer shows the source status (dots with words: which groups have no source, which
//! sources fail) and an empty search ends in a card that says why (design.md 4.4), with "Ask
//! admin" for members and "Set up" for admins. Below two characters it lists the recently
//! opened items and suggested commands; Ctrl/Cmd+Enter adds a result as Planned (toast with
//! Undo) or runs the next step of an entry, and the palette stays open (WP4-4). A
//! [`PaletteTarget`] makes choosing a result do something else for one opening.

use contracts::admin::{SkippedSourceDto, SourceStatusPageDto};
use contracts::{CatalogItemDto, SearchResponseDto, SearchResultsEventDto};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use ui::{
    Command, CommandHints, CommandItem, CommandLeading, Dialog, Segmented, SegmentedOption,
    StatusDot,
};

use crate::admin_api;
use crate::admin_state::source_state;
use crate::api;
use crate::events::use_events;
use crate::i18n::{I18n, kind_label, t, use_i18n};
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
/// The last row of a search: "Add ... by hand" (v1-manual). It opens the dialog of
/// [`crate::manual_item`] and goes nowhere by itself.
const CMD_MANUAL: &str = "cmd:manual";

/// A row that is a command, not a catalogue item: Enter and Ctrl+Enter both run it.
fn is_command(id: &str) -> bool {
    id == CMD_MANUAL || command_href(id).is_some()
}

/// "Add "<query>" by hand...": the row under the results of a search, in a group of its own.
/// The query is in its keywords, so the list's own filter keeps it whatever is typed.
fn manual_row(i18n: &I18n, query: &str) -> CommandItem {
    let query = query.trim();
    CommandItem {
        id: CMD_MANUAL.into(),
        label: i18n.t_args("manual-row", &[("query", query.to_string().into())]),
        group: i18n.t("manual-group"),
        keywords: query.to_string(),
        // the box of a result row with a plus in it: the labels of a search line up
        leading: Some(CommandLeading {
            icon: ui::IconName::Plus,
            image: None,
            square: false,
        }),
    }
}

/// Is "Add ... by hand" offered: the search has its answer and every asked source has
/// answered (the row never becomes the active one before the results do, so Enter does not
/// open it by accident), nothing else is being chosen (`target`: a match for an import row,
/// an item for a list or a rule) and somebody is signed in.
/// `answered` is the text the shown answer is for: while a newer text waits for its own
/// answer the row is not there (Enter must not add what was typed a moment ago, OT1A-1).
fn offers_manual(
    answered: Option<&str>,
    typed: &str,
    waiting: usize,
    target: bool,
    signed_in: bool,
) -> bool {
    answered.is_some_and(|a| a.trim() == typed.trim()) && waiting == 0 && !target && signed_in
}

/// `rows` with the row of [`manual_row`] last, whatever joins the results later.
fn with_manual_row(mut rows: Vec<CommandItem>, row: Option<CommandItem>) -> Vec<CommandItem> {
    rows.extend(row);
    rows
}

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
        leading: None,
    };
    let recent_group = i18n.t("search-recent");
    // a recent item leads with the same box as a result row (v1-polish P7)
    let mut out: Vec<CommandItem> = recent
        .iter()
        .map(|r| CommandItem {
            leading: Some(CommandLeading {
                icon: crate::library::kind_icon(&r.kind),
                image: r.cover.clone().filter(|url| !url.is_empty()),
                square: crate::library_state::square_art(&r.kind),
            }),
            ..row(r.id.clone(), r.label.clone(), &recent_group)
        })
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
                    // the page behind the palette shows the new entry at once (OT1B-9)
                    ctx.acted();
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

/// The small cover of a row (OT-2), by the one rule of every page (DEC-640): the cover this
/// instance has stored, in its smallest variant (WebP when there is one), else the provider's
/// address, which the viewer's browser loads. Without either the row shows the icon of its
/// kind, as it does while the picture loads and when it fails.
pub(crate) fn thumb(i: &CatalogItemDto) -> Option<String> {
    let cover = i.cover.as_ref()?;
    cover
        .variants
        .iter()
        .filter(|v| v.mime == "image/webp")
        .min_by_key(|v| v.width)
        .or_else(|| cover.variants.iter().min_by_key(|v| v.width))
        .map(|v| v.url.clone())
        .or_else(|| Some(cover.url.clone()))
        .filter(|url| !url.is_empty())
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
        leading: Some(CommandLeading {
            icon: crate::library::kind_icon(&i.kind),
            image: thumb(i),
            square: crate::library_state::square_art(&i.kind),
        }),
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

/// The source a local copy came from, if this search asks that source: the source of its
/// title, else of one of its external ids.
fn asked_source<'a>(i: &'a CatalogItemDto, pending: &[String]) -> Option<&'a str> {
    let asked = |s: &&str| pending.iter().any(|p| p == s);
    i.provenance
        .get("title")
        .map(String::as_str)
        .filter(asked)
        .or_else(|| {
            i.external_ids
                .iter()
                .map(|e| e.namespace.as_str())
                .find(asked)
        })
}

/// Rows with each group in one run, groups in the order of their first row (OT1B-7): the
/// list draws a heading before every run.
fn contiguous(rows: Vec<CommandItem>) -> Vec<CommandItem> {
    let mut groups: Vec<(String, Vec<CommandItem>)> = Vec::new();
    for row in rows {
        match groups.iter_mut().find(|(g, _)| *g == row.group) {
            Some((_, of_group)) => of_group.push(row),
            None => groups.push((row.group.clone(), vec![row])),
        }
    }
    groups.into_iter().flat_map(|(_, rows)| rows).collect()
}

/// How long the copies of a source that is being asked are held back for its answer
/// (OT1B-5). A source that answers within this time lists its titles once, in its own order;
/// after it the copies are listed in ours and a later answer only adds rows.
const HOLD_MS: u64 = 700;

/// The rows of a search's own answer (OT-3) and the rows held back. The hits in the
/// person's library lead, under `library`: the entry is what makes a local hit more relevant
/// than a source's answer. Every other hit is a copy this instance keeps of an earlier
/// answer. When its source is asked again it is held for that answer ([`HOLD_MS`]), in the
/// source's group; else it is listed under `local`, in the order of our own index.
fn local_rows(
    r: &SearchResponseDto,
    library: &str,
    local: &str,
) -> (Vec<CommandItem>, Vec<CommandItem>) {
    let mine = |i: &&CatalogItemDto| r.in_library.contains(&i.id);
    let mut rows: Vec<CommandItem> = r
        .items
        .iter()
        .filter(mine)
        .map(|i| to_item(i, library))
        .collect();
    let mut held = Vec::new();
    for i in r.items.iter().filter(|i| !mine(i)) {
        match asked_source(i, &r.pending) {
            Some(source) => held.push(to_item(i, source)),
            None => rows.push(to_item(i, local)),
        }
    }
    (contiguous(rows), held)
}

/// How many rows from the top must stay where they are when rows join the list (OT1B-5):
/// everything down to the active row, which is the first row while nothing is chosen.
/// Enter, Ctrl+Enter and a click take the active row, so it must not change under the person.
fn pinned(list: &[CommandItem], active: Option<&str>) -> usize {
    active
        .and_then(|id| list.iter().position(|r| r.id == id))
        .map_or(list.len().min(1), |at| at + 1)
}

/// Rows join the list (OT-3, OT1B-5): a row on screen stays where it is, new rows keep the
/// order they come in. They go to the end of their group when that is below the `pinned`
/// rows, else to the end of the list (the group then has a second heading there: a row
/// under the pointer or the keys does not move for the sake of one heading).
fn merge(list: &mut Vec<CommandItem>, batch: Vec<CommandItem>, pinned: usize) {
    let mut fresh: Vec<CommandItem> = Vec::new();
    for row in batch {
        if !list.iter().chain(&fresh).any(|x| x.id == row.id) {
            fresh.push(row);
        }
    }
    while let Some(group) = fresh.first().map(|f| f.group.clone()) {
        let (of_group, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut fresh)
            .into_iter()
            .partition(|f| f.group == group);
        fresh = rest;
        let at = list
            .iter()
            .rposition(|x| x.group == group)
            .map(|last| last + 1)
            .filter(|at| *at >= pinned)
            .unwrap_or(list.len());
        list.splice(at..at, of_group);
    }
}

/// The copies held back for their sources' answers, and the answers that came while copies
/// were still held (OT1B-5). Our own answer and the sources' answers become one addition to
/// the list: each source's titles in its own order, the groups in the order of our own
/// hits. Without the wait the group of whichever source answered first stood first.
#[derive(Debug, Default)]
struct Hold {
    /// Copies not yet named or released, in the order of our own hits.
    held: Vec<CommandItem>,
    /// The groups of the held copies, in the order of our own hits.
    order: Vec<String>,
    /// Rows of answers that arrived while copies were held.
    early: Vec<CommandItem>,
    /// Rows are being held (until every held group was answered, or [`HOLD_MS`]).
    open: bool,
}

impl Hold {
    fn new(held: Vec<CommandItem>) -> Self {
        let mut order: Vec<String> = Vec::new();
        for h in &held {
            if !order.contains(&h.group) {
                order.push(h.group.clone());
            }
        }
        Self {
            open: !held.is_empty(),
            held,
            order,
            early: Vec::new(),
        }
    }

    /// A source's rows arrived: the rows to list now, or `None` while copies of another
    /// group are still held. The held copies of the groups the answer names follow its rows
    /// (a source answers once per kind, and the event does not say which kind: an empty or
    /// failed answer names none, its copies wait for the hold time).
    fn arrived(&mut self, mut batch: Vec<CommandItem>) -> Option<Vec<CommandItem>> {
        if !self.open {
            return Some(batch);
        }
        let (named, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.held)
            .into_iter()
            .partition(|h| batch.iter().any(|b| b.group == h.group));
        self.held = rest;
        // a copy the answer names is the answer's row
        let unnamed: Vec<CommandItem> = named
            .into_iter()
            .filter(|h| !batch.iter().any(|b| b.id == h.id))
            .collect();
        batch.extend(unnamed);
        self.early.extend(batch);
        self.held.is_empty().then(|| self.release())
    }

    /// Everything that waited, groups in the order of our own hits, then the groups our
    /// index did not hit in the order they arrived; copies still held come last in their
    /// group, in our order.
    fn release(&mut self) -> Vec<CommandItem> {
        self.open = false;
        let mut rows = std::mem::take(&mut self.early);
        rows.append(&mut self.held);
        let rank = |r: &CommandItem| {
            self.order
                .iter()
                .position(|g| *g == r.group)
                .unwrap_or(usize::MAX)
        };
        rows.sort_by_key(rank);
        rows
    }
}

/// `batch` joins the list below the active row.
fn join(s: State, batch: Vec<CommandItem>) {
    if batch.is_empty() {
        return;
    }
    let active = s.active.get_untracked();
    s.items.update(|list| {
        let keep = pinned(list, active.as_deref());
        merge(list, batch, keep);
    });
}

/// One source's results join the list: its titles in the order it gave them, then the copies
/// held for it that it does not name ([`Hold`]).
fn add_results(r: &SearchResultsEventDto, s: State) {
    refresh_recent(s, &r.items);
    s.waiting.update(|w| w.retain(|c| c != &r.connector));
    let batch: Vec<CommandItem> = r.items.iter().map(|i| to_item(i, &r.connector)).collect();
    let mut now = None;
    s.held.update_value(|hold| now = hold.arrived(batch));
    if let Some(rows) = now {
        join(s, rows);
    }
}

/// The recent items a read names take the cover the read gives them (review POL-3): the
/// list in the browser holds the address of the day the item page was opened, which is the
/// provider's when the cover was not stored yet. No request is made for this.
fn refresh_recent(s: State, items: &[CatalogItemDto]) {
    let fresh: Vec<(String, Option<String>)> =
        items.iter().map(|i| (i.id.clone(), thumb(i))).collect();
    let list = s.username.with_value(|u| crate::recent::refresh(u, &fresh));
    if let Some(list) = list {
        s.recent.set(list);
    }
}

/// Streamed connector results of the current search join the list as they arrive.
fn stream_results(events: crate::events::LastEvent, s: State) {
    let stream = s.stream;
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
            add_results(&r, s);
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
    /// Copies held back for their source's answer ([`local_rows`], [`Hold`]).
    held: StoredValue<Hold>,
    /// The active row, told by the list ([`pinned`]).
    active: RwSignal<Option<String>>,
    /// The recent items of the idle list, read when the palette opens.
    recent: RwSignal<Vec<RecentItem>>,
    /// Whose recent items they are (the key of the list in this browser).
    username: StoredValue<String>,
}

impl State {
    fn new() -> Self {
        Self {
            query: RwSignal::new(String::new()),
            scope: RwSignal::new(SCOPES[0].to_string()),
            items: RwSignal::new(Vec::<CommandItem>::new()),
            stream: StoredValue::new(Stream::default()),
            waiting: RwSignal::new(Vec::<String>::new()),
            asked: RwSignal::new(None::<Asked>),
            held: StoredValue::new(Hold::default()),
            active: RwSignal::new(None::<String>),
            recent: RwSignal::new(Vec::new()),
            username: StoredValue::new(expect_context::<crate::app::ClientConfig>().username),
        }
    }
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
        held,
        ..
    } = s;
    let generation = StoredValue::new(0u64);
    Effect::new(move |_| {
        let q = query.get();
        let group = scope_group(&scope.get());
        let g = generation.get_value() + 1;
        generation.set_value(g);
        if q.trim().chars().count() < 2 {
            items.set(vec![]);
            held.set_value(Hold::default());
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
                    refresh_recent(s, &r.items);
                    let (rows, wait) = local_rows(&r, &t("search-library"), &t("search-local"));
                    items.set(rows);
                    held.set_value(Hold::new(wait));
                    // a source that has not answered by then: its copies in our order
                    set_timeout(
                        move || {
                            if generation.try_get_value() == Some(g) {
                                let mut rest = Vec::new();
                                held.update_value(|h| {
                                    if h.open {
                                        rest = h.release();
                                    }
                                });
                                join(s, rest);
                            }
                        },
                        std::time::Duration::from_millis(HOLD_MS),
                    );
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
                        add_results(e, s);
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
    kinds: RwSignal<String>,
) -> Callback<CommandItem> {
    let scope = expect_context::<PaletteScope>().0;
    let manual = expect_context::<crate::manual_item::ManualItem>();
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
        let typed = query.get_untracked();
        open.set(false);
        query.set(String::new());
        if item.id == CMD_MANUAL {
            manual.add(&typed, &kinds.get_untracked());
            return;
        }
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
        if scope.with_untracked(Option::is_some) || is_command(&item.id) {
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
fn recent_on_open(open: RwSignal<bool>, s: State) -> RwSignal<Vec<RecentItem>> {
    let State {
        recent, username, ..
    } = s;
    Effect::new(move |_| {
        if open.get() {
            recent.set(username.with_value(|u| crate::recent::load(u)));
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

/// The rows of the list: below two characters the recent items and the suggestions, else
/// the results of the search with "Add ... by hand" as the last row once it is offered
/// ([`offers_manual`], v1-manual).
fn shown_rows(
    s: State,
    scope: RwSignal<Option<PaletteTarget>>,
    recent: RwSignal<Vec<RecentItem>>,
    i18n: std::sync::Arc<I18n>,
    signed_in: bool,
) -> Memo<Vec<CommandItem>> {
    let State {
        query,
        items,
        waiting,
        asked,
        ..
    } = s;
    Memo::new(move |_| {
        if query.with(|q| q.trim().chars().count() < 2) {
            return recent.with(|r| idle_items(&i18n, r, scope.with(Option::is_none), signed_in));
        }
        let typed = query.get();
        let offered = asked.with(|a| {
            offers_manual(
                a.as_ref().map(|a| a.query.as_str()),
                &typed,
                waiting.with(Vec::len),
                scope.with(Option::is_some),
                signed_in,
            )
        });
        let row = offered.then(|| manual_row(&i18n, &typed));
        with_manual_row(items.get(), row)
    })
}

#[component]
pub fn SearchPalette() -> impl IntoView {
    let open = expect_context::<PaletteOpen>().0;
    let scope = expect_context::<PaletteScope>().0;
    let i18n = use_i18n();
    let signed_in = expect_context::<crate::app::ClientConfig>().signed_in;
    let s = State::new();
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
    stream_results(use_events(), s);
    let recent = recent_on_open(open, s);
    let (busy, quick_error) = (RwSignal::new(false), RwSignal::new(None::<String>));
    Effect::new(move |_| {
        if !open.get() {
            quick_error.set(None);
        }
    });
    let chose = funnel_flow(open);
    let on_select = select_handler(open, query, i18n.clone(), chose, s.scope);
    let on_secondary = secondary_handler(open, on_select, (busy, quick_error));
    // matching an import row: search for its title
    Effect::new(move |_| {
        if let Some(PaletteTarget::Match(m)) = scope.get() {
            query.set(m.title);
        }
    });
    let shown = shown_rows(s, scope, recent, i18n.clone(), signed_in);
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
                active_item=s.active
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
            FooterItem::Source { name, state, action, kinds, .. } => {
                // a word for every state, working too (colour is never the only cue, UX2-W4)
                let word = t(&format!("source-state-{state}"));
                let label = if state == "needs_contact" && !kinds.is_empty() {
                    // what waits for the contact: "Wikidata (Board game): Operator contact missing"
                    let kinds = kinds.iter().map(|k| kind_label(k)).collect::<Vec<_>>().join(", ");
                    i18n.t_args("search-footer-source-kinds", &[("source", name.clone().into()), ("kinds", kinds.into()), ("state", word.into())])
                } else {
                    i18n.t_args("search-footer-source", &[("source", name.clone().into()), ("state", word.into())])
                };
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
                kind: "movie".into(),
                cover: Some("https://image.example/dune.jpg".into()),
            },
            RecentItem {
                id: "g".into(),
                label: "Azul (2017)".into(),
                plays: true,
                kind: "board_game".into(),
                cover: None,
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
        // a recent item leads with the same box as a result row (v1-polish P7); a command
        // is a row of text
        assert_eq!(
            rows[0].leading,
            Some(CommandLeading {
                icon: ui::IconName::KindMovie,
                image: Some("https://image.example/dune.jpg".into()),
                square: false,
            })
        );
        assert_eq!(
            rows[1].leading,
            Some(CommandLeading {
                icon: ui::IconName::KindBoardGame,
                image: None,
                square: true,
            })
        );
        assert_eq!(rows[2].leading, None);
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

    /// Run with the English messages in context (`to_item` names the kind).
    fn with_en(f: impl FnOnce()) {
        Owner::new().with(|| {
            provide_context(std::sync::Arc::new(I18n::new(
                "en",
                std::collections::BTreeMap::new(),
            )));
            f();
        });
    }

    fn dto(id: &str, title: &str, source: &str) -> CatalogItemDto {
        CatalogItemDto {
            id: id.into(),
            kind: "movie".into(),
            title: title.into(),
            original_title: None,
            year: Some(2001),
            description: None,
            cover: None,
            external_ids: vec![contracts::ExternalIdDto {
                namespace: source.into(),
                value: id.into(),
            }],
            attributes: serde_json::Value::Null,
            state: "partial".into(),
            provenance: std::collections::BTreeMap::new(),
        }
    }

    fn answer(items: Vec<CatalogItemDto>, mine: &[&str], pending: &[&str]) -> SearchResponseDto {
        SearchResponseDto {
            search_id: "s1".into(),
            items,
            in_library: mine.iter().map(ToString::to_string).collect(),
            pending: pending.iter().map(ToString::to_string).collect(),
            skipped: Vec::new(),
            groups_without_source: Vec::new(),
        }
    }

    fn ids(rows: &[CommandItem]) -> Vec<(&str, &str)> {
        rows.iter()
            .map(|r| (r.id.as_str(), r.group.as_str()))
            .collect()
    }

    /// OT-3, OT1B-5, OT1B-7: only the person's library leads; the copies this instance keeps
    /// of a source that is asked wait for its answer, which lists them in the source's order
    /// ("Harry Potter and the Order of the Homeless" stood above the first film); nothing on
    /// screen moves when rows join, and each group stands under one heading.
    #[test]
    fn library_entries_lead_and_a_source_keeps_its_order() {
        with_en(library_rows_and_merge);
    }

    fn just_ids(rows: &[CommandItem]) -> Vec<&str> {
        rows.iter().map(|r| r.id.as_str()).collect()
    }

    /// Each group is one run of rows.
    fn one_heading_per_group(rows: &[CommandItem]) {
        let mut runs: Vec<&str> = rows.iter().map(|r| r.group.as_str()).collect();
        runs.dedup();
        let mut groups = runs.clone();
        groups.sort_unstable();
        groups.dedup();
        assert_eq!(runs.len(), groups.len(), "a group stands twice: {runs:?}");
    }

    fn library_rows_and_merge() {
        // the local index ranks the derivative title first; `own` is in the library, `man`
        // and `note` were typed in by hand (no source), `show` is a copy of a source that is
        // not asked, `old` is a kept copy the source no longer names
        let mut show = dto("show", "Harry Potter: The Series", "tvdb");
        show.kind = "show".into();
        let r = answer(
            vec![
                dto(
                    "homeless",
                    "Harry Potter and the Order of the Homeless",
                    "tmdb",
                ),
                dto("man", "Harry Potter (my notes)", "manual"),
                dto("stone", "Harry Potter and the Philosopher's Stone", "tmdb"),
                show,
                dto("own", "Harry Potter and the Goblet of Fire", "tmdb"),
                dto("note", "Harry Potter night", "manual"),
                dto("old", "Harry Potter: A History of Magic", "tmdb"),
            ],
            &["own"],
            &["tmdb"],
        );
        let (mut list, copies) = local_rows(&r, "In your library", "In your instance");
        assert_eq!(
            ids(&list),
            [
                ("own", "In your library - Movie"),
                ("man", "In your instance - Movie"),
                ("note", "In your instance - Movie"),
                ("show", "In your instance - Show"),
            ],
            "the library leads; the copies of the asked source are not listed yet"
        );
        assert_eq!(just_ids(&copies), ["homeless", "stone", "old"]);
        one_heading_per_group(&list);
        // the source answers: the first film, a new title, the derivative one, the library's
        let batch: Vec<CommandItem> = ["stone", "chamber", "homeless", "own"]
            .iter()
            .map(|id| to_item(&dto(id, id, "tmdb"), "tmdb"))
            .collect();
        let mut hold = Hold::new(copies);
        let batch = hold
            .arrived(batch)
            .expect("the one held group was answered");
        assert!(!hold.open);
        let before = list.clone();
        merge(&mut list, batch, pinned(&before, None));
        assert_eq!(
            just_ids(&list),
            [
                "own", "man", "note", "show", "stone", "chamber", "homeless", "old"
            ],
            "the source's order, then the kept copy it does not name; nothing moved"
        );
        assert_eq!(list[..4], before[..], "rows on screen keep their place");
        one_heading_per_group(&list);
        // a second source names a row that is on screen: it stays where it is
        let batch = vec![
            to_item(&dto("stone", "stone", "tvdb"), "tvdb"),
            to_item(&dto("hogwarts", "hogwarts", "tvdb"), "tvdb"),
        ];
        merge(&mut list, batch, 1);
        assert_eq!(
            just_ids(&list),
            [
                "own", "man", "note", "show", "stone", "chamber", "homeless", "old", "hogwarts"
            ]
        );
        one_heading_per_group(&list);
        // a source that is not asked this time gives no group: its copies are "in your instance"
        let quiet = answer(vec![dto("stone", "Stone", "tmdb")], &[], &[]);
        let (rows, waiting) = local_rows(&quiet, "L", "In your instance");
        assert_eq!(rows[0].group, "In your instance - Movie");
        assert_eq!(waiting.len(), 0);
    }

    /// OT1B-5: the source was slow, its copies were listed after the hold time in our order;
    /// its answer then only adds rows, and never above or at the active row.
    #[test]
    fn a_late_answer_adds_rows_below_the_active_row() {
        with_en(late_answers);
    }

    fn late_answers() {
        let row = |id: &str, source: &str| to_item(&dto(id, id, source), source);
        let start = || {
            vec![
                row("homeless", "tmdb"),
                row("stone", "tmdb"),
                row("b1", "igdb"),
            ]
        };
        let answer = || {
            vec![
                row("stone", "tmdb"),
                row("chamber", "tmdb"),
                row("homeless", "tmdb"),
            ]
        };
        // nothing chosen: the first row is the active one; the new title goes to the end of
        // its group, the rows on screen keep their order
        let mut list = start();
        let keep = pinned(&list, None);
        assert_eq!(keep, 1);
        merge(&mut list, answer(), keep);
        assert_eq!(just_ids(&list), ["homeless", "stone", "chamber", "b1"]);
        one_heading_per_group(&list);
        // the active row is below the group: nothing may be put above it, the new title
        // goes to the end of the list
        let mut list = start();
        let keep = pinned(&list, Some("b1"));
        assert_eq!(keep, 3);
        merge(&mut list, answer(), keep);
        assert_eq!(just_ids(&list), ["homeless", "stone", "b1", "chamber"]);
        // the active row is the last of the group: the new title follows it directly
        let mut list = start();
        let keep = pinned(&list, Some("stone"));
        merge(&mut list, answer(), keep);
        assert_eq!(just_ids(&list), ["homeless", "stone", "chamber", "b1"]);
        // an active item that is not in the list (typed on) and an empty list
        assert_eq!(pinned(&start(), Some("gone")), 1);
        assert_eq!(pinned(&[], None), 0);
    }

    /// OT1B-5: answers that arrive while copies are held are listed together with them, the
    /// groups in the order of our own hits and not in the order the sources answered.
    #[test]
    fn answers_wait_while_copies_are_held_and_keep_the_order_of_our_hits() {
        with_en(held_answers);
    }

    fn held_answers() {
        let row = |id: &str, source: &str, kind: &str| {
            let mut d = dto(id, id, source);
            d.kind = kind.into();
            to_item(&d, source)
        };
        // our index: a film first, then a show of the same source
        let copies = vec![
            row("stone", "tmdb", "movie"),
            row("series", "tmdb", "show"),
            row("homeless", "tmdb", "movie"),
        ];
        let mut hold = Hold::new(copies);
        // a source our index has no copy of answers first, then the shows: both wait
        assert_eq!(hold.arrived(vec![row("game", "igdb", "video_game")]), None);
        assert_eq!(hold.arrived(vec![row("other", "tmdb", "show")]), None);
        // an empty answer names no group
        assert_eq!(hold.arrived(Vec::new()), None);
        let rows = hold
            .arrived(vec![
                row("chamber", "tmdb", "movie"),
                row("stone", "tmdb", "movie"),
            ])
            .expect("every held group was answered");
        assert_eq!(
            just_ids(&rows),
            ["chamber", "stone", "homeless", "other", "series", "game"],
            "films first as in our index, each source's order, kept copies after it"
        );
        // from then on an answer is listed as it comes
        assert_eq!(
            hold.arrived(vec![row("late", "tvdb", "show")])
                .map(|r| r.len()),
            Some(1)
        );
        // the hold time is over before the films were answered: what waited, in our order
        let mut hold = Hold::new(vec![
            row("stone", "tmdb", "movie"),
            row("series", "tmdb", "show"),
        ]);
        assert_eq!(hold.arrived(vec![row("other", "tmdb", "show")]), None);
        assert_eq!(just_ids(&hold.release()), ["stone", "other", "series"]);
        // nothing held: nothing waits
        let mut hold = Hold::new(Vec::new());
        assert_eq!(
            hold.arrived(vec![row("a", "tmdb", "movie")])
                .map(|r| r.len()),
            Some(1)
        );
    }

    /// OT-2, DEC-640: a row shows the cover this instance has stored, in its smallest variant
    /// (WebP first); a cover known only by its provider address is loaded from there, as on
    /// every other page; without a cover the icon of the kind.
    #[test]
    fn thumb_prefers_the_smallest_stored_variant() {
        with_en(|| covers_of_rows(false));
    }

    #[test]
    fn thumb_falls_back_to_the_provider_address() {
        with_en(|| covers_of_rows(true));
    }

    fn covers_of_rows(provider: bool) {
        let variant = |url: &str, width: u32, mime: &str| contracts::ImageVariantDto {
            url: url.into(),
            width,
            mime: mime.into(),
        };
        let mut stored = dto("a", "Azul", "example");
        stored.kind = "board_game".into();
        stored.cover = Some(contracts::ImageDto {
            url: "/assets/h/card.webp".into(),
            variants: vec![
                variant("/assets/h/card.webp", 400, "image/webp"),
                variant("/assets/h/thumb.avif", 200, "image/avif"),
                variant("/assets/h/thumb.webp", 200, "image/webp"),
            ],
            thumbhash: None,
            dominant_color: None,
            width: Some(600),
            height: Some(600),
        });
        if !provider {
            assert_eq!(thumb(&stored).as_deref(), Some("/assets/h/thumb.webp"));
            assert_eq!(
                to_item(&stored, "example").leading,
                Some(CommandLeading {
                    icon: ui::IconName::KindBoardGame,
                    image: Some("/assets/h/thumb.webp".into()),
                    square: true,
                })
            );
            // no WebP: the smallest of what there is
            if let Some(c) = stored.cover.as_mut() {
                c.variants.retain(|v| v.mime != "image/webp");
            }
            assert_eq!(thumb(&stored).as_deref(), Some("/assets/h/thumb.avif"));
            return;
        }
        let mut remote = dto("m", "Heat", "tmdb");
        remote.cover = Some(contracts::ImageDto {
            url: "https://image.tmdb.org/t/p/w780/heat.jpg".into(),
            variants: Vec::new(),
            thumbhash: None,
            dominant_color: None,
            width: None,
            height: None,
        });
        let leading = |image: Option<&str>| {
            Some(CommandLeading {
                icon: ui::IconName::KindMovie,
                image: image.map(str::to_string),
                square: false,
            })
        };
        assert_eq!(
            to_item(&remote, "tmdb").leading,
            leading(Some("https://image.tmdb.org/t/p/w780/heat.jpg"))
        );
        // a cover without an address is no cover
        if let Some(c) = remote.cover.as_mut() {
            c.url = String::new();
        }
        assert_eq!(to_item(&remote, "tmdb").leading, leading(None));
        assert_eq!(
            to_item(&dto("n", "Heat", "tmdb"), "tmdb").leading,
            leading(None)
        );
    }

    #[test]
    fn the_add_by_hand_row_is_last_and_carries_the_query() {
        with_en(the_manual_row);
    }

    fn the_manual_row() {
        let i18n = use_i18n();
        let results = vec![
            to_item(&dto("a", "Dune", "tmdb"), "tmdb"),
            to_item(&dto("b", "Dune II", "tmdb"), "tmdb"),
        ];
        let rows = with_manual_row(results.clone(), Some(manual_row(&i18n, "  Dune zine ")));
        assert_eq!(just_ids(&rows), ["a", "b", CMD_MANUAL]);
        let last = rows.last().unwrap();
        assert_eq!(last.label, "Add \"Dune zine\" by hand...");
        assert_eq!(
            last.keywords, "Dune zine",
            "the list's filter keeps the row"
        );
        assert_eq!(last.group, "Not what you are looking for?");
        assert_eq!(
            last.leading.as_ref().map(|l| (l.icon, l.image.clone())),
            Some((ui::IconName::Plus, None)),
            "a box like the result rows, with a plus"
        );
        assert!(results.iter().all(|r| r.group != last.group));
        one_heading_per_group(&rows);
        // with no results it is the only row; not offered: the results as they are
        let alone = with_manual_row(Vec::new(), Some(manual_row(&i18n, "Dune zine")));
        assert_eq!(just_ids(&alone), [CMD_MANUAL]);
        assert_eq!(with_manual_row(results.clone(), None), results);
        // a command, not an item: Enter and Ctrl+Enter both open the dialog, no page
        assert!(is_command(CMD_MANUAL));
        assert_eq!(command_href(CMD_MANUAL), None);
        assert!(is_command(CMD_LISTS));
        assert!(!is_command("6f1c0a52-0000-7000-8000-000000000001"));
    }

    #[test]
    fn add_by_hand_is_offered_once_every_source_has_answered() {
        let dune = Some("dune zine");
        assert!(offers_manual(dune, "dune zine", 0, false, true));
        assert!(offers_manual(dune, " dune zine ", 0, false, true));
        assert!(
            !offers_manual(None, "dune zine", 0, false, true),
            "no answer yet"
        );
        assert!(
            !offers_manual(dune, "dune zines", 0, false, true),
            "the answer is for an older text"
        );
        assert!(
            !offers_manual(dune, "dune zine", 1, false, true),
            "a source is being asked"
        );
        assert!(
            !offers_manual(dune, "dune zine", 0, true, true),
            "something else is being chosen"
        );
        assert!(
            !offers_manual(dune, "dune zine", 0, false, false),
            "signed out"
        );
    }
}
