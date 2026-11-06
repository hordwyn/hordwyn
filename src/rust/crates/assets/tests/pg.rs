//! The Postgres object index and asset repository behave like the in-memory ones.

use std::sync::Arc;

use assets::index::OpCounts;
use assets::pg::{PgAssetRepo, PgObjectIndex};
use assets::{
    AssetKind, AssetRecord, AssetRepo, FetchPriority, MemObjectIndex, ObjectClass, ObjectIndex,
    StoredObject, Tier,
};
use chrono::{Duration, TimeZone, Utc};
use uuid::Uuid;

fn object(key: &str, tier: Tier, bytes: u64, age_h: i64, p: FetchPriority) -> StoredObject {
    StoredObject {
        key: key.into(),
        class: ObjectClass::Variant,
        tier,
        bytes,
        owner: None,
        asset: None,
        priority: p,
        last_accessed: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() - Duration::hours(age_h),
    }
}

async fn exercise(index: Arc<dyn ObjectIndex>) -> Vec<String> {
    let asset = Uuid::now_v7();
    index
        .record(&object("a", Tier::Cache, 10, 5, FetchPriority::Library))
        .await
        .unwrap();
    index
        .record(&object("b", Tier::Cache, 20, 2, FetchPriority::Catalog))
        .await
        .unwrap();
    index
        .record(&object("c", Tier::Pinned, 30, 50, FetchPriority::Library))
        .await
        .unwrap();
    let mut d = object("d", Tier::Cache, 5, 1, FetchPriority::Visible);
    d.asset = Some(asset);
    index.record(&d).await.unwrap();
    let u = index.usage().await.unwrap();
    assert_eq!((u.total, u.pinned, u.cache, u.objects), (65, 30, 35, 4));
    index.set_asset_tier(asset, Tier::Pinned).await.unwrap();
    assert_eq!(index.usage().await.unwrap().pinned, 35);
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let order: Vec<String> = index
        .eviction_candidates(None, now, 10)
        .await
        .unwrap()
        .into_iter()
        .map(|o| o.key)
        .collect();
    let day = now.date_naive();
    let ops = OpCounts {
        puts: 2,
        egress_bytes: 100,
        ..OpCounts::default()
    };
    index.bump_ops(day, ops).await.unwrap();
    let total = index.bump_ops(day, ops).await.unwrap();
    assert_eq!((total.puts, total.egress_bytes), (4, 200));
    assert_eq!(index.ops(day).await.unwrap(), total);
    index.remove(&["a".into()]).await.unwrap();
    assert!(index.get("a").await.unwrap().is_none());
    order
}

#[tokio::test]
async fn pg_index_matches_memory_index() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let pg = exercise(Arc::new(PgObjectIndex::new(pool))).await;
    let mem = exercise(Arc::new(MemObjectIndex::default())).await;
    // b: (2h+1s)*4 > a: (5h+1s)*1; pinned c and d never appear
    assert_eq!(pg, vec!["b".to_string(), "a".to_string()]);
    assert_eq!(pg, mem);
}

#[tokio::test]
async fn pg_asset_repo_round_trips() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    let repo = PgAssetRepo::new(pool);
    let mut a = AssetRecord {
        id: Uuid::now_v7(),
        kind: AssetKind::Cover,
        source_url: Some("https://image.tmdb.org/t/p/w780/x.jpg".into()),
        connector: Some("tmdb".into()),
        hash: None,
        width: None,
        height: None,
        thumbhash: None,
        dominant_color: None,
        pins: 0,
        owner: None,
        priority: FetchPriority::Library,
        failed: false,
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    };
    repo.upsert(&a).await.unwrap();
    a.hash = Some("abc".into());
    a.thumbhash = Some(vec![1, 2, 3]);
    a.width = Some(780);
    repo.upsert(&a).await.unwrap();
    assert_eq!(repo.get(a.id).await.unwrap().unwrap(), a);
    assert_eq!(repo.by_hash("abc").await.unwrap().unwrap().id, a.id);
    assert_eq!(
        repo.by_source("https://image.tmdb.org/t/p/w780/x.jpg")
            .await
            .unwrap()
            .unwrap()
            .id,
        a.id
    );
}
