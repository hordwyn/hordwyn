//! Append-only progress events (D-12). They never conflict: sync only appends.

use serde::{Deserialize, Serialize};

use crate::DomainError;
use crate::Timestamp;
use crate::ids::{EventId, ItemId, SessionId, UserId};
use crate::kind::ProgressModel;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Progress {
    /// `Single`: watched / finished once more.
    Finished,
    /// `Episodic`: an episode was watched / heard.
    Episode { season: u32, episode: u32 },
    /// `Paged`: position reached (at least one field set).
    Paged {
        page: Option<u32>,
        chapter: Option<u32>,
        volume: Option<u32>,
    },
    /// `Timed`: position in seconds.
    Position { seconds: u64 },
    /// `Sessions`: a session was logged.
    Session { session_id: SessionId },
}

impl Progress {
    #[must_use]
    pub const fn model(&self) -> ProgressModel {
        match self {
            Self::Finished => ProgressModel::Single,
            Self::Episode { .. } => ProgressModel::Episodic,
            Self::Paged { .. } => ProgressModel::Paged,
            Self::Position { .. } => ProgressModel::Timed,
            Self::Session { .. } => ProgressModel::Sessions,
        }
    }

    /// Check that this progress fits `model` and is well-formed.
    ///
    /// # Errors
    /// On a model mismatch or an empty `Paged` position.
    pub fn validate_for(&self, model: ProgressModel) -> Result<(), DomainError> {
        if self.model() != model {
            return Err(DomainError::ProgressModelMismatch {
                model: model.as_str().into(),
                got: self.model().as_str().into(),
            });
        }
        if let Self::Paged {
            page: None,
            chapter: None,
            volume: None,
        } = self
        {
            return Err(DomainError::invalid(
                "progress",
                "paged progress needs a page, chapter or volume",
            ));
        }
        Ok(())
    }
}

/// One immutable progress event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub id: EventId,
    pub user_id: UserId,
    pub item_id: ItemId,
    /// When it happened (user supplied, may be in the past).
    pub occurred_at: Timestamp,
    pub progress: Progress,
    /// Idempotency key from the client, if any (D-12).
    pub client_op_id: Option<uuid::Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_must_match_model() {
        assert!(
            Progress::Finished
                .validate_for(ProgressModel::Single)
                .is_ok()
        );
        assert!(
            Progress::Episode {
                season: 1,
                episode: 2
            }
            .validate_for(ProgressModel::Single)
            .is_err()
        );
        assert!(
            Progress::Position { seconds: 5 }
                .validate_for(ProgressModel::Timed)
                .is_ok()
        );
    }

    #[test]
    fn empty_paged_progress_is_rejected() {
        let p = Progress::Paged {
            page: None,
            chapter: None,
            volume: None,
        };
        assert!(p.validate_for(ProgressModel::Paged).is_err());
    }
}
