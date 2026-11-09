//! Keyboard logic without a DOM (design.md 3.2, 8): where a roving focus moves in a grid laid
//! out in columns, how many columns a rendered grid has, and the state machine of a two-key
//! chord such as `G then L`. The pages wire these to real elements; the rules live here so
//! they can be tested on their own.

/// Where the roving focus of a grid of `len` cells in `columns` columns moves for `key`
/// (APG grid pattern): arrows move by one cell or one row and stop at the edges, `Home` and
/// `End` go to the start and end of the row, with `ctrl` to the first and last cell. `None`:
/// not a navigation key (the caller leaves the event alone); `Some(i)`: the cell to focus,
/// which is `current` again at an edge (the caller still swallows the key, so the page does
/// not scroll instead).
#[must_use]
pub fn roving_target(
    current: usize,
    len: usize,
    columns: usize,
    key: &str,
    ctrl: bool,
) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let cols = columns.clamp(1, len);
    let at = current.min(len - 1);
    let row_start = at - at % cols;
    let target = match key {
        "ArrowRight" => (at + 1).min(len - 1),
        "ArrowLeft" => at.saturating_sub(1),
        "ArrowDown" if at + cols < len => at + cols,
        "ArrowUp" if at >= cols => at - cols,
        "ArrowDown" | "ArrowUp" => at,
        "Home" if ctrl => 0,
        "End" if ctrl => len - 1,
        "Home" => row_start,
        "End" => (row_start + cols - 1).min(len - 1),
        _ => return None,
    };
    Some(target)
}

/// The number of columns of a rendered grid from the top edges of its cells in order: the
/// cells of the first row share the first cell's top (within one pixel). At least 1.
#[must_use]
pub fn columns_from_tops(tops: &[f64]) -> usize {
    let Some(first) = tops.first() else {
        return 1;
    };
    tops.iter()
        .take_while(|t| (*t - first).abs() <= 1.0)
        .count()
        .max(1)
}

/// What a key did to a [`Chord`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChordStep {
    /// Not part of a chord: the caller handles the key as usual.
    Ignored,
    /// The leader key: the chord waits for its second key.
    Started,
    /// The second key of a started chord (lower case).
    Done(char),
    /// A key that ends a started chord without a match.
    Cancelled,
}

/// A two-key chord: a leader key, then one of `followers` within `window_ms`. Keys are
/// compared without case; modifier keys pressed alone (`Shift` ...) keep the chord waiting.
#[derive(Debug, Clone)]
pub struct Chord {
    leader: char,
    followers: &'static [char],
    window_ms: f64,
    since: Option<f64>,
}

impl Chord {
    #[must_use]
    pub fn new(leader: char, followers: &'static [char], window_ms: f64) -> Self {
        Self {
            leader,
            followers,
            window_ms,
            since: None,
        }
    }

    /// Feed one key (`KeyboardEvent.key`) pressed at `now` (milliseconds).
    pub fn key(&mut self, key: &str, now: f64) -> ChordStep {
        if matches!(key, "Shift" | "Control" | "Alt" | "Meta" | "CapsLock") {
            return ChordStep::Ignored;
        }
        let mut chars = key.chars();
        let one = match (chars.next(), chars.next()) {
            (Some(c), None) => Some(c.to_ascii_lowercase()),
            _ => None,
        };
        let waiting = self.pending(now);
        self.since = None;
        if one == Some(self.leader) {
            self.since = Some(now);
            return ChordStep::Started;
        }
        if !waiting {
            return ChordStep::Ignored;
        }
        match one {
            Some(c) if self.followers.contains(&c) => ChordStep::Done(c),
            _ => ChordStep::Cancelled,
        }
    }

    /// Whether the chord waits for its second key at `now`.
    #[must_use]
    pub fn pending(&self, now: f64) -> bool {
        self.since.is_some_and(|t| now - t <= self.window_ms)
    }

    /// Stop waiting (the window ran out, focus moved into a field ...).
    pub fn cancel(&mut self) {
        self.since = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GO: &[char] = &['h', 'l', 'y', 't', 'p'];

    #[test]
    fn arrows_move_by_cell_and_row_and_stop_at_the_edges() {
        // 7 cells in 3 columns:  0 1 2 / 3 4 5 / 6
        assert_eq!(roving_target(0, 7, 3, "ArrowRight", false), Some(1));
        assert_eq!(roving_target(2, 7, 3, "ArrowRight", false), Some(3));
        assert_eq!(roving_target(6, 7, 3, "ArrowRight", false), Some(6));
        assert_eq!(roving_target(0, 7, 3, "ArrowLeft", false), Some(0));
        assert_eq!(roving_target(3, 7, 3, "ArrowLeft", false), Some(2));
        assert_eq!(roving_target(1, 7, 3, "ArrowDown", false), Some(4));
        assert_eq!(roving_target(3, 7, 3, "ArrowDown", false), Some(6));
        // nothing below 4 and 5 in the short last row: stay
        assert_eq!(roving_target(4, 7, 3, "ArrowDown", false), Some(4));
        assert_eq!(roving_target(4, 7, 3, "ArrowUp", false), Some(1));
        assert_eq!(roving_target(1, 7, 3, "ArrowUp", false), Some(1));
    }

    #[test]
    fn home_and_end_go_to_the_row_ends_and_with_ctrl_to_the_grid_ends() {
        assert_eq!(roving_target(4, 7, 3, "Home", false), Some(3));
        assert_eq!(roving_target(4, 7, 3, "End", false), Some(5));
        assert_eq!(roving_target(6, 7, 3, "End", false), Some(6));
        assert_eq!(roving_target(4, 7, 3, "Home", true), Some(0));
        assert_eq!(roving_target(1, 7, 3, "End", true), Some(6));
    }

    #[test]
    fn other_keys_empty_grids_and_odd_columns() {
        assert_eq!(roving_target(0, 7, 3, "Enter", false), None);
        assert_eq!(roving_target(0, 7, 3, "w", false), None);
        assert_eq!(roving_target(0, 0, 3, "ArrowRight", false), None);
        // zero columns counts as one; more columns than cells as one row
        assert_eq!(roving_target(0, 3, 0, "ArrowDown", false), Some(1));
        assert_eq!(roving_target(0, 3, 8, "ArrowDown", false), Some(0));
        assert_eq!(roving_target(0, 3, 8, "End", false), Some(2));
        // a stale index after the grid shrank lands on the last cell
        assert_eq!(roving_target(9, 3, 2, "ArrowLeft", false), Some(1));
    }

    #[test]
    fn columns_come_from_the_first_row() {
        assert_eq!(columns_from_tops(&[]), 1);
        assert_eq!(columns_from_tops(&[10.0]), 1);
        assert_eq!(columns_from_tops(&[10.0, 10.0, 10.4, 300.0, 300.0]), 3);
        assert_eq!(columns_from_tops(&[10.0, 300.0, 600.0]), 1);
        assert_eq!(columns_from_tops(&[5.0, 5.0, 5.0]), 3);
    }

    #[test]
    fn a_chord_is_the_leader_then_a_follower_within_the_window() {
        let mut c = Chord::new('g', GO, 1500.0);
        assert_eq!(c.key("x", 0.0), ChordStep::Ignored);
        assert!(!c.pending(0.0));
        assert_eq!(c.key("g", 100.0), ChordStep::Started);
        assert!(c.pending(1000.0));
        assert_eq!(c.key("L", 1200.0), ChordStep::Done('l'));
        assert!(!c.pending(1200.0));
        // a modifier alone keeps it waiting, any other key ends it
        assert_eq!(c.key("G", 2000.0), ChordStep::Started);
        assert_eq!(c.key("Shift", 2100.0), ChordStep::Ignored);
        assert!(c.pending(2100.0));
        assert_eq!(c.key("q", 2200.0), ChordStep::Cancelled);
        assert!(!c.pending(2200.0));
        assert_eq!(c.key("l", 2300.0), ChordStep::Ignored);
    }

    #[test]
    fn the_window_runs_out_and_cancel_stops_waiting() {
        let mut c = Chord::new('g', GO, 1500.0);
        assert_eq!(c.key("g", 0.0), ChordStep::Started);
        assert!(!c.pending(1600.0));
        // too late: the follower is an ordinary key again
        assert_eq!(c.key("h", 1600.0), ChordStep::Ignored);
        assert_eq!(c.key("g", 2000.0), ChordStep::Started);
        // the leader again while waiting starts over
        assert_eq!(c.key("g", 2500.0), ChordStep::Started);
        assert!(c.pending(3900.0));
        c.cancel();
        assert!(!c.pending(3900.0));
        assert_eq!(c.key("p", 3950.0), ChordStep::Ignored);
    }
}
