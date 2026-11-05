//! Priority-aware concurrency gate: when permits are scarce, waiting requests are served in
//! lane order (`interactive` first), then FIFO (D-6.4 "priority lanes ... and the governor").

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::{Arc, Mutex};

use app::ports::Lane;
use tokio::sync::oneshot;

struct Waiter {
    lane: Lane,
    seq: u64,
    tx: oneshot::Sender<()>,
}

impl PartialEq for Waiter {
    fn eq(&self, other: &Self) -> bool {
        self.lane == other.lane && self.seq == other.seq
    }
}
impl Eq for Waiter {}
impl PartialOrd for Waiter {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Waiter {
    /// `BinaryHeap` is a max-heap: the "greatest" waiter is the most important one.
    fn cmp(&self, other: &Self) -> Ordering {
        other.lane.cmp(&self.lane).then(other.seq.cmp(&self.seq))
    }
}

struct State {
    available: usize,
    seq: u64,
    waiters: BinaryHeap<Waiter>,
}

pub struct Gate {
    state: Mutex<State>,
}

/// Returns its permit to the gate (or hands it to the best waiter) when dropped.
pub struct GatePermit {
    gate: Arc<Gate>,
}

impl Gate {
    #[must_use]
    pub fn new(permits: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                available: permits,
                seq: 0,
                waiters: BinaryHeap::new(),
            }),
        })
    }

    /// # Panics
    /// If the internal mutex is poisoned.
    pub async fn acquire(self: &Arc<Self>, lane: Lane) -> GatePermit {
        let rx = {
            let mut s = self.state.lock().expect("gate lock");
            if s.available > 0 && s.waiters.is_empty() {
                s.available -= 1;
                return GatePermit { gate: self.clone() };
            }
            let (tx, rx) = oneshot::channel();
            s.seq += 1;
            let seq = s.seq;
            s.waiters.push(Waiter { lane, seq, tx });
            rx
        };
        // The sender is only dropped after handing us the permit, or never.
        let _ = rx.await;
        GatePermit { gate: self.clone() }
    }

    fn release(&self) {
        let mut s = self.state.lock().expect("gate lock");
        while let Some(w) = s.waiters.pop() {
            // A dropped receiver means that waiter gave up; try the next one.
            if w.tx.send(()).is_ok() {
                return;
            }
        }
        s.available += 1;
    }
}

impl Drop for GatePermit {
    fn drop(&mut self) {
        self.gate.release();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn interactive_waiter_is_served_before_earlier_prefetch_waiters() {
        let gate = Gate::new(1);
        let held = gate.acquire(Lane::Interactive).await;
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        for (i, lane) in [Lane::Prefetch, Lane::Prefetch, Lane::Interactive]
            .into_iter()
            .enumerate()
        {
            let (g, o) = (gate.clone(), order.clone());
            tasks.push(tokio::spawn(async move {
                let _p = g.acquire(lane).await;
                o.lock().unwrap().push(lane);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }));
            // make arrival order deterministic
            tokio::time::sleep(Duration::from_millis(10 * (i as u64 + 1))).await;
        }
        drop(held);
        for t in tasks {
            t.await.unwrap();
        }
        assert_eq!(
            *order.lock().unwrap(),
            vec![Lane::Interactive, Lane::Prefetch, Lane::Prefetch]
        );
    }

    #[tokio::test]
    async fn cancelled_waiter_does_not_leak_the_permit() {
        let gate = Gate::new(1);
        let held = gate.acquire(Lane::Interactive).await;
        let g = gate.clone();
        let waiting = tokio::spawn(async move { g.acquire(Lane::Prefetch).await });
        tokio::time::sleep(Duration::from_millis(10)).await;
        waiting.abort();
        let _ = waiting.await;
        drop(held);
        let again =
            tokio::time::timeout(Duration::from_millis(100), gate.acquire(Lane::Prefetch)).await;
        assert!(again.is_ok());
    }
}
