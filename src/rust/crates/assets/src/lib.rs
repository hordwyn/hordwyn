//! Blob store, image pipeline, storage budget and eviction (D-7, D-8).
//!
//! Every byte the server stores goes through [`StorageBudget`]: it is also the `BlobStore`
//! handed to the HTTP cache and uploads, and the raw opendal backend is private to this crate.
#![deny(unsafe_code)]

mod backend;
pub mod budget;
pub mod config;
pub mod image;
pub mod index;
pub mod pg;
pub mod pool;
pub mod service;

pub use backend::BackendError;
pub use budget::{
    DiskSpace, EVICT_JOB, EvictionReport, NewObject, NoDisk, ReadOutcome, Refusal, StorageBudget,
    StorageReport, VolumeSpace, WriteOutcome,
};
pub use config::{
    Backend, ByteSize, FetchPolicy, FetchPriority, Format, MinFree, StorageConfig, Variant,
};
pub use index::{MemObjectIndex, ObjectClass, ObjectIndex, StoredObject, Tier, Usage};
pub use pool::CpuPool;
pub use service::{
    AssetError, AssetKind, AssetRecord, AssetRepo, AssetService, FETCH_JOB, MemAssetRepo, Served,
};
