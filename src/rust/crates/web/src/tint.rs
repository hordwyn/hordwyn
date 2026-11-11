//! Which tint a group takes (owner test OT-13, DEC-540). The colours are theme tokens of
//! the brand; a kind outside the four groups stays grey.

use ui::{SegmentedOption, Tint};

/// The tint of an activity group id (`watch`, `read`, `listen`, `play`); `other` and
/// anything unknown is neutral.
#[must_use]
pub fn group_tint(group: &str) -> Tint {
    match group {
        "watch" => Tint::Watch,
        "read" => Tint::Read,
        "listen" => Tint::Listen,
        "play" => Tint::Play,
        _ => Tint::Neutral,
    }
}

/// The tint of a chart on a group's tab; all groups together take the accent.
#[must_use]
pub fn chart_tint(group: &str) -> Tint {
    match group_tint(group) {
        Tint::Neutral => Tint::Accent,
        t => t,
    }
}

/// An option of a group switcher: the four groups get their marker dot, "All" and "Other"
/// none.
#[must_use]
pub fn group_option(id: &str, label: String) -> SegmentedOption {
    let o = SegmentedOption::new(id, label);
    match group_tint(id) {
        Tint::Neutral => o,
        t => o.tint(t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_groups_have_a_tint_and_the_rest_is_grey() {
        assert_eq!(group_tint("watch"), Tint::Watch);
        assert_eq!(group_tint("read"), Tint::Read);
        assert_eq!(group_tint("listen"), Tint::Listen);
        assert_eq!(group_tint("play"), Tint::Play);
        assert_eq!(group_tint("other"), Tint::Neutral);
        assert_eq!(group_tint("all"), Tint::Neutral);
        assert_eq!(group_tint(""), Tint::Neutral);
    }

    #[test]
    fn a_chart_of_all_groups_takes_the_accent() {
        assert_eq!(chart_tint("all"), Tint::Accent);
        assert_eq!(chart_tint("other"), Tint::Accent);
        assert_eq!(chart_tint("read"), Tint::Read);
    }

    #[test]
    fn group_options_carry_the_marker_of_their_group_only() {
        assert_eq!(group_option("play", "Play".into()).tint, Some(Tint::Play));
        assert_eq!(group_option("all", "All".into()).tint, None);
        assert_eq!(group_option("other", "Other".into()).tint, None);
    }
}
