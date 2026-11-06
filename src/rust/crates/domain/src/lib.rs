//! Pure domain types and rules. No IO, no async, no framework dependencies.
#![deny(unsafe_code)]

pub mod catalog;
pub mod companion;
pub mod error;
pub mod hlc;
pub mod ids;
pub mod kind;
pub mod library;
pub mod list;
pub mod net;
pub mod progress;
pub mod review;
pub mod session;
pub mod user;

pub use error::DomainError;

/// Timestamp type used across the domain.
pub type Timestamp = chrono::DateTime<chrono::Utc>;
