//! Catalog connectors, one module and cargo feature per provider (D-5.3).
//!
//! Connectors are plain data mappers: they build requests, send them through the
//! `HttpFetch` port they get in [`catalog_core::Ctx`] (governed in production) and map
//! responses to [`catalog_core::Snapshot`]s with typed `kinds` attributes. They never depend
//! on an HTTP client crate (enforced by the architecture test). Every connector's terms,
//! licence and attribution are documented in `docs/connectors/<id>.md`.
#![deny(unsafe_code)]

mod util;

#[cfg(feature = "tmdb")]
pub mod tmdb;

pub use util::get_json;
