//! Dedicated, bounded CPU pool (D-6.8). Image decode/resize/encode and big import parsing run
//! here, never on tokio worker threads: `run` hands the closure to rayon and awaits a oneshot
//! channel, so the calling task only parks.

use std::sync::Arc;

use thiserror::Error;

/// Thread name prefix of pool workers (tests assert work runs on them).
pub const THREAD_PREFIX: &str = "cpu-pool-";

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("cpu pool: {0}")]
    Build(String),
    /// The closure panicked; the panic is contained in the pool.
    #[error("cpu task panicked")]
    Panicked,
}

#[derive(Clone)]
pub struct CpuPool {
    pool: Arc<rayon::ThreadPool>,
}

impl CpuPool {
    /// A pool with `threads` workers (at least one).
    ///
    /// # Errors
    /// If the threads cannot be spawned.
    pub fn new(threads: usize) -> Result<Self, PoolError> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.max(1))
            .thread_name(|i| format!("{THREAD_PREFIX}{i}"))
            .build()
            .map_err(|e| PoolError::Build(e.to_string()))?;
        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    /// Half the available cores, at least one: leaves room for tokio and Postgres.
    ///
    /// # Errors
    /// If the threads cannot be spawned.
    pub fn default_size() -> Result<Self, PoolError> {
        let cores = std::thread::available_parallelism().map_or(2, std::num::NonZeroUsize::get);
        Self::new(cores.div_ceil(2))
    }

    /// Run `f` on the pool and await its result without blocking the async runtime.
    ///
    /// # Errors
    /// [`PoolError::Panicked`] if `f` panicked.
    pub async fn run<F, R>(&self, f: F) -> Result<R, PoolError>
    where
        F: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.pool.spawn(move || {
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            // The receiver may be gone if the caller was cancelled; nothing to do then.
            let _ = tx.send(out);
        });
        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(_)) | Err(_) => Err(PoolError::Panicked),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    fn busy(ms: u64) -> String {
        let end = Instant::now() + Duration::from_millis(ms);
        let mut x = 0u64;
        while Instant::now() < end {
            x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        }
        std::hint::black_box(x);
        std::thread::current()
            .name()
            .unwrap_or_default()
            .to_string()
    }

    /// On a current-thread runtime a CPU job run inline would freeze every other task. Through
    /// the pool, a ticker keeps running while the job burns 300 ms of CPU.
    #[test]
    fn cpu_work_never_blocks_the_runtime() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let pool = CpuPool::new(1).unwrap();
        let ticks = Arc::new(AtomicU32::new(0));
        let (thread, ticked) = rt.block_on(async {
            let t = ticks.clone();
            let ticker = tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    t.fetch_add(1, Ordering::SeqCst);
                }
            });
            let thread = pool.run(|| busy(300)).await.unwrap();
            ticker.abort();
            (thread, ticks.load(Ordering::SeqCst))
        });
        assert!(thread.starts_with(THREAD_PREFIX), "ran on {thread}");
        // 300 ms / 10 ms = 30 ticks if the runtime is free; inline work would give 0.
        assert!(ticked >= 10, "runtime was blocked: only {ticked} ticks");
    }

    #[tokio::test]
    async fn contains_panics() {
        let pool = CpuPool::new(1).unwrap();
        let r: Result<(), _> = pool.run(|| panic!("boom")).await;
        assert!(matches!(r, Err(PoolError::Panicked)));
        // the pool still works afterwards
        assert_eq!(pool.run(|| 2 + 2).await.unwrap(), 4);
    }
}
