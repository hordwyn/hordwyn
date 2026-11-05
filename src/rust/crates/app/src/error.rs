use domain::DomainError;
use thiserror::Error;

use crate::ports::RepoError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("forbidden")]
    Forbidden,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("storage error: {0}")]
    Repo(#[from] RepoError),
}

pub type AppResult<T> = Result<T, AppError>;

pub(crate) trait OptionExt<T> {
    fn found(self, what: &'static str) -> AppResult<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn found(self, what: &'static str) -> AppResult<T> {
        self.ok_or(AppError::NotFound(what))
    }
}
