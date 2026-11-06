//! Assets (D-7): provider images and uploads, served as content-addressed variants from
//! `/assets/{hash}/{variant}.{ext}`. A missing variant answers with a redirect to the provider
//! URL and queues its generation; at the hard cap nothing new is written (D-8).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use app::ports::{HttpFetch, HttpRequest, JobQueue, Lane, NewJob, PortError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::budget::{BudgetError, NewObject, ReadOutcome, StorageBudget, WriteOutcome};
use crate::config::{FetchPriority, Format, Variant};
use crate::image::{PipelineConfig, Processed, process};
use crate::index::{ObjectClass, Tier};
use crate::pool::CpuPool;

/// Job kind that downloads and processes one asset.
pub const FETCH_JOB: &str = "assets.fetch";

/// Largest source image we download.
pub const MAX_SOURCE_BYTES: usize = 25 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Cover,
    Backdrop,
    Person,
    Avatar,
    Upload,
}

impl AssetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Backdrop => "backdrop",
            Self::Person => "person",
            Self::Avatar => "avatar",
            Self::Upload => "upload",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::Cover,
            Self::Backdrop,
            Self::Person,
            Self::Avatar,
            Self::Upload,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRecord {
    pub id: Uuid,
    pub kind: AssetKind,
    /// Provider URL (redirect target while variants are missing).
    pub source_url: Option<String>,
    /// Connector whose governed HTTP policy fetches the source.
    pub connector: Option<String>,
    /// Content hash, known once processed.
    pub hash: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub thumbhash: Option<Vec<u8>>,
    pub dominant_color: Option<String>,
    /// Library items, lists and plays referencing the asset; > 0 pins it.
    pub pins: u32,
    pub owner: Option<Uuid>,
    pub priority: FetchPriority,
    pub failed: bool,
    pub created_at: DateTime<Utc>,
}

impl AssetRecord {
    #[must_use]
    pub fn tier(&self) -> Tier {
        if self.pins > 0 || self.kind == AssetKind::Upload {
            Tier::Pinned
        } else {
            Tier::Cache
        }
    }

    /// Path of a variant, if the asset has been processed.
    #[must_use]
    pub fn path(&self, variant: Variant, format: Format) -> Option<String> {
        self.hash
            .as_ref()
            .map(|h| format!("/assets/{h}/{}.{}", variant.as_str(), format.ext()))
    }
}

#[async_trait]
pub trait AssetRepo: Send + Sync {
    async fn upsert(&self, a: &AssetRecord) -> Result<(), PortError>;
    async fn get(&self, id: Uuid) -> Result<Option<AssetRecord>, PortError>;
    async fn by_hash(&self, hash: &str) -> Result<Option<AssetRecord>, PortError>;
    async fn by_source(&self, url: &str) -> Result<Option<AssetRecord>, PortError>;
}

#[derive(Default)]
pub struct MemAssetRepo {
    rows: Mutex<HashMap<Uuid, AssetRecord>>,
}

#[async_trait]
impl AssetRepo for MemAssetRepo {
    async fn upsert(&self, a: &AssetRecord) -> Result<(), PortError> {
        self.rows.lock().expect("assets").insert(a.id, a.clone());
        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<AssetRecord>, PortError> {
        Ok(self.rows.lock().expect("assets").get(&id).cloned())
    }

    async fn by_hash(&self, hash: &str) -> Result<Option<AssetRecord>, PortError> {
        Ok(self
            .rows
            .lock()
            .expect("assets")
            .values()
            .find(|a| a.hash.as_deref() == Some(hash))
            .cloned())
    }

    async fn by_source(&self, url: &str) -> Result<Option<AssetRecord>, PortError> {
        Ok(self
            .rows
            .lock()
            .expect("assets")
            .values()
            .find(|a| a.source_url.as_deref() == Some(url))
            .cloned())
    }
}

/// What `/assets/{hash}/{variant}.{ext}` answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Served {
    Bytes {
        mime: &'static str,
        bytes: Vec<u8>,
    },
    /// 302 to the provider's original.
    Redirect(String),
    NotFound,
}

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error(transparent)]
    Budget(#[from] BudgetError),
    #[error("assets: {0}")]
    Port(#[from] PortError),
    #[error("unknown asset {0}")]
    Unknown(Uuid),
    #[error("fetch: {0}")]
    Fetch(String),
    #[error("image: {0}")]
    Image(String),
    #[error("upload quota exceeded ({used} of {quota} bytes used)")]
    Quota { used: u64, quota: u64 },
    #[error("storage is full")]
    Full,
}

fn lane_for(p: FetchPriority) -> Lane {
    match p {
        FetchPriority::Visible => Lane::Interactive,
        FetchPriority::Library => Lane::BackgroundRefresh,
        FetchPriority::Related | FetchPriority::Catalog => Lane::Prefetch,
    }
}

fn object_key(hash: &str, variant: Variant, format: Format) -> String {
    format!("img/{hash}/{}.{}", variant.as_str(), format.ext())
}

pub struct AssetService {
    budget: Arc<StorageBudget>,
    repo: Arc<dyn AssetRepo>,
    http: Arc<dyn HttpFetch>,
    jobs: Arc<dyn JobQueue>,
    pool: CpuPool,
}

impl AssetService {
    #[must_use]
    pub fn new(
        budget: Arc<StorageBudget>,
        repo: Arc<dyn AssetRepo>,
        http: Arc<dyn HttpFetch>,
        jobs: Arc<dyn JobQueue>,
        pool: CpuPool,
    ) -> Self {
        Self {
            budget,
            repo,
            http,
            jobs,
            pool,
        }
    }

    #[must_use]
    pub fn budget(&self) -> &Arc<StorageBudget> {
        &self.budget
    }

    fn pipeline(&self) -> PipelineConfig {
        let c = self.budget.config();
        PipelineConfig::new(c.variants.clone(), c.formats.clone(), c.max_source_pixels)
    }

    /// Register a provider image (idempotent per URL) and queue its download if the fetch
    /// policy allows it for `priority`. Never waits for the download.
    ///
    /// # Errors
    /// Repository failures.
    pub async fn register(
        &self,
        kind: AssetKind,
        source_url: &str,
        connector: &str,
        priority: FetchPriority,
    ) -> Result<AssetRecord, AssetError> {
        let record = if let Some(mut a) = self.repo.by_source(source_url).await? {
            if priority < a.priority {
                a.priority = priority;
                self.repo.upsert(&a).await?;
            }
            a
        } else {
            let a = AssetRecord {
                id: Uuid::now_v7(),
                kind,
                source_url: Some(source_url.to_string()),
                connector: Some(connector.to_string()),
                hash: None,
                width: None,
                height: None,
                thumbhash: None,
                dominant_color: None,
                pins: 0,
                owner: None,
                priority,
                failed: false,
                created_at: Utc::now(),
            };
            self.repo.upsert(&a).await?;
            a
        };
        if record.hash.is_none() && self.budget.config().fetch_policy.allows(priority) {
            self.queue_fetch(record.id, priority).await;
        }
        Ok(record)
    }

    async fn queue_fetch(&self, id: Uuid, priority: FetchPriority) {
        let job = NewJob {
            lane: lane_for(priority),
            kind: FETCH_JOB.into(),
            payload: serde_json::json!({ "asset_id": id }),
            dedupe_key: Some(format!("{FETCH_JOB}:{id}")),
            priority: 0,
            run_at: None,
        };
        if let Err(e) = self.jobs.enqueue(job).await {
            tracing::warn!(error = %e, asset = %id, "could not queue asset fetch");
        }
    }

    /// The fetch job: download through the governed client, process on the CPU pool, store
    /// the variants through the budget. Refused variants are simply not stored (redirects).
    ///
    /// # Errors
    /// Unknown asset, download or decode failures (the job retries), storage errors.
    pub async fn fetch(&self, id: Uuid) -> Result<AssetRecord, AssetError> {
        let mut a = self.repo.get(id).await?.ok_or(AssetError::Unknown(id))?;
        let (Some(url), Some(connector)) = (a.source_url.clone(), a.connector.clone()) else {
            return Ok(a);
        };
        let resp = self
            .http
            .fetch(HttpRequest {
                connector,
                op: "image".into(),
                method: "GET".into(),
                url,
                headers: vec![],
                body: None,
                lane: lane_for(a.priority),
            })
            .await
            .map_err(|e| AssetError::Fetch(e.to_string()))?;
        if !(200..300).contains(&resp.status) {
            if resp.status == 404 {
                a.failed = true;
                self.repo.upsert(&a).await?;
                return Ok(a);
            }
            return Err(AssetError::Fetch(format!("status {}", resp.status)));
        }
        if resp.body.len() > MAX_SOURCE_BYTES {
            a.failed = true;
            self.repo.upsert(&a).await?;
            return Ok(a);
        }
        let processed = self.process(resp.body.clone()).await?;
        self.store(&mut a, &processed, Some(resp.body)).await?;
        Ok(a)
    }

    async fn process(&self, bytes: Vec<u8>) -> Result<Processed, AssetError> {
        let cfg = self.pipeline();
        self.pool
            .run(move || process(&bytes, &cfg))
            .await
            .map_err(|e| AssetError::Image(e.to_string()))?
            .map_err(|e| AssetError::Image(e.to_string()))
    }

    async fn store(
        &self,
        a: &mut AssetRecord,
        p: &Processed,
        original: Option<Vec<u8>>,
    ) -> Result<bool, AssetError> {
        let tier = a.tier();
        let mut all_stored = true;
        let write = |key: String, class: ObjectClass, bytes: Vec<u8>| {
            let obj = NewObject {
                key,
                class,
                tier,
                owner: a.owner,
                asset: Some(a.id),
                priority: a.priority,
            };
            let budget = self.budget.clone();
            async move { budget.write(obj, bytes).await }
        };
        for v in &p.variants {
            let out = write(
                object_key(&p.hash, v.variant, v.format),
                if a.kind == AssetKind::Upload {
                    ObjectClass::Upload
                } else {
                    ObjectClass::Variant
                },
                v.bytes.clone(),
            )
            .await?;
            match out {
                WriteOutcome::Stored | WriteOutcome::AlreadyStored => {}
                WriteOutcome::Refused(crate::budget::Refusal::UploadQuota { used, quota }) => {
                    return Err(AssetError::Quota { used, quota });
                }
                WriteOutcome::Refused(_) => all_stored = false,
            }
        }
        if self.budget.config().keep_originals
            && let Some(orig) = original
        {
            let _ = write(format!("orig/{}", p.hash), ObjectClass::Original, orig).await?;
        }
        a.hash = Some(p.hash.clone());
        a.width = Some(p.width);
        a.height = Some(p.height);
        a.thumbhash = Some(p.thumbhash.clone());
        a.dominant_color = Some(p.dominant_color.clone());
        a.failed = false;
        self.repo.upsert(a).await?;
        Ok(all_stored)
    }

    /// Store a user upload (pinned, counted against the uploader's quota).
    ///
    /// # Errors
    /// Quota exceeded, storage full, or an undecodable image.
    pub async fn upload(&self, owner: Uuid, bytes: Vec<u8>) -> Result<AssetRecord, AssetError> {
        let processed = self.process(bytes).await?;
        let mut a = match self.repo.by_hash(&processed.hash).await? {
            // Same content already stored (content-addressed): reference it, store nothing.
            Some(mut a) => {
                a.pins += 1;
                self.repo.upsert(&a).await?;
                self.budget.set_asset_tier(a.id, Tier::Pinned).await?;
                return Ok(a);
            }
            None => AssetRecord {
                id: Uuid::now_v7(),
                kind: AssetKind::Upload,
                source_url: None,
                connector: None,
                hash: None,
                width: None,
                height: None,
                thumbhash: None,
                dominant_color: None,
                pins: 1,
                owner: Some(owner),
                priority: FetchPriority::Visible,
                failed: false,
                created_at: Utc::now(),
            },
        };
        if self.store(&mut a, &processed, None).await? {
            Ok(a)
        } else {
            Err(AssetError::Full)
        }
    }

    /// A library item, list or play now references the asset: pin it (never evicted).
    ///
    /// # Errors
    /// Unknown asset or storage errors.
    pub async fn pin(&self, id: Uuid) -> Result<(), AssetError> {
        let mut a = self.repo.get(id).await?.ok_or(AssetError::Unknown(id))?;
        a.pins += 1;
        if a.priority > FetchPriority::Library {
            a.priority = FetchPriority::Library;
        }
        self.repo.upsert(&a).await?;
        self.budget.set_asset_tier(id, Tier::Pinned).await?;
        if a.hash.is_none() && self.budget.config().fetch_policy.allows(a.priority) {
            self.queue_fetch(id, a.priority).await;
        }
        Ok(())
    }

    /// A reference went away; the last one moves the asset back to the cache tier.
    ///
    /// # Errors
    /// Unknown asset or storage errors.
    pub async fn unpin(&self, id: Uuid) -> Result<(), AssetError> {
        let mut a = self.repo.get(id).await?.ok_or(AssetError::Unknown(id))?;
        a.pins = a.pins.saturating_sub(1);
        self.repo.upsert(&a).await?;
        self.budget.set_asset_tier(id, a.tier()).await?;
        Ok(())
    }

    /// Answer `/assets/{hash}/{variant}.{ext}`.
    ///
    /// # Errors
    /// Storage failures.
    pub async fn serve(&self, hash: &str, variant: &str, ext: &str) -> Result<Served, AssetError> {
        let cfg = self.budget.config();
        let (Some(variant), Some(format)) = (Variant::parse(variant), Format::from_ext(ext)) else {
            return Ok(Served::NotFound);
        };
        if !cfg.variants.contains(&variant) || !cfg.formats.contains(&format) {
            return Ok(Served::NotFound);
        }
        let Some(a) = self.repo.by_hash(hash).await? else {
            return Ok(Served::NotFound);
        };
        match self.budget.read(&object_key(hash, variant, format)).await? {
            ReadOutcome::Bytes(bytes) => Ok(Served::Bytes {
                mime: format.mime(),
                bytes,
            }),
            outcome => {
                if outcome == ReadOutcome::Missing && !self.budget.at_hard_cap() {
                    self.queue_fetch(a.id, FetchPriority::Visible).await;
                }
                Ok(a.source_url.map_or(Served::NotFound, Served::Redirect))
            }
        }
    }

    /// # Errors
    /// Repository failures.
    pub async fn get(&self, id: Uuid) -> Result<Option<AssetRecord>, AssetError> {
        Ok(self.repo.get(id).await?)
    }
}
