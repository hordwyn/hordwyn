//! Log sheets and inline loggers of the item page (design.md 5.4, 7.3; addendum-1 1.3 "5.4
//! Log sheets"): `WatchedSheet` (date chips, people, rating and note behind "More"),
//! `PlaySheet` (players, scores, winner; duration, place, notes and co-op behind "More"),
//! `PagesInline` and `PositionInline`. Every save shows "Logged: ..." with Undo; a failure
//! keeps the sheet open with the data and an inline error; closing a changed sheet asks
//! first. The sheets' state lives in the page (a refetch never resets an open sheet); the
//! form rules are in [`crate::log_form`].

use std::collections::BTreeMap;

use contracts::{CompanionDto, NextStepDto};
use leptos::html;
use leptos::prelude::*;
use ui::{
    Button, ButtonSize, ButtonVariant, Disclosure, DisclosureMode, FormField, Input, Label,
    Segmented, SegmentedOption, Select, SelectOption, Sheet, SheetSide, Switch, Textarea,
    disclosure_storage_key,
};

use crate::api;
use crate::history_state::Day;
use crate::i18n::{I18n, t, use_i18n};
use crate::log_form::{
    DateChip, FormError, PlayForm, Player, WatchedForm, format_position, page_progress,
    position_progress,
};
use crate::next_step::{LoggedStep, StepCtx, run_step, toast_logged};
use crate::widgets::{CompanionPicker, RatingInput, progress_label};

/// What the sheets need to know about the item.
#[derive(Clone)]
pub struct ItemInfo {
    pub item_id: String,
    pub title: String,
    pub rating: Option<u8>,
    pub note: String,
    /// Times finished so far (the sheet says when a log is a rewatch).
    pub finished: u32,
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

/// "Discard your changes?" inside a sheet that was asked to close while changed (D15: no
/// second dialog). Focus moves to "Keep editing".
#[component]
fn DiscardPrompt(confirm: RwSignal<bool>, discard: Callback<()>) -> impl IntoView {
    let keep = NodeRef::<html::Button>::new();
    Effect::new(move |_| {
        if confirm.get()
            && let Some(b) = keep.get()
        {
            let _ = b.focus();
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
            <div role="group" aria-label=question.clone() class="grid gap-2 rounded-md border border-warning/50 bg-warning-soft p-3">
                <p class="text-sm font-medium">{question.clone()}</p>
                <div class="flex flex-wrap gap-2">
                    <button node_ref=keep type="button" on:click=move |_| confirm.set(false)
                        class="inline-flex h-9 items-center rounded-md border border-input bg-background px-3 text-sm font-medium hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                        {keep_label.clone()}
                    </button>
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
    open: RwSignal<bool>,
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
            open: RwSignal::new(false),
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
        self.with.set(Vec::new());
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

/// The "With..." sheet of single and episodic items (design.md 5.4).
#[component]
pub fn WatchedSheet(
    state: WatchedState,
    info: ItemInfo,
    companions: RwSignal<Vec<CompanionDto>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    let info = StoredValue::new(info);
    Effect::new(move |_| {
        if s.target.with(Option::is_some) && !s.open.get_untracked() {
            info.with_value(|i| s.reset(i));
            load_companions(companions);
            s.open.set(true);
        }
    });
    Effect::new(move |_| {
        if !s.open.get() {
            s.target.set(None);
        }
    });
    let ctx = StepCtx::current();
    let i2 = i18n.clone();
    let save = Callback::new(move |()| save_watched(s, info.get_value(), ctx.clone(), i2.clone()));
    let can_close = close_guard(move || s.dirty(), s.confirm);
    let discard = Callback::new(move |()| {
        s.initial.set(s.form());
        s.open.set(false);
    });
    let rewatch = info.with_value(|i| i.finished);
    view! {
        <Sheet open=s.open title=t("log-sheet-title") close_label=t("dialog-close")
            description=info.with_value(|i| i.title.clone()) side=SheetSide::Adaptive can_close=can_close>
            <WatchedFields state=s companions=companions rewatch=rewatch save=save discard=discard/>
        </Sheet>
    }
}

#[component]
fn WatchedFields(
    state: WatchedState,
    companions: RwSignal<Vec<CompanionDto>>,
    rewatch: u32,
    save: Callback<()>,
    discard: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    let step = move || s.target.with(|t| t.as_ref().map(|t| t.label.clone()));
    let again = (rewatch > 0).then(|| i18n.t_args("log-rewatch", &[("count", rewatch.into())]));
    view! {
        <form class="grid gap-4" novalidate
            on:submit=move |ev| { ev.prevent_default(); save.run(()); }
            on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                ev.prevent_default();
                save.run(());
            }>
            <p class="text-sm font-medium" data-testid="sheet-step">{step}</p>
            {again.map(|a| view! { <p class="text-sm text-muted-foreground">{a}</p> })}
            <DateChips id="watched-when" chip=s.chip picked=s.picked error=s.date_error/>
            <CompanionPicker id="watched-people" legend=t("log-people") companions=companions selected=s.with/>
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
            <DiscardPrompt confirm=s.confirm discard=discard/>
            {move || s.error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
            <div class="flex justify-end gap-2">
                <Button r#type="submit" loading=s.busy>{t("log-save")}</Button>
            </div>
        </form>
    }
}

fn save_watched(s: WatchedState, info: ItemInfo, ctx: StepCtx, i18n: std::sync::Arc<I18n>) {
    let Some(target) = s.target.get_untracked() else {
        return;
    };
    let form = s.form();
    let op = uuid::Uuid::now_v7().to_string();
    let payload = match form.payload(target.progress.clone(), today(), op) {
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
                toast_logged(&ctx, &target.label, title, vec![event.id], Some(refresh));
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
    opened: RwSignal<bool>,
    chip: RwSignal<String>,
    picked: RwSignal<String>,
    players: RwSignal<Vec<String>>,
    scores: RwSignal<BTreeMap<String, String>>,
    /// Companion id of a winner chosen by hand; empty: the highest score.
    winner: RwSignal<String>,
    coop: RwSignal<bool>,
    coop_won: RwSignal<bool>,
    duration: RwSignal<String>,
    location: RwSignal<String>,
    notes: RwSignal<String>,
    initial: RwSignal<PlayForm>,
    confirm: RwSignal<bool>,
    errors: RwSignal<Vec<String>>,
    busy: RwSignal<bool>,
    companions: RwSignal<Vec<CompanionDto>>,
}

impl PlayState {
    #[must_use]
    pub fn new(companions: RwSignal<Vec<CompanionDto>>) -> Self {
        Self {
            open: RwSignal::new(false),
            opened: RwSignal::new(false),
            chip: RwSignal::new("today".into()),
            picked: RwSignal::new(String::new()),
            players: RwSignal::new(Vec::new()),
            scores: RwSignal::new(BTreeMap::new()),
            winner: RwSignal::new(String::new()),
            coop: RwSignal::new(false),
            coop_won: RwSignal::new(true),
            duration: RwSignal::new(String::new()),
            location: RwSignal::new(String::new()),
            notes: RwSignal::new(String::new()),
            initial: RwSignal::new(PlayForm::default()),
            confirm: RwSignal::new(false),
            errors: RwSignal::new(Vec::new()),
            busy: RwSignal::new(false),
            companions,
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
            winner: Some(self.winner.get_untracked()).filter(|w| !w.is_empty()),
            coop: self.coop.get_untracked(),
            coop_won: self.coop_won.get_untracked(),
            duration: self.duration.get_untracked(),
            location: self.location.get_untracked(),
            notes: self.notes.get_untracked(),
        }
    }

    /// Fresh fields; the players start as the last group that played this game, else you
    /// (design.md 5.4, D12).
    fn reset(&self, last_group: Vec<String>) {
        self.chip.set("today".into());
        self.picked.set(String::new());
        self.scores.set(BTreeMap::new());
        self.winner.set(String::new());
        self.coop.set(false);
        self.coop_won.set(true);
        self.duration.set(String::new());
        self.location.set(String::new());
        self.notes.set(String::new());
        let me = self
            .companions
            .with_untracked(|cs| cs.iter().find(|c| c.is_self).map(|c| c.id.clone()));
        let players = if last_group.is_empty() {
            me.into_iter().collect()
        } else {
            last_group
        };
        self.players.set(players);
        self.initial.set(self.form());
        self.confirm.set(false);
        self.errors.set(Vec::new());
    }

    fn dirty(&self) -> bool {
        self.form() != self.initial.get_untracked()
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

/// The "Log a play" sheet of board and video games (design.md 5.4).
#[component]
pub fn PlaySheet(state: PlayState, info: ItemInfo, #[prop(into)] title: String) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    let info = StoredValue::new(info);
    Effect::new(move |_| {
        let open = s.open.get();
        if open && !s.opened.get_untracked() {
            s.opened.set(true);
            s.busy.set(true);
            let item_id = info.with_value(|i| i.item_id.clone());
            leptos::task::spawn_local(async move {
                if s.companions.with_untracked(Vec::is_empty)
                    && let Ok(cs) = api::companions().await
                {
                    s.companions.set(cs);
                }
                s.reset(last_group(&item_id).await);
                s.busy.set(false);
            });
        } else if !open {
            s.opened.set(false);
        }
    });
    let ctx = StepCtx::current();
    let i2 = i18n.clone();
    let save = Callback::new(move |()| save_play(&s, info.get_value(), ctx.clone(), i2.clone()));
    let can_close = close_guard(move || s.dirty(), s.confirm);
    let discard = Callback::new(move |()| {
        s.initial.set(s.form());
        s.open.set(false);
    });
    view! {
        <Sheet open=s.open title=title close_label=t("dialog-close")
            description=info.with_value(|i| i.title.clone()) side=SheetSide::Adaptive can_close=can_close>
            <PlayFields state=s save=save discard=discard/>
        </Sheet>
    }
}

#[component]
fn PlayFields(state: PlayState, save: Callback<()>, discard: Callback<()>) -> impl IntoView {
    let s = state;
    let no_error = Signal::derive(|| None::<String>);
    view! {
        <form class="grid gap-4" novalidate
            on:submit=move |ev| { ev.prevent_default(); save.run(()); }
            on:keydown=move |ev: leptos::ev::KeyboardEvent| if ev.key() == "Enter" && (ev.ctrl_key() || ev.meta_key()) {
                ev.prevent_default();
                save.run(());
            }>
            <DateChips id="play-when" chip=s.chip picked=s.picked error=no_error/>
            <CompanionPicker id="play-people" legend=t("log-players") companions=s.companions selected=s.players include_self=true/>
            <Scores state=s/>
            <Disclosure id="play-more" title=t("log-more") summary=t("log-more-play")
                mode=DisclosureMode::Button level=3 storage_key=disclosure_storage_key("item", "play-more")>
                <PlayMore state=s/>
            </Disclosure>
            <DiscardPrompt confirm=s.confirm discard=discard/>
            {move || {
                let errors = s.errors.get();
                (!errors.is_empty()).then(|| view! {
                    <ul role="alert" class="grid gap-1 text-sm text-destructive-text">
                        {errors.into_iter().map(|e| view! { <li>{e}</li> }).collect_view()}
                    </ul>
                })
            }}
            <div class="flex justify-end gap-2">
                <Button r#type="submit" loading=s.busy>{t("log-save")}</Button>
            </div>
        </form>
    }
}

/// One score field per player and the winner (derived from the highest score unless chosen).
#[component]
fn Scores(state: PlayState) -> impl IntoView {
    let i18n = use_i18n();
    let s = state;
    let auto = i18n.t("log-winner-auto");
    let winners = Signal::derive(move || {
        let mut o = vec![SelectOption {
            value: String::new(),
            label: auto.clone(),
        }];
        o.extend(s.players.get().into_iter().map(|id| SelectOption {
            label: s.name(&id),
            value: id,
        }));
        o
    });
    let rows = move || s.players.get().into_iter().enumerate().collect::<Vec<_>>();
    view! {
        <div class="grid gap-3">
            <For each=rows key=|(_, id)| id.clone() let:row>
                {
                    let (n, id) = row;
                    let field = format!("play-score-{n}");
                    let value = RwSignal::new(s.scores.with_untracked(|m| m.get(&id).cloned().unwrap_or_default()));
                    let key = id.clone();
                    Effect::new(move |_| {
                        let v = value.get();
                        s.scores.update(|m| { m.insert(key.clone(), v); });
                    });
                    let label = use_i18n().t_args("log-score", &[("name", s.name(&id).into())]);
                    view! {
                        <FormField id=field.clone() label=label>
                            <Input id=field.clone() value=value inputmode="decimal" autocomplete="off" class="max-w-32"/>
                        </FormField>
                    }
                }
            </For>
            <Show when=move || !s.coop.get()>
                <div class="grid gap-1">
                    <Label r#for="play-winner">{use_i18n().t("log-winner")}</Label>
                    <Select id="play-winner" value=s.winner options=winners class="max-w-xs"/>
                </div>
            </Show>
        </div>
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
                <div class="flex items-center gap-3">
                    <Switch id="play-coop-won" checked=s.coop_won/>
                    <Label r#for="play-coop-won">{use_i18n().t("log-coop-won")}</Label>
                </div>
            </Show>
            <FormField id="play-duration" label=t("log-duration")>
                <Input id="play-duration" value=s.duration inputmode="numeric" autocomplete="off" class="max-w-32"/>
            </FormField>
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
            s.errors
                .set(errors.iter().map(|e| error_message(&i18n, e)).collect());
            return;
        }
    };
    s.errors.set(Vec::new());
    s.busy.set(true);
    leptos::task::spawn_local(async move {
        let ItemInfo {
            item_id,
            title,
            refresh,
            ..
        } = info;
        match api::log_play(item_id, payload).await {
            Ok(event) => {
                let label = i18n.t("log-play-step");
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

/// Mark the entry completed (the "Finished" of pages and positions); a page or position at
/// the end does not complete an entry on its own.
fn complete(item_id: String, ctx: &StepCtx, refresh: Callback<()>) {
    let word = ctx.i18n.t("status-completed");
    let done = ctx
        .i18n
        .t_args("library-status-changed", &[("status", word.into())]);
    let toasts = ctx.toasts;
    leptos::task::spawn_local(async move {
        if api::set_status(item_id, "completed".into()).await.is_ok() {
            toasts.push(done, None, ui::ToastVariant::Default);
        }
        refresh.run(());
    });
}

/// Page field with Update, +10, +25 and Finished (design.md 5.4 Pages); Enter saves.
#[component]
pub fn PagesInline(info: ItemInfo, step: NextStepDto) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let value = RwSignal::new(step.page.map(|p| p.to_string()).unwrap_or_default());
    let (busy, error) = (RwSignal::new(false), RwSignal::new(None::<String>));
    let pages = step.pages;
    let info = StoredValue::new(info);
    let i2 = i18n.clone();
    let log = move |text: String| match page_progress(&text, pages) {
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
            };
            run_step(ctx.clone(), step, busy, error, Some(refresh));
        }
    };
    let log = StoredValue::new(log);
    let plus = move |n: u32| {
        let now = value.get_untracked().trim().parse::<u32>().unwrap_or(0);
        let next = pages.map_or(now + n, |p| (now + n).min(p));
        value.set(next.to_string());
        log.with_value(|l| l(next.to_string()));
    };
    let ctx2 = StepCtx::current();
    let finish = move |_| {
        if let Some(p) = pages {
            log.with_value(|l| l(p.to_string()));
        }
        let ItemInfo {
            item_id, refresh, ..
        } = info.get_value();
        complete(item_id, &ctx2, refresh);
    };
    let of = pages.map(|p| i18n.t_args("log-of-pages", &[("pages", p.into())]));
    let initial = value.get_untracked();
    view! {
        <form class="grid gap-2" novalidate on:submit=move |ev| { ev.prevent_default(); log.with_value(|l| l(value.get_untracked())); }>
            <div class="flex flex-wrap items-end gap-2">
                <FormField id="pages-at" label=t("log-page")>
                    <Input id="pages-at" value=value inputmode="numeric" autocomplete="off" class="w-24"
                        invalid=Signal::derive(move || error.with(Option::is_some)) described_by="pages-at-error" attr:value=initial/>
                </FormField>
                {of.map(|o| view! { <span class="pb-2 text-sm text-muted-foreground">{o}</span> })}
                <Button r#type="submit" size=ButtonSize::Lg loading=busy>{t("log-update")}</Button>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Lg on:click=move |_| plus(10)>{use_i18n().t_args("log-plus-pages", &[("count", 10.into())])}</Button>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Lg on:click=move |_| plus(25)>{use_i18n().t_args("log-plus-pages", &[("count", 25.into())])}</Button>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Lg on:click=finish>{t("log-finished")}</Button>
            </div>
            <p id="pages-at-error" role="alert" class="text-sm text-destructive-text">{move || error.get()}</p>
        </form>
    }
}

/// Position field (`h:mm:ss`) with Update, +15 min and Finished (design.md 5.4 Position).
#[component]
pub fn PositionInline(info: ItemInfo, step: NextStepDto) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let start = step.seconds.map(format_position).unwrap_or_default();
    let value = RwSignal::new(start.clone());
    let (busy, error) = (RwSignal::new(false), RwSignal::new(None::<String>));
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
            };
            run_step(ctx.clone(), step, busy, error, Some(refresh));
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
            item_id, refresh, ..
        } = info.get_value();
        complete(item_id, &ctx2, refresh);
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
