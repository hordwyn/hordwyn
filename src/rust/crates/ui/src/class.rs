//! Class-name helpers.

/// Join class fragments, skipping empty ones. Later fragments come last, so a caller's
/// `class` placed at the end extends or overrides earlier utilities.
#[must_use]
pub fn cn(parts: &[&str]) -> String {
    let mut out = String::new();
    for p in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(p);
    }
    out
}

/// A tiny `cva`: base classes plus one class string per selected variant dimension.
#[must_use]
pub fn cva(base: &str, variants: &[&str]) -> String {
    let mut parts = vec![base];
    parts.extend_from_slice(variants);
    cn(&parts)
}

/// Stable, id-safe slug (`[a-z0-9-]`) for deriving element ids that match between SSR and
/// hydration.
#[must_use]
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(cn(&["a", "", " b ", "c"]), "a b c");
        assert_eq!(cva("base", &["v1", "", "s2"]), "base v1 s2");
        assert_eq!(slug("Hello, World!"), "hello-world");
    }
}
