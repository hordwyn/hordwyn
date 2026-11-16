use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use app::ports::Lane;
use async_trait::async_trait;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::Instrument;

use crate::{Job, PgQueue};

/// Runs jobs of one `kind`.
#[async_trait]
pub trait Handler: Send + Sync {
    /// # Errors
    /// A message stored as `last_error`; the job is retried with backoff.
    async fn run(&self, job: &Job) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Jobs run at the same time by this worker.
    pub concurrency: usize,
    /// How long a claimed job is protected from other workers.
    pub lease: Duration,
    /// Sleep when the queue is empty.
    pub poll_interval: Duration,
    /// Lanes this worker serves.
    pub lanes: Vec<Lane>,
    /// How long shutdown waits for running jobs before giving up on them (they are then
    /// claimed again by another worker after their lease expires).
    pub shutdown_grace: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            lease: Duration::from_secs(300),
            poll_interval: Duration::from_millis(250),
            lanes: vec![
                Lane::Interactive,
                Lane::UserImport,
                Lane::BackgroundRefresh,
                Lane::Prefetch,
            ],
            shutdown_grace: Duration::from_secs(30),
        }
    }
}

pub struct Worker {
    queue: PgQueue,
    handlers: HashMap<String, Arc<dyn Handler>>,
    config: WorkerConfig,
}

impl Worker {
    #[must_use]
    pub fn new(queue: PgQueue, config: WorkerConfig) -> Self {
        Self {
            queue,
            handlers: HashMap::new(),
            config,
        }
    }

    #[must_use]
    pub fn handle(mut self, kind: &str, handler: Arc<dyn Handler>) -> Self {
        self.handlers.insert(kind.to_string(), handler);
        self
    }

    /// Claim and run jobs until `shutdown` is cancelled, then stop claiming and wait up to
    /// `shutdown_grace` for running jobs to finish.
    ///
    /// # Panics
    /// Never: the internal semaphore is never closed.
    pub async fn run(self, shutdown: CancellationToken) {
        let permits = Arc::new(Semaphore::new(self.config.concurrency));
        let handlers = Arc::new(self.handlers);
        loop {
            let permit = tokio::select! {
                () = shutdown.cancelled() => break,
                p = permits.clone().acquire_owned() => p.expect("semaphore is never closed"),
            };
            let job = match self
                .queue
                .claim(&self.config.lanes, self.config.lease)
                .await
            {
                Ok(Some(job)) => job,
                Ok(None) => {
                    drop(permit);
                    tokio::select! {
                        () = shutdown.cancelled() => break,
                        () = tokio::time::sleep(self.config.poll_interval) => continue,
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "claiming a job failed");
                    drop(permit);
                    tokio::select! {
                        () = shutdown.cancelled() => break,
                        () = tokio::time::sleep(self.config.poll_interval) => continue,
                    }
                }
            };
            let queue = self.queue.clone();
            let handlers = handlers.clone();
            let lease = self.config.lease;
            tokio::spawn(async move {
                let _permit = permit;
                execute(&queue, &handlers, job, lease).await;
            });
        }
        let all = u32::try_from(self.config.concurrency).unwrap_or(u32::MAX);
        if tokio::time::timeout(self.config.shutdown_grace, permits.acquire_many(all))
            .await
            .is_err()
        {
            tracing::warn!("shutdown grace elapsed with jobs still running");
        }
    }
}

async fn execute(
    queue: &PgQueue,
    handlers: &HashMap<String, Arc<dyn Handler>>,
    job: Job,
    lease: Duration,
) {
    // `instrument`, not `span.enter()`: an entered guard held across `.await` would attach the
    // span to whatever else runs on this thread meanwhile.
    let span = tracing::info_span!("job", id = %job.id, kind = %job.kind, attempt = job.attempts);
    execute_in_span(queue, handlers, job, lease)
        .instrument(span)
        .await;
}

async fn execute_in_span(
    queue: &PgQueue,
    handlers: &HashMap<String, Arc<dyn Handler>>,
    job: Job,
    lease: Duration,
) {
    let run = async {
        match handlers.get(&job.kind) {
            Some(h) => h.run(&job).await,
            None => Err(format!("no handler for job kind `{}`", job.kind)),
        }
    };
    // heartbeat: renew the lease while the handler runs (G2-3), so a long import is not
    // claimed a second time; a crashed worker stops renewing and the job is reclaimed
    let beat = async {
        let mut every = tokio::time::interval((lease / 3).max(Duration::from_millis(100)));
        every.tick().await;
        loop {
            every.tick().await;
            match queue.extend(&job, lease).await {
                Ok(true) => {}
                // fenced (DEC-844): the lease lapsed and another worker claimed the job;
                // this run's result will not be recorded. The handler is not interrupted
                // (at-least-once: in this rare case a job runs twice).
                Ok(false) => tracing::warn!(
                    "lost the job: its lease lapsed and another worker claimed it; this run's result will be discarded"
                ),
                Err(e) => tracing::warn!(error = %e, "extending the job lease failed"),
            }
        }
    };
    let result = tokio::select! {
        r = run => r,
        () = beat => unreachable!("the heartbeat never ends"),
    };
    let stored = match result {
        Ok(()) => queue.complete(&job).await,
        Err(e) => {
            tracing::warn!(error = %e, "job failed");
            queue.fail(&job, &e).await
        }
    };
    match stored {
        Ok(true) => {}
        Ok(false) => tracing::warn!(
            "the job result was discarded: its lease lapsed and another claim holds the job"
        ),
        Err(e) => tracing::error!(error = %e, "recording the job result failed"),
    }
}
