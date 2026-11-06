use std::collections::BTreeMap;

use app::ports::KindCatalog;
use domain::kind::{KindDescriptor, KindId};
use serde_json::Value;

use crate::KindModule;

type Validator = fn(&Value) -> Result<Value, String>;

struct Entry {
    descriptor: KindDescriptor,
    validate: Validator,
}

/// Registered kinds. Implements the `app` port [`KindCatalog`].
#[derive(Default)]
pub struct Registry {
    kinds: BTreeMap<KindId, Entry>,
}

fn validate<K: KindModule>(v: &Value) -> Result<Value, String> {
    let typed: K::Attributes = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(typed).map_err(|e| e.to_string())
}

impl Registry {
    /// Empty registry.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Keep only the listed kinds (per-deployment feature toggle, D-10). Unknown ids are ignored.
    pub fn retain(&mut self, ids: &[String]) {
        self.kinds
            .retain(|k, _| ids.iter().any(|i| i == k.as_str()));
    }

    /// Registry with every built-in kind.
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut r = Self::empty();
        crate::builtin::register_all(&mut r);
        r
    }

    /// Register a kind module.
    ///
    /// # Panics
    /// If the id is invalid, already registered, or a facet does not name an attribute.
    pub fn register<K: KindModule>(&mut self) -> &mut Self {
        let id = KindId::new(K::ID).expect("kind id must be valid");
        assert!(
            !self.kinds.contains_key(&id),
            "kind `{id}` registered twice"
        );
        let schema =
            serde_json::to_value(schemars::schema_for!(K::Attributes)).expect("schema serialises");
        let props = schema.get("properties").and_then(Value::as_object);
        for f in K::facets() {
            assert!(
                props.is_some_and(|p| p.contains_key(&f)),
                "facet `{f}` of kind `{id}` is not an attribute"
            );
        }
        let descriptor = KindDescriptor {
            id: id.clone(),
            progress_model: K::PROGRESS_MODEL,
            extra_statuses: K::extra_statuses(),
            attributes_schema: schema,
            facets: K::facets(),
            relations: K::relations(),
            default_connectors: K::default_connectors(),
        };
        self.kinds.insert(
            id,
            Entry {
                descriptor,
                validate: validate::<K>,
            },
        );
        self
    }

    pub fn ids(&self) -> impl Iterator<Item = &KindId> {
        self.kinds.keys()
    }
}

impl KindCatalog for Registry {
    fn get(&self, id: &KindId) -> Option<KindDescriptor> {
        self.kinds.get(id).map(|e| e.descriptor.clone())
    }

    fn all(&self) -> Vec<KindDescriptor> {
        self.kinds.values().map(|e| e.descriptor.clone()).collect()
    }

    fn validate_attributes(&self, kind: &KindId, attributes: &Value) -> Result<Value, String> {
        let entry = self
            .kinds
            .get(kind)
            .ok_or_else(|| format!("unknown kind `{kind}`"))?;
        (entry.validate)(attributes)
    }
}
