//! Keyset paging cursors of the library (DEC-416): a page ends at its last row's sort key and
//! entry id, and the next page starts strictly after it. Deep pages cost as much as the first
//! (no `OFFSET`), and rows added or removed meanwhile do not shift the pages.
//!
//! The wire form is opaque to clients: URL-safe base64 (no padding) of a small JSON object.
//! Opaque, not confidential: it carries the last row's key (a lowercased title, a time or a
//! number) readably; that is the caller's own data, and a cursor never widens a query (UX4-21).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::ports::{LibraryCursor, LibrarySort, SortValue};

/// Longest cursor accepted (a title key of a few hundred characters fits easily).
const MAX_WIRE: usize = 4096;

impl SortValue {
    /// Is this the kind of key `sort` orders by?
    #[must_use]
    pub const fn fits(&self, sort: LibrarySort) -> bool {
        matches!(
            (self, sort),
            (Self::Text(_), LibrarySort::Title)
                | (
                    Self::Time(_),
                    LibrarySort::Added | LibrarySort::LastActivity
                )
                | (
                    Self::Int(_),
                    LibrarySort::Year | LibrarySort::Rating | LibrarySort::Plays
                )
        )
    }
}

impl LibraryCursor {
    /// The opaque text form clients send back as `after`.
    #[must_use]
    pub fn encode(&self) -> String {
        // serialising plain data with derived impls cannot fail
        let json = serde_json::to_vec(self).unwrap_or_default();
        URL_SAFE_NO_PAD.encode(json)
    }

    /// Read a cursor made by [`LibraryCursor::encode`]; `None` for anything else.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        if text.len() > MAX_WIRE {
            return None;
        }
        let bytes = URL_SAFE_NO_PAD.decode(text.trim()).ok()?;
        let c: Self = serde_json::from_slice(&bytes).ok()?;
        c.key.fits(c.sort).then_some(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::ids::EntryId;

    #[test]
    fn a_cursor_survives_its_text_form() {
        let at = chrono::DateTime::parse_from_rfc3339("2026-03-04T05:06:07.123456789Z")
            .unwrap()
            .to_utc();
        for (sort, key) in [
            (
                LibrarySort::Title,
                SortValue::Text("Zoe's \"x\" / 1".into()),
            ),
            (LibrarySort::Added, SortValue::Time(at)),
            (LibrarySort::Year, SortValue::Int(None)),
            (LibrarySort::Plays, SortValue::Int(Some(-3))),
        ] {
            let c = LibraryCursor {
                sort,
                descending: true,
                key,
                id: EntryId(uuid::Uuid::from_u128(7)),
            };
            let text = c.encode();
            assert!(
                text.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            );
            assert_eq!(LibraryCursor::decode(&text), Some(c));
        }
    }

    #[test]
    fn foreign_text_is_no_cursor() {
        assert_eq!(LibraryCursor::decode("50"), None);
        assert_eq!(LibraryCursor::decode("not base64!"), None);
        assert_eq!(LibraryCursor::decode(&"A".repeat(MAX_WIRE + 1)), None);
        // a key of the wrong type for its sort
        let wrong = LibraryCursor {
            sort: LibrarySort::Title,
            descending: false,
            key: SortValue::Int(Some(1)),
            id: EntryId(uuid::Uuid::nil()),
        };
        assert_eq!(LibraryCursor::decode(&wrong.encode()), None);
    }
}
