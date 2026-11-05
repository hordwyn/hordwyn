//! Hybrid logical clock (D-12) and last-writer-wins fields.
//!
//! A timestamp is `(wall_ms, counter, node)`, ordered lexicographically. `node` breaks ties
//! between devices that produce the same `(wall_ms, counter)`.

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct Hlc {
    pub wall_ms: u64,
    pub counter: u32,
    pub node: u32,
}

impl Hlc {
    #[must_use]
    pub const fn new(wall_ms: u64, counter: u32, node: u32) -> Self {
        Self {
            wall_ms,
            counter,
            node,
        }
    }

    /// Next local timestamp after `self`, given the physical clock reading `now_ms`.
    #[must_use]
    pub fn tick(self, now_ms: u64) -> Self {
        if now_ms > self.wall_ms {
            Self::new(now_ms, 0, self.node)
        } else {
            Self::new(self.wall_ms, self.counter.saturating_add(1), self.node)
        }
    }

    /// Next local timestamp after receiving `remote`.
    #[must_use]
    pub fn receive(self, remote: Hlc, now_ms: u64) -> Self {
        let wall = now_ms.max(self.wall_ms).max(remote.wall_ms);
        let counter = if wall == self.wall_ms && wall == remote.wall_ms {
            self.counter.max(remote.counter).saturating_add(1)
        } else if wall == self.wall_ms {
            self.counter.saturating_add(1)
        } else if wall == remote.wall_ms {
            remote.counter.saturating_add(1)
        } else {
            0
        };
        Self::new(wall, counter, self.node)
    }
}

/// A mutable field with last-writer-wins semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lww<T> {
    pub value: T,
    pub stamp: Hlc,
}

impl<T> Lww<T> {
    pub const fn new(value: T, stamp: Hlc) -> Self {
        Self { value, stamp }
    }

    /// Apply a write. Returns `true` if it won (its stamp is newer than the current one).
    pub fn set(&mut self, value: T, stamp: Hlc) -> bool {
        if stamp > self.stamp {
            self.value = value;
            self.stamp = stamp;
            true
        } else {
            false
        }
    }
}

impl std::fmt::Display for Hlc {
    /// Sortable text form: `{wall_ms:016x}-{counter:08x}-{node:08x}`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:016x}-{:08x}-{:08x}",
            self.wall_ms, self.counter, self.node
        )
    }
}

impl std::str::FromStr for Hlc {
    type Err = crate::DomainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || crate::DomainError::invalid("hlc", format!("`{s}` is not an HLC stamp"));
        let mut parts = s.split('-');
        let (Some(w), Some(c), Some(n), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(bad());
        };
        Ok(Self::new(
            u64::from_str_radix(w, 16).map_err(|_| bad())?,
            u32::from_str_radix(c, 16).map_err(|_| bad())?,
            u32::from_str_radix(n, 16).map_err(|_| bad())?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_uses_wall_clock_when_it_advances() {
        let h = Hlc::new(100, 5, 1).tick(200);
        assert_eq!(h, Hlc::new(200, 0, 1));
    }

    #[test]
    fn tick_increments_counter_when_clock_stalls_or_goes_back() {
        assert_eq!(Hlc::new(100, 5, 1).tick(100), Hlc::new(100, 6, 1));
        assert_eq!(Hlc::new(100, 5, 1).tick(50), Hlc::new(100, 6, 1));
    }

    #[test]
    fn receive_is_strictly_greater_than_both_inputs() {
        let local = Hlc::new(100, 2, 1);
        let remote = Hlc::new(100, 7, 2);
        let next = local.receive(remote, 90);
        assert!(next > local && next.wall_ms == 100 && next.counter == 8);
        let ahead = Hlc::new(500, 3, 2);
        let next = local.receive(ahead, 90);
        assert_eq!((next.wall_ms, next.counter), (500, 4));
    }

    #[test]
    fn lww_keeps_newest_regardless_of_arrival_order() {
        let a = Hlc::new(10, 0, 1);
        let b = Hlc::new(10, 0, 2);
        let mut x = Lww::new("init", Hlc::default());
        let mut y = x.clone();
        x.set("a", a);
        x.set("b", b);
        y.set("b", b);
        y.set("a", a);
        assert_eq!(x, y);
        assert_eq!(x.value, "b");
    }
}

#[cfg(test)]
mod text_tests {
    use super::Hlc;

    #[test]
    fn text_form_roundtrips_and_sorts_like_the_struct() {
        let a = Hlc::new(1_700_000_000_000, 3, 9);
        let b = Hlc::new(1_700_000_000_000, 12, 1);
        assert_eq!(a.to_string().parse::<Hlc>().unwrap(), a);
        assert!(a < b && a.to_string() < b.to_string());
        assert!("zz".parse::<Hlc>().is_err());
    }
}
