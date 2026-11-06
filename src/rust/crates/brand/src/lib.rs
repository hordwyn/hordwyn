//! White-label configuration (D-10): product name, short name, locale, theme tokens, logos,
//! string overrides. Everything brand-specific is read from a brand directory
//! (`config/brand/<name>/brand.toml`); code only ever uses values from [`Brand`].
#![deny(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BrandError {
    #[error("reading {0}: {1}")]
    Read(String, std::io::Error),
    #[error("invalid brand.toml: {0}")]
    Parse(String),
    #[error("invalid brand: {0}")]
    Invalid(String),
}

/// `brand.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brand {
    /// Product name shown to users.
    pub name: String,
    /// Short name (PWA, tab titles). Also the base for cookie names.
    pub short_name: String,
    #[serde(default = "default_locale")]
    pub default_locale: String,
    /// Support or project URL shown on the about page.
    #[serde(default)]
    pub support_url: Option<String>,
    /// Legal links (imprint, privacy policy) shown in the footer.
    #[serde(default)]
    pub legal: BTreeMap<String, String>,
    /// CSS custom properties for the light theme (`--primary` -> `oklch(...)` etc.).
    #[serde(default)]
    pub theme: BTreeMap<String, String>,
    /// The same for the dark theme.
    #[serde(default)]
    pub theme_dark: BTreeMap<String, String>,
    /// File names (relative to the brand directory) of logos and icons.
    #[serde(default)]
    pub logos: Logos,
    /// i18n key -> text overrides (per locale: `[strings.en]`).
    #[serde(default)]
    pub strings: BTreeMap<String, BTreeMap<String, String>>,
    /// Desktop bundle identifier (Tauri), reverse-DNS.
    #[serde(default)]
    pub bundle_id: Option<String>,
    /// Directory the brand was loaded from (not part of the file).
    #[serde(skip)]
    pub dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logos {
    pub light: Option<String>,
    pub dark: Option<String>,
    pub favicon: Option<String>,
}

fn default_locale() -> String {
    "en".into()
}

impl Brand {
    /// Load `<dir>/brand.toml`.
    ///
    /// # Errors
    /// Missing or invalid file.
    pub fn load(dir: &Path) -> Result<Self, BrandError> {
        let path = dir.join("brand.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| BrandError::Read(path.display().to_string(), e))?;
        let mut b = Self::parse(&text)?;
        b.dir = dir.to_path_buf();
        Ok(b)
    }

    /// # Errors
    /// Invalid TOML or values.
    pub fn parse(text: &str) -> Result<Self, BrandError> {
        let b: Self = toml::from_str(text).map_err(|e| BrandError::Parse(e.to_string()))?;
        if b.name.trim().is_empty() || b.short_name.trim().is_empty() {
            return Err(BrandError::Invalid(
                "name and short_name are required".into(),
            ));
        }
        if b.ident().is_empty() {
            return Err(BrandError::Invalid(
                "short_name needs at least one ASCII letter or digit".into(),
            ));
        }
        Ok(b)
    }

    /// Lowercase ASCII identifier derived from `short_name`: cookie prefix, header prefix.
    #[must_use]
    pub fn ident(&self) -> String {
        let s: String = self
            .short_name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        s
    }

    /// `User-Agent` for outgoing requests (D-10), e.g. `name/0.1.0 (+support_url)`.
    #[must_use]
    pub fn user_agent(&self, version: &str) -> String {
        match &self.support_url {
            Some(u) => format!("{}/{version} (+{u})", self.ident()),
            None => format!("{}/{version}", self.ident()),
        }
    }

    /// Text for an i18n key, if the brand overrides it.
    #[must_use]
    pub fn string(&self, locale: &str, key: &str) -> Option<&str> {
        self.strings
            .get(locale)
            .and_then(|m| m.get(key))
            .map(String::as_str)
    }

    /// CSS that sets the brand's theme tokens (`:root { ... } .dark { ... }`).
    #[must_use]
    pub fn theme_css(&self) -> String {
        let block = |sel: &str, vars: &BTreeMap<String, String>| {
            if vars.is_empty() {
                return String::new();
            }
            let mut body = String::new();
            for (k, v) in vars
                .iter()
                .filter(|(k, v)| valid_css_name(k) && valid_css_value(v))
            {
                let _ = write!(body, "--{}:{v};", k.trim_start_matches("--"));
            }
            format!("{sel}{{{body}}}")
        };
        format!(
            "{}{}",
            block(":root", &self.theme),
            block(".dark", &self.theme_dark)
        )
    }
}

fn valid_css_name(k: &str) -> bool {
    let k = k.trim_start_matches("--");
    !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Theme values come from the operator's config, but they end up inside a `<style>` element:
/// refuse anything that could close it or start a new rule.
fn valid_css_value(v: &str) -> bool {
    !v.is_empty() && !v.contains(['<', '>', '{', '}', ';', '\\']) && !v.contains("*/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_brand_loads() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../config/brand/default");
        let b = Brand::load(&dir).unwrap();
        assert!(!b.name.is_empty(), "brand name");
        assert_eq!(b.default_locale, "en");
    }

    #[test]
    fn ident_and_user_agent() {
        let b = Brand::parse("name = \"My Shelf\"\nshort_name = \"My Shelf!\"\n").unwrap();
        assert_eq!(b.ident(), "myshelf");
        assert_eq!(b.user_agent("1.0"), "myshelf/1.0");
    }

    #[test]
    fn theme_css_drops_unsafe_values() {
        let b = Brand::parse(
            "name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary = \"oklch(0.5 0.2 250)\"\nevil = \"red;}</style>\"\n",
        )
        .unwrap();
        let css = b.theme_css();
        assert_eq!(css, ":root{--primary:oklch(0.5 0.2 250);}");
    }

    #[test]
    fn rejects_empty_names() {
        assert!(Brand::parse("name = \"\"\nshort_name = \"x\"\n").is_err());
        assert!(Brand::parse("name = \"x\"\nshort_name = \"!!\"\n").is_err());
    }
}
