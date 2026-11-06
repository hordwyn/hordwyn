//! Search fan-out (D-6.6): query every connector serving the requested kinds in parallel and
//! report each connector's results as soon as they arrive. Results are stored as catalog
//! items (deduplicated by external id) so they have ids, show up in repeat searches and can
//! be added to a library right away. Callers run this in the background; nothing waits on it.

use std::future::Future;
use std::sync::Arc;

use app::ports::{HttpFetch, Lane};
use app::{App, NewCatalogItem};
use domain::catalog::CatalogItem;
use domain::kind::KindId;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::connector::Ctx;
use crate::model::{Capability, ImageKind, SearchRequest, Snapshot};
use crate::registry::Registry;

/// One connector's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct FanoutResult {
    pub connector: String,
    pub items: Vec<CatalogItem>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct SearchFanout {
    app: App,
    registry: Registry,
    http: Arc<dyn HttpFetch>,
}

/// Snapshot -> data for `App::upsert_catalog_item`.
#[must_use]
pub fn new_item(s: &Snapshot) -> Option<NewCatalogItem> {
    Some(NewCatalogItem {
        kind: KindId::new(s.kind.clone()).ok()?,
        title: s.title.clone(),
        original_title: s.original_title.clone(),
        year: s.year,
        description: s.description.clone(),
        external_ids: s.external_ids.clone(),
        attributes: s.attributes.clone(),
        cover_url: s
            .images
            .iter()
            .find(|i| i.kind == ImageKind::Cover)
            .map(|i| i.url.clone()),
    })
}

impl SearchFanout {
    #[must_use]
    pub fn new(app: App, registry: Registry, http: Arc<dyn HttpFetch>) -> Self {
        Self {
            app,
            registry,
            http,
        }
    }

    /// `(connector, kind)` pairs a search for `kinds` would query.
    #[must_use]
    pub fn targets(&self, kinds: &[String]) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for kind in kinds {
            for c in self.registry.for_kind(kind, Capability::Search) {
                out.push((c.info().id.clone(), kind.clone()));
            }
        }
        out
    }

    /// Run the fan-out, calling `on_result` once per connector as results arrive.
    pub async fn run<F, Fut>(&self, query: &str, kinds: &[String], limit: u32, on_result: F)
    where
        F: Fn(FanoutResult) -> Fut,
        Fut: Future<Output = ()>,
    {
        let ctx = Ctx {
            http: self.http.clone(),
            lane: Lane::Interactive,
        };
        let mut pending = FuturesUnordered::new();
        for (connector, kind) in self.targets(kinds) {
            let Some(c) = self.registry.get(&connector) else {
                continue;
            };
            let ctx = ctx.clone();
            let req = SearchRequest {
                kind,
                query: query.to_string(),
                language: None,
                limit,
            };
            pending.push(async move {
                let res = match c.as_search() {
                    Some(s) => s.search(&ctx, &req).await,
                    None => Ok(vec![]),
                };
                (connector, res)
            });
        }
        while let Some((connector, res)) = pending.next().await {
            let result = match res {
                Ok(snaps) => {
                    let mut items = Vec::new();
                    for s in snaps.iter().filter_map(new_item) {
                        match self.app.ensure_catalog_item(s).await {
                            Ok(item) => items.push(item),
                            Err(e) => {
                                tracing::warn!(connector = %connector, error = %e, "search result rejected");
                            }
                        }
                    }
                    FanoutResult {
                        connector,
                        items,
                        error: None,
                    }
                }
                Err(e) => FanoutResult {
                    connector,
                    items: vec![],
                    error: Some(e.to_string()),
                },
            };
            on_result(result).await;
        }
    }
}
