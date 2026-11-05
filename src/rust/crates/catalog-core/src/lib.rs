//! Catalog core: connector capability traits, registry, the governed HTTP client
//! (allowlist, rate limit, priority gate, circuit breaker, `Retry-After`), singleflight,
//! the content-addressed HTTP cache, merging with field provenance, and the background
//! refresh that implements stale-while-revalidate (D-5.2, D-6).
//!
//! Connectors never own an HTTP client: they receive the `HttpFetch` port in [`Ctx`], whose
//! production implementation is [`GovernedHttp`]. Every connector input and output is plain
//! serialisable data so the same contract can later be a WIT world for WASM connectors.
#![deny(unsafe_code)]

mod breaker;
pub mod cache;
mod connector;
mod gate;
mod http;
pub mod merge;
mod model;
pub mod refresh;
mod registry;
mod singleflight;

pub use breaker::Breaker;
pub use connector::{Connector, Ctx, Details, Images, Search};
pub use gate::Gate;
pub use http::{ConnectorPolicy, GovernedHttp};
pub use model::{
    Attribution, Auth, Capability, ConnectorError, ConnectorInfo, DetailsRequest, ImageKind,
    ImageRef, RateLimit, SearchRequest, Snapshot,
};
pub use registry::Registry;
pub use singleflight::SingleFlight;
