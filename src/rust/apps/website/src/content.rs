//! Content model of the website and its loader.
//!
//! Everything a visitor reads lives in the content directory, not in code: `site.toml` (the
//! product name, addresses, contact slots) and one directory per locale with `strings.toml`
//! (navigation and footer labels), `home.toml` and `features.toml` (the structured pages) and
//! one Markdown file per text page. Files are read as text first and the tokens of
//! [`SiteConfig::tokens`] (`{name}`, `{root}`, `{docs}` ...) are filled in, so the product
//! name is written down once.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::markdown;

/// Marker of a slot the owner still has to fill (imprint fields, the security contact).
/// `website check` lists them; `website build --strict` refuses to build while one is left.
pub const PLACEHOLDER: &str = "PLACEHOLDER";

/// Text pages, in the order of the site map. `not-found` becomes `/404.html`.
pub const PAGE_SLUGS: &[&str] = &[
    "self-hosting",
    "hub",
    "changelog",
    "security",
    "attributions",
    "imprint",
    "privacy",
    "not-found",
];

/// `site.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    /// Product name; the only place it is written down (`{name}` everywhere else).
    pub name: String,
    /// Public address without a trailing slash, e.g. `https://example.org`.
    pub base_url: String,
    /// Locales, the first is the default and lives at `/`; the others at `/<code>/`.
    pub locales: Vec<String>,
    /// Where the documentation is published, e.g. `/docs/`.
    pub docs_url: String,
    /// The install page inside the documentation.
    pub docs_install_url: String,
    /// Address of the planned hub.
    pub hub_url: String,
    /// Language of the documentation (it is not translated): links to it from a page in
    /// another language carry `hreflang`.
    #[serde(default = "default_docs_lang")]
    pub docs_lang: String,
    /// Public source repository (`https:`). Empty: an open slot, the links are left out and
    /// strict mode refuses to publish.
    #[serde(default)]
    pub source_url: String,
    /// `Contact:` of `security.txt` (a `mailto:` or `https:` URI); empty: a marked placeholder.
    #[serde(default)]
    pub security_contact: String,
    /// `Expires:` of `security.txt` (RFC 3339); required with a contact (RFC 9116).
    #[serde(default)]
    pub security_expires: String,
}

fn default_docs_lang() -> String {
    "en".to_string()
}

impl SiteConfig {
    #[must_use]
    pub fn default_locale(&self) -> &str {
        self.locales.first().map_or("en", String::as_str)
    }

    /// Path prefix of a locale: empty for the default, `/de` for the others.
    #[must_use]
    pub fn root(&self, locale: &str) -> String {
        if locale == self.default_locale() {
            String::new()
        } else {
            format!("/{locale}")
        }
    }

    /// Tokens filled into content files of `locale`.
    #[must_use]
    pub fn tokens(&self, locale: &str) -> Vec<(&'static str, String)> {
        vec![
            ("{name}", self.name.clone()),
            ("{root}", self.root(locale)),
            ("{base}", self.base_url.clone()),
            ("{docs}", self.docs_url.clone()),
            ("{install}", self.docs_install_url.clone()),
            ("{hub}", self.hub_url.clone()),
            ("{source}", self.source_or_slot()),
            ("{security_contact}", self.security_contact_markdown()),
        ]
    }

    /// The repository address, or a marked slot while there is none.
    #[must_use]
    pub fn source_or_slot(&self) -> String {
        if self.source_url.is_empty() {
            format!("[{PLACEHOLDER}: address of the public source repository]")
        } else {
            self.source_url.clone()
        }
    }

    /// The security contact as a Markdown link, or a marked slot while there is none, so
    /// the security page and `security.txt` are filled from the same value.
    #[must_use]
    pub fn security_contact_markdown(&self) -> String {
        let contact = &self.security_contact;
        if contact.is_empty() {
            return format!("[{PLACEHOLDER}: security contact, set in site.toml]");
        }
        let label = contact.strip_prefix("mailto:").unwrap_or(contact);
        format!("[{label}]({contact})")
    }
}

/// A link with its label.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub label: String,
    pub href: String,
    /// Language of the target when it differs from the page's (the English documentation).
    #[serde(default)]
    pub hreflang: Option<String>,
}

/// Head of a section: small label, heading, optional lead text.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intro {
    pub eyebrow: String,
    pub heading: String,
    #[serde(default)]
    pub text: String,
}

/// Whether a release has it. `shipped` is a claim: strict mode publishes it only when
/// `release.toml` confirms the item's id (see [`crate::gate`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Shipped,
    Coming,
}

/// `release.toml`: what the owner (or the orchestrator) confirmed against a release tag.
/// Empty until the first release is tagged.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    /// The tag the claims were checked against, e.g. `v1.0.0`; empty: nothing is released.
    #[serde(default)]
    pub version: String,
    /// Ids of the kinds and features confirmed to be in that release.
    #[serde(default)]
    pub confirmed: Vec<String>,
}

impl Release {
    #[must_use]
    pub fn confirms(&self, id: &str) -> bool {
        !self.version.is_empty() && self.confirmed.iter().any(|c| c == id)
    }
}

/// A card: icon (semantic name of the `ui` allowlist), title, text.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Point {
    /// Id of the feature in `release.toml` (features page; empty on the landing page).
    #[serde(default)]
    pub id: String,
    /// Set for a feature of the features page.
    #[serde(default)]
    pub status: Option<Status>,
    pub icon: String,
    pub title: String,
    pub text: String,
}

/// One media kind on the landing page or the features page.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kind {
    /// Kind id of the application (`movie`, `board_game`): picks icon and group tint.
    pub kind: String,
    pub title: String,
    pub text: String,
    pub status: Status,
    /// What exactly works (features page).
    #[serde(default)]
    pub points: Vec<String>,
    /// Where the data comes from (features page).
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hero {
    pub eyebrow: String,
    pub headline: String,
    pub sub: String,
    pub primary: Link,
    pub secondary: Link,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindsSection {
    pub intro: Intro,
    pub items: Vec<Kind>,
    /// Line under the grid about the kinds that are not in v1.
    pub later: String,
    pub more: Link,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointsSection {
    pub intro: Intro,
    pub points: Vec<Point>,
}

/// A capture of the application: `content/shots/<view>-<locale>-<light|dark>.png`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shot {
    /// File stem without locale and theme: `home`, `library`.
    pub view: String,
    /// What the picture shows, for readers who do not see it.
    pub alt: String,
    #[serde(default)]
    pub caption: String,
}

/// Captures of the application (real ones, with invented sample data).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screenshots {
    pub intro: Intro,
    /// Line under the pictures: what they are.
    pub note: String,
    /// The picture in the hero of the landing page.
    pub hero: Shot,
    /// The wide picture of the section.
    pub wide: Shot,
    /// Phone-sized pictures.
    pub phones: Vec<Shot>,
}

/// Pixel size of a capture, read from its PNG header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShotSize {
    pub width: u32,
    pub height: u32,
}

/// One part of the name with its meaning.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordPart {
    pub part: String,
    pub meaning: String,
}

/// Why the product has its name.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameStory {
    pub eyebrow: String,
    pub heading: String,
    pub parts: Vec<WordPart>,
    /// The gloss under the parts (inline Markdown).
    pub gloss: String,
    /// Markdown.
    pub body: String,
    /// Markdown: where the meaning was read.
    pub sources: String,
    #[serde(skip)]
    pub gloss_html: String,
    #[serde(skip)]
    pub body_html: String,
    #[serde(skip)]
    pub sources_html: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub title: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepsSection {
    pub intro: Intro,
    pub steps: Vec<Step>,
    pub more: Link,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Teaser {
    pub intro: Intro,
    pub more: Link,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Closing {
    pub heading: String,
    pub text: String,
    pub primary: Link,
    pub secondary: Link,
}

/// `home.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Home {
    pub title: String,
    pub description: String,
    pub hero: Hero,
    pub facts: Vec<Fact>,
    pub kinds: KindsSection,
    pub together: PointsSection,
    pub screenshots: Screenshots,
    pub name: NameStory,
    pub selfhost: StepsSection,
    pub hub: Teaser,
    pub closing: Closing,
}

/// The kinds of one activity group (watch, read, listen, play).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindGroup {
    pub heading: String,
    pub items: Vec<Kind>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Later {
    pub intro: Intro,
    pub items: Vec<String>,
}

/// `features.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Features {
    pub title: String,
    pub description: String,
    pub eyebrow: String,
    pub heading: String,
    pub lead: String,
    /// Label in front of a kind's data source.
    pub source_label: String,
    pub groups: Vec<KindGroup>,
    pub everything: PointsSection,
    pub later: Later,
}

/// Front matter of a text page (`+++` TOML `+++`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageMeta {
    pub title: String,
    pub description: String,
    pub eyebrow: String,
    /// Date of the last change, ISO 8601, shown under the title.
    #[serde(default)]
    pub updated: Option<String>,
    /// Notice shown above the text while the page has open owner slots, and only then (see
    /// [`Page::open_slots`]). Without one the general notice of `strings.toml` is used.
    #[serde(default)]
    pub notice: Option<String>,
}

/// A text page: front matter and the body rendered to HTML.
#[derive(Debug, Clone)]
pub struct Page {
    pub slug: String,
    pub meta: PageMeta,
    pub body_html: String,
    /// Open owner slots in the text after the tokens were filled. The notice is derived
    /// from this number, so a filled page cannot keep it and an open one cannot lose it.
    pub open_slots: usize,
}

/// Everything of one locale.
#[derive(Debug, Clone)]
pub struct Locale {
    pub code: String,
    pub strings: BTreeMap<String, String>,
    pub home: Home,
    pub features: Features,
    pub pages: Vec<Page>,
}

impl Locale {
    /// Text of a label.
    ///
    /// # Panics
    /// If the key is missing: a content error, found by the build and by the parity test.
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.strings
            .get(key)
            .unwrap_or_else(|| panic!("strings.toml of `{}` has no key `{key}`", self.code))
            .clone()
    }

    #[must_use]
    pub fn page(&self, slug: &str) -> Option<&Page> {
        self.pages.iter().find(|p| p.slug == slug)
    }
}

/// The whole site.
#[derive(Debug, Clone)]
pub struct Site {
    pub config: SiteConfig,
    pub release: Release,
    pub locales: Vec<Locale>,
    /// Ids named by `<!-- shipped:ID -->` blocks of the text pages.
    pub conditional_ids: BTreeSet<String>,
    /// Sizes of the captures in `shots/`, by file name.
    pub shots: BTreeMap<String, ShotSize>,
}

/// One open owner slot: file (relative to the content directory), line, text of the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSlot {
    pub file: String,
    pub line: usize,
    pub text: String,
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}

fn fill(text: &str, tokens: &[(&'static str, String)]) -> String {
    tokens
        .iter()
        .fold(text.to_string(), |acc, (k, v)| acc.replace(k, v))
}

fn parse_toml<T: DeserializeOwned>(path: &Path, tokens: &[(&'static str, String)]) -> Result<T> {
    toml::from_str(&fill(&read(path)?, tokens)).with_context(|| format!("{}", path.display()))
}

/// Split `+++` front matter from the Markdown body.
///
/// # Errors
/// If the text does not start with a `+++` block.
pub fn split_front_matter(text: &str) -> Result<(&str, &str)> {
    let rest = text
        .strip_prefix("+++")
        .ok_or_else(|| anyhow!("missing `+++` front matter"))?;
    let end = rest
        .find("\n+++")
        .ok_or_else(|| anyhow!("front matter is not closed with `+++`"))?;
    Ok((&rest[..end], rest[end + 4..].trim_start_matches('\n')))
}

/// Parse a text page.
///
/// # Errors
/// Missing or invalid front matter.
pub fn parse_page(slug: &str, text: &str) -> Result<Page> {
    let (front, body) = split_front_matter(text)?;
    let meta: PageMeta = toml::from_str(front).context("front matter")?;
    Ok(Page {
        slug: slug.to_string(),
        meta,
        body_html: markdown::render(body),
        open_slots: body.matches(PLACEHOLDER).count(),
    })
}

/// Keep or drop the blocks `<!-- shipped:ID -->` ... `<!-- /shipped -->` of a text page: a
/// block stays only when `release` confirms its id. The marker lines are removed either way;
/// every id met is added to `seen`.
#[must_use]
pub fn release_blocks(text: &str, release: &Release, seen: &mut BTreeSet<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut keep = true;
    for line in text.lines() {
        let mark = line.trim();
        if let Some(id) = mark
            .strip_prefix("<!-- shipped:")
            .and_then(|rest| rest.strip_suffix("-->"))
        {
            let id = id.trim();
            seen.insert(id.to_string());
            keep = release.confirms(id);
        } else if mark == "<!-- /shipped -->" {
            keep = true;
        } else if keep {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Width and height of a PNG file (the IHDR chunk).
///
/// # Errors
/// If the file cannot be read or is not a PNG.
pub fn png_size(path: &Path) -> Result<ShotSize> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        bail!("{} is not a PNG file", path.display());
    }
    let be =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Ok(ShotSize {
        width: be(16),
        height: be(20),
    })
}

fn load_shots(dir: &Path) -> Result<BTreeMap<String, ShotSize>> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(out);
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.extension().is_some_and(|e| e == "png") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            out.insert(name.into_owned(), png_size(&path)?);
        }
    }
    Ok(out)
}

fn load_locale(
    dir: &Path,
    config: &SiteConfig,
    release: &Release,
    code: &str,
    seen: &mut BTreeSet<String>,
) -> Result<Locale> {
    let mut tokens = config.tokens(code);
    let dir = dir.join(code);
    let strings: BTreeMap<String, String> = parse_toml(&dir.join("strings.toml"), &tokens)?;
    let features: Features = parse_toml(&dir.join("features.toml"), &tokens)?;
    // the landing page counts the kinds instead of repeating a number
    let shipped = features
        .groups
        .iter()
        .flat_map(|g| &g.items)
        .filter(|k| k.status == Status::Shipped)
        .count();
    tokens.push(("{kinds_shipped}", shipped.to_string()));
    let mut home: Home = parse_toml(&dir.join("home.toml"), &tokens)?;
    home.name.gloss_html = markdown::render_inline(&home.name.gloss);
    home.name.body_html = markdown::render(&home.name.body);
    home.name.sources_html = markdown::render(&home.name.sources);
    let mut pages = Vec::new();
    for slug in PAGE_SLUGS {
        let path = dir.join(format!("{slug}.md"));
        let text = release_blocks(&fill(&read(&path)?, &tokens), release, seen);
        let page = parse_page(slug, &text).with_context(|| format!("{}", path.display()))?;
        pages.push(page);
    }
    Ok(Locale {
        code: code.to_string(),
        strings,
        home,
        features,
        pages,
    })
}

impl Site {
    /// Load `site.toml` and every locale below `dir`.
    ///
    /// # Errors
    /// A missing or invalid content file.
    pub fn load(dir: &Path) -> Result<Self> {
        let config: SiteConfig = parse_toml(&dir.join("site.toml"), &[])?;
        if config.locales.is_empty() {
            bail!("site.toml: `locales` is empty");
        }
        if config.base_url.ends_with('/') {
            bail!("site.toml: `base_url` must not end with a slash");
        }
        // tokens are filled into the text of the TOML files before they are parsed
        if config.name.contains(['"', '\\', '{', '}']) {
            bail!("site.toml: `name` must not contain a double quote, a backslash or a brace");
        }
        let release: Release = parse_toml(&dir.join("release.toml"), &[])?;
        let mut conditional_ids = BTreeSet::new();
        let locales = config
            .locales
            .iter()
            .map(|code| load_locale(dir, &config, &release, code, &mut conditional_ids))
            .collect::<Result<Vec<_>>>()?;
        let shots = load_shots(&dir.join("shots"))?;
        for locale in &locales {
            let s = &locale.home.screenshots;
            for shot in [&s.hero, &s.wide].into_iter().chain(&s.phones) {
                for theme in ["light", "dark"] {
                    let file = format!("{}-{}-{theme}.png", shot.view, locale.code);
                    if !shots.contains_key(&file) {
                        bail!(
                            "home.toml of `{}` shows the capture shots/{file}, which does not exist",
                            locale.code
                        );
                    }
                }
            }
        }
        Ok(Self {
            config,
            release,
            locales,
            conditional_ids,
            shots,
        })
    }

    /// Ids of everything the pages mark as shipped: kinds (features page and landing page)
    /// and features. The conditional blocks of the text pages are not among them: they are
    /// shown only when confirmed, so they never block.
    #[must_use]
    pub fn shipped_ids(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for locale in &self.locales {
            let kinds = locale
                .features
                .groups
                .iter()
                .flat_map(|g| &g.items)
                .chain(&locale.home.kinds.items);
            for kind in kinds.filter(|k| k.status == Status::Shipped) {
                out.insert(kind.kind.clone());
            }
            for point in &locale.features.everything.points {
                if point.status == Some(Status::Shipped) {
                    out.insert(point.id.clone());
                }
            }
        }
        out
    }

    /// Every id `release.toml` may confirm.
    #[must_use]
    pub fn known_ids(&self) -> BTreeSet<String> {
        let mut out = self.conditional_ids.clone();
        for locale in &self.locales {
            for kind in locale.features.groups.iter().flat_map(|g| &g.items) {
                out.insert(kind.kind.clone());
            }
            for point in &locale.features.everything.points {
                out.insert(point.id.clone());
            }
        }
        out
    }
}

/// Every line of the content files (`.toml`, `.md`) that still carries the [`PLACEHOLDER`]
/// marker. Comment lines of TOML files explain the marker and are not slots; in Markdown a
/// line that starts with `#` is a heading and counts.
///
/// # Errors
/// If the directory cannot be read.
pub fn open_slots(dir: &Path) -> Result<Vec<OpenSlot>> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<OpenSlot>) -> Result<()> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .with_context(|| format!("reading {}", dir.display()))?
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, out)?;
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !matches!(ext, "toml" | "md") {
                continue;
            }
            let toml = ext == "toml";
            let text = read(&path)?;
            let file = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            for (i, line) in text.lines().enumerate() {
                if line.contains(PLACEHOLDER) && !(toml && line.trim_start().starts_with('#')) {
                    out.push(OpenSlot {
                        file: file.clone(),
                        line: i + 1,
                        text: line.trim().to_string(),
                    });
                }
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_is_split_from_the_body() {
        let (front, body) = split_front_matter("+++\ntitle = \"A\"\n+++\n\nText\n").unwrap();
        assert_eq!(front.trim(), "title = \"A\"");
        assert_eq!(body, "Text\n");
        assert!(split_front_matter("Text").is_err());
        assert!(split_front_matter("+++\ntitle = 1\n").is_err());
    }

    #[test]
    fn a_page_needs_its_front_matter_keys() {
        let page = parse_page(
            "x",
            "+++\ntitle = \"T\"\ndescription = \"D\"\neyebrow = \"E\"\n+++\nSome *text*.\n",
        )
        .unwrap();
        assert_eq!(page.meta.title, "T");
        assert!(page.body_html.contains("<em>text</em>"));
        assert!(parse_page("x", "+++\ntitle = \"T\"\n+++\n").is_err());
        assert!(parse_page("x", "+++\ntitle = \"T\"\nextra = 1\n+++\n").is_err());
    }

    #[test]
    fn a_page_counts_its_open_slots() {
        let head = "+++\ntitle = \"T\"\ndescription = \"D\"\neyebrow = \"E\"\n+++\n";
        let closed = parse_page("x", &format!("{head}Text\n")).unwrap();
        assert_eq!(closed.open_slots, 0);
        let open = format!("{head}## [{PLACEHOLDER}: a]\n\n**[{PLACEHOLDER}: b]**\n");
        assert_eq!(parse_page("x", &open).unwrap().open_slots, 2);
    }

    #[test]
    fn release_blocks_stay_only_when_the_release_confirms_them() {
        let text = "a\n<!-- shipped:x -->\nb\n<!-- /shipped -->\nc\n";
        let mut seen = BTreeSet::new();
        assert_eq!(
            release_blocks(text, &Release::default(), &mut seen),
            "a\nc\n"
        );
        assert!(seen.contains("x"));
        let release = Release {
            version: "v1.0.0".into(),
            confirmed: vec!["x".into()],
        };
        assert_eq!(release_blocks(text, &release, &mut seen), "a\nb\nc\n");
        // a list of ids without a version confirms nothing
        let untagged = Release {
            version: String::new(),
            confirmed: vec!["x".into()],
        };
        assert!(!untagged.confirms("x"));
    }

    #[test]
    fn slots_are_found_in_markdown_headings_but_not_in_toml_comments() {
        let dir = std::env::temp_dir().join(format!("website-slots-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("a.md"),
            format!("## [{PLACEHOLDER}: heading]\ntext\n"),
        )
        .unwrap();
        std::fs::write(
            dir.join("b.toml"),
            format!("# {PLACEHOLDER} explained\nx = \"\" # {PLACEHOLDER}: value\n"),
        )
        .unwrap();
        std::fs::write(dir.join("c.png"), [0x89, 0xff, 0x00]).unwrap();
        let slots = open_slots(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let found: Vec<(&str, usize)> = slots.iter().map(|s| (s.file.as_str(), s.line)).collect();
        assert_eq!(found, [("a.md", 1), ("b.toml", 2)]);
    }

    #[test]
    fn tokens_are_filled_per_locale() {
        let config: SiteConfig = toml::from_str(
            "name = \"N\"\nbase_url = \"https://e.org\"\nlocales = [\"en\", \"de\"]\n\
             docs_url = \"/docs/\"\ndocs_install_url = \"/docs/i.html\"\nhub_url = \"https://h.e.org\"\n",
        )
        .unwrap();
        assert_eq!(
            fill("{name} at {root}/x/", &config.tokens("en")),
            "N at /x/"
        );
        assert_eq!(
            fill("{name} at {root}/x/", &config.tokens("de")),
            "N at /de/x/"
        );
    }
}
