//! Merge snapshots from several connectors by per-field priority; every field keeps its
//! provenance (D-6.7).

use std::collections::{BTreeMap, HashMap};

use domain::catalog::ExternalId;
use serde_json::{Map, Value};

use crate::model::{ImageKind, Snapshot};

/// Field priority: per field, connector ids in order; fields not listed use `default`.
#[derive(Debug, Clone, Default)]
pub struct FieldPriority {
    pub default: Vec<String>,
    pub fields: HashMap<String, Vec<String>>,
}

impl FieldPriority {
    fn rank(&self, field: &str, connector: &str) -> usize {
        let order = self.fields.get(field).unwrap_or(&self.default);
        order
            .iter()
            .position(|c| c == connector)
            .unwrap_or(usize::MAX)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub attributes: Value,
    pub external_ids: Vec<ExternalId>,
    pub cover_url: Option<String>,
    /// `field -> connector`, attribute fields as `attributes.<name>`.
    pub provenance: BTreeMap<String, String>,
}

fn is_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// Merge `snapshots` (any order). Returns `None` if there are none.
#[must_use]
pub fn merge(snapshots: &[Snapshot], priority: &FieldPriority) -> Option<Merged> {
    if snapshots.is_empty() {
        return None;
    }
    let mut provenance = BTreeMap::new();
    let mut pick = |field: &str, get: &dyn Fn(&Snapshot) -> Value| -> Option<Value> {
        let best = snapshots
            .iter()
            .filter(|s| !is_empty(&get(s)))
            .min_by_key(|s| priority.rank(field, &s.connector))?;
        provenance.insert(field.to_string(), best.connector.clone());
        Some(get(best))
    };
    let title = pick("title", &|s| Value::String(s.title.clone()))
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default();
    let original_title = pick("original_title", &|s| {
        s.original_title.clone().map_or(Value::Null, Value::String)
    })
    .and_then(|v| v.as_str().map(String::from));
    let year = pick("year", &|s| s.year.map_or(Value::Null, Value::from))
        .and_then(|v| v.as_i64())
        .and_then(|y| i32::try_from(y).ok());
    let description = pick("description", &|s| {
        s.description.clone().map_or(Value::Null, Value::String)
    })
    .and_then(|v| v.as_str().map(String::from));
    let cover_url = pick("cover_url", &|s| {
        s.images
            .iter()
            .find(|i| i.kind == ImageKind::Cover)
            .map_or(Value::Null, |i| Value::String(i.url.clone()))
    })
    .and_then(|v| v.as_str().map(String::from));

    let mut keys: Vec<String> = snapshots
        .iter()
        .filter_map(|s| s.attributes.as_object())
        .flat_map(|o| o.keys().cloned())
        .collect();
    keys.sort();
    keys.dedup();
    let mut attributes = Map::new();
    for k in keys {
        let field = format!("attributes.{k}");
        if let Some(v) = pick(&field, &|s| {
            s.attributes.get(&k).cloned().unwrap_or(Value::Null)
        }) {
            attributes.insert(k, v);
        }
    }

    let mut external_ids: Vec<ExternalId> = snapshots
        .iter()
        .flat_map(|s| s.external_ids.clone())
        .collect();
    external_ids.sort();
    external_ids.dedup();

    Some(Merged {
        title,
        original_title,
        year,
        description,
        attributes: Value::Object(attributes),
        external_ids,
        cover_url,
        provenance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snap(connector: &str, title: &str, attrs: Value, year: Option<i32>) -> Snapshot {
        Snapshot {
            connector: connector.into(),
            kind: "show".into(),
            external_ids: vec![ExternalId::new(connector, "1")],
            title: title.into(),
            original_title: None,
            year,
            description: None,
            attributes: attrs,
            images: vec![],
            complete: true,
        }
    }

    #[test]
    fn per_field_priority_and_provenance() {
        let a = snap(
            "tmdb",
            "Dark",
            json!({"genres": ["Drama"], "network": null}),
            None,
        );
        let b = snap(
            "tvdb",
            "DARK",
            json!({"genres": ["Mystery"], "network": "Netflix"}),
            Some(2017),
        );
        let mut prio = FieldPriority {
            default: vec!["tmdb".into(), "tvdb".into()],
            ..FieldPriority::default()
        };
        prio.fields.insert(
            "attributes.genres".into(),
            vec!["tvdb".into(), "tmdb".into()],
        );
        let m = merge(&[b.clone(), a.clone()], &prio).unwrap();
        assert_eq!(m.title, "Dark");
        assert_eq!(m.year, Some(2017), "falls back to the source that has it");
        assert_eq!(
            m.attributes,
            json!({"genres": ["Mystery"], "network": "Netflix"})
        );
        assert_eq!(m.provenance["title"], "tmdb");
        assert_eq!(m.provenance["attributes.genres"], "tvdb");
        assert_eq!(m.provenance["attributes.network"], "tvdb");
        assert_eq!(m.external_ids.len(), 2);
        assert!(merge(&[], &prio).is_none());
    }
}
