//! `CopyField` (read-only value with a Copy button) and `KeyHint` (`<kbd>` shortcut hints).

use std::time::Duration;

use leptos::html;
use leptos::prelude::*;

use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::class::cn;
use crate::form::FIELD;
use crate::icon::{Icon, IconName};

/// Write `text` to the clipboard; `done(true)` once it is there, `done(false)` when the
/// Clipboard API is missing (insecure context) or refuses.
fn write_clipboard(text: &str, done: impl Fn(bool) + 'static) {
    use leptos::wasm_bindgen::{JsValue, closure::Closure};
    use web_sys::js_sys::Reflect;
    let nav = window().navigator();
    let available = Reflect::get(&nav, &JsValue::from_str("clipboard"))
        .is_ok_and(|c| !c.is_undefined() && !c.is_null());
    if !available {
        done(false);
        return;
    }
    let done = std::rc::Rc::new(done);
    let ok_done = done.clone();
    let ok = Closure::<dyn FnMut(JsValue)>::new(move |_| ok_done(true));
    let err = Closure::<dyn FnMut(JsValue)>::new(move |_| done(false));
    let clipboard = nav.clipboard();
    let _ = clipboard.write_text(text).then2(&ok, &err);
    ok.forget();
    err.forget();
}

/// Read-only value with a Copy button and a polite status line ("Copied"). Label it with
/// `<Label for=id>` or a `FormField`. Copying also selects the text, so `Ctrl+C` works where
/// the Clipboard API is unavailable; `failed_label` then tells the person so.
///
/// `shown_once` is the "shown once" variant for generated secrets (addendum-1 2.3): the note
/// is rendered under the field and referenced by `aria-describedby` (`{id}-once`).
#[component]
pub fn CopyField(
    #[prop(into)] id: String,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] copy_label: String,
    #[prop(into)] copied_label: String,
    #[prop(into, optional)] failed_label: MaybeProp<String>,
    #[prop(into, optional)] shown_once: MaybeProp<String>,
    #[prop(into, optional)] described_by: MaybeProp<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let once_id = format!("{id}-once");
    let once_ref = once_id.clone();
    let status = RwSignal::new(None::<String>);
    let input = NodeRef::<html::Input>::new();
    let copied_label = StoredValue::new(copied_label);
    let desc_ids = move || {
        let mut ids = Vec::new();
        if shown_once.get().is_some() {
            ids.push(once_ref.clone());
        }
        if let Some(d) = described_by.get() {
            ids.push(d);
        }
        (!ids.is_empty()).then(|| ids.join(" "))
    };
    let copy = move |_| {
        if let Some(el) = input.get_untracked() {
            el.select();
        }
        write_clipboard(&value.get_untracked(), move |ok| {
            let msg = if ok {
                Some(copied_label.get_value())
            } else {
                failed_label.get_untracked()
            };
            status.set(msg);
            set_timeout(move || status.set(None), Duration::from_secs(5));
        });
    };
    view! {
        <div class=move || cn(&["grid gap-1.5", &class.get().unwrap_or_default()])>
            <div class="flex items-center gap-2">
                <input
                    node_ref=input
                    id=id
                    type="text"
                    readonly=true
                    value=move || value.get()
                    aria-describedby=desc_ids
                    class=cn(&[FIELD, "h-9 bg-muted font-mono"])
                />
                <Button variant=ButtonVariant::Outline size=ButtonSize::Sm class="h-9" on:click=copy>
                    <Icon name=IconName::Copy/>
                    {copy_label}
                </Button>
            </div>
            {move || shown_once.get().map(|note| view! {
                <p id=once_id.clone() class="flex items-start gap-1.5 text-sm text-muted-foreground">
                    <Icon name=IconName::Lock class="mt-0.5"/>
                    <span>{note}</span>
                </p>
            })}
            <p role="status" class="text-sm font-medium text-success empty:hidden">{move || status.get()}</p>
        </div>
    }
}

/// Keyboard shortcut hint: `<kbd><kbd>Ctrl</kbd>+<kbd>K</kbd></kbd>` (nested `kbd` marks a key
/// combination, HTML Living Standard 4.5.24).
#[component]
pub fn KeyHint(
    #[prop(into)] keys: Vec<String>,
    /// Text between keys (default `+`).
    #[prop(into, optional)]
    separator: Option<String>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let sep = separator.unwrap_or_else(|| "+".to_string());
    let last = keys.len().saturating_sub(1);
    view! {
        <kbd class=move || cn(&["inline-flex items-center gap-0.5 font-sans text-xs text-muted-foreground", &class.get().unwrap_or_default()])>
            {keys.into_iter().enumerate().map(|(i, k)| view! {
                <kbd class="rounded border border-b-2 bg-muted px-1.5 font-mono text-xs text-foreground">{k}</kbd>
                {(i < last).then(|| sep.clone())}
            }).collect_view()}
        </kbd>
    }
}
