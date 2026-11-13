//! The search fan-out hands each connector the requesting member's own credential for that
//! connector (design.md 4.1: `TheTVDB` subscriber PIN; DEC-246 follow-up), and nothing to the
//! others.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use catalog_core::search::{SearchFanout, UserKeys};
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, CredentialSpec, Ctx,
    KeySlot, RateLimit, Registry, Search, SearchRequest, Snapshot,
};

struct Spy {
    info: ConnectorInfo,
    key: KeySlot,
    seen: Arc<Mutex<Vec<Option<String>>>>,
}

impl Spy {
    fn new(id: &str, seen: Arc<Mutex<Vec<Option<String>>>>) -> Self {
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
                contact: catalog_core::ContactUse::None,
            },
            key: KeySlot::new(Some("instance".into())),
            seen,
        }
    }
}

#[async_trait]
impl Search for Spy {
    async fn search(
        &self,
        ctx: &Ctx,
        _req: &SearchRequest,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        self.seen
            .lock()
            .unwrap()
            .push(ctx.user_key.as_deref().cloned());
        Ok(vec![])
    }
}

impl Connector for Spy {
    fn info(&self) -> &ConnectorInfo {
        &self.info
    }

    fn key_slot(&self) -> Option<KeySlot> {
        Some(self.key.clone())
    }

    fn credentials(&self) -> CredentialSpec {
        CredentialSpec {
            instance_fields: &["api_key"],
            user_fields: &["pin"],
        }
    }

    fn as_search(&self) -> Option<&dyn Search> {
        Some(self)
    }
}

#[tokio::test]
async fn each_connector_gets_the_members_own_key_and_no_other() {
    let env = testkit::TestEnv::new();
    let (mine, other) = (Arc::default(), Arc::default());
    let mut r = Registry::default();
    r.register(Arc::new(Spy::new("mine", Arc::clone(&mine))));
    r.register(Arc::new(Spy::new("other", Arc::clone(&other))));
    let http = Arc::new(testkit::ReplayHttp::default());
    let fanout = SearchFanout::new(env.app.clone(), r, http);
    let mut keys = UserKeys::new();
    keys.insert("mine".into(), zeroize::Zeroizing::new("1234".into()));
    let kinds = vec!["movie".to_string()];
    fanout.run_as(&keys, "alien", &kinds, 5, |_| async {}).await;
    fanout.run("alien", &kinds, 5, |_| async {}).await;
    assert_eq!(*mine.lock().unwrap(), vec![Some("1234".to_string()), None]);
    assert_eq!(*other.lock().unwrap(), vec![None, None]);
}

/// UX2-S9: the members' keys never show up in `Debug` output.
#[test]
fn user_keys_redact_their_values_in_debug() {
    let mut keys = UserKeys::new();
    keys.insert("tvdb".into(), zeroize::Zeroizing::new("pin-5678".into()));
    let shown = format!("{keys:?}");
    assert!(!shown.contains("pin-5678"), "{shown}");
    assert!(shown.contains("tvdb"), "{shown}");
}
