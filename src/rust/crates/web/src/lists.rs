//! Lists (design.md 6.5, 5.10; mockup `lists.html`): the overview with the tabs Mine / Shared
//! with me / On this instance and list cards; the list page (`/lists/:id`) with the editable
//! title and description, a drag handle and keyboard "Move up/down" (DEC-353), Remove, "Add" through the search palette
//! scoped to the list, "Add all to my library" for lists of others; the share dialog with the
//! three visibility choices; the public page of a link-shared list (`/shared/lists/:token`).
//! Reads come from server functions, changes go to the REST routes (`web::rest`, DEC-327).

use contracts::{ListCardDto, ListDto, ListPageDto, ListRowDto, ListsOverviewDto};
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde_json::json;
use ui::{
    Badge, BadgeVariant, Button, ButtonLink, ButtonSize, ButtonVariant, EmptyState, FormField,
    IconName, Input, Sheet, SheetSide, Tabs, TabsContent, TabsList, TabsTrigger, Textarea,
};

use crate::admin::ConfirmDialog;
use crate::api;
use crate::i18n::{I18n, kind_label, t, use_i18n};
use crate::pages::PageTitle;
use crate::people::{day_words, refetch_on_update};
use crate::rest;
use crate::settings::{Lead, Outcome, kept, loaded};
use crate::sharing::share_error;

use crate::list_drag::{DragHandle, ListDrag, row_class};
use crate::list_share::{ShareDialog, ShareMeta};

/// The visibility as a word (`lists-vis-<choice>`); a badge, never colour alone.
fn visibility_badge(v: &str) -> AnyView {
    let (variant, key) = match v {
        "instance" => (BadgeVariant::Info, "lists-vis-instance"),
        "link" => (BadgeVariant::Success, "lists-vis-link"),
        _ => (BadgeVariant::Outline, "lists-vis-private"),
    };
    view! { <Badge variant=variant>{t(key)}</Badge> }.into_any()
}

/// "3 items - Movie, Book".
fn count_and_kinds(i18n: &I18n, count: u32, kinds: &[String]) -> String {
    let items = i18n.t_args("lists-items", &[("count", count.into())]);
    if kinds.is_empty() {
        return items;
    }
    let kinds: Vec<String> = kinds.iter().map(|k| kind_label(k)).collect();
    i18n.t_args(
        "lists-items-kinds",
        &[("items", items.into()), ("kinds", kinds.join(", ").into())],
    )
}

#[component]
pub fn Lists() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::lists_overview());
    // nothing on the overview reads it again by itself: no own refresh to pace (LR-5)
    let _ = refetch_on_update(move || data.refetch());
    let new = RwSignal::new(false);
    // the palette's "New list" (`?new=1`, WP4-4) opens the sheet once the page is in the
    // browser; read in the effect (a client-side navigation builds the page first)
    let query = leptos_router::hooks::use_query_map();
    Effect::new(move |_| {
        if query.with(|q| q.get("new").is_some()) {
            new.set(true);
        }
    });
    view! {
        <PageTitle text=t("lists-title")/>
        <Lead text=t("lists-lead")/>
        // a grid page: the cards use the whole 96 rem column (DEC-541, review OT2D-2)
        <div class="grid gap-4">
            <Button class="w-fit" on:click=move |_| new.set(true)>{t("lists-new-open")}</Button>
            {loaded(data, move |ov: ListsOverviewDto| view! { <Overview ov=ov/> })}
        </div>
        <NewList open=new/>
    }
}

#[component]
fn Overview(ov: ListsOverviewDto) -> impl IntoView {
    let i18n = use_i18n();
    let tab = kept("ls-tab", || "mine".to_string());
    let label = |key: &str, n: usize| i18n.t_args(key, &[("count", n.into())]);
    let (mine_l, shared_l, here_l) = (
        label("lists-tab-mine", ov.mine.len()),
        label("lists-tab-shared", ov.shared_with_me.len()),
        label("lists-tab-instance", ov.on_instance.len()),
    );
    let tabs = StoredValue::new(ov);
    view! {
        <Tabs id="ls-tabs" active=tab>
            <TabsList label=t("lists-tabs") class="h-auto w-fit flex-wrap">
                <TabsTrigger value="mine">{mine_l}</TabsTrigger>
                <TabsTrigger value="shared">{shared_l}</TabsTrigger>
                <TabsTrigger value="instance">{here_l}</TabsTrigger>
            </TabsList>
            <TabsContent value="mine">
                {move || view! { <CardsOr lists=tabs.with_value(|o| o.mine.clone()) empty="lists-empty"/> }}
            </TabsContent>
            <TabsContent value="shared">
                {move || view! { <CardsOr lists=tabs.with_value(|o| o.shared_with_me.clone()) empty="lists-empty-shared"/> }}
            </TabsContent>
            <TabsContent value="instance">
                {move || view! { <CardsOr lists=tabs.with_value(|o| o.on_instance.clone()) empty="lists-empty-instance"/> }}
            </TabsContent>
        </Tabs>
    }
}

#[component]
fn CardsOr(lists: Vec<ListCardDto>, empty: &'static str) -> impl IntoView {
    if lists.is_empty() {
        view! { <EmptyState title=t(empty) icon=IconName::List level=2/> }.into_any()
    } else {
        view! { <ListCards lists=lists/> }.into_any()
    }
}

/// List cards (design.md 6.5): cover strip, count and kinds, visibility, owner, freshness.
/// A fourth column where the cards have 80 rem or more to themselves (the Lists page on a
/// wide screen, DEC-541); on a person's page they sit in a narrower column and stay at three.
#[component]
pub(crate) fn ListCards(lists: Vec<ListCardDto>) -> impl IntoView {
    view! {
        <div class="@container mt-2">
            <ul class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 @7xl:grid-cols-4" aria-label=t("lists-title")>
                {lists.into_iter().map(|l| view! { <ListCard l=l/> }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
fn ListCard(l: ListCardDto) -> impl IntoView {
    let i18n = use_i18n();
    let what = count_and_kinds(&i18n, l.count, &l.kinds);
    let updated = i18n.t_args(
        "lists-updated",
        &[("when", day_words(&i18n, &l.updated_day).into())],
    );
    let by =
        (!l.mine).then(|| i18n.t_args("sessions-by", &[("name", l.owner_name.clone().into())]));
    let covers = l
        .covers
        .iter()
        .map(|c| view! { <img src=c.url.clone() alt="" loading="lazy" class="h-20 w-14 shrink-0 rounded object-cover"/> })
        .collect_view();
    view! {
        <li class="grid gap-2 rounded-xl border border-border bg-card p-4 shadow-elevation-1" data-testid="ls-card">
            <div class="flex h-20 gap-1 overflow-hidden" aria-hidden="true">{covers}</div>
            <h3 class="break-words font-semibold">
                <a href=format!("/lists/{}", l.id) class="underline-offset-2 hover:underline">{l.name.clone()}</a>
            </h3>
            <p class="text-sm text-muted-foreground">{what}</p>
            <p class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                {visibility_badge(&l.visibility)}
                {by.map(|b| view! { <span>{b}</span> })}
                <span>{updated}</span>
            </p>
        </li>
    }
}

/// "New list": a one-field sheet, then the list page.
#[component]
fn NewList(open: RwSignal<bool>) -> impl IntoView {
    let i18n = use_i18n();
    let name = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let navigate = leptos_router::hooks::use_navigate();
    let create = Action::new(move |(): &()| {
        let n = name.get_untracked();
        let navigate = navigate.clone();
        async move {
            match rest::call::<ListDto>("POST", "/lists", Some(json!({"name": n}))).await {
                Ok(Some(l)) => {
                    open.set(false);
                    name.set(String::new());
                    navigate(
                        &format!("/lists/{}", l.id),
                        leptos_router::NavigateOptions::default(),
                    );
                }
                Ok(None) => error.set(Some(t("error-generic"))),
                Err(e) => error.set(Some(share_error(&e))),
            }
        }
    });
    view! {
        <Sheet open=open side=SheetSide::Adaptive title=i18n.t("lists-new-open") close_label=i18n.t("dialog-close")
            initial_focus="ls-new-name">
            <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); create.dispatch(()); }>
                <FormField id="ls-new-name" label=t("lists-new") error=Signal::derive(move || error.get())>
                    <Input id="ls-new-name" value=name autocomplete="off"/>
                </FormField>
                <Button r#type="submit" class="w-fit" loading=create.pending()>{t("lists-create")}</Button>
            </form>
        </Sheet>
    }
}

/// The page's live messages: a move ("Alpha is now number 3 of 3."), an add, a removal.
#[derive(Clone, Copy)]
struct Page {
    /// Read the page again after an own change (paced with the events, LR-5).
    refresh: Callback<()>,
    outcome: RwSignal<Option<Result<String, String>>>,
    status: RwSignal<String>,
    /// After a move the button of the moved row gets focus back: `lst-up-<item>` etc.
    focus: RwSignal<Option<String>>,
    share: RwSignal<bool>,
    meta: RwSignal<Option<ShareMeta>>,
}

#[component]
pub fn ListPage() -> impl IntoView {
    let params = use_params_map();
    let id = move || params.with(|p| p.get("id").unwrap_or_default());
    let data = Resource::new(id, api::list_page);
    let own = refetch_on_update(move || data.refetch());
    let refresh = own.wrap(Callback::new(move |()| data.refetch()));
    let page = Page {
        refresh,
        outcome: RwSignal::new(None),
        status: RwSignal::new(String::new()),
        focus: RwSignal::new(None),
        share: RwSignal::new(false),
        meta: RwSignal::new(None),
    };
    provide_context(page);
    provide_context(ListDrag::new());
    // the palette's scope ends with the page
    let target = expect_context::<crate::palette::PaletteScope>().0;
    on_cleanup(move || target.set(None));
    view! {
        <p class="mb-2 text-sm"><a href="/lists" class="text-primary underline">{t("lists-back")}</a></p>
        <div class="grid max-w-4xl gap-4">
            <Outcome message=page.outcome/>
            <p role="status" class="sr-only" data-testid="lst-status">{move || page.status.get()}</p>
            {loaded(data, move |l: ListPageDto| view! { <ListBody l=l/> })}
        </div>
        <ShareDialog open=page.share meta=page.meta on_saved=refresh/>
    }
}

#[component]
fn ListBody(l: ListPageDto) -> impl IntoView {
    let i18n = use_i18n();
    let by = i18n.t_args("sessions-by", &[("name", l.owner_name.clone().into())]);
    let updated = i18n.t_args(
        "lists-updated",
        &[("when", day_words(&i18n, &l.updated_day).into())],
    );
    let count = l.items.len();
    let head = if l.mine {
        view! { <OwnHead l=l.clone()/> }.into_any()
    } else {
        view! {
            <PageTitle text=l.name.clone()/>
            {l.description.clone().map(|d| view! { <p class="whitespace-pre-line">{d}</p> })}
            <p class="text-sm text-muted-foreground">{by}</p>
            <AddAll id=l.id.clone() shared_token=None/>
        }
        .into_any()
    };
    let rows = l.items.clone();
    let (mine, list_id, name) = (l.mine, l.id.clone(), l.name.clone());
    view! {
        {head}
        <p class="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
            {visibility_badge(&l.visibility)}
            <span>{count_and_kinds(&i18n, u32::try_from(count).unwrap_or(u32::MAX), &[])}</span>
            <span>{updated}</span>
        </p>
        {mine.then(|| view! { <AddButton id=list_id.clone() name=name.clone()/> })}
        {if rows.is_empty() {
            view! { <EmptyState title=t("lists-empty-items") icon=IconName::List level=2/> }.into_any()
        } else {
            view! {
                <ol class="grid gap-2" aria-label=l.name.clone()>
                    {rows.into_iter().enumerate().map(|(i, r)| view! {
                        <Row r=r index=i count=count list=list_id.clone() mine=mine/>
                    }).collect_view()}
                </ol>
            }.into_any()
        }}
    }
}

/// "Add": the search palette, scoped to this list (choosing a result adds it here).
#[component]
fn AddButton(id: String, name: String) -> impl IntoView {
    let target = expect_context::<crate::palette::PaletteScope>().0;
    let open = expect_context::<crate::palette::PaletteOpen>().0;
    let label = use_i18n().t_args("lists-add-named", &[("name", name.clone().into())]);
    view! {
        <Button class="w-fit" attr:aria-label=label on:click=move |_| {
            target.set(Some(crate::palette::PaletteTarget::List {
                id: id.clone(),
                name: name.clone(),
            }));
            open.set(true);
        }>{t("lists-add")}</Button>
    }
}

/// The owner's head: the title and description, editable in place, Share and Delete.
#[component]
fn OwnHead(l: ListPageDto) -> impl IntoView {
    let page = expect_context::<Page>();
    let editing = kept("ls-editing", || false);
    let share = page.share;
    page.meta.set(Some(ShareMeta {
        id: l.id.clone(),
        name: l.name.clone(),
        visibility: l.visibility.clone(),
        share_path: l.share_path.clone(),
    }));
    let delete = RwSignal::new(false);
    let i18n = use_i18n();
    let id = StoredValue::new(l.id.clone());
    let remove = Action::new(move |(): &()| {
        let path = format!("/lists/{}", id.get_value());
        async move {
            match rest::send("DELETE", &path, None).await {
                Ok(()) => crate::pages::go("/lists"),
                Err(e) => page.outcome.set(Some(Err(share_error(&e)))),
            }
        }
    });
    let title = Signal::derive({
        let (i, n) = (i18n.clone(), l.name.clone());
        move || i.t_args("lists-delete-title", &[("name", n.clone().into())])
    });
    let share_label = i18n.t_args("lists-share-named", &[("name", l.name.clone().into())]);
    let edit = l.clone();
    view! {
        <PageTitle text=l.name.clone()/>
        {l.description.clone().map(|d| view! { <p class="whitespace-pre-line">{d}</p> })}
        <Show when=move || editing.get()>
            <EditForm l=edit.clone() editing=editing/>
        </Show>
        <div class="flex flex-wrap gap-2">
            <Button variant=ButtonVariant::Outline attr:aria-label=share_label on:click=move |_| share.set(true)>{t("lists-share")}</Button>
            <Button variant=ButtonVariant::Outline on:click=move |_| editing.update(|e| *e = !*e)
                attr:aria-expanded=move || if editing.get() { "true" } else { "false" }>
                {t("lists-edit")}
            </Button>
            <Button variant=ButtonVariant::Ghost on:click=move |_| delete.set(true)>{t("lists-delete")}</Button>
        </div>
        <ConfirmDialog open=delete title=title body=Signal::derive(move || t("lists-delete-body"))
            confirm=t("lists-delete") on_confirm=Callback::new(move |()| { remove.dispatch(()); })/>
    }
}

#[component]
fn EditForm(l: ListPageDto, editing: RwSignal<bool>) -> impl IntoView {
    let page = expect_context::<Page>();
    let name = RwSignal::new(l.name.clone());
    let description = RwSignal::new(l.description.clone().unwrap_or_default());
    let error = RwSignal::new(None::<String>);
    let id = l.id;
    let save = Action::new(move |(): &()| {
        let path = format!("/lists/{id}");
        let body =
            json!({"name": name.get_untracked(), "description": description.get_untracked()});
        async move {
            match rest::send("PATCH", &path, Some(body)).await {
                Ok(()) => {
                    editing.set(false);
                    page.outcome.set(Some(Ok(t("lists-saved"))));
                    page.refresh.run(());
                }
                Err(e) => error.set(Some(share_error(&e))),
            }
        }
    });
    view! {
        <form class="grid max-w-xl gap-3 rounded-xl border border-border p-4" on:submit=move |ev| { ev.prevent_default(); save.dispatch(()); }>
            <FormField id="lst-title" label=t("lists-title-label") error=Signal::derive(move || error.get())>
                <Input id="lst-title" value=name autocomplete="off"/>
            </FormField>
            <FormField id="lst-description" label=t("lists-description")>
                <Textarea id="lst-description" value=description rows=3u32/>
            </FormField>
            <div class="flex flex-wrap gap-2">
                <Button r#type="submit" loading=save.pending()>{t("lists-save")}</Button>
                <Button variant=ButtonVariant::Outline on:click=move |_| editing.set(false)>{t("reauth-cancel")}</Button>
            </div>
        </form>
    }
}

/// One item: title, kind, "In your library"; for the owner a drag handle, Move up / Move
/// down and Remove.
#[component]
fn Row(r: ListRowDto, index: usize, count: usize, list: String, mine: bool) -> impl IntoView {
    let i18n = use_i18n();
    let item = r.item;
    let kind = kind_label(&item.kind);
    let title = match item.year {
        Some(y) => format!("{} ({y})", crate::i18n::title_text(&item.title)),
        None => crate::i18n::title_text(&item.title),
    };
    let drag = expect_context::<ListDrag>();
    let (handle, controls) = if mine {
        let mv = move_action(
            list.clone(),
            item.id.clone(),
            crate::i18n::title_text(&item.title),
            count,
        );
        let on_drop = Callback::new(move |to: usize| {
            mv.dispatch((to, None));
        });
        (
            Some(view! { <DragHandle index=index count=count on_drop=on_drop/> }),
            Some(
                view! { <RowControls mv=mv item=item.id.clone() title=crate::i18n::title_text(&item.title) list=list index=index count=count/> },
            ),
        )
    } else {
        (None, None)
    };
    view! {
        <li class=move || format!("flex flex-wrap items-center gap-3 rounded-md border border-border px-3 py-2 {}", row_class(drag, index))
            data-testid="lst-row" data-drag-row="">
            {handle}
            <span class="w-6 text-right text-sm text-muted-foreground" aria-hidden="true">{index + 1}</span>
            {item.cover.clone().map(|c| view! { <img src=c.url alt="" loading="lazy" class="h-12 w-8 rounded object-cover"/> })}
            <span class="min-w-0 flex-1">
                <a href=format!("/items/{}", item.id) class="font-medium underline-offset-2 hover:underline">{title}</a>
                <span class="block text-xs text-muted-foreground">{kind}</span>
            </span>
            {r.in_library.then(|| view! { <Badge variant=BadgeVariant::Secondary>{i18n.t("lists-in-library")}</Badge> })}
            {controls}
        </li>
    }
}

/// A move of one row to an index; with a focus target (the keyboard buttons) the moved row's
/// button gets the focus back after the refetch.
type MoveAction = Action<(usize, Option<String>), ()>;

fn move_action(list: String, item: String, title: String, count: usize) -> MoveAction {
    let page = expect_context::<Page>();
    Action::new(move |(to, focus): &(usize, Option<String>)| {
        let (to, focus) = (*to, focus.clone());
        let path = format!("/lists/{list}/items/{item}/position");
        let title = title.clone();
        let i18n = use_i18n();
        async move {
            match rest::send("PUT", &path, Some(json!({"to": to}))).await {
                Ok(()) => {
                    page.status.set(i18n.t_args(
                        "lists-moved",
                        &[
                            ("title", title.into()),
                            ("position", (to + 1).into()),
                            ("count", count.into()),
                        ],
                    ));
                    if let Some(focus) = focus {
                        page.focus.set(Some(focus.clone()));
                        set_timeout(
                            move || {
                                if page.focus.get_untracked() == Some(focus) {
                                    page.focus.set(None);
                                }
                            },
                            std::time::Duration::from_secs(FOCUS_KEEP_SECS),
                        );
                    }
                    page.refresh.run(());
                }
                Err(e) => page.outcome.set(Some(Err(share_error(&e)))),
            }
        }
    })
}

#[component]
fn RowControls(
    mv: MoveAction,
    list: String,
    item: String,
    title: String,
    index: usize,
    count: usize,
) -> impl IntoView {
    let i18n = use_i18n();
    let page = expect_context::<Page>();
    let (up_id, down_id) = (format!("lst-up-{item}"), format!("lst-down-{item}"));
    // focus comes back to the moved row's button in the render after the refetch (keyboard
    // reorder): when the row mounts, so the rows being replaced do not take it; the target
    // stays a few seconds, as the `entity.updated` event of the move refetches once more
    let (u, d) = (up_id.clone(), down_id.clone());
    Effect::new(move |_| {
        if let Some(f) = page.focus.get_untracked()
            && (f == u || f == d)
        {
            focus_by_id(&f);
        }
    });
    let remove = Action::new({
        let (list, item, title) = (list, item, title.clone());
        move |(): &()| {
            let path = format!("/lists/{list}/items/{item}");
            let title = title.clone();
            let i18n = use_i18n();
            async move {
                let r = rest::send("DELETE", &path, None).await;
                page.outcome.set(Some(match r {
                    Ok(()) => Ok(i18n.t_args("lists-removed", &[("title", title.into())])),
                    Err(e) => Err(share_error(&e)),
                }));
                page.refresh.run(());
            }
        }
    });
    let label = |key: &str| i18n.t_args(key, &[("title", title.clone().into())]);
    let (up_label, down_label, remove_label) = (
        label("lists-move-up-named"),
        label("lists-move-down-named"),
        label("lists-remove-named"),
    );
    let (first, last) = (index == 0, index + 1 >= count);
    // the button that keeps the focus after the move: at either end the same arrow is
    // disabled (focus would fall to the body), so the other arrow takes it (UX3-6, WCAG 2.4.3)
    let up_focus = if index <= 1 {
        down_id.clone()
    } else {
        up_id.clone()
    };
    let down_focus = if index + 2 >= count {
        up_id.clone()
    } else {
        down_id.clone()
    };
    let (u2, d2) = (up_focus, down_focus);
    view! {
        <span class="flex flex-wrap gap-1">
            <Button size=ButtonSize::Sm variant=ButtonVariant::Outline disabled=first attr:id=up_id attr:aria-label=up_label
                on:click=move |_| { mv.dispatch((index.saturating_sub(1), Some(u2.clone()))); }>{t("lists-move-up")}</Button>
            <Button size=ButtonSize::Sm variant=ButtonVariant::Outline disabled=last attr:id=down_id attr:aria-label=down_label
                on:click=move |_| { mv.dispatch((index + 1, Some(d2.clone()))); }>{t("lists-move-down")}</Button>
            <Button size=ButtonSize::Sm variant=ButtonVariant::Ghost attr:aria-label=remove_label
                on:click=move |_| { remove.dispatch(()); }>{t("lists-remove")}</Button>
        </span>
    }
}

/// How long the moved row keeps the focus target across refetches.
const FOCUS_KEEP_SECS: u64 = 3;

/// Focus the element with this id (browser only).
fn focus_by_id(id: &str) {
    #[cfg(feature = "hydrate")]
    if let Some(el) = document().get_element_by_id(id) {
        use leptos::wasm_bindgen::JsCast;
        if let Ok(el) = el.dyn_into::<web_sys::HtmlElement>() {
            let _ = el.focus();
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}

/// "Add all to my library" for a list of someone else (by id) or a link-shared list.
#[component]
pub(crate) fn AddAll(id: String, shared_token: Option<String>) -> impl IntoView {
    // the adds refetch the page (`entity.updated`): the outcome outlives that render
    let message = kept("ls-added", || None::<Result<String, String>>);
    let run = Action::new(move |(): &()| {
        let path = match &shared_token {
            Some(token) => format!("/shared/lists/{token}/library"),
            None => format!("/lists/{id}/library"),
        };
        let i18n = use_i18n();
        async move {
            let r = rest::call::<contracts::AddedDto>("POST", &path, None).await;
            message.set(Some(match r {
                Ok(Some(a)) => Ok(i18n.t_args(
                    "lists-added-all",
                    &[("added", a.added.into()), ("already", a.already.into())],
                )),
                Ok(None) => Err(t("error-generic")),
                Err(e) => Err(share_error(&e)),
            }));
        }
    });
    view! {
        <div class="grid gap-2">
            <Button class="w-fit" loading=run.pending() on:click=move |_| { run.dispatch(()); }>{t("lists-add-all")}</Button>
            <Outcome message=message/>
        </div>
    }
}

/// The public page of a link-shared list (design.md 6.5): the brand header of the shell, the
/// title, the owner and the posters; never ratings or notes; "Sign in to add these to your
/// library"; a friendly page when the link was revoked.
#[component]
pub fn SharedList() -> impl IntoView {
    let params = use_params_map();
    let token = move || params.with(|p| p.get("token").unwrap_or_default());
    let data = Resource::new(token, api::shared_list);
    let cfg = expect_context::<crate::app::ClientConfig>();
    view! {
        <Transition fallback=|| view! { <ui::Skeleton class="h-24 w-full"/> }>
            {move || data.get().map(|r| match r {
                Err(_) => view! { <SharedMissing/> }.into_any(),
                Ok(l) => view! { <SharedBody l=l token=token() signed_in=cfg.signed_in/> }.into_any(),
            })}
        </Transition>
    }
}

/// An unknown or revoked link answers 404, so link checkers and crawlers drop it (UX3-16).
/// The route renders in `SsrMode::Async`: the status is still open when this renders.
fn not_found_status() {
    #[cfg(feature = "ssr")]
    if let Some(r) = use_context::<leptos_axum::ResponseOptions>() {
        r.set_status(axum::http::StatusCode::NOT_FOUND);
    }
}

#[component]
fn SharedMissing() -> impl IntoView {
    not_found_status();
    let cfg = expect_context::<crate::app::ClientConfig>();
    view! {
        <section class="mx-auto max-w-xl py-16 text-center" data-testid="sl-missing">
            <PageTitle text=t("shared-list-missing")/>
            <p class="text-muted-foreground">{t("shared-list-missing-lead")}</p>
            <p class="mt-6 flex justify-center gap-4">
                <a href="/" class="text-primary underline">{t("nav-home")}</a>
                {cfg.signed_in.then(|| view! { <a href="/lists" class="text-primary underline">{t("lists-title")}</a> })}
            </p>
        </section>
    }
}

#[component]
fn SharedBody(l: contracts::SharedListDto, token: String, signed_in: bool) -> impl IntoView {
    let i18n = use_i18n();
    let product = expect_context::<crate::app::ClientConfig>().product_name;
    let by = i18n.t_args("sessions-by", &[("name", l.owner_name.clone().into())]);
    let made = i18n.t_args("shared-list-made-with", &[("product", product.into())]);
    // the whole return path is one query value (UX3-19)
    let next = format!(
        "/sign-in?next={}",
        crate::sign_in::encode(&format!("/shared/lists/{token}"))
    );
    let cta = if signed_in {
        view! { <AddAll id=String::new() shared_token=Some(token)/> }.into_any()
    } else {
        view! {
            <ButtonLink href=next class="min-h-11 w-fit">{t("shared-list-cta")}</ButtonLink>
        }
        .into_any()
    };
    let posters = l
        .items
        .into_iter()
        .map(|i| {
            let title = match i.year {
                Some(y) => format!("{} ({y})", crate::i18n::title_text(&i.title)),
                None => crate::i18n::title_text(&i.title),
            };
            view! {
                <li class="grid gap-1">
                    {i.cover.map(|c| view! { <img src=c.url alt="" loading="lazy" class="aspect-[2/3] w-full rounded-md object-cover"/> })}
                    <span class="text-sm font-medium">{title}</span>
                    <span class="text-xs text-muted-foreground">{kind_label(&i.kind)}</span>
                </li>
            }
        })
        .collect_view();
    view! {
        <div class="grid max-w-5xl gap-4">
            <PageTitle text=l.name.clone()/>
            <p class="text-sm text-muted-foreground">{by}</p>
            {l.description.clone().map(|d| view! { <p class="whitespace-pre-line">{d}</p> })}
            {cta}
            <ul class="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5" aria-label=l.name.clone()>{posters}</ul>
            <footer class="mt-8 text-xs text-muted-foreground">{made}</footer>
        </div>
    }
}
