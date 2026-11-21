//! Log dialogs and inline loggers of the item page (design.md 5.4, 7.3; addendum-1 1.3 "5.4
//! Log sheets"): `WatchedSheet` (titled with the step; date chips, "Watched with" people,
//! rating and note behind "More"), `PlaySheet` (date and duration, players, scores, winner;
//! place, notes and co-op behind "More"; for a video game the scores wait behind "More" too),
//! `PagesInline` and `PositionInline`. The two "sheets" are [`ui::FormDialog`]s (OT-10,
//! DEC-539): centered from 640 px (32 rem for watched, 42 rem for a play), a bottom sheet
//! below, the fields scroll between the title and a footer with Cancel and Save. Every save
//! shows "Logged: ..." with Undo; a failure keeps the dialog open with the data and an error
//! above its buttons; closing a changed dialog asks first. Their state lives in the page and
//! the page mounts them once, outside the view that a refetch renders afresh (review OT2B-1:
//! a refetch under an open dialog took the focus, the place in the form and what was typed);
//! what they know about the item ([`ItemInfo`]) they read when they open and when they save.
//! The form rules are in [`crate::log_form`].

use std::collections::BTreeMap;

use contracts::{CompanionDto, GameGroupDto, NextStepDto};
use leptos::html;
use leptos::prelude::*;
use ui::{
    Button, ButtonSize, ButtonVariant, Chip, DialogWidth, Disclosure, DisclosureMode, FormDialog,
    FormField, Input, Label, Segmented, SegmentedOption, Switch, Textarea, cn,
    disclosure_storage_key,
};

use crate::item_words::{Verb, again_line, with_legend_key};

use crate::api;
use crate::history_state::Day;
use crate::i18n::{I18n, t, use_i18n};
use crate::log_form::{
    CoopResult, DateChip, FormError, PageTotal, PlayForm, Player, WatchedForm, WinnerChoice,
    finish_page, format_duration, format_position, group_players, page_progress, parse_page_total,
    position_progress,
};
use crate::next_step::{LoggedStep, StepCtx, Then, run_step, run_step_then, toast_logged};
use crate::widgets::{CompanionPicker, RatingInput, progress_label};

/// Id of the action bar's button that opens the watched dialog: it gets the focus back when
/// the button that opened the dialog is gone (a refetch rendered the bar anew, or the dialog
/// was opened from a menu).
pub const WITH_OPENER: &str = "ip-log-with";
/// The same for the play dialog.
pub const PLAY_OPENER: &str = "ip-log-play";

/// What the sheets need to know about the item.
#[derive(Clone)]
pub struct ItemInfo {
    pub item_id: String,
    pub title: String,
    pub rating: Option<u8>,
    pub note: String,
    /// Times gone through so far when the next step is one more ("again"), else 0: the sheet
    /// says which watch the next one will be (OT-7; the header's count, review OT2A-1).
    pub finished: u32,
    /// The verb of the kind's group ("Watched with", "Played with"); `None` for a kind
    /// without one and while the kind is not known.
    pub verb: Option<Verb>,
    /// A board game: its plays have players with scores, open in the dialog (OT-16).
    pub board_game: bool,
    /// Minutes of the newest play, the prefill of the next one (OT-16).
    pub last_duration: Option<u32>,
    /// When the rewatch run that is running started (the timestamp of its start event);
    /// `None` without one. A view cannot be dated before it (review OT2E-1).
    pub run_start: Option<String>,
    /// Refetch the page.
    pub refresh: Callback<()>,
}

/// What the watched sheet logs and its words ("Watched S2 E5").
#[derive(Debug, Clone, PartialEq)]
pub struct SheetTarget {
    pub progress: serde_json::Value,
    pub label: String,
}

/// The message of a form error.
#[must_use]
pub fn error_message(i: &I18n, e: &FormError) -> String {
    match e {
        FormError::PageBeyond(n) => i.t_args(e.key(), &[("pages", (*n).into())]),
        FormError::BeforeRun(start) => i.t_args(e.key(), &[("date", i.date(start).into())]),
        _ => i.t(e.key()),
    }
}

fn chip_of(s: &str) -> DateChip {
    match s {
        "yesterday" => DateChip::Yesterday,
        "pick" => DateChip::Pick,
        _ => DateChip::Today,
    }
}

/// Today as the page knows it (server-rendered, so both sides agree).
fn today() -> Day {
    crate::history::today()
}

/// The date chips: Today, Yesterday, Pick a date (with a date field).
#[component]
fn DateChips(
    id: &'static str,
    chip: RwSignal<String>,
    picked: RwSignal<String>,
    #[prop(into)] error: Signal<Option<String>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let options = vec![
        SegmentedOption::new("today", i18n.t("log-today")),
        SegmentedOption::new("yesterday", i18n.t("log-yesterday")),
        SegmentedOption::new("pick", i18n.t("log-pick")),
    ];
    let date_id = format!("{id}-date");
    let error_id = format!("{id}-date-error");
    let described = error_id.clone();
    let max = today().iso();
    view! {
        <div class="grid gap-2">
            <Segmented id=id label=t("log-when") value=chip options=options/>
            <Show when=move || chip.get() == "pick">
                <div class="grid gap-1">
                    <Label r#for=date_id.clone()>{use_i18n().t("log-date")}</Label>
                    <input id=date_id.clone() type="date" max=max.clone()
                        aria-invalid=move || error.get().map(|_| "true")
                        aria-describedby=described.clone()
                        prop:value=move || picked.get()
                        on:input=move |ev| picked.set(event_target_value(&ev))
                        class="flex h-9 w-full max-w-xs rounded-md border border-input bg-background px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"/>
                </div>
            </Show>
            <p id=error_id.clone() class="text-sm text-destructive-text" role="alert">{move || error.get()}</p>
        </div>
    }
}

/// "Discard your changes?" in the footer of a dialog that was asked to close while changed
/// (D15: no second dialog). Focus moves to "Keep editing".
#[component]
fn DiscardPrompt(confirm: RwSignal<bool>, discard: Callback<()>) -> impl IntoView {
    // the first button ("Keep editing") takes the focus
    let group = NodeRef::<html::Div>::new();
    Effect::new(move |_| {
        if confirm.get()
            && let Some(el) = group.get()
            && let Ok(Some(b)) = el.query_selector("button")
        {
            use leptos::wasm_bindgen::JsCast;
            if let Ok(b) = b.dyn_into::<leptos::web_sys::HtmlElement>() {
                let _ = b.focus();
            }
        }
    });
    let i18n = use_i18n();
    let (question, discard_label, keep_label) = (
        i18n.t("log-discard-question"),
        i18n.t("log-discard"),
        i18n.t("log-keep"),
    );
    view! {
        <Show when=move || confirm.get()>
            <div node_ref=group role="group" aria-label=question.clone() class="grid gap-2 rounded-md border border-warning/50 bg-warning-soft p-3">
                <p class="text-sm font-medium">{question.clone()}</p>
                <div class="flex flex-wrap gap-2">
                    {let label = keep_label.clone(); view! { <Button variant=ButtonVariant::Outline on:click=move |_| confirm.set(false)>{label}</Button> }}
                    {let label = discard_label.clone(); view! { <Button variant=ButtonVariant::Destructive on:click=move |_| discard.run(())>{label}</Button> }}
                </div>
            </div>
        </Show>
    }
}

/// State of the watched sheet, owned by the page.
#[derive(Clone, Copy)]
pub struct WatchedState {
    /// Set to open the sheet for a step; cleared when it closes.
    pub target: RwSignal<Option<SheetTarget>>,
    /// People the next opening starts with (the palette's "Log with ...", WP4-4).
    pub preset: RwSignal<Vec<String>>,
    open: RwSignal<bool>,
    /// The dialog's title: the step it logs, in the past tense; kept while it closes.
    title: RwSignal<String>,
    chip: RwSignal<String>,
    picked: RwSignal<String>,
    with: RwSignal<Vec<String>>,
    rating: RwSignal<Option<u8>>,
    note: RwSignal<String>,
    initial: RwSignal<WatchedForm>,
    confirm: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    date_error: RwSignal<Option<String>>,
    busy: RwSignal<bool>,
}

impl Default for WatchedState {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchedState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            target: RwSignal::new(None),
            preset: RwSignal::new(Vec::new()),
            open: RwSignal::new(false),
            title: RwSignal::new(String::new()),
            chip: RwSignal::new("today".into()),
            picked: RwSignal::new(String::new()),
            with: RwSignal::new(Vec::new()),
            rating: RwSignal::new(None),
            note: RwSignal::new(String::new()),
            initial: RwSignal::new(WatchedForm::default()),
            confirm: RwSignal::new(false),
            error: RwSignal::new(None),
            date_error: RwSignal::new(None),
            busy: RwSignal::new(false),
        }
    }

    fn form(&self) -> WatchedForm {
        WatchedForm {
            date: chip_of(&self.chip.get_untracked()),
            picked: self.picked.get_untracked(),
            with: self.with.get_untracked(),
            rating: self.rating.get_untracked(),
            note: self.note.get_untracked(),
        }
    }

    /// Fresh fields for a new log; the rating and note start as the entry has them (D12).
    fn reset(&self, info: &ItemInfo) {
        self.chip.set("today".into());
        self.picked.set(String::new());
        self.with.set(self.preset.get_untracked());
        self.preset.set(Vec::new());
        self.rating.set(info.rating);
        self.note.set(info.note.clone());
        self.initial.set(self.form());
        self.confirm.set(false);
        self.error.set(None);
        self.date_error.set(None);
    }

    fn dirty(&self) -> bool {
        self.form() != self.initial.get_untracked()
    }

    /// Close without asking (the page shows another item now).
    pub fn dismiss(&self) {
        if self.open.get_untracked() {
            self.confirm.set(false);
            self.open.set(false);
        }
    }
}

/// Load the viewer's people once a sheet opens (not on every page view).
fn load_companions(companions: RwSignal<Vec<CompanionDto>>) {
    if !companions.with_untracked(Vec::is_empty) {
        return;
    }
    leptos::task::spawn_local(async move {
        if let Ok(cs) = api::companions().await {
            companions.set(cs);
        }
    });
}

/// Ask before closing a changed sheet: the first close shows the discard question.
fn close_guard(
    dirty: impl Fn() -> bool + Send + Sync + 'static,
    confirm: RwSignal<bool>,
) -> Callback<(), bool> {
    Callback::new(move |()| {
        if dirty() && !confirm.get_untracked() {
            confirm.set(true);
            false
        } else {
            true
        }
    })
}

/// The "Watched with..." sheet of single and episodic items (design.md 5.4). Its title is the
/// step it logs ("Watched S2 E5"), its description the item (OT-6). Mounted once per page;
/// `info` is the item the page shows, read when the dialog opens and when it saves.
#[component]
pub fn WatchedSheet(
    state: WatchedState,
    info: RwSignal<Option<ItemInfo>>,
    companions: RwSignal<Vec<CompanionDto>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    // before the first opening (and for the ids of the panel) the plain name
    let plain = i18n.t("log-sheet-title");
    let title = Signal::derive(move || {
        Some(s.title.get())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| plain.clone())
    });
    Effect::new(move |_| {
        if let Some(label) = s.target.with(|t| t.as_ref().map(|t| t.label.clone()))
            && !s.open.get_untracked()
            && let Some(i) = info.get_untracked()
        {
            s.title.set(label);
            s.reset(&i);
            load_companions(companions);
            s.open.set(true);
        }
    });
    Effect::new(move |_| {
        if !s.open.get() {
            s.target.set(None);
        }
    });
    crate::funnel::sheet_flow(s.open);
    let ctx = StepCtx::current();
    let i2 = i18n.clone();
    // sheets stay online-only (WP4-5): offline the save waits
    let offline = crate::offline::is_offline();
    let save = Callback::new(move |()| {
        if !offline.get_untracked()
            && let Some(i) = info.get_untracked()
        {
            save_watched(s, i, ctx.clone(), i2.clone());
        }
    });
    let can_close = close_guard(move || s.dirty(), s.confirm);
    let discard = Callback::new(move |()| {
        s.initial.set(s.form());
        s.open.set(false);
    });
    let errors = Signal::derive(move || s.error.get().into_iter().collect::<Vec<_>>());
    let footer = move || {
        view! {
            <LogFooter form=WATCHED_FORM open=s.open busy=s.busy confirm=s.confirm
                discard=discard can_close=can_close errors=errors errors_id=WATCHED_ERRORS/>
        }
    };
    view! {
        <FormDialog open=s.open title=title close_label=t("dialog-close") return_to=WITH_OPENER
            description=item_title(info) can_close=can_close footer=footer>
            <WatchedFields state=s companions=companions info=info save=save/>
        </FormDialog>
    }
}

/// The item's title as the line under a dialog's title. A memo: a refetch that brings the
/// same title changes nothing in an open dialog.
fn item_title(info: RwSignal<Option<ItemInfo>>) -> MaybeProp<String> {
    let title = Memo::new(move |_| info.with(|i| i.as_ref().map(|i| i.title.clone())));
    MaybeProp::derive(move || title.get())
}

/// Ids of the two forms: the Save button in the dialog's footer names its form with them.
const WATCHED_FORM: &str = "watched-form";
const PLAY_FORM: &str = "play-form";
/// Ids of their error lists: a field that is wrong is described by its list.
const WATCHED_ERRORS: &str = "watched-errors";
const PLAY_ERRORS: &str = "play-errors";

/// The fields; built at each opening, for the item the page shows then.
#[component]
fn WatchedFields(
    state: WatchedState,
    companions: RwSignal<Vec<CompanionDto>>,
    info: RwSignal<Option<ItemInfo>>,
    save: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    let (rewatch, verb) =
        info.with_untracked(|i| i.as_ref().map_or((0, None), |i| (i.finished, i.verb)));
    let again = again_line(&i18n, verb, rewatch);
    view! {
        <form id=WATCHED_FORM class="grid gap-4" novalidate
            on:submit=move |ev| { ev.prevent_default(); save.run(()); }
            on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                ev.prevent_default();
                save.run(());
            }>
            {again.map(|a| view! { <p class="text-sm text-muted-foreground">{a}</p> })}
            <DateChips id="watched-when" chip=s.chip picked=s.picked error=s.date_error/>
            <div class="grid gap-1">
                <CompanionPicker id="watched-people" legend=t(with_legend_key(verb)) companions=companions selected=s.with
                    described_by="watched-people-hint"/>
                <p id="watched-people-hint" class="text-xs text-muted-foreground">{t("log-with-hint")}</p>
            </div>
            <Disclosure id="watched-more" title=t("log-more") summary=t("log-more-watched")
                mode=DisclosureMode::Button level=3 storage_key=disclosure_storage_key("item", "log-more")>
                <div class="grid gap-4 pt-2">
                    <div class="grid gap-1">
                        <span class="text-sm font-medium">{t("item-rating")}</span>
                        <RatingInput value=s.rating id="watched-rating"/>
                    </div>
                    <FormField id="watched-note" label=t("log-note") description=t("item-note-hint")>
                        <Textarea id="watched-note" value=s.note rows=3u32 described_by="watched-note-description"/>
                    </FormField>
                </div>
            </Disclosure>
        </form>
    }
}

fn save_watched(s: WatchedState, info: ItemInfo, ctx: StepCtx, i18n: std::sync::Arc<I18n>) {
    let Some(target) = s.target.get_untracked() else {
        return;
    };
    let form = s.form();
    let op = uuid::Uuid::now_v7().to_string();
    let payload = match form.payload(
        target.progress.clone(),
        today(),
        op,
        info.run_start.as_deref(),
    ) {
        Ok(p) => p,
        Err(e) => {
            s.date_error.set(Some(error_message(&i18n, &e)));
            return;
        }
    };
    s.date_error.set(None);
    s.error.set(None);
    s.busy.set(true);
    leptos::task::spawn_local(async move {
        let ItemInfo {
            item_id,
            title,
            rating,
            note,
            refresh,
            verb,
            ..
        } = info;
        match api::log_progress(item_id.clone(), payload).await {
            Ok(event) => {
                if form.rating != rating {
                    let _ = api::rate(item_id.clone(), form.rating).await;
                }
                if form.note.trim() != note.trim() {
                    let n = Some(form.note.trim().to_string()).filter(|n| !n.is_empty());
                    let _ = api::set_entry_note(item_id, n).await;
                }
                ctx.funnel.saved();
                ctx.funnel.logged();
                let said = crate::next_step::said_title(&ctx.i18n, verb, title, &[&event]);
                toast_logged(&ctx, &target.label, said, vec![event.id], Some(refresh));
                s.initial.set(s.form());
                s.open.set(false);
                refresh.run(());
            }
            Err(e) => {
                let text = crate::pages::error_text(&e);
                s.error.set(Some(
                    i18n.t_args("library-step-failed", &[("error", text.into())]),
                ));
            }
        }
        s.busy.set(false);
    });
}

/// State of the play sheet, owned by the page.
#[derive(Clone, Copy)]
pub struct PlayState {
    pub open: RwSignal<bool>,
    /// Players the next opening adds to you (the palette's "Log with ...", WP4-4).
    pub preset: RwSignal<Vec<String>>,
    /// The game group the next opening starts with (People's "Log a play with", WP4-6).
    pub preset_group: RwSignal<String>,
    opened: RwSignal<bool>,
    chip: RwSignal<String>,
    picked: RwSignal<String>,
    players: RwSignal<Vec<String>>,
    scores: RwSignal<BTreeMap<String, String>>,
    /// The winner as chosen by hand; `Auto`: the highest score.
    winner: RwSignal<WinnerChoice>,
    coop: RwSignal<bool>,
    /// How a co-op play ended: `none` (where it starts, OT-18), `won` or `lost`.
    coop_result: RwSignal<String>,
    duration: RwSignal<String>,
    location: RwSignal<String>,
    notes: RwSignal<String>,
    initial: RwSignal<PlayForm>,
    confirm: RwSignal<bool>,
    errors: RwSignal<Vec<String>>,
    /// What the last save found wrong, for the marks on the fields: the companions whose
    /// score is not a number, and the duration.
    bad_scores: RwSignal<Vec<String>>,
    bad_duration: RwSignal<bool>,
    busy: RwSignal<bool>,
    companions: RwSignal<Vec<CompanionDto>>,
    /// The chosen group chip (its id); empty: none.
    group: RwSignal<String>,
    groups: RwSignal<Vec<GameGroupDto>>,
}

impl PlayState {
    #[must_use]
    pub fn new(companions: RwSignal<Vec<CompanionDto>>) -> Self {
        Self {
            open: RwSignal::new(false),
            preset: RwSignal::new(Vec::new()),
            preset_group: RwSignal::new(String::new()),
            opened: RwSignal::new(false),
            chip: RwSignal::new("today".into()),
            picked: RwSignal::new(String::new()),
            players: RwSignal::new(Vec::new()),
            scores: RwSignal::new(BTreeMap::new()),
            winner: RwSignal::new(WinnerChoice::Auto),
            coop: RwSignal::new(false),
            coop_result: RwSignal::new(CoopResult::default().as_str().to_string()),
            duration: RwSignal::new(String::new()),
            location: RwSignal::new(String::new()),
            notes: RwSignal::new(String::new()),
            initial: RwSignal::new(PlayForm::default()),
            confirm: RwSignal::new(false),
            errors: RwSignal::new(Vec::new()),
            bad_scores: RwSignal::new(Vec::new()),
            bad_duration: RwSignal::new(false),
            busy: RwSignal::new(false),
            companions,
            group: RwSignal::new(String::new()),
            groups: RwSignal::new(Vec::new()),
        }
    }

    fn name(&self, id: &str) -> String {
        self.companions.with_untracked(|cs| {
            cs.iter()
                .find(|c| c.id == id)
                .map(|c| c.name.clone())
                .unwrap_or_default()
        })
    }

    fn form(&self) -> PlayForm {
        let scores = self.scores.get_untracked();
        PlayForm {
            date: chip_of(&self.chip.get_untracked()),
            picked: self.picked.get_untracked(),
            players: self
                .players
                .get_untracked()
                .into_iter()
                .map(|id| Player {
                    name: self.name(&id),
                    score: scores.get(&id).cloned().unwrap_or_default(),
                    companion_id: id,
                })
                .collect(),
            winner: self.winner.get_untracked(),
            coop: self.coop.get_untracked(),
            coop_result: CoopResult::parse(&self.coop_result.get_untracked()),
            duration: self.duration.get_untracked(),
            location: self.location.get_untracked(),
            notes: self.notes.get_untracked(),
            group: self.group.get_untracked(),
        }
    }

    /// The members of a loaded group (empty for an unknown one).
    fn members_of(&self, group: &str) -> Vec<String> {
        self.groups.with_untracked(|gs| {
            gs.iter()
                .find(|g| g.id == group)
                .map(|g| g.members.iter().map(|m| m.id.clone()).collect())
                .unwrap_or_default()
        })
    }

    fn me(&self) -> Option<String> {
        self.companions
            .with_untracked(|cs| cs.iter().find(|c| c.is_self).map(|c| c.id.clone()))
    }

    /// A group chip pressed (`on`) or released: the players become you and its members.
    fn choose_group(&self, id: &str, on: bool) {
        if on {
            self.group.set(id.to_string());
            self.players
                .set(group_players(self.me().as_deref(), &self.members_of(id)));
        } else if self.group.get_untracked() == id {
            self.group.set(String::new());
        }
    }

    /// Fresh fields; the players start as the last group that played this game, else you,
    /// and the duration as the last play's (design.md 5.4, D12; OT-16).
    fn reset(&self, last_group: Vec<String>, last_duration: Option<u32>) {
        self.chip.set("today".into());
        self.picked.set(String::new());
        self.scores.set(BTreeMap::new());
        self.winner.set(WinnerChoice::Auto);
        self.coop.set(false);
        self.coop_result
            .set(CoopResult::default().as_str().to_string());
        self.duration
            .set(last_duration.map(format_duration).unwrap_or_default());
        self.location.set(String::new());
        self.notes.set(String::new());
        let me = self.me();
        let preset = self.preset.get_untracked();
        self.preset.set(Vec::new());
        let group = self.preset_group.get_untracked();
        self.preset_group.set(String::new());
        self.group.set(group.clone());
        let players = if !group.is_empty() {
            let mut members = self.members_of(&group);
            members.extend(preset);
            group_players(me.as_deref(), &members)
        } else if !preset.is_empty() {
            let mut p: Vec<String> = me.into_iter().collect();
            p.extend(
                preset
                    .into_iter()
                    .filter(|id| !p.contains(id))
                    .collect::<Vec<_>>(),
            );
            p
        } else if last_group.is_empty() {
            me.into_iter().collect()
        } else {
            last_group
        };
        self.players.set(players);
        self.initial.set(self.form());
        self.confirm.set(false);
        self.show_errors(&[], &[]);
    }

    /// The errors of a save: one line per kind above the buttons, a mark on each field.
    fn show_errors(&self, lines: &[String], errors: &[FormError]) {
        self.errors.set(lines.to_vec());
        self.bad_scores.set(
            errors
                .iter()
                .filter_map(|e| match e {
                    FormError::Score(id) => Some(id.clone()),
                    _ => None,
                })
                .collect(),
        );
        self.bad_duration.set(errors.contains(&FormError::Duration));
    }

    fn dirty(&self) -> bool {
        self.form() != self.initial.get_untracked()
    }

    /// Close without asking (the page shows another item now).
    pub fn dismiss(&self) {
        if self.open.get_untracked() {
            self.confirm.set(false);
            self.open.set(false);
        }
    }
}

/// One line per kind of error, in the order of the form: ten scores that are not numbers
/// are one line, not ten (review OT2B-3: the list scrolled and no key reached its end).
#[must_use]
pub fn error_lines(i: &I18n, errors: &[FormError]) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for e in errors {
        let line = error_message(i, e);
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines
}

/// The game groups for the chips.
async fn load_groups(groups: RwSignal<Vec<GameGroupDto>>) {
    if let Ok(gs) = api::game_groups().await {
        groups.set(gs);
    }
}

/// The people of the newest play of `item_id` (for the prefill).
async fn last_group(item_id: &str) -> Vec<String> {
    let Ok(sessions) = api::sessions(false).await else {
        return Vec::new();
    };
    sessions
        .into_iter()
        .filter(|s| s.item_id == item_id && s.mine)
        .max_by(|a, b| a.started_at.cmp(&b.started_at))
        .map(|s| s.participants.into_iter().map(|p| p.companion_id).collect())
        .unwrap_or_default()
}

/// The "Log a play" sheet of board and video games (design.md 5.4). Mounted once per page;
/// `info` is the item the page shows, read when the dialog opens and when it saves, `title`
/// the step it logs ("Log a play", "Log a session").
#[component]
pub fn PlaySheet(
    state: PlayState,
    info: RwSignal<Option<ItemInfo>>,
    #[prop(into)] title: Signal<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    Effect::new(move |_| {
        let open = s.open.get();
        if open && !s.opened.get_untracked() {
            let Some((item_id, last_duration)) =
                info.with_untracked(|i| i.as_ref().map(|i| (i.item_id.clone(), i.last_duration)))
            else {
                // asked for before the page knows its item: open once it does
                info.track();
                return;
            };
            s.opened.set(true);
            s.busy.set(true);
            leptos::task::spawn_local(async move {
                if s.companions.with_untracked(Vec::is_empty)
                    && let Ok(cs) = api::companions().await
                {
                    s.companions.set(cs);
                }
                // the groups first only when the sheet starts with one (its members are the
                // players); otherwise the fields are ready before the chips arrive
                let preset = !s.preset_group.with_untracked(String::is_empty);
                if preset {
                    load_groups(s.groups).await;
                }
                s.reset(last_group(&item_id).await, last_duration);
                s.busy.set(false);
                if !preset {
                    load_groups(s.groups).await;
                }
            });
        } else if !open {
            s.opened.set(false);
        }
    });
    crate::funnel::sheet_flow(s.open);
    let ctx = StepCtx::current();
    let i2 = i18n.clone();
    let offline = crate::offline::is_offline();
    let save = Callback::new(move |()| {
        if !offline.get_untracked()
            && let Some(i) = info.get_untracked()
        {
            save_play(&s, i, ctx.clone(), i2.clone());
        }
    });
    let can_close = close_guard(move || s.dirty(), s.confirm);
    let discard = Callback::new(move |()| {
        s.initial.set(s.form());
        s.open.set(false);
    });
    let footer = move || {
        view! {
            <LogFooter form=PLAY_FORM open=s.open busy=s.busy confirm=s.confirm
                discard=discard can_close=can_close errors=s.errors errors_id=PLAY_ERRORS/>
        }
    };
    view! {
        <FormDialog open=s.open title=title close_label=t("dialog-close") width=DialogWidth::Wide
            return_to=PLAY_OPENER description=item_title(info) can_close=can_close footer=footer>
            <PlayFields state=s info=info save=save/>
        </FormDialog>
    }
}

/// The fields of the play dialog. A board game (`board`) has "Players" with their scores and
/// the winner open; any other game has "Played with" and keeps the scores behind "More", so
/// it opens with three inputs: when, duration, played with (OT-6, OT-16; OD-01). Built at
/// each opening, for the item the page shows then.
#[component]
fn PlayFields(
    state: PlayState,
    info: RwSignal<Option<ItemInfo>>,
    save: Callback<()>,
) -> impl IntoView {
    let s = state;
    let board = info.with_untracked(|i| i.as_ref().is_some_and(|i| i.board_game));
    // a wrong duration is marked and points at the line that says why
    let duration_described = Signal::derive(move || {
        if s.bad_duration.get() {
            format!("play-duration-description {PLAY_ERRORS}")
        } else {
            "play-duration-description".to_string()
        }
    });
    let no_error = Signal::derive(|| None::<String>);
    let (legend, more) = if board {
        ("log-players", "log-more-play")
    } else {
        (with_legend_key(Some(Verb::Play)), "log-more-session")
    };
    view! {
        <form id=PLAY_FORM class="grid gap-4" novalidate
            on:submit=move |ev| { ev.prevent_default(); save.run(()); }
            on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                ev.prevent_default();
                save.run(());
            }>
            // row 1 as in the mockup (log.html): the date and the duration, open (OT-16)
            <div class="flex flex-wrap items-start gap-x-4 gap-y-4">
                <DateChips id="play-when" chip=s.chip picked=s.picked error=no_error/>
                // "1:30" or "90" (OT-16): a text field, the colon is not on a numeric keypad
                // (the fixed width keeps it beside the date in the 342 px of a 390 px phone;
                // the hint wraps inside it)
                <div class="w-[7.5rem]">
                    <FormField id="play-duration" label=t("log-duration") description=t("log-duration-hint")>
                        <Input id="play-duration" value=s.duration autocomplete="off" class="max-w-24"
                            invalid=s.bad_duration described_by=duration_described/>
                    </FormField>
                </div>
            </div>
            <GroupChips state=s/>
            <CompanionPicker id="play-people" legend=t(legend) companions=s.companions selected=s.players include_self=true/>
            {board.then(|| view! { <Scores state=s/> })}
            <Disclosure id="play-more" title=t("log-more") summary=t(more)
                mode=DisclosureMode::Button level=3 storage_key=disclosure_storage_key("item", "play-more")>
                {(!board).then(|| view! { <Scores state=s/> })}
                <PlayMore state=s/>
            </Disclosure>
        </form>
    }
}

/// One chip per game group (design.md 6.7): pressing one makes the players you and its
/// members, and the play is logged as the group.
#[component]
fn GroupChips(state: PlayState) -> impl IntoView {
    let s = state;
    let groups = move || s.groups.get();
    view! {
        <Show when=move || !s.groups.with(Vec::is_empty)>
            <div role="group" aria-labelledby="play-groups-legend" class="grid gap-2" data-testid="play-groups">
                <span id="play-groups-legend" class="text-sm font-medium">{use_i18n().t("log-groups")}</span>
                <div class="flex flex-wrap gap-2">
                    <For each=groups key=|g| g.id.clone() let:g>
                        {
                            let gid = g.id.clone();
                            let watch = gid.clone();
                            let pressed = RwSignal::new(s.group.get_untracked() == gid);
                            Effect::new(move |_| pressed.set(s.group.with(|x| *x == watch)));
                            view! {
                                <Chip pressed=pressed on_change=Callback::new(move |on: bool| s.choose_group(&gid, on))>
                                    {g.name.clone()}
                                </Chip>
                            }
                        }
                    </For>
                </div>
            </div>
        </Show>
    }
}

/// The id of a player's score field and of their "won" radio: from the companion id, so a
/// row keeps its ids when players before it leave or join (review OT2F-1).
fn score_id(companion: &str) -> String {
    format!("play-score-{companion}")
}

fn won_id(companion: &str) -> String {
    format!("play-won-{companion}")
}

/// The radio of the last row of the score table: "No winner".
const WON_NONE: &str = "play-won-none";
const WON_HINT: &str = "play-scores-hint";

/// Move the keyboard focus to the radio that is marked now (review OT2F-3: "Use the highest
/// score" leaves the page when it is used, and the focus with it).
fn focus_mark(winner: Option<&str>) {
    use leptos::wasm_bindgen::JsCast;
    let id = winner.map_or_else(|| WON_NONE.to_string(), won_id);
    if let Some(el) = document()
        .get_element_by_id(&id)
        .and_then(|el| el.dyn_into::<leptos::web_sys::HtmlElement>().ok())
    {
        let _ = el.focus();
    }
}

/// What a screen reader hears when the mark follows the scores (review OT2F-6): the name of
/// the player the highest score marks, or that the mark is gone. Nothing while the person
/// chose the winner, and nothing before the first mark.
fn winner_live(s: &PlayState, winner: Memo<Option<String>>) -> RwSignal<String> {
    let s = *s;
    let live = RwSignal::new(String::new());
    let i18n = use_i18n();
    Effect::new(move |before: Option<Option<String>>| {
        let now = winner.get();
        let auto = s.winner.with(|w| *w == WinnerChoice::Auto) && !s.coop.get();
        if let Some(before) = before
            && auto
            && before != now
        {
            live.set(match &now {
                Some(id) => i18n.t_args("log-winner-live", &[("name", s.name(id).into())]),
                None => i18n.t("log-winner-live-none"),
            });
        }
        now
    });
    live
}

/// The scores as one compact table (ux-owner-test.md 2.3): a row per player with the score
/// field and a "won" radio, and a last row "No winner". The mark follows the highest score
/// of at least two until the person chooses ("Use the highest score" takes the choice back);
/// a co-op play has no such column.
#[component]
fn Scores(state: PlayState) -> impl IntoView {
    let s = state;
    // the winner as a save would send it
    let winner = Memo::new(move |_| {
        s.players.track();
        s.scores.track();
        s.winner.track();
        s.coop.track();
        s.form().winner()
    });
    // a winner chosen by hand who left the table is no choice any more (review OT2F-5)
    Effect::new(move |_| {
        let gone = s.players.with(|players| {
            s.winner
                .with_untracked(|w| matches!(w, WinnerChoice::Player(id) if !players.contains(id)))
        });
        if gone {
            s.winner.set(WinnerChoice::Auto);
        }
    });
    let live = winner_live(&s, winner);
    let reset = move |_| {
        s.winner.set(WinnerChoice::Auto);
        focus_mark(s.form().winner().as_deref());
    };
    let rows = move || s.players.get();
    let head = "px-2 py-1 text-left text-xs font-medium text-muted-foreground";
    let described = move || (!s.coop.get()).then_some(WON_HINT);
    view! {
        <div class="grid gap-2" data-testid="play-scores">
            // fixed columns: the name takes what the score and the mark leave and is cut
            // there, so the table fits a 320 px sheet (review OT2F-11)
            <table class="w-full max-w-md table-fixed border-collapse text-sm" aria-describedby=described>
                <caption class="sr-only">{t("log-scores")}</caption>
                <thead>
                    <tr class="border-b border-border">
                        <th scope="col" class=head>{t("log-scores-player")}</th>
                        <th scope="col" class=cn(&[head, "w-28"])>{t("log-scores-score")}</th>
                        <Show when=move || !s.coop.get()>
                            <th scope="col" class=cn(&[head, "w-16"])>{use_i18n().t("log-scores-won")}</th>
                        </Show>
                    </tr>
                </thead>
                <tbody>
                    <For each=rows key=Clone::clone let:id>
                        <ScoreRow state=s id=id winner=winner/>
                    </For>
                </tbody>
                <Show when=move || !s.coop.get()>
                    <tfoot>
                        <tr class="border-t border-border">
                            <td id="play-won-none-label" colspan="2" class="px-2 py-1 text-muted-foreground">
                                {use_i18n().t("log-winner-none")}
                            </td>
                            <td class="px-2 py-1">
                                <label r#for=WON_NONE class="flex size-9 cursor-pointer items-center justify-center">
                                    <input id=WON_NONE type="radio" name="play-winner" class=ui::cn(&["size-4", ui::RADIO])
                                        aria-labelledby="play-won-none-label"
                                        prop:checked=move || winner.get().is_none()
                                        on:change=move |_| s.winner.set(WinnerChoice::Nobody)/>
                                </label>
                            </td>
                        </tr>
                    </tfoot>
                </Show>
            </table>
            <Show when=move || !s.coop.get()>
                <p id=WON_HINT class="text-xs text-muted-foreground">{use_i18n().t("log-winner-hint")}</p>
            </Show>
            <p class="sr-only" role="status" data-testid="play-winner-live">{move || live.get()}</p>
            <Show when=move || !s.coop.get() && s.winner.with(|w| *w != WinnerChoice::Auto)>
                <div>
                    <Button r#type="button" variant=ButtonVariant::Link size=ButtonSize::Sm on:click=reset>
                        {use_i18n().t("log-winner-reset")}
                    </Button>
                </div>
            </Show>
        </div>
    }
}

/// One player of the score table.
#[component]
fn ScoreRow(state: PlayState, id: String, winner: Memo<Option<String>>) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    let name = s.name(&id);
    let field = score_id(&id);
    let value = RwSignal::new(
        s.scores
            .with_untracked(|m| m.get(&id).cloned().unwrap_or_default()),
    );
    let key = id.clone();
    Effect::new(move |_| {
        let v = value.get();
        s.scores.update(|m| {
            m.insert(key.clone(), v);
        });
    });
    let label = i18n.t_args("log-score", &[("name", name.clone().into())]);
    let won_label = i18n.t_args("log-scores-won-name", &[("name", name.clone().into())]);
    let bad_id = id.clone();
    let bad = Signal::derive(move || s.bad_scores.with(|b| b.contains(&bad_id)));
    let described = Signal::derive(move || bad.get().then(|| PLAY_ERRORS.to_string()));
    let (mine, pick) = (id.clone(), id.clone());
    let radio = won_id(&id);
    view! {
        <tr class="border-b border-border last:border-0">
            <th scope="row" class="truncate px-2 py-1 text-left font-normal">{name}</th>
            <td class="px-2 py-1">
                <label r#for=field.clone() class="sr-only">{label}</label>
                <Input id=field.clone() value=value inputmode="decimal" autocomplete="off" class="max-w-24"
                    invalid=bad described_by=described/>
            </td>
            <Show when=move || !s.coop.get()>
                {
                    let (mine, pick, radio, won_label) = (mine.clone(), pick.clone(), radio.clone(), won_label.clone());
                    view! {
                        <td class="px-2 py-1">
                            // 36 px around the radio: the target size of WCAG 2.5.8
                            <label r#for=radio.clone() class="flex size-9 cursor-pointer items-center justify-center">
                                <input id=radio.clone() type="radio" name="play-winner" class=ui::cn(&["size-4", ui::RADIO])
                                    prop:checked=move || winner.get().as_deref() == Some(mine.as_str())
                                    on:change=move |_| s.winner.set(WinnerChoice::Player(pick.clone()))/>
                                <span class="sr-only">{won_label}</span>
                            </label>
                        </td>
                    }
                }
            </Show>
        </tr>
    }
}

/// The rarer play fields behind "More".
#[component]
fn PlayMore(state: PlayState) -> impl IntoView {
    let s = state;
    view! {
        <div class="grid gap-4 pt-2">
            <div class="flex items-center gap-3">
                <Switch id="play-coop" checked=s.coop/>
                <Label r#for="play-coop">{t("log-coop")}</Label>
            </div>
            <Show when=move || s.coop.get()>
                {
                    let i = use_i18n();
                    let options = vec![
                        SegmentedOption::new("none", i.t("log-coop-none")),
                        SegmentedOption::new("won", i.t("log-coop-won")),
                        SegmentedOption::new("lost", i.t("log-coop-lost")),
                    ];
                    view! { <Segmented id="play-coop-result" label=i.t("log-coop-result") value=s.coop_result options=options/> }
                }
            </Show>
            <FormField id="play-location" label=t("log-location")>
                <Input id="play-location" value=s.location autocomplete="off"/>
            </FormField>
            <FormField id="play-notes" label=t("log-notes")>
                <Textarea id="play-notes" value=s.notes rows=3u32/>
            </FormField>
        </div>
    }
}

fn save_play(s: &PlayState, info: ItemInfo, ctx: StepCtx, i18n: std::sync::Arc<I18n>) {
    let s = *s;
    let op = uuid::Uuid::now_v7().to_string();
    let payload = match s.form().payload(today(), op) {
        Ok(p) => p,
        Err(errors) => {
            s.show_errors(&error_lines(&i18n, &errors), &errors);
            return;
        }
    };
    s.show_errors(&[], &[]);
    s.busy.set(true);
    leptos::task::spawn_local(async move {
        // a board game is played in plays, a video game in sessions (v1-polish P3)
        let label = i18n.t(crate::item_words::play_step_key(info.board_game));
        let ItemInfo {
            item_id,
            title,
            refresh,
            ..
        } = info;
        match api::log_play(item_id, payload).await {
            Ok(event) => {
                ctx.funnel.saved();
                ctx.funnel.logged();
                toast_logged(&ctx, &label, title, vec![event.id], Some(refresh));
                s.initial.set(s.form());
                s.open.set(false);
                refresh.run(());
            }
            Err(e) => {
                let text = crate::pages::error_text(&e);
                s.errors.set(vec![
                    i18n.t_args("library-step-failed", &[("error", text.into())]),
                ]);
            }
        }
        s.busy.set(false);
    });
}

/// The typed text and the error line of the inline page or position field, owned by the item
/// page: every log refreshes the page (and its `entity.updated` event once more), and each
/// refetch renders the action bar afresh, so state held by the field itself lost a typed
/// "soon" and its "Enter a page number from 1 on." under a late refetch (DEC-477).
#[derive(Clone, Copy)]
pub struct InlineDraft {
    /// What the person typed while it differs from the stored value; `None` shows the stored one.
    pub text: RwSignal<Option<String>>,
    pub error: RwSignal<Option<String>>,
    /// The text of the page total editor while it is open ("Change" next to the page
    /// field); `None`: closed.
    pub total: RwSignal<Option<String>>,
    /// The page total was just saved: the action bar is rendered afresh with the refetched
    /// page, and the new "Change" button takes the focus the old one had.
    pub refocus: RwSignal<bool>,
}

impl InlineDraft {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: RwSignal::new(None),
            error: RwSignal::new(None),
            total: RwSignal::new(None),
            refocus: RwSignal::new(false),
        }
    }

    /// Forget the draft and its error (another item was opened).
    pub fn clear(self) {
        self.text.set(None);
        self.error.set(None);
        self.total.set(None);
    }

    /// The field's signal: starts from the draft (else `stored`) and keeps the draft in step.
    fn field(self, stored: String) -> RwSignal<String> {
        let value = RwSignal::new(self.text.get_untracked().unwrap_or_else(|| stored.clone()));
        Effect::new(move |_| {
            let v = value.get();
            let typed = (v != stored).then_some(v);
            if self.text.with_untracked(|t| *t != typed) {
                self.text.set(typed);
            }
        });
        value
    }

    /// `refresh` after a log that went through, with the draft dropped first.
    fn done(self, refresh: Callback<()>) -> Callback<()> {
        Callback::new(move |()| {
            self.text.set(None);
            refresh.run(());
        })
    }
}

impl Default for InlineDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// Mark the entry completed (the "Finished" of pages and positions); a page or position at
/// the end does not complete an entry on its own. Offline the status goes into the queue
/// (review BOOK-6), which says "Saved offline, will sync".
fn complete(item: (String, String), ctx: &StepCtx, refresh: Callback<()>) {
    let word = ctx.i18n.t("status-completed");
    let done = ctx
        .i18n
        .t_args("library-status-changed", &[("status", word.into())]);
    let ctx = ctx.clone();
    leptos::task::spawn_local(async move {
        let (item_id, title) = item;
        match api::set_status(item_id.clone(), COMPLETED.into()).await {
            Ok(_) => {
                ctx.toasts.push(done, None, ui::ToastVariant::Default);
            }
            Err(e) if crate::offline::network_error(&e) => {
                ctx.queue_field(title, |hlc| {
                    crate::offline::status_command(&item_id, COMPLETED, hlc)
                });
            }
            Err(_) => {}
        }
        // the page that asked may be gone by now (another page, a re-render): nothing to
        // refetch then (DEC-870)
        let _ = refresh.try_run(());
    });
}

/// The status "Finished" sets.
const COMPLETED: &str = "completed";

/// Page field with Update, +10, +25 and Finished (design.md 5.4 Pages); Enter saves. With a
/// known total the field is followed by "of N pages" ("(estimate)" for a median the source
/// gave), and whoever may change the shared item gets "Change" (or "Set page total") there.
#[component]
pub fn PagesInline(
    info: ItemInfo,
    step: NextStepDto,
    draft: InlineDraft,
    #[prop(optional)] total: PageTotal,
) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let value = draft.field(step.page.map(|p| p.to_string()).unwrap_or_default());
    let (busy, error) = (RwSignal::new(false), draft.error);
    let (pages, at) = (step.pages, step.page);
    Effect::new(move |_| {
        if draft.refocus.get_untracked() {
            draft.refocus.set(false);
            request_animation_frame(|| crate::auth_pages::focus(TOTAL_OPENER));
        }
    });
    let info = StoredValue::new(info);
    let i2 = i18n.clone();
    // `then`: "Finished" completes once the page is logged (not after an undo), or queues
    // Completed behind the page when offline
    let log = move |text: String, then: Option<Then>| match page_progress(&text, pages) {
        Err(e) => error.set(Some(error_message(&i2, &e))),
        Ok(progress) => {
            let ItemInfo {
                item_id,
                title,
                refresh,
                ..
            } = info.get_value();
            let label = progress_label(&i2, &progress);
            let step = LoggedStep {
                item_id,
                title,
                label,
                progress,
                verb: None,
            };
            let done = Some(draft.done(refresh));
            match then {
                Some(then) => run_step_then(ctx.clone(), step, busy, error, done, then),
                None => run_step(ctx.clone(), step, busy, error, done),
            }
        }
    };
    let log = StoredValue::new(log);
    let plus = move |n: u32| {
        let now = value.get_untracked().trim().parse::<u32>().unwrap_or(0);
        let next = pages.map_or(now + n, |p| (now + n).min(p));
        value.set(next.to_string());
        log.with_value(|l| l(next.to_string(), None));
    };
    let ctx2 = StepCtx::current();
    // with a known total, finishing counts the pages that were left: the last page is logged
    // first, then the entry is completed (v1-books, G16)
    let finish = move |_| {
        let ItemInfo {
            item_id,
            title,
            refresh,
            ..
        } = info.get_value();
        let ctx = ctx2.clone();
        let then = Then::new(
            move || complete((item_id.clone(), title.clone()), &ctx, refresh),
            COMPLETED,
        );
        match finish_page(at, pages) {
            Some(last) => log.with_value(|l| l(last.to_string(), Some(then))),
            None => then.run(),
        }
    };
    let initial = value.get_untracked();
    // the total stands beside the field: it is part of what the field is about
    let described = if pages.is_some() {
        "pages-at-error pages-of"
    } else {
        "pages-at-error"
    };
    view! {
        <div class="grid gap-2">
            <form class="grid gap-2" novalidate on:submit=move |ev| { ev.prevent_default(); log.with_value(|l| l(value.get_untracked(), None)); }>
                <div class="flex flex-wrap items-end gap-2">
                    <FormField id="pages-at" label=t("log-page")>
                        <Input id="pages-at" value=value inputmode="numeric" autocomplete="off" class="w-24"
                            invalid=Signal::derive(move || error.with(Option::is_some)) described_by=described attr:value=initial/>
                    </FormField>
                    <PageTotalLine pages=pages total=total draft=draft/>
                    <Button r#type="submit" size=ButtonSize::Lg loading=busy>{t("log-update")}</Button>
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Lg on:click=move |_| plus(10)>{use_i18n().t_args("log-plus-pages", &[("count", 10.into())])}</Button>
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Lg on:click=move |_| plus(25)>{use_i18n().t_args("log-plus-pages", &[("count", 25.into())])}</Button>
                    <Button variant=ButtonVariant::Outline size=ButtonSize::Lg disabled=busy on:click=finish>{t("log-finished")}</Button>
                </div>
                <p id="pages-at-error" role="alert" class="text-sm text-destructive-text">{move || error.get()}</p>
            </form>
            {move || draft.total.with(Option::is_some).then(|| view! { <PageTotalForm info=info.get_value() draft=draft/> })}
        </div>
    }
}

/// "of N pages" after the page field ("(estimate)" for a source's median), and for whoever
/// may change the shared item the button that opens the page total form: "Change", or
/// "Set page total" when the item has none.
#[component]
fn PageTotalLine(pages: Option<u32>, total: PageTotal, draft: InlineDraft) -> impl IntoView {
    let of = pages.map(|p| {
        let key = if total.estimated {
            "log-of-pages-estimate"
        } else {
            "log-of-pages"
        };
        use_i18n().t_args(key, &[("pages", p.into())])
    });
    let change = total.can_edit.then(|| {
        let label = if pages.is_some() {
            t("log-pages-change")
        } else {
            t("log-pages-set")
        };
        // "Change" alone does not say what; the name does (WCAG 2.5.3 keeps the visible word)
        let aria = pages.is_some().then(|| t("log-pages-change-label"));
        let open = move |_| {
            draft
                .total
                .set(Some(pages.map(|p| p.to_string()).unwrap_or_default()));
            request_animation_frame(|| crate::auth_pages::focus(TOTAL_FIELD));
        };
        view! {
            <Button variant=ButtonVariant::Link size=ButtonSize::Sm attr:id=TOTAL_OPENER class="mb-1"
                attr:aria-label=aria
                attr:aria-expanded=move || if draft.total.with(Option::is_some) { "true" } else { "false" }
                on:click=open>{label}</Button>
        }
    });
    view! {
        {of.map(|o| view! { <span id="pages-of" class="pb-2 text-sm text-muted-foreground" data-testid="pages-of">{o}</span> })}
        {change}
    }
}

/// DOM ids of the page total editor and of the button that opens it.
const TOTAL_FIELD: &str = "pages-total";
const TOTAL_OPENER: &str = "pages-total-open";

/// The small form behind "Change": one number, Save and Cancel. Saving writes the total to
/// the shared item as set by a person (it survives the source's refreshes) and refetches.
#[component]
fn PageTotalForm(info: ItemInfo, draft: InlineDraft) -> impl IntoView {
    let i18n = use_i18n();
    let value = RwSignal::new(draft.total.get_untracked().unwrap_or_default());
    // the typed text lives with the page, like the page field's (DEC-477)
    Effect::new(move |_| {
        let v = value.get();
        if draft
            .total
            .with_untracked(|t| t.as_ref().is_some_and(|t| *t != v))
        {
            draft.total.set(Some(v));
        }
    });
    let (busy, error) = (RwSignal::new(false), RwSignal::new(None::<String>));
    let close = move || {
        draft.total.set(None);
        crate::auth_pages::focus(TOTAL_OPENER);
    };
    let toasts = ui::use_toasts();
    let ItemInfo {
        item_id, refresh, ..
    } = info;
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(pages) = parse_page_total(&value.get_untracked()) else {
            error.set(Some(i18n.t("log-error-pages-total")));
            return;
        };
        error.set(None);
        busy.set(true);
        let (item_id, i18n) = (item_id.clone(), i18n.clone());
        leptos::task::spawn_local(async move {
            match api::set_page_count(item_id, pages).await {
                Ok(_) => {
                    toasts.push(
                        i18n.t_args("log-pages-total-saved", &[("pages", pages.into())]),
                        None,
                        ui::ToastVariant::Default,
                    );
                    close();
                    draft.refocus.set(true);
                    refresh.run(());
                }
                Err(e) => error.set(Some(crate::pages::error_text(&e))),
            }
            busy.set(false);
        });
    };
    view! {
        <form class="grid gap-2" novalidate on:submit=save data-testid="pages-total-form">
            <div class="flex flex-wrap items-end gap-2">
                <FormField id=TOTAL_FIELD label=t("log-pages-total") description=t("log-pages-total-hint")>
                    <Input id=TOTAL_FIELD value=value inputmode="numeric" autocomplete="off" class="w-24"
                        invalid=Signal::derive(move || error.with(Option::is_some))
                        described_by=format!("{} pages-total-error", ui::field_described_by(TOTAL_FIELD))/>
                </FormField>
                <Button r#type="submit" loading=busy>{t("log-save")}</Button>
                <Button variant=ButtonVariant::Outline on:click=move |_| close()>{t("log-cancel")}</Button>
            </div>
            <p id="pages-total-error" role="alert" class="text-sm text-destructive-text">{move || error.get()}</p>
        </form>
    }
}

/// Position field (`h:mm:ss`) with Update, +15 min and Finished (design.md 5.4 Position).
#[component]
pub fn PositionInline(info: ItemInfo, step: NextStepDto, draft: InlineDraft) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let value = draft.field(step.seconds.map(format_position).unwrap_or_default());
    let start = value.get_untracked();
    let (busy, error) = (RwSignal::new(false), draft.error);
    let total = step.total_seconds;
    let info = StoredValue::new(info);
    let i2 = i18n.clone();
    let log = move |text: String| match position_progress(&text, total) {
        Err(e) => error.set(Some(error_message(&i2, &e))),
        Ok(progress) => {
            let ItemInfo {
                item_id,
                title,
                refresh,
                ..
            } = info.get_value();
            let label = progress_label(&i2, &progress);
            let step = LoggedStep {
                item_id,
                title,
                label,
                progress,
                verb: None,
            };
            run_step(ctx.clone(), step, busy, error, Some(draft.done(refresh)));
        }
    };
    let log = StoredValue::new(log);
    let plus = move |_| {
        let now = crate::log_form::parse_position(&value.get_untracked()).unwrap_or(0);
        let next = total.map_or(now + 900, |t| (now + 900).min(t));
        value.set(format_position(next));
        log.with_value(|l| l(format_position(next)));
    };
    let ctx2 = StepCtx::current();
    let finish = move |_| {
        if let Some(t) = total {
            log.with_value(|l| l(format_position(t)));
        }
        let ItemInfo {
            item_id,
            title,
            refresh,
            ..
        } = info.get_value();
        complete((item_id, title), &ctx2, refresh);
    };
    let of = total.map(|t| i18n.t_args("log-of-time", &[("time", format_position(t).into())]));
    view! {
        <form class="grid gap-2" novalidate on:submit=move |ev| { ev.prevent_default(); log.with_value(|l| l(value.get_untracked())); }>
            <div class="flex flex-wrap items-end gap-2">
                <FormField id="position-at" label=t("log-position") description=t("log-position-hint")>
                    <Input id="position-at" value=value autocomplete="off" class="w-28"
                        invalid=Signal::derive(move || error.with(Option::is_some))
                        described_by="position-at-description position-at-error" attr:value=start/>
                </FormField>
                {of.map(|o| view! { <span class="pb-2 text-sm text-muted-foreground">{o}</span> })}
                <Button r#type="submit" size=ButtonSize::Lg loading=busy>{t("log-update")}</Button>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Lg on:click=plus>{use_i18n().t_args("log-plus-minutes", &[("count", 15.into())])}</Button>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Lg on:click=finish>{t("log-finished")}</Button>
            </div>
            <p id="position-at-error" role="alert" class="text-sm text-destructive-text">{move || error.get()}</p>
        </form>
    }
}

/// The footer of a log dialog (OT-10): the discard question and what went wrong (a line per
/// kind of error, so the list needs no scroller of its own, review OT2B-3), above the
/// buttons, then Cancel and Save. Save submits the form named `form` and is
/// disabled with "You are offline" next to it while the browser is offline (the dialogs are
/// not queued, WP4-5). Cancel asks `can_close` like Escape and the close button do. Below
/// 640 px (the bottom sheet) Save is 44 px high and as wide as the sheet, and Cancel is left
/// to the close button (addendum-1 D10).
#[component]
fn LogFooter(
    form: &'static str,
    open: RwSignal<bool>,
    busy: RwSignal<bool>,
    confirm: RwSignal<bool>,
    discard: Callback<()>,
    can_close: Callback<(), bool>,
    #[prop(into)] errors: Signal<Vec<String>>,
    /// Id of the error list; the fields that are wrong name it in `aria-describedby`.
    errors_id: &'static str,
) -> impl IntoView {
    let offline = crate::offline::is_offline();
    let cancel = move |_| {
        if can_close.run(()) {
            open.set(false);
        }
    };
    view! {
        <div class="grid gap-3">
            <DiscardPrompt confirm=confirm discard=discard/>
            {move || {
                let errors = errors.get();
                // the alert is around the list: a `ul` with that role is no list any more
                (!errors.is_empty()).then(|| view! {
                    <div id=errors_id role="alert">
                        <ul class="grid gap-1 text-sm text-destructive-text">
                            {errors.into_iter().map(|e| view! { <li>{e}</li> }).collect_view()}
                        </ul>
                    </div>
                })
            }}
            <div class="flex flex-wrap items-center justify-end gap-2">
                <Show when=move || offline.get()>
                    <p class="mr-auto text-small text-muted-foreground">{t("offline-sheet")}</p>
                </Show>
                <Button variant=ButtonVariant::Outline class="max-sm:hidden" on:click=cancel>{t("log-cancel")}</Button>
                <Button r#type="submit" attr:form=form loading=busy disabled=offline class="max-sm:h-11 max-sm:w-full">
                    {t("log-save")}
                </Button>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    /// Review OT2B-3: ten players with "42 pts" were ten equal lines in a list that scrolled.
    #[test]
    fn errors_are_one_line_per_kind_in_the_order_of_the_form() {
        let en = I18n::new("en", BTreeMap::new());
        let errors = [
            FormError::DateInFuture,
            FormError::Score("a".into()),
            FormError::Score("b".into()),
            FormError::Score("c".into()),
            FormError::Duration,
        ];
        assert_eq!(
            error_lines(&en, &errors),
            [
                "The date is in the future.",
                "Scores are numbers, for example 42 or 12.5.",
                "Enter the duration as hours:minutes (1:30) or as minutes (90).",
            ]
        );
        assert_eq!(error_lines(&en, &[]).len(), 0);
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(error_lines(&de, &errors[1..4]).len(), 1);
    }

    /// Review OT2E-1: the message names the day the run started.
    #[test]
    fn a_day_before_a_rewatch_names_the_day_it_started() {
        let en = I18n::new("en", BTreeMap::new());
        let de = I18n::new("de", BTreeMap::new());
        let e = FormError::BeforeRun("2026-03-09T15:30:00+00:00".into());
        assert_eq!(
            error_message(&en, &e),
            "You started again on Mar 9, 2026. Pick that day or a later one."
        );
        assert_eq!(
            error_message(&de, &e),
            "Du hast am 09.03.2026 nochmal von vorn begonnen. W\u{e4}hle diesen Tag oder einen sp\u{e4}teren."
        );
    }
}
