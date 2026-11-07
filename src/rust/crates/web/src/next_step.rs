//! The next step of an entry (design.md 3.4) as a button: label and action come from the
//! server's `NextStepDto`; one-click steps log progress and offer Undo in a toast.

use contracts::{NewProgressDto, NextStepDto, ProgressSummaryDto};
use fluent_bundle::FluentValue;
use leptos::prelude::*;
use ui::{Icon, IconName, ToastAction, ToastVariant, Toasts, cn, use_toasts};

use crate::api;
use crate::i18n::{I18n, use_i18n};

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
        return Some(i.t_args(
            "library-progress-episodes",
            &[
                ("done", summary.episodes_watched.into()),
                ("total", total.into()),
            ],
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

/// Log `progress` for `item_id` and show "Logged: <label>" with Undo; `on_done` runs after
/// the log and after an undo (to refetch). Errors land in `error` (shown inline next to
/// the control: errors never go through toasts, design.md 7.1).
/// Contexts a step needs, taken while the component is built (event handlers run outside
/// the reactive owner).
#[derive(Clone)]
pub struct StepCtx {
    pub i18n: std::sync::Arc<I18n>,
    pub toasts: Toasts,
}

impl StepCtx {
    #[must_use]
    pub fn current() -> Self {
        Self {
            i18n: use_i18n(),
            toasts: use_toasts(),
        }
    }
}

/// One logged step: what to log for which item, and the words for the toast.
#[derive(Clone)]
pub struct LoggedStep {
    pub item_id: String,
    pub title: String,
    pub label: String,
    pub progress: serde_json::Value,
}

pub fn run_step(
    ctx: StepCtx,
    step: LoggedStep,
    busy: RwSignal<bool>,
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
) {
    let StepCtx { i18n, toasts } = ctx;
    let LoggedStep {
        item_id,
        title,
        label,
        progress,
    } = step;
    busy.set(true);
    error.set(None);
    leptos::task::spawn_local(async move {
        let p = NewProgressDto {
            progress,
            occurred_at: None,
            client_op_id: Some(uuid::Uuid::now_v7().to_string()),
            with: Vec::new(),
        };
        match api::log_progress(item_id, p).await {
            Ok(event) => {
                let ctx = StepCtx { i18n, toasts };
                toast_logged(&ctx, &label, title, vec![event.id], on_done);
                if let Some(cb) = on_done {
                    cb.run(());
                }
            }
            Err(e) => {
                let text = crate::pages::error_text(&e);
                error.set(Some(
                    i18n.t_args("library-step-failed", &[("error", text.into())]),
                ));
            }
        }
        busy.set(false);
    });
}

/// Take a logged event back (`undo_progress`) and say so in a toast ("Undone", or that it
/// did not work); `on_done` runs either way (refetch). The toast's Undo and the History
/// page's Undo buttons both end here.
pub fn undo_event(ctx: &StepCtx, event_id: String, on_done: Option<Callback<()>>) {
    let (undone, failed) = (ctx.i18n.t("library-undone"), ctx.i18n.t("error-generic"));
    let toasts = ctx.toasts;
    leptos::task::spawn_local(async move {
        match api::undo_progress(event_id).await {
            Ok(_) => toasts.push(undone, None, ToastVariant::Default),
            Err(_) => toasts.push(failed, None, ToastVariant::Destructive),
        }
        if let Some(cb) = on_done {
            cb.run(());
        }
    });
}

/// Take several logged events back (the Undo of a bulk mark), newest first, then say so
/// once; `on_done` runs after the last.
pub fn undo_events(ctx: &StepCtx, mut event_ids: Vec<String>, on_done: Option<Callback<()>>) {
    let (undone, failed) = (ctx.i18n.t("library-undone"), ctx.i18n.t("error-generic"));
    let toasts = ctx.toasts;
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
        if let Some(cb) = on_done {
            cb.run(());
        }
    });
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

/// Shared look of the compact (pill on a poster) variant.
const PILL: &str = "inline-flex min-h-7 max-w-full items-center gap-1 rounded-full border border-border \
    bg-background/95 px-2.5 text-xs font-semibold text-foreground shadow-sm";
const PILL_ACTION: &str = "cursor-pointer hover:bg-accent focus-visible:outline-none \
    focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-60";

/// The primary action of an entry (design.md 7.3): label and action from its next step.
/// Finish and episode steps log in one click (toast with Undo); page, position and session
/// steps need input and open the item page, where the log sheets live; "Up to date" is a
/// label only. `compact` is the pill on a poster card.
#[component]
pub fn NextStepButton(
    #[prop(into)] item_id: String,
    /// The item's title, for the accessible name and the toast.
    #[prop(into)]
    title: String,
    step: NextStepDto,
    #[prop(optional)] compact: bool,
    #[prop(optional_no_strip, into)] on_done: Option<Callback<()>>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let label = next_step_label(&i18n, &step);
    let named = i18n.t_args(
        "library-next-step-named",
        &[
            ("step", label.clone().into()),
            ("title", title.clone().into()),
        ],
    );
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let look = if compact {
        PILL
    } else {
        "inline-flex h-8 items-center gap-1.5 rounded-md bg-primary px-3 text-xs font-medium text-primary-foreground"
    };
    let extra = move || class.get().unwrap_or_default();
    let control = match step_progress(&step) {
        Some(progress) => {
            let logged = LoggedStep {
                item_id: item_id.clone(),
                title: title.clone(),
                label: label.clone(),
                progress,
            };
            let ctx = StepCtx::current();
            view! {
                <button
                    type="button"
                    aria-label=named
                    aria-busy=move || busy.get().then_some("true")
                    disabled=move || busy.get()
                    class=move || cn(&[look, PILL_ACTION, &extra()])
                    on:click=move |_| run_step(ctx.clone(), logged.clone(), busy, error, on_done)
                >
                    <Icon name=IconName::Check/>
                    <span class="truncate">{label.clone()}</span>
                </button>
            }
            .into_any()
        }
        None if step.action == "caught_up" => view! {
            <span class=move || cn(&[look, &extra()])>
                <Icon name=IconName::CheckCheck/>
                <span class="truncate">{label.clone()}</span>
            </span>
        }
        .into_any(),
        None => view! {
            <a href=format!("/items/{item_id}") aria-label=named class=move || cn(&[look, PILL_ACTION, &extra()])>
                <Icon name=IconName::Pencil/>
                <span class="truncate">{label.clone()}</span>
            </a>
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
        assert_eq!(next_step_label(&en, &ep), "Watched S2 E5");
        assert_eq!(next_step_label(&de, &ep), "S2 E5 gesehen");
        assert_eq!(
            next_step_label(&en, &step("finish", "next-step-watched-again")),
            "Watched again"
        );
        assert_eq!(
            next_step_label(&de, &step("session", "next-step-log-play")),
            "Partie eintragen"
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
