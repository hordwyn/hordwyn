//! Adding a kind needs no change outside this crate: this test defines a brand-new kind
//! inline, registers it, and drives the unchanged `app` use-cases with it.

use std::sync::Arc;

use app::NewCatalogItem;
use domain::catalog::ExternalId;
use domain::kind::{KindId, ProgressModel, Status};
use domain::progress::Progress;
use domain::user::{Actor, Role};
use kinds::{KindModule, Registry};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use testkit::TestEnv;

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WineAttributes {
    grape: String,
    vintage: Option<u16>,
}

struct Wine;

impl KindModule for Wine {
    type Attributes = WineAttributes;
    const ID: &'static str = "ext:test/wine";
    const PROGRESS_MODEL: ProgressModel = ProgressModel::Single;

    fn extra_statuses() -> Vec<String> {
        vec!["in_cellar".into()]
    }

    fn facets() -> Vec<String> {
        vec!["grape".into()]
    }
}

#[tokio::test]
async fn dummy_kind_works_end_to_end_through_app() {
    let mut registry = Registry::with_builtins();
    registry.register::<Wine>();
    let env = TestEnv::with_kinds(Arc::new(registry));
    let user = env
        .app
        .register_user("sam", "Sam", None, Role::Member)
        .await
        .unwrap();
    let me = Actor {
        user_id: user.id,
        role: user.role,
    };
    let kind = KindId::new("ext:test/wine").unwrap();

    let bad = env
        .app
        .upsert_catalog_item(NewCatalogItem {
            kind: kind.clone(),
            title: "Barolo".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![],
            attributes: serde_json::json!({"vintage": 2016}),
            cover_url: None,
        })
        .await;
    assert!(bad.is_err(), "grape is required by the typed attributes");

    let item = env
        .app
        .upsert_catalog_item(NewCatalogItem {
            kind,
            title: "Barolo".into(),
            original_title: None,
            year: Some(2016),
            description: None,
            external_ids: vec![ExternalId::new("cellar", "b-16")],
            attributes: serde_json::json!({"grape": "Nebbiolo", "vintage": 2016}),
            cover_url: None,
        })
        .await
        .unwrap();

    env.app
        .add_to_library(me, item.id, Some(Status::new("in_cellar")))
        .await
        .unwrap();
    env.app
        .log_progress(me, item.id, Progress::Finished, None, None)
        .await
        .unwrap();
    let entry = env.app.entry(me, item.id).await.unwrap().unwrap();
    assert_eq!(entry.status.value, Status::completed());
}
