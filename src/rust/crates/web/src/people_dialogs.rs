//! The People dialogs (design.md 5.11, 6.7; mockup `companions.html`): "Add a person", invite
//! with its consequence and the single-use link (14 days, DEC-022), link by username,
//! unlink, merge (keep / fold in, undoable from the merge record), edit and delete ("keeps
//! history"). One [`Ui`] per page holds which dialog is open and for whom.

use contracts::{InviteDto, MergeSuggestionDto, PersonDto, PersonRefDto};
use leptos::prelude::*;
use serde_json::json;
use ui::{
    Alert, AlertVariant, Avatar, Button, ButtonVariant, CopyField, Dialog, FormField, Input, Label,
    Select, SelectOption, Sheet, SheetSide, Textarea,
};

use crate::admin::ConfirmDialog;
use crate::i18n::{t, use_i18n};
use crate::rest;
use crate::sharing::share_error;

/// Two entries to merge: `remove` is folded into `keep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergePair {
    pub keep: String,
    pub remove: String,
}

impl From<&MergeSuggestionDto> for MergePair {
    fn from(s: &MergeSuggestionDto) -> Self {
        Self {
            keep: s.keep_id.clone(),
            remove: s.remove_id.clone(),
        }
    }
}

impl MergePair {
    /// Fold `p` into another person (the first that is not `p`; chosen in the dialog).
    pub fn with(p: &PersonDto, others: &[PersonRefDto]) -> Self {
        Self {
            keep: others
                .iter()
                .find(|o| o.id != p.id)
                .map(|o| o.id.clone())
                .unwrap_or_default(),
            remove: p.id.clone(),
        }
    }
}

/// Which dialog is open and for whom; shared by the People page and a person page.
#[derive(Clone, Copy)]
pub(crate) struct Ui {
    pub target: RwSignal<Option<PersonDto>>,
    pub add: RwSignal<bool>,
    pub invite: RwSignal<bool>,
    pub link: RwSignal<bool>,
    pub unlink: RwSignal<bool>,
    pub merge: RwSignal<bool>,
    pub edit: RwSignal<bool>,
    pub remove: RwSignal<bool>,
    pub pair: RwSignal<Option<MergePair>>,
    /// Every person of the page (merge candidates, names and counts).
    pub others: RwSignal<Vec<PersonRefDto>>,
    pub outcome: RwSignal<Option<Result<String, String>>>,
    /// The last merge, for its Undo: (merged-away id, sentence).
    pub undo: RwSignal<Option<(String, String)>>,
    pub invite_link: RwSignal<Option<Result<String, String>>>,
    pub refresh: Callback<()>,
    /// Go to `/people` after a delete (the person page).
    pub leave_on_delete: bool,
    create_invite: Action<String, ()>,
}

impl Ui {
    pub fn new(refresh: Callback<()>) -> Self {
        let invite_link = RwSignal::new(None::<Result<String, String>>);
        let create_invite = Action::new(move |id: &String| {
            let path = format!("/companions/{id}/invite");
            async move {
                let r = rest::call::<InviteDto>("POST", &path, Some(json!({}))).await;
                invite_link.set(Some(match r {
                    Ok(Some(i)) => Ok(crate::sharing::absolute(&i.path)),
                    Ok(None) => Err(t("error-generic")),
                    Err(e) => Err(share_error(&e)),
                }));
            }
        });
        Self {
            target: RwSignal::new(None),
            add: RwSignal::new(false),
            invite: RwSignal::new(false),
            link: RwSignal::new(false),
            unlink: RwSignal::new(false),
            merge: RwSignal::new(false),
            edit: RwSignal::new(false),
            remove: RwSignal::new(false),
            pair: RwSignal::new(None),
            others: RwSignal::new(Vec::new()),
            outcome: RwSignal::new(None),
            undo: RwSignal::new(None),
            invite_link,
            refresh,
            leave_on_delete: false,
            create_invite,
        }
    }

    /// Open `dialog` for `p`.
    pub fn open(self, p: &PersonDto, dialog: RwSignal<bool>) {
        self.target.set(Some(p.clone()));
        dialog.set(true);
    }

    /// Open the invite dialog and make its single-use link.
    pub fn open_invite(self, p: &PersonDto) {
        self.invite_link.set(None);
        self.open(p, self.invite);
        self.create_invite.dispatch(p.id.clone());
    }

    pub fn open_merge(self, pair: MergePair) {
        self.pair.set(Some(pair));
        self.merge.set(true);
    }

    /// A change's outcome on the page (the error in words), then a refetch.
    pub fn report(self, r: Result<String, ServerFnError>) {
        self.outcome.set(Some(r.map_err(|e| share_error(&e))));
        self.refresh.run(());
    }

    fn name_of(self, id: &str) -> (String, u32) {
        self.others.with_untracked(|o| {
            o.iter()
                .find(|p| p.id == id)
                .map_or_else(|| (String::new(), 0), |p| (p.name.clone(), p.together))
        })
    }
}

/// Every dialog of the page, mounted once outside the refetched data.
#[component]
pub(crate) fn PersonDialogs(ui: Ui) -> impl IntoView {
    view! {
        <AddPerson ui=ui/>
        <Invite ui=ui/>
        <LinkDialog ui=ui/>
        <Unlink ui=ui/>
        <Merge ui=ui/>
        <Edit ui=ui/>
        <Delete ui=ui/>
    }
}

/// The last merge with its Undo (DEC-017: undoable through the merge record).
#[component]
pub(crate) fn UndoMerge(ui: Ui) -> impl IntoView {
    let undo = Action::new(move |id: &String| {
        let path = format!("/companions/{id}/unmerge");
        let i18n = use_i18n();
        async move {
            let r = rest::call::<contracts::CompanionDto>("POST", &path, None).await;
            ui.undo.set(None);
            ui.report(r.map(|c| {
                let name = c.map(|c| c.name).unwrap_or_default();
                i18n.t_args("companions-unmerged", &[("name", name.into())])
            }));
        }
    });
    move || {
        ui.undo.get().map(|(id, text)| {
            view! {
                <Alert variant=AlertVariant::Success actions=move || {
                    let id = id.clone();
                    view! {
                        <Button variant=ButtonVariant::Outline on:click=move |_| { undo.dispatch(id.clone()); }>
                            {t("companions-merge-undo")}
                        </Button>
                    }
                }>{text}</Alert>
            }
        })
    }
}

#[component]
fn AddPerson(ui: Ui) -> impl IntoView {
    let i18n = use_i18n();
    let name = RwSignal::new(String::new());
    let note = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let save = Action::new(move |(): &()| {
        let (n, notes) = (name.get_untracked(), note.get_untracked());
        let i18n = use_i18n();
        async move {
            let body = json!({"name": n, "notes": notes});
            match rest::send("POST", "/companions", Some(body)).await {
                Ok(()) => {
                    ui.add.set(false);
                    name.set(String::new());
                    note.set(String::new());
                    error.set(None);
                    ui.report(Ok(i18n.t_args(
                        "companions-added",
                        &[("name", n.trim().to_string().into())],
                    )));
                }
                Err(e) => error.set(Some(share_error(&e))),
            }
        }
    });
    view! {
        <Sheet open=ui.add side=SheetSide::Adaptive title=i18n.t("companions-add-open") close_label=i18n.t("dialog-close")
            initial_focus="pp-add-name">
            <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); save.dispatch(()); }>
                <FormField id="pp-add-name" label=t("companions-new") error=Signal::derive(move || error.get())>
                    <Input id="pp-add-name" value=name autocomplete="off"/>
                </FormField>
                <FormField id="pp-add-note" label=t("companions-note") description=t("companions-note-hint")>
                    <Textarea id="pp-add-note" value=note rows=3u32/>
                </FormField>
                <Button r#type="submit" class="w-fit" loading=save.pending()>{t("companions-add")}</Button>
            </form>
        </Sheet>
    }
}

#[component]
fn Invite(ui: Ui) -> impl IntoView {
    move || {
        let i18n = use_i18n();
        let p = ui.target.get().unwrap_or_else(blank);
        let title = i18n.t_args(
            "companions-invite-title",
            &[("name", p.name.clone().into())],
        );
        let body = i18n.t_args(
            "companions-invite-consequence",
            &[
                ("name", p.name.clone().into()),
                ("count", p.together.into()),
            ],
        );
        let send = i18n.t_args("companions-invite-send", &[("name", p.name.clone().into())]);
        view! {
            <Dialog open=ui.invite title=title close_label=i18n.t("dialog-close")>
                <div class="grid gap-4">
                    <p>{body.clone()}</p>
                    {move || match ui.invite_link.get() {
                        None => view! { <p role="status" class="text-sm text-muted-foreground">{t("companions-invite-making")}</p> }.into_any(),
                        Some(Err(e)) => view! { <Alert variant=AlertVariant::Destructive role=ui::AlertRole::Alert>{e}</Alert> }.into_any(),
                        Some(Ok(url)) => view! {
                            <div class="grid gap-1">
                                <Label r#for="pp-invite-link">{t("companions-invite-link")}</Label>
                                <CopyField id="pp-invite-link" value=Signal::derive(move || url.clone())
                                    copy_label=t("settings-copy") copied_label=t("settings-copied")/>
                            </div>
                        }.into_any(),
                    }}
                    <p class="text-sm text-muted-foreground">{send.clone()}</p>
                    <crate::sharing::Privacy/>
                    <Button variant=ButtonVariant::Outline class="w-fit" on:click=move |_| ui.invite.set(false)>{t("dialog-close")}</Button>
                </div>
            </Dialog>
        }
    }
}

/// An empty person, for dialogs rendered before anyone was picked.
fn blank() -> PersonDto {
    PersonDto {
        id: String::new(),
        name: String::new(),
        notes: None,
        link_state: "none".into(),
        linked_username: None,
        together: 0,
        plays: 0,
        wins: 0,
        decided: 0,
        last: None,
        most_played: None,
    }
}

#[component]
fn LinkDialog(ui: Ui) -> impl IntoView {
    let username = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    let send = Action::new(move |(): &()| {
        let id = ui.target.get_untracked().map(|p| p.id).unwrap_or_default();
        let u = username.get_untracked();
        async move {
            let path = format!("/companions/{id}/link-requests");
            match rest::send("POST", &path, Some(json!({"username": u}))).await {
                Ok(()) => {
                    ui.link.set(false);
                    username.set(String::new());
                    error.set(None);
                    ui.report(Ok(t("companions-link-sent")));
                }
                Err(e) => error.set(Some(share_error(&e))),
            }
        }
    });
    move || {
        let i18n = use_i18n();
        let p = ui.target.get().unwrap_or_else(blank);
        let title = i18n.t_args("companions-link-title", &[("name", p.name.into())]);
        view! {
            <Dialog open=ui.link title=title description=i18n.t("companions-link-lead") close_label=i18n.t("dialog-close")
                initial_focus="pp-link-username">
                <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); send.dispatch(()); }>
                    <FormField id="pp-link-username" label=t("companions-link-username") error=Signal::derive(move || error.get())>
                        <Input id="pp-link-username" value=username autocomplete="off"/>
                    </FormField>
                    <div class="flex flex-wrap justify-end gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| ui.link.set(false)>{t("reauth-cancel")}</Button>
                        <Button r#type="submit" loading=send.pending()>{t("companions-link-submit")}</Button>
                    </div>
                </form>
            </Dialog>
        }
    }
}

#[component]
fn Unlink(ui: Ui) -> impl IntoView {
    let i18n = use_i18n();
    let name = move || ui.target.get().map(|p| p.name).unwrap_or_default();
    let (i1, i2) = (i18n.clone(), i18n.clone());
    let title =
        Signal::derive(move || i1.t_args("companions-unlink-title", &[("name", name().into())]));
    let body =
        Signal::derive(move || i2.t_args("companions-unlink-body", &[("name", name().into())]));
    let run = Action::new(move |(): &()| {
        let p = ui.target.get_untracked().unwrap_or_else(blank);
        let i18n = use_i18n();
        async move {
            let r = rest::send("POST", &format!("/companions/{}/unlink", p.id), None).await;
            ui.report(r.map(|()| i18n.t_args("companions-unlinked", &[("name", p.name.into())])));
        }
    });
    view! {
        <ConfirmDialog open=ui.unlink title=title body=body confirm=t("companions-unlink")
            on_confirm=Callback::new(move |()| { run.dispatch(()); })/>
    }
}

#[component]
fn Delete(ui: Ui) -> impl IntoView {
    let i18n = use_i18n();
    let p = move || ui.target.get().unwrap_or_else(blank);
    let i1 = i18n.clone();
    let title =
        Signal::derive(move || i1.t_args("companions-delete-title", &[("name", p().name.into())]));
    let body = Signal::derive(move || {
        let p = p();
        let mut text = if p.together > 0 {
            i18n.t_args(
                "companions-delete-history",
                &[
                    ("name", p.name.clone().into()),
                    ("count", p.together.into()),
                ],
            )
        } else {
            i18n.t_args("companions-delete-none", &[("name", p.name.clone().into())])
        };
        if let Some(u) = p.linked_username {
            text.push(' ');
            text.push_str(&i18n.t_args("companions-delete-link", &[("username", u.into())]));
        }
        text
    });
    let run = Action::new(move |(): &()| {
        let p = ui.target.get_untracked().unwrap_or_else(blank);
        let i18n = use_i18n();
        async move {
            let r = rest::send("DELETE", &format!("/companions/{}", p.id), None).await;
            if r.is_ok() && ui.leave_on_delete {
                crate::pages::go("/people");
                return;
            }
            ui.report(r.map(|()| i18n.t_args("companions-deleted", &[("name", p.name.into())])));
        }
    });
    view! {
        <ConfirmDialog open=ui.remove title=title body=body confirm=t("companions-delete")
            on_confirm=Callback::new(move |()| { run.dispatch(()); })/>
    }
}

#[component]
fn Edit(ui: Ui) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let note = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if ui.edit.get()
            && let Some(p) = ui.target.get_untracked()
        {
            name.set(p.name);
            note.set(p.notes.unwrap_or_default());
            error.set(None);
        }
    });
    let save = Action::new(move |(): &()| {
        let id = ui.target.get_untracked().map(|p| p.id).unwrap_or_default();
        let body = json!({"name": name.get_untracked(), "notes": note.get_untracked()});
        async move {
            match rest::send("PATCH", &format!("/companions/{id}"), Some(body)).await {
                Ok(()) => {
                    ui.edit.set(false);
                    ui.report(Ok(t("companions-saved")));
                }
                Err(e) => error.set(Some(share_error(&e))),
            }
        }
    });
    move || {
        let i18n = use_i18n();
        let p = ui.target.get().unwrap_or_else(blank);
        let title = i18n.t_args("companions-edit-title", &[("name", p.name.into())]);
        view! {
            <Dialog open=ui.edit title=title close_label=i18n.t("dialog-close") initial_focus="pp-edit-name">
                <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); save.dispatch(()); }>
                    <FormField id="pp-edit-name" label=t("companions-new") error=Signal::derive(move || error.get())>
                        <Input id="pp-edit-name" value=name autocomplete="off"/>
                    </FormField>
                    <FormField id="pp-edit-note" label=t("companions-note") description=t("companions-note-hint")>
                        <Textarea id="pp-edit-note" value=note rows=3u32/>
                    </FormField>
                    <div class="flex flex-wrap justify-end gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| ui.edit.set(false)>{t("reauth-cancel")}</Button>
                        <Button r#type="submit" loading=save.pending()>{t("companions-save")}</Button>
                    </div>
                </form>
            </Dialog>
        }
    }
}

/// Merge the chosen pair (`keep` stays, the dialog's other entry folds in), then offer Undo.
fn merge_action(ui: Ui, keep: RwSignal<String>, error: RwSignal<Option<String>>) -> Action<(), ()> {
    Action::new(move |(): &()| {
        let (k, r) = (
            keep.get_untracked(),
            ui.pair
                .get_untracked()
                .map(|p| p.remove)
                .unwrap_or_default(),
        );
        let ((keep_name, _), (remove_name, _)) = (ui.name_of(&k), ui.name_of(&r));
        let i18n = use_i18n();
        async move {
            let path = format!("/companions/{k}/merge");
            match rest::send("POST", &path, Some(json!({"remove": r}))).await {
                Ok(()) => {
                    ui.merge.set(false);
                    let text = i18n.t_args(
                        "companions-merged",
                        &[("keep", keep_name.into()), ("remove", remove_name.into())],
                    );
                    ui.outcome.set(None);
                    ui.undo.set(Some((r, text)));
                    ui.refresh.run(());
                }
                Err(e) => error.set(Some(share_error(&e))),
            }
        }
    })
}

/// "Keep" choices: every other person but the one that folds in.
fn keep_options(ui: Ui, remove: &str) -> Vec<SelectOption> {
    std::iter::once(SelectOption {
        value: String::new(),
        label: t("companions-choose"),
    })
    .chain(
        ui.others
            .get()
            .into_iter()
            .filter(|o| o.id != remove)
            .map(|o| SelectOption {
                value: o.id,
                label: o.name,
            }),
    )
    .collect()
}

#[component]
fn Merge(ui: Ui) -> impl IntoView {
    let keep = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if let Some(p) = ui.pair.get() {
            keep.set(p.keep);
            error.set(None);
        }
    });
    let remove = move || ui.pair.get().map(|p| p.remove).unwrap_or_default();
    let swap = move |_| {
        let (k, r) = (keep.get_untracked(), remove());
        if !k.is_empty() {
            ui.pair.set(Some(MergePair { keep: r, remove: k }));
        }
    };
    let run = merge_action(ui, keep, error);
    move || {
        let i18n = use_i18n();
        let r = remove();
        let (rname, rcount) = ui.name_of(&r);
        let options = Signal::derive(move || keep_options(ui, &remove()));
        let consequence = {
            let i18n = i18n.clone();
            let rname = rname.clone();
            move || {
                let (kname, _) = ui.name_of(&keep.get());
                i18n.t_args(
                    "companions-merge-consequence",
                    &[
                        ("keep", kname.into()),
                        ("remove", rname.clone().into()),
                        ("count", rcount.into()),
                    ],
                )
            }
        };
        let fold = i18n.t_args("companions-merge-fold-count", &[("count", rcount.into())]);
        view! {
            <Dialog open=ui.merge title=i18n.t("companions-merge-title") close_label=i18n.t("dialog-close")>
                <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); run.dispatch(()); }>
                    <FormField id="pp-merge-keep" label=t("companions-merge-keep")>
                        <Select id="pp-merge-keep" value=keep options=options/>
                    </FormField>
                    <div class="grid gap-1">
                        <span class="text-sm font-medium">{t("companions-merge-remove")}</span>
                        <div class="flex items-center gap-3">
                            <Avatar name=rname.clone() decorative=true/>
                            <span class="font-medium" data-testid="pp-merge-fold">{rname.clone()}</span>
                            <span class="text-sm text-muted-foreground">{fold.clone()}</span>
                        </div>
                    </div>
                    <Button variant=ButtonVariant::Ghost class="w-fit" on:click=swap>{t("companions-merge-swap")}</Button>
                    <p role="status" class="text-sm">{consequence.clone()}</p>
                    {move || error.get().map(|e| view! { <Alert variant=AlertVariant::Destructive role=ui::AlertRole::Alert>{e}</Alert> })}
                    <div class="flex flex-wrap justify-end gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| ui.merge.set(false)>{t("reauth-cancel")}</Button>
                        <Button r#type="submit" disabled=Signal::derive(move || keep.get().is_empty())>{t("companions-merge-submit")}</Button>
                    </div>
                </form>
            </Dialog>
        }
    }
}
