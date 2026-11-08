//! The Rules matrix of Settings > Notifications (design.md 6.8, 5.8) without the view: one row
//! per event (and per event pattern a rule uses), one switch per channel. A row's switches
//! show the rule that decides it, exactly as the server applies them (`notify::plan`): the
//! user's own rule for exactly that event, else the instance default for it; a locked default
//! always wins. Turning a switch writes the user's own rule for the row (copying the default's
//! delivery mode the first time), so the default stays as the admin set it.
//! Rules for one list or person (a filter) are listed apart and never drive the matrix.

use contracts::{ChannelDto, EventTypeDto, SubscriptionDto};
use serde_json::{Value, json};

/// One matrix row.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleRow {
    /// An event id, `group.*` or `*`.
    pub event: String,
    /// The user's own rule for exactly this event, without a filter.
    pub own: Option<SubscriptionDto>,
    /// The instance default for exactly this event.
    pub default: Option<SubscriptionDto>,
}

impl RuleRow {
    /// The rule that decides this row: a locked default, else the own rule, else the default.
    #[must_use]
    pub fn deciding(&self) -> Option<&SubscriptionDto> {
        match (&self.own, &self.default) {
            (_, Some(d)) if d.locked => Some(d),
            (Some(o), _) => Some(o),
            (None, d) => d.as_ref(),
        }
    }

    /// Locked by the instance: no switches.
    #[must_use]
    pub fn locked(&self) -> bool {
        self.default.as_ref().is_some_and(|d| d.locked)
    }

    /// Decided by the instance default (labelled in the matrix).
    #[must_use]
    pub fn from_default(&self) -> bool {
        self.locked() || (self.own.is_none() && self.default.is_some())
    }

    /// Does this row reach `channel`?
    #[must_use]
    pub fn reaches(&self, channel: &str) -> bool {
        self.deciding().is_some_and(|s| sends_to(s, channel))
    }
}

/// Does the rule `s` send to `channel` (enabled, and all channels or this one)?
#[must_use]
pub fn sends_to(s: &SubscriptionDto, channel: &str) -> bool {
    s.enabled
        && match s.targets["type"].as_str() {
            Some("only") => names(s, channel),
            _ => true,
        }
}

/// Does the rule list `channel` by id?
fn names(s: &SubscriptionDto, channel: &str) -> bool {
    s.targets["ids"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|i| i.as_str() == Some(channel)))
}

/// Does the rule have a filter (one list, item, person or kind)?
#[must_use]
pub fn filtered(s: &SubscriptionDto) -> bool {
    s.filter.as_object().is_some_and(|m| {
        m.values()
            .any(|v| v.as_array().is_some_and(|a| !a.is_empty()))
    })
}

/// The matrix rows: the catalogue's events in its order, then any pattern (`release.*`,
/// `*`) an own rule or a default uses, in the order they appear.
#[must_use]
pub fn rows(events: &[EventTypeDto], subscriptions: &[SubscriptionDto]) -> Vec<RuleRow> {
    let mut ids: Vec<String> = events.iter().map(|e| e.id.clone()).collect();
    for s in subscriptions.iter().filter(|s| !filtered(s)) {
        if !ids.contains(&s.event) {
            ids.push(s.event.clone());
        }
    }
    ids.into_iter()
        .map(|event| {
            let pick = |default: bool| {
                subscriptions
                    .iter()
                    .find(|s| s.instance_default == default && !filtered(s) && s.event == event)
                    .cloned()
            };
            RuleRow {
                own: pick(false),
                default: pick(true),
                event,
            }
        })
        .collect()
}

/// The own rule to save when `channel` is switched `on` in `row` (`channels`: every channel
/// of the user, for rules that send to all of them; `zone`: for a rule made from scratch).
#[must_use]
pub fn toggled(
    row: &RuleRow,
    channels: &[ChannelDto],
    channel: &str,
    on: bool,
    zone: &str,
) -> SubscriptionDto {
    let mut base = match (&row.own, &row.default) {
        (Some(o), _) => o.clone(),
        (None, Some(d)) => SubscriptionDto {
            id: None,
            instance_default: false,
            locked: false,
            filter: Value::Null,
            ..d.clone()
        },
        (None, None) => SubscriptionDto {
            id: None,
            event: row.event.clone(),
            filter: Value::Null,
            targets: json!({"type": "only", "ids": []}),
            mode: json!({"mode": "immediate"}),
            quiet_hours: None,
            timezone: zone.to_string(),
            rate_limit_per_hour: None,
            enabled: true,
            locked: false,
            instance_default: false,
        },
    };
    let mut ids: Vec<String> = channels
        .iter()
        .filter(|c| sends_to(&base, &c.id))
        .map(|c| c.id.clone())
        .collect();
    ids.retain(|i| i != channel);
    if on {
        ids.push(channel.to_string());
    }
    base.enabled = true;
    base.targets = json!({"type": "only", "ids": ids});
    base
}

/// Rows the instance suggests (an unlocked default) that do not reach `channel` yet.
#[must_use]
pub fn suggested_off(rows: &[RuleRow], channel: &str) -> Vec<String> {
    rows.iter()
        .filter(|r| r.default.is_some() && !r.locked() && !r.reaches(channel))
        .map(|r| r.event.clone())
        .collect()
}

/// A new channel: no switchable row reaches it yet (a locked default reaches every channel
/// and does not count) and no own rule names it; its first switch then offers the instance
/// defaults (design.md 5.8).
#[must_use]
pub fn is_new(rows: &[RuleRow], own: &[SubscriptionDto], channel: &str) -> bool {
    !rows.iter().any(|r| !r.locked() && r.reaches(channel))
        && !own.iter().any(|s| names(s, channel))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: &str) -> EventTypeDto {
        EventTypeDto {
            id: id.into(),
            audience: "user".into(),
            args: vec![],
        }
    }

    fn sub(event: &str, targets: Value, default: bool, locked: bool) -> SubscriptionDto {
        SubscriptionDto {
            id: Some(format!("{event}-{default}")),
            event: event.into(),
            filter: Value::Null,
            targets,
            mode: json!({"mode": "digest", "cron": "0 8 * * *"}),
            quiet_hours: None,
            timezone: "Europe/Berlin".into(),
            rate_limit_per_hour: None,
            enabled: true,
            locked,
            instance_default: default,
        }
    }

    fn channel(id: &str) -> ChannelDto {
        ChannelDto {
            id: id.into(),
            name: id.into(),
            kind: "ntfy".into(),
            enabled: true,
            config: Value::Null,
            secret: None,
            health: contracts::ChannelHealthDto::default(),
        }
    }

    #[test]
    fn rows_follow_the_catalogue_then_the_patterns_and_the_deciding_rule() {
        let all = json!({"type": "all"});
        let subs = vec![
            sub(
                "import.finished",
                json!({"type": "only", "ids": ["a"]}),
                false,
                false,
            ),
            sub("import.finished", all.clone(), true, false),
            sub("admin.signup", all.clone(), true, true),
            sub(
                "admin.signup",
                json!({"type": "only", "ids": []}),
                false,
                false,
            ),
            sub("release.*", all.clone(), true, false),
            SubscriptionDto {
                filter: json!({"lists": ["l1"]}),
                ..sub("social.list_shared", all, false, false)
            },
        ];
        let events = [
            ev("import.finished"),
            ev("admin.signup"),
            ev("social.list_shared"),
        ];
        let rows = rows(&events, &subs);
        let names: Vec<_> = rows.iter().map(|r| r.event.as_str()).collect();
        assert_eq!(
            names,
            [
                "import.finished",
                "admin.signup",
                "social.list_shared",
                "release.*"
            ]
        );
        // the own rule overrides an unlocked default
        assert!(rows[0].reaches("a"));
        assert!(!rows[0].reaches("b"));
        assert!(!rows[0].from_default());
        // a locked default wins over the own rule
        assert!(rows[1].locked() && rows[1].reaches("b"));
        // a filtered rule does not drive the matrix
        assert_eq!(rows[2].own, None);
        assert!(!rows[2].reaches("a"));
        assert!(rows[3].from_default() && rows[3].reaches("a"));
    }

    #[test]
    fn a_switch_writes_the_own_rule_copying_the_default_once() {
        let chans = [channel("a"), channel("b")];
        let row = RuleRow {
            event: "import.finished".into(),
            own: None,
            default: Some(sub("import.finished", json!({"type": "all"}), true, false)),
        };
        let s = toggled(&row, &chans, "b", false, "UTC");
        assert_eq!(s.id, None);
        assert!(!s.instance_default);
        assert_eq!(s.targets, json!({"type": "only", "ids": ["a"]}));
        assert_eq!(s.mode["mode"], "digest", "the default's mode is kept");
        assert_eq!(s.timezone, "Europe/Berlin");
        let fresh = RuleRow {
            event: "import.failed".into(),
            own: None,
            default: None,
        };
        let s = toggled(&fresh, &chans, "a", true, "Pacific/Auckland");
        assert_eq!(s.targets, json!({"type": "only", "ids": ["a"]}));
        assert_eq!(s.mode, json!({"mode": "immediate"}));
        assert_eq!(s.timezone, "Pacific/Auckland");
        // a muted own rule switched on sends to that channel only
        let muted = RuleRow {
            own: Some(SubscriptionDto {
                enabled: false,
                ..sub("import.failed", json!({"type": "all"}), false, false)
            }),
            ..fresh
        };
        let s = toggled(&muted, &chans, "b", true, "UTC");
        assert!(s.enabled);
        assert_eq!(s.targets, json!({"type": "only", "ids": ["b"]}));
    }

    #[test]
    fn a_new_channel_is_offered_the_suggested_rows() {
        let only_a = json!({"type": "only", "ids": ["a"]});
        let subs = vec![
            sub("import.finished", json!({"type": "all"}), true, false),
            sub("import.finished", only_a.clone(), false, false),
            sub("import.failed", json!({"type": "all"}), true, true),
            sub("social.link_request", only_a, false, false),
        ];
        let events = [
            ev("import.finished"),
            ev("import.failed"),
            ev("social.link_request"),
        ];
        let rows = rows(&events, &subs);
        let own: Vec<_> = subs
            .iter()
            .filter(|s| !s.instance_default)
            .cloned()
            .collect();
        // b is reached by the locked default only: still new; own rules name a
        assert!(is_new(&rows, &own, "b"));
        assert!(!is_new(&rows, &own, "a"));
        assert_eq!(suggested_off(&rows, "b"), ["import.finished"]);
        assert_eq!(suggested_off(&rows, "a"), Vec::<String>::new());
    }
}
