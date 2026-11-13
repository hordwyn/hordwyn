//! `cargo xtask record <connector>`: re-record a connector's HTTP fixtures from the live API
//! (KICKOFF section 5). Uses `curl` so xtask needs no HTTP client.
//!
//! A connector with a key needs it in the environment. Open Library has none: it asks
//! clients to name a contact instead, so `SOURCE_OPERATOR_CONTACT` must be set (it goes into the
//! `User-Agent` of the recording requests, nowhere else), the requests are sent one per 1.2
//! seconds (its limit without an agreement is 1 per second), synthetic fixtures are left
//! alone, and an answer is trimmed to the top-level fields its fixture already holds.

use std::process::Command;

use anyhow::{Context, Result, bail};

/// How one connector is recorded.
struct Recording {
    connector: &'static str,
    /// Variable that holds the key (or, without `header`, the operator contact).
    env: &'static str,
    /// Header template; `{}` is replaced by the key. `None`: no key, the variable is the
    /// contact of the `User-Agent`.
    header: Option<&'static str>,
    /// Pause between two requests.
    pause_ms: u64,
    /// Keep only the top-level fields of the body that the fixture already has.
    trim: bool,
}

const RECORDINGS: &[Recording] = &[
    Recording {
        connector: "tmdb",
        env: "SOURCE_TMDB_API_KEY",
        header: Some("Authorization: Bearer {}"),
        pause_ms: 0,
        trim: false,
    },
    Recording {
        connector: "openlibrary",
        env: "SOURCE_OPERATOR_CONTACT",
        header: None,
        pause_ms: 1200,
        trim: true,
    },
];

/// `fresh` with only the top-level fields `old` has (objects only; anything else is
/// `fresh`), and a list no longer than the one `old` holds: a fixture whose lists were cut
/// when it was made ("covers cut to the first 6") stays cut (review BOOK-17).
#[must_use]
pub fn trimmed(old: &serde_json::Value, fresh: serde_json::Value) -> serde_json::Value {
    match (old.as_object(), fresh) {
        (Some(keep), serde_json::Value::Object(new)) if !keep.is_empty() => {
            let cut = |(k, mut v): (String, serde_json::Value)| {
                let was = keep.get(&k)?;
                if let (Some(was), Some(now)) = (was.as_array(), v.as_array_mut()) {
                    now.truncate(was.len());
                }
                Some((k, v))
            };
            serde_json::Value::Object(new.into_iter().filter_map(cut).collect())
        }
        (_, fresh) => fresh,
    }
}

/// May `contact` stand in the `User-Agent` of the recording requests: printable ASCII
/// without a space and without parentheses (the test of `catalog_core::check_contact` for
/// what could break out of the header comment; xtask does not link the server crates).
#[must_use]
pub fn contact_fits_a_header(contact: &str) -> bool {
    !contact.is_empty()
        && contact.len() <= 200
        && contact
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '(' && c != ')')
}

/// Is the fixture marked as synthetic (`fixture.synthetic`, or a `fixture` text that starts
/// with the word)?
#[must_use]
pub fn is_synthetic(fixture: &serde_json::Value) -> bool {
    fixture["fixture"]["synthetic"].as_bool().unwrap_or(false)
        || fixture["fixture"]
            .as_str()
            .is_some_and(|s| s.starts_with("synthetic"))
}

/// Re-record every fixture of `connector`.
///
/// # Errors
/// Unknown connector, missing key or contact, or a failed request.
pub fn record(connector: &str) -> Result<()> {
    let Some(r) = RECORDINGS.iter().find(|r| r.connector == connector) else {
        bail!("no recording support for `{connector}`");
    };
    let secret = std::env::var(r.env).with_context(|| format!("{} is not set", r.env))?;
    if r.header.is_none() && !contact_fits_a_header(&secret) {
        bail!(
            "{} is no contact for a header: printable ASCII without spaces or parentheses",
            r.env
        );
    }
    let dir = crate::workspace_root()
        .join("crates/connectors/fixtures")
        .join(connector);
    let mut paths: Vec<_> = std::fs::read_dir(&dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    let date = Command::new("date").arg("+%F").output()?;
    let today = String::from_utf8_lossy(&date.stdout).trim().to_string();
    let extra = match r.header {
        Some(template) => vec!["-H".to_string(), template.replace("{}", &secret)],
        None => vec![
            "-L".to_string(),
            "-A".to_string(),
            format!("tracker-fixtures/{} ({secret})", env!("CARGO_PKG_VERSION")),
        ],
    };
    for path in paths {
        let mut f: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path)?)?;
        if r.trim && is_synthetic(&f) {
            println!("kept (synthetic) {}", path.display());
            continue;
        }
        let url = f["request"]["url"]
            .as_str()
            .context("request.url")?
            .to_string();
        let out = Command::new("curl")
            .args([
                "-sS",
                "-w",
                "\n%{http_code}",
                "-H",
                "Accept: application/json",
            ])
            .args(&extra)
            .arg(&url)
            .output()?;
        let text = String::from_utf8(out.stdout)?;
        let (body, status) = text.rsplit_once('\n').context("curl output")?;
        f["status"] = status.trim().parse::<u16>()?.into();
        let body: serde_json::Value = serde_json::from_str(body)?;
        f["body"] = if r.trim {
            trimmed(&f["body"], body)
        } else {
            body
        };
        f["fixture"]["synthetic"] = false.into();
        f["fixture"]["source"] = format!("recorded from {url}").into();
        f["fixture"]["checked"] = today.clone().into();
        std::fs::write(&path, serde_json::to_string_pretty(&f)? + "\n")?;
        println!("recorded {}", path.display());
        std::thread::sleep(std::time::Duration::from_millis(r.pause_ms));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_answer_is_trimmed_to_the_fields_its_fixture_holds() {
        let old = json!({"key": "/works/OL1W", "title": "Old", "covers": [1]});
        let fresh = json!({"key": "/works/OL1W", "title": "New", "covers": [1, 2], "links": [], "revision": 9});
        assert_eq!(
            trimmed(&old, fresh),
            json!({"key": "/works/OL1W", "title": "New", "covers": [1]}),
            "the list stays as short as the fixture cut it"
        );
        // nothing to go by: the answer as it is
        assert_eq!(trimmed(&json!({}), json!({"a": 1})), json!({"a": 1}));
        assert_eq!(trimmed(&json!(null), json!([1])), json!([1]));
    }

    #[test]
    fn a_contact_that_could_leave_the_header_comment_is_refused() {
        assert!(contact_fits_a_header("ops@example.org"));
        assert!(contact_fits_a_header("https://example.org/contact"));
        for bad in [
            "",
            "me at home",
            "a\r\nX-Injected: 1",
            "a)b",
            "(a",
            "m\u{fc}ller@example.org",
        ] {
            assert!(!contact_fits_a_header(bad), "{bad:?}");
        }
    }

    #[test]
    fn synthetic_fixtures_are_recognised_in_both_header_forms() {
        assert!(is_synthetic(&json!({"fixture": {"synthetic": true}})));
        assert!(is_synthetic(
            &json!({"fixture": "synthetic: built from the docs"})
        ));
        assert!(!is_synthetic(&json!({"fixture": {"synthetic": false}})));
        assert!(!is_synthetic(&json!({"fixture": "recorded"})));
    }

    #[test]
    fn open_library_is_recorded_slowly_and_with_a_contact() {
        let r = RECORDINGS
            .iter()
            .find(|r| r.connector == "openlibrary")
            .unwrap();
        assert!(r.header.is_none() && r.trim);
        assert_eq!(r.env, "SOURCE_OPERATOR_CONTACT");
        assert!(r.pause_ms >= 1000, "at most one request per second");
    }
}
