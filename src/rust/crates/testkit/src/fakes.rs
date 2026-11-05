//! Fakes for the infrastructure ports.

use std::collections::HashMap;
use std::sync::Mutex;

use app::ports::{
    AppEvent, BlobStore, Clock, EventBus, HttpFetch, HttpRequest, HttpResponse, IdGen, JobQueue,
    NewJob, Notification, Notifier, PortError, SearchDoc, SearchHit, SearchIndex, SearchQuery,
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

#[derive(Default)]
pub struct RecordingNotifier {
    sent: Mutex<Vec<Notification>>,
}

impl RecordingNotifier {
    #[must_use]
    pub fn sent(&self) -> Vec<Notification> {
        self.sent.lock().expect("lock").clone()
    }
}

#[async_trait]
impl Notifier for RecordingNotifier {
    async fn send(&self, n: &Notification) -> Result<(), PortError> {
        self.sent.lock().expect("lock").push(n.clone());
        Ok(())
    }
}

/// Replays recorded responses keyed by `METHOD url`; unknown requests fail.
#[derive(Default)]
pub struct ReplayHttp {
    responses: Mutex<HashMap<String, HttpResponse>>,
    requests: Mutex<Vec<HttpRequest>>,
}

impl ReplayHttp {
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
    async fn fetch(&self, req: HttpRequest) -> Result<HttpResponse, PortError> {
        let key = format!("{} {}", req.method, req.url);
        self.requests.lock().expect("lock").push(req);
        self.responses
            .lock()
            .expect("lock")
            .get(&key)
            .cloned()
            .ok_or_else(|| PortError(format!("no recorded response for {key}")))
    }
}
