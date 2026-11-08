//! State and wording helpers of the settings and admin area (addendum-1 3.2 and 3.4,
//! design.md 3.7): the page lists of the side nav, setting provenance, the editable
//! numbers, the audit filter that lives in the URL, and status dots. Pure functions, so
//! the pages stay thin and these are unit tested.

use contracts::admin::{AuditQueryDto, SettingDto};
use serde_json::Value;
use ui::StatusState;

/// The eight admin pages (path, nav label key) in the order of addendum-1 3.4 (DEC-255
/// reversed in phase 3: Branding is the Overview card, Security moved into Instance).
pub const ADMIN_PAGES: [(&str, &str); 8] = [
    ("/admin", "admin-nav-overview"),
    ("/admin/instance", "admin-nav-instance"),
    ("/admin/sso", "admin-nav-sso"),
    ("/admin/users", "admin-nav-users"),
    ("/admin/sources", "admin-nav-sources"),
    ("/admin/notifications", "admin-nav-notifications"),
    ("/admin/storage", "admin-nav-storage"),
    ("/admin/audit", "admin-nav-audit"),
];

/// The personal settings pages ("You", design.md 3.3 and 6.12).
pub const SETTINGS_PAGES: [(&str, &str); 6] = [
    ("/settings/account", "settings-nav-account"),
    ("/settings/notifications", "nav-notifications"),
    ("/settings/integrations", "nav-integrations"),
    ("/settings/sources", "nav-sources"),
    ("/settings/imports", "settings-nav-imports"),
    ("/settings/privacy", "settings-nav-privacy"),
];

/// Message key of a setting's label (`admin-setting-<key with . replaced by _>`).
#[must_use]
pub fn setting_label_key(key: &str) -> String {
    format!("admin-setting-{}", key.replace('.', "_"))
}

/// Where a setting's value comes from, as the page shows it (addendum-1 3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// Stored or default: the admin can change it here.
    Editable,
    /// Set by this environment variable: read-only with its name.
    Env(String),
    /// Set in the configuration file: read-only.
    File,
}

#[must_use]
pub fn origin(s: &SettingDto) -> Origin {
    match s.source.as_str() {
        "env" => Origin::Env(s.env_var.clone().unwrap_or_else(|| s.env_name.clone())),
        "file" => Origin::File,
        _ if !s.editable => Origin::File,
        _ => Origin::Editable,
    }
}

#[must_use]
pub fn find_setting<'a>(list: &'a [SettingDto], key: &str) -> Option<&'a SettingDto> {
    list.iter().find(|s| s.key == key)
}

const UNITS: [(&str, u64); 5] = [
    ("TiB", 1 << 40),
    ("GiB", 1 << 30),
    ("MiB", 1 << 20),
    ("KiB", 1 << 10),
    ("B", 1),
];

/// A byte count in binary units with at most one decimal ("1.5 GiB").
#[must_use]
pub fn format_bytes(n: u64) -> String {
    let (unit, size) = UNITS
        .iter()
        .find(|(_, m)| n >= *m)
        .copied()
        .unwrap_or(("B", 1));
    // sizes stay far below 2^53
    #[allow(clippy::cast_precision_loss)]
    let v = n as f64 / size as f64;
    if (v - v.round()).abs() < 0.05 {
        format!("{v:.0} {unit}")
    } else {
        format!("{v:.1} {unit}")
    }
}

/// The value sent for a byte size as typed ("20 GiB"; the server parses it).
#[must_use]
pub fn byte_value(text: &str) -> Option<Value> {
    let t = text.trim();
    (!t.is_empty()).then(|| Value::String(t.to_string()))
}

/// A watermark fraction as a percent number ("90", "82.5").
#[must_use]
pub fn percent_text(fraction: f64) -> String {
    let p = (fraction * 1000.0).round() / 10.0;
    if (p - p.round()).abs() < f64::EPSILON {
        format!("{p:.0}")
    } else {
        format!("{p:.1}")
    }
}

/// A percent typed by the admin ("90", "82,5 %") as the fraction the server stores.
#[must_use]
pub fn fraction_value(text: &str) -> Option<Value> {
    let p: f64 = text
        .trim()
        .trim_end_matches('%')
        .trim()
        .replace(',', ".")
        .parse()
        .ok()?;
    (p > 0.0 && p <= 100.0).then(|| Value::from(p / 100.0))
}

/// Session length in days (1 to 365).
#[must_use]
pub fn days_value(text: &str) -> Option<Value> {
    let d: u32 = text.trim().parse().ok()?;
    (1..=365).contains(&d).then(|| Value::from(d))
}

/// The audit log filters, kept in the URL query (`?actor=<user id>&action=role.changed`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuditFilter {
    /// User id, empty for anyone.
    pub actor: String,
    /// Exact action or a prefix ending in `.`, empty for any.
    pub action: String,
}

/// Percent-encode a query value (RFC 3986 unreserved characters stay).
fn encode(v: &str) -> String {
    v.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

impl AuditFilter {
    /// From the URL query pairs (already decoded); unknown keys are ignored.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut f = Self::default();
        for (k, v) in pairs {
            match k {
                "actor" => f.actor = v.to_string(),
                "action" => f.action = v.to_string(),
                _ => {}
            }
        }
        f
    }

    /// `?actor=..&action=..` with the set filters, or empty.
    #[must_use]
    pub fn query(&self) -> String {
        let parts: Vec<String> = [("actor", &self.actor), ("action", &self.action)]
            .into_iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| format!("{k}={}", encode(v)))
            .collect();
        if parts.is_empty() {
            String::new()
        } else {
            format!("?{}", parts.join("&"))
        }
    }

    /// The Export CSV link with the same filters.
    #[must_use]
    pub fn csv_href(&self) -> String {
        format!("/api/v1/admin/audit.csv{}", self.query())
    }

    #[must_use]
    pub fn to_dto(&self, limit: u32) -> AuditQueryDto {
        let some = |v: &str| (!v.is_empty()).then(|| v.to_string());
        AuditQueryDto {
            actor: some(&self.actor),
            action: some(&self.action),
            limit: Some(limit),
            ..AuditQueryDto::default()
        }
    }
}

/// Message key of an audit action (`admin-audit-<action with . replaced by _>`).
#[must_use]
pub fn audit_action_key(action: &str) -> String {
    format!("admin-audit-{}", action.replace('.', "_"))
}

fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// One line for an audit row's detail: `key: from -> to` for changes (`from -> to` when no
/// key is named), else the fields in order; empty values and the proof method are left
/// out (the detail never holds secret values).
#[must_use]
pub fn audit_detail(detail: &Value) -> String {
    let Value::Object(m) = detail else {
        return String::new();
    };
    if let (Some(from), Some(to)) = (m.get("from"), m.get("to")) {
        let change = format!("{} -> {}", plain(from), plain(to));
        return match m.get("key") {
            Some(k) => format!("{}: {change}", plain(k)),
            None => change,
        };
    }
    m.iter()
        .filter(|(k, v)| !v.is_null() && k.as_str() != "proof")
        .map(|(k, v)| format!("{k}: {}", plain(v)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The dot of an Overview health row (`ok`, `warning`, `error`, `off`).
#[must_use]
pub fn health_state(status: &str) -> StatusState {
    match status {
        "ok" => StatusState::Ok,
        "warning" => StatusState::Warn,
        "error" => StatusState::Bad,
        _ => StatusState::Off,
    }
}

/// The dot of a data-source state (design.md 4.4).
#[must_use]
pub fn source_state(state: &str) -> StatusState {
    match state {
        "working" => StatusState::Ok,
        "rate_limited" | "slow" | "retrying" | "circuit_open" => StatusState::Warn,
        "key_rejected" | "key_unavailable" | "unavailable" => StatusState::Bad,
        _ => StatusState::Off,
    }
}

/// A member asks an admin about a source unless it works or only needs their own account.
#[must_use]
pub fn ask_admin_for(state: &str) -> bool {
    !matches!(state, "working" | "needs_account")
}

/// "How to get a key" for a source (design.md 5.1 step 3, 6.10; UX2-W1): three numbered
/// lines and the provider's developer page (URLs from `docs/connectors/<id>.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyGuide {
    /// Message keys of the three lines, in order.
    pub steps: [&'static str; 3],
    pub url: &'static str,
}

/// The guide of a connector that needs an instance key; `None` for the others.
#[must_use]
pub fn key_guide(id: &str) -> Option<KeyGuide> {
    let (steps, url) = match id {
        "tmdb" => (
            [
                "admin-sources-howto-tmdb-1",
                "admin-sources-howto-tmdb-2",
                "admin-sources-howto-tmdb-3",
            ],
            "https://developer.themoviedb.org/reference/intro/getting-started",
        ),
        "igdb" => (
            [
                "admin-sources-howto-igdb-1",
                "admin-sources-howto-igdb-2",
                "admin-sources-howto-igdb-3",
            ],
            "https://api-docs.igdb.com/",
        ),
        "tvdb" => (
            [
                "admin-sources-howto-tvdb-1",
                "admin-sources-howto-tvdb-2",
                "admin-sources-howto-tvdb-3",
            ],
            "https://thetvdb.com/api-information",
        ),
        "podcastindex" => (
            [
                "admin-sources-howto-podcastindex-1",
                "admin-sources-howto-podcastindex-2",
                "admin-sources-howto-podcastindex-3",
            ],
            "https://api.podcastindex.org",
        ),
        _ => return None,
    };
    Some(KeyGuide { steps, url })
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::admin::SettingDto;
    use serde_json::json;

    #[test]
    fn every_keyed_source_has_a_three_line_guide_with_an_https_link() {
        for id in ["tmdb", "igdb", "tvdb", "podcastindex"] {
            let g = key_guide(id).unwrap_or_else(|| panic!("{id}"));
            assert!(g.url.starts_with("https://"), "{id}");
            assert!(g.steps.iter().all(|k| k.contains(id)), "{id}");
        }
        assert_eq!(key_guide("openlibrary"), None, "no key, no guide");
    }

    fn setting(key: &str, source: &str, env_var: Option<&str>) -> SettingDto {
        SettingDto {
            key: key.into(),
            value: json!(30),
            source: source.into(),
            env_var: env_var.map(String::from),
            env_name: format!("APP__{}", key.to_uppercase().replace('.', "__")),
            editable: !matches!(source, "env" | "file"),
            shadowed_value: None,
            needs_reauth: true,
            applies_next_session: true,
        }
    }

    #[test]
    fn eight_admin_pages_in_the_addendum_order() {
        let hrefs: Vec<_> = ADMIN_PAGES.iter().map(|(h, _)| *h).collect();
        assert_eq!(
            hrefs,
            [
                "/admin",
                "/admin/instance",
                "/admin/sso",
                "/admin/users",
                "/admin/sources",
                "/admin/notifications",
                "/admin/storage",
                "/admin/audit"
            ]
        );
        let you: Vec<_> = SETTINGS_PAGES.iter().map(|(h, _)| *h).collect();
        assert_eq!(
            you,
            [
                "/settings/account",
                "/settings/notifications",
                "/settings/integrations",
                "/settings/sources",
                "/settings/imports",
                "/settings/privacy"
            ]
        );
    }

    #[test]
    fn settings_carry_their_label_key_and_origin() {
        assert_eq!(
            setting_label_key("auth.session_days"),
            "admin-setting-auth_session_days"
        );
        let env = setting(
            "auth.secure_cookies",
            "env",
            Some("APP__AUTH__SECURE_COOKIES"),
        );
        assert_eq!(
            origin(&env),
            Origin::Env("APP__AUTH__SECURE_COOKIES".into())
        );
        assert_eq!(
            origin(&setting("auth.secure_cookies", "file", None)),
            Origin::File
        );
        assert_eq!(
            origin(&setting("auth.session_days", "db", None)),
            Origin::Editable
        );
        assert_eq!(
            origin(&setting("auth.session_days", "default", None)),
            Origin::Editable
        );
        let list = vec![env.clone(), setting("auth.session_days", "db", None)];
        assert_eq!(find_setting(&list, "auth.secure_cookies"), Some(&env));
        assert_eq!(find_setting(&list, "nope"), None);
    }

    #[test]
    fn byte_sizes_read_and_write_in_binary_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(900), "900 B");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5 GiB");
        assert_eq!(format_bytes(1536 * 1024 * 1024), "1.5 GiB");
        assert_eq!(format_bytes(512 * 1024 * 1024), "512 MiB");
        assert_eq!(byte_value(" 2 GiB "), Some(json!("2 GiB")));
        assert_eq!(byte_value("  "), None);
    }

    #[test]
    fn watermarks_are_edited_as_percent() {
        assert_eq!(percent_text(0.9), "90");
        assert_eq!(percent_text(0.825), "82.5");
        assert_eq!(fraction_value("90"), Some(json!(0.9)));
        assert_eq!(fraction_value("82,5 %"), Some(json!(0.825)));
        assert_eq!(fraction_value("0"), None);
        assert_eq!(fraction_value("101"), None);
        assert_eq!(fraction_value("lots"), None);
        assert_eq!(days_value("30"), Some(json!(30)));
        assert_eq!(days_value("0"), None);
        assert_eq!(days_value("366"), None);
    }

    #[test]
    fn the_audit_filter_lives_in_the_url() {
        let f = AuditFilter::from_pairs([
            ("actor", "0190-ab"),
            ("action", "role.changed"),
            ("other", "x"),
        ]);
        assert_eq!(f.actor, "0190-ab");
        assert_eq!(f.action, "role.changed");
        assert_eq!(f.query(), "?actor=0190-ab&action=role.changed");
        assert_eq!(
            f.csv_href(),
            "/api/v1/admin/audit.csv?actor=0190-ab&action=role.changed"
        );
        let dto = f.to_dto(50);
        assert_eq!(dto.actor.as_deref(), Some("0190-ab"));
        assert_eq!(dto.action.as_deref(), Some("role.changed"));
        assert_eq!(dto.limit, Some(50));
        let none = AuditFilter::default();
        assert_eq!(none.query(), "");
        assert_eq!(none.to_dto(50).actor, None);
        assert_eq!(none.csv_href(), "/api/v1/admin/audit.csv");
        // values are encoded
        let odd = AuditFilter {
            action: "a b&c".into(),
            ..AuditFilter::default()
        };
        assert_eq!(odd.query(), "?action=a%20b%26c");
    }

    #[test]
    fn audit_rows_name_the_action_and_summarise_the_detail() {
        assert_eq!(
            audit_action_key("setting.changed"),
            "admin-audit-setting_changed"
        );
        assert_eq!(
            audit_detail(
                &json!({"key": "auth.signup", "from": "open", "to": "invite", "proof": "password"})
            ),
            "auth.signup: open -> invite"
        );
        // role changes name no key; the proof method is not shown
        assert_eq!(
            audit_detail(&json!({"from": "member", "to": "admin", "proof": "password"})),
            "member -> admin"
        );
        assert_eq!(
            audit_detail(&json!({"username": "mia", "proof": "password"})),
            "username: mia"
        );
        assert_eq!(audit_detail(&json!({"proof": null, "n": 3})), "n: 3");
        assert_eq!(audit_detail(&json!(null)), "");
    }

    #[test]
    fn health_and_source_states_map_to_a_dot() {
        use ui::StatusState;
        assert_eq!(health_state("ok"), StatusState::Ok);
        assert_eq!(health_state("warning"), StatusState::Warn);
        assert_eq!(health_state("error"), StatusState::Bad);
        assert_eq!(health_state("off"), StatusState::Off);
        assert_eq!(source_state("working"), StatusState::Ok);
        assert_eq!(source_state("rate_limited"), StatusState::Warn);
        assert_eq!(source_state("slow"), StatusState::Warn);
        assert_eq!(source_state("key_rejected"), StatusState::Bad);
        assert_eq!(source_state("not_set_up"), StatusState::Off);
        assert_eq!(source_state("needs_account"), StatusState::Off);
    }

    #[test]
    fn members_ask_for_sources_that_do_not_work_for_them() {
        assert!(!ask_admin_for("working"));
        assert!(
            !ask_admin_for("needs_account"),
            "they set their own credential"
        );
        assert!(ask_admin_for("not_set_up"));
        assert!(ask_admin_for("unavailable"));
    }
}
