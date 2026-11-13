//! People > Game groups (design.md 6.7, WP4-6): one card per group with its members, the
//! plays logged as the group and the latest, "Log a play with" (the search palette picks the
//! game, then its play sheet opens with the group chosen: the scoped pick of WP4-4), Edit and
//! Delete; the "New group" and edit dialog and the delete confirmation are mounted once
//! outside the refetched page. Changes go to the REST routes (DEC-327).

use contracts::{CompanionDto, GameGroupDto};
use leptos::prelude::*;
use serde_json::json;
use ui::{
    Button, ButtonSize, ButtonVariant, CardActions, Dialog, DropdownMenu, EmptyState, FormField,
    IconName, Input, MenuItem,
};

use crate::admin::ConfirmDialog;
use crate::api;
use crate::i18n::{t, use_i18n};
use crate::people::day_words;
use crate::rest;
use crate::settings::kept_menu;
use crate::sharing::share_error;
use crate::widgets::CompanionPicker;

/// What the group form is open for.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FormTarget {
    New,
    Edit(GameGroupDto),
}

impl FormTarget {
    fn group(self) -> Option<GameGroupDto> {
        match self {
            Self::New => None,
            Self::Edit(g) => Some(g),
        }
    }
}

/// Which group dialog is open and for which group.
#[derive(Clone, Copy)]
pub(crate) struct GroupUi {
    pub form: RwSignal<Option<FormTarget>>,
    pub remove: RwSignal<Option<GameGroupDto>>,
    /// Shows a change's outcome on the page and refetches it (the People `Ui`).
    pub report: Callback<Result<String, ServerFnError>>,
}

impl GroupUi {
    pub fn new(report: Callback<Result<String, ServerFnError>>) -> Self {
        Self {
            form: RwSignal::new(None),
            remove: RwSignal::new(None),
            report,
        }
    }
}

/// The tab's content: "New group" and the cards.
#[component]
pub(crate) fn GroupsTab(groups: Vec<GameGroupDto>) -> impl IntoView {
    let ui = expect_context::<GroupUi>();
    let body = if groups.is_empty() {
        view! { <EmptyState title=t("groups-empty") icon=IconName::Users body=t("groups-empty-body") level=2/> }.into_any()
    } else {
        view! {
            <ul class="grid gap-3 md:grid-cols-2" aria-label=t("groups-list")>
                {groups.into_iter().map(|g| view! { <GroupCard g=g/> }).collect_view()}
            </ul>
        }
        .into_any()
    };
    view! {
        <div class="mt-2 grid gap-3">
            <Button class="w-fit" variant=ButtonVariant::Outline on:click=move |_| ui.form.set(Some(FormTarget::New))>{t("groups-new")}</Button>
            {body}
        </div>
    }
}

/// "Log a play with <group>": the palette scoped to the group (DEC-428 pattern).
fn log_button(g: &GameGroupDto) -> impl IntoView + use<> {
    let i18n = use_i18n();
    let scope = expect_context::<crate::palette::PaletteScope>().0;
    let open = expect_context::<crate::palette::PaletteOpen>().0;
    let label = i18n.t_args("groups-log-named", &[("name", g.name.clone().into())]);
    let (id, name) = (g.id.clone(), g.name.clone());
    view! {
        <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=label
            on:click=move |_| {
                scope.set(Some(crate::palette::PaletteTarget::LogWithGroup {
                    group_id: id.clone(),
                    name: name.clone(),
                }));
                open.set(true);
            }>
            {t("groups-log")}
        </Button>
    }
}

#[component]
fn GroupCard(g: GameGroupDto) -> impl IntoView {
    let i18n = use_i18n();
    let ui = expect_context::<GroupUi>();
    let members = if g.members.is_empty() {
        i18n.t("groups-no-members")
    } else {
        let names: Vec<String> = g.members.iter().map(|m| m.name.clone()).collect();
        i18n.t_args("groups-members", &[("names", names.join(", ").into())])
    };
    let last = g.last.as_ref().map_or_else(
        || i18n.t("groups-never"),
        |l| {
            i18n.t_args(
                "groups-last",
                &[
                    ("title", crate::i18n::title_text(&l.title).into()),
                    ("when", day_words(&i18n, &l.local_day).into()),
                ],
            )
        },
    );
    let (for_edit, for_delete) = (g.clone(), g.clone());
    let items = vec![
        MenuItem {
            label: t("groups-edit"),
            on_select: Callback::new(move |()| {
                ui.form.set(Some(FormTarget::Edit(for_edit.clone())));
            }),
            destructive: false,
        },
        MenuItem {
            label: t("groups-delete"),
            on_select: Callback::new(move |()| ui.remove.set(Some(for_delete.clone()))),
            destructive: true,
        },
    ];
    let menu_label = i18n.t_args("groups-menu-named", &[("name", g.name.clone().into())]);
    let log = log_button(&g);
    let menu = kept_menu(&format!("pp-group-menu-{}", g.id));
    view! {
        <li class="grid gap-3 rounded-xl border border-border bg-card p-4 shadow-elevation-1" data-testid="pp-group-card">
            <div class="flex items-start gap-3">
                <div class="min-w-0 flex-1">
                    <h3 class="break-words font-semibold">{g.name.clone()}</h3>
                    <p class="text-sm text-muted-foreground">{members}</p>
                </div>
                <DropdownMenu label=menu_label id=format!("pp-group-menu-{}", g.id) items=items
                    open=menu.open active=menu.active/>
            </div>
            <dl class="flex flex-wrap gap-x-6 gap-y-1">
                <div class="grid">
                    <dt class="text-xs text-muted-foreground">{t("groups-plays")}</dt>
                    <dd class="text-lg font-semibold" data-testid="pp-group-plays">{g.plays.to_string()}</dd>
                </div>
            </dl>
            <p class="text-sm text-muted-foreground">{last}</p>
            <CardActions>{log}</CardActions>
        </li>
    }
}

/// The group dialogs, mounted once outside the refetched data.
#[component]
pub(crate) fn GroupDialogs(ui: GroupUi) -> impl IntoView {
    view! {
        <GroupForm ui=ui/>
        <DeleteGroup ui=ui/>
    }
}

/// New group or edit: a name and the members (the person picker without you).
#[component]
fn GroupForm(ui: GroupUi) -> impl IntoView {
    let open = RwSignal::new(false);
    let name = RwSignal::new(String::new());
    let members = RwSignal::new(Vec::<String>::new());
    let companions = RwSignal::new(Vec::<CompanionDto>::new());
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        let Some(target) = ui.form.get() else {
            open.set(false);
            return;
        };
        let target = target.group();
        name.set(target.as_ref().map(|g| g.name.clone()).unwrap_or_default());
        members.set(
            target
                .map(|g| g.members.into_iter().map(|m| m.id).collect())
                .unwrap_or_default(),
        );
        error.set(None);
        open.set(true);
        leptos::task::spawn_local(async move {
            if let Ok(cs) = api::companions().await {
                companions.set(cs);
            }
        });
    });
    Effect::new(move |_| {
        if !open.get() && ui.form.get_untracked().is_some() {
            ui.form.set(None);
        }
    });
    let save = Action::new(move |(): &()| {
        let editing = ui.form.get_untracked().and_then(FormTarget::group);
        let (n, m) = (name.get_untracked(), members.get_untracked());
        let i18n = use_i18n();
        async move {
            if m.is_empty() {
                error.set(Some(i18n.t("groups-need-members")));
                return;
            }
            let body = json!({"name": n, "members": m});
            let r = match &editing {
                Some(g) => rest::send("PUT", &format!("/game-groups/{}", g.id), Some(body)).await,
                None => rest::send("POST", "/game-groups", Some(body)).await,
            };
            match r {
                Ok(()) => {
                    open.set(false);
                    let key = if editing.is_some() {
                        "groups-saved"
                    } else {
                        "groups-created"
                    };
                    ui.report.run(Ok(
                        i18n.t_args(key, &[("name", n.trim().to_string().into())])
                    ));
                }
                Err(e) => error.set(Some(share_error(&e))),
            }
        }
    });
    move || {
        let i18n = use_i18n();
        let title = match ui.form.get().and_then(FormTarget::group) {
            Some(g) => i18n.t_args("groups-edit-title", &[("name", g.name.into())]),
            None => i18n.t("groups-new-title"),
        };
        view! {
            <Dialog open=open title=title close_label=i18n.t("dialog-close") initial_focus="pp-group-name">
                <form class="grid gap-4" novalidate on:submit=move |ev| { ev.prevent_default(); save.dispatch(()); }>
                    <FormField id="pp-group-name" label=t("groups-name") error=Signal::derive(move || error.get())>
                        <Input id="pp-group-name" value=name autocomplete="off"/>
                    </FormField>
                    <CompanionPicker id="pp-group-members" legend=t("groups-members-legend") companions=companions selected=members/>
                    <div class="flex flex-wrap justify-end gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| open.set(false)>{t("reauth-cancel")}</Button>
                        <Button r#type="submit" loading=save.pending()>{t("groups-save")}</Button>
                    </div>
                </form>
            </Dialog>
        }
    }
}

#[component]
fn DeleteGroup(ui: GroupUi) -> impl IntoView {
    let open = RwSignal::new(false);
    Effect::new(move |_| open.set(ui.remove.with(Option::is_some)));
    let i18n = use_i18n();
    let i1 = i18n.clone();
    let name = move || {
        ui.remove
            .with(|g| g.as_ref().map(|g| g.name.clone()).unwrap_or_default())
    };
    let title =
        Signal::derive(move || i1.t_args("groups-delete-title", &[("name", name().into())]));
    let body = Signal::derive(move || {
        let plays = ui.remove.with(|g| g.as_ref().map_or(0, |g| g.plays));
        i18n.t_args("groups-delete-body", &[("count", plays.into())])
    });
    let run = Action::new(move |(): &()| {
        let g = ui.remove.get_untracked();
        let i18n = use_i18n();
        async move {
            let Some(g) = g else { return };
            let r = rest::send("DELETE", &format!("/game-groups/{}", g.id), None).await;
            ui.remove.set(None);
            ui.report
                .run(r.map(|()| i18n.t_args("groups-deleted", &[("name", g.name.into())])));
        }
    });
    view! {
        <ConfirmDialog open=open title=title body=body confirm=t("groups-delete")
            on_confirm=Callback::new(move |()| { run.dispatch(()); })/>
    }
}
