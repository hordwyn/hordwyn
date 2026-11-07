//! Built-in media-kind modules: descriptors and typed attributes (D-5.1).
//!
//! A kind is a type implementing [`KindModule`]. Its typed `Attributes` struct gives Rust code
//! types, while storage and the UI stay generic: the struct's JSON Schema (generated with
//! schemars) becomes the descriptor's `attributes_schema`, and writes are validated by
//! deserialising into the struct. Adding a kind means adding one module here (or registering
//! one at runtime); nothing outside this crate changes.
#![deny(unsafe_code)]

pub mod builtin;
mod registry;

use domain::kind::{KindGroup, ProgressModel, RelationType};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

pub use registry::Registry;

/// A media kind with typed attributes.
pub trait KindModule: 'static {
    /// Kind-specific attributes. Unknown fields must be rejected
    /// (`#[serde(deny_unknown_fields)]`).
    type Attributes: Serialize + DeserializeOwned + JsonSchema;

    /// Stable kind id, e.g. `movie`.
    const ID: &'static str;
    const PROGRESS_MODEL: ProgressModel;

    /// Activity group (design.md 3.1). Built-in kinds take the group of their module
    /// (`screen` -> Watch, `read`, `listen`, `play`); a kind that does not say is in `Other`.
    #[must_use]
    fn group() -> KindGroup {
        KindGroup::Other
    }

    /// Statuses in addition to the five core ones.
    #[must_use]
    fn extra_statuses() -> Vec<String> {
        Vec::new()
    }

    /// Attribute names exposed as search facets. Must exist in `Attributes`.
    #[must_use]
    fn facets() -> Vec<String> {
        Vec::new()
    }

    #[must_use]
    fn relations() -> Vec<RelationType> {
        Vec::new()
    }

    #[must_use]
    fn default_connectors() -> Vec<String> {
        Vec::new()
    }
}

pub(crate) fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(ToString::to_string).collect()
}

pub(crate) fn relations(v: &[&str]) -> Vec<RelationType> {
    v.iter().map(|r| RelationType((*r).to_string())).collect()
}
