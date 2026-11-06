//! Optional self-hosted web font of a brand (design.md 2.8, owner ruling DEC-171).
//!
//! Files live inside the brand directory and are served from our own origin under
//! [`FONT_ROUTE`]; nothing is loaded from a CDN. The URL carries a fingerprint of the file
//! content, so the server can mark responses as immutable.

use std::fmt::Write as _;
use std::path::{Component, Path};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::BrandError;

/// URL prefix the server serves brand fonts under: `/brand/fonts/<fingerprint>/<file name>`.
pub const FONT_ROUTE: &str = "/brand/fonts/";

/// System stack after the brand font: Tailwind v4 `--font-sans`
/// (<https://unpkg.com/tailwindcss@4.1.0/theme.css>, design.md 2.8).
pub const SYSTEM_SANS: &str = "ui-sans-serif,system-ui,sans-serif,\"Apple Color Emoji\",\"Segoe UI Emoji\",\"Segoe UI Symbol\",\"Noto Color Emoji\"";

/// `[fonts]` in `brand.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    /// Family name of the UI font; put first in `--font-sans`, before [`SYSTEM_SANS`].
    pub sans: String,
    /// `@font-face` sources (WOFF2 only), relative to the brand directory.
    #[serde(default)]
    pub files: Vec<FontFile>,
}

/// One `[[fonts.files]]` entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontFile {
    /// Path inside the brand directory, e.g. `fonts/Inter.woff2`.
    pub file: String,
    /// CSS `font-weight`: one weight (`400`) or a variable range (`200 800`).
    #[serde(default = "default_weight")]
    pub weight: String,
    /// CSS `font-style`: `normal` or `italic`.
    #[serde(default = "default_style")]
    pub style: String,
}

fn default_weight() -> String {
    "400".into()
}

fn default_style() -> String {
    "normal".into()
}

/// A font file read at start-up, ready to serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontAsset {
    /// Same-origin URL path (starts with [`FONT_ROUTE`]).
    pub url: String,
    pub weight: String,
    pub style: String,
    pub bytes: Arc<[u8]>,
}

impl FontAsset {
    /// Media type of the response (RFC 8081).
    pub const CONTENT_TYPE: &'static str = "font/woff2";
}

impl Fonts {
    pub(crate) fn validate(&self) -> Result<(), BrandError> {
        let bad = |m: String| Err(BrandError::Invalid(m));
        let family_ok = |s: &str| {
            !s.trim().is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        };
        if !family_ok(&self.sans) {
            return bad(format!(
                "fonts.sans {:?}: use letters, digits, spaces, - _ .",
                self.sans
            ));
        }
        for f in &self.files {
            let p = Path::new(&f.file);
            // the name ends up in a CSS url("...") and in the route: keep it plain
            let plain = f
                .file
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'));
            let relative = p.components().all(|c| matches!(c, Component::Normal(_)));
            if !plain || !relative || p.extension().is_none_or(|e| e != "woff2") {
                return bad(format!(
                    "fonts.files {:?}: a .woff2 path inside the brand directory \
                     (letters, digits, . _ - /)",
                    f.file
                ));
            }
            if !matches!(f.style.as_str(), "normal" | "italic") {
                return bad(format!("fonts.files style {:?}: normal or italic", f.style));
            }
            let weights: Vec<&str> = f.weight.split_whitespace().collect();
            let weight_ok = (1..=2).contains(&weights.len())
                && weights
                    .iter()
                    .all(|w| w.parse::<u16>().is_ok_and(|n| (1..=1000).contains(&n)));
            if !weight_ok {
                return bad(format!(
                    "fonts.files weight {:?}: 1..1000, or a range like \"200 800\"",
                    f.weight
                ));
            }
        }
        Ok(())
    }

    /// Read every file, check it is WOFF2 and give it a content-addressed URL.
    pub(crate) fn load(&self, dir: &Path) -> Result<Vec<FontAsset>, BrandError> {
        self.files
            .iter()
            .map(|f| {
                let path = dir.join(&f.file);
                let bytes = std::fs::read(&path)
                    .map_err(|e| BrandError::Read(path.display().to_string(), e))?;
                if !bytes.starts_with(b"wOF2") {
                    return Err(BrandError::Invalid(format!(
                        "{} is not a WOFF2 file",
                        f.file
                    )));
                }
                let name = Path::new(&f.file)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Ok(FontAsset {
                    url: format!("{FONT_ROUTE}{:016x}/{name}", fnv1a(&bytes)),
                    weight: f.weight.clone(),
                    style: f.style.clone(),
                    bytes: bytes.into(),
                })
            })
            .collect()
    }

    /// `@font-face` rules for `assets` plus `:root{--font-sans:...}`.
    pub(crate) fn css(&self, assets: &[FontAsset]) -> String {
        let mut css = String::new();
        for a in assets {
            let _ = write!(
                css,
                "@font-face{{font-family:\"{}\";src:url(\"{}\") format(\"woff2\");\
                 font-weight:{};font-style:{};font-display:swap;}}",
                self.sans, a.url, a.weight, a.style
            );
        }
        let _ = write!(css, ":root{{--font-sans:\"{}\",{SYSTEM_SANS};}}", self.sans);
        css
    }
}

/// FNV-1a, 64 bit: a stable content fingerprint for cache busting (not a security hash).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}
