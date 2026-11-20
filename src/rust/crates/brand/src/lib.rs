//! White-label configuration (D-10): product name, short name, locale, theme tokens, logos,
//! string overrides. Everything brand-specific is read from a brand directory
//! (`config/brand/<name>/brand.toml`); code only ever uses values from [`Brand`].
#![deny(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod contrast;
mod fonts;

pub use fonts::{FONT_ROUTE, FontAsset, FontFile, Fonts, SYSTEM_SANS};

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
    /// Where the source of this build lives (About page, #18).
    #[serde(default)]
    pub source_url: Option<String>,
    /// The documentation (About page).
    #[serde(default)]
    pub docs_url: Option<String>,
    /// The release notes of a version; `{version}` is replaced with the running one
    /// (`1.1.0`). The footer's version and the About page link it.
    #[serde(default)]
    pub release_notes_url: Option<String>,
    /// Where to report a security problem privately (About page).
    #[serde(default)]
    pub security_url: Option<String>,
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
    /// Optional self-hosted UI font (design.md 2.8).
    #[serde(default)]
    pub fonts: Option<Fonts>,
    /// Desktop bundle identifier (Tauri), reverse-DNS.
    #[serde(default)]
    pub bundle_id: Option<String>,
    /// Directory the brand was loaded from (not part of the file).
    #[serde(skip)]
    pub dir: PathBuf,
    /// Font files read by [`Brand::load`] (not part of the file).
    #[serde(skip)]
    font_assets: Vec<FontAsset>,
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
        if let Some(f) = &b.fonts {
            b.font_assets = f.load(dir)?;
        }
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
        if let Some(f) = &b.fonts {
            f.validate()?;
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

    /// CSS that sets the brand's theme tokens (`:root:not(.dark) { ... } :root.dark { ... }`).
    ///
    /// The selectors outrank `:root` and `.dark` of the stylesheet, so the brand wins whatever
    /// the order of the style elements in the head, and light values never leak into the dark
    /// theme (a theme without values keeps the stylesheet defaults).
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
            block(":root:not(.dark)", &self.theme),
            block(":root.dark", &self.theme_dark)
        )
    }

    /// `@font-face` rules for the brand font (same-origin URLs) and `--font-sans`; empty without
    /// a `[fonts]` section.
    #[must_use]
    pub fn font_css(&self) -> String {
        self.fonts
            .as_ref()
            .map(|f| f.css(&self.font_assets))
            .unwrap_or_default()
    }

    /// Everything the page head needs from the brand: fonts, then theme tokens.
    #[must_use]
    pub fn head_css(&self) -> String {
        format!("{}{}", self.font_css(), self.theme_css())
    }

    /// Font files to serve, read at load.
    #[must_use]
    pub fn font_assets(&self) -> &[FontAsset] {
        &self.font_assets
    }

    /// The font served at `url` (a path below [`FONT_ROUTE`]).
    #[must_use]
    pub fn font_asset(&self, url: &str) -> Option<&FontAsset> {
        self.font_assets.iter().find(|a| a.url == url)
    }

    /// Every colour pair the stylesheet and the components rely on, per theme, with its WCAG
    /// ratio: the accent pair, the accent on the page surfaces and on its soft surface, and
    /// per group the tint and the text on its soft surface (DEC-540). Tokens the brand does
    /// not set take the stylesheet fallbacks.
    #[must_use]
    pub fn contrast_checks(&self) -> Vec<contrast::PairCheck> {
        contrast::checks(&self.theme, &self.theme_dark)
    }

    /// The pairs of [`Brand::contrast_checks`] below their threshold (4.5:1 for text, WCAG
    /// 1.4.3; 3:1 for bars and markers, 1.4.11) or not checkable. The server logs them as
    /// warnings at start-up (design.md 7.4).
    #[must_use]
    pub fn contrast_findings(&self) -> Vec<contrast::PairCheck> {
        self.contrast_checks()
            .into_iter()
            .filter(contrast::PairCheck::fails)
            .collect()
    }

    /// The accent pair (`primary` / `primary-foreground`) of the light and the dark theme
    /// with its contrast ratio, defaults filled in (the admin Branding page, design.md 6.13).
    #[must_use]
    pub fn accent_pairs(&self) -> Vec<contrast::ContrastFinding> {
        contrast::pairs(&self.theme, &self.theme_dark)
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
        assert_eq!(css, ":root:not(.dark){--primary:oklch(0.5 0.2 250);}");
    }

    #[test]
    fn theme_css_beats_the_stylesheet_defaults() {
        // `:root:not(.dark)` and `:root.dark` outrank `:root` and `.dark` of app.css whatever
        // the order of the style elements; light values do not leak into the dark theme
        let b = Brand::parse(
            "name = \"A\"\nshort_name = \"a\"\n[theme]\nprimary = \"#7008e7\"\n[theme_dark]\nprimary = \"#a684ff\"\n",
        )
        .unwrap();
        assert_eq!(
            b.theme_css(),
            ":root:not(.dark){--primary:#7008e7;}:root.dark{--primary:#a684ff;}"
        );
    }

    #[test]
    fn rejects_empty_names() {
        assert!(Brand::parse("name = \"\"\nshort_name = \"x\"\n").is_err());
        assert!(Brand::parse("name = \"x\"\nshort_name = \"!!\"\n").is_err());
    }
}
