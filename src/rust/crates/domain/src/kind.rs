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

/// Activity group of a kind (design.md 3.1): Watch, Read, Listen, Play. A kind that names
/// no known group (a runtime kind, later) is in `Other`, which the UI shows only when it is
/// not empty. Serialised as the lower-case id; unknown ids read as `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum KindGroup {
    Watch,
    Read,
    Listen,
    Play,
    #[default]
    Other,
}

impl KindGroup {
    /// Every group in display order.
    pub const ALL: [Self; 5] = [
        Self::Watch,
        Self::Read,
        Self::Listen,
        Self::Play,
        Self::Other,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::Read => "read",
            Self::Listen => "listen",
            Self::Play => "play",
            Self::Other => "other",
        }
    }

    /// The group with this id; unknown ids are `Other`.
    #[must_use]
    pub fn parse(s: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|g| g.as_str() == s)
            .unwrap_or(Self::Other)
    }
}

impl Serialize for KindGroup {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for KindGroup {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(Self::parse(&s))
    }
}

/// Data description of a media kind (D-5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KindDescriptor {
    pub id: KindId,
    /// Activity group; descriptors without one are in [`KindGroup::Other`].
    #[serde(default)]
    pub group: KindGroup,
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

    /// Do the kind's attributes have a field `name` (a property of the schema)?
    #[must_use]
    pub fn has_attribute(&self, name: &str) -> bool {
        self.attributes_schema
            .get("properties")
            .is_some_and(|p| p.get(name).is_some())
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
    fn groups_have_stable_ids_and_unknown_ones_fall_into_other() {
        assert_eq!(
            KindGroup::ALL.map(KindGroup::as_str),
            ["watch", "read", "listen", "play", "other"]
        );
        assert_eq!(KindGroup::parse("listen"), KindGroup::Listen);
        assert_eq!(KindGroup::parse("cooking"), KindGroup::Other);
        let g: KindGroup = serde_json::from_str("\"wine\"").unwrap();
        assert_eq!(g, KindGroup::Other);
        assert_eq!(serde_json::to_string(&KindGroup::Play).unwrap(), "\"play\"");
        // a descriptor without a group (older runtime kinds) is in "other"
        let d: KindDescriptor = serde_json::from_value(serde_json::json!({
            "id": "ext:acme/wine", "progress_model": "single", "extra_statuses": [],
            "attributes_schema": {}, "facets": [], "relations": [], "default_connectors": []
        }))
        .unwrap();
        assert_eq!(d.group, KindGroup::Other);
    }

    #[test]
    fn statuses_include_core_and_extra() {
        let d = KindDescriptor {
            id: KindId::new("video_game").unwrap(),
            group: KindGroup::Play,
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
