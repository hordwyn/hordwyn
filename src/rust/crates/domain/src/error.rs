//! Domain rule violations.

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("invalid value for {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("status `{status}` is not allowed for kind `{kind}`")]
    StatusNotAllowed { kind: String, status: String },
    #[error("progress of type {got} does not fit progress model {model}")]
    ProgressModelMismatch { model: String, got: String },
    #[error("operation not allowed: {0}")]
    NotAllowed(String),
}

impl DomainError {
    pub fn invalid(field: &'static str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            field,
            reason: reason.into(),
        }
    }
}
