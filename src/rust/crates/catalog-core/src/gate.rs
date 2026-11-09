//! Priority-aware concurrency gate: when permits are scarce, waiting requests are served in
//! lane order (`interactive` first), then FIFO (D-6.4 "priority lanes ... and the governor").
//! In hosted mode the interactive lane is shared fairly (design.md 4.1): its waiters are
//! served round-robin per user, then FIFO, so one member's burst does not hold up the others.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::{Arc, Mutex};

use app::ports::Lane;
use domain::ids::UserId;
use tokio::sync::oneshot;

struct Waiter {
    lane: Lane,
    /// The waiter's turn among its user's queued interactive waiters (fair share; 0 else).
    round: u64,
    seq: u64,
    tx: oneshot::Sender<()>,
}

impl PartialEq for Waiter {
    fn eq(&self, other: &Self) -> bool {
        self.lane == other.lane && self.round == other.round && self.seq == other.seq
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
        other
            .lane
            .cmp(&self.lane)
            .then(other.round.cmp(&self.round))
            .then(other.seq.cmp(&self.seq))
    }
}

struct State {
    available: usize,
    seq: u64,
    waiters: BinaryHeap<Waiter>,
    /// Fair share: the round of the interactive waiter served last; a new waiter never gets
    /// an earlier round, so a user who was just served does not jump a queued tail (UX4-2).
    current_round: u64,
    /// Fair share: the last round given to each user still at or past `current_round`.
    last_round: HashMap<Option<UserId>, u64>,
}

pub struct Gate {
    state: Mutex<State>,
}

/// Returns its permit to the gate (or hands it to the best waiter) when dropped.
pub struct GatePermit {
    gate: Arc<Gate>,
}

/// A queued waiter's receiver. Dropped before it took the permit (the request was
/// cancelled), it closes the channel so no permit is sent any more, and returns a permit that
/// was already sent (UX4-3).
struct Pending {
    gate: Arc<Gate>,
    rx: oneshot::Receiver<()>,
    taken: bool,
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.taken {
            return;
        }
        self.rx.close();
        if self.rx.try_recv().is_ok() {
            self.gate.release();
        }
    }
}

impl Gate {
    #[must_use]
    pub fn new(permits: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                available: permits,
                seq: 0,
                waiters: BinaryHeap::new(),
                current_round: 0,
                last_round: HashMap::new(),
            }),
        })
    }

    /// Wait for a permit in lane order, first come first served within a lane.
    pub async fn acquire(self: &Arc<Self>, lane: Lane) -> GatePermit {
        self.acquire_for(lane, None, false).await
    }

    /// Wait for a permit. With `fair` (hosted mode), interactive waiters are served one per
    /// `user` in turn: a waiter queues one round after its own user's last request (queued or
    /// served), and never before the round being served.
    ///
    /// # Panics
    /// If the internal mutex is poisoned.
    pub async fn acquire_for(
        self: &Arc<Self>,
        lane: Lane,
        user: Option<UserId>,
        fair: bool,
    ) -> GatePermit {
        let rx = {
            let mut s = self.state.lock().expect("gate lock");
            if s.available > 0 && s.waiters.is_empty() {
                s.available -= 1;
                if fair && lane == Lane::Interactive {
                    let round = s.next_round(user);
                    s.last_round.insert(user, round);
                }
                return GatePermit { gate: self.clone() };
            }
            let round = if fair && lane == Lane::Interactive {
                let round = s.next_round(user);
                s.last_round.insert(user, round);
                round
            } else {
                0
            };
            let (tx, rx) = oneshot::channel();
            s.seq += 1;
            let seq = s.seq;
            s.waiters.push(Waiter {
                lane,
                round,
                seq,
                tx,
            });
            rx
        };
        let mut pending = Pending {
            gate: self.clone(),
            rx,
            taken: false,
        };
        // The sender is only dropped after handing us the permit, or never.
        let _ = (&mut pending.rx).await;
        pending.taken = true;
        GatePermit { gate: self.clone() }
    }

    fn release(&self) {
        let mut s = self.state.lock().expect("gate lock");
        while let Some(w) = s.waiters.pop() {
            // A dropped receiver means that waiter gave up; try the next one.
            if w.tx.send(()).is_ok() {
                if w.lane == Lane::Interactive && w.round > s.current_round {
                    s.current_round = w.round;
                    let current = s.current_round;
                    s.last_round.retain(|_, r| *r >= current);
                }
                return;
            }
        }
        s.available += 1;
    }
}

impl State {
    fn next_round(&self, user: Option<UserId>) -> u64 {
        self.last_round
            .get(&user)
            .map_or(0, |r| r + 1)
            .max(self.current_round)
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

#[cfg(test)]
mod fair_share_tests {
    use super::*;
    use domain::ids::UserId;
    use std::time::Duration;

    /// Where B's single request is served when A queued five before it.
    async fn position_of_b(fair: bool) -> usize {
        let gate = Gate::new(1);
        let held = gate.acquire(Lane::Interactive).await;
        let a = UserId(uuid::Uuid::from_u128(1));
        let b = UserId(uuid::Uuid::from_u128(2));
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        for user in [a, a, a, a, a, b] {
            let (g, o) = (gate.clone(), order.clone());
            tasks.push(tokio::spawn(async move {
                let _p = g.acquire_for(Lane::Interactive, Some(user), fair).await;
                o.lock().unwrap().push(user);
                tokio::time::sleep(Duration::from_millis(2)).await;
            }));
            // make arrival order deterministic
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        drop(held);
        for t in tasks {
            t.await.unwrap();
        }
        let order = order.lock().unwrap();
        assert_eq!(order.len(), 6);
        order.iter().position(|u| *u == b).unwrap()
    }

    #[tokio::test]
    async fn hosted_mode_serves_interactive_waiters_round_robin_per_user() {
        // design.md 4.1: per-user fair share on the interactive lane in hosted mode only
        assert_eq!(position_of_b(true).await, 1, "hosted: B second");
        assert_eq!(position_of_b(false).await, 5, "home: B sixth (first come)");
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    use domain::ids::UserId;
    use std::time::Duration;

    #[tokio::test]
    async fn a_waiter_cancelled_after_it_was_handed_the_permit_returns_it() {
        // UX4-3: the permit was sent, then the waiter was dropped before its next poll
        let gate = Gate::new(1);
        let held = gate.acquire(Lane::Interactive).await;
        let g = gate.clone();
        let waiting = tokio::spawn(async move { g.acquire(Lane::Interactive).await });
        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(held);
        waiting.abort();
        let _ = waiting.await;
        let again =
            tokio::time::timeout(Duration::from_millis(200), gate.acquire(Lane::Prefetch)).await;
        assert!(again.is_ok(), "the lane lost its only permit");
    }

    #[tokio::test]
    async fn a_steady_stream_of_other_users_does_not_starve_a_queued_burst() {
        // UX4-2: B and C ask one request at a time (each asks again as soon as it is served)
        // while A has five queued; with three users in turn A's fifth comes by position 15
        let gate = Gate::new(1);
        let held = gate.acquire(Lane::Interactive).await;
        let user = |n| UserId(uuid::Uuid::from_u128(n));
        let (burst, one, two) = (user(1), user(2), user(3));
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut tasks = Vec::new();
        let mut asks = vec![(burst, 1); 5];
        asks.extend([(one, 10), (two, 10)]);
        for (who, times) in asks {
            let (g, o) = (gate.clone(), order.clone());
            tasks.push(tokio::spawn(async move {
                for _ in 0..times {
                    let _p = g.acquire_for(Lane::Interactive, Some(who), true).await;
                    o.lock().unwrap().push(who);
                    tokio::time::sleep(Duration::from_millis(2)).await;
                }
            }));
            // make arrival order deterministic
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        drop(held);
        for t in tasks {
            t.await.unwrap();
        }
        let order = order.lock().unwrap();
        assert_eq!(order.len(), 25);
        let fifth = order
            .iter()
            .enumerate()
            .filter(|(_, u)| **u == burst)
            .nth(4)
            .unwrap()
            .0;
        assert!(fifth < 15, "A's fifth request served at position {fifth}");
    }
}
