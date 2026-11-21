//! The next step of an entry (design.md 3.4) as a button: label and action come from the
//! server's `NextStepDto`; one-click steps log progress and offer Undo in a toast.

use contracts::{NewProgressDto, NextStepDto, ProgressSummaryDto};
use fluent_bundle::FluentValue;
use leptos::prelude::*;
use ui::{
    Button, ButtonLink, ButtonSize, Icon, IconName, ToastAction, ToastVariant, Toasts, cn,
    use_toasts,
};

use crate::api;
use crate::i18n::{I18n, use_i18n};
use crate::item_words::{AddUndo, RewatchWords, Verb, add_undo, entry_untouched};

/// The button label: the server names the Fluent key (`next-step-*`), the episode steps take
/// `$season` and `$episode`.
#[must_use]
pub fn next_step_label(i: &I18n, step: &NextStepDto) -> String {
    let num = |v: Option<u32>| FluentValue::from(v.unwrap_or(0));
    i.t_args(
        &step.label_key,
        &[("season", num(step.season)), ("episode", num(step.episode))],
    )
}

/// The step in the past tense, for the toast ("Logged: Watched S2 E5") and the title of the
/// log dialog: the `logged-step-*` twin of the `next-step-*` key (OT-5: buttons are
/// imperatives, confirmations are not). Steps without a twin keep their label.
#[must_use]
pub fn logged_step_label(i: &I18n, step: &NextStepDto) -> String {
    let Some(rest) = step.label_key.strip_prefix("next-step-") else {
        return next_step_label(i, step);
    };
    let key = format!("logged-step-{rest}");
    let num = |v: Option<u32>| FluentValue::from(v.unwrap_or(0));
    let text = i.t_args(
        &key,
        &[("season", num(step.season)), ("episode", num(step.episode))],
    );
    // a missing message comes back as its key
    if text == key {
        next_step_label(i, step)
    } else {
        text
    }
}

/// The words on the pill of a poster card: an episode step is "S2 E5" there (the imperative
/// sentence does not fit a card at 360 px; it stays the accessible name), other steps their
/// label.
#[must_use]
pub fn card_step_label(i: &I18n, step: &NextStepDto) -> String {
    if step.action == "episode" {
        let num = |v: Option<u32>| FluentValue::from(v.unwrap_or(0));
        i.t_args(
            "next-step-episode-short",
            &[("season", num(step.season)), ("episode", num(step.episode))],
        )
    } else {
        next_step_label(i, step)
    }
}

/// The progress a one-click step logs; `None` when the step needs more input (page,
/// position, a session with players) or there is nothing to do (up to date).
#[must_use]
pub fn step_progress(step: &NextStepDto) -> Option<serde_json::Value> {
    match step.action.as_str() {
        "finish" => Some(serde_json::json!({"type": "finished"})),
        "episode" => Some(serde_json::json!({
            "type": "episode",
            "season": step.season?,
            "episode": step.episode?,
        })),
        _ => None,
    }
}

/// Progress in words for the edge bar's accessible text; `None` without a fraction.
#[must_use]
pub fn progress_text(i: &I18n, summary: &ProgressSummaryDto, step: &NextStepDto) -> Option<String> {
    let fraction = summary.fraction?;
    if let Some(total) = summary.episodes_total
        && matches!(step.action.as_str(), "episode" | "caught_up")
    {
        // a running rewatch counts within itself (DEC-587)
        let done = summary
            .rewatch
            .as_ref()
            .filter(|r| r.running)
            .map_or(summary.episodes_watched, |r| r.done);
        return Some(i.t_args(
            "library-progress-episodes",
            &[("done", done.into()), ("total", total.into())],
        ));
    }
    if let (Some(page), Some(pages)) = (step.page, step.pages) {
        return Some(i.t_args(
            "library-progress-pages",
            &[("page", page.into()), ("pages", pages.into())],
        ));
    }
    Some(i.t_args(
        "library-progress-percent",
        &[("percent", percent(fraction).into())],
    ))
}

/// 0..=1 as a whole percentage.
#[must_use]
pub fn percent(fraction: f32) -> u32 {
    // clamped to 0..=100 first, so the cast cannot truncate or wrap
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let p = (fraction.clamp(0.0, 1.0) * 100.0).round() as u32;
    p
}

/// Contexts a step needs, taken while the component is built (event handlers run outside
/// the reactive owner).
#[derive(Clone)]
pub struct StepCtx {
    pub i18n: std::sync::Arc<I18n>,
    pub toasts: Toasts,
    /// The offline queue (signed-in shell only, WP4-5).
    pub offline: Option<crate::offline::Offline>,
    pub funnel: crate::funnel::Funnel,
    /// Where an action made away from the page that shows its result is counted
    /// ([`StepCtx::acted`]).
    pub own: Option<crate::events::OwnAction>,
}

impl StepCtx {
    #[must_use]
    pub fn current() -> Self {
        Self {
            i18n: use_i18n(),
            toasts: use_toasts(),
            offline: use_context::<crate::offline::Offline>(),
            funnel: crate::funnel::Funnel::current(),
            own: use_context::<crate::events::OwnAction>(),
        }
    }

    /// The person's action went through and no page asked to be refreshed for it (the
    /// palette's quick path, the Undo of its toast): the open page shows it at once instead
    /// of waiting for the paced event (OT1B-9).
    pub fn acted(&self) {
        if let Some(own) = self.own {
            own.bump();
        }
    }

    /// After an action: the caller's refresh, else [`StepCtx::acted`].
    fn done(&self, on_done: Option<Callback<()>>) {
        match on_done {
            Some(cb) => cb.run(()),
            None => self.acted(),
        }
    }

    /// Queue `commands` after a network failure ("Saved offline, will sync" with Undo);
    /// `false` without a queue (the caller shows its error) or when the queue is full.
    pub fn save_offline(
        &self,
        title: String,
        commands: Vec<crate::offline::Queued>,
        on_undo: Option<Callback<()>>,
    ) -> bool {
        let Some(offline) = self.offline else {
            return false;
        };
        let ctx = crate::offline::OfflineCtx {
            offline,
            i18n: self.i18n.clone(),
            toasts: self.toasts,
        };
        crate::offline::save_offline(&ctx, title, commands, on_undo)
    }

    /// Queue a status or rating change, stamped now by the client clock (`make` gets the
    /// stamp); `false` without a queue.
    pub fn queue_field(
        &self,
        title: String,
        make: impl FnOnce(&str) -> contracts::SyncCommandDto,
    ) -> bool {
        let Some(offline) = self.offline else {
            return false;
        };
        let cmd = make(&offline.stamp());
        let q = crate::offline::Queued::new(cmd, &title, crate::offline::now_ms());
        self.save_offline(title, vec![q], None)
    }
}

/// One logged step: what to log for which item, and the words for the toast.
#[derive(Clone)]
pub struct LoggedStep {
    pub item_id: String,
    pub title: String,
    /// The step in the past tense ([`logged_step_label`]).
    pub label: String,
    pub progress: serde_json::Value,
    /// The verb of the item's kind where the caller knows it (the toast of the episode that
    /// finishes a rewatch run is worded per verb); `None` reads like a show.
    pub verb: Option<Verb>,
}

/// Log `progress` for `item_id` and show "Logged: <label>" with Undo; `on_done` runs after
/// the log and after an undo (to refetch). Errors land in `error` (shown inline next to
/// the control: errors never go through toasts, design.md 7.1). Offline (a network failure)
/// the log goes into the queue with the same op id (WP4-5). One funnel flow `next_step`.
pub fn run_step(
    ctx: StepCtx,
    step: LoggedStep,
    busy: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
) {
    log_step(ctx, step, None, busy, error, on_done, None);
}

/// What follows a logged step: "Finished" on a book completes the entry after its last page
/// is logged (v1-books).
///
/// The follow-up is a plain closure, not a reactive `Callback`: the bar that makes it is
/// rendered again by the refetch the log itself starts, often before the log's answer is
/// back, and a `Callback` dies with the bar's owner (DEC-870).
#[derive(Clone)]
pub struct Then {
    /// Runs once when the log went through (not after an undo of it).
    online: std::sync::Arc<dyn Fn() + Send + Sync>,
    /// Offline nothing can run after the log: this status is queued behind it instead, so
    /// the sync logs the page and then sets the status, in that order (review BOOK-6).
    pub status: &'static str,
}

impl Then {
    pub fn new(online: impl Fn() + Send + Sync + 'static, status: &'static str) -> Self {
        Self {
            online: std::sync::Arc::new(online),
            status,
        }
    }

    /// The follow-up, also when the bar that made it is gone.
    pub fn run(&self) {
        (self.online)();
    }
}

/// [`run_step`] with what follows it ([`Then`]): run when the log went through, or queued
/// behind the log when it could only be saved offline.
pub fn run_step_then(
    ctx: StepCtx,
    step: LoggedStep,
    busy: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
    then: Then,
) {
    log_step(ctx, step, None, busy, error, on_done, Some(then));
}

/// [`run_step`] for an item that is not in the library (OT-4): the first log adds it. The
/// toast says so, and its Undo takes the log and the add back ([`toast_logged_added`]).
/// `done` turns true once the log went through (not for one queued offline): the bar that
/// offered the first step waits for the refetched page from then on (review OT2B-4).
pub fn run_first_step(
    ctx: StepCtx,
    step: LoggedStep,
    busy: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    done: RwSignal<bool>,
    on_done: Option<Callback<()>>,
) {
    log_step(ctx, step, Some(done), busy, error, on_done, None);
}

/// `added`: the log is the first of an item outside the library; see [`run_first_step`].
fn log_step(
    ctx: StepCtx,
    step: LoggedStep,
    added: Option<RwSignal<bool>>,
    busy: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
    then: Option<Then>,
) {
    let LoggedStep {
        item_id,
        title,
        label,
        progress,
        verb,
    } = step;
    ctx.funnel.intent(crate::funnel::Flow::NextStep);
    busy.set(true);
    error.set(None);
    leptos::task::spawn_local(async move {
        let op = uuid::Uuid::now_v7().to_string();
        let p = NewProgressDto {
            progress: progress.clone(),
            occurred_at: None,
            client_op_id: Some(op.clone()),
            with: Vec::new(),
        };
        match api::log_progress(item_id.clone(), p).await {
            Ok(event) => {
                ctx.funnel.logged();
                if let Some(done) = added {
                    // the bar may be gone already (another page): nothing to tell then
                    let _ = done.try_set(true);
                    toast_logged_added(&ctx, &label, (item_id, title), event.id, on_done);
                } else {
                    let said = said_title(&ctx.i18n, verb, title, &[&event]);
                    toast_logged(&ctx, &label, said, vec![event.id], on_done);
                }
                ctx.done(on_done);
                if let Some(then) = then {
                    then.run();
                }
            }
            Err(e) if crate::offline::network_error(&e) && ctx.offline.is_some() => {
                let hlc = ctx.offline.map(crate::offline::Offline::stamp);
                let cmd = crate::offline::log_command(
                    &item_id,
                    progress,
                    crate::offline::now_rfc3339(),
                    hlc.as_deref(),
                );
                let q =
                    crate::offline::Queued::with_op_id(op, cmd, &title, crate::offline::now_ms());
                // what follows the log is stamped after it and waits behind it in the queue
                let status = then.zip(ctx.offline).map(|(then, offline)| {
                    crate::offline::status_command(&item_id, then.status, &offline.stamp())
                });
                let queued = crate::offline::log_then_status(q, status, crate::offline::now_ms());
                ctx.save_offline(title, queued, None);
                ctx.funnel.logged();
            }
            Err(e) => {
                ctx.funnel.abandoned_if(crate::funnel::Flow::NextStep);
                let text = crate::pages::error_text(&e);
                error.set(Some(
                    ctx.i18n
                        .t_args("library-step-failed", &[("error", text.into())]),
                ));
            }
        }
        busy.set(false);
    });
}

/// Log a marker of a rewatch run: its start on a Completed show ([`rewatch_start`], sets In
/// progress; ot2e, DEC-587) or its stop while it runs ([`rewatch_stop`], gives Completed
/// back and ends the run for good; DEC-590). Says so (`said`, "Rewatch started") with Undo,
/// which takes the marker and its status move back. Offline the marker goes into the queue
/// with its stamp, like every log (DEC-455). `on_done` runs after the log and after an undo
/// (to refetch); an error lands in `error`.
pub fn log_run_marker(
    ctx: StepCtx,
    (item_id, title): (String, String),
    (marker, said): (serde_json::Value, String),
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
) {
    error.set(None);
    leptos::task::spawn_local(async move {
        let op = uuid::Uuid::now_v7().to_string();
        let p = NewProgressDto {
            progress: marker.clone(),
            occurred_at: None,
            client_op_id: Some(op.clone()),
            with: Vec::new(),
        };
        match api::log_progress(item_id.clone(), p).await {
            Ok(event) => {
                let undo_ctx = ctx.clone();
                let undo = ToastAction::new(
                    ctx.i18n.t("library-undo"),
                    Callback::new(move |()| undo_event(&undo_ctx, event.id.clone(), on_done)),
                );
                ctx.toasts.push_action(said, Some(title), undo);
            }
            Err(e) if crate::offline::network_error(&e) && ctx.offline.is_some() => {
                let q = crate::offline::queued_log(&item_id, &title, op, marker, None, ctx.offline);
                ctx.save_offline(title, vec![q], None);
            }
            Err(e) => {
                let text = crate::pages::error_text(&e);
                error.set(Some(
                    ctx.i18n
                        .t_args("library-step-failed", &[("error", text.into())]),
                ));
            }
        }
        if let Some(cb) = on_done {
            cb.run(());
        }
    });
}

/// Take a logged event back (`undo_progress`) and say so in a toast ("Undone", or that it
/// did not work); `on_done` runs either way (refetch). The toast's Undo and the History
/// page's Undo buttons both end here.
pub fn undo_event(ctx: &StepCtx, event_id: String, on_done: Option<Callback<()>>) {
    let (undone, failed) = (ctx.i18n.t("library-undone"), ctx.i18n.t("error-generic"));
    let toasts = ctx.toasts;
    let ctx = ctx.clone();
    leptos::task::spawn_local(async move {
        match api::undo_progress(event_id).await {
            Ok(_) => toasts.push(undone, None, ToastVariant::Default),
            Err(_) => toasts.push(failed, None, ToastVariant::Destructive),
        }
        ctx.done(on_done);
    });
}

/// Take several logged events back (the Undo of a bulk mark), newest first, then say so
/// once; `on_done` runs after the last.
pub fn undo_events(ctx: &StepCtx, mut event_ids: Vec<String>, on_done: Option<Callback<()>>) {
    let (undone, failed) = (ctx.i18n.t("library-undone"), ctx.i18n.t("error-generic"));
    let toasts = ctx.toasts;
    let ctx = ctx.clone();
    leptos::task::spawn_local(async move {
        let mut ok = true;
        event_ids.reverse();
        for id in event_ids {
            ok &= api::undo_progress(id).await.is_ok();
        }
        if ok {
            toasts.push(undone, None, ToastVariant::Default);
        } else {
            toasts.push(failed, None, ToastVariant::Destructive);
        }
        ctx.done(on_done);
    });
}

/// The progress that starts a rewatch run (DEC-587).
#[must_use]
pub fn rewatch_start() -> serde_json::Value {
    serde_json::json!({"type": "rewatch"})
}

/// The progress that stops the rewatch run that is running (DEC-590).
#[must_use]
pub fn rewatch_stop() -> serde_json::Value {
    serde_json::json!({"type": "rewatch_stop"})
}

/// The second line of the toast of a log: the title, or "Rewatch finished: <title>" (per
/// `verb`) when one of the logged `events` was the episode that finished a rewatch run (its
/// answer names the status it moved the entry to; ot2e).
#[must_use]
pub fn said_title(
    i: &I18n,
    verb: Option<Verb>,
    title: String,
    events: &[&contracts::ProgressEventDto],
) -> String {
    let finished = events
        .iter()
        .any(|e| e.progress["type"] == "episode" && e.status.as_deref() == Some("completed"));
    if finished {
        i.t_args(RewatchWords::of(verb).finished, &[("title", title.into())])
    } else {
        title
    }
}

/// "Logged: <label>" with Undo for the events just logged (one step, a sheet or a bulk
/// mark); Undo takes them all back.
pub fn toast_logged(
    ctx: &StepCtx,
    label: &str,
    title: String,
    event_ids: Vec<String>,
    on_done: Option<Callback<()>>,
) {
    let logged = ctx.i18n.t_args("library-logged", &[("step", label.into())]);
    let undo_ctx = ctx.clone();
    let undo = ToastAction::new(
        ctx.i18n.t("library-undo"),
        Callback::new(move |()| match event_ids.as_slice() {
            [one] => undo_event(&undo_ctx, one.clone(), on_done),
            _ => undo_events(&undo_ctx, event_ids.clone(), on_done),
        }),
    );
    ctx.toasts.push_action(logged, Some(title), undo);
}

/// "Logged: <label>" for the first log of an item that was not in the library: the second
/// line says that it was added ("<title>, added to your library"), and Undo takes both back,
/// the event and then the entry. The entry goes only while it is as the add left it
/// ([`entry_untouched`]); a rating or a tag given meanwhile keeps it (review OT2A-3: Undo
/// left the item in the library as Planned, and nothing had said that it was added).
pub fn toast_logged_added(
    ctx: &StepCtx,
    label: &str,
    (item_id, title): (String, String),
    event_id: String,
    on_done: Option<Callback<()>>,
) {
    let logged = ctx.i18n.t_args("library-logged", &[("step", label.into())]);
    let said = ctx
        .i18n
        .t_args("item-logged-added", &[("title", title.into())]);
    let undo_ctx = ctx.clone();
    let undo = ToastAction::new(
        ctx.i18n.t("library-undo"),
        Callback::new(move |()| {
            let (ctx, id, event) = (undo_ctx.clone(), item_id.clone(), event_id.clone());
            leptos::task::spawn_local(async move {
                let mut ok = api::undo_progress(event).await.is_ok();
                let untouched = ok
                    && api::item_page(id.clone()).await.is_ok_and(|p| {
                        p.entry
                            .as_ref()
                            .is_some_and(|e| entry_untouched(e, p.progress.len(), p.tags.len()))
                    });
                if untouched {
                    let req = crate::bulk::request(vec![id], "remove");
                    ok = api::library_bulk(req).await.is_ok();
                }
                if ok {
                    ctx.toasts
                        .push(ctx.i18n.t("library-undone"), None, ToastVariant::Default);
                } else {
                    ctx.toasts
                        .push(ctx.i18n.t("error-generic"), None, ToastVariant::Destructive);
                }
                ctx.done(on_done);
            });
        }),
    );
    ctx.toasts.push_action(logged, Some(said), undo);
}

/// Take an add back: remove the entry of `item_id` while it is as the add left it
/// ([`add_undo`]). `Ok(false)`: it was changed meanwhile and stays (review OT2A-8: the Undo
/// removed an entry with the watch logged and the rating given since).
async fn undo_add(item_id: String) -> Result<bool, ServerFnError> {
    let page = api::item_page(item_id.clone()).await?;
    match add_undo(page.entry.as_ref(), page.progress.len(), page.tags.len()) {
        AddUndo::Gone => Ok(true),
        AddUndo::Keep => Ok(false),
        AddUndo::Remove => {
            let req = crate::bulk::request(vec![item_id], "remove");
            api::library_bulk(req).await.map(|_| true)
        }
    }
}

/// "Added to your library as Planned" with the title and Undo, which removes the entry again
/// (WP4-3) unless it was changed since; then the answer is "Kept in your library: ..." (review
/// OT2A-8). One toast for every add: the item page's "Want to watch" and the palette's quick
/// path (OT-4). `on_done` runs after an undo (to refetch).
pub fn toast_added(ctx: &StepCtx, item_id: String, title: String, on_done: Option<Callback<()>>) {
    let undo_ctx = ctx.clone();
    let undo = ToastAction::new(
        ctx.i18n.t("library-undo"),
        Callback::new(move |()| {
            let (ctx, id) = (undo_ctx.clone(), item_id.clone());
            leptos::task::spawn_local(async move {
                let (key, variant) = match undo_add(id).await {
                    Ok(true) => ("library-undone", ToastVariant::Default),
                    Ok(false) => ("item-added-kept", ToastVariant::Default),
                    Err(_) => ("error-generic", ToastVariant::Destructive),
                };
                ctx.toasts.push(ctx.i18n.t(key), None, variant);
                ctx.done(on_done);
            });
        }),
    );
    ctx.toasts
        .push_action(ctx.i18n.t("item-added-planned"), Some(title), undo);
}

/// Shared look of the compact (pill on a poster) variant.
const PILL: &str = "inline-flex min-h-7 max-w-full items-center gap-1 rounded-full border border-border \
    bg-background/95 px-2.5 text-xs font-semibold text-foreground shadow-sm";
const PILL_ACTION: &str = "cursor-pointer focus-visible:outline-none focus-visible:ring-2 \
    focus-visible:ring-ring disabled:opacity-60";
/// The pill's hover; its own text stays on `accent`.
const PILL_HOVER: &str = "hover:bg-accent";
/// "Up to date" outside a poster: a label, not a button (a state does not look like an action,
/// OT-5).
const LABEL: &str =
    "inline-flex h-8 items-center gap-1.5 text-xs font-medium text-muted-foreground";

/// The primary action of an entry (design.md 7.3): label and action from its next step.
/// Finish and episode steps log in one click (toast with Undo); page, position and session
/// steps need input and open the item page, where the log sheets live; "Up to date" is a
/// label only. `compact` is the pill on a poster card (an episode step reads "S2 E5" there);
/// otherwise it is a small [`ui::Button`] (or [`ui::ButtonLink`]), so it shares its size with
/// the other actions of a card (OT-12).
#[component]
pub fn NextStepButton(
    #[prop(into)] item_id: String,
    /// The item's title, for the accessible name and the toast.
    #[prop(into)]
    title: String,
    step: NextStepDto,
    #[prop(optional)] compact: bool,
    #[prop(optional_no_strip, into)] on_done: Option<Callback<()>>,
    /// The tab stop of the control (a roving grid sets `-1` on all but one card).
    #[prop(into, optional)]
    tabindex: MaybeProp<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let label = if compact {
        card_step_label(&i18n, &step)
    } else {
        next_step_label(&i18n, &step)
    };
    let named = i18n.t_args(
        "library-next-step-named",
        &[
            ("step", next_step_label(&i18n, &step).into()),
            ("title", title.clone().into()),
        ],
    );
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let extra = Signal::derive(move || class.get().unwrap_or_default());
    let href = format!("/items/{item_id}");
    let logged = step_progress(&step).map(|progress| LoggedStep {
        item_id,
        title,
        label: logged_step_label(&i18n, &step),
        progress,
        verb: None,
    });
    let ctx = StepCtx::current();
    let control = match (logged, compact) {
        (Some(logged), true) => view! {
            <button
                type="button"
                aria-label=named
                data-next-step=""
                tabindex=move || tabindex.get()
                aria-busy=move || busy.get().then_some("true")
                disabled=move || busy.get()
                class=move || cn(&[PILL, PILL_ACTION, PILL_HOVER, &extra.get()])
                on:click=move |_| run_step(ctx.clone(), logged.clone(), busy, error, on_done)
            >
                <Icon name=IconName::Check/>
                <span class="truncate">{label.clone()}</span>
            </button>
        }
        .into_any(),
        (Some(logged), false) => view! {
            <Button size=ButtonSize::Sm loading=busy class=extra attr:aria-label=named attr:data-next-step=""
                attr:tabindex=move || tabindex.get()
                on:click=move |_| run_step(ctx.clone(), logged.clone(), busy, error, on_done)>
                <Icon name=IconName::Check/>
                <span class="truncate">{label.clone()}</span>
            </Button>
        }
        .into_any(),
        (None, _) if step.action == "caught_up" => view! {
            <span class=move || cn(&[if compact { PILL } else { LABEL }, &extra.get()])>
                <Icon name=IconName::CheckCheck/>
                <span class="truncate">{label.clone()}</span>
            </span>
        }
        .into_any(),
        (None, true) => view! {
            <a href=href aria-label=named data-next-step="" tabindex=move || tabindex.get() class=move || cn(&[PILL, PILL_ACTION, PILL_HOVER, &extra.get()])>
                <Icon name=IconName::Pencil/>
                <span class="truncate">{label.clone()}</span>
            </a>
        }
        .into_any(),
        (None, false) => view! {
            <ButtonLink href=href size=ButtonSize::Sm class=extra attr:aria-label=named attr:data-next-step=""
                attr:tabindex=move || tabindex.get()>
                <Icon name=IconName::Pencil/>
                <span class="truncate">{label.clone()}</span>
            </ButtonLink>
        }
        .into_any(),
    };
    view! {
        {control}
        {move || error.get().map(|e| view! { <p role="alert" class="text-xs text-destructive-text">{e}</p> })}
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use contracts::{NextStepDto, ProgressSummaryDto};

    use super::*;

    /// "Finished" on a book (pipeline of #16, books.spec): the bar that made the follow-up is
    /// rendered again by the refetch its own page log starts, and on a slow host that happens
    /// before the log's answer. What follows the log must still run then: it completes the
    /// entry, and a disposed follow-up panicked the page instead.
    #[test]
    fn what_follows_a_log_runs_after_the_bar_that_made_it_is_gone() {
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let bar = Owner::new();
        let then = bar.with(|| {
            let ran = ran.clone();
            Then::new(
                move || ran.store(true, std::sync::atomic::Ordering::SeqCst),
                "completed",
            )
        });
        bar.cleanup();
        drop(bar);
        then.run();
        assert!(ran.load(std::sync::atomic::Ordering::SeqCst));
    }

    fn step(action: &str, key: &str) -> NextStepDto {
        NextStepDto {
            action: action.into(),
            label_key: key.into(),
            ..NextStepDto::default()
        }
    }

    #[test]
    fn labels_come_from_the_key_with_variables() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        let ep = NextStepDto {
            season: Some(2),
            episode: Some(5),
            ..step("episode", "next-step-episode")
        };
        // OT-5: a button says what it does (an imperative), not a state
        assert_eq!(next_step_label(&en, &ep), "Mark S2 E5 watched");
        assert_eq!(next_step_label(&de, &ep), "S2 E5 als gesehen markieren");
        let listened = NextStepDto {
            label_key: "next-step-episode-listened".into(),
            ..ep.clone()
        };
        assert_eq!(next_step_label(&en, &listened), "Mark S2 E5 listened");
        assert_eq!(
            next_step_label(&de, &listened),
            "S2 E5 als geh\u{f6}rt markieren"
        );
        for (key, en_text, de_text) in [
            (
                "next-step-watched",
                "Mark as watched",
                "Als gesehen markieren",
            ),
            (
                "next-step-watched-again",
                "Log a rewatch",
                "Nochmal gesehen eintragen",
            ),
            (
                "next-step-finished",
                "Mark as finished",
                "Als beendet markieren",
            ),
            (
                "next-step-finished-again",
                "Log it again",
                "Nochmal eintragen",
            ),
        ] {
            assert_eq!(next_step_label(&en, &step("finish", key)), en_text);
            assert_eq!(next_step_label(&de, &step("finish", key)), de_text);
        }
        assert_eq!(
            next_step_label(&de, &step("session", "next-step-log-play")),
            "Partie eintragen"
        );
    }

    /// OT-5: the toast and the dialog title confirm in the past tense ("Logged: Watched S2
    /// E5"); steps without a past form keep their label.
    #[test]
    fn confirmations_keep_the_past_tense() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        let ep = NextStepDto {
            season: Some(2),
            episode: Some(5),
            ..step("episode", "next-step-episode")
        };
        assert_eq!(logged_step_label(&en, &ep), "Watched S2 E5");
        assert_eq!(logged_step_label(&de, &ep), "S2 E5 gesehen");
        let listened = NextStepDto {
            label_key: "next-step-episode-listened".into(),
            ..ep.clone()
        };
        assert_eq!(logged_step_label(&en, &listened), "Listened to S2 E5");
        for (key, en_text, de_text) in [
            ("next-step-watched", "Watched", "Gesehen"),
            (
                "next-step-watched-again",
                "Watched again",
                "Nochmal gesehen",
            ),
            ("next-step-finished", "Finished", "Beendet"),
            (
                "next-step-finished-again",
                "Finished again",
                "Nochmal beendet",
            ),
        ] {
            assert_eq!(logged_step_label(&en, &step("finish", key)), en_text);
            assert_eq!(logged_step_label(&de, &step("finish", key)), de_text);
        }
        assert_eq!(
            logged_step_label(&en, &step("session", "next-step-log-play")),
            "Log a play"
        );
    }

    /// The pill on a poster is narrow: an episode step shows "S2 E5" there (the full
    /// sentence stays its accessible name), other steps their label.
    #[test]
    fn the_card_pill_shortens_episode_steps_only() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        let ep = NextStepDto {
            season: Some(2),
            episode: Some(5),
            ..step("episode", "next-step-episode")
        };
        assert_eq!(card_step_label(&en, &ep), "S2 E5");
        assert_eq!(card_step_label(&de, &ep), "S2 E5");
        assert_eq!(
            card_step_label(&en, &step("finish", "next-step-watched")),
            "Mark as watched"
        );
        assert_eq!(
            card_step_label(&en, &step("caught_up", "next-step-caught-up")),
            "Up to date"
        );
    }

    #[test]
    fn only_finish_and_episode_are_one_click() {
        assert_eq!(
            step_progress(&step("finish", "next-step-watched")),
            Some(serde_json::json!({"type": "finished"}))
        );
        let ep = NextStepDto {
            season: Some(1),
            episode: Some(3),
            ..step("episode", "next-step-episode")
        };
        assert_eq!(
            step_progress(&ep),
            Some(serde_json::json!({"type": "episode", "season": 1, "episode": 3}))
        );
        for a in ["page", "position", "session", "caught_up"] {
            assert_eq!(step_progress(&step(a, "x")), None, "{a}");
        }
        // an episode step without numbers cannot be logged blind
        assert_eq!(step_progress(&step("episode", "next-step-episode")), None);
    }

    /// ot2e: the toast of the episode that finished a rewatch run says so.
    #[test]
    fn the_toast_says_when_a_log_finished_a_rewatch() {
        let en = I18n::new("en", BTreeMap::new());
        let de = I18n::new("de", BTreeMap::new());
        let event =
            |progress: serde_json::Value, status: Option<&str>| contracts::ProgressEventDto {
                id: "e".into(),
                item_id: "i".into(),
                occurred_at: "2026-11-11T10:00:00Z".into(),
                progress,
                status: status.map(String::from),
            };
        let ep = serde_json::json!({"type": "episode", "season": 1, "episode": 3});
        let plain = event(ep.clone(), None);
        let last = event(ep.clone(), Some("completed"));
        let first = event(ep, Some("in_progress"));
        let movie = event(serde_json::json!({"type": "finished"}), Some("completed"));
        let said = |i: &I18n, events: &[&contracts::ProgressEventDto]| {
            said_title(i, None, "Dark".into(), events)
        };
        assert_eq!(said(&en, &[&plain]), "Dark");
        assert_eq!(said(&en, &[&first]), "Dark");
        assert_eq!(
            said(&en, &[&movie]),
            "Dark",
            "a first watch is not a rewatch"
        );
        // review OT2E-10: the approved wording, per verb
        assert_eq!(said(&en, &[&plain, &last]), "Rewatch finished: Dark");
        assert_eq!(said(&de, &[&last]), "Wiederholung beendet: Dark");
        assert_eq!(
            said_title(&en, Some(Verb::Listen), "Dark".into(), &[&last]),
            "Finished listening again: Dark"
        );
        // the stop marker answers Completed too and finished nothing
        let stop = event(rewatch_stop(), Some("completed"));
        assert_eq!(said(&en, &[&stop]), "Dark");
        assert_eq!(rewatch_start(), serde_json::json!({"type": "rewatch"}));
        assert_eq!(rewatch_stop(), serde_json::json!({"type": "rewatch_stop"}));
    }

    #[test]
    fn the_progress_bar_reads_as_words() {
        let en = I18n::new("en", BTreeMap::new());
        let none = ProgressSummaryDto::default();
        assert_eq!(progress_text(&en, &none, &step("finish", "x")), None);
        let eps = ProgressSummaryDto {
            episodes_watched: 4,
            episodes_total: Some(10),
            fraction: Some(0.4),
            ..ProgressSummaryDto::default()
        };
        assert_eq!(
            progress_text(&en, &eps, &step("episode", "x")).as_deref(),
            Some("4 of 10 episodes")
        );
        // a running rewatch counts within itself (ot2e); a stopped one does not
        let again = ProgressSummaryDto {
            episodes_watched: 10,
            fraction: Some(0.3),
            rewatch: Some(contracts::RewatchDto {
                start: "e1".into(),
                done: 3,
                running: true,
            }),
            ..eps.clone()
        };
        assert_eq!(
            progress_text(&en, &again, &step("episode", "x")).as_deref(),
            Some("3 of 10 episodes")
        );
        let stopped = ProgressSummaryDto {
            rewatch: again.rewatch.clone().map(|r| contracts::RewatchDto {
                running: false,
                ..r
            }),
            ..again
        };
        assert_eq!(
            progress_text(&en, &stopped, &step("caught_up", "x")).as_deref(),
            Some("10 of 10 episodes")
        );
        let pages = ProgressSummaryDto {
            fraction: Some(0.5),
            ..ProgressSummaryDto::default()
        };
        let p = NextStepDto {
            page: Some(120),
            pages: Some(240),
            ..step("page", "x")
        };
        assert_eq!(
            progress_text(&en, &pages, &p).as_deref(),
            Some("Page 120 of 240")
        );
        let other = ProgressSummaryDto {
            fraction: Some(0.25),
            ..ProgressSummaryDto::default()
        };
        assert_eq!(
            progress_text(&en, &other, &step("position", "x")).as_deref(),
            Some("25% done")
        );
    }
}
