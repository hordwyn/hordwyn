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
mod credentials;
mod gate;
mod http;
pub mod merge;
mod model;
pub mod pg;
mod plain;
pub mod refresh;
mod registry;
mod resolve;
pub mod retention;
pub mod search;
mod singleflight;
mod stats;

pub use breaker::Breaker;
pub use connector::{Connector, Ctx, Details, Episodes, Images, Search};
pub use credentials::{CredentialSpec, CredentialState, KeySlot};
pub use gate::Gate;
pub use http::{ConnectorPolicy, GovernedHttp, HostList, PrivateRule};
pub use model::{
    Attribution, Auth, Capability, ConnectorError, ConnectorInfo, DetailsRequest, EpisodesRequest,
    ImageKind, ImageRef, RateLimit, SearchRequest, Snapshot,
};
pub use plain::PlainHttp;
pub use registry::Registry;
pub use singleflight::SingleFlight;
pub use stats::{ConnectorStats, SourceCounters, SourceStats, StatsMarks, StatsStore};
