use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use crate::connector::Connector;
use crate::contact::{ContactUse, OperatorContact};
use crate::credentials::CredentialState;
use crate::model::{Attribution, Capability, ConnectorInfo};
use crate::stats::SourceStats;

/// Compiled-in connectors, in registration order (which is also the default merge priority).
/// Every connector is registered; its [`CredentialState`] decides whether search and refresh
/// use it (design.md 4.2). Clones share the states.
#[derive(Default, Clone)]
pub struct Registry {
    order: Vec<String>,
    connectors: BTreeMap<String, Arc<dyn Connector>>,
    states: Arc<RwLock<BTreeMap<String, CredentialState>>>,
    /// Sources an admin switched off (DEC-289); shared by clones like the states.
    disabled: Arc<RwLock<BTreeSet<String>>>,
    stats: Option<Arc<SourceStats>>,
    /// The operator contact (shared by clones): a connector that requires it is not usable
    /// without one.
    contact: Option<OperatorContact>,
    /// The instance language (the brand's `default_locale`): what a refresh asks the sources
    /// in, and a search of a member who left their language automatic (WD-2).
    language: Option<String>,
}

impl Registry {
    /// The instance language the sources are asked in when no member's language applies.
    #[must_use]
    pub fn with_language(mut self, language: &str) -> Self {
        self.language = Some(language.to_string());
        self
    }

    /// The instance language, if set.
    #[must_use]
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// Read rejections (401/403) from the governed client's counters.
    #[must_use]
    pub fn with_stats(mut self, stats: Arc<SourceStats>) -> Self {
        self.stats = Some(stats);
        self
    }

    /// Read the operator contact: a connector whose [`ConnectorInfo::contact`] is
    /// `Required` is in state [`CredentialState::NeedsContact`] while none is set.
    #[must_use]
    pub fn with_contact(mut self, contact: OperatorContact) -> Self {
        self.contact = Some(contact);
        self
    }

    /// The counters, if wired.
    #[must_use]
    pub fn stats(&self) -> Option<&Arc<SourceStats>> {
        self.stats.as_ref()
    }

    /// # Panics
    /// If a connector with the same id is already registered.
    pub fn register(&mut self, c: Arc<dyn Connector>) -> &mut Self {
        let id = c.info().id.clone();
        assert!(
            !self.connectors.contains_key(&id),
            "connector `{id}` registered twice"
        );
        let state = if c.credentials().instance_fields.is_empty() {
            CredentialState::NotNeeded
        } else if c.key_slot().is_some_and(|k| k.get().is_some()) {
            CredentialState::Env
        } else {
            CredentialState::Missing
        };
        self.states
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id.clone(), state);
        self.order.push(id.clone());
        self.connectors.insert(id, c);
        self
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<Arc<dyn Connector>> {
        self.connectors.get(id).cloned()
    }

    /// The stored state, or `Invalid` when the provider rejected the current key.
    #[must_use]
    pub fn credential(&self, id: &str) -> CredentialState {
        let state = self
            .states
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .copied()
            .unwrap_or(CredentialState::Missing);
        let rejected = self.stats.as_ref().is_some_and(|s| s.rejected(id));
        if rejected && matches!(state, CredentialState::Env | CredentialState::Db) {
            CredentialState::Invalid
        } else if state.usable() && self.needs_contact(id) {
            CredentialState::NeedsContact
        } else {
            state
        }
    }

    /// Does `id` require the operator contact while none is set?
    fn needs_contact(&self, id: &str) -> bool {
        self.connectors
            .get(id)
            .is_some_and(|c| c.info().contact == ContactUse::Required)
            && self.contact.as_ref().is_none_or(|c| c.get().is_none())
    }

    /// Set the credential state (and the key, for `Env`/`Db`); a new key clears an earlier
    /// rejection.
    pub fn set_key(&self, id: &str, key: Option<String>, state: CredentialState) {
        if let Some(slot) = self.connectors.get(id).and_then(|c| c.key_slot()) {
            slot.set(key);
        }
        if let Some(s) = &self.stats {
            s.key_changed(id);
        }
        self.states
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id.to_string(), state);
    }

    /// Switch a source off or on (the admin's Disabled switch, DEC-289): a disabled source
    /// serves no search and no lookup, and is not reported as skipped.
    pub fn set_enabled(&self, id: &str, enabled: bool) {
        let mut d = self
            .disabled
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if enabled {
            d.remove(id);
        } else {
            d.insert(id.to_string());
        }
    }

    #[must_use]
    pub fn is_enabled(&self, id: &str) -> bool {
        !self
            .disabled
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(id)
    }

    #[must_use]
    pub fn infos(&self) -> Vec<ConnectorInfo> {
        self.order
            .iter()
            .map(|id| self.connectors[id].info().clone())
            .collect()
    }

    fn serving(&self, kind: &str, capability: Capability) -> Vec<&Arc<dyn Connector>> {
        self.order
            .iter()
            .map(|id| &self.connectors[id])
            .filter(|c| {
                let i = c.info();
                i.kinds.iter().any(|k| k == kind)
                    && i.capabilities.contains(&capability)
                    && self.is_enabled(&i.id)
            })
            .collect()
    }

    /// Usable connectors serving `kind` with `capability`, in priority order.
    #[must_use]
    pub fn for_kind(&self, kind: &str, capability: Capability) -> Vec<Arc<dyn Connector>> {
        self.serving(kind, capability)
            .into_iter()
            .filter(|c| self.credential(&c.info().id).usable())
            .cloned()
            .collect()
    }

    /// Connectors that would serve `kinds` but are skipped, with the reason (the palette
    /// footer and the no-results card, design.md 4.4).
    #[must_use]
    pub fn skipped(
        &self,
        kinds: &[String],
        capability: Capability,
    ) -> Vec<(String, CredentialState)> {
        let mut out: Vec<(String, CredentialState)> = Vec::new();
        for kind in kinds {
            for c in self.serving(kind, capability) {
                let id = &c.info().id;
                let state = self.credential(id);
                if !state.usable() && !out.iter().any(|(i, _)| i == id) {
                    out.push((id.clone(), state));
                }
            }
        }
        out
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
