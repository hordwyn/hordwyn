//! Notification texts: Fluent messages per event (and optionally per channel), built in for
//! every supported locale, overridable by the brand (`[strings.<lang>]` in `brand.toml`) and
//! by admins, in that order.
//!
//! Lookup for an event `social.link_request` on channel `email`:
//! `social-link_request-email-title`, then `social-link_request-title` (same for `-body`).

use std::collections::BTreeMap;

use contracts::locale::{Digits, format_decimal};
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::types::FluentNumberStyle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use crate::catalogue::message_id;

/// Supported locales; the first is the fallback.
pub use contracts::locale::LOCALES;

fn source(locale: &str) -> &'static str {
    match locale {
        "de" => include_str!("../locales/de.ftl"),
        _ => include_str!("../locales/en.ftl"),
    }
}

/// Numbers in texts follow the locale (`1.200` in German, DEC-200); the bundle takes a plain
/// function, so there is one per locale.
fn number(locale: &str, v: &FluentValue<'_>) -> Option<String> {
    let FluentValue::Number(n) = v else {
        return None;
    };
    let o = &n.options;
    (o.style == FluentNumberStyle::Decimal).then(|| {
        let digits = Digits {
            min_integer: o.minimum_integer_digits.unwrap_or(1),
            min_fraction: o.minimum_fraction_digits.unwrap_or(0),
            max_fraction: o.maximum_fraction_digits.unwrap_or(3),
            grouping: o.use_grouping,
        };
        format_decimal(locale, n.value, digits)
    })
}

fn formatter_en<M>(v: &FluentValue<'_>, _: &M) -> Option<String> {
    number("en", v)
}

fn formatter_de<M>(v: &FluentValue<'_>, _: &M) -> Option<String> {
    number("de", v)
}

/// `key -> text` overrides per locale.
pub type Overrides = BTreeMap<String, BTreeMap<String, String>>;

/// One override as a Fluent resource (`key = text`, continuation lines indented).
fn as_resource(key: &str, text: &str) -> Option<FluentResource> {
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

pub struct Templates {
    bundles: BTreeMap<&'static str, FluentBundle<FluentResource>>,
    product: String,
}

/// Rendered title and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    pub title: String,
    pub body: String,
}

impl Templates {
    /// Built-in texts with brand overrides, then admin overrides (both `locale -> key -> text`).
    #[must_use]
    pub fn new(product: &str, brand: &Overrides, admin: &Overrides) -> Self {
        let bundles = LOCALES
            .iter()
            .map(|&locale| {
                let id: LanguageIdentifier = locale.parse().unwrap_or_default();
                let mut b = FluentBundle::new_concurrent(vec![id]);
                b.set_use_isolating(false);
                b.set_formatter(Some(if locale == "de" {
                    formatter_de
                } else {
                    formatter_en
                }));
                let _ = b.add_builtins();
                let res = FluentResource::try_new(source(locale).to_string())
                    .unwrap_or_else(|(res, _errors)| res);
                let _ = b.add_resource(res);
                for layer in [brand, admin] {
                    for (key, text) in layer.get(locale).into_iter().flatten() {
                        if let Some(r) = as_resource(key, text) {
                            b.add_resource_overriding(r);
                        }
                    }
                }
                (locale, b)
            })
            .collect();
        Self {
            bundles,
            product: product.to_string(),
        }
    }

    fn format(
        &self,
        locale: &str,
        keys: &[String],
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<String> {
        let mut fargs = FluentArgs::new();
        fargs.set("product", FluentValue::from(self.product.clone()));
        for (k, v) in args {
            let value = match v {
                serde_json::Value::String(s) => FluentValue::from(s.clone()),
                serde_json::Value::Number(n) => n
                    .as_f64()
                    .map_or_else(|| FluentValue::from(n.to_string()), FluentValue::from),
                other => FluentValue::from(other.to_string()),
            };
            fargs.set(k.clone(), value);
        }
        let locales = [locale, LOCALES[0]];
        for l in locales {
            let Some(b) = self.bundles.get(l) else {
                continue;
            };
            for key in keys {
                if let Some(pattern) = b.get_message(key).and_then(|m| m.value()) {
                    let mut errors = vec![];
                    return Some(
                        b.format_pattern(pattern, Some(&fargs), &mut errors)
                            .into_owned(),
                    );
                }
            }
        }
        None
    }

    /// Title and body of an event for a channel kind (`inbox`, `email`, ...).
    #[must_use]
    pub fn render(
        &self,
        event: &str,
        channel: &str,
        locale: &str,
        args: &serde_json::Map<String, serde_json::Value>,
    ) -> Rendered {
        let id = message_id(event);
        let keys = |part: &str| vec![format!("{id}-{channel}-{part}"), format!("{id}-{part}")];
        Rendered {
            title: self
                .format(locale, &keys("title"), args)
                .unwrap_or_else(|| event.to_string()),
            body: self.format(locale, &keys("body"), args).unwrap_or_default(),
        }
    }

    /// Title of a digest with `count` entries.
    #[must_use]
    pub fn digest_title(&self, locale: &str, count: usize) -> String {
        let mut args = serde_json::Map::new();
        args.insert("count".into(), count.into());
        self.format(locale, &["digest-title".into()], &args)
            .unwrap_or_else(|| format!("{count}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::CATALOGUE;

    fn args(pairs: &[(&str, &str)]) -> serde_json::Map<String, serde_json::Value> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), serde_json::Value::from(*v)))
            .collect()
    }

    #[test]
    fn every_message_belongs_to_an_event_or_the_digest() {
        // `<event>-title`, `<event>-<channel>-body`, ...; parity across locales is checked by
        // the repository test (xtask/tests/locales.rs)
        let events: Vec<String> = CATALOGUE
            .iter()
            .map(|e| message_id(e.id))
            .chain(["digest".to_string(), "test".to_string()])
            .collect();
        let keys = source("en")
            .lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_lowercase()) && l.contains(" ="))
            .map(|l| l.split(" =").next().unwrap_or_default());
        for key in keys {
            let base = key
                .strip_suffix("-title")
                .or_else(|| key.strip_suffix("-body"))
                .unwrap_or_else(|| panic!("{key}: neither -title nor -body"));
            assert!(
                events.iter().any(|e| base == e
                    || base
                        .strip_prefix(e.as_str())
                        .is_some_and(|c| c.starts_with('-'))),
                "{key} belongs to no event"
            );
        }
    }

    #[test]
    fn numbers_in_texts_follow_the_locale() {
        let t = Templates::new("Shelf", &Overrides::new(), &Overrides::new());
        let mut a = serde_json::Map::new();
        a.insert("imported".into(), 1200.into());
        a.insert("errors".into(), 1.into());
        let de = t.render("import.finished", "inbox", "de", &a);
        assert_eq!(
            de.body,
            "1.200 Eintr\u{e4}ge importiert, eine Zeile mit Problemen."
        );
        let en = t.render("import.finished", "inbox", "en", &a);
        assert_eq!(en.body, "1,200 entries imported, one row with problems.");
    }

    #[test]
    fn every_event_has_texts_in_every_locale() {
        let t = Templates::new("Shelf", &Overrides::new(), &Overrides::new());
        for e in CATALOGUE {
            for l in LOCALES {
                let b = &t.bundles[l];
                let id = message_id(e.id);
                for part in ["title", "body"] {
                    assert!(
                        b.has_message(&format!("{id}-{part}")),
                        "{l}: {id}-{part} missing"
                    );
                }
            }
        }
    }

    #[test]
    fn renders_with_arguments_and_layers_overrides() {
        let a = args(&[("from", "Anna"), ("companion", "B.")]);
        let plain = Templates::new("Shelf", &Overrides::new(), &Overrides::new());
        let r = plain.render("social.link_request", "inbox", "en", &a);
        assert!(r.title.contains("Anna"), "{r:?}");
        let mut brand = Overrides::new();
        brand.entry("en".into()).or_default().insert(
            "social-link_request-title".into(),
            "{ $from } wants to link on { $product }".into(),
        );
        let mut admin = Overrides::new();
        admin.entry("en".into()).or_default().insert(
            "social-link_request-email-title".into(),
            "Mail: { $from }".into(),
        );
        let t = Templates::new("Shelf", &brand, &admin);
        assert_eq!(
            t.render("social.link_request", "inbox", "en", &a).title,
            "Anna wants to link on Shelf"
        );
        assert_eq!(
            t.render("social.link_request", "email", "en", &a).title,
            "Mail: Anna"
        );
        // unknown locale falls back to English; invalid override keys are ignored
        assert_eq!(
            t.render("social.link_request", "inbox", "fr", &a).title,
            "Anna wants to link on Shelf"
        );
        assert!(as_resource("bad key", "x").is_none());
        assert_eq!(t.digest_title("de", 3), "3 Benachrichtigungen");
    }
}
