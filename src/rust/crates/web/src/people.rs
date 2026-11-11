//! People (design.md 3.6, 6.7, 5.11; mockup `companions.html`): requests for you as cards on
//! top, tabs All / Linked accounts / Game groups (`game_groups`), person cards with initials,
//! link state and the numbers people log plays for, duplicate suggestions, and the person page
//! (`/people/:id`) with the history filtered to the person and the lists they share. The
//! dialogs (invite, link, unlink, merge, delete, "Add a person") live in `people_dialogs`.
//! Reads come from server functions, changes go to the REST routes (`web::rest`, DEC-327).

use contracts::{LinkRequestDto, MergeSuggestionDto, PeoplePageDto, PersonDto, PersonRefDto};
use leptos::prelude::*;
use ui::{
    Avatar, Badge, BadgeVariant, Button, ButtonLink, ButtonSize, ButtonVariant, CardActions,
    DropdownMenu, EmptyState, IconName, MenuItem, Tabs, TabsContent, TabsList, TabsTrigger,
};

use crate::api;
use crate::events::use_events;
use crate::i18n::{I18n, t, use_i18n};
use crate::pages::PageTitle;
use crate::people_dialogs::{MergePair, PersonDialogs, Ui};
use crate::rest;
use crate::settings::{Lead, Outcome, kept, loaded};

/// Refetch on any `entity.updated` event (people, sessions and links change elsewhere).
pub(crate) fn refetch_on_update(refetch: impl Fn() + Send + Sync + 'static) {
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && topic == "entity.updated"
        {
            refetch();
        }
    });
}

/// Win rate in whole percent, `None` before the first decided play.
#[must_use]
pub fn win_percent(wins: u32, decided: u32) -> Option<u32> {
    (decided > 0).then(|| (wins * 100 + decided / 2) / decided)
}

/// "Today", "Yesterday" or the date of a local day (`2026-03-07`).
pub(crate) fn day_words(i18n: &I18n, day: &str) -> String {
    let today = crate::history::today();
    match crate::history_state::Day::parse(day) {
        Some(d) if d == today => i18n.t("history-today"),
        Some(d) if d.days() + 1 == today.days() => i18n.t("history-yesterday"),
        Some(_) => i18n.date(day),
        None => day.to_string(),
    }
}

#[component]
pub fn People() -> impl IntoView {
    let data = Resource::new(|| (), |()| api::people_page());
    refetch_on_update(move || data.refetch());
    let ui = Ui::new(Callback::new(move |()| data.refetch()));
    provide_context(ui);
    let groups = crate::game_groups::GroupUi::new(Callback::new(move |r| ui.report(r)));
    provide_context(groups);
    view! {
        <PageTitle text=t("companions-title")/>
        <Lead text=t("companions-lead")/>
        // a grid page: the cards use the whole 96 rem column (DEC-541, review OT2D-2); the
        // requests and suggestions are lines of text and keep a readable measure
        <div class="grid gap-4">
            <Button class="w-fit" on:click=move |_| ui.add.set(true)>{t("companions-add-open")}</Button>
            <Outcome message=ui.outcome/>
            <crate::people_dialogs::UndoMerge ui=ui/>
            {loaded(data, move |page: PeoplePageDto| {
                ui.others.set(page.people.iter().map(|p| PersonRefDto { id: p.id.clone(), name: p.name.clone(), together: p.together }).collect());
                view! { <PeopleBody page=page/> }
            })}
        </div>
        <PersonDialogs ui=ui/>
        <crate::game_groups::GroupDialogs ui=groups/>
    }
}

#[component]
fn PeopleBody(page: PeoplePageDto) -> impl IntoView {
    let i18n = use_i18n();
    let tab = kept("pp-tab", || "all".to_string());
    let linked: Vec<PersonDto> = page
        .people
        .iter()
        .filter(|p| p.link_state == "linked")
        .cloned()
        .collect();
    let all_label = i18n.t_args("companions-tab-all", &[("count", page.people.len().into())]);
    let linked_label = i18n.t_args("companions-tab-linked", &[("count", linked.len().into())]);
    let groups_label = i18n.t_args(
        "companions-tab-groups",
        &[("count", page.groups.len().into())],
    );
    let groups = StoredValue::new(page.groups);
    let all = StoredValue::new(page.people);
    let linked = StoredValue::new(linked);
    view! {
        {(!page.requests.is_empty()).then(|| view! { <Requests requests=page.requests.clone()/> })}
        {(!page.suggestions.is_empty()).then(|| view! { <Suggestions list=page.suggestions.clone()/> })}
        <Tabs id="pp-tabs" active=tab>
            <TabsList label=t("companions-tabs") class="w-fit">
                <TabsTrigger value="all">{all_label}</TabsTrigger>
                <TabsTrigger value="linked">{linked_label}</TabsTrigger>
                <TabsTrigger value="groups">{groups_label}</TabsTrigger>
            </TabsList>
            <TabsContent value="all">
                {move || view! { <Cards people=all.get_value() empty=t("companions-empty")/> }}
            </TabsContent>
            <TabsContent value="linked">
                {move || view! { <Cards people=linked.get_value() empty=t("companions-empty-linked")/> }}
            </TabsContent>
            <TabsContent value="groups">
                {move || view! { <crate::game_groups::GroupsTab groups=groups.get_value()/> }}
            </TabsContent>
        </Tabs>
    }
}

/// Requests addressed to you, with the consequence spelled out (mockup annotation 1).
#[component]
fn Requests(requests: Vec<LinkRequestDto>) -> impl IntoView {
    let i18n = use_i18n();
    let ui = expect_context::<Ui>();
    let answer = Action::new(move |(id, from, accept): &(String, String, bool)| {
        let (id, from, accept) = (id.clone(), from.clone(), *accept);
        let i18n = use_i18n();
        async move {
            let verb = if accept { "accept" } else { "decline" };
            let r = rest::send("POST", &format!("/link-requests/{id}/{verb}"), None).await;
            ui.report(r.map(|()| {
                if accept {
                    i18n.t_args("companions-accepted", &[("from", from.into())])
                } else {
                    i18n.t("companions-declined")
                }
            }));
        }
    });
    let cards = requests
        .into_iter()
        .map(|r| {
            let text = i18n.t_args(
                "companions-request-text",
                &[
                    ("from", r.from_name.clone().into()),
                    ("name", r.companion_name.clone().into()),
                ],
            );
            // one name per card: the buttons of two requests must not sound alike (UX3-26)
            let from: [(&str, fluent_bundle::FluentValue<'_>); 1] = [("from", r.from_name.clone().into())];
            let accept_label = i18n.t_args("companions-accept-named", &from);
            let decline_label = i18n.t_args("companions-decline-named", &from);
            let (a, d) = (
                (r.id.clone(), r.from_name.clone(), true),
                (r.id.clone(), r.from_name.clone(), false),
            );
            view! {
                <li class="grid gap-3 rounded-xl border border-info/40 bg-info-soft p-4" data-testid="pp-request">
                    <p>{text}</p>
                    <div class="flex flex-wrap gap-2">
                        <Button size=ButtonSize::Sm attr:aria-label=accept_label on:click=move |_| { answer.dispatch(a.clone()); }>{t("companions-accept")}</Button>
                        <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=decline_label on:click=move |_| { answer.dispatch(d.clone()); }>{t("companions-decline")}</Button>
                    </div>
                </li>
            }
        })
        .collect_view();
    view! {
        <section aria-labelledby="pp-requests-h" class="grid max-w-4xl gap-2">
            <h2 id="pp-requests-h" class="text-lg font-semibold">{t("companions-requests")}</h2>
            <ul class="grid gap-2">{cards}</ul>
        </section>
    }
}

/// Duplicates by normalised name, each with one Merge button that opens the merge dialog.
#[component]
fn Suggestions(list: Vec<MergeSuggestionDto>) -> impl IntoView {
    let i18n = use_i18n();
    let ui = expect_context::<Ui>();
    let rows = list
        .into_iter()
        .map(|s| {
            let text = i18n.t_args(
                "companions-suggest",
                &[("keep", s.keep_name.clone().into()), ("remove", s.remove_name.clone().into())],
            );
            let label = i18n.t_args(
                "companions-suggest-merge",
                &[("keep", s.keep_name.clone().into()), ("remove", s.remove_name.clone().into())],
            );
            let pair = MergePair::from(&s);
            view! {
                <li class="flex flex-wrap items-center gap-3 rounded-xl border border-dashed border-border p-4" data-testid="pp-suggestion">
                    <p class="min-w-0 flex-1">{text}</p>
                    <Button size=ButtonSize::Sm variant=ButtonVariant::Outline on:click=move |_| ui.open_merge(pair.clone())>{label}</Button>
                </li>
            }
        })
        .collect_view();
    view! { <ul class="grid max-w-4xl gap-2" aria-label=t("companions-suggestions")>{rows}</ul> }
}

#[component]
fn Cards(people: Vec<PersonDto>, #[prop(into)] empty: String) -> impl IntoView {
    if people.is_empty() {
        return view! { <EmptyState title=empty icon=IconName::Users level=2/> }.into_any();
    }
    view! {
        // a third column where the cards have 80 rem or more (a wide screen, DEC-541)
        <div class="@container mt-2">
            <ul class="grid gap-3 md:grid-cols-2 @7xl:grid-cols-3" aria-label=t("companions-title")>
                {people.into_iter().map(|p| view! { <PersonCard p=p/> }).collect_view()}
            </ul>
        </div>
    }
    .into_any()
}

/// The link state as a badge: a word, never colour alone.
#[component]
pub(crate) fn LinkBadge(p: PersonDto) -> impl IntoView {
    let i18n = use_i18n();
    match (p.link_state.as_str(), p.linked_username) {
        ("linked", Some(u)) => view! {
            <Badge variant=BadgeVariant::Success>{i18n.t_args("companions-linked-to", &[("username", u.into())])}</Badge>
        }
        .into_any(),
        ("linked", None) => view! { <Badge variant=BadgeVariant::Success>{t("companions-linked")}</Badge> }.into_any(),
        ("requested", _) => view! { <Badge variant=BadgeVariant::Info>{t("companions-requested")}</Badge> }.into_any(),
        _ => view! { <Badge variant=BadgeVariant::Outline>{t("companions-no-account")}</Badge> }.into_any(),
    }
}

/// Together, plays and win rate as a description list.
#[component]
pub(crate) fn Numbers(p: PersonDto) -> impl IntoView {
    let i18n = use_i18n();
    let rate = win_percent(p.wins, p.decided).map_or_else(
        || t("companions-no-rate"),
        |n| i18n.t_args("companions-percent", &[("percent", n.into())]),
    );
    let cell = |label: String, value: String| {
        view! {
            <div class="grid">
                <dt class="text-xs text-muted-foreground">{label}</dt>
                <dd class="text-lg font-semibold">{value}</dd>
            </div>
        }
    };
    view! {
        <dl class="flex flex-wrap gap-x-6 gap-y-1">
            {cell(t("companions-together"), p.together.to_string())}
            {cell(t("companions-plays"), p.plays.to_string())}
            {cell(t("companions-win-rate"), rate)}
        </dl>
    }
}

/// "Last: Azul, Today" and "Most played: Azul, 3 plays".
pub(crate) fn last_and_most(i18n: &I18n, p: &PersonDto) -> (Option<String>, Option<String>) {
    let last = p.last.as_ref().map(|l| {
        i18n.t_args(
            "companions-last",
            &[
                ("title", crate::i18n::title_text(&l.title).into()),
                ("when", day_words(i18n, &l.local_day).into()),
            ],
        )
    });
    let most = p.most_played.as_ref().map(|m| {
        i18n.t_args(
            "companions-most-played",
            &[
                ("title", crate::i18n::title_text(&m.title).into()),
                ("plays", m.plays.into()),
            ],
        )
    });
    (last, most)
}

/// The card menu: link or unlink, merge, delete (mockup annotation 3).
pub(crate) fn menu_items(ui: Ui, p: &PersonDto) -> Vec<MenuItem> {
    let item = |label: &str, destructive: bool, run: Callback<()>| MenuItem {
        label: t(label),
        on_select: run,
        destructive,
    };
    let mut out = Vec::new();
    let (for_link, for_merge, for_edit, for_delete) = (p.clone(), p.clone(), p.clone(), p.clone());
    match p.link_state.as_str() {
        "linked" => out.push(item(
            "companions-unlink",
            false,
            Callback::new(move |()| ui.open(&for_link, ui.unlink)),
        )),
        "requested" => {}
        _ => out.push(item(
            "companions-link",
            false,
            Callback::new(move |()| ui.open(&for_link, ui.link)),
        )),
    }
    out.push(item(
        "companions-merge",
        false,
        Callback::new(move |()| {
            ui.open_merge(MergePair::with(&for_merge, &ui.others.get_untracked()));
        }),
    ));
    out.push(item(
        "companions-edit",
        false,
        Callback::new(move |()| ui.open(&for_edit, ui.edit)),
    ));
    out.push(item(
        "companions-delete",
        true,
        Callback::new(move |()| ui.open(&for_delete, ui.remove)),
    ));
    out
}

/// "Log with <person>" (DEC-353): the search palette picks the item, then its log sheet opens
/// with the person (the scoped pick, WP4-4).
fn log_with_button(p: &PersonDto) -> impl IntoView + use<> {
    let i18n = use_i18n();
    let scope = expect_context::<crate::palette::PaletteScope>().0;
    let open = expect_context::<crate::palette::PaletteOpen>().0;
    let label = i18n.t_args(
        "companions-log-with-named",
        &[("name", p.name.clone().into())],
    );
    let (id, name) = (p.id.clone(), p.name.clone());
    view! {
        <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=label
            on:click=move |_| {
                scope.set(Some(crate::palette::PaletteTarget::LogWith {
                    companion_id: id.clone(),
                    name: name.clone(),
                }));
                open.set(true);
            }>
            {t("companions-log-with")}
        </Button>
    }
}

#[component]
fn PersonCard(p: PersonDto) -> impl IntoView {
    let i18n = use_i18n();
    let ui = expect_context::<Ui>();
    let (last, most) = last_and_most(&i18n, &p);
    let history = i18n.t_args(
        "companions-history-named",
        &[("name", p.name.clone().into())],
    );
    let menu_label = i18n.t_args("companions-menu-named", &[("name", p.name.clone().into())]);
    let invite = (p.link_state == "none").then(|| {
        let label = i18n.t_args(
            "companions-invite-named",
            &[("name", p.name.clone().into())],
        );
        let target = p.clone();
        view! {
            <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=label
                on:click=move |_| ui.open_invite(&target)>
                {t("companions-invite")}
            </Button>
        }
    });
    let log_with = log_with_button(&p);
    let items = menu_items(ui, &p);
    view! {
        <li class="grid gap-3 rounded-xl border border-border bg-card p-4 shadow-elevation-1" data-testid="pp-card">
            <div class="flex items-start gap-3">
                <Avatar name=p.name.clone() decorative=true/>
                <div class="min-w-0 flex-1">
                    <h3 class="break-words font-semibold">
                        <a href=format!("/people/{}", p.id) class="underline-offset-2 hover:underline">{p.name.clone()}</a>
                    </h3>
                    <LinkBadge p=p.clone()/>
                </div>
                <DropdownMenu label=menu_label id=format!("pp-menu-{}", p.id) items=items/>
            </div>
            <Numbers p=p.clone()/>
            {last.map(|l| view! { <p class="text-sm text-muted-foreground">{l}</p> })}
            {most.map(|m| view! { <p class="text-sm text-muted-foreground">{m}</p> })}
            // one size per card footer, the link first (OT-12)
            <CardActions>
                <ButtonLink href=format!("/history?person={}", p.id) size=ButtonSize::Sm variant=ButtonVariant::Outline
                    attr:aria-label=history>
                    {t("companions-history")}
                </ButtonLink>
                {invite}
                {log_with}
            </CardActions>
        </li>
    }
}

/// A person page (design.md 6.7): the header with the stats, the history filtered to the
/// person, the lists they share, the entries merged into this one, and the actions.
#[component]
pub fn PersonPage() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let id = move || params.with(|p| p.get("id").unwrap_or_default());
    let data = Resource::new(id, api::person_page);
    let history = Resource::new(id, |id| {
        api::activity_feed(contracts::ActivityQueryDto {
            person: Some(id),
            limit: Some(HISTORY_ROWS),
            ..contracts::ActivityQueryDto::default()
        })
    });
    let refresh = move || {
        data.refetch();
        history.refetch();
    };
    refetch_on_update(refresh);
    let mut ui = Ui::new(Callback::new(move |()| refresh()));
    ui.leave_on_delete = true;
    provide_context(ui);
    view! {
        <p class="mb-2 text-sm"><a href="/people" class="text-primary underline">{t("companions-person-back")}</a></p>
        <div class="grid max-w-4xl gap-4">
            <Outcome message=ui.outcome/>
            <crate::people_dialogs::UndoMerge ui=ui/>
            {loaded(data, move |page: contracts::PersonPageDto| {
                let mut all = page.others.clone();
                all.push(PersonRefDto { id: page.person.id.clone(), name: page.person.name.clone(), together: page.person.together });
                ui.others.set(all);
                view! { <PersonBody page=page history=history/> }
            })}
        </div>
        <PersonDialogs ui=ui/>
    }
}

/// Rows of the person's history on their page; "All history with ..." shows the rest.
const HISTORY_ROWS: u32 = 10;

#[component]
fn PersonBody(
    page: contracts::PersonPageDto,
    history: Resource<Result<contracts::ActivityPageDto, ServerFnError>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let ui = expect_context::<Ui>();
    let p = page.person;
    let (last, most) = last_and_most(&i18n, &p);
    let menu_label = i18n.t_args("companions-menu-named", &[("name", p.name.clone().into())]);
    let items = menu_items(ui, &p);
    let invite = (p.link_state == "none").then(|| {
        let target = p.clone();
        view! { <Button variant=ButtonVariant::Outline on:click=move |_| ui.open_invite(&target)>{t("companions-invite")}</Button> }
    });
    let name = p.name.clone();
    view! {
        <header class="grid gap-3 rounded-xl border border-border bg-card p-4 shadow-elevation-1">
            <div class="flex items-start gap-3">
                <Avatar name=p.name.clone() decorative=true class="size-12 text-base"/>
                <div class="min-w-0 flex-1">
                    <PageTitle text=p.name.clone()/>
                    <LinkBadge p=p.clone()/>
                </div>
                <DropdownMenu label=menu_label id="pp-page-menu" items=items/>
            </div>
            {p.notes.clone().map(|n| view! { <p class="text-sm">{n}</p> })}
            <Numbers p=p.clone()/>
            {last.map(|l| view! { <p class="text-sm text-muted-foreground">{l}</p> })}
            {most.map(|m| view! { <p class="text-sm text-muted-foreground">{m}</p> })}
            <div class="flex flex-wrap gap-2">{invite}</div>
        </header>
        <PersonHistory id=p.id.clone() name=name.clone() history=history/>
        <PersonLists name=name linked=p.link_state == "linked" lists=page.lists/>
        {(!page.merged.is_empty()).then(|| view! { <Merged merged=page.merged/> })}
    }
}

#[component]
fn PersonHistory(
    id: String,
    name: String,
    history: Resource<Result<contracts::ActivityPageDto, ServerFnError>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let heading = i18n.t_args(
        "companions-person-history",
        &[("name", name.clone().into())],
    );
    let all = i18n.t_args(
        "companions-person-all-history",
        &[("name", name.clone().into())],
    );
    let none = i18n.t_args("companions-person-no-history", &[("name", name.into())]);
    let refresh = Callback::new(move |()| history.refetch());
    view! {
        <section aria-labelledby="pp-history-h" class="grid gap-2">
            <h2 id="pp-history-h" class="text-lg font-semibold">{heading}</h2>
            {loaded(history, move |page: contracts::ActivityPageDto| {
                if page.rows.is_empty() {
                    return view! { <p class="text-sm text-muted-foreground">{none.clone()}</p> }.into_any();
                }
                view! {
                    <ul class="grid gap-2">
                        {page.rows.into_iter().map(|row| view! { <crate::history::ActivityRow row=row dated=true on_change=refresh/> }).collect_view()}
                    </ul>
                }.into_any()
            })}
            <p><a href=format!("/history?person={id}") class="text-sm text-primary underline">{all}</a></p>
        </section>
    }
}

#[component]
fn PersonLists(name: String, linked: bool, lists: Vec<contracts::ListCardDto>) -> impl IntoView {
    let i18n = use_i18n();
    let heading = i18n.t_args("companions-person-lists", &[("name", name.clone().into())]);
    let empty = if linked {
        i18n.t_args("companions-person-no-lists", &[("name", name.into())])
    } else {
        i18n.t_args("companions-person-lists-unlinked", &[("name", name.into())])
    };
    view! {
        <section aria-labelledby="pp-lists-h" class="grid gap-2">
            <h2 id="pp-lists-h" class="text-lg font-semibold">{heading}</h2>
            {if lists.is_empty() {
                view! { <p class="text-sm text-muted-foreground">{empty}</p> }.into_any()
            } else {
                view! { <crate::lists::ListCards lists=lists/> }.into_any()
            }}
        </section>
    }
}

#[component]
fn Merged(merged: Vec<contracts::MergedDto>) -> impl IntoView {
    let i18n = use_i18n();
    let ui = expect_context::<Ui>();
    let undo = Action::new(move |id: &String| {
        let path = format!("/companions/{id}/unmerge");
        let i18n = use_i18n();
        async move {
            let r = rest::call::<contracts::CompanionDto>("POST", &path, None).await;
            ui.report(r.map(|c| {
                let name = c.map(|c| c.name).unwrap_or_default();
                i18n.t_args("companions-unmerged", &[("name", name.into())])
            }));
        }
    });
    let rows = merged
        .into_iter()
        .map(|m| {
            let text = i18n.t_args(
                "companions-person-merged-row",
                &[("name", m.name.clone().into()), ("when", i18n.date(&m.at).into())],
            );
            let label = i18n.t_args("companions-merge-undo-named", &[("name", m.name.into())]);
            let id = m.id;
            view! {
                <li class="flex flex-wrap items-center gap-3">
                    <span class="flex-1">{text}</span>
                    <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=label
                        on:click=move |_| { undo.dispatch(id.clone()); }>{t("companions-merge-undo")}</Button>
                </li>
            }
        })
        .collect_view();
    view! {
        <section aria-labelledby="pp-merged-h" class="grid gap-2">
            <h2 id="pp-merged-h" class="text-lg font-semibold">{t("companions-person-merged")}</h2>
            <ul class="grid gap-2">{rows}</ul>
        </section>
    }
}
