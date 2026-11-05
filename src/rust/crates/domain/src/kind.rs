//! Media kinds are described by data ([`KindDescriptor`]), not by a Rust enum (D-5.1).

use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Stable kind id, for example `movie` or `board_game`; plugins use `ext:<vendor>/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KindId(String);

impl KindId {
    /// Validate and wrap a kind id: `[a-z0-9_]+`, or `ext:` followed by `[a-z0-9_/-]+`.
    ///
    /// # Errors
    /// If the id is empty or contains other characters.
    pub fn new(id: impl Into<String>) -> Result<Self, DomainError> {
        let id = id.into();
        let (body, extra): (&str, &[char]) = match id.strip_prefix("ext:") {
            Some(rest) => (rest, &['/', '-']),
            None => (id.as_str(), &[]),
        };
        let ok = !body.is_empty()
            && body.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || extra.contains(&c)
            });
        if ok {
            Ok(Self(id))
        } else {
            Err(DomainError::invalid(
                "kind_id",
                format!("`{id}` is not a valid kind id"),
            ))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KindId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How progress is tracked for a kind. Closed set; adding one is a code change (D-5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgressModel {
    /// Done / not done, with rewatch count (movie).
    Single,
    /// Season / episode tree (show, anime, podcast).
    Episodic,
    /// Page / chapter / volume (book, manga, comic).
    Paged,
    /// Position in seconds (audiobook, music album).
    Timed,
    /// List of sessions with duration, participants, outcome (video game, board game).
    Sessions,
}

impl ProgressModel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Episodic => "episodic",
            Self::Paged => "paged",
            Self::Timed => "timed",
            Self::Sessions => "sessions",
        }
    }
}

/// Library status. The five core statuses exist for every kind; kinds may add more.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Status(String);

impl Status {
    pub const PLANNED: &'static str = "planned";
    pub const IN_PROGRESS: &'static str = "in_progress";
    pub const COMPLETED: &'static str = "completed";
    pub const PAUSED: &'static str = "paused";
    pub const DROPPED: &'static str = "dropped";
    pub const CORE: [&'static str; 5] = [
        Self::PLANNED,
        Self::IN_PROGRESS,
        Self::COMPLETED,
        Self::PAUSED,
        Self::DROPPED,
    ];

    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn planned() -> Self {
        Self::new(Self::PLANNED)
    }

    #[must_use]
    pub fn in_progress() -> Self {
        Self::new(Self::IN_PROGRESS)
    }

    #[must_use]
    pub fn completed() -> Self {
        Self::new(Self::COMPLETED)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Allowed relation between catalog items, e.g. `sequel`, `expansion_of`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelationType(pub String);

/// Data description of a media kind (D-5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KindDescriptor {
    pub id: KindId,
    pub progress_model: ProgressModel,
    /// Kind-specific statuses in addition to [`Status::CORE`].
    pub extra_statuses: Vec<String>,
    /// JSON Schema of the `attributes` object.
    pub attributes_schema: serde_json::Value,
    /// Attribute paths exposed as search facets.
    pub facets: Vec<String>,
    pub relations: Vec<RelationType>,
    /// Connector ids, for example `tmdb`.
    pub default_connectors: Vec<String>,
}

impl KindDescriptor {
    /// All statuses allowed for this kind, core first.
    pub fn statuses(&self) -> impl Iterator<Item = &str> {
        Status::CORE
            .iter()
            .copied()
            .chain(self.extra_statuses.iter().map(String::as_str))
    }

    /// # Errors
    /// If `status` is neither a core status nor one of the kind's extra statuses.
    pub fn check_status(&self, status: &Status) -> Result<(), DomainError> {
        if self.statuses().any(|s| s == status.as_str()) {
            Ok(())
        } else {
            Err(DomainError::StatusNotAllowed {
                kind: self.id.to_string(),
                status: status.as_str().to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_id_validation() {
        assert!(KindId::new("board_game").is_ok());
        assert!(KindId::new("ext:acme/wine-tasting").is_ok());
        assert!(KindId::new("").is_err());
        assert!(KindId::new("Movie").is_err());
        assert!(KindId::new("a/b").is_err());
        assert!(KindId::new("ext:").is_err());
    }

    #[test]
    fn statuses_include_core_and_extra() {
        let d = KindDescriptor {
            id: KindId::new("video_game").unwrap(),
            progress_model: ProgressModel::Sessions,
            extra_statuses: vec!["endless".into()],
            attributes_schema: serde_json::json!({}),
            facets: vec![],
            relations: vec![],
            default_connectors: vec![],
        };
        assert!(d.check_status(&Status::planned()).is_ok());
        assert!(d.check_status(&Status::new("endless")).is_ok());
        assert!(d.check_status(&Status::new("owned")).is_err());
    }
}
