//! Bulk edits and removing from the library (design.md 3.5, 6.2, 6.3; WP4-3): running a bulk
//! request with a toast that offers Undo, the list picker, and the Library list view's bulk
//! bar (count, Set status, Add tag, Add to list, Remove with a confirm).

use std::collections::BTreeSet;

use contracts::{BulkRequestDto, ListDto};
use leptos::prelude::*;
use ui::{
    Button, ButtonSize, ButtonVariant, Dialog, DropdownMenu, FormField, Input, MenuItem,
    ToastAction, ToastVariant,
};

use crate::api;
use crate::i18n::{t, use_i18n};
use crate::next_step::StepCtx;

/// A bulk request for `ids` doing `op`.
#[must_use]
pub fn request(ids: Vec<String>, op: &str) -> BulkRequestDto {
    BulkRequestDto {
        item_ids: ids,
        op: op.to_string(),
        ..BulkRequestDto::default()
    }
}

/// Run a bulk request. Done: the funnel's bulk flow ends as logged, then a toast titled
/// `done(n)` for the `n` entries changed (with how many could not be) and Undo, which puts
/// everything back; `done` runs only then, after the flow ended (a remove clears the selection
/// there, UX4-7). Failed: the error in `error`, also when the undo fails (errors never go
/// through toasts, DEC-418). `on_done` runs after the change and after an undo.
pub fn run_bulk(
    ctx: &StepCtx,
    req: BulkRequestDto,
    done: impl FnOnce(u32) -> String + 'static,
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
) {
    let ctx = ctx.clone();
    error.set(None);
    leptos::task::spawn_local(async move {
        match api::library_bulk(req).await {
            Ok(r) => {
                ctx.funnel.logged_if(crate::funnel::Flow::Bulk);
                let failed = r.results.iter().filter(|x| !x.ok).count();
                let note = (failed > 0).then(|| {
                    ctx.i18n
                        .t_args("library-bulk-failed", &[("count", failed.into())])
                });
                let (undo_ctx, token) = (ctx.clone(), r.undo);
                let undo = ToastAction::new(
                    ctx.i18n.t("library-undo"),
                    Callback::new(move |()| undo_bulk(&undo_ctx, token.clone(), error, on_done)),
                );
                ctx.toasts.push_action(done(r.done), note, undo);
            }
            Err(e) => error.set(Some(failure_text(&ctx.i18n, &e, false))),
        }
        if let Some(cb) = on_done {
            cb.run(());
        }
    });
}

/// The message key of a failed bulk change (`undo: false`) or undo: a change too large to
/// undo (422 `invalid`) and an undo that no longer applies (400 `bad_request`: expired, edited
/// or someone else's token) have their own words; anything else names the error.
#[must_use]
pub fn failure_key(code: &str, undo: bool) -> &'static str {
    match (code, undo) {
        ("invalid", false) => "library-bulk-too-large",
        ("bad_request", true) => "library-undo-expired",
        (_, false) => "library-step-failed",
        (_, true) => "library-undo-failed",
    }
}

fn failure_text(i18n: &crate::i18n::I18n, e: &ServerFnError, undo: bool) -> String {
    let error = crate::pages::error_text(e);
    i18n.t_args(
        failure_key(&crate::api::error_code(e), undo),
        &[("error", error.into())],
    )
}

fn undo_bulk(
    ctx: &StepCtx,
    token: String,
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
) {
    let ctx = ctx.clone();
    error.set(None);
    leptos::task::spawn_local(async move {
        match api::library_bulk_undo(token).await {
            Ok(()) => ctx
                .toasts
                .push(ctx.i18n.t("library-undone"), None, ToastVariant::Default),
            Err(e) => error.set(Some(failure_text(&ctx.i18n, &e, true))),
        }
        if let Some(cb) = on_done {
            cb.run(());
        }
    });
}

/// "Removed <title> from your library" with Undo (quick menu, item page).
pub fn remove_one(
    ctx: &StepCtx,
    item_id: String,
    title: &str,
    error: RwSignal<Option<String>>,
    on_done: Option<Callback<()>>,
) {
    let done = ctx
        .i18n
        .t_args("library-removed", &[("title", title.to_string().into())]);
    run_bulk(
        ctx,
        request(vec![item_id], "remove"),
        move |_| done,
        error,
        on_done,
    );
}

/// Which items the list picker adds (`None`: closed).
#[derive(Clone, Copy)]
pub struct ListPick(pub RwSignal<Option<Vec<String>>>);

/// "Add to list": the caller's lists as buttons; one click adds the items (toast with Undo).
#[component]
pub fn ListPicker(target: RwSignal<Option<Vec<String>>>, on_done: Callback<()>) -> impl IntoView {
    let open = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let lists = Resource::new(
        move || target.get().is_some(),
        |show| async move { if show { api::lists().await } else { Ok(vec![]) } },
    );
    Effect::new(move |_| {
        if target.with(Option::is_some) {
            error.set(None);
            open.set(true);
        }
    });
    Effect::new(move |prev: Option<bool>| {
        let now = open.get();
        if prev == Some(true) && !now {
            target.set(None);
        }
        now
    });
    let ctx = StepCtx::current();
    let pick = Callback::new(move |l: ListDto| {
        let ids = target.get_untracked().unwrap_or_default();
        let (i, name) = (ctx.i18n.clone(), l.name.clone());
        let done = move |n: u32| {
            i.t_args(
                "library-bulk-listed",
                &[("count", n.into()), ("list", name.into())],
            )
        };
        let req = BulkRequestDto {
            list_id: Some(l.id),
            ..request(ids, "add_to_list")
        };
        open.set(false);
        run_bulk(&ctx, req, done, error, Some(on_done));
    });
    view! {
        {move || target.get().map(|ids| {
            let i = use_i18n();
            let title = i.t_args("library-list-pick-title", &[("count", ids.len().into())]);
            view! {
                <Dialog open=open title=title close_label=i.t("dialog-close")>
                    <Suspense fallback=|| view! { <p class="text-sm text-muted-foreground">{t("library-loading")}</p> }>
                        {move || lists.get().map(|r| match r {
                            Ok(v) if v.is_empty() => view! {
                                <p class="text-sm text-muted-foreground">
                                    {t("library-list-none")} " "
                                    <a href="/lists" class="font-medium underline underline-offset-2">{t("library-list-create")}</a>
                                </p>
                            }.into_any(),
                            Ok(v) => view! {
                                <ul class="grid gap-2" data-testid="list-pick">
                                    {v.into_iter().map(|l| {
                                        let name = l.name.clone();
                                        view! {
                                            <li><Button variant=ButtonVariant::Outline class="w-full justify-start"
                                                on:click=move |_| pick.run(l.clone())>{name}</Button></li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }.into_any(),
                            Err(e) => view! { <p role="alert" class="text-sm text-destructive-text">{crate::pages::error_text(&e)}</p> }.into_any(),
                        })}
                    </Suspense>
                </Dialog>
            }
        })}
        {move || error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
    }
}

/// The selection of the Library list view: item ids, with the kind of each for the status
/// menu. It lives above the rows so a background refresh keeps it (DEC-398).
#[derive(Clone, Copy)]
pub struct Selection {
    pub ids: RwSignal<BTreeSet<String>>,
    /// The error line of a bulk change or its undo; owned with the selection, so it stays
    /// visible after a remove has cleared the selection and the bar is gone (DEC-418).
    pub error: RwSignal<Option<String>>,
}

impl Selection {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ids: RwSignal::new(BTreeSet::new()),
            error: RwSignal::new(None),
        }
    }

    pub fn toggle(&self, id: &str, on: bool) {
        self.ids.update(|s| {
            if on {
                s.insert(id.to_string());
            } else {
                s.remove(id);
            }
        });
    }
}

impl Default for Selection {
    fn default() -> Self {
        Self::new()
    }
}

/// The bulk bar as a funnel flow (WP4-5): the first selected row is the intent, a bulk edit
/// ends it as logged, clearing the selection (or leaving the page) unedited abandons it.
fn bulk_flow(funnel: crate::funnel::Funnel, count: Signal<usize>) {
    use crate::funnel::Flow;
    Effect::new(move |prev: Option<usize>| {
        let now = count.get();
        if now > 0 && prev.unwrap_or(0) == 0 {
            funnel.intent(Flow::Bulk);
        } else if now == 0 && prev.is_some_and(|p| p > 0) {
            funnel.abandoned_if(Flow::Bulk);
        }
        now
    });
    on_cleanup(move || funnel.abandoned_if(Flow::Bulk));
}

/// The sticky bar of the list view while rows are selected: the count, Set status (the
/// statuses of the selected kinds), Add tag, Add to list, Remove (confirm with the count),
/// and Clear.
#[component]
pub fn BulkBar(
    selection: Selection,
    /// The statuses offered (the selected rows' kinds).
    #[prop(into)]
    statuses: Signal<Vec<String>>,
    list_pick: ListPick,
    on_done: Callback<()>,
) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let error = selection.error;
    let confirm = RwSignal::new(false);
    let tagging = RwSignal::new(false);
    let count = move || selection.ids.with(BTreeSet::len);
    bulk_flow(ctx.funnel, Signal::derive(count));
    // bulk edits stay online-only (WP4-5)
    let offline = crate::offline::is_offline();
    let ids = move || {
        selection
            .ids
            .get_untracked()
            .into_iter()
            .collect::<Vec<_>>()
    };
    let status_items = {
        let ctx = ctx.clone();
        move || {
            let i = use_i18n();
            statuses
                .get()
                .into_iter()
                .map(|s| {
                    let word = i.t(&format!("status-{s}"));
                    let ctx = ctx.clone();
                    MenuItem {
                        label: i.t_args("library-mark", &[("status", word.clone().into())]),
                        on_select: Callback::new(move |()| {
                            let ids = ids();
                            let (i, word) = (ctx.i18n.clone(), word.clone());
                            let done = move |n: u32| {
                                i.t_args(
                                    "library-bulk-status",
                                    &[("count", n.into()), ("status", word.into())],
                                )
                            };
                            let req = BulkRequestDto {
                                status: Some(s.clone()),
                                ..request(ids, "set_status")
                            };
                            run_bulk(&ctx, req, done, error, Some(on_done));
                        }),
                        destructive: false,
                    }
                })
                .collect::<Vec<_>>()
        }
    };
    let remove = {
        let ctx = ctx.clone();
        Callback::new(move |()| {
            let ids = ids();
            let i = ctx.i18n.clone();
            // cleared once the change went through, after the flow ended as logged (UX4-7);
            // on an error the bar stays with the error and the selection
            let done = move |n: u32| {
                selection.ids.set(BTreeSet::new());
                i.t_args("library-bulk-removed", &[("count", n.into())])
            };
            run_bulk(&ctx, request(ids, "remove"), done, error, Some(on_done));
        })
    };
    let confirm_title = Signal::derive(move || {
        use_i18n().t_args("library-bulk-remove-title", &[("count", count().into())])
    });
    let confirm_body = Signal::derive(move || use_i18n().t("library-bulk-remove-body"));
    view! {
        <div role="toolbar" aria-label=i18n.t("library-bulk-label") data-testid="bulk-bar" data-sticky-bar=""
            class="sticky bottom-[calc(var(--tab-bar-height)+0.5rem)] z-20 mt-3 flex flex-wrap items-center gap-2 rounded-lg border border-border bg-card p-2 shadow-elevation-2">
            <p class="px-2 text-sm font-medium" role="status">
                {move || use_i18n().t_args("library-bulk-count", &[("count", count().into())])}
            </p>
            {move || if offline.get() {
                view! { <p class="px-2 text-sm text-muted-foreground">{t("offline-bulk")}</p> }.into_any()
            } else {
                view! {
                    <DropdownMenu label=use_i18n().t("library-bulk-status-menu") items=status_items() id="bulk-status" trigger_class="h-9 px-3 text-sm"/>
                }.into_any()
            }}
            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm disabled=offline on:click=move |_| tagging.set(true)>{t("library-bulk-tag")}</Button>
            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm disabled=offline on:click=move |_| list_pick.0.set(Some(ids()))>{t("library-bulk-list")}</Button>
            <Button variant=ButtonVariant::Destructive size=ButtonSize::Sm disabled=offline on:click=move |_| confirm.set(true)>{t("library-bulk-remove")}</Button>
            <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on:click=move |_| selection.ids.set(BTreeSet::new())>{t("library-bulk-clear")}</Button>
            {move || error.get().map(|e| view! { <p role="alert" class="w-full px-2 text-sm text-destructive-text">{e}</p> })}
        </div>
        <crate::admin::ConfirmDialog open=confirm title=confirm_title body=confirm_body
            confirm=i18n.t("library-bulk-remove-confirm") on_confirm=remove/>
        <BulkTag open=tagging selection=selection error=error on_done=on_done/>
    }
}

/// "Add tag" for the selection: one field, then a toast with Undo.
#[component]
fn BulkTag(
    open: RwSignal<bool>,
    selection: Selection,
    error: RwSignal<Option<String>>,
    on_done: Callback<()>,
) -> impl IntoView {
    let tag = RwSignal::new(String::new());
    let ctx = StepCtx::current();
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let v = tag.get_untracked().trim().to_string();
        if v.is_empty() {
            return;
        }
        let ids: Vec<String> = selection.ids.get_untracked().into_iter().collect();
        let (i, t) = (ctx.i18n.clone(), v.clone());
        let done = move |n: u32| {
            i.t_args(
                "library-bulk-tagged",
                &[("count", n.into()), ("tag", t.into())],
            )
        };
        let req = BulkRequestDto {
            tag: Some(v),
            ..request(ids, "add_tag")
        };
        open.set(false);
        tag.set(String::new());
        run_bulk(&ctx, req, done, error, Some(on_done));
    };
    let i = use_i18n();
    view! {
        <Dialog open=open title=i.t("library-bulk-tag-title") close_label=i.t("dialog-close") initial_focus="bulk-tag">
            <form class="flex items-end gap-2" on:submit=submit.clone()>
                <div class="flex-1">
                    <FormField id="bulk-tag" label=use_i18n().t("library-tag-new")>
                        <Input id="bulk-tag" value=tag autocomplete="off"/>
                    </FormField>
                </div>
                <Button r#type="submit">{t("library-tag-add")}</Button>
            </form>
        </Dialog>
    }
}

#[cfg(test)]
mod tests {
    use super::failure_key;

    #[test]
    fn a_too_large_change_and_an_expired_undo_say_so() {
        assert_eq!(failure_key("invalid", false), "library-bulk-too-large");
        assert_eq!(failure_key("bad_request", true), "library-undo-expired");
        assert_eq!(failure_key("internal", false), "library-step-failed");
        assert_eq!(failure_key("internal", true), "library-undo-failed");
        assert_eq!(failure_key("bad_request", false), "library-step-failed");
    }
}
