//! Domain widgets (D-3): poster card with thumbhash placeholder, rating input, companion picker.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use contracts::{CompanionDto, ImageDto, LibraryRowDto};
use leptos::prelude::*;
use leptos_router::NavigateOptions;
use ui::{
    Badge, BadgeVariant, Button, Chip, Combobox, Dialog, DropdownMenu, FormField, Icon, IconName,
    Input, MenuItem, ProgressBar, ProgressSize, Rating, SelectOption, ToastVariant, cn,
};

use crate::api;
use crate::i18n::{t, use_i18n};
use crate::library_state::square_art;
pub use crate::next_step::NextStepButton;
use crate::next_step::{
    LoggedStep, StepCtx, next_step_label, progress_text, run_step, step_progress,
};

/// Decode a base64 thumbhash into a tiny BMP data URL (no image codec needed; identical on
/// server and client, so hydration matches).
#[must_use]
pub fn thumbhash_data_url(b64: &str) -> Option<String> {
    let hash = STANDARD.decode(b64).ok()?;
    let (width, height, rgba) = thumbhash::thumb_hash_to_rgba(&hash).ok()?;
    let row = (width * 3).div_ceil(4) * 4;
    let size = 54 + row * height;
    let mut b = Vec::with_capacity(size);
    let le32 = |v: usize| u32::try_from(v).unwrap_or(0).to_le_bytes();
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&le32(size));
    b.extend_from_slice(&[0, 0, 0, 0]);
    b.extend_from_slice(&le32(54));
    b.extend_from_slice(&le32(40));
    b.extend_from_slice(&le32(width));
    b.extend_from_slice(&le32(height));
    b.extend_from_slice(&[1, 0, 24, 0]);
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&le32(row * height));
    b.extend_from_slice(&[0; 16]);
    for y in (0..height).rev() {
        for x in 0..width {
            let px = &rgba[(y * width + x) * 4..(y * width + x) * 4 + 4];
            // premultiply onto white so transparent areas stay light
            let blend = |c: u8| {
                let alpha = u16::from(px[3]);
                u8::try_from((u16::from(c) * alpha + 255 * (255 - alpha)) / 255).unwrap_or(u8::MAX)
            };
            b.extend_from_slice(&[blend(px[2]), blend(px[1]), blend(px[0])]);
        }
        b.resize(b.len() + (row - width * 3), 0);
    }
    Some(format!("data:image/bmp;base64,{}", STANDARD.encode(b)))
}

/// `srcset` for the image variants of one format.
fn srcset(img: &ImageDto, mime: &str) -> String {
    img.variants
        .iter()
        .filter(|v| v.mime == mime)
        .map(|v| format!("{} {}w", v.url, v.width))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Cover with an instant placeholder (thumbhash, else the dominant colour).
#[component]
pub fn Cover(
    #[prop(into)] title: String,
    image: Option<ImageDto>,
    #[prop(into, optional)] class: String,
    /// CSS `sizes` hint for `srcset`.
    #[prop(into, optional)]
    sizes: Option<String>,
    /// Square art (album covers, board game boxes) instead of a 2:3 poster.
    #[prop(optional)]
    square: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let alt = i18n.t_args("item-cover-alt", &[("title", title.clone().into())]);
    let aspect = if square {
        "aspect-square"
    } else {
        "aspect-[2/3]"
    };
    let base = format!("relative overflow-hidden rounded-md bg-muted {aspect} {class}");
    match image {
        None => view! {
            <div class=base role="img" aria-label=alt>
                <span class="absolute inset-0 flex items-center justify-center p-2 text-center text-xs text-muted-foreground">{title}</span>
            </div>
        }
        .into_any(),
        Some(img) => {
            let placeholder = img.thumbhash.as_deref().and_then(thumbhash_data_url);
            let style = match (&placeholder, &img.dominant_color) {
                (Some(url), _) => format!("background-image:url('{url}');background-size:cover"),
                (None, Some(c)) => format!("background-color:{c}"),
                (None, None) => String::new(),
            };
            let avif = srcset(&img, "image/avif");
            let webp = srcset(&img, "image/webp");
            let sizes = sizes.unwrap_or_else(|| "(min-width: 768px) 200px, 45vw".into());
            view! {
                <div class=base style=style>
                    <picture>
                        {(!avif.is_empty()).then(|| view! { <source type="image/avif" srcset=avif.clone() sizes=sizes.clone()/> })}
                        {(!webp.is_empty()).then(|| view! { <source type="image/webp" srcset=webp.clone() sizes=sizes.clone()/> })}
                        <img
                            src=img.url.clone()
                            alt=alt
                            loading="lazy"
                            decoding="async"
                            referrerpolicy="no-referrer"
                            class="absolute inset-0 h-full w-full object-cover"
                        />
                    </picture>
                </div>
            }
            .into_any()
        }
    }
}

/// Grid card of a library row (design.md 6.2, 7.3): cover (square for music, podcasts and
/// board games) with the progress bar on its edge, the next-step pill (a one-click
/// [`NextStepButton`] while the entry is planned or in progress, else the status word), a
/// quick menu, title, kind, year, status and tags.
#[component]
pub fn PosterCard(
    row: LibraryRowDto,
    /// Statuses the kind allows ("Mark as ..." in the quick menu).
    #[prop(optional)]
    statuses: Vec<String>,
    /// Runs after anything on the card changed the entry (refetch).
    #[prop(optional_no_strip, into)]
    on_change: Option<Callback<()>>,
    /// Opens the tag editor for this row.
    #[prop(optional_no_strip, into)]
    on_tags: Option<Callback<LibraryRowDto>>,
    /// Opens the list picker for this row.
    #[prop(optional_no_strip, into)]
    on_list: Option<Callback<LibraryRowDto>>,
    /// Whether the quick menu is open, kept by the caller across rebuilds of the card.
    #[prop(optional_no_strip, into)]
    menu_open: Option<RwSignal<bool>>,
    /// In a roving grid: whether this card holds the grid's one tab stop (its link, pill and
    /// menu get `tabindex` 0, else -1).
    #[prop(optional_no_strip, into)]
    tab_stop: Option<Signal<bool>>,
) -> impl IntoView {
    let tab =
        Signal::derive(move || tab_stop.map(|s| if s.get() { "0" } else { "-1" }.to_string()));
    let i18n = use_i18n();
    let item = row.item.clone();
    let href = format!("/items/{}", item.id);
    let kind = crate::i18n::kind_label(&item.kind);
    let status = row.entry.status.clone();
    let status_word = t(&format!("status-{status}"));
    let square = square_art(&item.kind);
    let progress = progress_text(&i18n, &row.summary, &row.next_step);
    let fraction = row.summary.fraction;
    let active = matches!(status.as_str(), "planned" | "in_progress");
    let pill = if active {
        view! {
            <NextStepButton item_id=item.id.clone() title=crate::i18n::title_text(&item.title) step=row.next_step.clone() compact=true on_done=on_change tabindex=tab/>
        }
        .into_any()
    } else {
        view! { <span class="inline-flex min-h-7 items-center rounded-full border border-border bg-background/95 px-2.5 text-xs font-semibold text-foreground shadow-sm">{status_word.clone()}</span> }.into_any()
    };
    let bar = fraction.zip(progress).map(|(f, text)| {
        let value = Signal::derive(move || crate::next_step::percent(f));
        view! {
            <ProgressBar label=t("library-progress") value=value value_text=text size=ProgressSize::Thin
                class="absolute inset-x-0 bottom-0 rounded-none bg-background/60"/>
        }
    });
    view! {
        <li class="group/card relative flex min-w-0 flex-col gap-1.5" data-testid="library-card" data-entry=row.entry.id.clone()>
            <a href=href id=format!("card-{}", row.entry.id) tabindex=move || tab.get() class="group flex flex-col gap-1.5 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <div class="relative">
                    <Cover title=crate::i18n::title_text(&item.title) image=item.cover.clone() square=square sizes="(min-width: 768px) 180px, 45vw"/>
                    {bar}
                </div>
                <span class="line-clamp-2 text-sm font-medium group-hover:underline">{crate::i18n::title_text(&item.title)}</span>
            </a>
            <div class="absolute left-1.5 top-1.5 flex max-w-[calc(100%-3.5rem)] flex-col items-start gap-1">{pill}</div>
            <QuickMenu row=row.clone() statuses=statuses on_change=on_change on_tags=on_tags on_list=on_list open=menu_open tabindex=tab
                class="absolute right-1.5 top-1.5 [@media(hover:hover)]:opacity-0 group-hover/card:opacity-100 focus-within:opacity-100"/>
            <span class="text-xs text-muted-foreground">
                {kind}
                {item.year.map(|y| format!(" - {y}"))}
                " - " {status_word}
            </span>
            <TagBadges tags=row.tags.clone() max=3/>
        </li>
    }
}

/// An entry's tags as small badges; beyond `max` a "+n" badge.
#[component]
pub fn TagBadges(tags: Vec<String>, #[prop(optional)] max: Option<usize>) -> impl IntoView {
    let max = max.unwrap_or(usize::MAX);
    let more = tags.len().saturating_sub(max);
    (!tags.is_empty()).then(|| {
        let label = t("library-tags");
        view! {
            <ul class="flex flex-wrap gap-1" aria-label=label>
                {tags.into_iter().take(max).map(|tag| view! {
                    <li><Badge variant=BadgeVariant::Outline class="font-medium">{tag}</Badge></li>
                }).collect_view()}
                {(more > 0).then(|| view! { <li><Badge variant=BadgeVariant::Outline>{format!("+{more}")}</Badge></li> })}
            </ul>
        }
    })
}

/// "Mark as <status>" in the quick menu: the change with a toast, queued while offline.
fn status_item(
    ctx: &StepCtx,
    (item_id, title): (&str, &str),
    s: String,
    error: RwSignal<Option<String>>,
    on_change: Option<Callback<()>>,
) -> MenuItem {
    let i18n = ctx.i18n.clone();
    let word = i18n.t(&format!("status-{s}"));
    let done = i18n.t_args("library-status-changed", &[("status", word.clone().into())]);
    let (id, toasts, i, c, title) = (
        item_id.to_string(),
        ctx.toasts,
        ctx.i18n.clone(),
        ctx.clone(),
        title.to_string(),
    );
    MenuItem {
        label: i18n.t_args("library-mark", &[("status", word.into())]),
        on_select: Callback::new(move |()| {
            let (id, s, done, i, c, title) = (
                id.clone(),
                s.clone(),
                done.clone(),
                i.clone(),
                c.clone(),
                title.clone(),
            );
            error.set(None);
            leptos::task::spawn_local(async move {
                match api::set_status(id.clone(), s.clone()).await {
                    Ok(_) => {
                        toasts.push(done, None, ToastVariant::Default);
                        if let Some(cb) = on_change {
                            cb.run(());
                        }
                    }
                    // offline: queued with a client stamp (WP4-5)
                    Err(e)
                        if crate::offline::network_error(&e)
                            && c.queue_field(title, |h| {
                                crate::offline::status_command(&id, &s, h)
                            }) => {}
                    Err(e) => {
                        let text = crate::pages::error_text(&e);
                        error.set(Some(
                            i.t_args("library-step-failed", &[("error", text.into())]),
                        ));
                    }
                }
            });
        }),
        destructive: false,
    }
}

/// The quick menu of a card or list row (design.md 6.2): the next step, "Mark as" each other
/// status, edit tags, add to a list, open the item, remove from the library (toast with
/// Undo).
#[component]
pub fn QuickMenu(
    row: LibraryRowDto,
    statuses: Vec<String>,
    #[prop(optional_no_strip, into)] on_change: Option<Callback<()>>,
    #[prop(optional_no_strip, into)] on_tags: Option<Callback<LibraryRowDto>>,
    #[prop(optional_no_strip, into)] on_list: Option<Callback<LibraryRowDto>>,
    #[prop(into, optional)] class: MaybeProp<String>,
    /// Whether the menu is open, kept by the caller across rebuilds of the row.
    #[prop(optional_no_strip, into)]
    open: Option<RwSignal<bool>>,
    /// The trigger's tab stop (see [`PosterCard`]).
    #[prop(into, optional)]
    tabindex: MaybeProp<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let ctx = StepCtx::current();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let item_id = row.item.id.clone();
    let title = crate::i18n::title_text(&row.item.title);
    let mut items = Vec::new();
    if let Some(progress) = step_progress(&row.next_step) {
        let label = next_step_label(&i18n, &row.next_step);
        let logged = LoggedStep {
            item_id: item_id.clone(),
            title: title.clone(),
            label: label.clone(),
            progress,
        };
        let ctx = ctx.clone();
        items.push(MenuItem {
            label,
            on_select: Callback::new(move |()| {
                run_step(ctx.clone(), logged.clone(), busy, error, on_change);
            }),
            destructive: false,
        });
    }
    for s in statuses.into_iter().filter(|s| *s != row.entry.status) {
        items.push(status_item(&ctx, (&item_id, &title), s, error, on_change));
    }
    if let Some(cb) = on_tags {
        let r = row.clone();
        items.push(MenuItem {
            label: i18n.t("library-edit-tags"),
            on_select: Callback::new(move |()| cb.run(r.clone())),
            destructive: false,
        });
    }
    if let Some(cb) = on_list {
        let r = row.clone();
        items.push(MenuItem {
            label: i18n.t("library-add-to-list"),
            on_select: Callback::new(move |()| cb.run(r.clone())),
            destructive: false,
        });
    }
    let nav = StoredValue::new(leptos_router::hooks::use_navigate());
    let href = format!("/items/{item_id}");
    items.push(MenuItem {
        label: i18n.t("library-open"),
        on_select: Callback::new(move |()| {
            nav.with_value(|n| n(&href, NavigateOptions::default()));
        }),
        destructive: false,
    });
    {
        let (ctx, id, title) = (ctx.clone(), item_id.clone(), title.clone());
        items.push(MenuItem {
            label: i18n.t("library-remove"),
            on_select: Callback::new(move |()| {
                crate::bulk::remove_one(&ctx, id.clone(), &title, error, on_change);
            }),
            destructive: true,
        });
    }
    let label = i18n.t_args("library-actions", &[("title", title.into())]);
    view! {
        <div class=move || cn(&["flex flex-col items-end gap-1", &class.get().unwrap_or_default()])>
            <DropdownMenu label=label items=items id=format!("actions-{}", row.entry.id) open=open trigger_tabindex=tabindex
                trigger_class="size-8 rounded-full border border-border bg-background/95 px-0 shadow-sm"
                trigger=Box::new(|| view! { <Icon name=IconName::More/> }.into_any())/>
            {move || error.get().map(|e| view! { <p role="alert" class="rounded bg-background px-1 text-xs text-destructive-text">{e}</p> })}
        </div>
    }
}

/// The tag editor dialog for one entry (`target`: the row; `None` closes it). Adds and
/// removes tags one at a time; `known` feeds the suggestions.
#[component]
pub fn TagEditor(
    target: RwSignal<Option<LibraryRowDto>>,
    #[prop(into)] known: Signal<Vec<String>>,
    #[prop(optional_no_strip, into)] on_change: Option<Callback<()>>,
) -> impl IntoView {
    let i18n = use_i18n();
    let open = RwSignal::new(false);
    let tags = RwSignal::new(Vec::<String>::new());
    let new_tag = RwSignal::new(String::new());
    let error = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if let Some(r) = target.get() {
            tags.set(r.tags.clone());
            new_tag.set(String::new());
            error.set(None);
            open.set(true);
        }
    });
    Effect::new(move |prev: Option<bool>| {
        let now = open.get();
        if prev == Some(true) && !now {
            target.set(None);
            if let Some(cb) = on_change {
                cb.run(());
            }
        }
        now
    });
    let item_id = move || {
        target.with_untracked(|r| r.as_ref().map(|r| r.item.id.clone()).unwrap_or_default())
    };
    let i2 = i18n.clone();
    let apply = move |res: Result<Vec<String>, ServerFnError>| match res {
        Ok(t) => tags.set(t),
        Err(e) => error.set(Some(i2.t_args(
            "library-step-failed",
            &[("error", crate::pages::error_text(&e).into())],
        ))),
    };
    let add = Action::new(move |tag: &String| {
        let (id, tag) = (item_id(), tag.clone());
        async move { api::add_tag(id, tag).await }
    });
    let remove = Action::new(move |tag: &String| {
        let (id, tag) = (item_id(), tag.clone());
        async move { api::remove_tag(id, tag).await }
    });
    let apply2 = apply.clone();
    Effect::new(move |_| {
        if let Some(r) = add.value().get() {
            if r.is_ok() {
                new_tag.set(String::new());
            }
            apply(r);
        }
    });
    Effect::new(move |_| {
        if let Some(r) = remove.value().get() {
            apply2(r);
        }
    });
    let title = move || {
        target
            .with(|r| r.as_ref().map(|r| crate::i18n::title_text(&r.item.title)))
            .unwrap_or_default()
    };
    view! {
        {move || target.get().map(|_| {
            let i = use_i18n();
            let heading = i.t_args("library-tags-title", &[("title", title().into())]);
            let close = i.t("dialog-close");
            view! {
                <Dialog open=open title=heading close_label=close>
                    <TagEditorBody tags=tags new_tag=new_tag error=error known=known add=add remove=remove/>
                </Dialog>
            }
        })}
    }
}

type TagAction = Action<String, Result<Vec<String>, ServerFnError>>;

/// The tag editor's content: the tags as remove buttons, the add form with suggestions and
/// the inline error.
#[component]
fn TagEditorBody(
    tags: RwSignal<Vec<String>>,
    new_tag: RwSignal<String>,
    error: RwSignal<Option<String>>,
    known: Signal<Vec<String>>,
    add: TagAction,
    remove: TagAction,
) -> impl IntoView {
    let i = use_i18n();
    let list_label = i.t("library-tags");
    let new_label = i.t("library-tag-new");
    let add_label = i.t("library-tag-add");
    view! {
        <div class="grid gap-4">
            <ul class="flex flex-wrap gap-2" aria-label=list_label>
                <For each=move || tags.get() key=|t| t.clone() let:tag>
                    <li>
                        <button type="button"
                            class="inline-flex min-h-8 items-center gap-1 rounded-full border border-input px-3 text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring cursor-pointer"
                            aria-label=use_i18n().t_args("library-tag-remove", &[("tag", tag.clone().into())])
                            on:click={ let tag = tag.clone(); move |_| { remove.dispatch(tag.clone()); } }>
                            {tag.clone()} <Icon name=IconName::Close/>
                        </button>
                    </li>
                </For>
            </ul>
            <form class="flex items-end gap-2" on:submit=move |ev| {
                ev.prevent_default();
                let v = new_tag.get_untracked().trim().to_string();
                if !v.is_empty() {
                    add.dispatch(v);
                }
            }>
                <div class="flex-1">
                    <FormField id="tag-new" label=new_label>
                        <Input id="tag-new" value=new_tag autocomplete="off" attr:list="tag-known"/>
                    </FormField>
                </div>
                <datalist id="tag-known">
                    {move || known.get().into_iter().map(|k| view! { <option value=k></option> }).collect_view()}
                </datalist>
                <Button r#type="submit">{add_label}</Button>
            </form>
            {move || error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
        </div>
    }
}

/// The person's rating as five stars in half steps over `ui::Rating`, stored as 0..=100
/// (DEC-018): 3.5 stars = 70. This wrapper only maps the scale and the translated labels.
#[component]
pub fn RatingInput(
    value: RwSignal<Option<u8>>,
    #[prop(optional_no_strip, into)] on_change: Option<Callback<Option<u8>>>,
    /// Element id (default `item-rating`; the log sheet has its own).
    #[prop(into, optional)]
    id: Option<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let label = i18n.t("item-rating");
    let option_label = Callback::new(move |v: u8| {
        i18n.t_args("item-rate", &[("stars", (f64::from(v) / 20.0).into())])
    });
    view! {
        <Rating
            id=id.unwrap_or_else(|| "item-rating".into())
            label=label
            value=value
            stars=5
            half_steps=true
            option_label=option_label
            on_change=Callback::new(move |v| if let Some(cb) = on_change { cb.run(v) })
        />
    }
}

/// A logged progress event in words (`{"type": "episode", "season": 2, "episode": 5}` ->
/// "Season 2, episode 5"); unknown types show their name.
#[must_use]
pub fn progress_label(i: &crate::i18n::I18n, p: &serde_json::Value) -> String {
    let n = |field: &str| p[field].as_u64();
    let num = |v: u64| fluent_bundle::FluentValue::from(v);
    match p["type"].as_str().unwrap_or_default() {
        "finished" => i.t("progress-finished"),
        "episode" => i.t_args(
            "progress-episode",
            &[
                ("season", num(n("season").unwrap_or(0))),
                ("episode", num(n("episode").unwrap_or(0))),
            ],
        ),
        "paged" => match (n("page"), n("chapter"), n("volume")) {
            (Some(v), _, _) => i.t_args("progress-page", &[("page", num(v))]),
            (None, Some(v), _) => i.t_args("progress-chapter", &[("chapter", num(v))]),
            (None, None, v) => i.t_args("progress-volume", &[("volume", num(v.unwrap_or(0)))]),
        },
        "position" => {
            let s = n("seconds").unwrap_or(0);
            let time = format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60);
            i.t_args("progress-position", &[("time", time.into())])
        }
        "session" => i.t("progress-session"),
        other => other.to_string(),
    }
}

/// How many people the picker shows as chips before the rest are only found by typing.
const PICKER_CHIPS: usize = 8;

/// People of a log sheet (design.md 7.3): toggle chips for the selected and the first few
/// other people, a combobox to find anyone else by name, and inline create for a new name
/// (the new person is selected at once). `include_self` lists the viewer's own companion
/// (players of a play); "watched with" leaves it out.
#[component]
pub fn CompanionPicker(
    #[prop(into)] id: String,
    #[prop(into)] legend: String,
    companions: RwSignal<Vec<CompanionDto>>,
    selected: RwSignal<Vec<String>>,
    #[prop(optional)] include_self: bool,
) -> impl IntoView {
    let i18n = use_i18n();
    let error = RwSignal::new(None::<String>);
    let active = move || {
        companions.with(|cs| {
            cs.iter()
                .filter(|c| c.merged_into.is_none() && (include_self || !c.is_self))
                .cloned()
                .collect::<Vec<_>>()
        })
    };
    let chips = move || {
        let sel = selected.get();
        let mut out: Vec<CompanionDto> = active();
        // selected first, then the others in their order, capped
        out.sort_by_key(|c| !sel.contains(&c.id));
        let keep = sel.len().max(PICKER_CHIPS);
        out.truncate(keep);
        out
    };
    let toggle = move |id: &str, on: bool| {
        selected.update(|s| {
            s.retain(|x| x != id);
            if on {
                s.push(id.to_string());
            }
        });
    };
    let options = Signal::derive(move || {
        active()
            .into_iter()
            .map(|c| SelectOption {
                value: c.id,
                label: c.name,
            })
            .collect::<Vec<_>>()
    });
    let found = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        if let Some(id) = found.get() {
            toggle(&id, true);
            found.set(None);
        }
    });
    let failed = i18n.t("error-generic");
    let create = Callback::new(move |name: String| {
        let failed = failed.clone();
        leptos::task::spawn_local(async move {
            match api::create_companion(name).await {
                Ok(c) => {
                    let id = c.id.clone();
                    companions.update(|cs| cs.push(c));
                    toggle(&id, true);
                    error.set(None);
                }
                Err(_) => error.set(Some(failed)),
            }
        });
    });
    let i2 = i18n.clone();
    let create_label =
        Callback::new(move |name: String| i2.t_args("log-people-create", &[("name", name.into())]));
    let legend_id = format!("{id}-legend");
    let labelled = legend_id.clone();
    view! {
        <div role="group" aria-labelledby=labelled class="grid gap-2">
            <span id=legend_id.clone() class="text-sm font-medium">{legend}</span>
            <div class="flex flex-wrap gap-2">
                <For each=chips key=|c| c.id.clone() let:c>
                    {
                        let cid = c.id.clone();
                        let pressed = RwSignal::new(selected.with_untracked(|s| s.contains(&cid)));
                        let watch = cid.clone();
                        Effect::new(move |_| pressed.set(selected.with(|s| s.contains(&watch))));
                        view! {
                            <Chip pressed=pressed on_change=Callback::new(move |on: bool| toggle(&cid, on))>
                                {c.name.clone()}
                            </Chip>
                        }
                    }
                </For>
            </div>
            <Combobox id=format!("{id}-find") label=i18n.t("log-people-add") options=options selected=found
                empty_label=i18n.t("log-people-none") placeholder=i18n.t("log-people-add")
                clear_on_select=true on_create=create create_label=create_label/>
            {move || error.get().map(|e| view! { <p role="alert" class="text-sm text-destructive-text">{e}</p> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::i18n::I18n;

    #[test]
    fn progress_reads_as_words_in_each_locale() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        let ep = serde_json::json!({"type": "episode", "season": 2, "episode": 5});
        assert_eq!(super::progress_label(&en, &ep), "Season 2, episode 5");
        assert_eq!(super::progress_label(&de, &ep), "Staffel 2, Folge 5");
        let done = serde_json::json!({"type": "finished"});
        assert_eq!(super::progress_label(&de, &done), "Abgeschlossen");
        let pos = serde_json::json!({"type": "position", "seconds": 3725});
        assert_eq!(super::progress_label(&en, &pos), "At 1:02:05");
        let page = serde_json::json!({"type": "paged", "page": 1212});
        assert_eq!(super::progress_label(&de, &page), "Seite 1.212");
    }

    #[test]
    fn thumbhash_becomes_a_bmp_data_url() {
        // a real thumbhash (from an image encoded by the assets pipeline)
        let rgba = vec![200u8; 16 * 16 * 4];
        let hash = thumbhash::rgba_to_thumb_hash(16, 16, &rgba);
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, hash);
        let url = super::thumbhash_data_url(&b64).unwrap();
        assert!(url.starts_with("data:image/bmp;base64,Qk"));
        assert!(super::thumbhash_data_url("not base64!").is_none());
    }
}
