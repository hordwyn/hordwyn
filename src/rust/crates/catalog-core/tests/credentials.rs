//! design.md 4.2: connectors are always registered; a credential state decides whether
//! search and refresh may use them.

use std::sync::Arc;

use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorInfo, CredentialSpec, CredentialState,
    KeySlot, RateLimit, Registry, SourceStats,
};

struct Fake {
    info: ConnectorInfo,
    key: KeySlot,
}

impl Fake {
    fn new(id: &str, key: Option<&str>) -> Self {
        Self {
            info: ConnectorInfo {
                id: id.into(),
                name: id.to_uppercase(),
                kinds: vec!["movie".into()],
                capabilities: vec![Capability::Search],
                rate_limit: RateLimit {
                    per_second: 1,
                    burst: 1,
                    max_concurrency: 1,
                },
                auth: Auth::Bearer { env: "X".into() },
                attribution: Attribution {
                    text: String::new(),
                    url: String::new(),
                    licence: String::new(),
                },
                hosts: vec![],
                cache_ttl_secs: 0,
            },
            key: KeySlot::new(key.map(String::from)),
        }
    }
}

impl Connector for Fake {
    fn info(&self) -> &ConnectorInfo {
        &self.info
    }

    fn key_slot(&self) -> Option<KeySlot> {
        Some(self.key.clone())
    }

    fn credentials(&self) -> CredentialSpec {
        CredentialSpec {
            instance_fields: &["api_key"],
            user_fields: &[],
        }
    }
}

#[test]
fn missing_rejected_and_locked_connectors_are_registered_but_skipped() {
    let stats = Arc::new(SourceStats::default());
    let mut r = Registry::default().with_stats(stats.clone());
    r.register(Arc::new(Fake::new("a", Some("k"))));
    r.register(Arc::new(Fake::new("b", None)));
    assert_eq!(r.infos().len(), 2, "always registered");
    assert_eq!(r.credential("a"), CredentialState::Env);
    assert_eq!(r.credential("b"), CredentialState::Missing);
    let ids = |r: &Registry| -> Vec<String> {
        r.for_kind("movie", Capability::Search)
            .iter()
            .map(|c| c.info().id.clone())
            .collect()
    };
    assert_eq!(ids(&r), vec!["a"]);
    // a key set in the admin UI takes effect at once, in every clone of the registry
    let clone = r.clone();
    r.set_key("b", Some("db-key".into()), CredentialState::Db);
    assert_eq!(ids(&clone), vec!["a", "b"]);
    assert_eq!(
        clone
            .get("b")
            .unwrap()
            .key_slot()
            .unwrap()
            .get()
            .as_deref()
            .map(String::as_str),
        Some("db-key")
    );
    // last answer 401: invalid until the key changes
    stats.record_status("a", 401);
    assert_eq!(r.credential("a"), CredentialState::Invalid);
    assert_eq!(ids(&r), vec!["b"]);
    r.set_key("a", Some("new".into()), CredentialState::Db);
    assert_eq!(r.credential("a"), CredentialState::Db);
    r.set_key("b", None, CredentialState::Locked);
    assert_eq!(ids(&r), vec!["a"]);
    let skipped = r.skipped(&["movie".to_string()], Capability::Search);
    assert_eq!(skipped, vec![("b".to_string(), CredentialState::Locked)]);
}

#[test]
fn key_slots_bump_their_version_and_hide_the_value_in_debug() {
    let s = KeySlot::new(Some("secret-value".into()));
    let v = s.version();
    s.set(Some("other".into()));
    assert!(s.version() > v);
    assert!(!format!("{s:?}").contains("other"));
}
