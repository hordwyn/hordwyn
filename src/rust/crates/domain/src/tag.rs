//! Tags: the user's own flat labels on library entries (design.md 3.1). Private to their
//! owner like notes; never shown on shared lists or to linked people.

use crate::DomainError;

/// Longest tag in characters.
pub const MAX_TAG_CHARS: usize = 50;
/// Most tags on one entry.
pub const MAX_TAGS_PER_ENTRY: usize = 50;

/// Clean a tag as typed: trim, collapse inner whitespace. Case is kept for display; two
/// tags that differ only in case are the same tag ([`tag_key`]).
///
/// # Errors
/// Empty, too long, or containing control characters.
pub fn normalize_tag(raw: &str) -> Result<String, DomainError> {
    let tag = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if tag.is_empty() {
        return Err(DomainError::invalid("tag", "tag must not be empty"));
    }
    if tag.chars().count() > MAX_TAG_CHARS {
        return Err(DomainError::invalid("tag", "tag is too long"));
    }
    if tag.chars().any(char::is_control) {
        return Err(DomainError::invalid(
            "tag",
            "tag contains control characters",
        ));
    }
    Ok(tag)
}

/// Comparison key of a normalised tag (case-insensitive).
#[must_use]
pub fn tag_key(tag: &str) -> String {
    tag.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_trimmed_collapsed_and_bounded() {
        assert_eq!(normalize_tag("  with   Anna ").unwrap(), "with Anna");
        assert!(normalize_tag("   ").is_err());
        assert!(normalize_tag(&"x".repeat(51)).is_err());
        assert!(normalize_tag(&"x".repeat(50)).is_ok());
        assert_eq!(tag_key("Cosy"), tag_key("cosy"));
    }
}
