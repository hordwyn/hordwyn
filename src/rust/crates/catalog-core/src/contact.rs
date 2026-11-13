//! The operator contact (`instance.operator_contact`): an e-mail address or an https URL of
//! the person who runs the instance. Sources that ask clients to identify themselves get it
//! in the `User-Agent` (`<ident>/<version> (<contact>)`); every other source never sees it.

use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Longest contact accepted.
pub const MAX_CONTACT_CHARS: usize = 200;

/// Does a connector send the operator contact?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactUse {
    /// Never: the requests carry the plain application `User-Agent`.
    #[default]
    None,
    /// Sent when set; the source also answers without it.
    Preferred,
    /// Sent with every request; without a contact the source is not asked at all.
    Required,
}

/// Why a value is not a usable contact.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ContactError {
    #[error("the contact is empty")]
    Empty,
    #[error("the contact is longer than 200 characters")]
    TooLong,
    /// A control character, a space, a parenthesis or a character outside ASCII: the value
    /// goes into a header, inside a comment in parentheses.
    #[error("the contact holds a character that cannot be sent in a header")]
    Character,
    #[error("the contact is neither an e-mail address nor an https URL")]
    Form,
}

/// Check a contact: an e-mail address (`name@host.tld`) or an `https://` URL, at most
/// [`MAX_CONTACT_CHARS`] characters, printable ASCII without spaces and parentheses.
/// Returns the value without surrounding white space.
///
/// # Errors
/// See [`ContactError`].
pub fn check_contact(value: &str) -> Result<String, ContactError> {
    let v = value.trim_matches(' ');
    if v.is_empty() {
        return Err(ContactError::Empty);
    }
    if v.chars().count() > MAX_CONTACT_CHARS {
        return Err(ContactError::TooLong);
    }
    if !v
        .chars()
        .all(|c| c.is_ascii_graphic() && c != '(' && c != ')')
    {
        return Err(ContactError::Character);
    }
    let url = v.starts_with("https://")
        && url::Url::parse(v).is_ok_and(|u| u.host_str().is_some_and(|h| !h.is_empty()));
    let mail = !v.contains("://")
        && v.split_once('@').is_some_and(|(name, host)| {
            !name.is_empty()
                && !host.contains('@')
                && host.contains('.')
                && host.split('.').all(|part| !part.is_empty())
        });
    if url || mail {
        Ok(v.to_string())
    } else {
        Err(ContactError::Form)
    }
}

/// The contact as the running services read it: set at start from the configuration and
/// again whenever an admin changes it. Clones share the value.
#[derive(Debug, Clone, Default)]
pub struct OperatorContact(Arc<RwLock<Option<String>>>);

impl OperatorContact {
    /// Replace the contact (`None`: no contact).
    ///
    /// # Errors
    /// A value [`check_contact`] refuses; the earlier contact stays.
    pub fn set(&self, value: Option<&str>) -> Result<(), ContactError> {
        let next = value.map(check_contact).transpose()?;
        *self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = next;
        Ok(())
    }

    #[must_use]
    pub fn get(&self) -> Option<String> {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_or_an_https_url_is_a_contact() {
        assert_eq!(
            check_contact(" ops@example.org ").as_deref(),
            Ok("ops@example.org")
        );
        assert_eq!(
            check_contact("https://example.org/contact").as_deref(),
            Ok("https://example.org/contact")
        );
        for bad in [
            "http://example.org",
            "example.org",
            "ops@",
            "@example.org",
            "ops@host",
        ] {
            assert_eq!(check_contact(bad), Err(ContactError::Form), "{bad}");
        }
        assert_eq!(check_contact("  "), Err(ContactError::Empty));
        let long = format!("{}@example.org", "a".repeat(200));
        assert_eq!(check_contact(&long), Err(ContactError::TooLong));
    }

    #[test]
    fn a_contact_with_a_line_break_is_refused() {
        for bad in [
            "ops@example.org\r\nX-Injected: 1",
            "ops@example.org\n",
            "ops@exam\tple.org",
            "ops @example.org",
            "ops@example.org)",
            "ops@ex\u{e4}mple.org",
        ] {
            assert_eq!(check_contact(bad), Err(ContactError::Character), "{bad:?}");
        }
        let shared = OperatorContact::default();
        shared.set(Some("ops@example.org")).unwrap();
        assert_eq!(
            shared.set(Some("ops@example.org\r\nX: 1")),
            Err(ContactError::Character)
        );
        assert_eq!(shared.get().as_deref(), Some("ops@example.org"));
        shared.set(None).unwrap();
        assert_eq!(shared.get(), None);
    }
}
