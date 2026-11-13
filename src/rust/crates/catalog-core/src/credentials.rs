//! The credential model of design.md 4.2: every compiled-in connector is registered; a
//! [`CredentialState`] says whether it can be used, and a [`KeySlot`] holds the key it reads
//! per request, so a key changed in the admin UI takes effect without a restart.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use zeroize::Zeroizing;

/// Where a connector's credential comes from, or why it cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CredentialState {
    /// The source needs no key.
    NotNeeded,
    /// No key configured: the source is skipped.
    Missing,
    /// Set by the environment or the config file (read-only in the UI).
    Env,
    /// Stored encrypted in the database (admin UI or wizard).
    Db,
    /// The provider rejected the key (last answer 401 or 403).
    Invalid,
    /// A stored key exists but cannot be decrypted (addendum-1 2.2).
    Locked,
    /// The source asks for the operator contact and none is set: it is not asked.
    NeedsContact,
}

impl CredentialState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotNeeded => "not_needed",
            Self::Missing => "missing",
            Self::Env => "env",
            Self::Db => "db",
            Self::Invalid => "invalid",
            Self::Locked => "locked",
            Self::NeedsContact => "needs_contact",
        }
    }

    /// May search and refresh use the connector?
    #[must_use]
    pub fn usable(self) -> bool {
        matches!(self, Self::NotNeeded | Self::Env | Self::Db)
    }
}

/// The credential fields a connector asks for (names are stable keys of the stored JSON
/// and of the UI labels). Values are joined with `:` into the connector's key string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CredentialSpec {
    /// Fields of the instance credential (empty: no key needed).
    pub instance_fields: &'static [&'static str],
    /// Fields each member adds for themselves (`TheTVDB` subscriber PIN).
    pub user_fields: &'static [&'static str],
}

impl CredentialSpec {
    pub const NONE: Self = Self {
        instance_fields: &[],
        user_fields: &[],
    };
}

static VERSIONS: AtomicU64 = AtomicU64::new(1);

/// A connector's key, shared between the connector and the server; readers always see the
/// latest value. `Debug` never prints the key.
#[derive(Clone, Default)]
pub struct KeySlot(Arc<RwLock<(Option<Zeroizing<String>>, u64)>>);

impl std::fmt::Debug for KeySlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeySlot")
            .field("set", &self.get().is_some())
            .field("version", &self.version())
            .finish()
    }
}

impl KeySlot {
    #[must_use]
    pub fn new(key: Option<String>) -> Self {
        let s = Self::default();
        s.set(key);
        s
    }

    /// The current key.
    ///
    /// # Panics
    /// Never in practice (a poisoned lock is recovered).
    #[must_use]
    pub fn get(&self) -> Option<Zeroizing<String>> {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
            .clone()
    }

    /// Replace the key; the version changes so cached tokens derived from it are dropped.
    pub fn set(&self, key: Option<String>) {
        let mut g = self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *g = (
            key.filter(|k| !k.is_empty()).map(Zeroizing::new),
            VERSIONS.fetch_add(1, Ordering::Relaxed),
        );
    }

    /// Changes with every [`KeySlot::set`].
    #[must_use]
    pub fn version(&self) -> u64 {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1
    }
}
