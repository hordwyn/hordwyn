//! Why a search finds nothing, said the same way in three places (design.md 4.4, 6.1, 6.4):
//! the palette footer (source status dots with words and one action each), the palette's
//! no-results card (the reason in a sentence) and the admin nudge on Home (one status-first
//! line with one action, snoozable for 7 days). Pure functions over the server's
//! `SourceStatusPageDto` and `SearchResponseDto`, unit tested here; the views stay thin.

use contracts::admin::{SkippedSourceDto, SourceStatusPageDto};

/// Snooze length of the Home nudge (design.md 4.4).
pub const SNOOZE_MS: f64 = 7.0 * 24.0 * 3600.0 * 1000.0;

/// Where an admin sets up sources.
pub const SET_UP_HREF: &str = "/admin/sources";

/// The one action of a footer row or reason line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintAction {
    /// Admins: a link to Admin > Data sources.
    SetUp,
    /// Members: "Ask admin" about a whole kind group.
    AskGroup(String),
}

/// One row of the palette footer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FooterItem {
    /// A source that answers or has trouble: dot, name and (unless working) its state word.
    Source {
        id: String,
        name: String,
        /// The kinds the source serves (named on a row that waits for the operator contact).
        kinds: Vec<String>,
        state: String,
        action: Option<HintAction>,
    },
    /// A kind group with no working source at all ("Board games: no source set up").
    Group {
        group: String,
        action: Option<HintAction>,
    },
}

/// States that mean "answers" (possibly slowly) or only lacks the member's own account.
fn serving(state: &str) -> bool {
    matches!(
        state,
        "working" | "rate_limited" | "slow" | "needs_account" | "retrying"
    )
}

/// A source the admin should look at: it is set up (or meant to be) but does not answer.
fn failing(state: &str) -> bool {
    matches!(
        state,
        "key_rejected"
            | "key_unavailable"
            | "circuit_open"
            | "unavailable"
            | "retrying"
            | "needs_contact"
    )
}

fn group_action(admin: bool, group: &str) -> HintAction {
    if admin {
        HintAction::SetUp
    } else {
        HintAction::AskGroup(group.to_string())
    }
}

/// The footer rows: every source that serves or fails (not-set-up sources of groups that
/// another source serves are left out, they explain nothing), then the groups without a
/// source. Disabled sources never show, and nothing shows while every source works
/// (addendum-1 6.4, D14).
#[must_use]
pub fn footer_items(status: &SourceStatusPageDto) -> Vec<FooterItem> {
    let admin = status.is_admin;
    let mut out: Vec<FooterItem> = status
        .sources
        .iter()
        .filter(|s| serving(&s.state) || failing(&s.state))
        .map(|s| FooterItem::Source {
            id: s.id.clone(),
            name: s.name.clone(),
            kinds: s.kinds.clone(),
            state: s.state.clone(),
            action: (admin && failing(&s.state)).then_some(HintAction::SetUp),
        })
        .collect();
    out.extend(
        status
            .groups_without_source
            .iter()
            .map(|g| FooterItem::Group {
                group: g.clone(),
                action: Some(group_action(admin, g)),
            }),
    );
    // addendum-1 6.4 (D14): nothing to say while every source works
    let all_working = out
        .iter()
        .all(|f| matches!(f, FooterItem::Source { state, .. } if state == "working"));
    if all_working {
        out.clear();
    }
    out
}

/// Why the palette shows no results (the no-results card, design.md 5.3 and 6.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoResults {
    /// No source was asked at all: "Search is not set up yet"; the one action, if any.
    NotSetUp { action: Option<HintAction> },
    /// Some groups or sources were not searched: say which and why.
    Explained {
        searched: Vec<String>,
        groups: Vec<(String, HintAction)>,
        skipped: Vec<SkippedSourceDto>,
    },
    /// Everything was searched and nothing matched.
    NothingFound { searched: Vec<String> },
}

/// The reason for an empty result. `asked` are the names of the sources the search asked
/// (the response's `pending`, by name); the library is always searched.
#[must_use]
pub fn no_results(
    admin: bool,
    asked: &[String],
    skipped: &[SkippedSourceDto],
    groups_without_source: &[String],
) -> NoResults {
    if asked.is_empty() && (!groups_without_source.is_empty() || !skipped.is_empty()) {
        // a member can only ask about a group: with none named there is no action (UX2-W5)
        let action = match groups_without_source.first() {
            Some(g) => Some(group_action(admin, g)),
            None => admin.then_some(HintAction::SetUp),
        };
        return NoResults::NotSetUp { action };
    }
    if groups_without_source.is_empty() && skipped.is_empty() {
        return NoResults::NothingFound {
            searched: asked.to_vec(),
        };
    }
    NoResults::Explained {
        searched: asked.to_vec(),
        groups: groups_without_source
            .iter()
            .map(|g| (g.clone(), group_action(admin, g)))
            .collect(),
        skipped: skipped.to_vec(),
    }
}

/// The Home nudge for admins (design.md 6.1; DEC-237): the one thing to do first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nudge {
    /// The setup wizard was left before the end.
    Setup,
    /// Whole kind groups have no working source.
    NoSource(Vec<String>),
    /// Sources that are set up but do not answer (names).
    Failing(Vec<String>),
    /// Sources that are not asked until the operator contact is set (names).
    NeedsContact(Vec<String>),
}

impl Nudge {
    /// Where its one action leads.
    #[must_use]
    pub fn href(&self) -> &'static str {
        match self {
            Self::Setup => "/setup",
            Self::NoSource(_) | Self::Failing(_) | Self::NeedsContact(_) => SET_UP_HREF,
        }
    }

    /// Can the admin answer "Don't remind me"? Only for the operator contact: not giving it
    /// is a choice (the consent `Required` asks for), the other nudges are things to fix
    /// (WD-10, LB-40).
    #[must_use]
    pub fn can_decline(&self) -> bool {
        matches!(self, Self::NeedsContact(_))
    }

    /// Identity for the snooze: a new problem shows again at once.
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Setup => "setup".into(),
            Self::NoSource(g) => format!("groups:{}", g.join(",")),
            Self::Failing(s) => format!("failing:{}", s.join(",")),
            Self::NeedsContact(s) => format!("contact:{}", s.join(",")),
        }
    }
}

/// The nudge to show, if any: never for members; an open setup first, then sources waiting for
/// the operator contact, then groups without a source, then failing sources.
#[must_use]
pub fn nudge(status: &SourceStatusPageDto) -> Option<Nudge> {
    if !status.is_admin {
        return None;
    }
    if status.setup_open {
        return Some(Nudge::Setup);
    }
    let names = |contact: bool| -> Vec<String> {
        status
            .sources
            .iter()
            .filter(|s| failing(&s.state) && (s.state == "needs_contact") == contact)
            .map(|s| s.name.clone())
            .collect()
    };
    // a source that waits for the contact is not broken: it is named on its own, before a
    // group without a source (the contact is one field, and it may be why the group has none)
    let waiting = names(true);
    if !waiting.is_empty() {
        return Some(Nudge::NeedsContact(waiting));
    }
    if !status.groups_without_source.is_empty() {
        return Some(Nudge::NoSource(status.groups_without_source.clone()));
    }
    let failing = names(false);
    (!failing.is_empty()).then_some(Nudge::Failing(failing))
}

/// The stored snooze (`<key>|<until ms>`) covers `key` at `now_ms`.
#[must_use]
pub fn snoozed(stored: Option<&str>, key: &str, now_ms: f64) -> bool {
    stored
        .and_then(|s| s.rsplit_once('|'))
        .and_then(|(k, until)| Some((k, until.parse::<f64>().ok()?)))
        .is_some_and(|(k, until)| k == key && now_ms < until)
}

/// Did the admin answer "Don't remind me" to the nudge `key` in this browser (`stored` is the
/// key they declined)? A new problem has another key and shows.
#[must_use]
pub fn declined(stored: Option<&str>, key: &str) -> bool {
    stored == Some(key)
}

/// The value to store when the admin snoozes `key` at `now_ms`.
#[must_use]
pub fn snooze_value(key: &str, now_ms: f64) -> String {
    format!("{key}|{}", now_ms + SNOOZE_MS)
}

#[cfg(test)]
mod tests {
    use contracts::admin::SourceStatusDto;

    use super::*;

    fn src(id: &str, state: &str) -> SourceStatusDto {
        SourceStatusDto {
            id: id.into(),
            name: id.to_uppercase(),
            kinds: vec![],
            groups: vec![],
            state: state.into(),
            credential: None,
            env_var: None,
            calls_24h: 0,
            rate_limited_24h: 0,
            last_ok_at: None,
            last_error_at: None,
            last_error: None,
            last_auth_status: None,
            retry_at: None,
            user_fields: vec![],
            user_credential_set: false,
        }
    }

    fn page(admin: bool, sources: Vec<SourceStatusDto>, groups: &[&str]) -> SourceStatusPageDto {
        SourceStatusPageDto {
            sources,
            groups_without_source: groups.iter().map(|g| (*g).to_string()).collect(),
            is_admin: admin,
            setup_open: false,
        }
    }

    fn skipped(id: &str, state: &str) -> SkippedSourceDto {
        SkippedSourceDto {
            id: id.into(),
            name: id.to_uppercase(),
            state: state.into(),
        }
    }

    /// v1-books 3.1.1: a source that waits for the operator contact is named, with Set up.
    #[test]
    fn a_source_without_the_operator_contact_is_shown_with_set_up() {
        let mut wikidata = src("wikidata", "needs_contact");
        wikidata.kinds = vec!["board_game".into()];
        let sources = vec![src("tmdb", "working"), wikidata];
        let admin = footer_items(&page(true, sources, &[]));
        assert_eq!(admin.len(), 2);
        assert!(matches!(
            &admin[1],
            FooterItem::Source { state, action: Some(HintAction::SetUp), .. } if state == "needs_contact"
        ));
        // v1-wikidata: the row says what waits for it ("Board game (WIKIDATA)")
        assert!(matches!(&admin[1], FooterItem::Source { kinds, .. } if kinds == &["board_game"]));
        assert_eq!(
            crate::admin_state::source_state("needs_contact"),
            ui::StatusState::Bad
        );
    }

    #[test]
    fn the_footer_names_answering_and_failing_sources_then_groups_without_one() {
        let sources = vec![
            src("tmdb", "working"),
            src("tvdb", "not_set_up"),
            src("igdb", "key_rejected"),
            src("podcastindex", "rate_limited"),
            src("wikidata", "disabled"),
        ];
        let admin = footer_items(&page(true, sources.clone(), &["play"]));
        let ids: Vec<String> = admin
            .iter()
            .map(|f| match f {
                FooterItem::Source { id, .. } => id.clone(),
                FooterItem::Group { group, .. } => format!("group:{group}"),
            })
            .collect();
        assert_eq!(ids, ["tmdb", "igdb", "podcastindex", "group:play"]);
        assert!(matches!(
            &admin[1],
            FooterItem::Source {
                action: Some(HintAction::SetUp),
                ..
            }
        ));
        assert!(matches!(&admin[0], FooterItem::Source { action: None, .. }));
        assert!(matches!(
            &admin[3],
            FooterItem::Group {
                action: Some(HintAction::SetUp),
                ..
            }
        ));
        let member = footer_items(&page(false, sources, &["play"]));
        assert!(
            matches!(&member[1], FooterItem::Source { action: None, .. }),
            "no detail action"
        );
        assert_eq!(
            member[3],
            FooterItem::Group {
                group: "play".into(),
                action: Some(HintAction::AskGroup("play".into()))
            }
        );
    }

    #[test]
    fn the_footer_stays_away_while_every_source_works() {
        // addendum-1 6.4 (D14): status only while a source is not Working
        let p = page(
            true,
            vec![src("tmdb", "working"), src("tvdb", "not_set_up")],
            &[],
        );
        assert_eq!(footer_items(&p).len(), 0);
        let p = page(
            false,
            vec![src("tmdb", "working"), src("igdb", "slow")],
            &[],
        );
        assert_eq!(footer_items(&p).len(), 2, "one source is slow: show all");
        let p = page(false, vec![src("tmdb", "working")], &["play"]);
        assert_eq!(footer_items(&p).len(), 2, "a group without a source");
    }

    #[test]
    fn an_empty_search_always_has_a_reason() {
        let asked = vec!["TMDB".to_string()];
        assert_eq!(
            no_results(false, &asked, &[], &[]),
            NoResults::NothingFound {
                searched: asked.clone()
            }
        );
        let r = no_results(
            false,
            &asked,
            &[skipped("igdb", "not_set_up")],
            &["play".into()],
        );
        assert_eq!(
            r,
            NoResults::Explained {
                searched: asked.clone(),
                groups: vec![("play".into(), HintAction::AskGroup("play".into()))],
                skipped: vec![skipped("igdb", "not_set_up")],
            }
        );
        let r = no_results(true, &asked, &[], &["read".into()]);
        assert!(
            matches!(r, NoResults::Explained { ref groups, .. } if groups[0].1 == HintAction::SetUp)
        );
        // nothing asked at all: search is not set up
        assert_eq!(
            no_results(
                true,
                &[],
                &[skipped("tmdb", "not_set_up")],
                &["watch".into()]
            ),
            NoResults::NotSetUp {
                action: Some(HintAction::SetUp)
            }
        );
        assert_eq!(
            no_results(false, &[], &[], &["watch".into()]),
            NoResults::NotSetUp {
                action: Some(HintAction::AskGroup("watch".into()))
            }
        );
        // UX2-W5: a member never gets the admin link; with no group named there is
        // nothing to ask about
        assert_eq!(
            no_results(false, &[], &[skipped("tmdb", "not_set_up")], &[]),
            NoResults::NotSetUp { action: None }
        );
    }

    #[test]
    fn the_nudge_is_for_admins_and_names_the_first_problem() {
        let mut p = page(false, vec![src("tmdb", "key_rejected")], &["play"]);
        p.setup_open = true;
        assert_eq!(nudge(&p), None, "never for members");
        p.is_admin = true;
        assert_eq!(nudge(&p), Some(Nudge::Setup));
        assert_eq!(Nudge::Setup.href(), "/setup");
        p.setup_open = false;
        assert_eq!(nudge(&p), Some(Nudge::NoSource(vec!["play".into()])));
        p.groups_without_source.clear();
        let n = nudge(&p).unwrap();
        assert_eq!(n, Nudge::Failing(vec!["TMDB".into()]));
        assert_eq!(n.href(), "/admin/sources");
        p.sources = vec![src("tmdb", "working"), src("tvdb", "not_set_up")];
        assert_eq!(nudge(&p), None, "nothing to do: no nudge");
    }

    /// v1-wikidata: a source that waits for the operator contact is named on its own, before
    /// sources that do not answer (it is not broken, it is not asked).
    #[test]
    fn the_nudge_names_a_source_that_waits_for_the_operator_contact() {
        let mut p = page(
            true,
            vec![
                src("tmdb", "key_rejected"),
                src("wikidata", "needs_contact"),
            ],
            &[],
        );
        let n = nudge(&p).unwrap();
        assert_eq!(n, Nudge::NeedsContact(vec!["WIKIDATA".into()]));
        assert_eq!(n.href(), "/admin/sources");
        assert_eq!(n.key(), "contact:WIKIDATA");
        p.sources.remove(1);
        assert_eq!(nudge(&p), Some(Nudge::Failing(vec!["TMDB".into()])));
        // before a group without a source: the contact is one field, and it may be the reason
        p.groups_without_source = vec!["play".into()];
        assert_eq!(nudge(&p), Some(Nudge::NoSource(vec!["play".into()])));
        p.sources = vec![src("wikidata", "needs_contact")];
        assert_eq!(
            nudge(&p),
            Some(Nudge::NeedsContact(vec!["WIKIDATA".into()]))
        );
        p.is_admin = false;
        assert_eq!(nudge(&p), None, "never for members");
    }

    #[test]
    fn a_snooze_lasts_seven_days_and_only_for_the_same_problem() {
        let n = Nudge::NoSource(vec!["play".into()]);
        let stored = snooze_value(&n.key(), 1000.0);
        assert!(snoozed(Some(&stored), &n.key(), 1000.0 + SNOOZE_MS - 1.0));
        assert!(!snoozed(Some(&stored), &n.key(), 1000.0 + SNOOZE_MS));
        let other = Nudge::NoSource(vec!["play".into(), "read".into()]);
        assert!(
            !snoozed(Some(&stored), &other.key(), 2000.0),
            "a new problem shows"
        );
        assert!(!snoozed(None, &n.key(), 0.0));
        assert!(!snoozed(Some("garbage"), &n.key(), 0.0));
    }

    /// WD-10: an admin who decides not to give the operator contact can answer the nudge
    /// "Don't remind me"; it stays hidden for that source in this browser (not for a new
    /// problem). The other nudges are things to fix and can only be snoozed.
    #[test]
    fn only_the_contact_nudge_can_be_declined_and_a_declined_one_stays_hidden() {
        let contact = Nudge::NeedsContact(vec!["Wikidata".into()]);
        assert!(contact.can_decline());
        for n in [
            Nudge::Setup,
            Nudge::NoSource(vec!["play".into()]),
            Nudge::Failing(vec!["TMDB".into()]),
        ] {
            assert!(!n.can_decline(), "{n:?}");
        }
        let stored = contact.key();
        assert!(declined(Some(&stored), &contact.key()));
        let two = Nudge::NeedsContact(vec!["Wikidata".into(), "Other".into()]);
        assert!(!declined(Some(&stored), &two.key()), "a new problem shows");
        assert!(!declined(None, &contact.key()));
    }
}
