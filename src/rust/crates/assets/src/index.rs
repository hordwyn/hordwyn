//! Accounting for every stored object (D-8): what is stored, how big, which tier, when it was
//! last read. The budget decides from this index; nothing is ever HEAD-ed on the backend.

use std::collections::HashMap;
use std::sync::Mutex;

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::FetchPriority;

/// What an object is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectClass {
    /// A derived image variant.
    Variant,
    /// A kept original image (`keep_originals`).
    Original,
    /// A raw provider response of the HTTP cache.
    HttpCache,
    /// A user upload (play photo, avatar).
    Upload,
}

impl ObjectClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Variant => "variant",
            Self::Original => "original",
            Self::HttpCache => "http_cache",
            Self::Upload => "upload",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Variant, Self::Original, Self::HttpCache, Self::Upload]
            .into_iter()
            .find(|c| c.as_str() == s)
    }
}

/// Pinned objects are never evicted; cache objects are (D-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Pinned,
    Cache,
}

impl Tier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pinned => "pinned",
            Self::Cache => "cache",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pinned" => Some(Self::Pinned),
            "cache" => Some(Self::Cache),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredObject {
    pub key: String,
    pub class: ObjectClass,
    pub tier: Tier,
    pub bytes: u64,
    /// Uploader, for the per-user upload quota.
    pub owner: Option<Uuid>,
    /// Asset this object belongs to (variants, originals).
    pub asset: Option<Uuid>,
    /// Why the bytes were fetched; weights eviction order.
    pub priority: FetchPriority,
    pub last_accessed: DateTime<Utc>,
}

/// Eviction weight: higher = evicted earlier at equal age (D-8 "weighted by tier").
#[must_use]
pub const fn eviction_weight(p: FetchPriority) -> u64 {
    match p {
        FetchPriority::Visible | FetchPriority::Library => 1,
        FetchPriority::Related => 2,
        FetchPriority::Catalog => 4,
    }
}

/// Eviction score of an object at `now`: age in seconds times its weight (plus one so that
/// fresh objects still order by weight).
#[must_use]
pub fn eviction_score(o: &StoredObject, now: DateTime<Utc>) -> u64 {
    let age = u64::try_from((now - o.last_accessed).num_seconds().max(0)).unwrap_or(0);
    (age + 1).saturating_mul(eviction_weight(o.priority))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Usage {
    pub total: u64,
    pub pinned: u64,
    pub cache: u64,
    pub http_cache: u64,
    pub objects: u64,
}

/// S3 operations of one UTC day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OpCounts {
    pub puts: u64,
    pub gets: u64,
    pub deletes: u64,
    pub egress_bytes: u64,
}

impl OpCounts {
    #[must_use]
    pub fn plus(self, o: Self) -> Self {
        Self {
            puts: self.puts + o.puts,
            gets: self.gets + o.gets,
            deletes: self.deletes + o.deletes,
            egress_bytes: self.egress_bytes + o.egress_bytes,
        }
    }
}

#[async_trait]
pub trait ObjectIndex: Send + Sync {
    /// Insert or replace by key.
    async fn record(&self, o: &StoredObject) -> Result<(), PortError>;
    async fn get(&self, key: &str) -> Result<Option<StoredObject>, PortError>;
    async fn touch(&self, key: &str, at: DateTime<Utc>) -> Result<(), PortError>;
    async fn remove(&self, keys: &[String]) -> Result<(), PortError>;
    async fn usage(&self) -> Result<Usage, PortError>;
    async fn upload_bytes(&self, owner: Uuid) -> Result<u64, PortError>;
    /// Cache-tier objects, most evictable first, optionally of one class only.
    async fn eviction_candidates(
        &self,
        class: Option<ObjectClass>,
        now: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<StoredObject>, PortError>;
    /// Move every object of `asset` to `tier` (pin on library add, unpin on removal).
    async fn set_asset_tier(&self, asset: Uuid, tier: Tier) -> Result<(), PortError>;
    /// Add to the counters of `day` and return the new totals.
    async fn bump_ops(&self, day: NaiveDate, delta: OpCounts) -> Result<OpCounts, PortError>;
    async fn ops(&self, day: NaiveDate) -> Result<OpCounts, PortError>;
}

/// In-memory index for tests and the memory backend.
#[derive(Default)]
pub struct MemObjectIndex {
    objects: Mutex<HashMap<String, StoredObject>>,
    ops: Mutex<HashMap<NaiveDate, OpCounts>>,
}

impl MemObjectIndex {
    /// Every recorded object (tests).
    ///
    /// # Panics
    /// If the lock is poisoned.
    #[must_use]
    pub fn all(&self) -> Vec<StoredObject> {
        self.objects
            .lock()
            .expect("index")
            .values()
            .cloned()
            .collect()
    }
}

#[async_trait]
impl ObjectIndex for MemObjectIndex {
    async fn record(&self, o: &StoredObject) -> Result<(), PortError> {
        self.objects
            .lock()
            .expect("index")
            .insert(o.key.clone(), o.clone());
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<StoredObject>, PortError> {
        Ok(self.objects.lock().expect("index").get(key).cloned())
    }

    async fn touch(&self, key: &str, at: DateTime<Utc>) -> Result<(), PortError> {
        if let Some(o) = self.objects.lock().expect("index").get_mut(key) {
            o.last_accessed = at;
        }
        Ok(())
    }

    async fn remove(&self, keys: &[String]) -> Result<(), PortError> {
        let mut m = self.objects.lock().expect("index");
        for k in keys {
            m.remove(k);
        }
        Ok(())
    }

    async fn usage(&self) -> Result<Usage, PortError> {
        let m = self.objects.lock().expect("index");
        let mut u = Usage::default();
        for o in m.values() {
            u.total += o.bytes;
            u.objects += 1;
            match o.tier {
                Tier::Pinned => u.pinned += o.bytes,
                Tier::Cache => u.cache += o.bytes,
            }
            if o.class == ObjectClass::HttpCache {
                u.http_cache += o.bytes;
            }
        }
        Ok(u)
    }

    async fn upload_bytes(&self, owner: Uuid) -> Result<u64, PortError> {
        Ok(self
            .objects
            .lock()
            .expect("index")
            .values()
            .filter(|o| o.class == ObjectClass::Upload && o.owner == Some(owner))
            .map(|o| o.bytes)
            .sum())
    }

    async fn eviction_candidates(
        &self,
        class: Option<ObjectClass>,
        now: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<StoredObject>, PortError> {
        let mut v: Vec<StoredObject> = self
            .objects
            .lock()
            .expect("index")
            .values()
            .filter(|o| o.tier == Tier::Cache && class.is_none_or(|c| c == o.class))
            .cloned()
            .collect();
        v.sort_by(|a, b| {
            eviction_score(b, now)
                .cmp(&eviction_score(a, now))
                .then_with(|| a.key.cmp(&b.key))
        });
        v.truncate(limit);
        Ok(v)
    }

    async fn set_asset_tier(&self, asset: Uuid, tier: Tier) -> Result<(), PortError> {
        for o in self.objects.lock().expect("index").values_mut() {
            if o.asset == Some(asset) {
                o.tier = tier;
            }
        }
        Ok(())
    }

    async fn bump_ops(&self, day: NaiveDate, delta: OpCounts) -> Result<OpCounts, PortError> {
        let mut m = self.ops.lock().expect("ops");
        let e = m.entry(day).or_default();
        *e = e.plus(delta);
        Ok(*e)
    }

    async fn ops(&self, day: NaiveDate) -> Result<OpCounts, PortError> {
        Ok(self
            .ops
            .lock()
            .expect("ops")
            .get(&day)
            .copied()
            .unwrap_or_default())
    }
}
