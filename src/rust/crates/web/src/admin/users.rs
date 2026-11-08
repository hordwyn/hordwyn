//! Admin > Users (design.md 6.13, addendum-1 3.3, 3.4): the open invites behind one
//! disclosure above the table, and the users with role, disable and delete; a role a
//! provider manages is badged "via group" and not offered for change (DEC-313).
//! Lock-out guards are the server's (`last_admin`, `self_*`); their sentences show inline
//! in the row.

use std::collections::BTreeMap;

use contracts::admin::{AdminInviteCreatedDto, AdminInviteDto, AdminUserDto, SettingDto};
use leptos::prelude::*;
use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, CopyField, Disclosure, DisclosureMode,
    FormField, Input, Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
    disclosure_storage_key, field_described_by,
};

use super::{AdminFrame, ConfirmDialog, SettingNote};
use crate::admin_api;
use crate::admin_state::{Origin, origin};
use crate::i18n::{t, use_i18n};
use crate::reauth::{gated, gated_settings, use_reauth};
use crate::settings::{Outcome, loaded};

type Message = RwSignal<Option<Result<String, String>>>;

#[component]
pub fn Users() -> impl IntoView {
    view! {
        <AdminFrame title=t("admin-nav-users") lead=t("admin-users-lead")>
            <Invites/>
            <UserTable/>
        </AdminFrame>
    }
}

const MODES: [(&str, &str, &str); 3] = [
    ("open", "admin-signup-open", "admin-signup-open-hint"),
    ("invite", "admin-signup-invite", "admin-signup-invite-hint"),
    ("closed", "admin-signup-closed", "admin-signup-closed-hint"),
];

/// The sign-up mode with a sentence per option (shown on Admin > Instance since phase 3:
/// one place per setting, addendum-1 3.4).
#[component]
pub(super) fn SignupForm(s: SettingDto) -> impl IntoView {
    let reauth = use_reauth();
    let fixed = origin(&s) != Origin::Editable;
    let value = RwSignal::new(s.value.as_str().unwrap_or("invite").to_string());
    let busy = RwSignal::new(false);
    let message: Message = RwSignal::new(None);
    let i18n = use_i18n();
    let save = move || {
        let v = value.get_untracked();
        let saved = i18n.t("admin-saved");
        let values = BTreeMap::from([("auth.signup".to_string(), serde_json::Value::from(v))]);
        gated_settings(
            reauth,
            busy,
            values,
            Callback::new(move |r: Result<_, String>| message.set(Some(r.map(|_| saved.clone())))),
        );
    };
    view! {
        <form on:submit=move |ev| { ev.prevent_default(); save(); }>
            <fieldset class="grid gap-3">
                <legend class="mb-2 text-lg font-semibold">{t("admin-users-signup")}</legend>
                {MODES.iter().map(|(mode, label, hint)| {
                    let id = format!("adm-signup-{mode}");
                    let hint_id = format!("{id}-hint");
                    view! {
                        <div class="flex items-start gap-3">
                            <input type="radio" name="signup" id=id.clone() value=*mode disabled=fixed
                                aria-describedby=hint_id.clone() class="mt-1 size-4 accent-primary"
                                prop:checked=move || value.get() == *mode
                                on:change=move |_| value.set((*mode).to_string())/>
                            <span class="grid gap-0.5">
                                <label for=id class="text-sm font-medium">{t(label)}</label>
                                <span id=hint_id class="text-sm text-muted-foreground">{t(hint)}</span>
                            </span>
                        </div>
                    }
                }).collect_view()}
            </fieldset>
            <div class="mt-3 grid gap-3">
                <SettingNote s=s.clone()/>
                <Outcome message=message/>
                {(!fixed).then(|| view! { <Button r#type="submit" class="w-fit" loading=busy>{t("admin-save")}</Button> })}
            </div>
        </form>
    }
}

// ---------------------------------------------------------------- invites

#[component]
fn Invites() -> impl IntoView {
    let data = Resource::new(|| (), |()| admin_api::admin_invites());
    let open = RwSignal::new(false);
    let created = RwSignal::new(None::<AdminInviteCreatedDto>);
    view! {
        <div class="mb-8 max-w-3xl">
            {loaded(data, move |list| {
                let count = list.iter().filter(|i| i.state == "open").count();
                let title = use_i18n().t_args("admin-invites-title", &[("count", count.into())]);
                view! {
                    <Disclosure id="adm-invites" title=title summary=t("admin-invites-summary") mode=DisclosureMode::Button
                        level=2 open=open storage_key=disclosure_storage_key("users", "invites")>
                        <NewInvite created=created data=data/>
                        <ul class="mt-4 grid gap-2" aria-label=t("admin-invites-list")>
                            {list.into_iter().map(|i| view! { <InviteRow i=i data=data/> }).collect_view()}
                        </ul>
                    </Disclosure>
                }
            })}
        </div>
    }
}

#[component]
fn NewInvite(
    created: RwSignal<Option<AdminInviteCreatedDto>>,
    data: Resource<Result<Vec<AdminInviteDto>, ServerFnError>>,
) -> impl IntoView {
    let reauth = use_reauth();
    let email = RwSignal::new(String::new());
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let message: Message = RwSignal::new(None);
    let create = move || {
        let (e, n) = (email.get_untracked(), note.get_untracked());
        let call = move || {
            let some = |s: &String| (!s.trim().is_empty()).then(|| s.trim().to_string());
            admin_api::create_admin_invite(some(&e), some(&n))
        };
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<AdminInviteCreatedDto, String>| match r {
                Ok(c) => {
                    email.set(String::new());
                    note.set(String::new());
                    message.set(None);
                    created.set(Some(c));
                    data.refetch();
                }
                Err(e) => message.set(Some(Err(e))),
            }),
        );
    };
    view! {
        <form class="grid gap-3 sm:grid-cols-2" on:submit=move |ev| { ev.prevent_default(); create(); }>
            <FormField id="adm-invite-email" label=t("admin-invite-email")>
                <Input id="adm-invite-email" r#type="email" value=email autocomplete="off" described_by=field_described_by("adm-invite-email")/>
            </FormField>
            <FormField id="adm-invite-note" label=t("admin-invite-note")>
                <Input id="adm-invite-note" value=note described_by=field_described_by("adm-invite-note")/>
            </FormField>
            <div class="sm:col-span-2 grid gap-3">
                <Outcome message=message/>
                <Button r#type="submit" class="w-fit" loading=busy>{t("admin-invite-create")}</Button>
            </div>
        </form>
        {move || created.get().map(|c| view! {
            <div class="mt-4">
                <FormField id="adm-invite-link" label=t("admin-invite-link")>
                    <CopyField id="adm-invite-link" value=Signal::stored(c.url.clone()) copy_label=t("settings-copy")
                        copied_label=t("settings-copied") shown_once=t("admin-invite-once")/>
                </FormField>
            </div>
        })}
    }
}

fn invite_state(state: &str) -> (String, BadgeVariant) {
    match state {
        "open" => (t("admin-invite-state-open"), BadgeVariant::Info),
        "used" => (t("admin-invite-state-used"), BadgeVariant::Success),
        "revoked" => (t("admin-invite-state-revoked"), BadgeVariant::Secondary),
        _ => (t("admin-invite-state-expired"), BadgeVariant::Secondary),
    }
}

#[component]
fn InviteRow(
    i: AdminInviteDto,
    data: Resource<Result<Vec<AdminInviteDto>, ServerFnError>>,
) -> impl IntoView {
    let reauth = use_reauth();
    let i18n = use_i18n();
    let busy = RwSignal::new(false);
    let message: Message = RwSignal::new(None);
    let who = i
        .email
        .clone()
        .or_else(|| i.note.clone())
        .unwrap_or_else(|| i18n.t("admin-invite-anyone"));
    let line = i18n.t_args(
        "admin-invite-line",
        &[
            ("created", i18n.date(&i.created_at).into()),
            ("expires", i18n.date(&i.expires_at).into()),
        ],
    );
    let (state, variant) = invite_state(&i.state);
    let label = i18n.t_args("admin-invite-revoke-named", &[("name", who.clone().into())]);
    let id = i.id.clone();
    let revoke = move |_| {
        let id = id.clone();
        let call = move || admin_api::revoke_admin_invite(id.clone());
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<AdminInviteDto, String>| match r {
                Ok(_) => data.refetch(),
                Err(e) => message.set(Some(Err(e))),
            }),
        );
    };
    view! {
        <li class="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-md border px-3 py-2 text-sm" data-testid="invite-row">
            <span class="font-medium">{who}</span>
            <Badge variant=variant>{state}</Badge>
            <span class="flex-1 text-muted-foreground">{line}</span>
            {(i.state == "open").then(|| view! {
                <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm attr:aria-label=label loading=busy on:click=revoke>
                    {t("admin-invite-revoke")}
                </Button>
            })}
            <div class="basis-full empty:hidden"><Outcome message=message/></div>
        </li>
    }
}

// ---------------------------------------------------------------- users

#[component]
fn UserTable() -> impl IntoView {
    let data = Resource::new(|| (), |()| admin_api::admin_users());
    let confirm = RwSignal::new(false);
    let target = RwSignal::new(None::<AdminUserDto>);
    let row_error = RwSignal::new(None::<(String, String)>);
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let i18n = use_i18n();
    let i18n_title = i18n.clone();
    let title = Signal::derive(move || {
        let name = target.get().map(|u| u.display_name).unwrap_or_default();
        i18n_title.t_args("admin-users-delete-title", &[("name", name.into())])
    });
    let delete = Callback::new(move |()| {
        let Some(u) = target.get_untracked() else {
            return;
        };
        let id = u.id.clone();
        let call = move || admin_api::delete_user(id.clone());
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<(), String>| match r {
                Ok(()) => data.refetch(),
                Err(e) => row_error.set(Some((u.id.clone(), e))),
            }),
        );
    });
    view! {
        <section aria-labelledby="adm-users-h" class="max-w-4xl">
            <h2 id="adm-users-h" class="mb-3 text-lg font-semibold">{t("admin-users-people")}</h2>
            {loaded(data, move |list| {
                let admins = list.iter().filter(|u| u.role == "admin" && !u.disabled).count();
                view! {
                    <Table>
                        <caption class="sr-only">{t("admin-users-people")}</caption>
                        <TableHeader>
                            <TableRow>
                                <TableHead>{t("admin-users-person")}</TableHead>
                                <TableHead>{t("admin-users-role")}</TableHead>
                                <TableHead>{t("admin-users-signs-in")}</TableHead>
                                <TableHead>{t("admin-users-last-seen")}</TableHead>
                                <TableHead>{t("admin-users-actions")}</TableHead>
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {list.into_iter().map(|u| view! {
                                <UserRow u=u only_admin=admins == 1 data=data row_error=row_error
                                    on_delete=Callback::new(move |u: AdminUserDto| { target.set(Some(u)); confirm.set(true); })/>
                            }).collect_view()}
                        </TableBody>
                    </Table>
                }
            })}
            <ConfirmDialog open=confirm title=title body=Signal::derive(move || i18n.t("admin-users-delete-body"))
                confirm=t("admin-users-delete") on_confirm=delete/>
        </section>
    }
}

fn signs_in_with(u: &AdminUserDto) -> String {
    u.signs_in_with
        .iter()
        .map(|m| {
            t(match m.as_str() {
                "password" => "admin-users-with-password",
                "totp" => "admin-users-with-totp",
                _ => "admin-users-with-oidc",
            })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[component]
fn UserRow(
    u: AdminUserDto,
    only_admin: bool,
    data: Resource<Result<Vec<AdminUserDto>, ServerFnError>>,
    row_error: RwSignal<Option<(String, String)>>,
    on_delete: Callback<AdminUserDto>,
) -> impl IntoView {
    let i18n = use_i18n();
    let role = if u.role == "admin" {
        t("admin-users-admin")
    } else {
        t("admin-users-member")
    };
    // a role from a provider's group rule (role_source = provider): badged, not editable
    let via = u.role_via_provider.clone().map(|p| {
        (
            t("admin-users-via-group"),
            i18n.t_args("admin-users-via-group-hint", &[("provider", p.into())]),
        )
    });
    let seen = u
        .last_seen_at
        .as_deref()
        .map_or_else(|| t("admin-users-never"), |s| i18n.datetime(s));
    let (display, username, is_self, disabled, signs) = (
        u.display_name.clone(),
        u.username.clone(),
        u.is_self,
        u.disabled,
        signs_in_with(&u),
    );
    let row_id = u.id.clone();
    let error = move || {
        row_error
            .get()
            .filter(|(id, _)| *id == row_id)
            .map(|(_, e)| e)
    };
    view! {
        <TableRow attr:data-testid="user-row">
            <TableCell>
                <span class="font-medium">{display}</span>
                {is_self.then(|| view! { " " <span class="text-muted-foreground">{t("admin-users-you")}</span> })}
                <span class="block font-mono text-xs text-muted-foreground">{username}</span>
            </TableCell>
            <TableCell>
                {role}
                {via.map(|(badge, hint)| view! {
                    " " <Badge variant=BadgeVariant::Info>{badge}</Badge>
                    <span class="block text-xs text-muted-foreground">{hint}</span>
                })}
                {disabled.then(|| view! { " " <Badge variant=BadgeVariant::Secondary>{t("admin-users-disabled")}</Badge> })}
            </TableCell>
            <TableCell>{signs}</TableCell>
            <TableCell class="sm:whitespace-nowrap">{seen}</TableCell>
            <TableCell>
                {if is_self {
                    let guard = if only_admin { t("admin-users-only-admin") } else { t("admin-users-self") };
                    view! { <p data-testid="self-guard" class="max-w-56 text-sm text-muted-foreground">{guard}</p> }.into_any()
                } else {
                    view! { <UserActions u=u data=data row_error=row_error on_delete=on_delete/> }.into_any()
                }}
                {move || error().map(|e| view! { <p role="alert" class="mt-1 text-sm text-destructive-text">{e}</p> })}
            </TableCell>
        </TableRow>
    }
}

/// Role, disable and delete for another person's row (re-auth gated); errors show in the
/// row (DEC-259).
#[component]
fn UserActions(
    u: AdminUserDto,
    data: Resource<Result<Vec<AdminUserDto>, ServerFnError>>,
    row_error: RwSignal<Option<(String, String)>>,
    on_delete: Callback<AdminUserDto>,
) -> impl IntoView {
    let reauth = use_reauth();
    let i18n = use_i18n();
    let busy = RwSignal::new(false);
    let id = u.id.clone();
    let run = Callback::new(move |make: fn(String) -> UserCall| {
        let (id, row) = (id.clone(), id.clone());
        let call = move || make(id.clone());
        gated(
            reauth,
            busy,
            call,
            Callback::new(move |r: Result<AdminUserDto, String>| match r {
                Ok(_) => {
                    row_error.set(None);
                    data.refetch();
                }
                Err(e) => row_error.set(Some((row.clone(), e))),
            }),
        );
    });
    let (is_admin, disabled) = (u.role == "admin", u.disabled);
    let managed = u.role_via_provider.is_some();
    let named = |key: &str| i18n.t_args(key, &[("name", u.display_name.clone().into())]);
    let role_label = named(if is_admin {
        "admin-users-demote-named"
    } else {
        "admin-users-promote-named"
    });
    let disable_label = named(if disabled {
        "admin-users-enable-named"
    } else {
        "admin-users-disable-named"
    });
    let delete_label = named("admin-users-delete-named");
    view! {
        <div class="flex flex-wrap gap-1">
            {(!managed).then(|| view! {
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=role_label loading=busy
                    on:click=move |_| run.run(if is_admin { demote } else { promote })>
                    {if is_admin { t("admin-users-demote") } else { t("admin-users-promote") }}
                </Button>
            })}
            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm attr:aria-label=disable_label loading=busy
                on:click=move |_| run.run(if disabled { enable } else { disable })>
                {if disabled { t("admin-users-enable") } else { t("admin-users-disable") }}
            </Button>
            <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm attr:aria-label=delete_label
                on:click=move |_| on_delete.run(u.clone())>
                {t("admin-users-delete")}
            </Button>
        </div>
    }
}

type UserCall =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<AdminUserDto, ServerFnError>>>>;

fn promote(id: String) -> UserCall {
    Box::pin(admin_api::set_user_role(id, "admin".into()))
}

fn demote(id: String) -> UserCall {
    Box::pin(admin_api::set_user_role(id, "member".into()))
}

fn disable(id: String) -> UserCall {
    Box::pin(admin_api::set_user_disabled(id, true))
}

fn enable(id: String) -> UserCall {
    Box::pin(admin_api::set_user_disabled(id, false))
}
