//! Fakes for the infrastructure ports.

use std::collections::HashMap;
use std::sync::Mutex;

use app::ports::{
    AppEvent, BlobStore, Clock, EventBus, HttpError, HttpFetch, HttpRequest, HttpResponse, IdGen,
    JobQueue, NewJob, PortError, SearchDoc, SearchHit, SearchIndex, SearchQuery,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use domain::hlc::Hlc;

/// Clock that only moves when told to. Starts at 2026-01-01T00:00:00Z.
pub struct FixedClock {
    state: Mutex<(DateTime<Utc>, Hlc)>,
}

impl Default for FixedClock {
    fn default() -> Self {
        let t = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .expect("valid")
            .with_timezone(&Utc);
        Self {
            state: Mutex::new((t, Hlc::new(0, 0, 1))),
        }
    }
}

impl FixedClock {
    pub fn set(&self, t: DateTime<Utc>) {
        self.state.lock().expect("lock").0 = t;
    }

    pub fn advance(&self, d: Duration) {
        let mut s = self.state.lock().expect("lock");
        s.0 += d;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.state.lock().expect("lock").0
    }

    fn observe(&self, remote: Hlc) {
        let mut s = self.state.lock().expect("lock");
        let now_ms = u64::try_from(s.0.timestamp_millis()).unwrap_or(0);
        s.1 = s.1.receive(remote, now_ms);
    }

    fn hlc(&self) -> Hlc {
        let mut s = self.state.lock().expect("lock");
        let now_ms = u64::try_from(s.0.timestamp_millis()).unwrap_or(0);
        s.1 = s.1.tick(now_ms);
        s.1
    }
}

/// Sequential, deterministic ids and tokens.
#[derive(Default)]
pub struct SeqIds {
    n: Mutex<u128>,
}

impl SeqIds {
    fn next(&self) -> u128 {
        let mut n = self.n.lock().expect("lock");
        *n += 1;
        *n
    }
}

impl IdGen for SeqIds {
    fn new_id(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(self.next())
    }

    fn new_token(&self) -> String {
        format!("token-{}", self.next())
    }

    fn hash_token(&self, token: &str) -> String {
        format!("hash:{token}")
    }
}

#[derive(Default)]
pub struct RecordingEvents {
    events: Mutex<Vec<AppEvent>>,
}

impl RecordingEvents {
    #[must_use]
    pub fn all(&self) -> Vec<AppEvent> {
        self.events.lock().expect("lock").clone()
    }

    #[must_use]
    pub fn topics(&self) -> Vec<String> {
        self.all().into_iter().map(|e| e.topic).collect()
    }

    pub fn clear(&self) {
        self.events.lock().expect("lock").clear();
    }
}

#[async_trait]
impl EventBus for RecordingEvents {
    async fn publish(&self, event: AppEvent) {
        self.events.lock().expect("lock").push(event);
    }
}

/// Notifications emitted by use-cases, for assertions.
#[derive(Default)]
pub struct RecordingNotifications {
    emitted: Mutex<Vec<app::ports::Notification>>,
}

impl RecordingNotifications {
    #[must_use]
    pub fn all(&self) -> Vec<app::ports::Notification> {
        self.emitted.lock().expect("lock").clone()
    }

    #[must_use]
    pub fn events(&self) -> Vec<String> {
        self.all().into_iter().map(|n| n.event).collect()
    }
}

#[async_trait]
impl app::ports::Notifications for RecordingNotifications {
    async fn emit(&self, n: app::ports::Notification) {
        self.emitted.lock().expect("lock").push(n);
    }
}

/// Job queue that records jobs and honours dedupe keys.
#[derive(Default)]
pub struct MemJobs {
    jobs: Mutex<Vec<(uuid::Uuid, NewJob)>>,
    n: Mutex<u128>,
}

impl MemJobs {
    #[must_use]
    pub fn jobs(&self) -> Vec<NewJob> {
        self.jobs
            .lock()
            .expect("lock")
            .iter()
            .map(|(_, j)| j.clone())
            .collect()
    }

    /// Remove and return all jobs (simulates a worker draining the queue).
    pub fn drain(&self) -> Vec<NewJob> {
        self.jobs
            .lock()
            .expect("lock")
            .drain(..)
            .map(|(_, j)| j)
            .collect()
    }
}

#[async_trait]
impl JobQueue for MemJobs {
    async fn enqueue(&self, job: NewJob) -> Result<uuid::Uuid, PortError> {
        let mut jobs = self.jobs.lock().expect("lock");
        if let Some(key) = &job.dedupe_key
            && let Some((id, _)) = jobs
                .iter()
                .find(|(_, j)| j.dedupe_key.as_ref() == Some(key))
        {
            return Ok(*id);
        }
        let mut n = self.n.lock().expect("lock");
        *n += 1;
        let id = uuid::Uuid::from_u128(*n);
        jobs.push((id, job));
        Ok(id)
    }
}

/// Substring search over JSON fields, filtered by owner like the real adapters.
#[derive(Default)]
pub struct MemSearch {
    indexes: Mutex<HashMap<String, Vec<SearchDoc>>>,
}

#[async_trait]
impl SearchIndex for MemSearch {
    async fn upsert(&self, index: &str, docs: Vec<SearchDoc>) -> Result<(), PortError> {
        let mut idx = self.indexes.lock().expect("lock");
        let list = idx.entry(index.to_string()).or_default();
        for d in docs {
            list.retain(|x| x.id != d.id);
            list.push(d);
        }
        Ok(())
    }

    async fn delete(&self, index: &str, ids: Vec<String>) -> Result<(), PortError> {
        if let Some(list) = self.indexes.lock().expect("lock").get_mut(index) {
            list.retain(|d| !ids.contains(&d.id));
        }
        Ok(())
    }

    async fn search(&self, q: SearchQuery) -> Result<Vec<SearchHit>, PortError> {
        let idx = self.indexes.lock().expect("lock");
        let text = q.text.to_lowercase();
        Ok(idx
            .get(&q.index)
            .into_iter()
            .flatten()
            .filter(|d| d.owner.is_none_or(|o| o == q.viewer))
            .filter(|d| d.fields.to_string().to_lowercase().contains(&text))
            .filter(|d| {
                q.filters
                    .iter()
                    .all(|(k, v)| d.fields.get(k).and_then(|x| x.as_str()) == Some(v.as_str()))
            })
            .take(q.limit)
            .map(|d| SearchHit {
                id: d.id.clone(),
                fields: d.fields.clone(),
            })
            .collect())
    }
}

#[derive(Default)]
pub struct MemBlobs {
    blobs: Mutex<HashMap<String, Vec<u8>>>,
}

impl MemBlobs {
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.blobs
            .lock()
            .expect("lock")
            .values()
            .map(Vec::len)
            .sum()
    }
}

#[async_trait]
impl BlobStore for MemBlobs {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), PortError> {
        self.blobs
            .lock()
            .expect("lock")
            .insert(key.to_string(), bytes);
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PortError> {
        Ok(self.blobs.lock().expect("lock").get(key).cloned())
    }

    async fn delete(&self, keys: &[String]) -> Result<(), PortError> {
        let mut b = self.blobs.lock().expect("lock");
        for k in keys {
            b.remove(k);
        }
        Ok(())
    }
}

/// Replays recorded responses keyed by `METHOD url`; unknown requests fail.
#[derive(Default)]
pub struct ReplayHttp {
    responses: Mutex<HashMap<String, HttpResponse>>,
    /// Responses that also require a request-body substring (`request.body_contains`), for
    /// APIs that POST different queries to one URL.
    by_body: Mutex<Vec<(String, String, HttpResponse)>>,
    requests: Mutex<Vec<HttpRequest>>,
    /// The pauses connectors asked for (`HttpFetch::pause`), in order.
    pauses: Mutex<Vec<(String, std::time::Duration)>>,
}

impl ReplayHttp {
    /// Record a response with headers (`Retry-After`, `Cache-Control`, ...).
    pub fn record_with_headers(
        &self,
        method: &str,
        url: &str,
        status: u16,
        headers: Vec<(String, String)>,
        body: impl Into<Vec<u8>>,
    ) {
        self.responses.lock().expect("lock").insert(
            format!("{method} {url}"),
            HttpResponse {
                status,
                headers,
                body: body.into(),
            },
        );
    }

    /// The pauses asked for so far, as `(connector, wait)`.
    #[must_use]
    pub fn pauses(&self) -> Vec<(String, std::time::Duration)> {
        self.pauses.lock().expect("lock").clone()
    }

    pub fn record(&self, method: &str, url: &str, status: u16, body: impl Into<Vec<u8>>) {
        self.responses.lock().expect("lock").insert(
            format!("{method} {url}"),
            HttpResponse {
                status,
                headers: vec![],
                body: body.into(),
            },
        );
    }

    #[must_use]
    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().expect("lock").clone()
    }
}

#[async_trait]
impl HttpFetch for ReplayHttp {
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, HttpError> {
        let key = format!("{} {}", req.method, req.url);
        let body = String::from_utf8_lossy(req.body.as_deref().unwrap_or_default()).to_string();
        self.requests.lock().expect("lock").push(req);
        if let Some((_, _, resp)) = self
            .by_body
            .lock()
            .expect("lock")
            .iter()
            .find(|(k, needle, _)| *k == key && body.contains(needle.as_str()))
        {
            return Ok(resp.clone());
        }
        self.responses
            .lock()
            .expect("lock")
            .get(&key)
            .cloned()
            .ok_or_else(|| HttpError::Network(format!("no recorded response for {key}")))
    }

    fn pause(&self, connector: &str, wait: std::time::Duration) {
        self.pauses
            .lock()
            .expect("lock")
            .push((connector.to_string(), wait));
    }
}

impl ReplayHttp {
    /// Load every recorded-HTTP fixture (`*.json` with `request`, `status`, `headers`,
    /// `body`) in `dir`.
    ///
    /// # Panics
    /// If the directory or a fixture cannot be read or parsed.
    #[must_use]
    pub fn from_dir(dir: &std::path::Path) -> Self {
        let replay = Self::default();
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("fixture dir {}: {e}", dir.display()))
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        entries.sort();
        for path in entries {
            let text = std::fs::read_to_string(&path).expect("read fixture");
            let f: serde_json::Value = serde_json::from_str(&text).expect("fixture json");
            let method = f["request"]["method"].as_str().expect("request.method");
            let url = f["request"]["url"].as_str().expect("request.url");
            let status =
                u16::try_from(f["status"].as_u64().expect("status")).expect("status range");
            let headers = f["headers"]
                .as_array()
                .map(|hs| {
                    hs.iter()
                        .filter_map(|h| {
                            Some((h[0].as_str()?.to_string(), h[1].as_str()?.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let body = match &f["body"] {
                serde_json::Value::String(s) => s.clone().into_bytes(),
                other => serde_json::to_vec(other).expect("body"),
            };
            let resp = HttpResponse {
                status,
                headers,
                body,
            };
            let key = format!("{method} {url}");
            match f["request"]["body_contains"].as_str() {
                Some(needle) => {
                    replay
                        .by_body
                        .lock()
                        .expect("lock")
                        .push((key, needle.to_string(), resp));
                }
                None => {
                    replay.responses.lock().expect("lock").insert(key, resp);
                }
            }
        }
        replay
    }
}

impl FixedClock {
    /// Current fake time (convenience for assertions).
    #[must_use]
    pub fn now_for_tests(&self) -> DateTime<Utc> {
        Clock::now(self)
    }
}

/// A stand-in for the key ring in tests: NOT encryption. The stored bytes carry the
/// envelope header (format 1, key version) and are bound to the slot, so slot mix-ups and
/// "key not loaded" (`version` not in `loaded`) behave like the real thing.
pub struct FakeCipher {
    pub version: u16,
    pub loaded: Vec<u16>,
}

impl Default for FakeCipher {
    fn default() -> Self {
        Self {
            version: 1,
            loaded: vec![1],
        }
    }
}

fn slot_tag(slot: app::ports::SecretSlot<'_>) -> [u8; 8] {
    let h = blake3::hash(format!("{}|{}|{}", slot.purpose, slot.row, slot.field).as_bytes());
    let mut t = [0u8; 8];
    t.copy_from_slice(&h.as_bytes()[..8]);
    t
}

impl app::ports::SecretCipher for FakeCipher {
    fn seal(&self, slot: app::ports::SecretSlot<'_>, plaintext: &[u8]) -> Vec<u8> {
        let tag = slot_tag(slot);
        let mut out = vec![1];
        out.extend_from_slice(&self.version.to_be_bytes());
        out.extend_from_slice(&tag);
        out.extend(
            plaintext
                .iter()
                .zip(tag.iter().cycle())
                .map(|(p, k)| p ^ k ^ 0x5a),
        );
        out
    }

    fn open(
        &self,
        slot: app::ports::SecretSlot<'_>,
        blob: &[u8],
    ) -> Result<Vec<u8>, app::ports::SecretLocked> {
        let version = (blob.len() >= 11).then(|| u16::from_be_bytes([blob[1], blob[2]]));
        let locked = app::ports::SecretLocked { version };
        let tag = slot_tag(slot);
        if !version.is_some_and(|v| self.loaded.contains(&v)) || blob[3..11] != tag {
            return Err(locked);
        }
        Ok(blob[11..]
            .iter()
            .zip(tag.iter().cycle())
            .map(|(c, k)| c ^ k ^ 0x5a)
            .collect())
    }

    fn rewrap(
        &self,
        slot: app::ports::SecretSlot<'_>,
        blob: &[u8],
    ) -> Result<Option<Vec<u8>>, app::ports::SecretLocked> {
        let plain = self.open(slot, blob)?;
        if blob[1..3] == self.version.to_be_bytes() {
            return Ok(None);
        }
        Ok(Some(self.seal(slot, &plain)))
    }
}
