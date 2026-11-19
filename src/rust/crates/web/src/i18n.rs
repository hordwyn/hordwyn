//! Every user-visible string goes through here (D-10). Messages are Fluent files embedded at
//! build time (`locales/<lang>.ftl`); the brand can override any key at runtime
//! (`[strings.<lang>]` in `brand.toml`), which is why this uses Fluent rather than a
//! compile-time catalogue (DEC-090).
//!
//! Numbers and dates follow the locale (DEC-200): fluent-bundle formats numbers without
//! locale data, so each bundle gets a small formatter for its locale (decimal and grouping
//! separators, the `NUMBER()` digit options); dates use the CLDR medium patterns of en and
//! de with month names from the messages.

use std::collections::BTreeMap;
use std::sync::Arc;

use contracts::locale::{Digits, format_decimal};
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::types::{FluentNumber, FluentNumberStyle};
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use leptos::prelude::*;
use unic_langid::LanguageIdentifier;

/// Supported locales; the first is the fallback for missing keys.
pub use contracts::locale::{LOCALES, supported};

fn source(locale: &str) -> &'static str {
    match locale {
        "de" => include_str!("../locales/de.ftl"),
        _ => include_str!("../locales/en.ftl"),
    }
}

/// A Fluent number in the locale's format with its `NUMBER()` digit options; `None` for
/// styles the shared formatter does not cover (currency, percent), which then print plain.
fn format_number(locale: &str, n: &FluentNumber) -> Option<String> {
    if n.options.style != FluentNumberStyle::Decimal {
        return None;
    }
    let o = &n.options;
    let digits = Digits {
        min_integer: o.minimum_integer_digits.unwrap_or(1),
        min_fraction: o.minimum_fraction_digits.unwrap_or(0),
        max_fraction: o.maximum_fraction_digits.unwrap_or(3),
        grouping: o.use_grouping,
    };
    Some(format_decimal(locale, n.value, digits))
}

/// Bundle formatters: a plain function per locale (the bundle takes a function pointer).
fn formatter_en<M>(v: &FluentValue<'_>, _: &M) -> Option<String> {
    match v {
        FluentValue::Number(n) => format_number("en", n),
        _ => None,
    }
}

fn formatter_de<M>(v: &FluentValue<'_>, _: &M) -> Option<String> {
    match v {
        FluentValue::Number(n) => format_number("de", n),
        _ => None,
    }
}

/// One brand override as a Fluent resource (`key = text`, continuation lines indented), so
/// overrides get the same variables, plurals and number formatting as the built-in texts.
fn override_resource(key: &str, text: &str) -> Option<FluentResource> {
    let valid = key.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid || text.trim().is_empty() {
        return None;
    }
    let body = text.lines().collect::<Vec<_>>().join("\n    ");
    FluentResource::try_new(format!("{key} =\n    {body}\n")).ok()
}

fn bundle(locale: &str, overrides: &BTreeMap<String, String>) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = locale.parse().unwrap_or_default();
    let mut b = FluentBundle::new_concurrent(vec![id]);
    // no Unicode isolation marks around placeables: output stays plain text
    b.set_use_isolating(false);
    // NUMBER() for translators and brand overrides
    let _ = b.add_builtins();
    b.set_formatter(Some(if locale == "de" {
        formatter_de
    } else {
        formatter_en
    }));
    let res =
        FluentResource::try_new(source(locale).to_string()).unwrap_or_else(|(res, _errors)| res);
    let _ = b.add_resource(res);
    for (key, text) in overrides {
        if let Some(r) = override_resource(key, text) {
            b.add_resource_overriding(r);
        }
    }
    b
}

/// A UTC timestamp or date as sent by the server (`2026-11-06` or `2026-11-06T21:05:00Z`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    year: u32,
    month: u32,
    day: u32,
    time: Option<(u32, u32)>,
}

fn parse_stamp(s: &str) -> Option<Stamp> {
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<u32>().ok();
    let stamp = Stamp {
        year: num(0..4)?,
        month: num(5..7).filter(|m| (1..=12).contains(m))?,
        day: num(8..10).filter(|d| (1..=31).contains(d))?,
        time: match s.get(10..11) {
            Some("T" | " ") => Some((num(11..13)?, num(14..16)?)),
            _ => None,
        },
    };
    (s.get(4..5) == Some("-") && s.get(7..8) == Some("-")).then_some(stamp)
}

pub struct I18n {
    locale: String,
    bundle: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
}

impl I18n {
    /// Messages for `locale` (unknown locales fall back to the first of [`LOCALES`]) with the
    /// brand's overrides for that locale.
    #[must_use]
    pub fn new(locale: &str, overrides: BTreeMap<String, String>) -> Self {
        let locale = supported(locale).unwrap_or(LOCALES[0]);
        Self {
            locale: locale.to_string(),
            bundle: bundle(locale, &overrides),
            fallback: bundle(LOCALES[0], &BTreeMap::new()),
        }
    }

    #[must_use]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// Text for `key`; the key itself if it is missing everywhere (visible in tests).
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.t_args(key, &[])
    }

    /// Text for `key` with arguments (`{ $name }` in the message). Numbers are formatted for
    /// the locale and select plural variants by the CLDR rules.
    #[must_use]
    pub fn t_args(&self, key: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        let mut fargs = FluentArgs::new();
        for (k, v) in args {
            fargs.set(*k, v.clone());
        }
        let fargs = (!args.is_empty()).then_some(&fargs);
        for b in [&self.bundle, &self.fallback] {
            if let Some(pattern) = b.get_message(key).and_then(|m| m.value()) {
                let mut errors = vec![];
                return b.format_pattern(pattern, fargs, &mut errors).into_owned();
            }
        }
        key.to_string()
    }

    /// A number for the locale (`3,5` in German).
    #[must_use]
    pub fn number(&self, value: f64) -> String {
        format_decimal(&self.locale, value, Digits::default())
    }

    /// The date part of a server timestamp in the locale's medium format (CLDR: en
    /// `MMM d, y`, de `dd.MM.y`); text that is not a timestamp comes back unchanged.
    #[must_use]
    pub fn date(&self, stamp: &str) -> String {
        let Some(s) = parse_stamp(stamp) else {
            return stamp.to_string();
        };
        if self.locale == "de" {
            format!("{:02}.{:02}.{}", s.day, s.month, s.year)
        } else {
            let month = self.t(&format!("month-short-{}", s.month));
            format!("{month} {}, {}", s.day, s.year)
        }
    }

    /// Day and month of a server timestamp without the year (CLDR `MMMd`: en `MMM d`, de
    /// `dd.MM.`), for dates in the current year.
    #[must_use]
    pub fn day_month(&self, stamp: &str) -> String {
        let Some(s) = parse_stamp(stamp) else {
            return stamp.to_string();
        };
        if self.locale == "de" {
            format!("{:02}.{:02}.", s.day, s.month)
        } else {
            let month = self.t(&format!("month-short-{}", s.month));
            format!("{month} {}", s.day)
        }
    }

    /// A time of day (CLDR short time: en `h:mm a`, de `HH:mm`).
    #[must_use]
    pub fn time(&self, hour: u32, minute: u32) -> String {
        if self.locale == "de" {
            format!("{hour:02}:{minute:02}")
        } else {
            let period = self.t(if hour < 12 { "time-am" } else { "time-pm" });
            let h = match hour % 12 {
                0 => 12,
                h => h,
            };
            format!("{h}:{minute:02} {period}")
        }
    }

    /// Date and time of a UTC server timestamp, marked as UTC (there is no per-user time
    /// zone yet, DEC-200); a bare date shows as [`I18n::date`].
    #[must_use]
    pub fn datetime(&self, stamp: &str) -> String {
        let Some(s) = parse_stamp(stamp) else {
            return stamp.to_string();
        };
        match s.time {
            Some((h, m)) => self.t_args(
                "datetime-utc",
                &[
                    ("date", self.date(stamp).into()),
                    ("time", self.time(h, m).into()),
                ],
            ),
            None => self.date(stamp),
        }
    }
}

/// Pick a supported locale from an `Accept-Language` header.
#[must_use]
pub fn negotiate(accept_language: Option<&str>, default: &str) -> String {
    negotiated(accept_language).unwrap_or_else(|| default.to_string())
}

/// The supported locale an `Accept-Language` header asks for, if any (no match: a page falls
/// back to the brand default, see [`negotiate`]).
#[must_use]
pub fn negotiated(accept_language: Option<&str>) -> Option<String> {
    let mut prefs: Vec<(f32, String)> = accept_language
        .unwrap_or_default()
        .split(',')
        .filter_map(|part| {
            let mut it = part.trim().split(';');
            let tag = it.next()?.trim().to_ascii_lowercase();
            let q = it
                .find_map(|p| p.trim().strip_prefix("q=").and_then(|q| q.parse().ok()))
                .unwrap_or(1.0);
            let lang = tag.split('-').next()?.to_string();
            (!lang.is_empty()).then_some((q, lang))
        })
        .collect();
    prefs.sort_by(|a, b| b.0.total_cmp(&a.0));
    prefs
        .into_iter()
        .map(|(_, l)| l)
        .find(|l| LOCALES.contains(&l.as_str()))
}

/// The locale a page renders in (DEC-201): the signed-in user's choice, else the browser's
/// `Accept-Language`, else the brand's default.
#[must_use]
pub fn effective_locale(user: Option<&str>, accept_language: Option<&str>, brand: &str) -> String {
    user.and_then(supported).map_or_else(
        || negotiate(accept_language, supported(brand).unwrap_or(LOCALES[0])),
        String::from,
    )
}

/// The current request's / page's messages.
///
/// # Panics
/// If no `I18n` was provided (the app root always provides one).
#[must_use]
pub fn use_i18n() -> Arc<I18n> {
    expect_context::<Arc<I18n>>()
}

/// Label of a media kind; kinds without a message (new or plugin kinds) show their id in
/// words, e.g. `wine_bottle` -> "Wine bottle" (DEC-098).
#[must_use]
pub fn kind_label(kind: &str) -> String {
    let key = format!("kind-{kind}");
    let text = t(&key);
    if text != key {
        return text;
    }
    let words = kind
        .rsplit('/')
        .next()
        .unwrap_or(kind)
        .replace(['_', '-'], " ");
    let mut c = words.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Shorthand inside views: `t("nav-library")`.
#[must_use]
pub fn t(key: &str) -> String {
    use_i18n().t(key)
}

/// A catalog title as shown: an item whose provider data expired was purged to an empty title
/// (DEC-466) and reads "Title unavailable - source data expired" (`catalog-title-unavailable`).
#[must_use]
pub fn title_text(title: &str) -> String {
    use_context::<Arc<I18n>>().map_or_else(|| title.to_string(), |i| i.title(title))
}

impl I18n {
    /// [`title_text`] with these messages.
    #[must_use]
    pub fn title(&self, title: &str) -> String {
        if title.trim().is_empty() {
            self.t("catalog-title-unavailable")
        } else {
            title.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_expired_title_reads_as_the_placeholder() {
        let en = I18n::new("en", BTreeMap::new());
        assert_eq!(en.title(""), "Title unavailable - source data expired");
        assert_eq!(en.title("Dune"), "Dune");
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(
            de.title(" "),
            "Titel nicht verf\u{fc}gbar - Quelldaten abgelaufen"
        );
    }

    #[test]
    fn translates_with_plurals_and_falls_back() {
        let en = I18n::new("en", BTreeMap::new());
        assert_eq!(en.t("nav-library"), "Library");
        assert_eq!(
            en.t_args("library-count", &[("count", 1.into())]),
            "One item"
        );
        assert_eq!(
            en.t_args("library-count", &[("count", 3.into())]),
            "3 items"
        );
        let de = I18n::new("de", BTreeMap::new());
        assert_eq!(de.t("nav-library"), "Bibliothek");
        assert_eq!(I18n::new("fr", BTreeMap::new()).locale(), "en");
        assert_eq!(en.t("no-such-key"), "no-such-key");
    }

    #[test]
    fn brand_overrides_win_and_keep_plurals_and_numbers() {
        let mut o = BTreeMap::new();
        o.insert("nav-library".to_string(), "Shelf".to_string());
        o.insert("item-source".to_string(), "Data by { $name }".to_string());
        o.insert(
            "library-count".to_string(),
            "{ $count ->\n    [one] One book\n   *[other] { $count } books\n}".to_string(),
        );
        let en = I18n::new("en", o);
        assert_eq!(en.t("nav-library"), "Shelf");
        assert_eq!(
            en.t_args("item-source", &[("name", "TMDB".into())]),
            "Data by TMDB"
        );
        assert_eq!(
            en.t_args("library-count", &[("count", 1.into())]),
            "One book"
        );
        assert_eq!(
            en.t_args("library-count", &[("count", 1200.into())]),
            "1,200 books"
        );
    }

    #[test]
    fn numbers_follow_the_locale() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        // the owner's bug: German showed "3.5"
        let de_rate = de.t_args("item-rate", &[("stars", 3.5.into())]);
        assert!(de_rate.contains("3,5"), "{de_rate}");
        let en_rate = en.t_args("item-rate", &[("stars", 3.5.into())]);
        assert!(en_rate.contains("3.5"), "{en_rate}");
        assert_eq!(en.number(1234.5), "1,234.5");
        assert_eq!(de.number(1234.5), "1.234,5");
        assert_eq!(de.number(-0.25), "-0,25");
        assert_eq!(en.number(3.0), "3");
        assert_eq!(de.number(1_000_000.0), "1.000.000");
        let mut n = FluentNumber::from(2.5);
        n.options.minimum_fraction_digits = Some(2);
        n.options.minimum_integer_digits = Some(2);
        n.options.use_grouping = false;
        assert_eq!(format_number("de", &n).as_deref(), Some("02,50"));
    }

    #[test]
    fn dates_and_times_follow_the_locale() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        assert_eq!(en.date("2026-11-06"), "Nov 6, 2026");
        assert_eq!(de.date("2026-11-06T21:05:00Z"), "06.11.2026");
        assert_eq!(
            en.datetime("2026-11-06T21:05:00Z"),
            "Nov 6, 2026, 9:05 PM UTC"
        );
        assert_eq!(de.datetime("2026-11-06T21:05:00Z"), "06.11.2026, 21:05 UTC");
        assert_eq!(en.time(0, 30), "12:30 AM");
        assert_eq!(de.time(8, 0), "08:00");
        assert_eq!(en.date("not a date"), "not a date");
        for m in 1..=12 {
            let key = format!("month-short-{m}");
            assert_ne!(en.t(&key), key);
        }
    }

    #[test]
    fn every_auth_error_code_has_a_message() {
        for l in LOCALES {
            let i = I18n::new(l, BTreeMap::new());
            for c in contracts::auth::AuthErrorCode::ALL {
                let key = format!("auth-error-{}", c.code());
                assert_ne!(i.t(&key), key, "{l} misses {key}");
            }
        }
    }

    /// UX0-6, owner rulings DEC-171: "People" names the page, "Completed" the status.
    #[test]
    fn people_and_completed_wording() {
        let (en, de) = (
            I18n::new("en", BTreeMap::new()),
            I18n::new("de", BTreeMap::new()),
        );
        assert_eq!(en.t("companions-title"), "People");
        assert_eq!(de.t("companions-title"), "Personen");
        assert_eq!(en.t("item-rating"), "Your rating");
        assert_eq!(de.t("item-rating"), "Deine Bewertung");
        for l in LOCALES {
            let i = I18n::new(l, BTreeMap::new());
            for key in [
                "companions-title",
                "companions-add",
                "companions-add-open",
                "companions-lead",
            ] {
                let text = i.t(key).to_lowercase();
                assert!(
                    !text.contains("companion") && !text.contains("begleit"),
                    "{l} {key}: {text}"
                );
            }
        }
    }

    #[test]
    fn negotiates_accept_language() {
        assert_eq!(negotiate(Some("de-AT,de;q=0.9,en;q=0.8"), "en"), "de");
        assert_eq!(negotiate(Some("fr-FR,en;q=0.5"), "de"), "en");
        assert_eq!(negotiate(Some("fr"), "de"), "de");
        assert_eq!(negotiate(None, "en"), "en");
    }

    #[test]
    fn the_user_choice_wins_then_the_browser_then_the_brand() {
        assert_eq!(effective_locale(Some("de"), Some("en"), "en"), "de");
        assert_eq!(effective_locale(None, Some("de-DE"), "en"), "de");
        assert_eq!(effective_locale(None, None, "de"), "de");
        // an unsupported stored or brand locale is ignored
        assert_eq!(effective_locale(Some("fr"), None, "de"), "de");
        assert_eq!(effective_locale(None, None, "xx"), "en");
        assert_eq!(supported("DE-at"), Some("de"));
    }
}
