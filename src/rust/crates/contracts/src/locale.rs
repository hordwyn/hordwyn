//! Supported locales and the number format of each, shared by the web app (pages), the
//! notification texts and the API's validation of a user's language (DEC-200, DEC-201).
//!
//! Number formatting covers decimal numbers for en and de only, from the CLDR "latn" number
//! symbols (en: decimal `.`, group `,`; de: decimal `,`, group `.`; both group from four
//! digits) and the ICU default of at most three fraction digits. Currency and percent styles
//! are not covered.

/// Locales with built-in texts; the first is the fallback for missing keys.
pub const LOCALES: [&str; 2] = ["en", "de"];

/// The name of a supported locale in its own language, for the language picker.
#[must_use]
pub fn locale_name(locale: &str) -> &str {
    match locale {
        "en" => "English",
        "de" => "Deutsch",
        other => other,
    }
}

/// `locale` if it is supported (case-insensitive, region ignored: `de-AT` -> `de`).
#[must_use]
pub fn supported(locale: &str) -> Option<&'static str> {
    let lang = locale.split(['-', '_']).next()?.to_ascii_lowercase();
    LOCALES.into_iter().find(|l| *l == lang)
}

/// Decimal and group separator of a locale.
#[must_use]
pub fn separators(locale: &str) -> (char, char) {
    if supported(locale) == Some("de") {
        (',', '.')
    } else {
        ('.', ',')
    }
}

/// Digit options, as Fluent's `NUMBER()` and `Intl.NumberFormat` name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Digits {
    pub min_integer: usize,
    pub min_fraction: usize,
    /// At least `min_fraction`.
    pub max_fraction: usize,
    pub grouping: bool,
}

impl Default for Digits {
    fn default() -> Self {
        Self {
            min_integer: 1,
            min_fraction: 0,
            max_fraction: 3,
            grouping: true,
        }
    }
}

/// `value` in the number format of `locale` (`1.234,5` in German); non-finite values print
/// as Rust does.
#[must_use]
pub fn format_decimal(locale: &str, value: f64, d: Digits) -> String {
    if !value.is_finite() {
        return value.to_string();
    }
    let (decimal, group) = separators(locale);
    let max_frac = d.max_fraction.max(d.min_fraction);
    let fixed = format!("{:.*}", max_frac, value.abs());
    let (int, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let mut frac = frac.trim_end_matches('0').to_string();
    while frac.len() < d.min_fraction {
        frac.push('0');
    }
    let int = format!("{int:0>width$}", width = d.min_integer.max(1));
    let mut out = String::new();
    if value < 0.0 && (int.bytes().any(|b| b != b'0') || !frac.is_empty()) {
        out.push('-');
    }
    for (i, c) in int.chars().enumerate() {
        if d.grouping && i > 0 && (int.len() - i).is_multiple_of(3) {
            out.push(group);
        }
        out.push(c);
    }
    if !frac.is_empty() {
        out.push(decimal);
        out.push_str(&frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_follow_the_locale() {
        let d = Digits::default();
        assert_eq!(format_decimal("en", 1234.5, d), "1,234.5");
        assert_eq!(format_decimal("de", 1234.5, d), "1.234,5");
        assert_eq!(format_decimal("de", 3.5, d), "3,5");
        assert_eq!(format_decimal("de", -0.25, d), "-0,25");
        assert_eq!(format_decimal("de", -0.0001, d), "0");
        assert_eq!(format_decimal("en", 3.0, d), "3");
        assert_eq!(format_decimal("de", 1_000_000.0, d), "1.000.000");
        assert_eq!(format_decimal("en", 2.0 / 3.0, d), "0.667");
        let padded = Digits {
            min_integer: 2,
            min_fraction: 2,
            max_fraction: 2,
            grouping: false,
        };
        assert_eq!(format_decimal("de", 2.5, padded), "02,50");
        assert_eq!(format_decimal("en", 12345.0, padded), "12345.00");
    }

    #[test]
    fn locales_are_matched_by_language() {
        assert_eq!(supported("DE-at"), Some("de"));
        assert_eq!(supported("en_GB"), Some("en"));
        assert_eq!(supported("fr"), None);
        assert_eq!(separators("de-CH"), (',', '.'));
    }

    #[test]
    fn every_locale_is_named_in_itself() {
        assert_eq!(locale_name("en"), "English");
        assert_eq!(locale_name("de"), "Deutsch");
        for l in LOCALES {
            assert_ne!(locale_name(l), l, "{l}");
        }
    }
}
