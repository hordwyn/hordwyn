//! The publishing gate: everything that must be settled before the site may go public.
//!
//! `website build --strict` and `website check --strict` fail while [`publish_blockers`]
//! returns anything; the deploy job publishes only a build whose list is empty. A blocker is
//! one of: an open owner slot, a page that still shows its notice, an invalid `security.txt`
//! (RFC 9116 needs `Contact` and `Expires`), no public source address, a claim marked
//! `shipped` that `release.toml` does not confirm, or published documentation that still
//! carries a working title or loads its diagrams from another server.

use chrono::{DateTime, Duration, Utc};

use crate::content::{OpenSlot, Site, SiteConfig};

/// RFC 9116 section 2.5.5: the expiry date should be less than a year ahead.
const MAX_EXPIRY_DAYS: i64 = 366;

/// What is wrong with the two values `security.txt` needs.
#[must_use]
pub fn security_txt_findings(config: &SiteConfig, now: DateTime<Utc>) -> Vec<String> {
    let mut out = Vec::new();
    let contact = &config.security_contact;
    if contact.is_empty() {
        out.push("site.toml: `security_contact` is empty (security.txt needs Contact)".into());
    } else if !(contact.starts_with("mailto:") && contact.len() > 7 && contact.contains('@')
        || contact.starts_with("https://") && contact.len() > 8)
    {
        out.push(format!(
            "site.toml: `security_contact` must be a mailto: or https: URI, got `{contact}`"
        ));
    }
    let expires = &config.security_expires;
    if expires.is_empty() {
        out.push("site.toml: `security_expires` is empty (security.txt needs Expires)".into());
        return out;
    }
    match DateTime::parse_from_rfc3339(expires) {
        Err(_) => out.push(format!(
            "site.toml: `security_expires` is not an RFC 3339 date and time \
             (like 2027-11-01T00:00:00Z), got `{expires}`"
        )),
        Ok(date) if date <= now => out.push(format!(
            "site.toml: `security_expires` ({expires}) is in the past"
        )),
        Ok(date) if date > now + Duration::days(MAX_EXPIRY_DAYS) => out.push(format!(
            "site.toml: `security_expires` ({expires}) is more than a year ahead"
        )),
        Ok(_) => {}
    }
    out
}

/// Claims of the pages that `release.toml` does not cover, and entries of it that name
/// nothing.
#[must_use]
pub fn release_findings(site: &Site) -> Vec<String> {
    let mut out = Vec::new();
    let release = &site.release;
    if release.version.is_empty() {
        out.push(
            "release.toml: `version` is empty - no release is confirmed, so nothing may be \
             published as shipped"
                .into(),
        );
    }
    for id in site.shipped_ids() {
        if !release.confirms(&id) {
            out.push(format!(
                "`{id}` is marked shipped but release.toml does not confirm it"
            ));
        }
    }
    let known = site.known_ids();
    for id in &release.confirmed {
        if !known.contains(id) {
            out.push(format!("release.toml confirms `{id}`, which no page names"));
        }
    }
    out
}

/// Value of the first `key:` line of a YAML text (enough for the two scalars read here).
fn yaml_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let value = line.trim().strip_prefix(key)?.strip_prefix(':')?;
        Some(value.trim().trim_matches(['"', '\'']))
    })
}

/// What is wrong with the documentation that is published below the website, read from the
/// Antora playbook: its title must carry the product name (not a working title), and the
/// diagrams must be fetched when the documentation is built (`kroki-fetch-diagram: true`),
/// or every reader's browser would load them from the diagram service.
#[must_use]
pub fn docs_findings(playbook: Option<&str>, name: &str) -> Vec<String> {
    let Some(text) = playbook else {
        return vec!["the Antora playbook of the documentation was not found".into()];
    };
    let mut out = Vec::new();
    let title = yaml_value(text, "title").unwrap_or("");
    if !title.contains(name) {
        out.push(format!(
            "the documentation is titled `{title}`: its title must carry the product name"
        ));
    }
    if yaml_value(text, "kroki-fetch-diagram") != Some("true") {
        out.push(
            "the documentation loads its diagrams from another server in the reader's browser \
             (`kroki-fetch-diagram` is not true in the Antora playbook)"
                .into(),
        );
    }
    out
}

/// Everything that blocks publishing, one line each; empty when the site may go public.
/// `slots` are the marked lines of the content files ([`crate::content::open_slots`]),
/// `playbook` is the text of the documentation's Antora playbook.
#[must_use]
pub fn publish_blockers(
    site: &Site,
    slots: &[OpenSlot],
    playbook: Option<&str>,
    now: DateTime<Utc>,
) -> Vec<String> {
    let mut out: Vec<String> = slots
        .iter()
        .map(|s| format!("open slot {}:{}: {}", s.file, s.line, s.text))
        .collect();
    for locale in &site.locales {
        for page in locale.pages.iter().filter(|p| p.open_slots > 0) {
            out.push(format!(
                "the page `{}` ({}) has {} open slot(s) and shows its notice",
                page.slug, locale.code, page.open_slots
            ));
        }
    }
    out.extend(security_txt_findings(&site.config, now));
    let source = &site.config.source_url;
    if source.is_empty() {
        out.push(
            "site.toml: `source_url` is empty (the site says open source and must link the code)"
                .into(),
        );
    } else if !source.starts_with("https://") {
        out.push(format!(
            "site.toml: `source_url` must be an https: address, got `{source}`"
        ));
    }
    out.extend(release_findings(site));
    out.extend(docs_findings(playbook, &site.config.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(contact: &str, expires: &str) -> SiteConfig {
        toml::from_str(&format!(
            "name = \"N\"\nbase_url = \"https://e.org\"\nlocales = [\"en\"]\ndocs_url = \"/docs/\"\n\
             docs_install_url = \"/docs/i.html\"\nhub_url = \"https://h.e.org\"\n\
             security_contact = \"{contact}\"\nsecurity_expires = \"{expires}\"\n"
        ))
        .unwrap()
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-11-13T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn a_valid_contact_and_expiry_pass() {
        for contact in ["mailto:security@example.org", "https://example.org/report"] {
            let found = security_txt_findings(&config(contact, "2027-11-01T00:00:00Z"), now());
            assert_eq!(found, Vec::<String>::new());
        }
    }

    #[test]
    fn security_txt_needs_both_values() {
        let found = security_txt_findings(&config("", ""), now());
        assert_eq!(found.len(), 2, "{found:?}");
        // the hole of the first version: markers deleted, one value left empty
        let found = security_txt_findings(&config("mailto:security@example.org", ""), now());
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("security_expires"));
    }

    #[test]
    fn the_contact_must_be_a_mailto_or_https_uri() {
        for bad in [
            "security@example.org",
            "http://example.org/report",
            "mailto:",
            "tel:+1",
        ] {
            let found = security_txt_findings(&config(bad, "2027-11-01T00:00:00Z"), now());
            assert_eq!(found.len(), 1, "{bad}: {found:?}");
            assert!(found[0].contains("security_contact"), "{bad}");
        }
    }

    #[test]
    fn the_expiry_must_be_rfc_3339_in_the_future_and_within_a_year() {
        let cases = [
            ("2027-11-01", "RFC 3339"),
            ("next year", "RFC 3339"),
            ("2026-11-13T11:59:59Z", "in the past"),
            ("2025-01-01T00:00:00Z", "in the past"),
            ("2027-11-15T00:00:00Z", "more than a year"),
            ("2031-01-01T00:00:00+01:00", "more than a year"),
        ];
        for (expires, why) in cases {
            let found = security_txt_findings(&config("mailto:s@example.org", expires), now());
            assert_eq!(found.len(), 1, "{expires}: {found:?}");
            assert!(found[0].contains(why), "{expires}: {found:?}");
        }
        let fine = config("mailto:s@example.org", "2027-11-13T12:00:00+01:00");
        assert_eq!(security_txt_findings(&fine, now()), Vec::<String>::new());
    }

    #[test]
    fn the_documentation_needs_the_product_name_and_fetched_diagrams() {
        let good = "site:\n  title: N documentation\nasciidoc:\n  attributes:\n    kroki-fetch-diagram: true\n";
        assert_eq!(docs_findings(Some(good), "N"), Vec::<String>::new());
        let working = good.replace("N documentation", "Tracker documentation");
        let found = docs_findings(Some(&working), "Name");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("Tracker documentation"));
        let remote = good.replace("diagram: true", "diagram: false");
        let found = docs_findings(Some(&remote), "N");
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("kroki-fetch-diagram"));
        let unset = "site:\n  title: N documentation\n";
        assert_eq!(docs_findings(Some(unset), "N").len(), 1);
        assert_eq!(docs_findings(None, "N").len(), 1);
    }
}
