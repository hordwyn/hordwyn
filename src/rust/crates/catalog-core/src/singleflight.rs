//! Concurrent calls with the same key share one execution (D-6.2).

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::sync::OnceCell;

pub struct SingleFlight<V> {
    inflight: Mutex<HashMap<String, Arc<OnceCell<V>>>>,
}

impl<V> Default for SingleFlight<V> {
    fn default() -> Self {
        Self {
            inflight: Mutex::new(HashMap::new()),
        }
    }
}

impl<V: Clone> SingleFlight<V> {
    /// Run `f` unless a call with `key` is already running; then wait for its result.
    ///
    /// # Panics
    /// If the internal mutex is poisoned.
    pub async fn run<F, Fut>(&self, key: &str, f: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        let cell = self
            .inflight
            .lock()
            .expect("singleflight lock")
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        let value = cell.get_or_init(f).await.clone();
        let mut map = self.inflight.lock().expect("singleflight lock");
        if map.get(key).is_some_and(|c| Arc::ptr_eq(c, &cell)) {
            map.remove(key);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn concurrent_calls_share_one_execution() {
        let sf = Arc::new(SingleFlight::<u32>::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..50 {
            let (sf, calls) = (sf.clone(), calls.clone());
            tasks.push(tokio::spawn(async move {
                sf.run("k", || async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    7
                })
                .await
            }));
        }
        for t in tasks {
            assert_eq!(t.await.unwrap(), 7);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // a later call runs again
        sf.run("k", || async {
            calls.fetch_add(1, Ordering::SeqCst);
            8
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
