//! Circuit breaker per connector: after `threshold` consecutive failures the circuit opens
//! for `cooldown`; then one trial request is let through (half-open).

use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Closed { failures: u32 },
    Open { until: Instant },
    HalfOpen { trial_running: bool },
}

pub struct Breaker {
    state: Mutex<State>,
    threshold: u32,
    cooldown: Duration,
}

impl Breaker {
    #[must_use]
    pub fn new(threshold: u32, cooldown: Duration) -> Self {
        Self {
            state: Mutex::new(State::Closed { failures: 0 }),
            threshold,
            cooldown,
        }
    }

    /// May a request be sent now?
    ///
    /// # Panics
    /// If the internal mutex is poisoned.
    pub fn allow(&self) -> bool {
        let mut s = self.state.lock().expect("breaker lock");
        match *s {
            State::Closed { .. } => true,
            State::Open { until } if Instant::now() >= until => {
                *s = State::HalfOpen {
                    trial_running: true,
                };
                true
            }
            State::Open { .. }
            | State::HalfOpen {
                trial_running: true,
            } => false,
            State::HalfOpen {
                trial_running: false,
            } => {
                *s = State::HalfOpen {
                    trial_running: true,
                };
                true
            }
        }
    }

    /// # Panics
    /// If the internal mutex is poisoned.
    pub fn success(&self) {
        *self.state.lock().expect("breaker lock") = State::Closed { failures: 0 };
    }

    /// # Panics
    /// If the internal mutex is poisoned.
    pub fn failure(&self) {
        let mut s = self.state.lock().expect("breaker lock");
        *s = match *s {
            State::Closed { failures } if failures + 1 < self.threshold => State::Closed {
                failures: failures + 1,
            },
            _ => {
                tracing::warn!("circuit opened");
                State::Open {
                    until: Instant::now() + self.cooldown,
                }
            }
        };
    }

    /// # Panics
    /// If the internal mutex is poisoned.
    #[must_use]
    pub fn is_open(&self) -> bool {
        matches!(
            *self.state.lock().expect("breaker lock"),
            State::Open { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_after_threshold_then_half_opens() {
        let b = Breaker::new(3, Duration::from_millis(30));
        for _ in 0..2 {
            b.failure();
            assert!(b.allow());
        }
        b.failure();
        assert!(!b.allow(), "open");
        std::thread::sleep(Duration::from_millis(40));
        assert!(b.allow(), "trial request");
        assert!(!b.allow(), "only one trial");
        b.failure();
        assert!(!b.allow(), "trial failed -> open again");
        std::thread::sleep(Duration::from_millis(40));
        assert!(b.allow());
        b.success();
        assert!(b.allow() && b.allow());
    }
}
