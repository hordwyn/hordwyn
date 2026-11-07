//! Ctrl-K search palette (D-6.6, design.md 6.4): local results first, then each connector's
//! results as its `search.results` event arrives for the current search id. The footer shows
//! the source status (dots with words: which groups have no source, which sources fail) and
//! an empty search ends in a card that says why (design.md 4.4), with "Ask admin" for
//! members and "Set up" for admins.

use contracts::admin::{SkippedSourceDto, SourceStatusPageDto};
use contracts::{CatalogItemDto, SearchResultsEventDto};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use ui::{Command, CommandItem, Dialog, Segmented, SegmentedOption, StatusDot};

use crate::admin_api;
use crate::admin_state::source_state;
use crate::events::use_events;
use crate::i18n::{t, use_i18n};
use crate::settings::AskAdmin;
use crate::source_hints::{
    FooterItem, HintAction, NoResults, SET_UP_HREF, footer_items, no_results,
};

/// Whether the palette is open (shared with the header button).
#[derive(Clone, Copy)]
pub struct PaletteOpen(pub RwSignal<bool>);

/// The palette fills the screen below 47.5rem (design.md 6.4, UX2-W13).
const FULL_SCREEN_ON_PHONES: &str = "max-[47.5rem]:inset-0 max-[47.5rem]:left-0 max-[47.5rem]:top-0 max-[47.5rem]:h-dvh max-[47.5rem]:max-w-none \
    max-[47.5rem]:translate-x-0 max-[47.5rem]:translate-y-0 max-[47.5rem]:rounded-none \
    max-[47.5rem]:content-start max-[47.5rem]:overflow-y-auto";

/// The scope segments of the palette (design.md 6.4): every kind, or one kind group.
const SCOPES: [&str; 5] = ["all", "watch", "read", "listen", "play"];

/// The `group` a search in `scope` sends (none for All).
fn scope_group(scope: &str) -> Option<String> {
    (scope != SCOPES[0]).then(|| scope.to_string())
}

fn to_item(i: &CatalogItemDto, group: &str) -> CommandItem {
    let kind = crate::i18n::kind_label(&i.kind);
    CommandItem {
        id: i.id.clone(),
        label: match i.year {
            Some(y) => format!("{} ({y})", i.title),
            None => i.title.clone(),
        },
        group: format!("{group} - {kind}"),
        keywords: i.original_title.clone().unwrap_or_default(),
    }
}

/// Streamed connector results of the current search join the list as they arrive.
fn stream_results(
    events: crate::events::LastEvent,
    search_id: RwSignal<Option<String>>,
    waiting: RwSignal<Vec<String>>,
    items: RwSignal<Vec<CommandItem>>,
) {
    Effect::new(move |_| {
        let Some((topic, data)) = events.0.get() else {
            return;
        };
        if topic != "search.results" {
            return;
        }
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(&data) else {
            return;
        };
        let Ok(r) = serde_json::from_value::<SearchResultsEventDto>(ev["payload"].clone()) else {
            return;
        };
        if search_id.get_untracked().as_deref() != Some(r.search_id.as_str()) {
            return;
        }
        waiting.update(|w| w.retain(|c| c != &r.connector));
        items.update(|list| {
            for i in &r.items {
                if !list.iter().any(|x| x.id == i.id) {
                    list.push(to_item(i, &r.connector));
                }
            }
        });
    });
}

/// The answer of the current search: what was asked, skipped and not served (the reason
/// lines of design.md 4.4).
#[derive(Debug, Clone, Default, PartialEq)]
struct Asked {
    query: String,
    pending: Vec<String>,
    skipped: Vec<SkippedSourceDto>,
    groups_without_source: Vec<String>,
}

/// The footer's source status, fetched when the palette opens (signed-in only).
fn footer_status(open: RwSignal<bool>) -> Resource<Option<SourceStatusPageDto>> {
    let signed_in = expect_context::<crate::app::ClientConfig>().signed_in;
    Resource::new(
        move || open.get() && signed_in,
        |load| async move {
            if load {
                admin_api::source_status().await.ok()
            } else {
                None
            }
        },
    )
}

/// Ctrl-K / Cmd-K anywhere opens the palette.
fn open_on_ctrl_k(open: RwSignal<bool>) {
    let _ = window_event_listener(leptos::ev::keydown, move |ev| {
        if (ev.ctrl_key() || ev.meta_key()) && ev.key().eq_ignore_ascii_case("k") {
            ev.prevent_default();
            open.set(true);
        }
    });
}

/// The palette's search state.
#[derive(Clone, Copy)]
struct State {
    query: RwSignal<String>,
    scope: RwSignal<String>,
    items: RwSignal<Vec<CommandItem>>,
    search_id: RwSignal<Option<String>>,
    waiting: RwSignal<Vec<String>>,
    asked: RwSignal<Option<Asked>>,
}

/// Search 250 ms after the last keystroke or scope change; a newer search wins.
fn debounced_search(s: State) {
    let State {
        query,
        scope,
        items,
        search_id,
        waiting,
        asked,
    } = s;
    let generation = StoredValue::new(0u64);
    Effect::new(move |_| {
        let q = query.get();
        let group = scope_group(&scope.get());
        let g = generation.get_value() + 1;
        generation.set_value(g);
        if q.trim().chars().count() < 2 {
            items.set(vec![]);
            search_id.set(None);
            waiting.set(vec![]);
            asked.set(None);
            return;
        }
        set_timeout(
            move || {
                if generation.get_value() != g {
                    return;
                }
                leptos::task::spawn_local(async move {
                    if let Ok(r) = crate::api::search(q.clone(), None, group).await
                        && generation.get_value() == g
                    {
                        let local = t("search-local");
                        items.set(r.items.iter().map(|i| to_item(i, &local)).collect());
                        waiting.set(r.pending.clone());
                        asked.set(Some(Asked {
                            query: q,
                            pending: r.pending,
                            skipped: r.skipped,
                            groups_without_source: r.groups_without_source,
                        }));
                        search_id.set(Some(r.search_id));
                    }
                });
            },
            std::time::Duration::from_millis(250),
        );
    });
}

#[component]
pub fn SearchPalette() -> impl IntoView {
    let open = expect_context::<PaletteOpen>().0;
    let i18n = use_i18n();
    let query = RwSignal::new(String::new());
    let scope = RwSignal::new(SCOPES[0].to_string());
    let items = RwSignal::new(Vec::<CommandItem>::new());
    let search_id = RwSignal::new(None::<String>);
    let waiting = RwSignal::new(Vec::<String>::new());
    let asked = RwSignal::new(None::<Asked>);
    let events = use_events();
    let status = footer_status(open);
    open_on_ctrl_k(open);

    debounced_search(State {
        query,
        scope,
        items,
        search_id,
        waiting,
        asked,
    });
    stream_results(events, search_id, waiting, items);

    let navigate = use_navigate();
    let on_select = Callback::new(move |item: CommandItem| {
        open.set(false);
        query.set(String::new());
        navigate(
            &format!("/items/{}", item.id),
            leptos_router::NavigateOptions::default(),
        );
    });
    // the no-results card shows once every asked source has answered with nothing
    let empty_answer = Memo::new(move |_| {
        asked
            .get()
            .filter(|_| waiting.with(Vec::is_empty) && items.with(Vec::is_empty))
    });
    let waiting_text = {
        let i18n = i18n.clone();
        Signal::derive(move || {
            let w = waiting.get();
            if w.is_empty() {
                String::new()
            } else {
                i18n.t_args("search-waiting", &[("sources", w.join(", ").into())])
            }
        })
    };
    let empty = {
        let i18n = i18n.clone();
        Signal::derive(move || {
            if empty_answer.with(Option::is_some) {
                String::new()
            } else if asked.with(Option::is_some) {
                i18n.t("search-searching")
            } else {
                i18n.t("search-empty")
            }
        })
    };
    let (label, placeholder) = (i18n.t("search-label"), i18n.t("search-placeholder"));
    let scopes: Vec<SegmentedOption> = SCOPES
        .iter()
        .map(|s| SegmentedOption::new(*s, i18n.t(&format!("kind-group-{s}"))))
        .collect();
    view! {
        <Dialog open=open title=i18n.t("search-title") close_label=i18n.t("dialog-close") class=FULL_SCREEN_ON_PHONES>
            <Segmented id="palette-scope" label=i18n.t("search-scope") value=scope options=scopes.clone() class="mb-3"/>
            <Command
                id="search"
                label=label.clone()
                placeholder=placeholder.clone()
                empty_label=empty
                items=Signal::derive(move || items.get())
                query=query
                on_select=on_select
            />
            // the reason card is announced when it appears (UX2-W8): the live region stays put
            <div role="status">
                <Suspense fallback=|| ()>
                    {move || empty_answer.get().map(|a| view! { <NoResultsCard asked=a status=status.get().flatten()/> })}
                </Suspense>
            </div>
            <Suspense fallback=|| ()>
                {move || {
                    let shown = empty_answer.with(Option::is_some);
                    status.get().flatten().map(|s| view! { <SourceFooter status=s actions=!shown/> })
                }}
            </Suspense>
            <p class="mt-2 text-xs text-muted-foreground" aria-live="polite">{waiting_text}</p>
        </Dialog>
    }
}

/// A source's display name from the status list (its id when unknown).
fn source_name(status: Option<&SourceStatusPageDto>, id: &str) -> String {
    status
        .and_then(|s| s.sources.iter().find(|x| x.id == id))
        .map_or_else(|| id.to_string(), |x| x.name.clone())
}

#[component]
fn HintActionView(action: HintAction, #[prop(into)] name: String) -> impl IntoView {
    match action {
        HintAction::SetUp => view! {
            <a class="text-sm font-medium underline" href=SET_UP_HREF>{t("settings-sources-set-up")}</a>
        }
        .into_any(),
        HintAction::AskGroup(g) => view! { <AskAdmin source=None group=Some(g) name=name/> }.into_any(),
    }
}

/// Why the search shows nothing (design.md 5.3, 6.4): never an empty list without a reason.
#[component]
fn NoResultsCard(asked: Asked, status: Option<SourceStatusPageDto>) -> impl IntoView {
    let i18n = use_i18n();
    let admin = status.as_ref().is_some_and(|s| s.is_admin);
    let names: Vec<String> = asked
        .pending
        .iter()
        .map(|id| source_name(status.as_ref(), id))
        .collect();
    let reason = no_results(admin, &names, &asked.skipped, &asked.groups_without_source);
    let title = i18n.t_args(
        "search-none-title",
        &[("query", asked.query.clone().into())],
    );
    let searched = |list: &[String]| {
        if list.is_empty() {
            i18n.t("search-none-library")
        } else {
            i18n.t_args(
                "search-none-searched",
                &[("sources", list.join(", ").into())],
            )
        }
    };
    let body = match reason {
        NoResults::NotSetUp { action } => {
            let lead = t(if admin { "search-unset-admin" } else { "search-unset-member" });
            let group = match &action {
                Some(HintAction::AskGroup(g)) => t(&format!("kind-group-{g}")),
                _ => String::new(),
            };
            return view! {
                <section data-testid="no-results" aria-labelledby="palette-none-h" class="mt-3 grid gap-2 rounded-xl border bg-card p-4">
                    <h3 id="palette-none-h" class="font-semibold">{t("search-unset-title")}</h3>
                    <p class="text-sm text-muted-foreground">{lead}</p>
                    {action.map(|a| view! { <div><HintActionView action=a name=group/></div> })}
                </section>
            }
            .into_any();
        }
        NoResults::NothingFound { searched: list } => view! {
            <p class="text-sm text-muted-foreground">{searched(&list)} " " {t("search-none-spelling")}</p>
        }
        .into_any(),
        NoResults::Explained { searched: list, groups, skipped } => {
            let lines = groups.into_iter().map(|(g, action)| {
                let name = t(&format!("kind-group-{g}"));
                let line = i18n.t_args("search-none-group", &[("group", name.clone().into())]);
                view! {
                    <li class="flex flex-wrap items-center gap-x-3 gap-y-1">
                        <span>{line}</span>
                        <HintActionView action=action name=name/>
                    </li>
                }
            }).collect_view();
            let skipped = skipped.into_iter().map(|s| {
                let state = t(&format!("source-state-{}", s.state));
                let line = i18n.t_args("search-none-skipped", &[("source", s.name.into()), ("state", state.into())]);
                view! { <li>{line}</li> }
            }).collect_view();
            view! {
                <p class="text-sm text-muted-foreground">{searched(&list)}</p>
                <ul class="grid gap-1 text-sm">{lines}{skipped}</ul>
            }
            .into_any()
        }
    };
    view! {
        <section data-testid="no-results" aria-labelledby="palette-none-h" class="mt-3 grid gap-2 rounded-xl border bg-card p-4">
            <h3 id="palette-none-h" class="font-semibold">{title}</h3>
            {body}
        </section>
    }
    .into_any()
}

/// Source status dots with words (design.md 6.4), each row with its one action unless the
/// no-results card above already offers it.
#[component]
fn SourceFooter(status: SourceStatusPageDto, actions: bool) -> impl IntoView {
    let i18n = use_i18n();
    let rows = footer_items(&status);
    (!rows.is_empty()).then(|| {
        let rows = rows.into_iter().map(|row| match row {
            FooterItem::Source { name, state, action, .. } => {
                // a word for every state, working too (colour is never the only cue, UX2-W4)
                let word = t(&format!("source-state-{state}"));
                let label = i18n.t_args("search-footer-source", &[("source", name.clone().into()), ("state", word.into())]);
                let dot = source_state(&state);
                view! {
                    <li class="flex flex-wrap items-center gap-x-2">
                        <StatusDot state=dot label=label/>
                        {action.filter(|_| actions).map(|a| view! { <HintActionView action=a name=name/> })}
                    </li>
                }
            }
            FooterItem::Group { group, action } => {
                let name = t(&format!("kind-group-{group}"));
                let label = i18n.t_args("search-footer-group", &[("group", name.clone().into())]);
                view! {
                    <li class="flex flex-wrap items-center gap-x-2">
                        <StatusDot state=ui::StatusState::Off label=label/>
                        {action.filter(|_| actions).map(|a| view! { <HintActionView action=a name=name/> })}
                    </li>
                }
            }
        }).collect_view();
        view! {
            <footer class="mt-3 border-t pt-3">
                <ul aria-label=t("search-footer-label") class="flex flex-wrap gap-x-4 gap-y-2">{rows}</ul>
            </footer>
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_searches_every_kind_and_a_segment_its_group() {
        assert_eq!(SCOPES[0], "all", "All is the first segment");
        assert_eq!(scope_group("all"), None);
        assert_eq!(scope_group("play"), Some("play".to_string()));
    }
}
