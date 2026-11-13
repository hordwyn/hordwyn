//! TMDB terms 1.C (DEC-446/450): catalog data from a source whose terms limit caching is
//! refreshed before it gets old, or purged when no refresh is possible (fake clock).

use std::sync::Arc;

use app::{CatalogRefresh, NewCatalogItem};
use catalog_core::retention::{HARD_LIMIT_DAYS, sweep};
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorInfo, CredentialSpec, KeySlot, RateLimit,
    Registry,
};
use chrono::Duration;
use domain::catalog::{EntityState, ExternalId};
use domain::ids::ItemId;
use domain::kind::KindId;
use testkit::TestEnv;

const MAX_AGE_DAYS: u32 = 150;

struct Fake {
    info: ConnectorInfo,
    key: KeySlot,
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

fn registry(tmdb_key: Option<&str>) -> Registry {
    let mut r = Registry::default();
    r.register(Arc::new(Fake {
        info: ConnectorInfo {
            id: "tmdb".into(),
            name: "TMDB".into(),
            kinds: vec!["movie".into()],
            capabilities: vec![Capability::Search, Capability::Details],
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
        key: KeySlot::new(tmdb_key.map(String::from)),
    }));
    r
}

async fn movie(env: &TestEnv, title: &str, ns: &str) -> ItemId {
    env.app
        .upsert_catalog_item(NewCatalogItem {
            kind: KindId::new("movie").unwrap(),
            title: title.into(),
            original_title: Some(format!("{title} (original)")),
            year: Some(1995),
            description: Some("From the source.".into()),
            external_ids: vec![ExternalId::new(ns, title)],
            attributes: serde_json::json!({"genres": ["x"]}),
            cover_url: Some(format!("https://image.example/{title}.jpg")),
            popularity: None,
        })
        .await
        .unwrap()
        .id
}

/// A person with `item` in the library (it is then referenced and kept).
async fn own(env: &TestEnv, item: ItemId) -> domain::user::Actor {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let u = env
        .app
        .register_user(
            &format!("person{n}"),
            "Person",
            None,
            domain::user::Role::Member,
        )
        .await
        .unwrap();
    let me = domain::user::Actor {
        user_id: u.id,
        role: u.role,
    };
    env.app.add_to_library(me, item, None).await.unwrap();
    me
}

fn refresh_jobs(env: &TestEnv) -> Vec<serde_json::Value> {
    env.jobs
        .jobs()
        .into_iter()
        .filter(|j| j.kind == app::REFRESH_JOB)
        .map(|j| {
            assert_eq!(j.lane, app::ports::Lane::BackgroundRefresh);
            j.payload["item_id"].clone()
        })
        .collect()
}

#[tokio::test]
async fn old_data_is_refreshed_while_the_source_can_answer() {
    let env = TestEnv::new();
    let old = movie(&env, "Heat", "tmdb").await;
    let other_source = movie(&env, "Ronin", "test").await;
    own(&env, old).await;
    env.clock.advance(Duration::days(100));
    let young = movie(&env, "Collateral", "tmdb").await;
    env.clock.advance(Duration::days(51));
    let reg = registry(Some("key"));

    let report = sweep(&env.app, &reg, MAX_AGE_DAYS).await.unwrap();
    assert_eq!(report.refreshed, vec![old]);
    assert_eq!(report.purged.len(), 0);
    assert_eq!(refresh_jobs(&env), vec![serde_json::json!(old)]);
    // nothing is purged while a refresh is possible
    let item = env.app.catalog_item(old).await.unwrap();
    assert_eq!(item.description.as_deref(), Some("From the source."));
    for id in [young, other_source] {
        assert_eq!(
            env.app.catalog_item(id).await.unwrap().state,
            EntityState::Partial
        );
    }

    // once refreshed, the item is young again
    env.app
        .apply_catalog_refresh(
            old,
            CatalogRefresh {
                title: "Heat".into(),
                original_title: None,
                year: Some(1995),
                description: Some("Fresh.".into()),
                attributes: serde_json::json!({"genres": ["x"]}),
                external_ids: vec![],
                cover_url: None,
                provenance: std::collections::BTreeMap::new(),
                fresh_for_secs: 60,
                popularity: None,
            },
        )
        .await
        .unwrap();
    let report = sweep(&env.app, &reg, MAX_AGE_DAYS).await.unwrap();
    assert_eq!(report.refreshed.len(), 0);
}

#[tokio::test]
async fn old_data_is_purged_when_no_refresh_is_possible() {
    let env = TestEnv::new();
    let old = movie(&env, "Heat", "tmdb").await;
    own(&env, old).await;
    let mut with_cover = env.app.catalog_item(old).await.unwrap();
    with_cover.cover_asset = Some("asset-1".into());
    env.app.repos.catalog.upsert(&with_cover).await.unwrap();
    env.clock
        .advance(Duration::days(i64::from(MAX_AGE_DAYS) + 1));
    let no_key = registry(None);

    let report = sweep(&env.app, &no_key, MAX_AGE_DAYS).await.unwrap();
    assert_eq!(report.refreshed.len(), 0);
    assert_eq!(report.purged.len(), 1);
    assert_eq!(report.purged[0].id, old);
    assert_eq!(report.purged[0].cover_asset.as_deref(), Some("asset-1"));
    assert_eq!(refresh_jobs(&env).len(), 0);
    let item = env.app.catalog_item(old).await.unwrap();
    // only the external id stays (UX4-9: title and year came from TMDB too)
    assert_eq!(item.title, "");
    assert!(item.title_unavailable());
    assert_eq!(item.year, None);
    assert_eq!(item.external_ids, vec![ExternalId::new("tmdb", "Heat")]);
    assert_eq!(item.description, None);
    assert_eq!(item.original_title, None);
    assert_eq!(item.cover_url, None);
    assert_eq!(item.cover_asset, None);
    assert_eq!(item.attributes, serde_json::json!({}));
    assert_eq!(item.state, EntityState::Stub);
    assert_eq!(item.fetched_at, None);

    // a purged item is not picked again
    let report = sweep(&env.app, &no_key, MAX_AGE_DAYS).await.unwrap();
    assert_eq!(report.purged.len(), 0);
}

#[tokio::test]
async fn data_past_the_hard_limit_is_purged_even_with_a_key() {
    let env = TestEnv::new();
    let stuck = movie(&env, "Heat", "tmdb").await;
    own(&env, stuck).await;
    // refreshes kept failing: the data is older than the terms allow
    env.clock.advance(Duration::days(HARD_LIMIT_DAYS + 1));
    let report = sweep(&env.app, &registry(Some("key")), MAX_AGE_DAYS)
        .await
        .unwrap();
    assert_eq!(report.refreshed.len(), 0);
    assert_eq!(report.purged.len(), 1);
    assert_eq!(report.purged[0].id, stuck);
    assert_eq!(env.app.catalog_item(stuck).await.unwrap().description, None);
}

fn entry(fresh_until: chrono::DateTime<chrono::Utc>) -> catalog_core::cache::CacheEntry {
    catalog_core::cache::CacheEntry {
        body_hash: String::new(),
        status: 200,
        headers: vec![],
        etag: None,
        last_modified: None,
        fetched_at: fresh_until - Duration::days(7),
        fresh_until,
        size: 0,
    }
}

/// Stored provider responses follow the same age limit: entries stale for longer are deleted,
/// with their bodies unless a kept entry has the same body.
async fn stale_http_cache_entries_go(index: Arc<dyn catalog_core::cache::HttpCacheIndex>) {
    use catalog_core::cache::{HttpCache, body_key};
    let blobs = Arc::new(testkit::MemBlobs::default());
    let cache = HttpCache {
        index,
        blobs: blobs.clone(),
    };
    let t0 = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let old = t0 - Duration::days(200);
    cache
        .store("old-unique", entry(old), b"only old")
        .await
        .unwrap();
    cache
        .store("old-shared", entry(old), b"shared")
        .await
        .unwrap();
    cache
        .store("young-shared", entry(t0), b"shared")
        .await
        .unwrap();

    let removed = cache
        .purge_stale_before(t0 - Duration::days(i64::from(MAX_AGE_DAYS)))
        .await
        .unwrap();
    assert_eq!(removed, 2);
    assert!(cache.lookup("old-unique").await.unwrap().is_none());
    assert!(cache.lookup("old-shared").await.unwrap().is_none());
    assert_eq!(
        cache.lookup("young-shared").await.unwrap().unwrap().1,
        b"shared".to_vec()
    );
    let only_old = blake3::hash(b"only old").to_hex().to_string();
    assert_eq!(
        app::ports::BlobStore::get(blobs.as_ref(), &body_key(&only_old))
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn stale_http_cache_entries_go_in_memory() {
    stale_http_cache_entries_go(Arc::new(catalog_core::cache::MemCacheIndex::default())).await;
}

#[tokio::test]
async fn stale_http_cache_entries_go_on_postgres() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    stale_http_cache_entries_go(Arc::new(catalog_core::pg::PgCacheIndex::new(pool))).await;
}

#[tokio::test]
async fn an_item_nobody_references_is_deleted_at_the_limit_instead_of_refreshed() {
    // UX4-9: search results and removed items are not refreshed forever
    let env = TestEnv::new();
    let unowned = movie(&env, "Heat", "tmdb").await;
    let mut with_cover = env.app.catalog_item(unowned).await.unwrap();
    with_cover.cover_asset = Some("asset-1".into());
    env.app.repos.catalog.upsert(&with_cover).await.unwrap();
    let in_library = movie(&env, "Ronin", "tmdb").await;
    let me = own(&env, in_library).await;
    let in_list = movie(&env, "Collateral", "tmdb").await;
    let list = env
        .app
        .create_list(me, "Mix", None, domain::list::Visibility::Private)
        .await
        .unwrap();
    env.app
        .add_to_list(me, list.id, in_list, None)
        .await
        .unwrap();
    // a removed entry keeps its history: still referenced
    let in_history = movie(&env, "Thief", "tmdb").await;
    env.app.add_to_library(me, in_history, None).await.unwrap();
    env.app
        .log_progress(
            me,
            in_history,
            domain::progress::Progress::Finished,
            None,
            None,
        )
        .await
        .unwrap();
    env.app.remove_from_library(me, in_history).await.unwrap();
    env.clock
        .advance(Duration::days(i64::from(MAX_AGE_DAYS) + 1));

    let report = sweep(&env.app, &registry(Some("key")), MAX_AGE_DAYS)
        .await
        .unwrap();
    assert_eq!(report.deleted.len(), 1);
    assert_eq!(report.deleted[0].id, unowned);
    assert_eq!(report.deleted[0].cover_asset.as_deref(), Some("asset-1"));
    assert!(env.app.catalog_item(unowned).await.is_err());
    let mut refreshed = report.refreshed.clone();
    refreshed.sort_by_key(|i| i.0);
    let mut kept = vec![in_library, in_list, in_history];
    kept.sort_by_key(|i| i.0);
    assert_eq!(refreshed, kept);
    assert_eq!(report.purged.len(), 0);
}

#[tokio::test]
async fn a_purge_keeps_the_persons_own_data_and_refetches_once_tmdb_is_usable() {
    // UX4-9: past the limit without a usable source only the external id and the person's
    // data stay; with a key again the item is fetched again, at most once a day
    let env = TestEnv::new();
    let heat = movie(&env, "Heat", "tmdb").await;
    let me = own(&env, heat).await;
    env.app
        .set_status(me, heat, domain::kind::Status::completed(), None)
        .await
        .unwrap();
    env.app
        .set_notes(me, heat, Some("mine".into()), None)
        .await
        .unwrap();
    env.app.add_tag(me, heat, "Cosy").await.unwrap();
    env.clock
        .advance(Duration::days(i64::from(MAX_AGE_DAYS) + 1));
    let report = sweep(&env.app, &registry(None), MAX_AGE_DAYS)
        .await
        .unwrap();
    assert_eq!(report.purged.len(), 1);
    let item = env.app.catalog_item(heat).await.unwrap();
    assert!(item.title_unavailable());
    assert_eq!((item.year, item.description), (None, None));
    let entry = env.app.entry(me, heat).await.unwrap().unwrap();
    assert_eq!(entry.status.value, domain::kind::Status::completed());
    assert_eq!(entry.notes.value.as_deref(), Some("mine"));
    assert_eq!(env.app.entry_tags(me, heat).await.unwrap(), ["Cosy"]);

    // no key yet: nothing to refetch
    let report = sweep(&env.app, &registry(None), MAX_AGE_DAYS)
        .await
        .unwrap();
    assert_eq!(report.refetched.len(), 0);
    // a key again: one refetch, not again within a day
    let reg = registry(Some("key"));
    let report = sweep(&env.app, &reg, MAX_AGE_DAYS).await.unwrap();
    assert_eq!(report.refetched, vec![heat]);
    assert_eq!(refresh_jobs(&env), vec![serde_json::json!(heat)]);
    env.clock.advance(Duration::hours(2));
    let report = sweep(&env.app, &reg, MAX_AGE_DAYS).await.unwrap();
    assert_eq!(report.refetched.len(), 0);
    env.clock.advance(Duration::days(1));
    let report = sweep(&env.app, &reg, MAX_AGE_DAYS).await.unwrap();
    assert_eq!(report.refetched, vec![heat]);

    // the refetch brings the data back and the item is no longer expired
    env.app
        .apply_catalog_refresh(
            heat,
            CatalogRefresh {
                title: "Heat".into(),
                original_title: None,
                year: Some(1995),
                description: Some("Fresh.".into()),
                attributes: serde_json::json!({"genres": ["x"]}),
                external_ids: vec![],
                cover_url: None,
                provenance: std::collections::BTreeMap::new(),
                fresh_for_secs: 60,
                popularity: None,
            },
        )
        .await
        .unwrap();
    assert!(
        !env.app
            .catalog_item(heat)
            .await
            .unwrap()
            .title_unavailable()
    );
    env.clock.advance(Duration::days(2));
    let report = sweep(&env.app, &reg, MAX_AGE_DAYS).await.unwrap();
    assert_eq!((report.refetched.len(), report.purged.len()), (0, 0));
}
