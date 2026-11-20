//! "Add by hand" and "Edit details" (v1-manual, v1-plan.md 4a): one dialog for an item no
//! source knows. The palette opens it for a new item (title from the search, kind from the
//! scope); it lists items of the same kind that already have the title ("Already here"),
//! creates the item, adds it to the library and opens its page. The item page opens it to
//! correct title, year and picture address. The picture address is loaded by the browsers
//! that show the item, never by the server. Mounted once, in the shell.

use contracts::{CatalogItemDto, ErrorDto, KindsDto};
use leptos::prelude::*;
use serde_json::json;
use ui::{Button, ButtonVariant, FormDialog, FormField, Input, Select, SelectOption};

use crate::i18n::{kind_label, t, title_text, use_i18n};
use crate::rest;

/// What the dialog is open for.
#[derive(Clone)]
enum Target {
    /// A new item: the palette's query and scope (`all` or a kind group).
    New { title: String, scope: String },
    /// The details of an item; `done` runs after the save (the item page refetches).
    Edit {
        item: Box<CatalogItemDto>,
        /// A source answers for the item: title and year only, the cover follows the source
        /// (#16).
        sourced: bool,
        done: Callback<()>,
    },
}

/// The dialog's handle, in the page context: the palette and the item page open it.
#[derive(Clone, Copy)]
pub struct ManualItem {
    open: RwSignal<bool>,
    target: RwSignal<Option<Target>>,
}

impl Default for ManualItem {
    fn default() -> Self {
        Self::new()
    }
}

impl ManualItem {
    #[must_use]
    pub fn new() -> Self {
        Self {
            open: RwSignal::new(false),
            target: RwSignal::new(None),
        }
    }

    /// "Add ... by hand": `title` is what was searched for, `scope` the palette's scope.
    pub fn add(self, title: &str, scope: &str) {
        self.target.set(Some(Target::New {
            title: title.trim().to_string(),
            scope: scope.to_string(),
        }));
        self.show();
    }

    /// Open on the next turn of the event loop: what opened the dialog (the palette, the
    /// item page's menu) closes first and hands the focus back to its own opener, and that
    /// is where this dialog returns the focus when it closes. Opened in the same turn, it
    /// would remember a row or a menu item that is gone by then.
    fn show(self) {
        set_timeout(
            move || {
                let _ = self.open.try_set(true);
            },
            std::time::Duration::ZERO,
        );
    }

    /// "Edit details" of `item`.
    pub fn edit(self, item: CatalogItemDto, sourced: bool, done: Callback<()>) {
        self.target.set(Some(Target::Edit {
            item: Box::new(item),
            sourced,
            done,
        }));
        self.show();
    }
}

/// A field of the form (the DOM id of its input is [`Field::id`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Kind,
    Year,
    Cover,
}

impl Field {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Title => "mi-title",
            Self::Kind => "mi-kind",
            Self::Year => "mi-year",
            Self::Cover => "mi-cover",
        }
    }
}

/// The form as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub title: String,
    pub kind: String,
    pub year: String,
    pub cover: String,
}

/// The form as sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub title: String,
    pub kind: String,
    pub year: Option<i32>,
    pub cover_url: Option<String>,
}

/// The longest title the server takes (`app::MAX_TITLE_CHARS`).
const MAX_TITLE_CHARS: usize = 500;

/// What can be said without asking the server: the first field that is wrong and the key
/// of its message. `need_kind`: a new item (an edit keeps the kind). The server checks
/// again and knows more (the latest year, the exact rule of an address).
///
/// # Errors
/// The field and the message key.
pub fn check(draft: &Draft, need_kind: bool) -> Result<Checked, (Field, &'static str)> {
    let title = draft.title.trim();
    if title.is_empty() {
        return Err((Field::Title, "manual-error-title"));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err((Field::Title, "manual-error-title-long"));
    }
    let kind = draft.kind.trim();
    if need_kind && kind.is_empty() {
        return Err((Field::Kind, "manual-error-kind"));
    }
    let year = match draft.year.trim() {
        "" => None,
        y => Some(
            y.parse::<i32>()
                .ok()
                .filter(|y| *y >= 1)
                .ok_or((Field::Year, "manual-error-year"))?,
        ),
    };
    let cover = draft.cover.trim();
    let https = cover
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case("https://"));
    if !cover.is_empty() && (!https || cover.len() == 8 || cover.contains(char::is_whitespace)) {
        return Err((Field::Cover, "manual-error-cover"));
    }
    Ok(Checked {
        title: title.to_string(),
        kind: kind.to_string(),
        year,
        cover_url: (!cover.is_empty()).then(|| cover.to_string()),
    })
}

/// The server's refusal as the field it is about (none: a line under the form) and the key
/// of its message. `edit`: a 403 is about the whole edit, not about the picture address.
#[must_use]
pub fn refusal(e: &ErrorDto, edit: bool) -> (Option<Field>, &'static str) {
    match (e.code.as_str(), e.field.as_deref()) {
        ("invalid", Some("title")) => (Some(Field::Title), "manual-error-title"),
        ("invalid", Some("kind")) => (Some(Field::Kind), "manual-error-kind"),
        ("invalid", Some("year")) => (Some(Field::Year), "manual-error-year"),
        ("invalid", Some("cover_url")) => (Some(Field::Cover), "manual-error-cover"),
        ("forbidden", _) if edit => (None, "manual-error-forbidden"),
        ("forbidden", _) => (Some(Field::Cover), "manual-error-cover-forbidden"),
        // a new item: items of that title are there (they are listed under the form)
        ("conflict", Some("title")) if !edit => (Some(Field::Title), "manual-error-duplicate"),
        ("conflict", _) => (None, "manual-error-sourced"),
        _ => (None, "error-generic"),
    }
}

/// The kinds of the select and the one chosen when the dialog opens. With a scope that is a
/// kind group, the group's kinds lead; a kind is chosen only when the group holds exactly
/// one enabled kind. Otherwise none is (the select starts with "Choose a kind..."): a film
/// added as a board game, or a comic as a book, by default would be an accident, and the
/// kind of an item cannot be changed afterwards (review MAN-1).
#[must_use]
pub fn kind_choice(kinds: &KindsDto, scope: &str) -> (Vec<String>, String) {
    let in_scope = |group: &str| group == scope;
    let mut ids: Vec<String> = kinds
        .kinds
        .iter()
        .filter(|k| in_scope(&k.group))
        .map(|k| k.id.clone())
        .collect();
    let chosen = match ids.as_slice() {
        [only] => only.clone(),
        _ => String::new(),
    };
    ids.extend(
        kinds
            .kinds
            .iter()
            .filter(|k| !in_scope(&k.group))
            .map(|k| k.id.clone()),
    );
    (ids, chosen)
}

/// What "Add to library" does for a new item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewStep {
    /// The item was made by an earlier press; only the add to the library is repeated.
    AddOnly(String),
    /// `POST /items`; `allow_duplicate` once the server named the items of that title.
    Create { allow_duplicate: bool },
}

/// The kind and title as the server compares them for "Already here".
#[must_use]
pub fn duplicate_key(kind: &str, title: &str) -> (String, String) {
    (kind.to_string(), title.trim().to_lowercase())
}

/// The next press of "Add to library" for a new item (review MAN-2). `created`: the item an
/// earlier press made (its add to the library failed): only the add is repeated, never a
/// second `POST /items`. `refused_for`: the kind and title the server refused because items
/// of that title exist and named them; pressing again for the same kind and title is the
/// person's choice of a second one. "Already here" may also be shown before the first press
/// (it is asked as the title is typed), but the server decides, so a fast Enter cannot skip
/// it.
#[must_use]
pub fn new_item_step(
    created: Option<&str>,
    refused_for: Option<&(String, String)>,
    checked: &Checked,
) -> NewStep {
    if let Some(id) = created {
        return NewStep::AddOnly(id.to_string());
    }
    NewStep::Create {
        allow_duplicate: refused_for == Some(&duplicate_key(&checked.kind, &checked.title)),
    }
}

/// The names of the "Already here" rows: two items of the same title and year get a number
/// each, so their rows and "Use this one" buttons can be told apart (review MAN-7).
#[must_use]
pub fn distinct_labels(labels: Vec<String>) -> Vec<String> {
    let total = |l: &String| labels.iter().filter(|o| *o == l).count();
    let mut seen: std::collections::HashMap<&String, usize> = std::collections::HashMap::new();
    let mut out = Vec::with_capacity(labels.len());
    for l in &labels {
        if total(l) > 1 {
            let n = seen.entry(l).or_default();
            *n += 1;
            out.push(format!("{l} #{n}"));
        } else {
            out.push(l.clone());
        }
    }
    out
}

/// After a saved edit: the item page refetches. The page owns `done`; when the person left it
/// while the save was on its way (browser Back), it is gone and nothing is to be refetched
/// (review MAN-11: `run` panics on a disposed callback and stops the app).
fn after_edit(done: Callback<()>) {
    let _ = done.try_run(());
}

/// A person corrected the title or the year of the item (`user` provenance, #16).
fn corrected_fields(item: &CatalogItemDto) -> bool {
    ["title", "year"]
        .iter()
        .any(|f| item.provenance.get(*f).map(String::as_str) == Some("user"))
}

/// "Use the source's values": the corrections go back to the source, which refreshes the item.
fn use_source(form: Form, handle: ManualItem) {
    let Some(Target::Edit { item, done, .. }) = handle.target.get_untracked() else {
        return;
    };
    if form.busy.get_untracked() {
        return;
    }
    form.busy.set(true);
    leptos::task::spawn_local(async move {
        let path = format!("/items/{}/corrections", item.id);
        match rest::call_dto::<CatalogItemDto>("DELETE", &path, None).await {
            Ok(_) => {
                handle.open.set(false);
                form.busy.set(false);
                after_edit(done);
            }
            Err(e) => form.refuse(&e, true),
        }
    });
}

/// The path of the "Already here" read.
#[must_use]
pub fn titled_path(kind: &str, title: &str) -> String {
    format!(
        "/items/titled?kind={}&title={}",
        crate::library_state::encode(kind),
        crate::library_state::encode(title.trim())
    )
}

/// The dialog's state.
#[derive(Clone, Copy)]
struct Form {
    title: RwSignal<String>,
    kind: RwSignal<String>,
    year: RwSignal<String>,
    cover: RwSignal<String>,
    kinds: RwSignal<Vec<String>>,
    /// The field that is wrong and its message.
    field_error: RwSignal<Option<(Field, String)>>,
    /// An error that is about no field.
    error: RwSignal<Option<String>>,
    busy: RwSignal<bool>,
    /// Items of the same kind that already have the title.
    existing: RwSignal<Vec<CatalogItemDto>>,
    /// The item an earlier press made (its add to the library failed): see [`new_item_step`].
    created: RwSignal<Option<String>>,
    /// The kind and title the server refused as already there: see [`new_item_step`].
    refused_for: RwSignal<Option<(String, String)>>,
}

impl Form {
    fn new() -> Self {
        Self {
            title: RwSignal::new(String::new()),
            kind: RwSignal::new(String::new()),
            year: RwSignal::new(String::new()),
            cover: RwSignal::new(String::new()),
            kinds: RwSignal::new(Vec::new()),
            field_error: RwSignal::new(None),
            error: RwSignal::new(None),
            busy: RwSignal::new(false),
            existing: RwSignal::new(Vec::new()),
            created: RwSignal::new(None),
            refused_for: RwSignal::new(None),
        }
    }

    fn draft(self) -> Draft {
        Draft {
            title: self.title.get_untracked(),
            kind: self.kind.get_untracked(),
            year: self.year.get_untracked(),
            cover: self.cover.get_untracked(),
        }
    }

    /// The fields as the dialog opens for `target`.
    fn reset(self, target: &Target) {
        self.field_error.set(None);
        self.error.set(None);
        self.busy.set(false);
        self.existing.set(Vec::new());
        self.created.set(None);
        self.refused_for.set(None);
        match target {
            Target::New { title, .. } => {
                self.title.set(title.clone());
                self.kind.set(String::new());
                self.year.set(String::new());
                self.cover.set(String::new());
            }
            Target::Edit { item, .. } => {
                self.title.set(item.title.clone());
                self.kind.set(item.kind.clone());
                self.year
                    .set(item.year.map(|y| y.to_string()).unwrap_or_default());
                // only an address a person typed: a stored picture or a source's address is
                // not theirs to edit, and saving the form unchanged must not claim it
                let typed = item.provenance.get("cover_url").map(String::as_str) == Some("user");
                self.cover.set(
                    item.cover
                        .as_ref()
                        .filter(|_| typed)
                        .map(|c| c.url.clone())
                        .unwrap_or_default(),
                );
            }
        }
    }

    fn refuse(self, e: &ErrorDto, edit: bool) {
        let (field, key) = refusal(e, edit);
        self.busy.set(false);
        match field {
            Some(f) => {
                self.field_error.set(Some((f, t(key))));
                // as the client's own check does: the field may be out of view at 360 px
                // (review MAN-7)
                let _ = crate::keyboard::focus(&format!("#{}", f.id()));
            }
            None => self.error.set(Some(t(key))),
        }
    }

    fn message(self, field: Field) -> Signal<Option<String>> {
        let errors = self.field_error;
        Signal::derive(move || {
            errors.with(|e| {
                e.as_ref()
                    .filter(|(f, _)| *f == field)
                    .map(|(_, m)| m.clone())
            })
        })
    }
}

/// The kinds of the select, read when the dialog opens for a new item.
fn load_kinds(form: Form, scope: String) {
    leptos::task::spawn_local(async move {
        if let Ok(kinds) = crate::api::kinds().await {
            let (ids, chosen) = kind_choice(&kinds, &scope);
            form.kinds.set(ids);
            // not over a choice the person made while the kinds were on their way
            if form.kind.with_untracked(String::is_empty) {
                form.kind.set(chosen);
            }
        }
    });
}

/// "Already here": 300 ms after the last change of title or kind, the items of that kind
/// with that title; a newer question wins.
fn watch_existing(form: Form, open: RwSignal<bool>, is_new: Signal<bool>) {
    let generation = StoredValue::new(0u64);
    Effect::new(move |_| {
        let (title, kind) = (form.title.get(), form.kind.get());
        let asked = open.get() && is_new.get();
        let g = generation.get_value() + 1;
        generation.set_value(g);
        if !asked || title.trim().is_empty() || kind.is_empty() {
            form.existing.set(Vec::new());
            return;
        }
        set_timeout(
            move || {
                if generation.try_get_value() != Some(g) {
                    return;
                }
                leptos::task::spawn_local(async move {
                    let found =
                        rest::call::<Vec<CatalogItemDto>>("GET", &titled_path(&kind, &title), None)
                            .await;
                    if generation.try_get_value() == Some(g) {
                        form.existing.set(found.ok().flatten().unwrap_or_default());
                    }
                });
            },
            std::time::Duration::from_millis(300),
        );
    });
}

/// Put the item into the library (a no-op when it is there) and open its page.
async fn add_and_open(
    form: Form,
    open: RwSignal<bool>,
    item_id: String,
    navigate: impl Fn(&str, leptos_router::NavigateOptions),
    acted: crate::next_step::StepCtx,
) {
    match crate::api::add_to_library(item_id.clone()).await {
        Ok(_) => {
            acted.funnel.logged();
            acted.acted();
            open.set(false);
            navigate(
                &format!("/items/{item_id}"),
                leptos_router::NavigateOptions::default(),
            );
        }
        Err(e) => {
            form.error.set(Some(crate::pages::error_text(&e)));
            form.busy.set(false);
        }
    }
}

/// The save of the form: checks, then `POST /items` and the add to the library, or
/// `PATCH /items/{id}`.
fn submit(
    form: Form,
    handle: ManualItem,
    navigate: impl Fn(&str, leptos_router::NavigateOptions) + Clone + 'static,
    ctx: crate::next_step::StepCtx,
) {
    if form.busy.get_untracked() {
        return;
    }
    let Some(target) = handle.target.get_untracked() else {
        return;
    };
    let is_new = matches!(target, Target::New { .. });
    form.error.set(None);
    let checked = match check(&form.draft(), is_new) {
        Ok(c) => c,
        Err((field, key)) => {
            form.field_error.set(Some((field, t(key))));
            let _ = crate::keyboard::focus(&format!("#{}", field.id()));
            return;
        }
    };
    form.field_error.set(None);
    form.busy.set(true);
    leptos::task::spawn_local(async move {
        match target {
            Target::New { .. } => create(form, handle, checked, navigate, ctx).await,
            Target::Edit { item, done, .. } => {
                let body = json!({
                    "title": checked.title, "year": checked.year, "cover_url": checked.cover_url,
                });
                let path = format!("/items/{}", item.id);
                match rest::call_dto::<CatalogItemDto>("PATCH", &path, Some(body)).await {
                    Ok(_) => {
                        handle.open.set(false);
                        form.busy.set(false);
                        after_edit(done);
                    }
                    Err(e) => form.refuse(&e, true),
                }
            }
        }
    });
}

/// A new item: `POST /items` (refused once when items of the title are there, which are
/// then listed), then the add to the library and the item's page. A retry after a failed
/// add only repeats the add ([`new_item_step`]).
async fn create(
    form: Form,
    handle: ManualItem,
    checked: Checked,
    navigate: impl Fn(&str, leptos_router::NavigateOptions),
    ctx: crate::next_step::StepCtx,
) {
    let created = form.created.get_untracked();
    let refused_for = form.refused_for.get_untracked();
    let allow_duplicate = match new_item_step(created.as_deref(), refused_for.as_ref(), &checked) {
        NewStep::AddOnly(id) => return add_and_open(form, handle.open, id, navigate, ctx).await,
        NewStep::Create { allow_duplicate } => allow_duplicate,
    };
    let body = json!({
        "kind": checked.kind, "title": checked.title, "year": checked.year,
        "cover_url": checked.cover_url, "allow_duplicate": allow_duplicate,
    });
    match rest::call_dto::<CatalogItemDto>("POST", "/items", Some(body)).await {
        Ok(Some(item)) => {
            form.created.set(Some(item.id.clone()));
            add_and_open(form, handle.open, item.id, navigate, ctx).await;
        }
        Ok(None) => form.refuse(&generic(), false),
        Err(e) => {
            if refusal(&e, false).1 == "manual-error-duplicate" {
                // the items the server found, under the form; the next press is a choice
                let path = titled_path(&checked.kind, &checked.title);
                let found = rest::call::<Vec<CatalogItemDto>>("GET", &path, None).await;
                form.existing.set(found.ok().flatten().unwrap_or_default());
                form.refused_for
                    .set(Some(duplicate_key(&checked.kind, &checked.title)));
            }
            form.refuse(&e, false);
        }
    }
}

fn generic() -> ErrorDto {
    ErrorDto {
        code: "generic".into(),
        message: String::new(),
        retry_after_secs: None,
        field: None,
    }
}

const FORM_ID: &str = "mi-form";

/// The dialog. Mounted once in the shell of a signed-in page.
#[component]
pub fn ManualItemDialog() -> impl IntoView {
    let handle = expect_context::<ManualItem>();
    let i18n = use_i18n();
    let form = Form::new();
    let is_new = Signal::derive(move || {
        handle
            .target
            .with(|t| matches!(t, Some(Target::New { .. })))
    });
    // an item a source answers for: title and year are corrected, the cover stays (#16)
    let sourced = Signal::derive(move || {
        handle
            .target
            .with(|t| matches!(t, Some(Target::Edit { sourced: true, .. })))
    });
    // ... and when a person corrected one of them, "Use the source's values" gives them back
    let corrected = Signal::derive(move || {
        handle.target.with(|t| match t {
            Some(Target::Edit {
                item,
                sourced: true,
                ..
            }) => corrected_fields(item),
            _ => false,
        })
    });
    // the fields are set when the dialog opens, not while it is open
    Effect::new(move |was: Option<bool>| {
        let open = handle.open.get();
        if open
            && was != Some(true)
            && let Some(target) = handle.target.get_untracked()
        {
            form.reset(&target);
            if let Target::New { scope, .. } = target {
                load_kinds(form, scope);
            }
        }
        open
    });
    watch_existing(form, handle.open, is_new);
    let (add_title, edit_title) = (i18n.t("manual-add-title"), i18n.t("manual-edit-title"));
    let title = Signal::derive(move || {
        if is_new.get() {
            add_title.clone()
        } else {
            edit_title.clone()
        }
    });
    let lead = i18n.t("manual-add-lead");
    let description = Signal::derive(move || is_new.get().then(|| lead.clone()));
    let navigate = leptos_router::hooks::use_navigate();
    let ctx = crate::next_step::StepCtx::current();
    let save = {
        let (navigate, ctx) = (navigate.clone(), ctx.clone());
        Callback::new(move |()| submit(form, handle, navigate.clone(), ctx.clone()))
    };
    let use_existing = Callback::new(move |id: String| {
        if form.busy.get_untracked() {
            return;
        }
        form.busy.set(true);
        let (navigate, ctx) = (navigate.clone(), ctx.clone());
        leptos::task::spawn_local(async move {
            add_and_open(form, handle.open, id, navigate, ctx).await;
        });
    });
    let footer = move || {
        view! {
            <div class="grid gap-3">
                {move || form.error.get().map(|e| view! {
                    <p role="alert" class="text-sm text-destructive-text" data-testid="mi-error">{e}</p>
                })}
                <div class="flex flex-wrap items-center justify-end gap-2">
                    <Show when=move || corrected.get()>
                        <Button variant=ButtonVariant::Ghost class="mr-auto max-sm:h-11 max-sm:w-full" attr:data-testid="mi-use-source"
                            on:click=move |_| use_source(form, handle)>
                            {t("manual-use-source")}
                        </Button>
                    </Show>
                    <Button variant=ButtonVariant::Outline class="max-sm:hidden" on:click=move |_| handle.open.set(false)>
                        {t("log-cancel")}
                    </Button>
                    <Button r#type="submit" attr:form=FORM_ID loading=form.busy class="max-sm:h-11 max-sm:w-full">
                        {move || if is_new.get() { t("manual-submit-add") } else { t("manual-submit-edit") }}
                    </Button>
                </div>
            </div>
        }
    };
    view! {
        <FormDialog open=handle.open title=title close_label=i18n.t("dialog-close") description=description
            initial_focus=Field::Title.id() footer=footer>
            <form id=FORM_ID class="grid gap-4" novalidate on:submit=move |ev| { ev.prevent_default(); save.run(()); }>
                <Fields form=form is_new=is_new sourced=sourced/>
                <Existing form=form use_existing=use_existing/>
            </form>
        </FormDialog>
    }
}

#[component]
fn Fields(form: Form, is_new: Signal<bool>, sourced: Signal<bool>) -> impl IntoView {
    let options = Signal::derive(move || {
        let mut out = vec![SelectOption {
            value: String::new(),
            label: t("manual-kind-choose"),
        }];
        out.extend(form.kinds.get().into_iter().map(|id| SelectOption {
            label: kind_label(&id),
            value: id,
        }));
        out
    });
    let wrong = move |f: Field| Signal::derive(move || form.message(f).get().is_some());
    // only ids that are in the page: the hint when the field has one, the error while shown
    let described = move |f: Field, hint: bool| {
        Signal::derive(move || {
            let mut ids = Vec::new();
            if hint {
                ids.push(format!("{}-description", f.id()));
            }
            if form.message(f).get().is_some() {
                ids.push(format!("{}-error", f.id()));
            }
            (!ids.is_empty()).then(|| ids.join(" "))
        })
    };
    let (kind_wrong, kind_described) = (wrong(Field::Kind), described(Field::Kind, false));
    view! {
        <FormField id=Field::Title.id() label=t("manual-field-title") error=form.message(Field::Title)>
            <Input id=Field::Title.id() value=form.title autocomplete="off" invalid=wrong(Field::Title)
                described_by=described(Field::Title, false)/>
        </FormField>
        <Show when=move || is_new.get()>
            <FormField id=Field::Kind.id() label=t("manual-field-kind") error=form.message(Field::Kind)>
                // tied to its error like the inputs (review MAN-7)
                <Select id=Field::Kind.id() value=form.kind options=options
                    attr:aria-invalid=move || kind_wrong.get().then_some("true")
                    attr:aria-describedby=move || kind_described.get()/>
            </FormField>
        </Show>
        <FormField id=Field::Year.id() label=t("manual-field-year") description=t("manual-field-year-hint")
            error=form.message(Field::Year)>
            <Input id=Field::Year.id() value=form.year inputmode="numeric" autocomplete="off" class="max-w-32"
                invalid=wrong(Field::Year) described_by=described(Field::Year, true)/>
        </FormField>
        <Show when=move || !sourced.get()
            fallback=|| view! { <p class="text-sm text-muted-foreground" data-testid="mi-sourced-note">{t("manual-sourced-note")}</p> }>
            <FormField id=Field::Cover.id() label=t("manual-field-cover") description=t("manual-field-cover-hint")
                error=form.message(Field::Cover)>
                <Input id=Field::Cover.id() value=form.cover r#type="url" inputmode="url" autocomplete="off"
                    invalid=wrong(Field::Cover) described_by=described(Field::Cover, true)/>
            </FormField>
        </Show>
    }
}

/// "Already here": the items of that kind with that title, each with "Use this one" (it is
/// added to the library and its page opens). Announced when it appears.
#[component]
fn Existing(form: Form, use_existing: Callback<String>) -> impl IntoView {
    view! {
        <div role="status">
            {move || {
                let items = form.existing.get();
                (!items.is_empty()).then(|| {
                    let labels = distinct_labels(items.iter().map(|item| {
                        let title = title_text(&item.title);
                        match item.year {
                            Some(y) => format!("{title} ({y})"),
                            None => title,
                        }
                    }).collect());
                    let rows = items.into_iter().zip(labels).map(|(item, label)| {
                        let name = use_i18n().t_args("manual-existing-use-named", &[("title", label.clone().into())]);
                        let id = item.id.clone();
                        view! {
                            <li class="flex flex-wrap items-center justify-between gap-2">
                                <span class="min-w-0 break-words text-sm">{label}</span>
                                <Button variant=ButtonVariant::Outline size=ui::ButtonSize::Sm disabled=form.busy
                                    attr:aria-label=name on:click=move |_| use_existing.run(id.clone())>
                                    {t("manual-existing-use")}
                                </Button>
                            </li>
                        }
                    }).collect_view();
                    view! {
                        <section data-testid="mi-existing" aria-labelledby="mi-existing-h" class="grid gap-2 rounded-lg border border-border bg-card p-3">
                            <h3 id="mi-existing-h" class="text-sm font-semibold">{t("manual-existing-title")}</h3>
                            <ul class="grid gap-2">{rows}</ul>
                        </section>
                    }
                })
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use contracts::KindDto;

    use super::*;

    fn draft(title: &str, kind: &str, year: &str, cover: &str) -> Draft {
        Draft {
            title: title.into(),
            kind: kind.into(),
            year: year.into(),
            cover: cover.into(),
        }
    }

    #[test]
    fn the_form_is_checked_field_by_field_before_it_is_sent() {
        let ok = check(
            &draft(
                " Marsh ",
                "board_game",
                " 2019 ",
                " https://img.example/a.jpg ",
            ),
            true,
        )
        .unwrap();
        assert_eq!(
            ok,
            Checked {
                title: "Marsh".into(),
                kind: "board_game".into(),
                year: Some(2019),
                cover_url: Some("https://img.example/a.jpg".into()),
            }
        );
        let plain = check(&draft("Marsh", "book", "", ""), true).unwrap();
        assert_eq!((plain.year, plain.cover_url), (None, None));
        let field = |d: Draft, need_kind: bool| check(&d, need_kind).unwrap_err();
        assert_eq!(
            field(draft("  ", "book", "", ""), true),
            (Field::Title, "manual-error-title")
        );
        assert_eq!(
            field(draft(&"x".repeat(501), "book", "", ""), true),
            (Field::Title, "manual-error-title-long")
        );
        assert_eq!(
            field(draft("A", "", "", ""), true),
            (Field::Kind, "manual-error-kind")
        );
        // an edit keeps the kind and does not ask for one
        assert!(check(&draft("A", "", "", ""), false).is_ok());
        for year in ["abc", "19.5", "0", "-4", "1e3"] {
            assert_eq!(
                field(draft("A", "book", year, ""), true),
                (Field::Year, "manual-error-year"),
                "{year}"
            );
        }
        for cover in [
            "http://img.example/a.jpg",
            "img.example/a.jpg",
            "https://",
            "https://img.example/a b.jpg",
            "ftp://x",
        ] {
            assert_eq!(
                field(draft("A", "book", "", cover), true),
                (Field::Cover, "manual-error-cover"),
                "{cover}"
            );
        }
        // the first wrong field is named
        assert_eq!(field(draft("", "", "x", "y"), true).0, Field::Title);
    }

    fn refused(code: &str, field: Option<&str>) -> ErrorDto {
        ErrorDto {
            code: code.into(),
            message: String::new(),
            retry_after_secs: None,
            field: field.map(Into::into),
        }
    }

    #[test]
    fn a_refusal_of_the_server_lands_on_its_field() {
        for (field, at, key) in [
            ("title", Field::Title, "manual-error-title"),
            ("kind", Field::Kind, "manual-error-kind"),
            ("year", Field::Year, "manual-error-year"),
            ("cover_url", Field::Cover, "manual-error-cover"),
        ] {
            assert_eq!(
                refusal(&refused("invalid", Some(field)), false),
                (Some(at), key)
            );
        }
        // a member's picture address on a hosted instance; an edit there is refused whole
        assert_eq!(
            refusal(&refused("forbidden", None), false),
            (Some(Field::Cover), "manual-error-cover-forbidden")
        );
        assert_eq!(
            refusal(&refused("forbidden", None), true),
            (None, "manual-error-forbidden")
        );
        assert_eq!(
            refusal(&refused("conflict", None), true),
            (None, "manual-error-sourced")
        );
        assert_eq!(
            refusal(&refused("generic", None), false),
            (None, "error-generic")
        );
        assert_eq!(
            refusal(&refused("invalid", None), false),
            (None, "error-generic")
        );
    }

    fn kinds() -> KindsDto {
        let kind = |id: &str, group: &str| KindDto {
            id: id.into(),
            group: group.into(),
            label_key: format!("kind-{id}"),
            progress_model: "single".into(),
            statuses: vec![],
            facets: vec![],
        };
        KindsDto {
            groups: vec![],
            kinds: vec![
                kind("movie", "watch"),
                kind("show", "watch"),
                kind("book", "read"),
                kind("video_game", "play"),
                kind("board_game", "play"),
            ],
        }
    }

    #[test]
    fn the_dialog_pre_selects_the_scopes_kind() {
        // a scope with several kinds: the person chooses (review MAN-1: a comic added from
        // Read as a book by default, which "Edit details" cannot change, is an accident)
        let (ids, chosen) = kind_choice(&kinds(), "play");
        assert_eq!(chosen, "", "two kinds in Play: none is chosen");
        assert_eq!(
            ids,
            ["video_game", "board_game", "movie", "show", "book"],
            "the scope's kinds lead, every enabled kind can be chosen"
        );
        assert_eq!(kind_choice(&kinds(), "watch").1, "");
        // a scope with exactly one enabled kind: that one
        let (ids, chosen) = kind_choice(&kinds(), "read");
        assert_eq!((ids[0].as_str(), chosen.as_str()), ("book", "book"));
        // every kind in scope: none is chosen for the person
        let (ids, chosen) = kind_choice(&kinds(), "all");
        assert_eq!(chosen, "");
        assert_eq!(ids, ["movie", "show", "book", "video_game", "board_game"]);
        // a scope without an enabled kind
        assert_eq!(kind_choice(&kinds(), "listen").1, "");
    }

    fn checked(kind: &str, title: &str) -> Checked {
        Checked {
            title: title.into(),
            kind: kind.into(),
            year: None,
            cover_url: None,
        }
    }

    #[test]
    fn a_second_item_of_a_title_is_a_choice_and_a_retry_does_not_make_one() {
        let marsh = checked("board_game", "Marsh");
        // the first press asks the server to refuse a title that is there
        assert_eq!(
            new_item_step(None, None, &marsh),
            NewStep::Create {
                allow_duplicate: false
            }
        );
        // after the server named the items of that title, the next press is the choice
        let refused_for = duplicate_key("board_game", "  marsh ");
        assert_eq!(
            new_item_step(None, Some(&refused_for), &marsh),
            NewStep::Create {
                allow_duplicate: true
            }
        );
        // another title or kind is asked again
        assert_eq!(
            new_item_step(None, Some(&refused_for), &checked("board_game", "Marshes")),
            NewStep::Create {
                allow_duplicate: false
            }
        );
        assert_eq!(
            new_item_step(None, Some(&refused_for), &checked("book", "Marsh")),
            NewStep::Create {
                allow_duplicate: false
            }
        );
        // the item was made and the add to the library failed: a retry only adds
        assert_eq!(
            new_item_step(Some("i-1"), None, &marsh),
            NewStep::AddOnly("i-1".into())
        );
        // the server's refusal of a title that is there lands on the title
        assert_eq!(
            refusal(&refused("conflict", Some("title")), false),
            (Some(Field::Title), "manual-error-duplicate")
        );
    }

    #[test]
    fn rows_of_the_same_name_are_told_apart() {
        let labels = distinct_labels(vec![
            "Catan (1995)".into(),
            "Azul".into(),
            "Catan (1995)".into(),
        ]);
        assert_eq!(labels, ["Catan (1995) #1", "Azul", "Catan (1995) #2"]);
        assert_eq!(distinct_labels(vec!["Azul".into()]), ["Azul"]);
    }

    #[test]
    fn a_save_that_answers_after_the_page_is_gone_does_not_panic() {
        // the item page owns `done`; Back while the PATCH is on its way disposes it
        // (review MAN-11)
        let owner = Owner::new();
        owner.set();
        let done = Callback::new(|()| {});
        done.dispose();
        after_edit(done);
    }

    #[test]
    fn the_lookup_of_a_title_is_encoded() {
        assert_eq!(
            titled_path("board_game", " Zug & Co? "),
            "/items/titled?kind=board_game&title=Zug%20%26%20Co%3F"
        );
    }
}
