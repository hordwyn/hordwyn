//! Writes the static site: one `index.html` per page and locale, the stylesheet under a
//! fingerprinted name, the brand fonts, `security.txt`, `robots.txt` and the site map.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use brand::Brand;
use leptos::prelude::*;

use crate::content::{PLACEHOLDER, Site, SiteConfig};
use crate::layout::{Ctx, Head, document};
use crate::pages::{FeaturesPage, HomePage, TextPage};

/// Where the inputs are and where the site goes.
#[derive(Debug, Clone)]
pub struct Options {
    pub content: PathBuf,
    pub brand: PathBuf,
    /// The compiled stylesheet (Tailwind output of `src/web/style/site.css`).
    pub css: PathBuf,
    /// Static files copied to `/assets/` (the theme script).
    pub assets: PathBuf,
    /// Vendored icon sets (`<set>/LICENSE`), published as `/licenses/<set>.txt`: the notices
    /// travel with the inline icons (ISC, MIT).
    pub licenses: PathBuf,
    pub out: PathBuf,
}

/// A rendered page: path below the output directory and the document.
#[derive(Debug, Clone)]
pub struct Rendered {
    pub file: String,
    pub html: String,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn render<V: IntoView + 'static>(f: impl FnOnce() -> V + 'static) -> String {
    Owner::new().with(|| f().into_view().to_html())
}

/// Paths of the pages without locale prefix, in site-map order; the not-found page is not
/// among them.
#[must_use]
pub fn page_paths() -> Vec<String> {
    let mut paths = vec!["/".to_string(), "/features/".to_string()];
    paths.extend(
        crate::content::PAGE_SLUGS
            .iter()
            .filter(|s| **s != "not-found")
            .map(|s| format!("/{s}/")),
    );
    paths
}

/// Render every page of every locale. `head` carries what is the same on all pages
/// (stylesheet address, brand CSS, font preloads).
#[must_use]
pub fn render_all(site: &Arc<Site>, head: &Head) -> Vec<Rendered> {
    let mut out = Vec::new();
    for (index, locale) in site.locales.iter().enumerate() {
        let root = site.config.root(&locale.code);
        let ctx = |path: &str| Ctx {
            site: Arc::clone(site),
            locale: index,
            path: path.to_string(),
        };
        let mut push =
            |ctx: &Ctx, file: String, title: &str, description: &str, body: String, noindex| {
                let image = format!(
                    "/assets/shots/{}-{}-light.png",
                    locale.home.screenshots.hero.view, locale.code
                );
                let head = Head {
                    title: title.to_string(),
                    description: description.to_string(),
                    noindex,
                    image: Some(image),
                    ..head.clone()
                };
                out.push(Rendered {
                    file,
                    html: document(ctx, &head, &body),
                });
            };
        let home = ctx("/");
        let body = render({
            let ctx = home.clone();
            move || view! { <HomePage ctx/> }
        });
        push(
            &home,
            format!("{root}/index.html"),
            &locale.home.title,
            &locale.home.description,
            body,
            false,
        );
        let features = ctx("/features/");
        let body = render({
            let ctx = features.clone();
            move || view! { <FeaturesPage ctx/> }
        });
        push(
            &features,
            format!("{root}/features/index.html"),
            &locale.features.title,
            &locale.features.description,
            body,
            false,
        );
        for page in &locale.pages {
            let missing = page.slug == "not-found";
            let (path, file) = if missing {
                ("/404.html".to_string(), format!("{root}/404.html"))
            } else {
                (
                    format!("/{}/", page.slug),
                    format!("{root}/{}/index.html", page.slug),
                )
            };
            let page_ctx = ctx(&path);
            let body = render({
                let ctx = page_ctx.clone();
                let page = page.clone();
                move || view! { <TextPage ctx page/> }
            });
            push(
                &page_ctx,
                file,
                &page.meta.title,
                &page.meta.description,
                body,
                missing,
            );
        }
    }
    out
}

/// `/.well-known/security.txt` (RFC 9116). Without a contact the file says so in comments and
/// carries no fields, so nobody is sent to an address that does not exist.
#[must_use]
pub fn security_txt(config: &SiteConfig) -> String {
    if config.security_contact.is_empty() || config.security_expires.is_empty() {
        return format!(
            "# {PLACEHOLDER}: no security contact is published yet.\n\
             # Set `security_contact` and `security_expires` in the website's site.toml.\n\
             # Policy: {}/security/\n",
            config.base_url
        );
    }
    let mut out = String::new();
    let _ = writeln!(out, "Contact: {}", config.security_contact);
    let _ = writeln!(out, "Expires: {}", config.security_expires);
    let _ = writeln!(out, "Preferred-Languages: {}", config.locales.join(", "));
    let _ = writeln!(
        out,
        "Canonical: {}/.well-known/security.txt",
        config.base_url
    );
    let _ = writeln!(out, "Policy: {}/security/", config.base_url);
    out
}

/// The site map: every page in every locale with its alternates.
#[must_use]
pub fn sitemap(config: &SiteConfig) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n",
    );
    for path in page_paths() {
        for locale in &config.locales {
            let _ = writeln!(
                out,
                "  <url><loc>{}{}{path}</loc>",
                config.base_url,
                config.root(locale)
            );
            for alt in &config.locales {
                let _ = writeln!(
                    out,
                    "    <xhtml:link rel=\"alternate\" hreflang=\"{alt}\" href=\"{}{}{path}\"/>",
                    config.base_url,
                    config.root(alt)
                );
            }
            out.push_str("  </url>\n");
        }
    }
    out.push_str("</urlset>\n");
    out
}

fn write(out: &Path, file: &str, bytes: &[u8]) -> Result<()> {
    let path = out.join(file.trim_start_matches('/'));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(&path, bytes).with_context(|| format!("writing {}", path.display()))
}

/// Empty the output directory, so a page or stylesheet of an earlier build cannot stay
/// behind and be tested or published. Only a directory that is empty or holds an earlier
/// build (it has the generator's `robots.txt` and `sitemap.xml`) is accepted.
fn prepare_out(out: &Path) -> Result<()> {
    let Ok(mut entries) = std::fs::read_dir(out) else {
        return Ok(());
    };
    if entries.next().is_none() {
        return Ok(());
    }
    if !(out.join("robots.txt").is_file() && out.join("sitemap.xml").is_file()) {
        bail!(
            "{} is not empty and not an earlier build of the website: refusing to clear it",
            out.display()
        );
    }
    std::fs::remove_dir_all(out).with_context(|| format!("clearing {}", out.display()))
}

/// Build the site into `options.out`. Returns the number of files written.
///
/// # Errors
/// Unreadable content, brand or stylesheet; an output directory that cannot be written.
pub fn build(options: &Options) -> Result<usize> {
    let site = Arc::new(Site::load(&options.content)?);
    let brand = Brand::load(&options.brand).context("loading the brand")?;
    let css = std::fs::read(&options.css)
        .with_context(|| format!("reading the stylesheet {}", options.css.display()))?;
    if css.is_empty() {
        bail!("the stylesheet {} is empty", options.css.display());
    }
    let css_href = format!("/assets/site.{:016x}.css", fnv1a(&css));
    let head = Head {
        brand_css: brand.head_css(),
        css_href: css_href.clone(),
        font_preloads: brand
            .font_assets()
            .iter()
            .filter(|a| a.style == "normal")
            .map(|a| a.url.clone())
            .collect(),
        ..Head::default()
    };
    prepare_out(&options.out)?;
    let mut count = 0;
    for page in render_all(&site, &head) {
        write(&options.out, &page.file, page.html.as_bytes())?;
        count += 1;
    }
    write(&options.out, &css_href, &css)?;
    for font in brand.font_assets() {
        write(&options.out, &font.url, &font.bytes)?;
    }
    // licence texts travel with the files they cover (OFL 1.1 section 2)
    let ofl = options.brand.join("fonts/OFL.txt");
    if ofl.exists() {
        write(&options.out, "/brand/fonts/OFL.txt", &std::fs::read(&ofl)?)?;
        count += 1;
    }
    let mut assets: Vec<_> = std::fs::read_dir(&options.assets)
        .with_context(|| format!("reading {}", options.assets.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    assets.sort();
    for path in &assets {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        write(
            &options.out,
            &format!("/assets/{name}"),
            &std::fs::read(path)?,
        )?;
    }
    // captures of the application (content/shots, real ones with sample data)
    for name in site.shots.keys() {
        write(
            &options.out,
            &format!("/assets/shots/{name}"),
            &std::fs::read(options.content.join("shots").join(name))?,
        )?;
    }
    let mut sets: Vec<_> = std::fs::read_dir(&options.licenses)
        .with_context(|| format!("reading {}", options.licenses.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("LICENSE").is_file())
        .collect();
    sets.sort();
    for set in &sets {
        let name = set.file_name().unwrap_or_default().to_string_lossy();
        write(
            &options.out,
            &format!("/licenses/{name}.txt"),
            &std::fs::read(set.join("LICENSE"))?,
        )?;
    }
    write(
        &options.out,
        "/.well-known/security.txt",
        security_txt(&site.config).as_bytes(),
    )?;
    write(
        &options.out,
        "/robots.txt",
        format!(
            "User-agent: *\nAllow: /\nSitemap: {}/sitemap.xml\n",
            site.config.base_url
        )
        .as_bytes(),
    )?;
    write(
        &options.out,
        "/sitemap.xml",
        sitemap(&site.config).as_bytes(),
    )?;
    Ok(count + 1 + brand.font_assets().len() + assets.len() + site.shots.len() + sets.len() + 3)
}
