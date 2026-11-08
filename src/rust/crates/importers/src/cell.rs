//! Spreadsheet-safe CSV cells (OWASP "CSV injection"): shared by the library export and the
//! admin audit CSV (UX2-9, UX3-5).

use std::borrow::Cow;

/// First characters a spreadsheet treats as the start of a formula.
const FORMULA_START: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

fn needs_guard(v: &str) -> bool {
    v.trim_start_matches('\'').starts_with(FORMULA_START)
}

/// `v` with a leading `'` when a spreadsheet would run it as a formula. A value that already
/// starts with `'` before such a character gets one more, so [`unguard`] restores it exactly.
#[must_use]
pub fn guard(v: &str) -> Cow<'_, str> {
    if needs_guard(v) {
        Cow::Owned(format!("'{v}"))
    } else {
        Cow::Borrowed(v)
    }
}

/// The value [`guard`] was given (our own reader of the export).
#[must_use]
pub fn unguard(v: &str) -> &str {
    match v.strip_prefix('\'') {
        Some(rest) if needs_guard(rest) => rest,
        _ => v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarded_cells_round_trip() {
        for v in [
            "=1+1", "+x", "-1", "@a", "\tx", "\rx", "'=x", "''+x", "plain", "'plain", "",
        ] {
            assert_eq!(unguard(&guard(v)), v, "{v:?}");
        }
        assert_eq!(guard("=1+1"), "'=1+1");
        assert_eq!(guard("plain"), "plain");
        assert_eq!(guard("'plain"), "'plain");
    }
}
