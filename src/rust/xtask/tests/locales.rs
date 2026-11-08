//! i18n checks on the real sources (owner ruling DEC-171): every catalogue (web, notify) has
//! the same keys and variables in every locale and no empty values; every key the web app
//! references exists and every key is used; no hard-coded text in `view!` markup of the web
//! app (the dev gallery excepted, DEC-199) or the label-free `ui` crate; brand overrides only
//! name existing keys.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use xtask::locales::{hard_coded, messages, parity, references, rust_files};

const LOCALES: [&str; 2] = ["en", "de"];

fn crates() -> std::path::PathBuf {
    xtask::workspace_root().join("crates")
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn catalogue(krate: &str, locale: &str) -> String {
    read(
        &crates()
            .join(krate)
            .join("locales")
            .join(format!("{locale}.ftl")),
    )
}

fn en_keys(krate: &str) -> BTreeSet<String> {
    messages(&catalogue(krate, "en"))
        .expect("en.ftl parses")
        .into_keys()
        .collect()
}

#[test]
fn every_catalogue_has_the_same_keys_and_variables_in_every_locale() {
    let mut problems = Vec::new();
    for krate in ["web", "notify"] {
        let sources: Vec<(&str, String)> =
            LOCALES.iter().map(|l| (*l, catalogue(krate, l))).collect();
        let refs: Vec<(&str, &str)> = sources.iter().map(|(l, s)| (*l, s.as_str())).collect();
        problems.extend(parity(krate, &refs));
    }
    assert_eq!(problems, Vec::<String>::new());
}

/// Keys the web app builds at run time from a fixed part and a value (`status-{status}`,
/// `kind-{kind}`): they count as used. Each family is listed here on purpose, so a new
/// dynamic family is a visible decision.
const WEB_FAMILIES: &[&str] = &[
    "auth-error-",
    "kind-",
    "status-",
    "sharing-error-",
    "notify-error-",
    "notify-group-",
    "notify-kind-",
    "notify-state-",
    "notify-event-",
    "notify-section-",
    "month-short-",
    // group names on Home, Stats and History (`KindGroupDto.label_key` is `kind-group-<id>`)
    "kind-group-",
    // next-step button labels, named by the server (`NextStepDto.label_key`)
    "next-step-",
    // facet filter labels, named by the kind descriptor (`KindDto.facets`)
    "library-facet-",
    // labels of the brand's legal links (`[legal]` keys)
    "legal-",
    // UX phase 2: words for codes the server names (contracts::admin); the admin pages
    // build them from the DTO fields
    "admin-error-",
    "admin-warning-",
    "admin-setting-",
    "source-state-",
    "source-test-",
    "admin-source-field-",
    "admin-health-",
    "setup-checklist-",
    "admin-audit-",
    // phase 3 (WP3-4): sign-in errors by code, re-auth round trip errors, probe outcomes,
    // mail connection security
    "signin-error-",
    "signin-reset-error-",
    "reauth-error-",
    "admin-sso-probe-",
    "admin-mail-security-",
    // WP3-7 imports page: format cards, import states, mapping targets and lost-entry codes
    // the importers name (contracts::imports)
    "imports-format-",
    "imports-state-",
    "imports-target-",
    "imports-lost-",
];

#[test]
fn web_keys_in_code_exist_and_every_key_is_used() {
    let keys = en_keys("web");
    let prefixes: BTreeSet<String> = keys
        .iter()
        .filter_map(|k| k.split('-').next().map(String::from))
        .collect();
    let mut used = BTreeSet::new();
    let mut families = BTreeSet::new();
    for f in rust_files(&crates().join("web/src")) {
        let (k, fam) = references(&read(&f), &prefixes);
        used.extend(k);
        families.extend(fam);
    }
    let missing: Vec<_> = used.iter().filter(|k| !keys.contains(*k)).collect();
    assert_eq!(
        missing,
        Vec::<&String>::new(),
        "keys used in code but not in en.ftl"
    );
    let unknown: Vec<_> = families
        .iter()
        .filter(|f| !WEB_FAMILIES.contains(&f.as_str()))
        .collect();
    assert_eq!(
        unknown,
        Vec::<&String>::new(),
        "new dynamic key families: list them"
    );
    let unused: Vec<_> = keys
        .iter()
        .filter(|k| !used.contains(*k) && !WEB_FAMILIES.iter().any(|f| k.starts_with(f)))
        .collect();
    assert_eq!(
        unused,
        Vec::<&String>::new(),
        "keys in en.ftl that no code uses"
    );
}

#[test]
fn every_notification_event_has_a_label_in_the_web_app() {
    let web = en_keys("web");
    let missing: Vec<_> = en_keys("notify")
        .into_iter()
        .filter_map(|k| k.strip_suffix("-title").map(String::from))
        .filter(|id| id.matches('-').count() == 1 && id != "digest")
        .filter(|id| !web.contains(&format!("notify-event-{id}")))
        .collect();
    assert_eq!(missing, Vec::<String>::new());
}

#[test]
fn no_hard_coded_text_in_views() {
    let mut found = Vec::new();
    for dir in ["web/src", "ui/src"] {
        for f in rust_files(&crates().join(dir)) {
            // the dev-only component gallery stays English (DEC-199)
            if f.ends_with("web/src/gallery.rs") {
                continue;
            }
            for (line, text) in hard_coded(&read(&f)) {
                found.push(format!("{}:{line}: {text:?}", f.display()));
            }
        }
    }
    assert_eq!(
        found,
        Vec::<String>::new(),
        "move these texts to the .ftl files"
    );
}

#[test]
fn brand_overrides_name_existing_keys() {
    let mut known = en_keys("web");
    known.extend(en_keys("notify"));
    let repo = xtask::repo_root();
    let mut unknown = Vec::new();
    for brand in [
        repo.join("config/brand/default"),
        repo.join("src/web/tests/brands/sample"),
    ] {
        let text = read(&brand.join("brand.toml"));
        let mut in_strings = false;
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                in_strings = line.starts_with("[strings.");
            } else if in_strings && let Some((key, _)) = line.split_once('=') {
                let key = key.trim().trim_matches('"');
                if !key.is_empty() && !key.starts_with('#') && !known.contains(key) {
                    unknown.push(format!("{}: {key}", brand.display()));
                }
            }
        }
    }
    assert_eq!(unknown, Vec::<String>::new());
}
