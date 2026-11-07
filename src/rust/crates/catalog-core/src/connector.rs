//! Capability traits. A connector implements only the capabilities it has and declares them
//! in [`ConnectorInfo::capabilities`].

use std::sync::Arc;

use app::ports::{HttpFetch, Lane};
use async_trait::async_trait;
use zeroize::Zeroizing;

use crate::model::{
    ConnectorError, ConnectorInfo, DetailsRequest, ImageRef, SearchRequest, Snapshot,
};

/// Everything a connector may use. HTTP goes through `http` only (governed in production).
#[derive(Clone)]
pub struct Ctx {
    pub http: Arc<dyn HttpFetch>,
    pub lane: Lane,
    /// The requesting member's own credential for this connector (its
    /// `CredentialSpec::user_fields` joined with `:`), for providers that identify each user
    /// (`TheTVDB` subscriber PIN, design.md 4.1). `None` for background work and for sources
    /// without per-user fields; the instance key applies then.
    pub user_key: Option<Zeroizing<String>>,
    /// The connector runs with an unsaved candidate key (Admin > Data sources > Test): a
    /// rejection must not mark the live key (UX2-S5).
    pub candidate_key: bool,
}

impl Ctx {
    #[must_use]
    pub fn new(http: Arc<dyn HttpFetch>, lane: Lane) -> Self {
        Self {
            http,
            lane,
            user_key: None,
            candidate_key: false,
        }
    }

    /// The same context for a member with their own credential (or none).
    #[must_use]
    pub fn with_user_key(mut self, key: Option<String>) -> Self {
        self.user_key = key.map(Zeroizing::new);
        self
    }

    /// The same context for testing an unsaved candidate key.
    #[must_use]
    pub fn for_candidate_key(mut self) -> Self {
        self.candidate_key = true;
        self
    }

    /// Does the request carry the member's own credential?
    #[must_use]
    pub fn uses_user_key(&self) -> bool {
        self.user_key.as_ref().is_some_and(|k| !k.trim().is_empty())
    }

    /// Says a 401/403 answer nothing about the instance credential (a member's own key or a
    /// candidate key)? Such answers stay out of the shared rejection state (UX2-S2, UX2-S5).
    #[must_use]
    pub fn skip_auth_record(&self) -> bool {
        self.candidate_key || self.uses_user_key()
    }
}

#[async_trait]
pub trait Search: Send + Sync {
    async fn search(&self, ctx: &Ctx, req: &SearchRequest)
    -> Result<Vec<Snapshot>, ConnectorError>;
}

#[async_trait]
pub trait Details: Send + Sync {
    async fn details(&self, ctx: &Ctx, req: &DetailsRequest) -> Result<Snapshot, ConnectorError>;
}

#[async_trait]
pub trait Images: Send + Sync {
    async fn images(
        &self,
        ctx: &Ctx,
        req: &DetailsRequest,
    ) -> Result<Vec<ImageRef>, ConnectorError>;
}

/// A registered connector. The `as_*` accessors expose the capabilities it implements.
pub trait Connector: Send + Sync {
    fn info(&self) -> &ConnectorInfo;

    /// The key the connector reads per request (`None`: it needs none).
    fn key_slot(&self) -> Option<crate::KeySlot> {
        None
    }

    /// The credential fields it asks for (design.md 4.2).
    fn credentials(&self) -> crate::CredentialSpec {
        crate::CredentialSpec::NONE
    }

    /// The same connector with another key, for testing a key before it is saved
    /// (`None`: only the stored key can be tested).
    fn probe(&self, key: Option<String>) -> Option<std::sync::Arc<dyn Connector>> {
        let _ = key;
        None
    }

    fn as_search(&self) -> Option<&dyn Search> {
        None
    }

    fn as_details(&self) -> Option<&dyn Details> {
        None
    }

    fn as_images(&self) -> Option<&dyn Images> {
        None
    }
}
