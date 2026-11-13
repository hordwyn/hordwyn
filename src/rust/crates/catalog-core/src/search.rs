//! Search fan-out (D-6.6): query every connector serving the requested kinds in parallel and
//! report each connector's results as soon as they arrive. Results are stored as catalog
//! items (deduplicated by external id) so they have ids, show up in repeat searches and can
//! be added to a library right away. Callers run this in the background; nothing waits on it.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;

use app::ports::{HttpFetch, Lane};
use app::{App, NewCatalogItem};
use domain::catalog::CatalogItem;
use domain::kind::KindId;
use futures::stream::{FuturesUnordered, StreamExt};
use zeroize::Zeroizing;

use crate::connector::Ctx;
use crate::model::{Capability, ImageKind, SearchRequest, Snapshot};
use crate::registry::Registry;

/// The requesting member's own credential per connector id (`CredentialSpec::user_fields`
/// joined with `:`), handed to that connector only ([`Ctx::user_key`]). `Debug` prints the
/// connector ids only (UX2-S9). It also carries who searches: the member (fair share) and
/// the language they chose (WD-2).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct UserKeys(
    BTreeMap<String, Zeroizing<String>>,
    Option<domain::ids::UserId>,
    Option<String>,
);

impl UserKeys {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// No credentials yet, for searches of `user` (the fair share of the interactive lane in
    /// hosted mode, design.md 4.1).
    #[must_use]
    pub fn for_user(user: domain::ids::UserId) -> Self {
        Self(BTreeMap::new(), Some(user), None)
    }

    /// The language the member chose (`None`: automatic, the instance language applies).
    #[must_use]
    pub fn with_language(mut self, language: Option<String>) -> Self {
        self.2 = language;
        self
    }

    /// The member's chosen language, if any.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.2.as_deref()
    }

    /// The member these searches are made for.
    #[must_use]
    pub fn user(&self) -> Option<domain::ids::UserId> {
        self.1
    }

    pub fn insert(&mut self, connector: String, key: Zeroizing<String>) {
        self.0.insert(connector, key);
    }

    #[must_use]
    pub fn get(&self, connector: &str) -> Option<&Zeroizing<String>> {
        self.0.get(connector)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for UserKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.0.keys()).finish()
    }
}

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
        popularity: s.popularity,
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

    /// Run the fan-out without member credentials (instance keys only).
    pub async fn run<F, Fut>(&self, query: &str, kinds: &[String], limit: u32, on_result: F)
    where
        F: Fn(FanoutResult) -> Fut,
        Fut: Future<Output = ()>,
    {
        self.run_as(&UserKeys::new(), query, kinds, limit, on_result)
            .await;
    }

    /// Run the fan-out for a member, calling `on_result` once per connector as results
    /// arrive. Each connector gets the member's own credential for it from `user_keys`.
    pub async fn run_as<F, Fut>(
        &self,
        user_keys: &UserKeys,
        query: &str,
        kinds: &[String],
        limit: u32,
        on_result: F,
    ) where
        F: Fn(FanoutResult) -> Fut,
        Fut: Future<Output = ()>,
    {
        let base = Ctx::new(self.http.clone(), Lane::Interactive);
        let mut pending = FuturesUnordered::new();
        for (connector, kind) in self.targets(kinds) {
            let Some(c) = self.registry.get(&connector) else {
                continue;
            };
            let mut ctx = base.clone();
            ctx.user_key = user_keys.get(&connector).cloned();
            ctx.user = user_keys.user();
            let req = SearchRequest {
                kind,
                query: query.to_string(),
                // the member's chosen language, else the instance language (WD-2, LB-35)
                language: user_keys
                    .language()
                    .or(self.registry.language())
                    .map(String::from),
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
