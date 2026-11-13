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
                contact: catalog_core::ContactUse::None,
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

/// DEC-289: an admin's Disabled switch takes the source out of search at once, and back in.
#[test]
fn a_disabled_source_serves_nothing_until_enabled_again() {
    let mut r = Registry::default();
    r.register(Arc::new(Fake::new("tmdb", Some("k"))));
    let shared = r.clone();
    assert_eq!(r.for_kind("movie", Capability::Search).len(), 1);
    shared.set_enabled("tmdb", false);
    assert!(!r.is_enabled("tmdb"));
    assert_eq!(r.for_kind("movie", Capability::Search).len(), 0);
    // a switched-off source is not "skipped for a missing key" either
    assert_eq!(r.skipped(&["movie".into()], Capability::Search).len(), 0);
    shared.set_enabled("tmdb", true);
    assert!(r.is_enabled("tmdb"));
    assert_eq!(r.for_kind("movie", Capability::Search).len(), 1);
}

/// A details source that counts its calls.
struct DetailsSpy {
    inner: Fake,
    calls: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl catalog_core::Details for DetailsSpy {
    async fn details(
        &self,
        _ctx: &catalog_core::Ctx,
        req: &catalog_core::DetailsRequest,
    ) -> Result<catalog_core::Snapshot, catalog_core::ConnectorError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(catalog_core::Snapshot {
            connector: "tmdb".into(),
            kind: req.kind.clone(),
            external_ids: vec![domain::catalog::ExternalId::new("tmdb", &req.id)],
            title: "Heat (refreshed)".into(),
            original_title: None,
            year: Some(1995),
            description: Some("From the source.".into()),
            attributes: serde_json::json!({}),
            images: vec![],
            complete: true,
            popularity: None,
        })
    }
}

impl Connector for DetailsSpy {
    fn info(&self) -> &ConnectorInfo {
        &self.inner.info
    }

    fn key_slot(&self) -> Option<KeySlot> {
        self.inner.key_slot()
    }

    fn credentials(&self) -> CredentialSpec {
        self.inner.credentials()
    }

    fn as_details(&self) -> Option<&dyn catalog_core::Details> {
        Some(self)
    }
}

/// DEC-383 (deferred to WP4-8): the Disabled switch also stops detail refreshes: a switched
/// off source is not asked, the item keeps its data; switched on again, it refreshes.
#[tokio::test]
async fn a_disabled_source_is_not_asked_for_details() {
    let mut fake = Fake::new("tmdb", Some("k"));
    fake.info.capabilities = vec![Capability::Details];
    let spy = Arc::new(DetailsSpy {
        inner: fake,
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let mut r = Registry::default();
    r.register(spy.clone());
    let env = testkit::TestEnv::new();
    let item = env
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: None,
            external_ids: vec![domain::catalog::ExternalId::new("tmdb", "949")],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let http = Arc::new(testkit::ReplayHttp::default());
    let refresh = || {
        catalog_core::refresh::refresh_item(
            &env.app,
            &r,
            http.clone(),
            app::ports::Lane::BackgroundRefresh,
            item.id,
        )
    };
    r.set_enabled("tmdb", false);
    refresh().await.unwrap();
    assert_eq!(spy.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(env.app.catalog_item(item.id).await.unwrap().title, "Heat");
    r.set_enabled("tmdb", true);
    refresh().await.unwrap();
    assert_eq!(spy.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        env.app.catalog_item(item.id).await.unwrap().title,
        "Heat (refreshed)"
    );
}

/// v1-books 3.1.1: a source that requires the operator contact is not asked without one,
/// and says why.
#[test]
fn a_source_that_requires_the_contact_is_skipped_until_one_is_set() {
    use catalog_core::{ContactUse, OperatorContact};
    let mut needs = Fake::new("strict", Some("k"));
    needs.info.contact = ContactUse::Required;
    let mut likes = Fake::new("polite", Some("k"));
    likes.info.contact = ContactUse::Preferred;
    let contact = OperatorContact::default();
    let mut r = Registry::default().with_contact(contact.clone());
    r.register(Arc::new(needs)).register(Arc::new(likes));
    assert_eq!(r.credential("strict"), CredentialState::NeedsContact);
    assert_eq!(r.credential("polite"), CredentialState::Env);
    let ids = |r: &Registry| -> Vec<String> {
        r.for_kind("movie", Capability::Search)
            .iter()
            .map(|c| c.info().id.clone())
            .collect()
    };
    assert_eq!(ids(&r), ["polite"]);
    assert_eq!(
        r.skipped(&["movie".into()], Capability::Search),
        [("strict".to_string(), CredentialState::NeedsContact)]
    );
    contact.set(Some("ops@example.org")).unwrap();
    assert_eq!(r.credential("strict"), CredentialState::Env);
    assert_eq!(ids(&r), ["strict", "polite"]);
}

/// v1-plan.md 7, B2: an item no registered source answers for (entered by hand, or with
/// ids of sources this instance does not have) is neither changed nor marked failed by the
/// refresh job; an item of a registered source whose refresh fails is, as before.
#[tokio::test]
async fn an_item_no_source_answers_for_is_left_alone_by_the_refresh() {
    let mut fake = Fake::new("tmdb", None);
    fake.info.capabilities = vec![Capability::Details];
    let mut r = Registry::default();
    r.register(Arc::new(fake));
    let env = testkit::TestEnv::new();
    let movie = |ids: Vec<domain::catalog::ExternalId>, title: &str| app::NewCatalogItem {
        kind: domain::kind::KindId::new("movie").unwrap(),
        title: title.into(),
        original_title: None,
        year: None,
        description: None,
        external_ids: ids,
        attributes: serde_json::json!({}),
        cover_url: None,
        popularity: None,
    };
    let by_hand = env
        .app
        .create_manual_item(
            domain::kind::KindId::new("movie").unwrap(),
            app::ItemDetails {
                title: "By hand".into(),
                year: None,
                cover_url: None,
            },
            true,
        )
        .await
        .unwrap();
    let foreign = env
        .app
        .upsert_catalog_item(movie(
            vec![domain::catalog::ExternalId::new("elsewhere", "1")],
            "Foreign",
        ))
        .await
        .unwrap();
    let book = env
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("book").unwrap(),
            ..movie(
                vec![domain::catalog::ExternalId::new("tmdb", "7")],
                "A book with a film id",
            )
        })
        .await
        .unwrap();
    let ours = env
        .app
        .upsert_catalog_item(movie(
            vec![domain::catalog::ExternalId::new("tmdb", "949")],
            "Ours",
        ))
        .await
        .unwrap();
    assert!(!catalog_core::refresh::source_known(&r, &by_hand));
    assert!(!catalog_core::refresh::source_known(&r, &foreign));
    assert!(
        !catalog_core::refresh::source_known(&r, &book),
        "not its kind"
    );
    assert!(
        catalog_core::refresh::source_known(&r, &ours),
        "the source owns its items also without a key"
    );
    let http = Arc::new(testkit::ReplayHttp::default());
    for item in [&by_hand, &foreign, &book] {
        catalog_core::refresh::refresh_item(
            &env.app,
            &r,
            http.clone(),
            app::ports::Lane::BackgroundRefresh,
            item.id,
        )
        .await
        .unwrap();
        let after = env.app.catalog_item(item.id).await.unwrap();
        assert_eq!(&after, item, "untouched: no retry time, no failed state");
    }
    catalog_core::refresh::refresh_item(
        &env.app,
        &r,
        http.clone(),
        app::ports::Lane::BackgroundRefresh,
        ours.id,
    )
    .await
    .unwrap();
    let after = env.app.catalog_item(ours.id).await.unwrap();
    assert!(after.fresh_until.is_some(), "retried later, as before");
}
