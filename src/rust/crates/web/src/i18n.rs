//! Every user-visible string goes through here (D-10). Messages are Fluent files embedded at
//! build time (`locales/<lang>.ftl`); the brand can override any key at runtime
//! (`[strings.<lang>]` in `brand.toml`), which is why this uses Fluent rather than a
//! compile-time catalogue (DEC-090).

use std::collections::BTreeMap;
use std::sync::Arc;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use leptos::prelude::*;
use unic_langid::LanguageIdentifier;

/// Supported locales; the first is the fallback for missing keys.
pub const LOCALES: [&str; 2] = ["en", "de"];

fn source(locale: &str) -> &'static str {
    match locale {
        "de" => include_str!("../locales/de.ftl"),
        _ => include_str!("../locales/en.ftl"),
    }
}

fn bundle(locale: &str) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = locale.parse().unwrap_or_default();
    let mut b = FluentBundle::new_concurrent(vec![id]);
    // no Unicode isolation marks around placeables: output stays plain text
    b.set_use_isolating(false);
    let res =
        FluentResource::try_new(source(locale).to_string()).unwrap_or_else(|(res, _errors)| res);
    let _ = b.add_resource(res);
    b
}

pub struct I18n {
    locale: String,
    bundle: FluentBundle<FluentResource>,
    fallback: FluentBundle<FluentResource>,
    overrides: BTreeMap<String, String>,
}

impl I18n {
    /// Messages for `locale` (unknown locales fall back to the first of [`LOCALES`]) with the
    /// brand's overrides for that locale.
    #[must_use]
    pub fn new(locale: &str, overrides: BTreeMap<String, String>) -> Self {
        let locale = if LOCALES.contains(&locale) {
            locale
        } else {
            LOCALES[0]
        };
        Self {
            locale: locale.to_string(),
            bundle: bundle(locale),
            fallback: bundle(LOCALES[0]),
            overrides,
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

    /// Text for `key` with arguments (`{ $name }` in the message).
    #[must_use]
    pub fn t_args(&self, key: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        if let Some(text) = self.overrides.get(key) {
            let mut out = text.clone();
            for (k, v) in args {
                let value = match v {
                    FluentValue::String(s) => s.to_string(),
                    FluentValue::Number(n) => n.as_string().to_string(),
                    _ => String::new(),
                };
                out = out
                    .replace(&format!("{{ ${k} }}"), &value)
                    .replace(&format!("{{${k}}}"), &value);
            }
            return out;
        }
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
}

/// Pick a supported locale from an `Accept-Language` header.
#[must_use]
pub fn negotiate(accept_language: Option<&str>, default: &str) -> String {
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
        .unwrap_or_else(|| default.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn brand_overrides_win() {
        let mut o = BTreeMap::new();
        o.insert("nav-library".to_string(), "Shelf".to_string());
        o.insert("item-source".to_string(), "Data by { $name }".to_string());
        let en = I18n::new("en", o);
        assert_eq!(en.t("nav-library"), "Shelf");
        assert_eq!(
            en.t_args("item-source", &[("name", "TMDB".into())]),
            "Data by TMDB"
        );
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

    #[test]
    fn every_key_exists_in_every_locale() {
        let keys = |src: &str| -> Vec<String> {
            src.lines()
                .filter(|l| !l.starts_with(['#', ' ', '*', '[']) && l.contains(" ="))
                .map(|l| l.split(" =").next().unwrap_or_default().to_string())
                .collect()
        };
        let en = keys(source("en"));
        for l in LOCALES {
            let other = keys(source(l));
            for k in &en {
                assert!(other.contains(k), "{l} misses {k}");
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
}
