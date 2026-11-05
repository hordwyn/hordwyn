//! Capability traits. A connector implements only the capabilities it has and declares them
//! in [`ConnectorInfo::capabilities`].

use std::sync::Arc;

use app::ports::{HttpFetch, Lane};
use async_trait::async_trait;

use crate::model::{
    ConnectorError, ConnectorInfo, DetailsRequest, ImageRef, SearchRequest, Snapshot,
};

/// Everything a connector may use. HTTP goes through `http` only (governed in production).
#[derive(Clone)]
pub struct Ctx {
    pub http: Arc<dyn HttpFetch>,
    pub lane: Lane,
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
