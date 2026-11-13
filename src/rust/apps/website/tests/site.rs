//! The website's own content, rendered: structure every page must have, parity of the
//! locales, links that resolve, the publishing gate, and the rule that the product name lives
//! in the content directory only.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use website::content::{
    Locale, OpenSlot, PAGE_SLUGS, PLACEHOLDER, Release, Site, Status, open_slots,
};
use website::gate::{docs_findings, publish_blockers, release_findings};
use website::generate::{Rendered, page_paths, render_all, security_txt, sitemap};
use website::layout::Head;

/// A playbook of finished documentation for the product name of the content directory.
fn finished_playbook(name: &str) -> String {
    format!(
        "site:\n  title: {name} documentation\nasciidoc:\n  attributes:\n    kroki-fetch-diagram: true\n"
    )
}

fn content_dir() -> PathBuf {
    website::crate_dir().join("content")
}

fn site() -> Arc<Site> {
    Arc::new(Site::load(&content_dir()).expect("content loads"))
}

fn render(site: &Arc<Site>) -> Vec<Rendered> {
    let head = Head {
        css_href: "/assets/site.css".into(),
        ..Head::default()
    };
    render_all(site, &head)
}

fn pages() -> Vec<Rendered> {
    render(&site())
}

fn page<'a>(all: &'a [Rendered], file: &str) -> &'a str {
    &all.iter()
        .find(|p| p.file == file)
        .unwrap_or_else(|| panic!("no page {file}"))
        .html
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// Values of every `href="..."` in `html`.
fn hrefs(html: &str) -> Vec<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .map(|h| h.replace("&amp;", "&"))
        .collect()
}

fn files_below(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_below(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn is_text(path: &Path) -> bool {
    path.extension().is_none_or(|e| e != "png")
}

fn feature_kinds(locale: &Locale) -> impl Iterator<Item = &website::content::Kind> {
    locale.features.groups.iter().flat_map(|g| &g.items)
}

#[test]
fn every_page_of_every_locale_is_rendered() {
    let files: BTreeSet<String> = pages().into_iter().map(|p| p.file).collect();
    for root in ["", "/de"] {
        assert!(files.contains(&format!("{root}/index.html")), "{files:?}");
        assert!(files.contains(&format!("{root}/features/index.html")));
        assert!(files.contains(&format!("{root}/404.html")));
        for slug in PAGE_SLUGS.iter().filter(|s| **s != "not-found") {
            assert!(
                files.contains(&format!("{root}/{slug}/index.html")),
                "{root}/{slug}"
            );
        }
    }
    assert_eq!(files.len(), 2 * (2 + PAGE_SLUGS.len()));
}

#[test]
fn every_page_has_one_h1_the_landmarks_and_its_language() {
    for page in pages() {
        let html = &page.html;
        let lang = if page.file.starts_with("/de/") {
            "de"
        } else {
            "en"
        };
        assert!(html.starts_with("<!DOCTYPE html>"), "{}", page.file);
        assert!(
            html.contains(&format!("<html lang=\"{lang}\">")),
            "{}",
            page.file
        );
        assert_eq!(count(html, "<h1"), 1, "one h1: {}", page.file);
        assert_eq!(count(html, "<main"), 1, "one main: {}", page.file);
        assert_eq!(count(html, "<header"), 1, "one banner: {}", page.file);
        assert_eq!(count(html, "<footer"), 1, "one contentinfo: {}", page.file);
        assert!(html.contains("href=\"#main\""), "skip link: {}", page.file);
        assert!(html.contains("id=\"main\""), "skip target: {}", page.file);
        assert!(
            html.contains("<meta name=\"description\" content=\""),
            "{}",
            page.file
        );
        assert!(!html.contains("<title></title>"), "{}", page.file);
        // nothing is loaded from another origin
        for needle in ["src=\"http", "src=\"//", "url(http", "@import"] {
            assert!(!html.contains(needle), "{needle} in {}", page.file);
        }
        assert!(
            !html.contains("rel=\"stylesheet\" href=\"http"),
            "{}",
            page.file
        );
    }
}

/// A token that no loader knows (`{nmae}`, a new `{source}` nobody filled) would be
/// published as it stands.
#[test]
fn no_page_is_left_with_an_unfilled_token() {
    for page in pages() {
        let html = &page.html;
        for (at, _) in html.match_indices('{') {
            let word: String = html[at + 1..]
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || *c == '_')
                .collect();
            let closed = html[at + 1 + word.len()..].starts_with('}');
            assert!(
                word.is_empty() || !closed,
                "unfilled token {{{word}}} in {}",
                page.file
            );
        }
    }
}

#[test]
fn pages_name_their_translations_and_the_not_found_page_stays_out_of_indexes() {
    for page in pages() {
        if page.file.ends_with("404.html") {
            assert!(page.html.contains("content=\"noindex\""), "{}", page.file);
            assert!(!page.html.contains("rel=\"canonical\""), "{}", page.file);
            continue;
        }
        assert!(page.html.contains("rel=\"canonical\""), "{}", page.file);
        assert!(page.html.contains("hreflang=\"en\""), "{}", page.file);
        assert!(page.html.contains("hreflang=\"de\""), "{}", page.file);
    }
}

#[test]
fn internal_links_point_at_pages_that_exist() {
    let all = pages();
    let site = site();
    let files: BTreeSet<String> = all.iter().map(|p| p.file.clone()).collect();
    // written by the generator or published next to the site (documentation; the deploy job
    // checks every /docs/ link against the Antora output)
    let known_prefixes = [
        "/assets/",
        "/brand/fonts/",
        "/licenses/",
        "/.well-known/security.txt",
        site.config.docs_url.as_str(),
    ];
    for page in &all {
        for href in hrefs(&page.html) {
            if !href.starts_with('/') {
                assert!(
                    href.starts_with("https://") || href.starts_with('#'),
                    "unexpected link {href} in {}",
                    page.file
                );
                continue;
            }
            if known_prefixes.iter().any(|p| href.starts_with(p)) {
                continue;
            }
            let file = if href.ends_with('/') {
                format!("{href}index.html")
            } else {
                href.clone()
            };
            assert!(
                files.contains(&file),
                "{href} in {} leads nowhere",
                page.file
            );
        }
    }
}

#[test]
fn the_locales_have_the_same_shape() {
    let site = site();
    let en = &site.locales[0];
    for other in &site.locales[1..] {
        let keys = |l: &Locale| l.strings.keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(en), keys(other), "strings.toml keys of {}", other.code);
        let kinds = |l: &Locale| {
            l.home
                .kinds
                .items
                .iter()
                .map(|k| (k.kind.clone(), k.status))
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds(en), kinds(other), "landing kinds of {}", other.code);
        let features = |l: &Locale| {
            feature_kinds(l)
                .map(|k| (k.kind.clone(), k.status, k.points.len()))
                .collect::<Vec<_>>()
        };
        assert_eq!(features(en), features(other), "{}", other.code);
        let points = |l: &Locale| {
            l.features
                .everything
                .points
                .iter()
                .map(|p| (p.id.clone(), p.status, p.icon.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(points(en), points(other), "features of {}", other.code);
        let shots = |l: &Locale| {
            let s = &l.home.screenshots;
            [&s.hero, &s.wide]
                .into_iter()
                .chain(&s.phones)
                .map(|shot| shot.view.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(shots(en), shots(other), "captures of {}", other.code);
        assert_eq!(en.home.facts.len(), other.home.facts.len());
        assert_eq!(
            en.home.together.points.len(),
            other.home.together.points.len()
        );
        assert_eq!(
            en.home.selfhost.steps.len(),
            other.home.selfhost.steps.len()
        );
        assert_eq!(
            en.features.later.items.len(),
            other.features.later.items.len()
        );
        for (a, b) in en.pages.iter().zip(&other.pages) {
            assert_eq!(a.slug, b.slug);
            assert_eq!(a.open_slots, b.open_slots, "open slots of {}", a.slug);
            assert_eq!(
                count(&a.body_html, "<h2"),
                count(&b.body_html, "<h2"),
                "sections of {}",
                a.slug
            );
        }
    }
}

/// Every claim has an id the release file can confirm, the landing page and the features
/// page agree, and each kind names where its data comes from.
#[test]
fn kinds_and_features_carry_a_status_and_an_id() {
    let site = site();
    for locale in &site.locales {
        let kinds: Vec<&str> = feature_kinds(locale).map(|k| k.kind.as_str()).collect();
        let unique: BTreeSet<&str> = kinds.iter().copied().collect();
        assert_eq!(kinds.len(), unique.len(), "a kind is listed twice");
        for kind in feature_kinds(locale) {
            assert!(!kind.source.is_empty(), "{} has no source", kind.kind);
        }
        for kind in &locale.home.kinds.items {
            let listed = feature_kinds(locale)
                .find(|k| k.kind == kind.kind)
                .unwrap_or_else(|| panic!("{} is not on the features page", kind.kind));
            assert_eq!(
                kind.status, listed.status,
                "{} differs between the landing page and the features page",
                kind.kind
            );
        }
        let mut ids = BTreeSet::new();
        for point in &locale.features.everything.points {
            assert!(
                !point.id.is_empty() && point.status.is_some(),
                "feature `{}` needs id and status",
                point.title
            );
            assert!(ids.insert(point.id.clone()), "id {} twice", point.id);
        }
    }
}

/// The release gate (review SITE-1): what the pages call shipped blocks publishing until
/// `release.toml` confirms it against a tag.
#[test]
fn shipped_claims_block_until_the_release_file_confirms_them() {
    let mut site = (*site()).clone();
    let shipped = site.shipped_ids();
    assert!(shipped.contains("movie"), "{shipped:?}");

    site.release = Release::default();
    let found = release_findings(&site);
    assert!(found[0].contains("`version` is empty"), "{found:?}");
    for id in &shipped {
        assert!(
            found.iter().any(|f| f.contains(&format!("`{id}`"))),
            "{id} is not blocked: {found:?}"
        );
    }

    // ids without a tag confirm nothing
    site.release.confirmed = shipped.iter().cloned().collect();
    assert_eq!(release_findings(&site).len(), 1 + shipped.len());

    site.release.version = "v1.0.0".into();
    assert_eq!(release_findings(&site), Vec::<String>::new());

    // one claim more than the release covers
    let dropped = site.release.confirmed.pop().unwrap();
    let found = release_findings(&site);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains(&dropped));

    // a confirmed id that no page names is a typing error
    site.release.confirmed.push(dropped);
    site.release.confirmed.push("bord_game".into());
    let found = release_findings(&site);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("bord_game"));
}

/// Until a release is confirmed, what is not on the main branch is shown as coming, and the
/// text that depends on a release is left out.
#[test]
fn before_the_first_release_unbuilt_things_are_marked_coming() {
    let site = site();
    if !site.release.version.is_empty() {
        return;
    }
    for locale in &site.locales {
        for kind in ["book", "board_game"] {
            let on_features = feature_kinds(locale).find(|k| k.kind == kind).unwrap();
            assert_eq!(on_features.status, Status::Coming, "{kind}");
        }
        let manual = locale
            .features
            .everything
            .points
            .iter()
            .find(|p| p.id == "manual_entry")
            .expect("manual entry is listed");
        assert_eq!(manual.status, Some(Status::Coming));
        let security = &locale.page("security").unwrap().body_html;
        assert!(!security.to_lowercase().contains("header"), "{security}");
        let credits = &locale.page("attributions").unwrap().body_html;
        assert!(!credits.contains("Open Library") && !credits.contains("Wikidata"));
    }
    assert!(site.conditional_ids.contains("security_headers"));
    let all = pages();
    // the landing page marks the two kinds and counts the kinds instead of claiming six
    let home = page(&all, "/index.html");
    assert_eq!(count(home, ">Coming<"), 2, "book and board game");
    assert!(home.contains(">4</dd>"), "four kinds are shipped");
}

/// A block that waits for a release appears once the release confirms its id.
#[test]
fn text_of_a_confirmed_release_appears() {
    let dir = std::env::temp_dir().join(format!("website-release-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&content_dir(), &dir);
    std::fs::write(
        dir.join("release.toml"),
        "version = \"v1.0.0\"\nconfirmed = [\"security_headers\"]\n",
    )
    .unwrap();
    let site = Site::load(&dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let security = &site.locales[0].page("security").unwrap().body_html;
    assert!(security.contains("security headers"), "{security}");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let (path, target) = (entry.path(), to.join(entry.file_name()));
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

fn now() -> DateTime<Utc> {
    std::time::SystemTime::now().into()
}

/// The content directory as it would be when everything is settled.
fn finished_site() -> Site {
    let mut site = (*site()).clone();
    site.config.security_contact = "mailto:security@example.org".into();
    site.config.security_expires = (now() + Duration::days(300)).to_rfc3339();
    site.config.source_url = "https://example.org/source".into();
    site.release = Release {
        version: "v1.0.0".into(),
        confirmed: site.shipped_ids().into_iter().collect(),
    };
    for locale in &mut site.locales {
        for page in &mut locale.pages {
            page.open_slots = 0;
        }
    }
    site
}

/// The strict gate (review SITE-4): a finished site passes, and each single thing left open
/// blocks it.
#[test]
fn the_gate_passes_a_finished_site_and_nothing_less() {
    type Defect = (&'static str, fn(&mut Site), &'static str);
    let site = finished_site();
    let playbook = finished_playbook(&site.config.name);
    let blockers = |site: &Site, slots: &[OpenSlot], playbook: Option<&str>| {
        publish_blockers(site, slots, playbook, now())
    };
    assert_eq!(
        blockers(&site, &[], Some(&playbook)),
        Vec::<String>::new(),
        "a finished site may be published"
    );

    let slot = OpenSlot {
        file: "en/imprint.md".into(),
        line: 12,
        text: format!("**[{PLACEHOLDER}: street]**"),
    };
    let found = blockers(&site, &[slot], Some(&playbook));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("en/imprint.md:12"));

    let defects: [Defect; 9] = [
        (
            "a page still has open slots",
            |s| s.locales[1].pages[0].open_slots = 2,
            "shows its notice",
        ),
        (
            "markers deleted, contact left empty",
            |s| s.config.security_contact.clear(),
            "security_contact",
        ),
        (
            "contact without a scheme",
            |s| s.config.security_contact = "security@example.org".into(),
            "mailto: or https:",
        ),
        (
            "expiry left empty",
            |s| s.config.security_expires.clear(),
            "security_expires",
        ),
        (
            "expiry without a time",
            |s| s.config.security_expires = "2027-11-01".into(),
            "RFC 3339",
        ),
        (
            "expiry in the past",
            |s| s.config.security_expires = "2024-01-01T00:00:00Z".into(),
            "in the past",
        ),
        (
            "no source address",
            |s| s.config.source_url.clear(),
            "source_url",
        ),
        (
            "no release confirmed",
            |s| s.release.version.clear(),
            "release.toml",
        ),
        (
            "a claim the release does not cover",
            |s| {
                s.release.confirmed.retain(|id| id != "movie");
            },
            "`movie`",
        ),
    ];
    for (what, break_it, expected) in defects {
        let mut broken = site.clone();
        break_it(&mut broken);
        let found = blockers(&broken, &[], Some(&playbook));
        assert!(
            found.iter().any(|f| f.contains(expected)),
            "{what}: {found:?}"
        );
    }

    let working_title = playbook.replace(&site.config.name, "Tracker");
    let found = blockers(&site, &[], Some(&working_title));
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("Tracker documentation"));
    assert_eq!(blockers(&site, &[], None).len(), 1);
}

/// The content as it is today is not finished, and the gate says why.
#[test]
fn the_gate_blocks_the_content_as_it_stands() {
    let site = site();
    let slots = open_slots(&content_dir()).unwrap();
    if slots.is_empty() && !site.release.version.is_empty() {
        return;
    }
    let playbook = finished_playbook(&site.config.name);
    let found = publish_blockers(&site, &slots, Some(&playbook), now());
    assert!(found.len() >= slots.len(), "{found:?}");
    for slot in &slots {
        let at = format!("{}:{}", slot.file, slot.line);
        assert!(found.iter().any(|f| f.contains(&at)), "{at}");
    }
}

/// The documentation is published below the website (review SITE-5): its diagrams must be
/// fetched when it is built, not by the reader's browser.
#[test]
fn the_documentation_does_not_load_its_diagrams_from_another_server() {
    let playbook = website::crate_dir().join("../../../../docs/antora-playbook.yml");
    let text = std::fs::read_to_string(playbook).expect("the Antora playbook");
    let found = docs_findings(Some(&text), "");
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn the_landing_page_tells_where_the_name_comes_from() {
    let all = pages();
    for file in ["/index.html", "/de/index.html"] {
        let html = page(&all, file);
        assert!(html.contains("id=\"name\""), "{file}");
        assert!(html.contains("2270"), "the line of the poem: {file}");
        assert!(html.contains("Beowulf"), "{file}");
        // the two parts, the gloss, three words in the text, one in the sources
        assert!(
            count(html, "lang=\"ang\"") >= 7,
            "the Old English words are marked: {file}"
        );
        assert!(
            !html.contains("oldenglishwordhord"),
            "only confirmed sources: {file}"
        );
    }
    let de = page(&all, "/de/index.html");
    assert!(
        de.contains("hreflang=\"en\" lang=\"en\">Brunetti"),
        "English source titles are marked on the German page"
    );
}

/// The captures of the application: real pictures, each with a text alternative and its
/// size, in the page's language and in both themes.
#[test]
fn the_landing_page_shows_captures_of_the_application() {
    let site = site();
    let all = pages();
    for (file, lang) in [("/index.html", "en"), ("/de/index.html", "de")] {
        let html = page(&all, file);
        assert!(!html.contains("data-slot=\"screenshot\""), "{file}");
        let images: Vec<&str> = html
            .split("<img ")
            .skip(1)
            .filter_map(|rest| rest.split('>').next())
            .collect();
        // hero, the wide picture and four phones, each light and dark
        assert_eq!(images.len(), 12, "{file}");
        for img in &images {
            let src = img
                .split("src=\"")
                .nth(1)
                .and_then(|r| r.split('"').next())
                .unwrap();
            let name = src.strip_prefix("/assets/shots/").expect(src);
            assert!(name.contains(&format!("-{lang}-")), "{name} on {file}");
            let pixels = site.shots.get(name).unwrap_or_else(|| panic!("{name}"));
            assert!(
                img.contains(&format!("width=\"{}\"", pixels.width)),
                "{img}"
            );
            assert!(
                img.contains(&format!("height=\"{}\"", pixels.height)),
                "{img}"
            );
            let alt = img
                .split("alt=\"")
                .nth(1)
                .and_then(|r| r.split('"').next())
                .unwrap_or("");
            assert!(alt.len() > 60, "a real text alternative: {img}");
        }
        assert_eq!(count(html, "-light.png\""), 6 + 1, "six and og:image");
        assert_eq!(count(html, "-dark.png\""), 6);
        assert_eq!(count(html, "loading=\"eager\""), 2, "the hero");
        assert!(html.contains(&format!(
            "<meta property=\"og:image\" content=\"{}/assets/shots/home-top-{lang}-light.png\">",
            site.config.base_url
        )));
    }
    // every capture exists in both languages and themes and is a real picture
    assert_eq!(site.shots.len() % 4, 0, "{:?}", site.shots.keys());
    for (name, size) in &site.shots {
        assert!(size.width >= 780 && size.height >= 800, "{name}: {size:?}");
    }
    assert!(content_dir().join("shots/README.md").is_file());
}

/// No row of the features page is left half empty: the kinds are one grid of an even number
/// of cards, each a section of its own that names its group.
#[test]
fn the_features_page_is_one_grid_of_kind_cards() {
    let all = pages();
    for file in ["/features/index.html", "/de/features/index.html"] {
        let html = page(&all, file);
        let cards = count(html, "data-kind=\"");
        assert_eq!(cards, 6, "{file}");
        assert_eq!(cards % 2, 0);
        assert!(count(html, "<h2") >= cards + 2, "{file}");
    }
}

#[test]
fn icons_named_in_the_content_are_in_the_allowlist() {
    let site = site();
    for locale in &site.locales {
        let points = locale
            .home
            .together
            .points
            .iter()
            .chain(&locale.features.everything.points);
        for point in points {
            assert!(
                ui::IconName::from_name(&point.icon).is_some(),
                "icon `{}` ({})",
                point.icon,
                locale.code
            );
        }
        for kind in &locale.home.kinds.items {
            let name = format!("kind-{}", kind.kind.replace('_', "-"));
            assert!(ui::IconName::from_name(&name).is_some(), "{name}");
        }
    }
}

#[test]
fn the_product_name_is_written_down_in_site_toml_only() {
    let name = site().config.name.clone();
    let web = website::crate_dir().join("../../../web");
    // code never names the product, in any spelling
    let mut code = Vec::new();
    files_below(&website::crate_dir().join("src"), &mut code);
    files_below(&website::crate_dir().join("tests"), &mut code);
    files_below(&web.join("site-tests"), &mut code);
    files_below(&web.join("site"), &mut code);
    code.push(web.join("style/site.css"));
    code.push(web.join("playwright.site.config.ts"));
    assert!(
        code.len() > 12,
        "the scan found the sources: {}",
        code.len()
    );
    for path in code.iter().filter(|p| is_text(p)) {
        let text = std::fs::read_to_string(path).unwrap().to_lowercase();
        assert!(
            !text.contains(&name.to_lowercase()),
            "{} names the product",
            path.display()
        );
    }
    // content says {name}; the Old English word and the address of its dictionary entry are
    // lower case and are not the name
    let mut content = Vec::new();
    files_below(&content_dir(), &mut content);
    assert!(content.len() > 20, "{}", content.len());
    for path in content.iter().filter(|p| is_text(p)) {
        if path.ends_with("content/site.toml") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        assert!(
            !text.contains(&name),
            "{} names the product",
            path.display()
        );
    }
}

#[test]
fn english_content_and_code_are_ascii() {
    let mut files = Vec::new();
    files_below(&content_dir().join("en"), &mut files);
    files_below(&content_dir().join("shots"), &mut files);
    files_below(&website::crate_dir().join("src"), &mut files);
    files.push(content_dir().join("site.toml"));
    files.push(content_dir().join("release.toml"));
    for path in files.iter().filter(|p| is_text(p)) {
        let text = std::fs::read_to_string(path).unwrap();
        let bad: Vec<char> = text.chars().filter(|c| !c.is_ascii()).collect();
        assert!(bad.is_empty(), "{}: {bad:?}", path.display());
    }
}

#[test]
fn owner_slots_are_marked_and_found() {
    let slots = open_slots(&content_dir()).unwrap();
    let in_file = |f: &str| slots.iter().filter(|s| s.file == f).count();
    // no address, name or contact is invented: the imprint is slots only
    assert!(in_file("en/imprint.md") >= 6, "{slots:?}");
    assert_eq!(in_file("en/imprint.md"), in_file("de/imprint.md"));
    assert!(in_file("en/privacy.md") >= 1);
    // the disclosure policy and the scope are the owner's to decide (review SITE-9)
    assert_eq!(in_file("en/security.md"), 2, "{slots:?}");
    assert_eq!(in_file("de/security.md"), 2);
    assert_eq!(
        in_file("site.toml"),
        3,
        "source address, security contact and expiry"
    );
    let site = site();
    for locale in &site.locales {
        for slug in ["imprint", "privacy", "security"] {
            let page = locale.page(slug).unwrap();
            assert!(page.open_slots > 0, "{slug}");
            assert!(
                !page.body_html.contains('@'),
                "{slug} carries no invented address"
            );
        }
        let security = &locale.page("security").unwrap().body_html;
        for promise in [
            "We confirm",
            "bug bounty",
            "Wir bestätigen",
            "Bug-Bounty-Programm",
        ] {
            assert!(
                !security.contains(promise) || security.contains(PLACEHOLDER),
                "{promise}"
            );
        }
        assert!(!security.contains("There is no bug bounty"));
    }
}

/// The notice is derived from the open slots of the page (review SITE-4): it cannot be
/// forgotten on a filled page or be missing on an open one.
#[test]
fn a_page_shows_its_notice_exactly_while_it_has_open_slots() {
    let mut site = (*site()).clone();
    let title = site.locales[0].t("notice-title");
    let shows = |site: &Site, file: &str| {
        let all = render(&Arc::new(site.clone()));
        page(&all, file).contains(&title)
    };
    assert!(shows(&site, "/imprint/index.html"));
    assert!(shows(&site, "/security/index.html"));
    assert!(
        shows(&site, "/self-hosting/index.html"),
        "the repository address is open"
    );
    assert!(!shows(&site, "/changelog/index.html"));
    assert!(!shows(&site, "/hub/index.html"));
    // filled: the notice of the page head is not shown any more
    for page in &mut site.locales[0].pages {
        page.open_slots = 0;
    }
    assert!(!shows(&site, "/imprint/index.html"));
    assert!(
        site.locales[0]
            .page("imprint")
            .unwrap()
            .meta
            .notice
            .is_some()
    );
}

/// The contact of the security page is the one of `security.txt`.
#[test]
fn the_security_page_shows_the_contact_of_site_toml() {
    let dir = std::env::temp_dir().join(format!("website-contact-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(&content_dir(), &dir);
    let toml = std::fs::read_to_string(dir.join("site.toml")).unwrap();
    let toml = toml.replace(
        "security_contact = \"\"",
        "security_contact = \"mailto:security@example.org\"",
    );
    std::fs::write(dir.join("site.toml"), toml).unwrap();
    let site = Site::load(&dir).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let security = site.locales[0].page("security").unwrap();
    assert!(
        security
            .body_html
            .contains("<a href=\"mailto:security@example.org\">security@example.org</a>"),
        "{}",
        security.body_html
    );
    assert_eq!(security.open_slots, 2, "the policy slots stay open");
}

#[test]
fn security_txt_without_a_contact_is_a_marked_placeholder() {
    let site = site();
    let mut config = site.config.clone();
    config.security_contact.clear();
    config.security_expires.clear();
    let text = security_txt(&config);
    assert!(text.contains(PLACEHOLDER), "{text}");
    assert!(
        text.lines().all(|l| l.starts_with('#')),
        "comments only, no field: {text}"
    );
    config.security_contact = "mailto:security@example.org".into();
    assert!(
        security_txt(&config).contains(PLACEHOLDER),
        "an expiry date is required too"
    );
    config.security_expires = "2027-11-01T00:00:00Z".into();
    let text = security_txt(&config);
    assert!(
        text.starts_with("Contact: mailto:security@example.org\nExpires: 2027-11-01T00:00:00Z\n")
    );
    assert!(text.contains(&format!(
        "Canonical: {}/.well-known/security.txt",
        config.base_url
    )));
    assert!(text.contains("Preferred-Languages: en, de"));
    assert!(!text.contains(PLACEHOLDER));
}

#[test]
fn the_site_map_lists_every_page_in_every_locale() {
    let site = site();
    let map = sitemap(&site.config);
    let base = &site.config.base_url;
    for path in page_paths() {
        assert!(map.contains(&format!("<loc>{base}{path}</loc>")), "{path}");
        assert!(
            map.contains(&format!("<loc>{base}/de{path}</loc>")),
            "de {path}"
        );
    }
    assert!(!map.contains("404"));
    assert_eq!(count(&map, "<url>"), 2 * page_paths().len());
}

#[test]
fn the_current_page_is_marked_in_the_navigation() {
    let all = pages();
    let features = page(&all, "/features/index.html");
    // top bar and drawer
    assert_eq!(count(features, "aria-current=\"page\""), 2, "{features}");
    let home = page(&all, "/index.html");
    assert_eq!(count(home, "aria-current=\"page\""), 0);
    // the language link leads to the same page in the other locale
    assert!(features.contains("href=\"/de/features/\" lang=\"de\" hreflang=\"de\""));
    let de = page(&all, "/de/features/index.html");
    assert!(de.contains("href=\"/features/\" lang=\"en\" hreflang=\"en\""));
}

/// The documentation is English only (review SITE-10): links to it from a German page say
/// so to assistive technology, links from an English page need no mark.
#[test]
fn links_to_the_english_documentation_are_marked_on_german_pages() {
    let site = site();
    let all = pages();
    let (docs, install) = (&site.config.docs_url, &site.config.docs_install_url);
    let mut seen = 0;
    for rendered in &all {
        let german = rendered.file.starts_with("/de/");
        let anchors = rendered
            .html
            .split("<a ")
            .skip(1)
            .filter_map(|rest| rest.split('>').next());
        for anchor in anchors {
            let to_docs = [docs, install]
                .iter()
                .any(|target| anchor.contains(&format!("href=\"{target}\"")));
            if to_docs {
                seen += 1;
                assert_eq!(
                    anchor.contains("hreflang=\"en\""),
                    german,
                    "<a {anchor}> in {}",
                    rendered.file
                );
            }
        }
    }
    assert!(
        seen > 60,
        "top bar, drawer and footer of every page: {seen}"
    );
}

/// GitLab Pages serves only `/404.html`, whatever the language of the wrong address.
#[test]
fn the_not_found_page_has_a_line_in_the_other_language() {
    let all = pages();
    let en = page(&all, "/404.html");
    assert!(en.contains("<p lang=\"de\">") && en.contains("href=\"/de/\" hreflang=\"de\""));
    let de = page(&all, "/de/404.html");
    assert!(de.contains("<p lang=\"en\">") && de.contains("href=\"/\" hreflang=\"en\""));
}

/// `generate::build`: the files on disk, the returned count, the fingerprinted stylesheet,
/// and a clean output directory on every run (review SITE-12, SITE-15).
#[test]
fn build_writes_the_site_and_clears_what_an_earlier_build_left() {
    let tmp = std::env::temp_dir().join(format!("website-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let css = tmp.join("site.css");
    std::fs::write(&css, "body{margin:0}").unwrap();
    let options = website::default_options(css.clone(), tmp.join("out"));

    let written = website::generate::build(&options).unwrap();
    let on_disk = |dir: &Path| {
        let mut files = Vec::new();
        files_below(dir, &mut files);
        files
    };
    let files = on_disk(&options.out);
    assert_eq!(written, files.len(), "the count is the number of files");
    let stylesheets = |files: &[PathBuf]| -> Vec<String> {
        files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "css"))
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    };
    let first = stylesheets(&files);
    assert_eq!(first.len(), 1, "{first:?}");
    let index = std::fs::read_to_string(options.out.join("index.html")).unwrap();
    assert!(index.contains(&format!("href=\"/assets/{}\"", first[0])));
    for file in [
        "de/index.html",
        "404.html",
        "robots.txt",
        "sitemap.xml",
        ".well-known/security.txt",
        "assets/theme.js",
        "assets/shots/home-top-en-light.png",
        "assets/shots/log-de-dark.png",
        "licenses/lucide.txt",
    ] {
        assert!(options.out.join(file).is_file(), "{file}");
    }

    // a changed stylesheet gets a new name, and neither the old one nor a page that no
    // longer exists stays behind
    std::fs::write(&css, "body{margin:1px}").unwrap();
    std::fs::create_dir_all(options.out.join("gone")).unwrap();
    std::fs::write(options.out.join("gone/index.html"), "old").unwrap();
    let again = website::generate::build(&options).unwrap();
    let files = on_disk(&options.out);
    assert_eq!(again, written);
    assert_eq!(files.len(), written);
    let second = stylesheets(&files);
    assert_eq!(second.len(), 1, "{second:?}");
    assert_ne!(first, second);
    assert!(!options.out.join("gone").exists());

    // a directory that is not a build of the website is never cleared
    let foreign = tmp.join("foreign");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("notes.txt"), "keep").unwrap();
    let mut wrong = options.clone();
    wrong.out = foreign.clone();
    let error = website::generate::build(&wrong).unwrap_err();
    assert!(format!("{error:#}").contains("refusing"), "{error:#}");
    assert!(foreign.join("notes.txt").is_file());
    std::fs::remove_dir_all(&tmp).unwrap();
}
