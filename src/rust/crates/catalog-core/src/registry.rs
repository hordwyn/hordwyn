use std::collections::BTreeMap;
use std::sync::Arc;

use crate::connector::Connector;
use crate::model::{Attribution, Capability, ConnectorInfo};

/// Enabled connectors, in registration order (which is also the default merge priority).
#[derive(Default, Clone)]
pub struct Registry {
    order: Vec<String>,
    connectors: BTreeMap<String, Arc<dyn Connector>>,
}

impl Registry {
    /// # Panics
    /// If a connector with the same id is already registered.
    pub fn register(&mut self, c: Arc<dyn Connector>) -> &mut Self {
        let id = c.info().id.clone();
        assert!(
            !self.connectors.contains_key(&id),
            "connector `{id}` registered twice"
        );
        self.order.push(id.clone());
        self.connectors.insert(id, c);
        self
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<dyn Connector>> {
        self.connectors.get(id).cloned()
    }

    #[must_use]
    pub fn infos(&self) -> Vec<ConnectorInfo> {
        self.order
            .iter()
            .map(|id| self.connectors[id].info().clone())
            .collect()
    }

    /// Connectors serving `kind` with `capability`, in priority order.
    #[must_use]
    pub fn for_kind(&self, kind: &str, capability: Capability) -> Vec<Arc<dyn Connector>> {
        self.order
            .iter()
            .map(|id| &self.connectors[id])
            .filter(|c| {
                let i = c.info();
                i.kinds.iter().any(|k| k == kind) && i.capabilities.contains(&capability)
            })
            .cloned()
            .collect()
    }

    /// Attribution of every enabled connector (the "Data sources" page).
    #[must_use]
    pub fn attributions(&self) -> Vec<(String, Attribution)> {
        self.infos()
            .into_iter()
            .map(|i| (i.name, i.attribution))
            .collect()
    }
}
