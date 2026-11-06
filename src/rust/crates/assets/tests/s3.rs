//! S3 backend against a real S3-compatible server (versitygw in compose or `cargo xtask
//! services up`). Skipped with a message unless `TEST_S3_ENDPOINT` is set.

use std::sync::Arc;

use assets::{
    Backend, FetchPriority, MemObjectIndex, NewObject, ObjectClass, ReadOutcome, StorageBudget,
    StorageConfig, Tier, WriteOutcome,
};
use testkit::fakes::{FixedClock, RecordingEvents};

fn s3_config() -> Option<StorageConfig> {
    let Ok(endpoint) = std::env::var("TEST_S3_ENDPOINT") else {
        eprintln!(
            "skipping S3 test: TEST_S3_ENDPOINT not set (start versitygw: `cargo xtask services up`)"
        );
        return None;
    };
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let mut c = StorageConfig {
        backend: Backend::S3,
        ..StorageConfig::default()
    };
    c.s3.endpoint = Some(endpoint);
    c.s3.bucket = env("TEST_S3_BUCKET", "assets");
    c.s3.access_key_id = Some(env("TEST_S3_ACCESS_KEY", "test"));
    c.s3.secret_access_key = Some(env("TEST_S3_SECRET_KEY", "testsecret"));
    c.s3.root = format!("/t-{}", uuid::Uuid::now_v7().simple());
    Some(c)
}

#[tokio::test]
async fn s3_round_trip_with_counters() {
    let Some(cfg) = s3_config() else {
        return;
    };
    let index = Arc::new(MemObjectIndex::default());
    let budget = StorageBudget::open(
        cfg,
        index.clone(),
        Arc::new(FixedClock::default()),
        Arc::new(RecordingEvents::default()),
    )
    .unwrap();
    let obj = NewObject {
        key: "img/abc/card.webp".into(),
        class: ObjectClass::Variant,
        tier: Tier::Cache,
        owner: None,
        asset: None,
        priority: FetchPriority::Visible,
    };
    assert_eq!(
        budget.write(obj.clone(), b"hello".to_vec()).await.unwrap(),
        WriteOutcome::Stored
    );
    // known key: no second PUT
    assert_eq!(
        budget.write(obj, b"hello".to_vec()).await.unwrap(),
        WriteOutcome::AlreadyStored
    );
    assert_eq!(
        budget.read("img/abc/card.webp").await.unwrap(),
        ReadOutcome::Bytes(b"hello".to_vec())
    );
    let report = budget.report().await.unwrap();
    assert_eq!(report.s3_ops_today.puts, 1);
    assert_eq!(report.s3_ops_today.gets, 1);
    assert_eq!(report.s3_ops_today.egress_bytes, 5);
    budget.remove(&["img/abc/card.webp".into()]).await.unwrap();
    assert_eq!(
        budget.read("img/abc/card.webp").await.unwrap(),
        ReadOutcome::Missing
    );
}
