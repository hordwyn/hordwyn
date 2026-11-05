//! HTTP cache (D-6.5): response bodies are stored content-addressed (blake3) in the
//! `BlobStore` port, so every byte goes through the storage budget once `assets` provides the
//! budgeted store (D-8); metadata (validators, freshness) sits in an [`HttpCacheIndex`].
//! Recorded bodies double as test fixtures and let parsers re-run without refetching.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use app::ports::{BlobStore, PortError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    /// blake3 hex of the body; the blob key is `http/<hash>`.
    pub body_hash: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub fresh_until: DateTime<Utc>,
    pub size: u64,
}

impl CacheEntry {
    #[must_use]
    pub fn is_fresh(&self, now: DateTime<Utc>) -> bool {
        now < self.fresh_until
    }

    #[must_use]
    pub fn can_revalidate(&self) -> bool {
        self.etag.is_some() || self.last_modified.is_some()
    }
}

/// Where cache metadata lives (memory in tests, Postgres in production).
#[async_trait]
pub trait HttpCacheIndex: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>, PortError>;
    async fn put(&self, key: &str, entry: CacheEntry) -> Result<(), PortError>;
}

#[derive(Default)]
pub struct MemCacheIndex {
    entries: Mutex<HashMap<String, CacheEntry>>,
}

#[async_trait]
impl HttpCacheIndex for MemCacheIndex {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>, PortError> {
        Ok(self.entries.lock().expect("lock").get(key).cloned())
    }

    async fn put(&self, key: &str, entry: CacheEntry) -> Result<(), PortError> {
        self.entries
            .lock()
            .expect("lock")
            .insert(key.to_string(), entry);
        Ok(())
    }
}

/// Index + blob store.
#[derive(Clone)]
pub struct HttpCache {
    pub index: Arc<dyn HttpCacheIndex>,
    pub blobs: Arc<dyn BlobStore>,
}

/// Cache key of a request (method + URL; credentials are never part of it).
#[must_use]
pub fn cache_key(method: &str, url: &str) -> String {
    blake3::hash(format!("{method} {url}").as_bytes())
        .to_hex()
        .to_string()
}

#[must_use]
pub fn body_key(hash: &str) -> String {
    format!("http/{hash}")
}

impl HttpCache {
    /// Look up an entry and its body. A missing body (evicted) counts as a miss.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn lookup(&self, key: &str) -> Result<Option<(CacheEntry, Vec<u8>)>, PortError> {
        let Some(entry) = self.index.get(key).await? else {
            return Ok(None);
        };
        Ok(self
            .blobs
            .get(&body_key(&entry.body_hash))
            .await?
            .map(|body| (entry, body)))
    }

    /// Store a body (deduplicated by content) and its metadata.
    ///
    /// # Errors
    /// Storage errors (for example the storage budget refusing the write).
    pub async fn store(
        &self,
        key: &str,
        mut entry: CacheEntry,
        body: &[u8],
    ) -> Result<(), PortError> {
        let hash = blake3::hash(body).to_hex().to_string();
        self.blobs.put(&body_key(&hash), body.to_vec()).await?;
        entry.body_hash = hash;
        entry.size = body.len() as u64;
        self.index.put(key, entry).await
    }
}

/// Freshness lifetime from `Cache-Control` (`no-store`/`no-cache` -> 0, `max-age=N` -> N),
/// otherwise `default_secs`.
#[must_use]
pub fn ttl_secs(headers: &[(String, String)], default_secs: u64) -> u64 {
    let Some((_, cc)) = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("cache-control"))
    else {
        return default_secs;
    };
    let cc = cc.to_ascii_lowercase();
    if cc.contains("no-store") || cc.contains("no-cache") {
        return 0;
    }
    cc.split(',')
        .filter_map(|d| d.trim().strip_prefix("max-age="))
        .find_map(|v| v.parse().ok())
        .unwrap_or(default_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(v: &str) -> Vec<(String, String)> {
        vec![("Cache-Control".into(), v.into())]
    }

    #[test]
    fn ttl_from_cache_control() {
        assert_eq!(ttl_secs(&h("public, max-age=600"), 5), 600);
        assert_eq!(ttl_secs(&h("no-store"), 5), 0);
        assert_eq!(ttl_secs(&[], 5), 5);
    }
}
