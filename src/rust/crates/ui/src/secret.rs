//! `SecretField`: a write-only secret (addendum-1 2.3). A stored value is shown as a
//! read-only "set on" line with Replace, never as the value or a run of masking
//! characters; Replace opens an empty password field (`autocomplete=off`) and Cancel keeps
//! the stored value. Test and Save belong to the caller (they differ per purpose).

use leptos::prelude::*;

use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::class::cn;
use crate::form::{Input, Label};

#[component]
pub fn SecretField(
    /// Id of the input; the stored line is `{id}-stored`, the hint `{id}-description`.
    #[prop(into)]
    id: String,
    #[prop(into)] label: String,
    /// The new value typed while replacing (empty: keep the stored one).
    value: RwSignal<String>,
    /// The Replace state; ignored while nothing is stored.
    editing: RwSignal<bool>,
    /// "Set 12 Oct 2026 by robin" when a value is stored.
    #[prop(into, optional)]
    stored: MaybeProp<String>,
    #[prop(into)] replace_label: String,
    /// Accessible name of Replace when a form has several secrets ("Replace Client ID",
    /// UX2-W7); it should start with `replace_label` (WCAG 2.5.3).
    #[prop(into, optional)]
    replace_name: MaybeProp<String>,
    #[prop(into)] cancel_label: String,
    #[prop(into, optional)] description: MaybeProp<String>,
    #[prop(into, optional)] invalid: Signal<bool>,
    #[prop(into, optional)] class: MaybeProp<String>,
) -> impl IntoView {
    let stored_id = format!("{id}-stored");
    let desc_id = format!("{id}-description");
    let ids = StoredValue::new((id, stored_id, desc_id, label, replace_label, cancel_label));
    let show_field = move || stored.get().is_none() || editing.get();
    let cancel = move |_| {
        value.set(String::new());
        editing.set(false);
    };
    view! {
        <div class=move || cn(&["grid gap-2", &class.get().unwrap_or_default()])>
            {move || {
                let (id, stored_id, desc_id, label, replace, cancel_label) = ids.get_value();
                let has_desc = description.get().is_some();
                if show_field() {
                    let described = has_desc.then(|| desc_id.clone());
                    view! {
                        <Label r#for=id.clone()>{label}</Label>
                        <div class="flex flex-wrap items-center gap-2">
                            <Input id=id r#type="password" value=value autocomplete="off"
                                invalid=invalid described_by=described class="min-w-0 flex-1"/>
                            {stored.get().is_some().then(|| view! {
                                <Button variant=ButtonVariant::Ghost size=ButtonSize::Sm on:click=cancel>
                                    {cancel_label}
                                </Button>
                            })}
                        </div>
                    }
                    .into_any()
                } else {
                    view! {
                        <p class="text-sm font-medium leading-none">{label}</p>
                        <div class="flex flex-wrap items-center gap-2">
                            <p id=stored_id.clone() class="flex-1 text-sm text-muted-foreground">{stored.get()}</p>
                            <Button variant=ButtonVariant::Outline size=ButtonSize::Sm
                                attr:aria-describedby=stored_id attr:aria-label=move || replace_name.get() on:click=move |_| editing.set(true)>
                                {replace}
                            </Button>
                        </div>
                    }
                    .into_any()
                }
            }}
            {move || description.get().map(|d| view! {
                <p id=ids.with_value(|i| i.2.clone()) class="text-sm text-muted-foreground">{d}</p>
            })}
        </div>
    }
}
