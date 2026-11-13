//! A tiny kind catalog for use-case tests. The real one lives in the `kinds` crate.

use app::ports::KindCatalog;
use domain::kind::{KindDescriptor, KindGroup, KindId, ProgressModel};
use serde_json::Value;

pub struct StaticKinds {
    kinds: Vec<KindDescriptor>,
}

fn descriptor(id: &str, model: ProgressModel, extra: &[&str]) -> KindDescriptor {
    let group = match id {
        "movie" | "show" => KindGroup::Watch,
        "book" => KindGroup::Read,
        "audiobook" => KindGroup::Listen,
        _ => KindGroup::Play,
    };
    let facets = if id == "movie" {
        vec!["genres".to_string()]
    } else {
        vec![]
    };
    KindDescriptor {
        id: KindId::new(id).expect("valid id"),
        group,
        progress_model: model,
        extra_statuses: extra.iter().map(ToString::to_string).collect(),
        attributes_schema: if id == "book" {
            serde_json::json!({"type": "object", "properties": {
                "series": {"type": "string"}, "series_position": {"type": "number"},
                "page_count": {"type": "integer"}}})
        } else {
            serde_json::json!({"type": "object"})
        },
        facets,
        relations: vec![],
        default_connectors: vec![],
    }
}

impl Default for StaticKinds {
    fn default() -> Self {
        Self {
            kinds: vec![
                descriptor("movie", ProgressModel::Single, &[]),
                descriptor("show", ProgressModel::Episodic, &[]),
                descriptor("book", ProgressModel::Paged, &[]),
                descriptor("audiobook", ProgressModel::Timed, &[]),
                descriptor("video_game", ProgressModel::Sessions, &["endless"]),
                descriptor("board_game", ProgressModel::Sessions, &[]),
            ],
        }
    }
}

impl StaticKinds {
    #[must_use]
    pub fn new(kinds: Vec<KindDescriptor>) -> Self {
        Self { kinds }
    }
}

impl KindCatalog for StaticKinds {
    fn get(&self, id: &KindId) -> Option<KindDescriptor> {
        self.kinds.iter().find(|k| &k.id == id).cloned()
    }

    fn all(&self) -> Vec<KindDescriptor> {
        self.kinds.clone()
    }

    fn validate_attributes(&self, kind: &KindId, attributes: &Value) -> Result<Value, String> {
        if self.get(kind).is_none() {
            return Err(format!("unknown kind `{kind}`"));
        }
        if attributes.is_object() {
            Ok(attributes.clone())
        } else {
            Err("attributes must be an object".into())
        }
    }
}
