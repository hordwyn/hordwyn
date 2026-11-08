//! Settings > Imports and export (design.md 6.11, R-35). One card per format (our own CSV or
//! JSON Lines, the legacy export, Trakt; Goodreads, BG Stats and BGG wait for a sample
//! export); a file is first read as a dry run whose preview says how many rows of each kind
//! it holds, shows sample rows, what maps to what and what is lost; "Import" runs the same
//! file for real with "x of y" progress over `import.progress` and Cancel; the result names
//! the imported, skipped and unmatched rows, keeps the unmatched ones listed (match each
//! through the search palette, which replays the row) and offers Try again. Export: one
//! download per format. Reads come from a server function, changes go to the REST routes
//! (`web::rest`, DEC-327).

use contracts::imports::{ImportDto, PreviewDto, UnmatchedDto};
use leptos::prelude::*;
use ui::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, EmptyState, IconName, ProgressBar,
};

use crate::api;
use crate::events::use_events;
use crate::i18n::{kind_label, t, use_i18n};
use crate::pages::PageTitle;
use crate::rest;
use crate::settings::{Lead, Outcome, SettingsLayout, loaded};

/// Format cards in page order: id and whether it can be imported in this build.
pub const CARDS: [(&str, bool); 6] = [
    ("generic", true),
    ("legacy", true),
    ("trakt", true),
    ("goodreads", false),
    ("bgstats", false),
    ("bgg", false),
];

/// The importer format for a file chosen on `card`: our own format by the file's ending
/// (`.csv`, else JSON Lines), the legacy export, Trakt.
#[must_use]
pub fn format_for(card: &str, file_name: &str) -> Option<&'static str> {
    match card {
        "generic" if file_name.to_ascii_lowercase().ends_with(".csv") => Some("generic-csv"),
        "generic" => Some("generic-jsonl"),
        "legacy" => Some("legacy-json"),
        "trakt" => Some("trakt-json"),
        _ => None,
    }
}

/// The card a format belongs to (its title on the result).
#[must_use]
pub fn card_of(format: &str) -> &'static str {
    match format {
        "legacy-json" => "legacy",
        "trakt-json" => "trakt",
        _ => "generic",
    }
}

/// Classes of a download link that looks like an outline button.
const LINK_BUTTON: &str = "inline-flex h-9 w-fit items-center justify-center gap-2 \
    whitespace-nowrap rounded-md border border-input bg-background px-4 py-2 text-sm \
    font-medium hover:bg-accent hover:text-accent-foreground focus-visible:outline-none \
    focus-visible:ring-2 focus-visible:ring-ring";

/// The file picker's label: a link button that shows the focus of its hidden input.
const PICK_LABEL: &str = "inline-flex h-9 w-fit cursor-pointer items-center justify-center gap-2 \
    whitespace-nowrap rounded-md border border-input bg-background px-4 py-2 text-sm \
    font-medium hover:bg-accent hover:text-accent-foreground peer-focus-visible:ring-2 \
    peer-focus-visible:ring-ring peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-background";

type Data = Resource<Result<Vec<ImportDto>, ServerFnError>>;

/// Page state shared by the cards and the panels.
#[derive(Clone, Copy)]
struct Page {
    data: Data,
    i18n: StoredValue<std::sync::Arc<crate::i18n::I18n>>,
    busy: RwSignal<bool>,
    outcome: RwSignal<Option<Result<String, String>>>,
}

#[component]
pub fn SettingsImports() -> impl IntoView {
    let data: Data = Resource::new(|| (), |()| api::imports());
    let page = Page {
        data,
        i18n: StoredValue::new(use_i18n()),
        busy: RwSignal::new(false),
        outcome: RwSignal::new(None),
    };
    let events = use_events();
    Effect::new(move |_| {
        if let Some((topic, _)) = events.0.get()
            && topic == "import.progress"
        {
            data.refetch();
        }
    });
    view! {
        <SettingsLayout>
            <PageTitle text=t("imports-title")/>
            <Lead text=t("imports-lead")/>
            <div class="grid max-w-4xl gap-8 pb-[var(--tab-bar-height)]">
                <section aria-labelledby="imp-cards-h" class="grid gap-3">
                    <h2 id="imp-cards-h" class="text-lg font-semibold">{t("imports-choose")}</h2>
                    <ul class="grid gap-3 sm:grid-cols-2">
                        {CARDS.iter().map(|(card, ready)| view! {
                            <FormatCard card=card ready=*ready page=page/>
                        }).collect_view()}
                    </ul>
                </section>
                <Outcome message=page.outcome/>
                <section aria-labelledby="imp-now-h" class="grid min-w-0 gap-3">
                    <h2 id="imp-now-h" class="text-lg font-semibold">{t("imports-latest")}</h2>
                    {loaded(data, move |list: Vec<ImportDto>| view! { <Imports list=list page=page/> })}
                </section>
                <Export/>
            </div>
        </SettingsLayout>
    }
}

/// Send a chosen file as a dry run.
#[cfg(feature = "hydrate")]
fn upload(page: Page, card: &'static str, file: web_sys::File) {
    let Some(format) = format_for(card, &file.name()) else {
        return;
    };
    let i18n = page.i18n.get_value();
    page.busy.set(true);
    page.outcome.set(None);
    leptos::task::spawn_local(async move {
        let path = format!("/imports?format={format}&dry_run=true");
        let r = rest::upload::<serde_json::Value>(&path, file).await;
        page.busy.set(false);
        if let Err(e) = r {
            page.outcome
                .set(Some(Err(crate::settings::admin_error_with(&i18n, &e))));
        }
        page.data.refetch();
    });
}

/// One format: what it reads, a file picker that is also a drop zone, or "waiting for a
/// sample" for formats without one.
#[component]
fn FormatCard(card: &'static str, ready: bool, page: Page) -> impl IntoView {
    let input_id = format!("imp-file-{card}");
    let over = RwSignal::new(false);
    let credit = (card == "trakt").then(|| {
        let product = expect_context::<crate::app::ClientConfig>().product_name;
        view! {
            <p class="text-xs text-muted-foreground" data-testid="imp-trakt-credit">
                {use_i18n().t_args("imports-trakt-credit", &[("product", product.into())])}
                " "<a href="https://trakt.tv" class="underline" rel="noopener">{t("imports-trakt-link")}</a>
            </p>
        }
    });
    let on_drop = move |ev: leptos::ev::DragEvent| {
        ev.prevent_default();
        over.set(false);
        #[cfg(feature = "hydrate")]
        if let Some(f) = ev
            .data_transfer()
            .and_then(|d| d.files())
            .and_then(|l| l.get(0))
        {
            upload(page, card, f);
        }
    };
    let on_change = move |ev: leptos::ev::Event| {
        #[cfg(feature = "hydrate")]
        {
            use leptos::wasm_bindgen::JsCast;
            if let Some(input) = ev
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
            {
                if let Some(f) = input.files().and_then(|l| l.get(0)) {
                    upload(page, card, f);
                }
                input.set_value("");
            }
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = ev;
    };
    let zone = move || {
        if over.get() {
            "grid gap-2 rounded-md border-2 border-dashed border-primary bg-accent p-3"
        } else {
            "grid gap-2 rounded-md border-2 border-dashed border-border p-3"
        }
    };
    view! {
        <li class="grid content-start gap-3 rounded-xl border border-border bg-card p-4 shadow-elevation-1"
            data-testid=format!("imp-card-{card}")>
            <div class="flex flex-wrap items-start justify-between gap-2">
                <h3 class="font-semibold">{t(&format!("imports-format-{card}"))}</h3>
                {(!ready).then(|| view! { <Badge variant=BadgeVariant::Secondary>{t("imports-waiting")}</Badge> })}
            </div>
            <p class="text-sm text-muted-foreground">{t(&format!("imports-format-{card}-body"))}</p>
            {credit}
            {ready.then(|| view! {
                <div class=zone
                    on:dragover=move |ev: leptos::ev::DragEvent| { ev.prevent_default(); over.set(true); }
                    on:dragleave=move |_| over.set(false)
                    on:drop=on_drop>
                    // the input stays in the tab order but is hidden: its label shows the focus
                    // ring (`peer`, so the input comes first; UX3-7, WCAG 2.4.7)
                    <input id=input_id.clone() type="file" class="peer sr-only" disabled=move || page.busy.get()
                        aria-describedby=format!("imp-drop-{card}") on:change=on_change/>
                    <label for=input_id.clone() class=PICK_LABEL>{t("imports-pick")}</label>
                    <p id=format!("imp-drop-{card}") class="text-xs text-muted-foreground">{t("imports-drop")}</p>
                </div>
            })}
        </li>
    }
}

/// The newest import in full, the earlier ones folded.
#[component]
fn Imports(list: Vec<ImportDto>, page: Page) -> impl IntoView {
    let mut list = list.into_iter();
    let Some(latest) = list.next() else {
        return view! {
            <EmptyState title=t("imports-none") icon=IconName::Upload level=3 body=t("imports-none-body")/>
        }
        .into_any();
    };
    let earlier: Vec<ImportDto> = list.collect();
    let i18n = use_i18n();
    view! {
        <ImportPanel r=latest page=page/>
        {(!earlier.is_empty()).then(|| view! {
            <h3 class="mt-4 font-semibold">{t("imports-earlier")}</h3>
            <ul class="grid gap-2" data-testid="imp-earlier">
                {earlier.into_iter().map(|r| {
                    let summary = i18n.t_args("imports-earlier-row", &[
                        ("format", t(&format!("imports-format-{}", card_of(&r.format))).into()),
                        ("when", i18n.datetime(&r.created_at).into()),
                        ("state", t(&format!("imports-state-{}", state_key(&r))).into()),
                    ]);
                    view! {
                        <li class="rounded-md border border-border">
                            <details>
                                <summary class="cursor-pointer px-3 py-2 text-sm">{summary}</summary>
                                <div class="grid gap-3 border-t border-border p-3"><ImportPanel r=r page=page/></div>
                            </details>
                        </li>
                    }
                }).collect_view()}
            </ul>
        })}
    }
    .into_any()
}

/// `dry_run` (a finished preview) or the state itself.
fn state_key(r: &ImportDto) -> &str {
    if r.dry_run && r.state == "done" {
        "previewed"
    } else {
        r.state.as_str()
    }
}

/// "x of y" (rows handled of the file's rows; the total is unknown until it is read).
#[must_use]
pub fn progress_words(i18n: &crate::i18n::I18n, r: &ImportDto) -> String {
    if r.total == 0 {
        i18n.t("imports-reading")
    } else {
        i18n.t_args(
            "imports-progress",
            &[("done", r.cursor.into()), ("total", r.total.into())],
        )
    }
}

/// One import: progress and Cancel while it runs, the preview of a dry run, the result of a
/// real run.
#[component]
fn ImportPanel(r: ImportDto, page: Page) -> impl IntoView {
    let i18n = use_i18n();
    let title = i18n.t_args(
        "imports-panel-title",
        &[
            (
                "format",
                t(&format!("imports-format-{}", card_of(&r.format))).into(),
            ),
            ("when", i18n.datetime(&r.created_at).into()),
        ],
    );
    let body = if r.active() {
        view! { <Running r=r.clone() page=page/> }.into_any()
    } else if r.dry_run && r.state == "done" {
        view! { <PreviewPanel r=r.clone() page=page/> }.into_any()
    } else {
        view! { <ResultPanel r=r.clone() page=page/> }.into_any()
    };
    let (label, state) = (title.clone(), state_key(&r).to_string());
    view! {
        <article class="grid min-w-0 gap-4 rounded-xl border border-border bg-card p-4" data-testid="imp-panel"
            data-state=state aria-label=label>
            <p class="text-sm text-muted-foreground">{title}</p>
            {body}
        </article>
    }
}

/// POST `path` (no body) and refetch; errors go to the outcome line.
fn post(page: Page, path: String) {
    let i18n = page.i18n.get_value();
    page.busy.set(true);
    leptos::task::spawn_local(async move {
        let r = rest::send("POST", &path, None).await;
        page.busy.set(false);
        page.outcome.set(
            r.err()
                .map(|e| Err(crate::settings::admin_error_with(&i18n, &e))),
        );
        page.data.refetch();
    });
}

#[component]
fn Running(r: ImportDto, page: Page) -> impl IntoView {
    let i18n = use_i18n();
    let words = progress_words(&i18n, &r);
    let cancelling = RwSignal::new(false);
    let id = r.id.clone();
    let heading = if r.dry_run {
        t("imports-checking")
    } else {
        t("imports-running")
    };
    let max = u32::try_from(r.total.max(1)).unwrap_or(u32::MAX);
    let value = u32::try_from(r.cursor).unwrap_or(u32::MAX);
    view! {
        <h3 class="font-semibold">{heading}</h3>
        <ProgressBar label=t("imports-progress-label") value=value max=max value_text=words.clone()/>
        <p class="text-sm" role="status" data-testid="imp-progress">{words}</p>
        <Button variant=ButtonVariant::Outline class="w-fit" loading=cancelling
            on:click=move |_| { cancelling.set(true); post(page, format!("/imports/{id}/cancel")); }>
            {move || if cancelling.get() { t("imports-cancelling") } else { t("imports-cancel") }}
        </Button>
    }
}

/// The dry run's preview (R-35): counts per kind, samples, mapping, what is lost; Import.
#[component]
fn PreviewPanel(r: ImportDto, page: Page) -> impl IntoView {
    let i18n = use_i18n();
    let p = r.preview.clone().unwrap_or_default();
    let rows: u64 = p.kinds.values().sum();
    let summary = i18n.t_args(
        "imports-preview-summary",
        &[
            ("rows", r.total.into()),
            ("events", p.events.into()),
            ("unmatched", p.unmatched.into()),
            ("errors", r.errors.len().into()),
        ],
    );
    let id = r.id.clone();
    let run = i18n.t_args("imports-run", &[("count", rows.into())]);
    view! {
        <h3 class="font-semibold">{t("imports-preview")}</h3>
        <p class="text-sm" data-testid="imp-summary">{summary}</p>
        <Kinds p=p.clone()/>
        <Samples p=p.clone()/>
        <Mapping p=p.clone()/>
        <LostList p=p/>
        <Errors r=r.clone()/>
        <div class="flex flex-wrap gap-2">
            <Button disabled=Signal::derive(move || page.busy.get() || rows == 0)
                on:click=move |_| post(page, format!("/imports/{id}/run"))>{run}</Button>
        </div>
    }
}

#[component]
fn Kinds(p: PreviewDto) -> impl IntoView {
    view! {
        <table class="w-full max-w-md text-sm" data-testid="imp-kinds">
            <caption class="mb-1 text-left font-medium">{t("imports-kinds")}</caption>
            <thead><tr class="border-b border-border text-left">
                <th scope="col" class="py-1 font-medium">{t("imports-kind")}</th>
                <th scope="col" class="py-1 text-right font-medium">{t("imports-rows")}</th>
            </tr></thead>
            <tbody>
                {p.kinds.into_iter().map(|(kind, n)| view! {
                    <tr class="border-b border-border">
                        <td class="py-1">{kind_label(&kind)}</td>
                        <td class="py-1 text-right tabular-nums">{n.to_string()}</td>
                    </tr>
                }).collect_view()}
            </tbody>
        </table>
        {(!p.lists.is_empty()).then(|| view! {
            <p class="text-sm">{use_i18n().t_args("imports-lists", &[("lists", p.lists.join(", ").into())])}</p>
        })}
    }
}

/// A library status of a sample row; a status the file names but the app does not know
/// shows as written (UX3-21).
fn status_word(i: &crate::i18n::I18n, v: &str) -> String {
    let key = format!("status-{v}");
    let text = i.t(&key);
    if text == key { v.to_string() } else { text }
}

#[component]
fn Samples(p: PreviewDto) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <div class="grid gap-1">
            <h4 class="text-sm font-medium">{t("imports-samples")}</h4>
            <ul class="grid gap-1 text-sm" data-testid="imp-samples">
                {p.samples.into_iter().map(|s| {
                    let status = s.status.as_deref().map_or_else(|| t("imports-no-status"), |v| status_word(&i18n, v));
                    let line = i18n.t_args("imports-sample", &[
                        ("kind", kind_label(&s.kind).into()),
                        ("status", status.into()),
                        ("events", s.events.into()),
                    ]);
                    view! {
                        <li class="flex flex-wrap items-baseline gap-x-2">
                            <span class="font-medium">{s.title}</span>
                            <span class="text-muted-foreground">{line}</span>
                            {s.unmatched.then(|| view! { <Badge variant=BadgeVariant::Warning>{t("imports-no-match")}</Badge> })}
                        </li>
                    }
                }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
fn Mapping(p: PreviewDto) -> impl IntoView {
    view! {
        <table class="w-full text-sm" data-testid="imp-mapping">
            <caption class="mb-1 text-left font-medium">{t("imports-mapping")}</caption>
            <thead><tr class="border-b border-border text-left">
                <th scope="col" class="py-1 font-medium">{t("imports-from")}</th>
                <th scope="col" class="py-1 font-medium">{t("imports-to")}</th>
            </tr></thead>
            <tbody>
                {p.mapping.into_iter().map(|m| view! {
                    <tr class="border-b border-border">
                        // the source's own field names, as in the file
                        <td class="break-words py-1 font-mono text-xs" lang="en">{m.from}</td>
                        <td class="py-1">{t(&format!("imports-target-{}", m.to))}</td>
                    </tr>
                }).collect_view()}
            </tbody>
        </table>
    }
}

#[component]
fn LostList(p: PreviewDto) -> impl IntoView {
    let i18n = use_i18n();
    let body = if p.lost.is_empty() {
        view! { <p class="text-sm">{t("imports-lost-nothing")}</p> }.into_any()
    } else {
        view! {
            <ul class="list-disc pl-5 text-sm" data-testid="imp-lost">
                {p.lost.into_iter().map(|l| {
                    let key = format!("imports-lost-{}", l.code);
                    let text = i18n.t_args(&key, &[("count", l.count.into())]);
                    let text = if text == key { format!("{} ({})", l.code, l.count) } else { text };
                    view! { <li>{text}</li> }
                }).collect_view()}
            </ul>
        }
        .into_any()
    };
    view! {
        <div class="grid gap-1">
            <h4 class="text-sm font-medium">{t("imports-lost")}</h4>
            {body}
        </div>
    }
}

#[component]
fn Errors(r: ImportDto) -> impl IntoView {
    (!r.errors.is_empty()).then(|| {
        let n = r.errors.len();
        view! {
            <details class="text-sm" data-testid="imp-errors">
                <summary class="cursor-pointer">{use_i18n().t_args("imports-errors", &[("count", n.into())])}</summary>
                <ul class="mt-1 grid gap-1">
                    {r.errors.into_iter().map(|e| {
                        let line = use_i18n().t_args("imports-error-line", &[("line", e.line.into())]);
                        view! {
                            <li><span class="font-medium">{line}</span>" "
                                // the importer's reason, as it said it (not translated)
                                <span lang="en" class="break-words font-mono text-xs">{e.message}</span></li>
                        }
                    }).collect_view()}
                </ul>
            </details>
        }
    })
}

/// A real run's result: imported, skipped, unmatched (kept, matched through the palette),
/// Try again.
#[component]
fn ResultPanel(r: ImportDto, page: Page) -> impl IntoView {
    let i18n = use_i18n();
    let heading = t(&format!("imports-state-{}", state_key(&r)));
    let counts = i18n.t_args(
        "imports-result",
        &[
            ("imported", r.imported.into()),
            ("skipped", r.errors.len().into()),
            ("unmatched", r.unmatched.len().into()),
        ],
    );
    let again = !r.errors.is_empty() || !r.unmatched.is_empty() || r.state != "done";
    let id = r.id.clone();
    let import_id = r.id.clone();
    view! {
        <h3 class="font-semibold">{heading}</h3>
        <p class="text-sm" data-testid="imp-result">{counts}</p>
        {(r.state == "cancelled").then(|| view! {
            <p class="text-sm">{use_i18n().t_args("imports-stopped-at", &[("done", r.cursor.into()), ("total", r.total.into())])}</p>
        })}
        {(!r.unmatched.is_empty()).then(|| view! {
            <div class="grid gap-2">
                <h4 class="text-sm font-medium">{t("imports-unmatched")}</h4>
                <p class="text-sm text-muted-foreground">{t("imports-unmatched-body")}</p>
                <ul class="grid gap-2" data-testid="imp-unmatched">
                    {r.unmatched.clone().into_iter().map(|u| view! {
                        <UnmatchedRow u=u import_id=import_id.clone() page=page/>
                    }).collect_view()}
                </ul>
            </div>
        })}
        <Errors r=r.clone()/>
        {again.then(|| view! {
            <Button variant=ButtonVariant::Outline class="w-fit" disabled=page.busy
                on:click=move |_| post(page, format!("/imports/{id}/run"))>{t("imports-again")}</Button>
        })}
    }
}

#[component]
fn UnmatchedRow(u: UnmatchedDto, import_id: String, page: Page) -> impl IntoView {
    let i18n = use_i18n();
    let matching = expect_context::<crate::palette::PaletteMatch>().0;
    let open = expect_context::<crate::palette::PaletteOpen>().0;
    let title = u.row.title.clone();
    let meta = match u.row.year {
        Some(y) => format!("{} {y}", kind_label(&u.row.kind)),
        None => kind_label(&u.row.kind),
    };
    let label = i18n.t_args("imports-find-named", &[("title", title.clone().into())]);
    let find = move |_| {
        let i18n = page.i18n.get_value();
        matching.set(Some(crate::palette::MatchTarget {
            import_id: import_id.clone(),
            source_id: u.row.source_id.clone(),
            title: title.clone(),
            done: Callback::new(move |err: Option<String>| {
                page.outcome
                    .set(Some(err.map_or_else(|| Ok(i18n.t("imports-matched")), Err)));
                page.data.refetch();
            }),
        }));
        open.set(true);
    };
    view! {
        <li class="flex flex-wrap items-center justify-between gap-2 rounded-md border border-border px-3 py-2">
            <span class="min-w-0">
                <span class="block break-words text-sm font-medium">{u.row.title.clone()}</span>
                <span class="block text-xs text-muted-foreground">{meta}</span>
            </span>
            <Button size=ButtonSize::Sm variant=ButtonVariant::Outline attr:aria-label=label
                on:click=find>{t("imports-find")}</Button>
        </li>
    }
}

/// Export: one download per format, each with what it keeps.
#[component]
fn Export() -> impl IntoView {
    view! {
        <section id="export" aria-labelledby="imp-export-h" class="grid gap-3">
            <h2 id="imp-export-h" class="text-lg font-semibold">{t("imports-export")}</h2>
            <p class="text-sm text-muted-foreground">{t("imports-export-lead")}</p>
            <ul class="grid gap-3 sm:grid-cols-2">
                <li class="grid content-start gap-2 rounded-xl border border-border bg-card p-4">
                    <a href="/api/v1/export?format=jsonl" download="library-export.jsonl" class=LINK_BUTTON
                        data-testid="imp-export-jsonl">{t("imports-export-jsonl")}</a>
                    <p class="text-sm text-muted-foreground">{t("imports-export-jsonl-body")}</p>
                </li>
                <li class="grid content-start gap-2 rounded-xl border border-border bg-card p-4">
                    <a href="/api/v1/export?format=csv" download="library-export.csv" class=LINK_BUTTON
                        data-testid="imp-export-csv">{t("imports-export-csv")}</a>
                    <p class="text-sm text-muted-foreground">{t("imports-export-csv-body")}</p>
                </li>
            </ul>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UX3-21: a status the file names but the app does not know shows as written, not as a
    /// message key.
    #[test]
    fn unknown_statuses_show_as_written() {
        let i = crate::i18n::I18n::new("en", std::collections::BTreeMap::new());
        assert_eq!(status_word(&i, "completed"), "Completed");
        assert_eq!(status_word(&i, "on_hold"), "on_hold");
    }

    #[test]
    fn files_pick_their_format_by_card_and_ending() {
        assert_eq!(format_for("generic", "lib.CSV"), Some("generic-csv"));
        assert_eq!(format_for("generic", "lib.jsonl"), Some("generic-jsonl"));
        assert_eq!(format_for("legacy", "export.json"), Some("legacy-json"));
        assert_eq!(format_for("trakt", "history.json"), Some("trakt-json"));
        assert_eq!(format_for("goodreads", "x.csv"), None);
        assert_eq!(card_of("generic-csv"), "generic");
        assert_eq!(card_of("trakt-json"), "trakt");
        let ready: Vec<_> = CARDS.iter().filter(|(_, r)| *r).map(|(c, _)| *c).collect();
        assert_eq!(ready, ["generic", "legacy", "trakt"]);
    }

    #[test]
    fn a_finished_dry_run_reads_as_previewed() {
        let mut r = ImportDto {
            dry_run: true,
            state: "done".into(),
            ..ImportDto::default()
        };
        assert_eq!(state_key(&r), "previewed");
        r.dry_run = false;
        assert_eq!(state_key(&r), "done");
        r.state = "running".into();
        assert!(r.active());
    }
}
