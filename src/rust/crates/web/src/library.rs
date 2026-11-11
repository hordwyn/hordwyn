//! The Library page (design.md 6.2, addendum-1 1.3): group switcher, kind chips and status
//! tabs with counts, one Filters button (tags and the kind's facets), a filter box, sort and
//! view, the grid or list, and "Load more". The whole state lives in the URL
//! ([`LibraryState`]); every control navigates, so the back button undoes a change.

use contracts::{KindsDto, LibraryCountsDto, LibraryRowDto, LibraryRowsDto, TagDto};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::NavigateOptions;
use leptos_router::hooks::{use_navigate, use_query_map};
use ui::{
    Alert, AlertRole, AlertVariant, Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Chip,
    EmptyState, Icon, IconName, Input, Segmented, SegmentedMode, SegmentedOption, Select,
    SelectOption, Skeleton, Table, TableBody, TableCell, TableHead, TableHeader, TableRow,
    disclosure_storage_key,
};

use crate::api;
use crate::bulk::{BulkBar, ListPick, ListPicker, Selection};
use crate::events::use_events;
use crate::i18n::{kind_label, t, use_i18n};
use crate::library_state::{
    Filter, LibraryState, PAGE_SIZE, SORTS, View, kind_chips, nearest_status, sort_label_key,
    status_tabs,
};
use crate::next_step::{NextStepButton, progress_text};
use crate::pages::{bad_query, error_text, fresh, go, needs_sign_in};
use crate::widgets::{PosterCard, QuickMenu, TagBadges, TagEditor};

type Res<T> = Resource<Result<T, ServerFnError>>;

/// A count in the locale's digits.
#[allow(clippy::cast_precision_loss)] // counts stay far below 2^52
fn count(i: &crate::i18n::I18n, n: u64) -> String {
    i.number(n as f64)
}

/// Everything the parts of the page share.
#[derive(Clone, Copy)]
struct Lib {
    state: Memo<LibraryState>,
    /// Navigate to another state (pushes a history entry).
    go: Callback<LibraryState>,
    /// The same, replacing the current entry (typing in the filter box).
    replace: Callback<LibraryState>,
    kinds: Res<KindsDto>,
    counts: Res<LibraryCountsDto>,
    tags: Res<Vec<TagDto>>,
    rows: Res<LibraryRowsDto>,
    /// Refetch counts, tags and rows after a change on a card.
    refresh: Callback<()>,
    edit_tags: Callback<LibraryRowDto>,
}

impl Lib {
    fn kinds_now(&self) -> Option<KindsDto> {
        self.kinds.get().and_then(Result::ok)
    }

    fn counts_now(&self) -> Option<LibraryCountsDto> {
        self.counts.get().and_then(Result::ok)
    }

    /// Statuses the kind allows (quick menu).
    fn statuses_of(&self, kind: &str) -> Vec<String> {
        self.kinds
            .get_untracked()
            .and_then(Result::ok)
            .and_then(|k| k.kinds.into_iter().find(|d| d.id == kind))
            .map(|d| d.statuses)
            .unwrap_or_default()
    }
}

/// `localStorage` key of the remembered group (design.md 6.2: remembered per user; per
/// browser here, DEC-223).
#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
const GROUP_KEY: &str = "app-library-group";

#[cfg(feature = "hydrate")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

/// Restore the remembered group on a bare `/library` (first render; a URL with any state is
/// shown as it is), then remember every later choice.
fn remember_group(state: Memo<LibraryState>, replace: Callback<LibraryState>) {
    Effect::new(move |prev: Option<()>| {
        let s = state.get();
        #[cfg(feature = "hydrate")]
        if let Some(store) = storage() {
            if prev.is_none() && s == LibraryState::default() {
                let saved = store.get_item(GROUP_KEY).ok().flatten();
                if let Some(g) = saved.filter(|g| g != "all") {
                    replace.run(s.with_group(Some(g)));
                    return;
                }
            }
            let _ = store.set_item(GROUP_KEY, s.group.as_deref().unwrap_or("all"));
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (s, prev, replace);
    });
}

#[component]
pub fn Library() -> impl IntoView {
    let i18n = use_i18n();
    let query = use_query_map();
    let state = Memo::new(move |_| {
        query.with(|q| LibraryState::from_lookup(|k| q.get_all(k).unwrap_or_default()))
    });
    let nav = StoredValue::new(use_navigate());
    let to = move |s: &LibraryState, replace: bool| {
        let href = s.href();
        nav.with_value(|n| {
            n(
                &href,
                NavigateOptions {
                    replace,
                    scroll: false,
                    ..NavigateOptions::default()
                },
            );
        });
    };
    let go_cb = Callback::new(move |s: LibraryState| to(&s, false));
    let replace = Callback::new(move |s: LibraryState| to(&s, true));
    // rows shown: one block, more after "Load more" for the same state
    let shown = RwSignal::new((String::new(), PAGE_SIZE));
    let limit = Memo::new(move |_| {
        let href = state.with(LibraryState::href);
        shown.with(|(h, n)| if *h == href { *n } else { PAGE_SIZE })
    });
    let kinds = Resource::new(|| (), |()| api::kinds());
    let counts = Resource::new(|| (), |()| api::library_counts());
    let tags = Resource::new(|| (), |()| api::tags());
    let rows = Resource::new(move || (state.get(), limit.get()), |(s, n)| load_rows(s, n));
    let refresh = Callback::new(move |()| {
        counts.refetch();
        tags.refetch();
        rows.refetch();
    });
    let editing = RwSignal::new(None::<LibraryRowDto>);
    let list_pick = bulk_state(state);
    let lib = Lib {
        state,
        go: go_cb,
        replace,
        kinds,
        counts,
        tags,
        rows,
        refresh,
        edit_tags: Callback::new(move |r| editing.set(Some(r))),
    };
    remember_group(state, replace);
    let events = use_events();
    let on_event = crate::offline::refresh_when_online(refresh);
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && topic == "entity.updated"
        {
            on_event.run(());
        }
    });
    slash_focuses_the_filter();
    let known = Signal::derive(move || {
        tags.get()
            .and_then(Result::ok)
            .map(|v| v.into_iter().map(|t| t.tag).collect())
            .unwrap_or_default()
    });
    let product = expect_context::<crate::app::ClientConfig>().product_name;
    let title = i18n.t("library-title");
    let palette = expect_context::<crate::palette::PaletteOpen>().0;
    view! {
        <Title text=format!("{title} - {product}")/>
        <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
            <div class="flex items-baseline gap-3">
                <h1 class="text-2xl font-semibold tracking-tight">{title}</h1>
                <Suspense fallback=|| ()>
                    {move || counts.get().and_then(Result::ok).map(|c| view! {
                        <span class="text-sm text-muted-foreground">{use_i18n().t_args("library-count", &[("count", c.total.into())])}</span>
                    })}
                </Suspense>
            </div>
            <Button size=ButtonSize::Sm on:click=move |_| palette.set(true)>
                <Icon name=IconName::Plus/> {i18n.t("library-add")}
            </Button>
        </div>
        <Toolbar lib=lib/>
        <Suspense fallback=|| view! { <Skeleton class="mb-3 h-8 w-2/3"/> }>
            <KindChips lib=lib/>
        </Suspense>
        <Suspense fallback=|| view! { <Skeleton class="mb-3 h-9 w-full"/> }>
            <StatusTabs lib=lib/>
        </Suspense>
        <FilterBar lib=lib/>
        <Results lib=lib shown=shown limit=limit/>
        <TagEditor target=editing known=known on_change=refresh/>
        <ListPicker target=list_pick.0 on_done=refresh/>
    }
}

/// Largest block one request returns (the server clamps `limit` to it).
const MAX_BLOCK: u32 = 500;

/// The first `wanted` rows of a state: one request up to [`MAX_BLOCK`] rows, then the next
/// blocks after the previous block's cursor (keyset paging, DEC-416), so "Load more" goes on
/// past 500 rows.
async fn load_rows(s: LibraryState, wanted: u32) -> Result<LibraryRowsDto, ServerFnError> {
    let mut q = s.query_dto(wanted.min(MAX_BLOCK));
    let mut out = api::library_query(q.clone()).await?;
    while out.rows.len() < wanted as usize
        && let Some(next) = out.next.take()
    {
        let left = u32::try_from(wanted as usize - out.rows.len()).unwrap_or(MAX_BLOCK);
        q.limit = left.min(MAX_BLOCK);
        q.after = Some(next);
        let more = api::library_query(q.clone()).await?;
        out.rows.extend(more.rows);
        out.total = more.total;
        out.next = more.next;
    }
    Ok(out)
}

/// The list view's selection (rows ticked for the bulk bar; kept through refreshes, cleared
/// when the state in the URL changes) and the list picker, as context; answers the picker.
fn bulk_state(state: Memo<LibraryState>) -> ListPick {
    let selection = Selection::new();
    let list_pick = ListPick(RwSignal::new(None));
    provide_context(selection);
    provide_context(list_pick);
    // a new filter, text, sort or view starts a new selection
    Effect::new(move |prev: Option<String>| {
        let href = state.with(LibraryState::href);
        if prev.is_some_and(|p| p != href) {
            selection.ids.set(std::collections::BTreeSet::new());
        }
        href
    });
    list_pick
}

/// `/` focuses the filter box (design.md 6.2), never while typing; ends with the page.
fn slash_focuses_the_filter() {
    crate::keyboard::page_keys(|_, key| key == "/" && crate::keyboard::focus("#lib-q"));
}

/// Keep `value` (a control's own signal) and the URL state in step: the state sets the
/// control (back button), a user change navigates.
fn sync<T: Clone + PartialEq + Send + Sync + 'static>(
    value: RwSignal<T>,
    from_state: impl Fn(&LibraryState) -> T + Send + Sync + 'static,
    to_state: impl Fn(&LibraryState, T) -> LibraryState + 'static,
    lib: Lib,
) {
    let from_state = std::sync::Arc::new(from_state);
    let f = from_state.clone();
    Effect::new(move |_| {
        let v = lib.state.with(|s| f(s));
        if value.get_untracked() != v {
            value.set(v);
        }
    });
    Effect::new(move |prev: Option<()>| {
        let v = value.get();
        if prev.is_some() {
            let s = lib.state.get_untracked();
            if from_state(&s) != v {
                lib.go.run(to_state(&s, v));
            }
        }
    });
}

/// Row 1: group switcher; view toggle and sort.
#[component]
fn Toolbar(lib: Lib) -> impl IntoView {
    let i18n = use_i18n();
    let s0 = lib.state.get_untracked();
    let group = RwSignal::new(s0.group.clone().unwrap_or_else(|| "all".into()));
    sync(
        group,
        |s| s.group.clone().unwrap_or_else(|| "all".into()),
        |s, g| s.with_group(Some(g)),
        lib,
    );
    let view_value = RwSignal::new(
        if s0.view == View::List {
            "list"
        } else {
            "grid"
        }
        .to_string(),
    );
    sync(
        view_value,
        |s| if s.view == View::List { "list" } else { "grid" }.to_string(),
        |s, v| s.with_view(if v == "list" { View::List } else { View::Grid }),
        lib,
    );
    let sort = RwSignal::new(s0.sort_key().to_string());
    sync(
        sort,
        |s| s.sort_key().to_string(),
        |s, v| s.with_sort(&v),
        lib,
    );
    let sorts: Vec<SelectOption> = SORTS
        .iter()
        .map(|s| SelectOption {
            value: (*s).to_string(),
            label: i18n.t(sort_label_key(s)),
        })
        .collect();
    let views = vec![
        SegmentedOption::new("grid", i18n.t("library-view-grid")).icon(IconName::Grid),
        SegmentedOption::new("list", i18n.t("library-view-list")).icon(IconName::List),
    ];
    let reversed = move || {
        lib.state
            .with(|s| s.is_descending() != LibraryState::default_descending(s.sort_key()))
    };
    view! {
        <div class="mb-3 flex flex-wrap items-center justify-between gap-3">
            <div class="-mx-4 max-w-[calc(100%+2rem)] overflow-x-auto px-4">
                <Suspense fallback=|| view! { <Skeleton class="h-9 w-72"/> }>
                    {move || lib.kinds_now().map(|k| {
                        let i = use_i18n();
                        let mut options = vec![SegmentedOption::new("all", i.t("kind-group-all"))];
                        options.extend(k.groups.iter().map(|g| crate::tint::group_option(&g.id, i.t(&g.label_key))));
                        view! { <Segmented id="lib-group" label=i.t("library-group") value=group options=options/> }
                    })}
                </Suspense>
            </div>
            <div class="flex flex-wrap items-center gap-2">
                <Segmented id="lib-view" label=i18n.t("library-view") value=view_value options=views/>
                <label for="lib-sort" class="sr-only">{i18n.t("library-sort")}</label>
                <Select id="lib-sort" value=sort options=sorts class="w-auto"/>
                <Button variant=ButtonVariant::Outline size=ButtonSize::Icon
                    attr:aria-label=i18n.t("library-sort-reverse")
                    attr:aria-pressed=move || if reversed() { "true" } else { "false" }
                    on:click=move |_| lib.go.run(lib.state.get_untracked().reversed())>
                    <Icon name=IconName::Sort/>
                </Button>
            </div>
        </div>
    }
}

/// Row 2: the kind chips of the group with their counts ("All kinds" first).
#[component]
fn KindChips(lib: Lib) -> impl IntoView {
    let chips = Memo::new(move |_| {
        let (k, c) = (lib.kinds_now()?, lib.counts_now()?);
        Some(lib.state.with(|s| kind_chips(&k, &c, s)))
    });
    let all_count = Memo::new(move |_| {
        let (k, c) = (lib.kinds_now()?, lib.counts_now()?);
        let s = lib
            .state
            .with(LibraryState::without_kinds)
            .with_status(None);
        Some(status_tabs(&k, &c, &s).0)
    });
    move || {
        chips.get().map(|list| {
            let i18n = use_i18n();
            let all = RwSignal::new(lib.state.with_untracked(|s| s.kinds.is_empty()));
            Effect::new(move |_| all.set(lib.state.with(|s| s.kinds.is_empty())));
            let chip_views = list
                .into_iter()
                .map(|(kind, n)| {
                    let k = kind.clone();
                    let pressed = RwSignal::new(lib.state.with_untracked(|s| s.kinds.contains(&k)));
                    let k2 = kind.clone();
                    Effect::new(move |_| pressed.set(lib.state.with(|s| s.kinds.contains(&k2))));
                    let k3 = kind.clone();
                    let on_change = Callback::new(move |_| {
                        let next = lib.state.get_untracked().toggled_kind(&k3);
                        pressed.set(next.kinds.contains(&k3));
                        lib.go.run(next);
                    });
                    let icon = kind_icon(&kind);
                    view! {
                        <li><Chip pressed=pressed icon=icon count=count(&i18n, n) on_change=on_change>{kind_label(&kind)}</Chip></li>
                    }
                })
                .collect_view();
            let on_all = Callback::new(move |_| {
                all.set(true);
                let s = lib.state.get_untracked();
                if !s.kinds.is_empty() {
                    lib.go.run(s.without_kinds());
                }
            });
            let all_n = all_count.get().map(|n| count(&i18n, n));
            let (kinds_label, all_label) = (i18n.t("library-kinds"), i18n.t("library-kinds-all"));
            view! {
                <ul class="mb-3 flex flex-wrap gap-2" aria-label=kinds_label>
                    <li><Chip pressed=all count=all_n on_change=on_all>{all_label}</Chip></li>
                    {chip_views}
                </ul>
            }
        })
    }
}

/// The icon of a built-in kind (`kind-<id>` in the icon allowlist).
pub(crate) fn kind_icon(kind: &str) -> IconName {
    match kind {
        "movie" => IconName::KindMovie,
        "show" => IconName::KindShow,
        "anime" => IconName::KindAnime,
        "manga" => IconName::KindManga,
        "book" => IconName::KindBook,
        "audiobook" => IconName::KindAudiobook,
        "podcast" => IconName::KindPodcast,
        "music" => IconName::KindMusic,
        "video_game" => IconName::KindVideoGame,
        "visual_novel" => IconName::KindVisualNovel,
        "comic" => IconName::KindComic,
        "board_game" => IconName::KindBoardGame,
        _ => IconName::Library,
    }
}

/// Row 3: status tabs with counts under the group and kind selection.
#[component]
fn StatusTabs(lib: Lib) -> impl IntoView {
    let value = RwSignal::new(
        lib.state
            .with_untracked(|s| s.status.clone())
            .unwrap_or_else(|| "all".into()),
    );
    sync(
        value,
        |s| s.status.clone().unwrap_or_else(|| "all".into()),
        |s, v| s.with_status(Some(v)),
        lib,
    );
    let tabs = Memo::new(move |_| {
        let (k, c) = (lib.kinds_now()?, lib.counts_now()?);
        Some(lib.state.with(|s| status_tabs(&k, &c, s)))
    });
    move || {
        tabs.get().map(|(total, list)| {
            let i18n = use_i18n();
            let label = |text: String, n: u64| format!("{text} {}", count(&i18n, n));
            let mut options = vec![
                SegmentedOption::new("all", label(i18n.t("library-status-all"), total))
                    .controls("lib-results"),
            ];
            options.extend(list.into_iter().map(|(st, n)| {
                SegmentedOption::new(st.clone(), label(i18n.t(&format!("status-{st}")), n))
                    .controls("lib-results")
            }));
            view! {
                <div class="-mx-4 mb-3 overflow-x-auto px-4">
                    <Segmented id="lib-status" label=i18n.t("library-status") value=value options=options mode=SegmentedMode::Tabs/>
                </div>
            }
        })
    }
}

/// The filter box, the one Filters button (OD-04) with its panel, and the active filters as
/// removable badges.
#[component]
fn FilterBar(lib: Lib) -> impl IntoView {
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    remember_open(open);
    let text = RwSignal::new(lib.state.with_untracked(|s| s.q.clone()));
    debounce_text(text, lib);
    let active = move || lib.state.with(LibraryState::panel_filters);
    view! {
        <div class="mb-3 flex flex-wrap items-center gap-2">
            <form role="search" class="min-w-0 flex-1 basis-48" on:submit=move |ev| {
                ev.prevent_default();
                lib.go.run(lib.state.get_untracked().with_text(&text.get_untracked()));
            }>
                <label for="lib-q" class="sr-only">{i18n.t("library-filter")}</label>
                <Input id="lib-q" value=text r#type="search" placeholder=i18n.t("library-filter-placeholder") autocomplete="off"/>
            </form>
            <Button variant=ButtonVariant::Outline
                attr:aria-expanded=move || if open.get() { "true" } else { "false" }
                attr:aria-controls="lib-filters"
                on:click=move |_| open.update(|o| *o = !*o)>
                <Icon name=IconName::Filter/>
                {i18n.t("library-filters")}
                {move || (active() > 0).then(|| view! {
                    <Badge variant=BadgeVariant::Secondary>{use_i18n().t_args("library-filters-active", &[("count", active().into())])}</Badge>
                })}
            </Button>
        </div>
        <ActiveFilters lib=lib/>
        <div id="lib-filters" hidden=move || !open.get() class="mb-4 grid gap-4 rounded-xl border border-border bg-card p-4">
            <TagFilter lib=lib/>
            <Suspense fallback=|| ()>
                <FacetFilters lib=lib/>
            </Suspense>
        </div>
    }
}

/// The Filters panel's open state, remembered per page (OD-02, addendum-1 D6).
fn remember_open(open: RwSignal<bool>) {
    Effect::new(move |prev: Option<()>| {
        let now = open.get();
        #[cfg(feature = "hydrate")]
        if let Some(s) = storage() {
            let key = disclosure_storage_key("library", "filters");
            if prev.is_none() {
                if s.get_item(&key).ok().flatten().as_deref() == Some("1") {
                    open.set(true);
                }
            } else {
                let _ = s.set_item(&key, if now { "1" } else { "0" });
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (now, prev, disclosure_storage_key);
    });
}

/// Typing in the filter box navigates after a pause, replacing the history entry (one entry
/// per search, not per key).
fn debounce_text(text: RwSignal<String>, lib: Lib) {
    Effect::new(move |_| {
        let q = lib.state.with(|s| s.q.clone());
        if text.get_untracked().trim() != q {
            text.set(q);
        }
    });
    let pending = StoredValue::new(None::<TimeoutHandle>);
    Effect::new(move |prev: Option<()>| {
        let v = text.get();
        if prev.is_none() || lib.state.with_untracked(|s| s.q == v.trim()) {
            return;
        }
        if let Some(h) = pending.get_value() {
            h.clear();
        }
        let h = set_timeout_with_handle(
            move || lib.replace.run(lib.state.get_untracked().with_text(&v)),
            std::time::Duration::from_millis(350),
        );
        pending.set_value(h.ok());
    });
}

#[component]
fn ActiveFilters(lib: Lib) -> impl IntoView {
    move || {
        let filters = lib.state.with(LibraryState::filters);
        (!filters.is_empty()).then(|| {
            let i18n = use_i18n();
            let (active_label, clear_label) = (i18n.t("library-active"), i18n.t("library-clear"));
            view! {
                <ul class="mb-3 flex flex-wrap items-center gap-2" aria-label=active_label>
                    {filters.into_iter().map(|f| {
                        let text = filter_text(&f);
                        let remove = use_i18n().t_args("library-remove-filter", &[("filter", text.clone().into())]);
                        view! {
                            <li>
                                <Badge variant=BadgeVariant::Secondary class="gap-1 py-0 pr-0.5">
                                    {text}
                                    <button type="button" aria-label=remove
                                        class="inline-flex size-6 items-center justify-center rounded-md hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
                                        on:click=move |_| lib.go.run(lib.state.get_untracked().without(&f))>
                                        <Icon name=IconName::Close/>
                                    </button>
                                </Badge>
                            </li>
                        }
                    }).collect_view()}
                    <li>
                        <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on:click=move |_| lib.go.run(lib.state.get_untracked().cleared())>
                            {clear_label}
                        </Button>
                    </li>
                </ul>
            }
        })
    }
}

fn facet_label(attr: &str) -> String {
    let key = format!("library-facet-{attr}");
    let text = t(&key);
    if text == key {
        attr.replace('_', " ")
    } else {
        text
    }
}

fn filter_text(f: &Filter) -> String {
    let i = use_i18n();
    match f {
        Filter::Tag(tag) => i.t_args("library-filter-tag", &[("tag", tag.clone().into())]),
        Filter::Facet(a, v) => i.t_args(
            "library-filter-facet",
            &[("name", facet_label(a).into()), ("value", v.clone().into())],
        ),
        Filter::Text(q) => i.t_args("library-filter-text", &[("text", q.clone().into())]),
    }
}

#[component]
fn TagFilter(lib: Lib) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <fieldset class="grid gap-2">
            <legend class="mb-2 text-sm font-semibold">{i18n.t("library-tags")}</legend>
            <Suspense fallback=|| view! { <Skeleton class="h-8 w-1/2"/> }>
                {move || lib.tags.get().and_then(Result::ok).map(|list| {
                    if list.is_empty() {
                        return view! { <p class="text-sm text-muted-foreground">{t("library-tags-none")}</p> }.into_any();
                    }
                    let i = use_i18n();
                    view! {
                        <ul class="flex flex-wrap gap-2">
                            {list.into_iter().map(|tag| {
                                let name = tag.tag.clone();
                                let n2 = tag.tag.clone();
                                let on = move |s: &LibraryState| s.tags.iter().any(|x| x.eq_ignore_ascii_case(&n2));
                                let pressed = RwSignal::new(lib.state.with_untracked(&on));
                                let on2 = on.clone();
                                Effect::new(move |_| pressed.set(lib.state.with(&on2)));
                                let n3 = name.clone();
                                let on_change = Callback::new(move |_| {
                                    let next = lib.state.get_untracked().toggled_tag(&n3);
                                    pressed.set(on(&next));
                                    lib.go.run(next);
                                });
                                view! {
                                    <li><Chip pressed=pressed icon=IconName::Tag count=count(&i, tag.entries) on_change=on_change>{name}</Chip></li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                })}
            </Suspense>
        </fieldset>
    }
}

/// Facet values seen in the loaded rows of `kind` (strings, numbers, or lists of them).
fn facet_values(rows: &[LibraryRowDto], kind: &str, attr: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in rows.iter().filter(|r| r.item.kind == kind) {
        let v = &r.item.attributes[attr];
        let list = v.as_array().cloned().unwrap_or_else(|| vec![v.clone()]);
        for x in list {
            let s = match x {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                _ => continue,
            };
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out.sort();
    out
}

/// One text field per facet of the single selected kind, with the values seen as
/// suggestions (DEC-224); without a single kind a hint says how to get them.
#[component]
fn FacetFilters(lib: Lib) -> impl IntoView {
    move || {
        let i18n = use_i18n();
        let Some(kind) = lib.state.with(|s| s.facet_kind().map(String::from)) else {
            return view! { <p class="text-sm text-muted-foreground">{i18n.t("library-facets-pick")}</p> }.into_any();
        };
        let facets = lib
            .kinds_now()
            .and_then(|k| k.kinds.into_iter().find(|d| d.id == kind))
            .map(|d| d.facets)
            .unwrap_or_default();
        let legend = i18n.t_args("library-facets", &[("kind", kind_label(&kind).into())]);
        let rows = lib
            .rows
            .get()
            .and_then(Result::ok)
            .map(|r| r.rows)
            .unwrap_or_default();
        view! {
            <fieldset class="grid gap-3">
                <legend class="mb-2 text-sm font-semibold">{legend}</legend>
                <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                    {facets.into_iter().map(|attr| {
                        let id = format!("lib-facet-{attr}");
                        let list = format!("{id}-values");
                        let values = facet_values(&rows, &kind, &attr);
                        let a = attr.clone();
                        let value = RwSignal::new(lib.state.with_untracked(|s| {
                            s.facets.iter().find(|(x, _)| *x == a).map(|(_, v)| v.clone()).unwrap_or_default()
                        }));
                        let a2 = attr.clone();
                        view! {
                            // `change` bubbles: the handler sits on the wrapper because the
                            // `Input` view does not take it (found by the UX1-11 e2e test)
                            <div class="grid gap-1.5"
                                on:change=move |_| lib.go.run(lib.state.get_untracked().with_facet(&a2, &value.get_untracked()))>
                                <label for=id.clone() class="text-sm font-medium">{facet_label(&attr)}</label>
                                <Input id=id.clone() value=value placeholder=t("library-facet-any") attr:list=list.clone()/>
                                <datalist id=list>
                                    {values.into_iter().map(|v| view! { <option value=v></option> }).collect_view()}
                                </datalist>
                            </div>
                        }
                    }).collect_view()}
                </div>
            </fieldset>
        }.into_any()
    }
}

/// The key of the results area. A page of rows keeps one key per view mode, so a refetch (a
/// change on a card, a server event) updates the rows in place and an open quick menu stays
/// open (DEC-398); the empty, error and sign-in states render afresh each time.
fn results_key(
    r: &Result<LibraryRowsDto, ServerFnError>,
    mode: View,
    renders: StoredValue<u32>,
) -> String {
    match r {
        Ok(page) if !page.rows.is_empty() => {
            if mode == View::Grid { "grid" } else { "list" }.into()
        }
        _ => format!("x{}", fresh(renders)),
    }
}

/// The rows of a page, keyed by entry (DEC-398). Each entry owns its quick menu's open state
/// for as long as it is on the page; a refetch leaves rows with unchanged data alone and
/// renders a changed row afresh (so its handlers carry the new data, UX1-2) with that state,
/// so a background refresh does not close a menu the user has open.
fn keyed_rows<V: IntoView + 'static>(
    rows: Signal<Vec<LibraryRowDto>>,
    row_view: impl Fn(LibraryRowDto, RwSignal<bool>) -> V + Clone + Send + Sync + 'static,
) -> impl IntoView {
    let by_id = Memo::new(move |_| {
        rows.with(|v| {
            v.iter()
                .map(|r| (r.entry.id.clone(), r.clone()))
                .collect::<std::collections::HashMap<_, _>>()
        })
    });
    let ids = move || rows.with(|v| v.iter().map(|r| r.entry.id.clone()).collect::<Vec<_>>());
    view! {
        <For each=ids key=String::clone children=move |id| {
            let open = RwSignal::new(false);
            let row = Memo::new(move |_| by_id.with(|m| m.get(&id).cloned()));
            let renders = StoredValue::new(0u32);
            let row_view = row_view.clone();
            view! {
                <For each=move || row.get().map(|r| (fresh(renders), r)) key=|(k, _)| *k
                    children=move |(_, r)| row_view(r, open)/>
            }
        }/>
    }
}

/// The latest loaded page for a rows view that outlives refetches. In the browser it follows
/// the resource (keeping the last page while an error or empty result replaces the view); on
/// the server nothing refetches, so it is the first page. (Every resource read under a
/// Suspense registers a suspense task; with this second read the server panicked now and then
/// in the Transition, "tried to access a reactive value ... already disposed", which cut the
/// SSR stream. DEC-398.)
fn latest_page(rows: Res<LibraryRowsDto>, first: LibraryRowsDto) -> Memo<LibraryRowsDto> {
    #[cfg(feature = "hydrate")]
    let page = Memo::new(move |prev: Option<&LibraryRowsDto>| match rows.get() {
        Some(Ok(p)) => p,
        _ => prev.cloned().unwrap_or_else(|| first.clone()),
    });
    #[cfg(not(feature = "hydrate"))]
    let page = {
        let _ = rows;
        Memo::new(move |_| first.clone())
    };
    page
}

/// The grid or list, "Load more", and the loading, empty and error states.
#[component]
fn Results(lib: Lib, shown: RwSignal<(String, u32)>, limit: Memo<u32>) -> impl IntoView {
    let i18n = use_i18n();
    let pending = RwSignal::new(false);
    let view_mode = Memo::new(move |_| lib.state.with(|s| s.view));
    let labelled = move || {
        format!(
            "lib-status-{}",
            lib.state
                .with(|s| s.status.clone().unwrap_or_else(|| "all".into()))
        )
    };
    // UX3-25 (DEC-456): only the browser follows the pending state. On the server the
    // `set_pending` effect is the one reader of the Transition's "none pending" memo, and that
    // memo reads the task set with a non-blocking try-lock (reactive_graph 0.2.16
    // `Plain::try_new`): a resource task finishing on another worker thread at that moment
    // made it panic as "already disposed" (e2e server log, leptos 0.8.23 transition.rs:115).
    #[cfg(not(feature = "ssr"))]
    let results = view! {
        <Transition fallback=|| view! { <PosterSkeletons/> } set_pending=pending>
            <ResultRows lib=lib shown=shown limit=limit view_mode=view_mode/>
        </Transition>
    };
    #[cfg(feature = "ssr")]
    let results = view! {
        <Transition fallback=|| view! { <PosterSkeletons/> }>
            <ResultRows lib=lib shown=shown limit=limit view_mode=view_mode/>
        </Transition>
    };
    view! {
        <section id="lib-results" role="tabpanel" aria-labelledby=labelled aria-busy=move || pending.get().then_some("true")>
            <h2 class="sr-only">{i18n.t("library-results")}</h2>
            {results}
        </section>
    }
}

/// The rows of the current query (inside the results' Transition).
#[component]
fn ResultRows(
    lib: Lib,
    shown: RwSignal<(String, u32)>,
    limit: Memo<u32>,
    view_mode: Memo<View>,
) -> impl IntoView {
    let renders = StoredValue::new(0u32);
    view! {
                <For each=move || lib.rows.get().map(|r| (results_key(&r, view_mode.get(), renders), r, view_mode.get())) key=|(k, _, _)| k.clone()
                    children=move |(_, r, mode)| match r {
                    Err(e) if needs_sign_in(&e) => {
                        go("/sign-in?next=/library");
                        ().into_any()
                    }
                    Err(e) if bad_query(&e) => view! { <NoRows lib=lib/> }.into_any(),
                    Err(e) => {
                        let retry = lib.refresh;
                        view! {
                            <Alert variant=AlertVariant::Destructive role=AlertRole::Alert title=t("library-error")
                                actions=move || view! { <Button variant=ButtonVariant::Outline size=ButtonSize::Sm on:click=move |_| retry.run(())>{t("library-retry")}</Button> }>
                                {error_text(&e)}
                            </Alert>
                        }.into_any()
                    }
                    Ok(page) if page.rows.is_empty() => view! { <NoRows lib=lib/> }.into_any(),
                    Ok(first) => {
                        let page = latest_page(lib.rows, first);
                        view! { <RowsPage lib=lib page=page grid={mode == View::Grid} shown=shown limit=limit/> }.into_any()
                    }
                }/>
    }
}

/// One loaded page of rows with "Showing N of M" and "Load more".
#[component]
fn RowsPage(
    lib: Lib,
    page: Memo<LibraryRowsDto>,
    grid: bool,
    shown: RwSignal<(String, u32)>,
    limit: Memo<u32>,
) -> impl IntoView {
    let rows = Signal::derive(move || page.with(|p| p.rows.clone()));
    let showing = move || {
        page.with(|p| {
            use_i18n().t_args(
                "library-showing",
                &[("shown", p.rows.len().into()), ("total", p.total.into())],
            )
        })
    };
    let more = move || page.with(|p| p.total > p.rows.len() as u64);
    view! {
        {if grid {
            view! { <Grid lib=lib rows=rows/> }.into_any()
        } else {
            view! { <ListView lib=lib rows=rows/> }.into_any()
        }}
        <div class="mt-6 flex flex-col items-center gap-2">
            <p class="text-sm text-muted-foreground" role="status">{showing}</p>
            {move || more().then(|| view! {
                <Button variant=ButtonVariant::Outline on:click=move |_| {
                    let href = lib.state.with_untracked(LibraryState::href);
                    shown.set((href, limit.get_untracked() + PAGE_SIZE));
                }>{t("library-more")}</Button>
            })}
        </div>
    }
}

const GRID: &str = "grid grid-cols-[repeat(auto-fill,minmax(8.5rem,1fr))] gap-4";

/// Twelve poster-shaped placeholders (design.md 6.2), with a status for screen readers.
#[component]
fn PosterSkeletons() -> impl IntoView {
    view! {
        <p class="sr-only" role="status">{t("library-loading")}</p>
        <div class=GRID>
            {(0..12).map(|_| view! {
                <div class="flex flex-col gap-2">
                    <Skeleton class="aspect-[2/3] w-full"/>
                    <Skeleton class="h-4 w-3/4"/>
                </div>
            }).collect_view()}
        </div>
    }
}

/// The grid's one tab stop (design.md 6.2, 8: roving tabindex): the entry whose card link,
/// pill and menu are in the tab order. It follows the focus, and falls back to the first card
/// when its entry leaves the page.
fn grid_tab_stop(rows: Signal<Vec<LibraryRowDto>>) -> RwSignal<Option<String>> {
    let active = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        rows.with(|v| {
            let here = active.with_untracked(|a| {
                a.as_ref()
                    .is_some_and(|id| v.iter().any(|r| r.entry.id == *id))
            });
            if !here {
                active.set(v.first().map(|r| r.entry.id.clone()));
            }
        });
    });
    active
}

/// The card links of the grid in order, with the index of the focused one.
#[cfg(feature = "hydrate")]
fn grid_links(grid: &web_sys::HtmlElement) -> (Vec<web_sys::HtmlElement>, Option<usize>) {
    use wasm_bindgen::JsCast;
    let list = grid.query_selector_all("a[id^=card-]").ok();
    let links: Vec<web_sys::HtmlElement> = list
        .map(|l| {
            (0..l.length())
                .filter_map(|i| l.item(i))
                .filter_map(|n| n.dyn_into().ok())
                .collect()
        })
        .unwrap_or_default();
    let focused = crate::keyboard::focused();
    let at = links.iter().position(|a| {
        focused
            .as_ref()
            .is_some_and(|f| f.is_same_node(Some(a.as_ref())))
    });
    (links, at)
}

/// Keys on a focused card (design.md 6.2): arrows by cell and by row (the column count is read
/// from the rendered layout), Home/End (with Ctrl: first/last), `W` its next step, `T` its
/// tags, `L` the list picker; Enter opens it (the link does that). Arrow keys work with the
/// shortcuts switched off too (they move inside the grid, WCAG 2.1.4 is about character keys).
fn grid_keys(lib: Lib, rows: Signal<Vec<LibraryRowDto>>, grid: NodeRef<leptos::html::Ul>) {
    let on = expect_context::<crate::keyboard::ShortcutsOn>().0;
    let pick = pick_one();
    let h = window_event_listener(leptos::ev::keydown, move |ev| {
        #[cfg(feature = "hydrate")]
        {
            let Some(g) = grid.get_untracked() else {
                return;
            };
            let (links, Some(at)) = grid_links(&g) else {
                return;
            };
            if ev.default_prevented() || ev.alt_key() || ev.meta_key() {
                return;
            }
            let tops: Vec<f64> = links
                .iter()
                .map(|a| a.get_bounding_client_rect().top())
                .collect();
            let cols = ui::columns_from_tops(&tops);
            if let Some(to) = ui::roving_target(at, links.len(), cols, &ev.key(), ev.ctrl_key()) {
                ev.prevent_default();
                let _ = links[to].focus();
                return;
            }
            if !crate::keyboard::plain_key(&ev, on) {
                return;
            }
            let id = links[at].id();
            let entry = id.trim_start_matches("card-");
            let Some(row) =
                rows.with_untracked(|v| v.iter().find(|r| r.entry.id == entry).cloned())
            else {
                return;
            };
            let used = match ev.key().to_lowercase().as_str() {
                "w" => {
                    crate::keyboard::click(&format!("[data-entry=\"{entry}\"] [data-next-step]"))
                }
                "t" => {
                    lib.edit_tags.run(row);
                    true
                }
                "l" => {
                    pick.run(row);
                    true
                }
                _ => false,
            };
            if used {
                ev.prevent_default();
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (&ev, lib, rows, grid, on, pick);
    });
    on_cleanup(move || h.remove());
}

#[component]
fn Grid(lib: Lib, rows: Signal<Vec<LibraryRowDto>>) -> impl IntoView {
    let label = t("library-results");
    let active = grid_tab_stop(rows);
    let grid = NodeRef::<leptos::html::Ul>::new();
    grid_keys(lib, rows, grid);
    // the tab stop follows the focus (a click, Tab into the grid, an arrow key)
    let follow = move |ev: leptos::ev::FocusEvent| {
        #[cfg(feature = "hydrate")]
        {
            use wasm_bindgen::JsCast;
            let card = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|el| el.closest("[data-entry]").ok().flatten())
                .and_then(|li| li.get_attribute("data-entry"));
            if let Some(id) = card
                && active.with_untracked(|a| a.as_deref() != Some(id.as_str()))
            {
                active.set(Some(id));
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = (ev, active);
    };
    view! {
        <ul class=GRID aria-label=label node_ref=grid on:focusin=follow>
            {keyed_rows(rows, move |row, open| {
                let statuses = lib.statuses_of(&row.item.kind);
                let id = row.entry.id.clone();
                let tab_stop = Signal::derive(move || active.with(|a| a.as_deref() == Some(id.as_str())));
                view! { <PosterCard row=row statuses=statuses on_change=lib.refresh on_tags=lib.edit_tags on_list=pick_one() menu_open=open tab_stop=tab_stop/> }
            })}
        </ul>
    }
}

/// `true` once the calling view runs in the browser: an effect runs only after hydration has
/// attached the handlers. The list's checkboxes are rendered disabled until then (the server's
/// HTML too), so a tick that lands before the handlers exist cannot leave a box ticked with
/// nothing selected; Playwright's `check` waits for an enabled box (DEC-485).
fn live() -> Signal<bool> {
    let live = RwSignal::new(false);
    Effect::new(move |_| live.set(true));
    live.into()
}

/// After a change of the select-all box, its tick and dash show the selection (the browser
/// ticked it on the click alone; `prop:checked` only writes when the selection changes).
fn shows_selection(ev: &leptos::ev::Event, all: bool, some: bool) {
    #[cfg(feature = "hydrate")]
    {
        let el: web_sys::HtmlInputElement = event_target(ev);
        el.set_checked(all);
        el.set_indeterminate(some);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (ev, all, some);
}

#[component]
fn ListView(lib: Lib, rows: Signal<Vec<LibraryRowDto>>) -> impl IntoView {
    let i18n = use_i18n();
    let sel = expect_context::<Selection>();
    let live = live();
    // rows that left the page (a refresh after a change) leave the selection
    Effect::new(move |_| {
        let here: std::collections::BTreeSet<String> =
            rows.with(|v| v.iter().map(|r| r.item.id.clone()).collect());
        if sel
            .ids
            .with_untracked(|s| s.iter().any(|id| !here.contains(id)))
        {
            sel.ids.update(|s| s.retain(|id| here.contains(id)));
        }
    });
    let all = move || {
        rows.with(|v| !v.is_empty() && sel.ids.with(|s| v.iter().all(|r| s.contains(&r.item.id))))
    };
    let some = move || !all() && sel.ids.with(|s| !s.is_empty());
    let select_all = move |on: bool| {
        let page: Vec<String> =
            rows.with_untracked(|v| v.iter().map(|r| r.item.id.clone()).collect());
        sel.ids.update(|s| {
            for id in page {
                if on {
                    s.insert(id);
                } else {
                    s.remove(&id);
                }
            }
        });
    };
    // the statuses of the selected rows' kinds, in first-seen order
    let statuses = Signal::derive(move || {
        let mut out: Vec<String> = Vec::new();
        rows.with(|v| {
            for r in v
                .iter()
                .filter(|r| sel.ids.with(|s| s.contains(&r.item.id)))
            {
                for st in lib.statuses_of(&r.item.kind) {
                    if !out.contains(&st) {
                        out.push(st);
                    }
                }
            }
        });
        out
    });
    let head: Vec<String> = [
        "library-col-title",
        "library-col-kind",
        "library-col-progress",
        "library-col-rating",
        "library-col-tags",
        "library-col-activity",
        "library-col-next",
    ]
    .iter()
    .map(|k| i18n.t(k))
    .collect();
    let all_label = i18n.t("library-select-all");
    let pick = expect_context::<ListPick>();
    view! {
        <Table>
            <TableHeader>
                <TableRow>
                    <TableHead attr:scope="col" class="w-10">
                        <input type="checkbox" id="lib-select-all" aria-label=all_label
                            class="size-5 rounded border-input accent-primary"
                            prop:checked=all prop:indeterminate=some disabled=move || !live.get()
                            on:change=move |ev| {
                                select_all(event_target_checked(&ev));
                                shows_selection(&ev, untrack(all), untrack(some));
                            }/>
                    </TableHead>
                    {head.into_iter().map(|h| view! { <TableHead attr:scope="col">{h}</TableHead> }).collect_view()}
                </TableRow>
            </TableHeader>
            <TableBody>
                {keyed_rows(rows, move |row, open| view! { <ListRow lib=lib row=row open=open live=live/> })}
            </TableBody>
        </Table>
        <Show when=move || sel.ids.with(|s| !s.is_empty())>
            <BulkBar selection=sel statuses=statuses list_pick=pick on_done=lib.refresh/>
        </Show>
        // a failed undo after a remove: the bar is gone, the error stays (DEC-418)
        {move || sel.ids.with(std::collections::BTreeSet::is_empty).then(|| sel.error.get()).flatten().map(|e| view! { <p role="alert" class="mt-3 text-sm text-destructive-text">{e}</p> })}
    }
}

#[component]
fn ListRow(
    lib: Lib,
    row: LibraryRowDto,
    open: RwSignal<bool>,
    live: Signal<bool>,
) -> impl IntoView {
    let i18n = use_i18n();
    let item = row.item.clone();
    let progress = progress_text(&i18n, &row.summary, &row.next_step)
        .unwrap_or_else(|| i18n.t(&format!("status-{}", row.entry.status)));
    let rating = row.entry.rating.map_or_else(
        || i18n.t("library-no-rating"),
        |r| {
            i18n.t_args(
                "library-rating-value",
                &[("stars", (f64::from(r) / 20.0).into())],
            )
        },
    );
    let statuses = lib.statuses_of(&item.kind);
    let (href, link_id) = (
        format!("/items/{}", item.id),
        format!("card-{}", row.entry.id),
    );
    let (id, title, kind) = (
        item.id.clone(),
        crate::i18n::title_text(&item.title),
        kind_label(&item.kind),
    );
    let date = i18n.date(&row.last_activity);
    let year = item.year;
    let link_text = title.clone();
    let (step, tags) = (row.next_step.clone(), row.tags.clone());
    let sel = expect_context::<Selection>();
    let (pick_id, check_label) = (
        item.id.clone(),
        i18n.t_args(
            "library-select",
            &[("title", crate::i18n::title_text(&item.title).into())],
        ),
    );
    let checked = {
        let id = pick_id.clone();
        move || sel.ids.with(|s| s.contains(&id))
    };
    view! {
        <TableRow>
            <TableCell>
                <input type="checkbox" aria-label=check_label data-testid="row-select"
                    class="size-5 rounded border-input accent-primary"
                    prop:checked=checked disabled=move || !live.get()
                    on:change=move |ev| sel.toggle(&pick_id, event_target_checked(&ev))/>
            </TableCell>
            <TableCell>
                <a href=href id=link_id class="font-medium hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">{link_text}</a>
                {year.map(|y| view! { <span class="ml-1 text-muted-foreground">{y}</span> })}
            </TableCell>
            <TableCell>{kind}</TableCell>
            <TableCell>{progress}</TableCell>
            <TableCell>{rating}</TableCell>
            <TableCell><TagBadges tags=tags/></TableCell>
            <TableCell class="whitespace-nowrap">{date}</TableCell>
            <TableCell>
                <div class="flex items-center gap-1">
                    <NextStepButton item_id=id title=title step=step on_done=lib.refresh/>
                    <QuickMenu row=row statuses=statuses on_change=lib.refresh on_tags=lib.edit_tags on_list=pick_one() open=open/>
                </div>
            </TableCell>
        </TableRow>
    }
}

/// No rows: an empty library (find something) or nothing matching (clear the filters, or
/// the nearest status with entries).
#[component]
fn NoRows(lib: Lib) -> impl IntoView {
    let palette = expect_context::<crate::palette::PaletteOpen>().0;
    move || {
        let i18n = use_i18n();
        let counts = lib.counts_now();
        if counts.as_ref().is_some_and(|c| c.total == 0) {
            return view! {
                <EmptyState title=i18n.t("library-empty") icon=IconName::Library body=i18n.t("library-empty-body")
                    primary=move || view! { <Button on:click=move |_| palette.set(true)><Icon name=IconName::Search/> {t("library-find")}</Button> }/>
            }
            .into_any();
        }
        let s = lib.state.get();
        let mut named: Vec<String> = s.filters().iter().map(filter_text).collect();
        if let Some(st) = &s.status {
            named.insert(0, i18n.t(&format!("status-{st}")));
        }
        named.splice(0..0, s.kinds.iter().map(|k| kind_label(k)));
        let nearest = lib
            .kinds_now()
            .zip(counts)
            .and_then(|(k, c)| nearest_status(&status_tabs(&k, &c, &s).1, &s))
            .filter(|_| s.status.is_some());
        let body = i18n.t_args(
            "library-no-match-body",
            &[("filters", named.join(", ").into())],
        );
        let secondary = nearest.map(|st| {
            let word = i18n.t(&format!("status-{st}"));
            let label = i18n.t_args("library-show-status", &[("status", word.into())]);
            move || {
                let (st, label) = (st.clone(), label.clone());
                view! {
                    <Button variant=ButtonVariant::Ghost on:click=move |_| lib.go.run(lib.state.get_untracked().with_status(Some(st.clone())))>{label}</Button>
                }
            }
        });
        let primary = move || {
            view! {
                <Button variant=ButtonVariant::Outline on:click=move |_| lib.go.run(lib.state.get_untracked().cleared())>{t("library-clear")}</Button>
            }
        };
        match secondary {
            Some(sec) => view! {
                <EmptyState title=i18n.t("library-no-match") icon=IconName::Search body=body primary=primary secondary=sec/>
            }
            .into_any(),
            None => view! {
                <EmptyState title=i18n.t("library-no-match") icon=IconName::Search body=body primary=primary/>
            }
            .into_any(),
        }
    }
}

/// The quick menu's "Add to list": the picker for one row.
fn pick_one() -> Callback<LibraryRowDto> {
    let pick = expect_context::<ListPick>();
    Callback::new(move |r: LibraryRowDto| pick.0.set(Some(vec![r.item.id])))
}
