//! Admin > Data sources: the operator contact (v1-books, `instance.operator_contact`). One
//! field: an e-mail address or an https URL that Open Library and Wikidata get in the
//! `User-Agent` of every request, so they can reach whoever runs the instance. Read-only
//! with its variable when the environment or the file sets it.

use std::collections::BTreeMap;

use contracts::admin::SettingDto;
use leptos::prelude::*;
use serde_json::Value;
use ui::{Button, FormField, Input, field_described_by};

use super::SettingNote;
use super::sources::Sources;
use crate::admin_state::{Origin, origin};
use crate::i18n::{t, use_i18n};
use crate::reauth::{gated_settings, use_reauth};
use crate::settings::{Outcome, kept};

/// The setting's key.
pub const KEY: &str = "instance.operator_contact";

/// What is stored for the typed text: `null` (no contact) for an empty field.
#[must_use]
pub fn contact_value(typed: &str) -> Value {
    let v = typed.trim();
    if v.is_empty() {
        Value::Null
    } else {
        Value::from(v)
    }
}

#[component]
pub fn OperatorContact(s: SettingDto, sources: Sources) -> impl IntoView {
    let editable = origin(&s) == Origin::Editable;
    let value = RwSignal::new(s.value.as_str().unwrap_or_default().to_string());
    let reauth = use_reauth();
    let busy = RwSignal::new(false);
    let message = kept("src-contact", || None::<Result<String, String>>);
    let i18n = use_i18n();
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let typed = contact_value(&value.get_untracked());
        let done = i18n.t(if typed.is_null() {
            "admin-contact-cleared"
        } else {
            "admin-saved"
        });
        gated_settings(
            reauth,
            busy,
            BTreeMap::from([(KEY.to_string(), typed)]),
            Callback::new(move |r: Result<_, String>| {
                // a source that requires the contact changes its state with it
                if r.is_ok() {
                    sources.refetch();
                }
                message.set(Some(r.map(|_| done.clone())));
            }),
        );
    };
    view! {
        <form class="mb-8 grid max-w-2xl gap-3" novalidate on:submit=save data-testid="operator-contact">
            <FormField id="adm-contact" label=t("admin-contact-label") description=t("admin-contact-hint")>
                <Input id="adm-contact" value=value autocomplete="off" readonly=!editable
                    described_by=field_described_by("adm-contact")/>
            </FormField>
            <SettingNote s=s/>
            <Outcome message=message/>
            {editable.then(|| view! { <Button r#type="submit" class="w-fit" loading=busy>{t("admin-save")}</Button> })}
        </form>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_field_clears_the_contact() {
        assert_eq!(contact_value("  "), Value::Null);
        assert_eq!(
            contact_value(" ops@example.org "),
            Value::from("ops@example.org")
        );
    }
}
