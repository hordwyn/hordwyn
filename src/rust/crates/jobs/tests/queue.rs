use std::sync::{Arc, Mutex};
use std::time::Duration;

use app::ports::{Lane, NewJob};
use async_trait::async_trait;
use jobs::{Handler, Job, PgQueue, Worker, WorkerConfig};
use tokio_util::sync::CancellationToken;

fn job(lane: Lane, kind: &str, key: Option<&str>) -> NewJob {
    NewJob {
        lane,
        kind: kind.into(),
        payload: serde_json::json!({}),
        dedupe_key: key.map(Into::into),
        priority: 0,
        run_at: None,
    }
}

async fn queue() -> Option<PgQueue> {
    Some(PgQueue::new(store_pg::testing::fresh_db().await?))
}

async fn state(q: &PgQueue, id: uuid::Uuid) -> (String, i32, Option<String>) {
    let r = sqlx::query!(
        "SELECT state, attempts, last_error FROM jobs WHERE id = $1",
        id
    )
    .fetch_one(q.pool())
    .await
    .unwrap();
    (r.state, r.attempts, r.last_error)
}

#[tokio::test]
async fn dedupe_key_prevents_duplicates_while_pending() {
    let Some(q) = queue().await else { return };
    let first = q
        .push(job(Lane::BackgroundRefresh, "refresh", Some("item:1")))
        .await
        .unwrap();
    let second = q
        .push(job(Lane::BackgroundRefresh, "refresh", Some("item:1")))
        .await
        .unwrap();
    assert_eq!(first, second);
    let claimed = q
        .claim(&[Lane::BackgroundRefresh], Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap();
    let third = q
        .push(job(Lane::BackgroundRefresh, "refresh", Some("item:1")))
        .await
        .unwrap();
    assert_eq!(third, first, "running job still dedupes");
    q.complete(claimed.id).await.unwrap();
    let fourth = q
        .push(job(Lane::BackgroundRefresh, "refresh", Some("item:1")))
        .await
        .unwrap();
    assert_ne!(fourth, first, "finished job does not block new work");
}

#[tokio::test]
async fn lanes_are_strictly_ordered() {
    let Some(q) = queue().await else { return };
    for _ in 0..50 {
        q.push(job(Lane::Prefetch, "p", None)).await.unwrap();
        q.push(job(Lane::UserImport, "i", None)).await.unwrap();
    }
    q.push(job(Lane::Interactive, "now", None)).await.unwrap();
    let lease = Duration::from_secs(60);
    let all = [
        Lane::Interactive,
        Lane::UserImport,
        Lane::BackgroundRefresh,
        Lane::Prefetch,
    ];
    assert_eq!(q.claim(&all, lease).await.unwrap().unwrap().kind, "now");
    for _ in 0..50 {
        assert_eq!(q.claim(&all, lease).await.unwrap().unwrap().kind, "i");
    }
    assert_eq!(q.claim(&all, lease).await.unwrap().unwrap().kind, "p");
}

struct Recorder {
    order: Mutex<Vec<String>>,
    delay: Duration,
}

#[async_trait]
impl Handler for Recorder {
    async fn run(&self, job: &Job) -> Result<(), String> {
        tokio::time::sleep(self.delay).await;
        self.order.lock().unwrap().push(job.kind.clone());
        Ok(())
    }
}

#[tokio::test]
async fn interactive_job_overtakes_a_running_bulk_import() {
    let Some(q) = queue().await else { return };
    for _ in 0..300 {
        q.push(job(Lane::UserImport, "import", None)).await.unwrap();
    }
    let rec = Arc::new(Recorder {
        order: Mutex::new(vec![]),
        delay: Duration::from_millis(5),
    });
    let cfg = WorkerConfig {
        concurrency: 2,
        poll_interval: Duration::from_millis(10),
        ..WorkerConfig::default()
    };
    let worker = Worker::new(q.clone(), cfg)
        .handle("import", rec.clone())
        .handle("interactive", rec.clone());
    let stop = CancellationToken::new();
    let handle = tokio::spawn(worker.run(stop.clone()));

    // poll instead of fixed sleeps (flaky on loaded CI runners)
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while rec.order.lock().unwrap().is_empty() {
        assert!(std::time::Instant::now() < deadline, "import never started");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let done_before = rec.order.lock().unwrap().len();
    assert!(
        done_before > 0 && done_before < 250,
        "import is under way ({done_before})"
    );
    q.push(job(Lane::Interactive, "interactive", None))
        .await
        .unwrap();
    while !rec.order.lock().unwrap().iter().any(|k| k == "interactive") {
        assert!(
            std::time::Instant::now() < deadline,
            "interactive job never ran"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    stop.cancel();
    handle.await.unwrap();

    let order = rec.order.lock().unwrap().clone();
    let pos = order
        .iter()
        .position(|k| k == "interactive")
        .expect("interactive job ran");
    // at most the jobs already running (concurrency) plus timing slack finish in between
    assert!(
        pos <= done_before + 4,
        "interactive at {pos}, enqueued after {done_before}"
    );
}

struct Flaky {
    fail_times: Mutex<u32>,
}

#[async_trait]
impl Handler for Flaky {
    async fn run(&self, _job: &Job) -> Result<(), String> {
        let mut n = self.fail_times.lock().unwrap();
        if *n > 0 {
            *n -= 1;
            return Err("upstream 503".into());
        }
        Ok(())
    }
}

#[tokio::test]
async fn failures_retry_with_backoff_then_die() {
    let Some(q) = queue().await else { return };
    let id = q.push(job(Lane::Interactive, "x", None)).await.unwrap();
    let lease = Duration::from_secs(60);
    let j = q.claim(&[Lane::Interactive], lease).await.unwrap().unwrap();
    q.fail(&j, "boom").await.unwrap();
    assert_eq!(
        state(&q, id).await,
        ("queued".into(), 1, Some("boom".into()))
    );
    assert!(
        q.claim(&[Lane::Interactive], lease)
            .await
            .unwrap()
            .is_none(),
        "not claimable before the backoff elapses"
    );
    let in_future: bool = sqlx::query_scalar!(
        "SELECT run_at > now() + interval '1 second' FROM jobs WHERE id = $1",
        id
    )
    .fetch_one(q.pool())
    .await
    .unwrap()
    .unwrap();
    assert!(in_future);

    // exhaust attempts
    sqlx::query!(
        "UPDATE jobs SET run_at = now(), attempts = max_attempts - 1 WHERE id = $1",
        id
    )
    .execute(q.pool())
    .await
    .unwrap();
    let j = q.claim(&[Lane::Interactive], lease).await.unwrap().unwrap();
    q.fail(&j, "still broken").await.unwrap();
    assert_eq!(state(&q, id).await.0, "dead");
}

#[tokio::test]
async fn worker_retries_flaky_handler_until_success() {
    let Some(q) = queue().await else { return };
    let id = q.push(job(Lane::Interactive, "flaky", None)).await.unwrap();
    let worker = Worker::new(
        q.clone(),
        WorkerConfig {
            poll_interval: Duration::from_millis(10),
            ..WorkerConfig::default()
        },
    )
    .handle(
        "flaky",
        Arc::new(Flaky {
            fail_times: Mutex::new(1),
        }),
    );
    let stop = CancellationToken::new();
    let handle = tokio::spawn(worker.run(stop.clone()));
    // poll instead of fixed sleeps (they were flaky on loaded CI runners)
    let wait_until = |want: &'static str, attempts: i32| {
        let q = q.clone();
        async move {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            loop {
                let s = state(&q, id).await;
                if s.0 == want && s.1 == attempts {
                    return s;
                }
                assert!(std::time::Instant::now() < deadline, "stuck at {s:?}");
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    };
    wait_until("queued", 1).await;
    sqlx::query!("UPDATE jobs SET run_at = now() WHERE id = $1", id)
        .execute(q.pool())
        .await
        .unwrap();
    wait_until("done", 2).await;
    stop.cancel();
    handle.await.unwrap();
    assert_eq!(
        state(&q, id).await,
        ("done".into(), 2, Some("upstream 503".into()))
    );
}

#[tokio::test]
async fn expired_lease_is_reclaimed() {
    let Some(q) = queue().await else { return };
    q.push(job(Lane::Interactive, "x", None)).await.unwrap();
    let first = q
        .claim(&[Lane::Interactive], Duration::from_millis(1))
        .await
        .unwrap()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let again = q
        .claim(&[Lane::Interactive], Duration::from_secs(60))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.id, again.id);
    assert_eq!(again.attempts, 2);
}

#[tokio::test]
async fn concurrent_claimers_never_share_a_job() {
    let Some(q) = queue().await else { return };
    for _ in 0..100 {
        q.push(job(Lane::Prefetch, "x", None)).await.unwrap();
    }
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let q = q.clone();
        tasks.push(tokio::spawn(async move {
            let mut got = Vec::new();
            while let Some(j) = q
                .claim(&[Lane::Prefetch], Duration::from_secs(60))
                .await
                .unwrap()
            {
                got.push(j.id);
            }
            got
        }));
    }
    let mut all = Vec::new();
    for t in tasks {
        all.extend(t.await.unwrap());
    }
    let n = all.len();
    all.sort();
    all.dedup();
    assert_eq!((n, all.len()), (100, 100));
}

#[tokio::test]
async fn cron_enqueues_once_per_occurrence() {
    let Some(q) = queue().await else { return };
    let t0 = chrono::DateTime::parse_from_rfc3339("2026-03-01T08:00:30Z")
        .unwrap()
        .to_utc();
    q.schedule(
        "digest",
        "*/10 * * * *",
        Lane::BackgroundRefresh,
        "notify.digest",
        serde_json::json!({}),
        t0,
    )
    .await
    .unwrap();
    assert_eq!(q.tick_cron(t0).await.unwrap(), 0, "not due yet");
    let t1 = t0 + chrono::Duration::minutes(10);
    assert_eq!(q.tick_cron(t1).await.unwrap(), 1);
    assert_eq!(
        q.tick_cron(t1).await.unwrap(),
        0,
        "same occurrence is not enqueued twice"
    );
    let next: chrono::DateTime<chrono::Utc> =
        sqlx::query_scalar!("SELECT next_run_at FROM cron_schedules WHERE name = 'digest'")
            .fetch_one(q.pool())
            .await
            .unwrap();
    assert_eq!(next.to_rfc3339(), "2026-03-01T08:20:00+00:00");
    assert!(
        q.schedule(
            "bad",
            "every day",
            Lane::Prefetch,
            "x",
            serde_json::json!(null),
            t0
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn shutdown_waits_for_running_jobs() {
    let Some(q) = queue().await else { return };
    let id = q.push(job(Lane::Interactive, "slow", None)).await.unwrap();
    let rec = Arc::new(Recorder {
        order: Mutex::new(vec![]),
        delay: Duration::from_millis(300),
    });
    let worker = Worker::new(
        q.clone(),
        WorkerConfig {
            poll_interval: Duration::from_millis(10),
            ..WorkerConfig::default()
        },
    )
    .handle("slow", rec.clone());
    let stop = CancellationToken::new();
    let handle = tokio::spawn(worker.run(stop.clone()));
    // Wait until the worker has claimed the job (a fixed sleep was flaky on loaded CI runners).
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while state(&q, id).await.0 == "queued" {
        assert!(
            std::time::Instant::now() < deadline,
            "job was never claimed"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    stop.cancel();
    handle.await.unwrap();
    assert_eq!(
        state(&q, id).await.0,
        "done",
        "in-flight job finished before run() returned"
    );
}
