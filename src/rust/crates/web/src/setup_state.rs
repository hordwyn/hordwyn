//! State of the setup wizard page (design.md 4.3, 5.1; addendum-1 5.1 and D12): which step
//! is shown, where each step's progress marker stands, which source cards start open and
//! where each item of the Done checklist leads. The server keeps the wizard's state
//! (`SetupStateDto`, resumable); these pure functions only map it to the page.

use contracts::admin::{SetupSourceDto, SetupStateDto};

/// The wizard steps in order (`SetupStateDto::steps` uses the same ids).
pub const STEPS: [&str; 4] = ["account", "hosting", "sources", "done"];

/// Hosting modes, the default first (owner ruling: home; the wizard asks).
pub const HOSTING: [&str; 2] = ["home", "hosted"];

/// Message key of a step's name in the progress list.
#[must_use]
pub fn step_label_key(step: &str) -> &'static str {
    match step {
        "account" => "setup-step-account",
        "hosting" => "setup-step-hosting",
        "sources" => "setup-step-sources",
        _ => "setup-step-done",
    }
}

/// Zero-based position of `step` in `steps` (unknown steps count as the first).
#[must_use]
pub fn step_index(steps: &[String], step: &str) -> usize {
    steps.iter().position(|s| s == step).unwrap_or(0)
}

/// The step the page shows: the server's step, or the one the admin went back to (only
/// steps before the server's are reachable; the account step never again once it exists).
#[must_use]
pub fn shown_step<'a>(state: &'a SetupStateDto, back_to: Option<&'a str>) -> &'a str {
    let current = step_index(&state.steps, &state.step);
    match back_to {
        Some(b) if b != "account" && step_index(&state.steps, b) < current => b,
        _ => state.step.as_str(),
    }
}

/// A source asks for a key the admin can type here (not set by the environment, not free).
#[must_use]
pub fn needs_key(s: &SetupSourceDto) -> bool {
    !matches!(s.credential.as_str(), "env" | "not_needed") && !s.fields.is_empty()
}

/// Source cards that start open (addendum-1 5.1): the first two that still need a key; the
/// others are one disclosure row each.
#[must_use]
pub fn open_sources(sources: &[SetupSourceDto]) -> Vec<String> {
    // a key the provider rejected first (UX2-6), then the sources without a key
    let with = |state: &'static str| {
        sources
            .iter()
            .filter(move |s| needs_key(s) && s.credential == state)
    };
    with("invalid")
        .chain(with("missing"))
        .take(2)
        .map(|s| s.id.clone())
        .collect()
}

/// Where a checklist item of the Done step leads (design.md 5.1).
#[must_use]
pub fn checklist_href(id: &str) -> &'static str {
    match id {
        "first_item" => "/library",
        "invite" => "/people",
        "media_server" => "/settings/integrations",
        "notification_defaults" => "/admin/notifications",
        "backup_key" => "/admin/instance",
        "identity_provider" => "/admin/sso",
        _ => "/admin",
    }
}

/// Where the admin lands after the wizard (design.md 4.3): hosted instances go on to
/// Users and sign-up to close sign-up or invite people; home instances go Home.
#[must_use]
pub fn landing(hosting: &str) -> &'static str {
    if hosting == "hosted" {
        "/admin/users"
    } else {
        "/"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(step: &str) -> SetupStateDto {
        SetupStateDto {
            step: step.into(),
            steps: STEPS.iter().map(|s| (*s).to_string()).collect(),
            hosting: "home".into(),
            hosting_fixed: false,
            sources: vec![],
            sources_read_only: false,
            checklist: vec![],
            done: vec![],
            key_file: None,
        }
    }

    fn source(id: &str, credential: &str, fields: &[&str]) -> SetupSourceDto {
        SetupSourceDto {
            id: id.into(),
            name: id.to_uppercase(),
            kinds: vec!["movie".into()],
            fields: fields.iter().map(|f| (*f).to_string()).collect(),
            credential: credential.into(),
            env_var: None,
            agreement_on_file: None,
        }
    }

    #[test]
    fn the_progress_marker_follows_the_servers_step() {
        let steps: Vec<String> = STEPS.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(step_index(&steps, "account"), 0);
        assert_eq!(step_index(&steps, "sources"), 2);
        assert_eq!(step_index(&steps, "done"), 3);
        assert_eq!(step_index(&steps, "bogus"), 0);
        assert_eq!(step_label_key("hosting"), "setup-step-hosting");
    }

    #[test]
    fn back_reaches_earlier_steps_but_never_the_account_again() {
        let s = state("sources");
        assert_eq!(shown_step(&s, None), "sources");
        assert_eq!(shown_step(&s, Some("hosting")), "hosting");
        assert_eq!(
            shown_step(&s, Some("account")),
            "sources",
            "the account exists"
        );
        assert_eq!(
            shown_step(&s, Some("done")),
            "sources",
            "not ahead of the server"
        );
        let s = state("account");
        assert_eq!(shown_step(&s, Some("hosting")), "account");
    }

    #[test]
    fn the_first_two_sources_that_need_a_key_start_open() {
        let list = vec![
            source("tmdb", "env", &["api_key"]),
            source("openlibrary", "not_needed", &[]),
            source("igdb", "missing", &["client_id", "client_secret"]),
            source("tvdb", "db", &["api_key"]),
            source("podcastindex", "missing", &["api_key", "api_secret"]),
            source("bgg", "missing", &["api_key"]),
        ];
        assert_eq!(open_sources(&list), vec!["igdb", "podcastindex"]);
        assert!(!needs_key(&list[0]), "set by the environment");
        assert!(!needs_key(&list[1]), "free source");
        assert!(needs_key(&list[3]), "stored: may be replaced");
    }

    /// UX2-6: a key typed in the UI and rejected by the provider needs attention: its card
    /// starts open, ahead of the sources that have no key yet.
    #[test]
    fn a_rejected_key_starts_open() {
        let list = vec![
            source("igdb", "missing", &["client_id", "client_secret"]),
            source("tmdb", "invalid", &["api_key"]),
            source("podcastindex", "missing", &["api_key", "api_secret"]),
        ];
        assert!(needs_key(&list[1]));
        assert_eq!(open_sources(&list), vec!["tmdb", "igdb"]);
    }

    #[test]
    fn checklist_items_lead_to_their_page_and_hosted_lands_on_users() {
        assert_eq!(checklist_href("first_item"), "/library");
        assert_eq!(checklist_href("invite"), "/people");
        assert_eq!(checklist_href("media_server"), "/settings/integrations");
        assert_eq!(
            checklist_href("notification_defaults"),
            "/admin/notifications"
        );
        assert_eq!(checklist_href("backup_key"), "/admin/instance");
        assert_eq!(checklist_href("identity_provider"), "/admin/sso");
        assert_eq!(landing("home"), "/");
        assert_eq!(landing("hosted"), "/admin/users");
        assert_eq!(HOSTING[0], "home", "home is the default");
    }
}
