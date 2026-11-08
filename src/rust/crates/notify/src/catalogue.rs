//! The event catalogue (D-17): every notification has a stable id, the template arguments it
//! carries, and an audience. Templates are Fluent messages `<id>-title` / `<id>-body` with
//! the dots of the id replaced by dashes (`social-link_request-title`).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventAudience {
    User,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EventDef {
    pub id: &'static str,
    pub audience: EventAudience,
    /// Template arguments (the payload schema).
    pub args: &'static [&'static str],
}

const fn user(id: &'static str, args: &'static [&'static str]) -> EventDef {
    EventDef {
        id,
        audience: EventAudience::User,
        args,
    }
}

const fn admin(id: &'static str, args: &'static [&'static str]) -> EventDef {
    EventDef {
        id,
        audience: EventAudience::Admin,
        args,
    }
}

pub const CATALOGUE: &[EventDef] = &[
    user("release.item_released", &["title"]),
    user("release.episode", &["title", "season", "episode"]),
    user("release.chapter", &["title", "number"]),
    user("release.date_changed", &["title", "date"]),
    user("library.status_changed", &["title", "status", "list"]),
    user("library.metadata_changed", &["title"]),
    user("social.link_request", &["from", "companion"]),
    user("social.link_accepted", &["user", "companion"]),
    user("social.session_shared", &["owner", "title"]),
    user("social.list_shared", &["owner", "list"]),
    user("boardgame.loan_due", &["title", "companion", "date"]),
    user("boardgame.loan_overdue", &["title", "companion", "date"]),
    user("boardgame.wishlist_owned", &["title", "user"]),
    user("integration.play_recorded", &["title", "source"]),
    user("integration.failing", &["name", "error"]),
    user("import.finished", &["imported", "errors"]),
    user("import.failed", &["error"]),
    admin("admin.storage_high", &["used", "cap"]),
    admin("admin.storage_full", &["used", "cap"]),
    admin("admin.connector_down", &["connector"]),
    admin("admin.signup", &["username", "name"]),
    admin("admin.job_backlog", &["lane", "count"]),
    // a member asked for a data source (design.md 4.4, "Ask admin")
    admin("admin.source_requested", &["source", "user"]),
    // the break-glass `/sign-in/local` was used (addendum-1 4.3, OD-17)
    admin("admin.local_sign_in", &["username", "address"]),
];

#[must_use]
pub fn event(id: &str) -> Option<&'static EventDef> {
    CATALOGUE.iter().find(|e| e.id == id)
}

/// Fluent message id prefix of an event.
#[must_use]
pub fn message_id(event: &str) -> String {
    event.replace('.', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_grouped() {
        let mut ids: Vec<_> = CATALOGUE.iter().map(|e| e.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATALOGUE.len());
        assert!(CATALOGUE.iter().all(|e| e.id.split_once('.').is_some()));
        assert!(
            CATALOGUE
                .iter()
                .all(|e| (e.audience == EventAudience::Admin) == e.id.starts_with("admin."))
        );
    }
}
