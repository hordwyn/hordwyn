//! The storage budget (D-8): every byte written by the server goes through here.
//!
//! Admission per write: dedupe by key (no HEAD, no second PUT) -> S3 daily PUT limit ->
//! per-user upload quota -> HTTP cache limit (evicts old responses to make room) -> hard cap
//! and minimum free disk (pinned writes may evict cache-tier objects synchronously, cache-tier
//! writes are refused so callers redirect instead). Above the high watermark an eviction job
//! is queued; eviction removes cache-tier objects by weighted LRU down to the low watermark.
//! Admission is serialised per process so concurrent writes cannot overshoot the cap together.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use app::ports::{AppEvent, BlobStore, Clock, EventBus, JobQueue, Lane, NewJob, PortError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::backend::{BackendError, RawStore};
use crate::config::{Backend, FetchPriority, StorageConfig};
use crate::index::{ObjectClass, ObjectIndex, OpCounts, StoredObject, Tier, Usage};

/// Job kind that runs [`StorageBudget::evict`].
pub const EVICT_JOB: &str = "storage.evict";

/// Free and total bytes of the volume holding the fs backend.
pub trait DiskSpace: Send + Sync {
    /// `(available, total)`, or `None` if unknown (S3, memory).
    fn space(&self) -> Option<(u64, u64)>;
}

/// The real volume, via `statvfs` (fs4).
pub struct VolumeSpace(pub std::path::PathBuf);

impl DiskSpace for VolumeSpace {
    fn space(&self) -> Option<(u64, u64)> {
        let avail = fs4::available_space(&self.0).ok()?;
        let total = fs4::total_space(&self.0).ok()?;
        Some((avail, total))
    }
}

/// No disk constraint (S3, memory, most tests).
pub struct NoDisk;

impl DiskSpace for NoDisk {
    fn space(&self) -> Option<(u64, u64)> {
        None
    }
}

/// Why a write was not stored. The caller degrades (redirect, skip), except for
/// [`Refusal::UploadQuota`], which is the user's own limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    HardCap,
    MinFreeDisk,
    HttpCacheFull,
    UploadQuota { used: u64, quota: u64 },
    S3PutLimit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Stored,
    /// Same key already stored (content-addressed): nothing was written.
    AlreadyStored,
    Refused(Refusal),
}

/// Outcome of reading an object for serving it to a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    Bytes(Vec<u8>),
    Missing,
    /// S3 daily GET or egress limit reached: redirect instead of proxying.
    OverLimit,
}

#[derive(Debug, Clone)]
pub struct NewObject {
    pub key: String,
    pub class: ObjectClass,
    pub tier: Tier,
    pub owner: Option<Uuid>,
    pub asset: Option<Uuid>,
    pub priority: FetchPriority,
}

#[derive(Debug, thiserror::Error)]
pub enum BudgetError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error("storage index: {0}")]
    Index(#[from] PortError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct EvictionReport {
    pub objects: u64,
    pub bytes: u64,
    pub before: u64,
    pub after: u64,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StorageReport {
    pub backend: Backend,
    pub usage: Usage,
    pub max_total_bytes: u64,
    pub high_watermark_bytes: u64,
    pub low_watermark_bytes: u64,
    /// Cap after the free-disk rule (fs).
    pub effective_cap: u64,
    pub disk_available: Option<u64>,
    pub disk_total: Option<u64>,
    pub http_cache_max_bytes: u64,
    pub s3_ops_today: OpCounts,
    pub at_hard_cap: bool,
    /// What eviction would remove right now.
    pub eviction: EvictionReport,
}

pub struct StorageBudget {
    cfg: StorageConfig,
    raw: RawStore,
    index: Arc<dyn ObjectIndex>,
    clock: Arc<dyn Clock>,
    disk: Arc<dyn DiskSpace>,
    events: Arc<dyn EventBus>,
    jobs: Option<Arc<dyn JobQueue>>,
    admission: Mutex<()>,
    capped: AtomicBool,
    above_high: AtomicBool,
    /// Count and limit operations per day (S3).
    metered: bool,
}

/// Bytes still needed so that `have <= want`.
fn excess(have: u64, want: u64) -> u64 {
    have.saturating_sub(want)
}

impl StorageBudget {
    /// Open the configured backend. The fs backend also watches free disk space.
    ///
    /// # Errors
    /// Invalid configuration or an unreachable backend.
    pub fn open(
        cfg: StorageConfig,
        index: Arc<dyn ObjectIndex>,
        clock: Arc<dyn Clock>,
        events: Arc<dyn EventBus>,
    ) -> Result<Self, BackendError> {
        cfg.validate().map_err(BackendError)?;
        let raw = RawStore::open(&cfg)?;
        let cfg_backend = cfg.backend;
        let disk: Arc<dyn DiskSpace> = match cfg.backend {
            Backend::Fs => Arc::new(VolumeSpace(cfg.fs_root.clone().into())),
            Backend::S3 | Backend::Memory => Arc::new(NoDisk),
        };
        Ok(Self {
            cfg,
            raw,
            index,
            clock,
            disk,
            events,
            jobs: None,
            admission: Mutex::new(()),
            capped: AtomicBool::new(false),
            above_high: AtomicBool::new(false),
            metered: cfg_backend == Backend::S3,
        })
    }

    /// Queue eviction jobs here instead of only evicting when asked.
    #[must_use]
    pub fn with_jobs(mut self, jobs: Arc<dyn JobQueue>) -> Self {
        self.jobs = Some(jobs);
        self
    }

    /// Apply the S3 daily operation limits regardless of backend (tests, S3 gateways).
    #[must_use]
    pub fn with_metering(mut self, on: bool) -> Self {
        self.metered = on;
        self
    }

    /// Replace the free-space probe (tests).
    #[must_use]
    pub fn with_disk(mut self, disk: Arc<dyn DiskSpace>) -> Self {
        self.disk = disk;
        self
    }

    #[must_use]
    pub fn config(&self) -> &StorageConfig {
        &self.cfg
    }

    /// Did the last write that needed room hit the hard cap (and none succeeded since)?
    #[must_use]
    pub fn at_hard_cap(&self) -> bool {
        self.capped.load(Ordering::Relaxed)
    }

    fn now(&self) -> DateTime<Utc> {
        self.clock.now()
    }

    fn is_s3(&self) -> bool {
        self.metered
    }

    /// Cap for total bytes right now: the configured cap, lowered so the volume keeps
    /// `min_free_disk` free (fs). Returns the cap and whether the disk rule lowered it.
    fn effective_cap(&self, used: u64) -> (u64, bool) {
        let cap = self.cfg.max_total_bytes.0;
        let Some((avail, total)) = self.disk.space() else {
            return (cap, false);
        };
        let reserve = self.cfg.min_free_disk.required(total);
        let disk_cap = (used + avail).saturating_sub(reserve);
        if disk_cap < cap {
            (disk_cap, true)
        } else {
            (cap, false)
        }
    }

    /// Store `bytes` under `obj.key` if the budget allows it.
    ///
    /// # Errors
    /// Backend or index failures (budget refusals are an `Ok` outcome).
    pub async fn write(&self, obj: NewObject, bytes: Vec<u8>) -> Result<WriteOutcome, BudgetError> {
        let n = bytes.len() as u64;
        let now = self.now();
        let _guard = self.admission.lock().await;
        if self.index.get(&obj.key).await?.is_some() {
            self.index.touch(&obj.key, now).await?;
            return Ok(WriteOutcome::AlreadyStored);
        }
        if self.is_s3()
            && self.index.ops(now.date_naive()).await?.puts >= self.cfg.s3.max_put_per_day
        {
            return Ok(Self::refuse(Refusal::S3PutLimit));
        }
        if obj.class == ObjectClass::Upload {
            let owner = obj.owner.unwrap_or_default();
            let used = self.index.upload_bytes(owner).await?;
            let quota = self.cfg.per_user_upload_quota.0;
            if used + n > quota {
                return Ok(WriteOutcome::Refused(Refusal::UploadQuota { used, quota }));
            }
        }
        if obj.class == ObjectClass::HttpCache {
            let max = self.cfg.http_cache_max_bytes.0;
            let usage = self.index.usage().await?;
            let over = excess(usage.http_cache + n, max);
            if over > 0 {
                self.evict_bytes(Some(ObjectClass::HttpCache), over, false)
                    .await?;
                if self.index.usage().await?.http_cache + n > max {
                    return Ok(Self::refuse(Refusal::HttpCacheFull));
                }
            }
        }
        let mut usage = self.index.usage().await?;
        let (mut cap, mut by_disk) = self.effective_cap(usage.total);
        if usage.total + n > cap && obj.tier == Tier::Pinned {
            // Pinned data (uploads, library covers) may push out cache-tier bytes.
            self.evict_bytes(None, usage.total + n - cap, false).await?;
            usage = self.index.usage().await?;
            (cap, by_disk) = self.effective_cap(usage.total);
        }
        if usage.total + n > cap {
            let reason = if by_disk {
                Refusal::MinFreeDisk
            } else {
                Refusal::HardCap
            };
            self.hit_cap(reason, usage.total, cap).await;
            return Ok(Self::refuse(reason));
        }

        self.raw.write(&obj.key, bytes).await?;
        self.index
            .record(&StoredObject {
                key: obj.key,
                class: obj.class,
                tier: obj.tier,
                bytes: n,
                owner: obj.owner,
                asset: obj.asset,
                priority: obj.priority,
                last_accessed: now,
            })
            .await?;
        if self.is_s3() {
            self.index
                .bump_ops(
                    now.date_naive(),
                    OpCounts {
                        puts: 1,
                        ..OpCounts::default()
                    },
                )
                .await?;
        }
        self.capped.store(false, Ordering::Relaxed);
        if usage.total + n > self.cfg.high_bytes() {
            self.above_high_watermark(usage.total + n).await;
        }
        Ok(WriteOutcome::Stored)
    }

    fn refuse(r: Refusal) -> WriteOutcome {
        tracing::info!(target: "metrics", metric = "storage.refused", reason = ?r);
        WriteOutcome::Refused(r)
    }

    /// Hard cap reached: metric every time, admin event once per episode (D-8).
    async fn hit_cap(&self, reason: Refusal, used: u64, cap: u64) {
        tracing::warn!(target: "metrics", metric = "storage.hard_cap", used, cap, ?reason);
        if !self.capped.swap(true, Ordering::Relaxed) {
            self.events
                .publish(AppEvent {
                    topic: "admin.storage.hard_cap".into(),
                    entity: None,
                    audience: vec![],
                    payload: serde_json::json!({ "used": used, "cap": cap, "reason": reason }),
                })
                .await;
        }
    }

    async fn above_high_watermark(&self, used: u64) {
        if !self.above_high.swap(true, Ordering::Relaxed) {
            self.events
                .publish(AppEvent {
                    topic: "admin.storage.high_watermark".into(),
                    entity: None,
                    audience: vec![],
                    payload: serde_json::json!({ "used": used, "high": self.cfg.high_bytes() }),
                })
                .await;
        }
        if let Some(jobs) = &self.jobs {
            let job = NewJob {
                lane: Lane::BackgroundRefresh,
                kind: EVICT_JOB.into(),
                payload: serde_json::Value::Null,
                dedupe_key: Some(EVICT_JOB.into()),
                priority: 10,
                run_at: None,
            };
            if let Err(e) = jobs.enqueue(job).await {
                tracing::warn!(error = %e, "could not queue storage eviction");
            }
        }
    }

    /// Read an object for serving. Counts S3 GETs and egress; past the daily limits it
    /// answers [`ReadOutcome::OverLimit`] so the caller redirects.
    ///
    /// # Errors
    /// Backend or index failures.
    pub async fn read(&self, key: &str) -> Result<ReadOutcome, BudgetError> {
        let now = self.now();
        let Some(obj) = self.index.get(key).await? else {
            return Ok(ReadOutcome::Missing);
        };
        if self.is_s3() {
            let ops = self.index.ops(now.date_naive()).await?;
            if ops.gets >= self.cfg.s3.max_get_per_day
                || ops.egress_bytes + obj.bytes > self.cfg.s3.max_egress_bytes_per_day.0
            {
                return Ok(ReadOutcome::OverLimit);
            }
        }
        let Some(bytes) = self.raw.read(key).await? else {
            // Lost on the backend: forget it so it can be regenerated.
            self.index.remove(&[key.to_string()]).await?;
            return Ok(ReadOutcome::Missing);
        };
        // Coarse LRU: one index write per object per hour at most.
        if now - obj.last_accessed > chrono::Duration::hours(1) {
            self.index.touch(key, now).await?;
        }
        if self.is_s3() {
            self.index
                .bump_ops(
                    now.date_naive(),
                    OpCounts {
                        gets: 1,
                        egress_bytes: bytes.len() as u64,
                        ..OpCounts::default()
                    },
                )
                .await?;
        }
        Ok(ReadOutcome::Bytes(bytes))
    }

    /// Remove objects (and their accounting).
    ///
    /// # Errors
    /// Backend or index failures.
    pub async fn remove(&self, keys: &[String]) -> Result<(), BudgetError> {
        self.raw.delete(keys).await?;
        self.index.remove(keys).await?;
        if self.is_s3() {
            self.index
                .bump_ops(
                    self.now().date_naive(),
                    OpCounts {
                        deletes: keys.len() as u64,
                        ..OpCounts::default()
                    },
                )
                .await?;
        }
        Ok(())
    }

    /// Pin or unpin every object of an asset.
    ///
    /// # Errors
    /// Index failures.
    pub async fn set_asset_tier(&self, asset: Uuid, tier: Tier) -> Result<(), BudgetError> {
        Ok(self.index.set_asset_tier(asset, tier).await?)
    }

    /// Evict cache-tier objects, most evictable first, until `need` bytes are freed or no
    /// candidates remain. With `dry_run` nothing is deleted.
    async fn evict_bytes(
        &self,
        class: Option<ObjectClass>,
        need: u64,
        dry_run: bool,
    ) -> Result<EvictionReport, BudgetError> {
        let before = self.index.usage().await?.total;
        let mut report = EvictionReport {
            before,
            after: before,
            dry_run,
            ..EvictionReport::default()
        };
        if need == 0 {
            return Ok(report);
        }
        let now = self.now();
        loop {
            // A dry run cannot delete between pages, so it looks at one large page.
            let limit = if dry_run { 1_000_000 } else { 256 };
            let candidates = self.index.eviction_candidates(class, now, limit).await?;
            if candidates.is_empty() {
                break;
            }
            let mut keys = Vec::new();
            for c in candidates {
                if report.bytes >= need {
                    break;
                }
                debug_assert_eq!(c.tier, Tier::Cache);
                report.bytes += c.bytes;
                report.objects += 1;
                keys.push(c.key);
            }
            if !dry_run && !keys.is_empty() {
                self.remove(&keys).await?;
            }
            if dry_run || report.bytes >= need {
                break;
            }
        }
        report.after = before.saturating_sub(report.bytes);
        Ok(report)
    }

    /// Bytes eviction should free now: down to the low watermark once above the high one,
    /// and enough to restore `min_free_disk` on the volume.
    fn eviction_need(&self, usage: &Usage) -> u64 {
        let mut need = 0;
        if usage.total > self.cfg.high_bytes() {
            need = excess(usage.total, self.cfg.low_bytes());
        }
        if let Some((avail, total)) = self.disk.space() {
            let reserve = self.cfg.min_free_disk.required(total);
            need = need.max(excess(reserve, avail));
        }
        need
    }

    /// The eviction job (D-8): above the high watermark, evict the cache tier down to the
    /// low watermark. Pinned objects are never touched.
    ///
    /// # Errors
    /// Backend or index failures.
    pub async fn evict(&self) -> Result<EvictionReport, BudgetError> {
        let _guard = self.admission.lock().await;
        let usage = self.index.usage().await?;
        let report = self
            .evict_bytes(None, self.eviction_need(&usage), false)
            .await?;
        if report.after <= self.cfg.high_bytes() {
            self.above_high.store(false, Ordering::Relaxed);
        }
        if report.objects > 0 {
            tracing::info!(target: "metrics", metric = "storage.evicted", objects = report.objects, bytes = report.bytes);
        }
        Ok(report)
    }

    /// Storage report for admins and `cli storage report --dry-run`.
    ///
    /// # Errors
    /// Index failures.
    pub async fn report(&self) -> Result<StorageReport, BudgetError> {
        let usage = self.index.usage().await?;
        let (cap, _) = self.effective_cap(usage.total);
        let space = self.disk.space();
        let eviction = self
            .evict_bytes(None, self.eviction_need(&usage), true)
            .await?;
        Ok(StorageReport {
            backend: self.cfg.backend,
            usage,
            max_total_bytes: self.cfg.max_total_bytes.0,
            high_watermark_bytes: self.cfg.high_bytes(),
            low_watermark_bytes: self.cfg.low_bytes(),
            effective_cap: cap,
            disk_available: space.map(|s| s.0),
            disk_total: space.map(|s| s.1),
            http_cache_max_bytes: self.cfg.http_cache_max_bytes.0,
            s3_ops_today: self.index.ops(self.now().date_naive()).await?,
            at_hard_cap: usage.total >= cap,
            eviction,
        })
    }
}

/// Classify a key written through the generic [`BlobStore`] port:
/// `http/<hash>` -> HTTP cache, `up/<user-uuid>/...` -> upload (pinned), else cache tier.
fn classify(key: &str) -> NewObject {
    let mut obj = NewObject {
        key: key.to_string(),
        class: ObjectClass::Variant,
        tier: Tier::Cache,
        owner: None,
        asset: None,
        priority: FetchPriority::Catalog,
    };
    if key.starts_with("http/") {
        obj.class = ObjectClass::HttpCache;
    } else if let Some(rest) = key.strip_prefix("up/") {
        obj.class = ObjectClass::Upload;
        obj.tier = Tier::Pinned;
        obj.priority = FetchPriority::Visible;
        obj.owner = rest.split('/').next().and_then(|u| Uuid::parse_str(u).ok());
    }
    obj
}

/// The budgeted store is the `BlobStore` every other crate gets (HTTP cache, uploads).
#[async_trait]
impl BlobStore for StorageBudget {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), PortError> {
        match self
            .write(classify(key), bytes)
            .await
            .map_err(|e| PortError(e.to_string()))?
        {
            WriteOutcome::Stored | WriteOutcome::AlreadyStored => Ok(()),
            WriteOutcome::Refused(r) => Err(PortError(format!("storage budget refused: {r:?}"))),
        }
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PortError> {
        match self.read(key).await.map_err(|e| PortError(e.to_string()))? {
            ReadOutcome::Bytes(b) => Ok(Some(b)),
            ReadOutcome::Missing | ReadOutcome::OverLimit => Ok(None),
        }
    }

    async fn delete(&self, keys: &[String]) -> Result<(), PortError> {
        self.remove(keys)
            .await
            .map_err(|e| PortError(e.to_string()))
    }
}
