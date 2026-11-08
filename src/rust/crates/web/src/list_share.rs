//! The share dialog of a list (design.md 5.10, 3.6; mockup `lists.html`): three choices with
//! the consequence under each; "Anyone with the link" makes the link in place with Copy; a
//! new link replaces the old one and says so; "Private" revokes the link. Changes go to
//! `PUT /lists/{id}/visibility` (`web::rest`, DEC-327).

use contracts::ListDto;
use leptos::prelude::*;
use serde_json::json;
use ui::{Button, ButtonVariant, CopyField, Dialog, Label};

use crate::i18n::{t, use_i18n};
use crate::rest;
use crate::sharing::{absolute, share_error};

/// The choices in the order the dialog shows them, with their label and consequence keys.
const CHOICES: [(&str, &str, &str); 3] = [
    ("private", "lists-share-private", "lists-share-private-hint"),
    (
        "instance",
        "lists-share-instance",
        "lists-share-instance-hint",
    ),
    ("link", "lists-share-link", "lists-share-link-hint"),
];

/// What a saved change says (`role=status`): the link revoked, replaced, made, or the new
/// choice.
#[must_use]
pub fn saved_message(before: &str, after: &str, new_link: bool) -> &'static str {
    match (before, after) {
        (_, "link") if new_link => "lists-share-replaced",
        ("link", "link") => "lists-share-kept",
        (_, "link") => "lists-share-done-link",
        ("link", _) => "lists-share-revoked",
        (_, "instance") => "lists-share-done-instance",
        _ => "lists-share-done-private",
    }
}

/// The list the dialog is about, as the list page last rendered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareMeta {
    pub id: String,
    pub name: String,
    pub visibility: String,
    pub share_path: Option<String>,
}

/// Mounted outside the list page's refetched data, so a save (which refetches, and the
/// `entity.updated` event that follows) keeps the dialog and its message; it reads `meta`
/// when it opens.
#[component]
pub fn ShareDialog(
    open: RwSignal<bool>,
    meta: RwSignal<Option<ShareMeta>>,
    on_saved: Callback<()>,
) -> impl IntoView {
    let choice = RwSignal::new(String::new());
    let saved = RwSignal::new(String::new());
    let path = RwSignal::new(None::<String>);
    let status = RwSignal::new(String::new());
    Effect::new(move |_| {
        if open.get()
            && let Some(m) = meta.get_untracked()
        {
            choice.set(m.visibility.clone());
            saved.set(m.visibility);
            path.set(m.share_path);
            status.set(String::new());
        }
    });
    let id = move || meta.with_untracked(|m| m.as_ref().map(|m| m.id.clone()).unwrap_or_default());
    let name =
        Memo::new(move |_| meta.with(|m| m.as_ref().map(|m| m.name.clone()).unwrap_or_default()));
    let save = Action::new(move |new_link: &bool| {
        let (new_link, before) = (*new_link, saved.get_untracked());
        let to = if new_link {
            "link".to_string()
        } else {
            choice.get_untracked()
        };
        let url = format!("/lists/{}/visibility", id());
        async move {
            let body = json!({"visibility": to, "new_link": new_link});
            match rest::call::<ListDto>("PUT", &url, Some(body)).await {
                Ok(Some(l)) => {
                    status.set(t(saved_message(&before, &l.visibility, new_link)));
                    saved.set(l.visibility.clone());
                    choice.set(l.visibility);
                    path.set(l.share_path);
                    on_saved.run(());
                }
                Ok(None) => status.set(t("error-generic")),
                Err(e) => status.set(share_error(&e)),
            }
        }
    });
    let link = move || {
        path.get().filter(|_| saved.get() == "link" && choice.get() == "link").map(|p| {
            let url = absolute(&p);
            view! {
                <div class="grid gap-2 rounded-md border border-border p-3">
                    <Label r#for="ls-share-url">{t("lists-share-the-link")}</Label>
                    <CopyField id="ls-share-url" value=Signal::derive(move || url.clone())
                        copy_label=t("settings-copy") copied_label=t("settings-copied")/>
                    <div class="flex flex-wrap items-center gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| { save.dispatch(true); }
                            attr:aria-describedby="ls-share-new-hint">{t("lists-share-new-link")}</Button>
                        <p id="ls-share-new-hint" class="text-sm text-muted-foreground">{t("lists-share-new-hint")}</p>
                    </div>
                </div>
            }
        })
    };
    move || {
        let i18n = use_i18n();
        let title = i18n.t_args("lists-share-title", &[("name", name.get().into())]);
        view! {
            <Dialog open=open title=title close_label=i18n.t("dialog-close")>
                <form class="grid gap-4" on:submit=move |ev| { ev.prevent_default(); save.dispatch(false); }>
                    <fieldset class="grid gap-3">
                        <legend class="sr-only">{t("lists-share-legend")}</legend>
                        <ShareOptions choice=choice/>
                    </fieldset>
                    {link}
                    <p role="status" class="text-sm">{move || status.get()}</p>
                    <div class="flex flex-wrap justify-end gap-2">
                        <Button variant=ButtonVariant::Outline on:click=move |_| open.set(false)>{t("dialog-close")}</Button>
                        <Button r#type="submit" loading=save.pending()>{t("lists-share-save")}</Button>
                    </div>
                </form>
            </Dialog>
        }
    }
}

/// The three radio choices, each with its consequence (`aria-describedby`).
#[component]
fn ShareOptions(choice: RwSignal<String>) -> impl IntoView {
    CHOICES
        .iter()
        .map(|(value, label, hint)| {
            let (value, input_id) = (*value, format!("ls-share-{value}"));
            let hint_id = format!("{input_id}-hint");
            view! {
                <div class="flex items-start gap-3">
                    <input type="radio" name="ls-share" id=input_id.clone() value=value aria-describedby=hint_id.clone()
                        class="mt-1 size-4 accent-primary"
                        prop:checked=move || choice.get() == value
                        on:change=move |_| choice.set(value.to_string())/>
                    <div class="grid">
                        <Label r#for=input_id>{t(label)}</Label>
                        <p id=hint_id class="text-sm text-muted-foreground">{t(hint)}</p>
                    </div>
                </div>
            }
        })
        .collect_view()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_choice_says_what_happened_to_the_link() {
        assert_eq!(
            saved_message("private", "link", false),
            "lists-share-done-link"
        );
        assert_eq!(saved_message("link", "link", false), "lists-share-kept");
        assert_eq!(saved_message("link", "link", true), "lists-share-replaced");
        assert_eq!(
            saved_message("link", "private", false),
            "lists-share-revoked"
        );
        assert_eq!(
            saved_message("link", "instance", false),
            "lists-share-revoked"
        );
        assert_eq!(
            saved_message("private", "instance", false),
            "lists-share-done-instance"
        );
        assert_eq!(
            saved_message("instance", "private", false),
            "lists-share-done-private"
        );
    }
}
