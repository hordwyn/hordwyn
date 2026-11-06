//! Storage budget behaviour (D-8) against the memory backend and the in-memory index.

use std::sync::Arc;

use assets::budget::DiskSpace;
use assets::{
    Backend, ByteSize, FetchPriority, MemObjectIndex, NewObject, ObjectClass, ObjectIndex,
    ReadOutcome, Refusal, StorageBudget, StorageConfig, Tier, WriteOutcome,
};
use chrono::Duration;
use proptest::prelude::*;
use testkit::fakes::{FixedClock, MemJobs, RecordingEvents};
use uuid::Uuid;

struct Env {
    budget: StorageBudget,
    index: Arc<MemObjectIndex>,
    clock: Arc<FixedClock>,
    events: Arc<RecordingEvents>,
    jobs: Arc<MemJobs>,
}

fn cfg(max: u64) -> StorageConfig {
    StorageConfig {
        backend: Backend::Memory,
        max_total_bytes: ByteSize(max),
        high_watermark: 0.9,
        low_watermark: 0.75,
        http_cache_max_bytes: ByteSize(max / 4),
        per_user_upload_quota: ByteSize(max / 2),
        ..StorageConfig::default()
    }
}

fn env_with(c: StorageConfig) -> Env {
    let index = Arc::new(MemObjectIndex::default());
    let clock = Arc::new(FixedClock::default());
    let events = Arc::new(RecordingEvents::default());
    let jobs = Arc::new(MemJobs::default());
    let budget = StorageBudget::open(c, index.clone(), clock.clone(), events.clone())
        .unwrap()
        .with_jobs(jobs.clone());
    Env {
        budget,
        index,
        clock,
        events,
        jobs,
    }
}

fn env(max: u64) -> Env {
    env_with(cfg(max))
}

fn obj(key: &str, tier: Tier) -> NewObject {
    NewObject {
        key: key.into(),
        class: ObjectClass::Variant,
        tier,
        owner: None,
        asset: None,
        priority: FetchPriority::Catalog,
    }
}

async fn put(e: &Env, key: &str, tier: Tier, n: usize) -> WriteOutcome {
    e.budget.write(obj(key, tier), vec![7; n]).await.unwrap()
}

#[tokio::test]
async fn same_key_is_written_once() {
    let e = env(1000);
    assert_eq!(put(&e, "a", Tier::Cache, 10).await, WriteOutcome::Stored);
    assert_eq!(
        put(&e, "a", Tier::Cache, 10).await,
        WriteOutcome::AlreadyStored
    );
    assert_eq!(e.index.usage().await.unwrap().total, 10);
}

#[tokio::test]
async fn at_hard_cap_cache_writes_are_refused_and_nothing_is_stored() {
    let e = env(100);
    assert_eq!(
        put(&e, "pinned", Tier::Pinned, 95).await,
        WriteOutcome::Stored
    );
    let before = e.index.usage().await.unwrap();
    assert_eq!(
        put(&e, "new", Tier::Cache, 10).await,
        WriteOutcome::Refused(Refusal::HardCap)
    );
    assert_eq!(e.index.usage().await.unwrap(), before);
    assert_eq!(e.budget.read("new").await.unwrap(), ReadOutcome::Missing);
    assert!(e.budget.at_hard_cap());
    // admin notification once per episode, metric every time
    put(&e, "new2", Tier::Cache, 10).await;
    let caps = e
        .events
        .topics()
        .iter()
        .filter(|t| *t == "admin.storage.hard_cap")
        .count();
    assert_eq!(caps, 1);
}

#[tokio::test]
async fn pinned_writes_push_out_cache_objects() {
    let e = env(100);
    put(&e, "old", Tier::Cache, 60).await;
    assert_eq!(put(&e, "up", Tier::Pinned, 60).await, WriteOutcome::Stored);
    let u = e.index.usage().await.unwrap();
    assert_eq!((u.total, u.pinned), (60, 60));
    assert_eq!(e.budget.read("old").await.unwrap(), ReadOutcome::Missing);
}

#[tokio::test]
async fn above_high_watermark_an_eviction_job_is_queued() {
    let e = env(100);
    put(&e, "a", Tier::Cache, 50).await;
    assert!(e.jobs.jobs().is_empty(), "below the high watermark");
    put(&e, "b", Tier::Cache, 45).await;
    let jobs = e.jobs.jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, assets::EVICT_JOB);
    assert_eq!(jobs[0].dedupe_key.as_deref(), Some(assets::EVICT_JOB));
}

#[tokio::test]
async fn eviction_removes_least_recently_used_first() {
    let e = env(100);
    put(&e, "old", Tier::Cache, 30).await;
    e.clock.advance(Duration::hours(2));
    put(&e, "mid", Tier::Cache, 30).await;
    e.clock.advance(Duration::hours(2));
    put(&e, "new", Tier::Cache, 35).await;
    let r = e.budget.evict().await.unwrap();
    assert!(r.after <= 75, "{r:?}");
    assert_eq!(e.budget.read("old").await.unwrap(), ReadOutcome::Missing);
    assert!(matches!(
        e.budget.read("new").await.unwrap(),
        ReadOutcome::Bytes(_)
    ));
}

#[tokio::test]
async fn http_cache_is_limited_separately() {
    let e = env(400); // http cache max 100
    let http = |k: &str| NewObject {
        class: ObjectClass::HttpCache,
        ..obj(&format!("http/{k}"), Tier::Cache)
    };
    e.budget.write(http("a"), vec![0; 60]).await.unwrap();
    e.clock.advance(Duration::hours(2));
    e.budget.write(http("b"), vec![0; 60]).await.unwrap();
    let u = e.index.usage().await.unwrap();
    assert_eq!(u.http_cache, 60, "oldest response evicted to make room");
    assert_eq!(
        e.budget.write(http("c"), vec![0; 150]).await.unwrap(),
        WriteOutcome::Refused(Refusal::HttpCacheFull)
    );
}

#[tokio::test]
async fn uploads_respect_the_per_user_quota() {
    let e = env(1000); // quota 500
    let user = Uuid::now_v7();
    let up = |k: &str| NewObject {
        class: ObjectClass::Upload,
        owner: Some(user),
        ..obj(k, Tier::Pinned)
    };
    assert_eq!(
        e.budget.write(up("p1"), vec![0; 400]).await.unwrap(),
        WriteOutcome::Stored
    );
    assert_eq!(
        e.budget.write(up("p2"), vec![0; 200]).await.unwrap(),
        WriteOutcome::Refused(Refusal::UploadQuota {
            used: 400,
            quota: 500
        })
    );
}

struct Disk(u64, u64);
impl DiskSpace for Disk {
    fn space(&self) -> Option<(u64, u64)> {
        Some((self.0, self.1))
    }
}

#[tokio::test]
async fn min_free_disk_lowers_the_cap() {
    let c = StorageConfig {
        min_free_disk: "10%".parse().unwrap(),
        ..cfg(10_000)
    };
    let e = env_with(c);
    // volume of 1000 bytes with 150 free: only 50 may be used before 100 (10 %) remain
    let budget = e.budget.with_disk(Arc::new(Disk(150, 1000)));
    assert_eq!(
        budget
            .write(obj("a", Tier::Cache), vec![0; 60])
            .await
            .unwrap(),
        WriteOutcome::Refused(Refusal::MinFreeDisk)
    );
    assert_eq!(
        budget
            .write(obj("b", Tier::Cache), vec![0; 40])
            .await
            .unwrap(),
        WriteOutcome::Stored
    );
}

#[tokio::test]
async fn s3_daily_limits_stop_puts_and_proxying() {
    let mut c = cfg(10_000);
    c.s3.max_put_per_day = 2;
    c.s3.max_get_per_day = 1;
    let e = env_with(c);
    let budget = e.budget.with_metering(true);
    for k in ["a", "b"] {
        assert_eq!(
            budget.write(obj(k, Tier::Cache), vec![0; 5]).await.unwrap(),
            WriteOutcome::Stored
        );
    }
    assert_eq!(
        budget
            .write(obj("c", Tier::Cache), vec![0; 5])
            .await
            .unwrap(),
        WriteOutcome::Refused(Refusal::S3PutLimit)
    );
    assert!(matches!(
        budget.read("a").await.unwrap(),
        ReadOutcome::Bytes(_)
    ));
    assert_eq!(budget.read("b").await.unwrap(), ReadOutcome::OverLimit);
    // a new UTC day resets the counters
    e.clock.advance(Duration::days(1));
    assert!(matches!(
        budget.read("b").await.unwrap(),
        ReadOutcome::Bytes(_)
    ));
}

#[tokio::test]
async fn report_shows_what_a_dry_run_would_evict() {
    let e = env(100);
    put(&e, "a", Tier::Cache, 50).await;
    put(&e, "b", Tier::Pinned, 45).await;
    let r = e.budget.report().await.unwrap();
    assert!(r.eviction.dry_run);
    assert_eq!(r.eviction.bytes, 50);
    assert_eq!(r.usage.total, 95, "dry run deletes nothing");
}

// ---------------------------------------------------------------- properties

#[derive(Debug, Clone)]
struct Spec {
    bytes: u64,
    pinned: bool,
    age_hours: i64,
    priority: FetchPriority,
}

fn spec() -> impl Strategy<Value = Spec> {
    (
        1u64..400,
        any::<bool>(),
        0i64..500,
        prop_oneof![
            Just(FetchPriority::Visible),
            Just(FetchPriority::Library),
            Just(FetchPriority::Related),
            Just(FetchPriority::Catalog)
        ],
    )
        .prop_map(|(bytes, pinned, age_hours, priority)| Spec {
            bytes,
            pinned,
            age_hours,
            priority,
        })
}

/// Fill the index directly (bypassing admission) with an arbitrary mix of objects, keeping
/// the pinned total below the low watermark, then evict.
fn run_eviction(specs: Vec<Spec>) -> (Vec<String>, Vec<String>, u64, u64) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    rt.block_on(async {
        let max = 10_000;
        let e = env(max);
        let low = e.budget.config().low_bytes();
        let now = e.clock.now();
        let mut pinned_total = 0;
        let mut pinned_keys = Vec::new();
        for (i, s) in specs.into_iter().enumerate() {
            let pinned = s.pinned && pinned_total + s.bytes <= low;
            let key = format!("k{i}");
            if pinned {
                pinned_total += s.bytes;
                pinned_keys.push(key.clone());
            }
            e.index
                .record(&assets::StoredObject {
                    key,
                    class: ObjectClass::Variant,
                    tier: if pinned { Tier::Pinned } else { Tier::Cache },
                    bytes: s.bytes,
                    owner: None,
                    asset: None,
                    priority: s.priority,
                    last_accessed: now - Duration::hours(s.age_hours),
                })
                .await
                .unwrap();
        }
        let before = e.index.usage().await.unwrap().total;
        e.budget.evict().await.unwrap();
        let left: Vec<String> = e.index.all().into_iter().map(|o| o.key).collect();
        let after = e.index.usage().await.unwrap().total;
        let high = e.budget.config().high_bytes();
        (
            pinned_keys,
            left,
            after,
            if before > high { low } else { before },
        )
    })
}

use app::ports::Clock;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn eviction_never_removes_pinned_objects(specs in prop::collection::vec(spec(), 0..80)) {
        let (pinned, left, _, _) = run_eviction(specs);
        for k in pinned {
            prop_assert!(left.contains(&k), "pinned {k} was evicted");
        }
    }

    #[test]
    fn eviction_always_ends_at_or_below_the_low_watermark(specs in prop::collection::vec(spec(), 0..80)) {
        let (_, _, after, target) = run_eviction(specs);
        prop_assert!(after <= target, "after {after} > target {target}");
    }
}
