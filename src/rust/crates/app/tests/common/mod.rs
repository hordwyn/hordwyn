#![allow(dead_code)]

use app::{App, NewCatalogItem};
use domain::catalog::{CatalogItem, ExternalId};
use domain::kind::KindId;
use domain::user::{Actor, Role, User};
use testkit::TestEnv;

pub async fn user(env: &TestEnv, name: &str) -> (User, Actor) {
    let u = env
        .app
        .register_user(name, &name.to_uppercase(), None, Role::Member)
        .await
        .unwrap();
    let a = Actor {
        user_id: u.id,
        role: u.role,
    };
    (u, a)
}

pub async fn item(app: &App, kind: &str, title: &str) -> CatalogItem {
    app.upsert_catalog_item(NewCatalogItem {
        kind: KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(1999),
        description: None,
        external_ids: vec![ExternalId::new("test", title)],
        attributes: serde_json::json!({}),
    })
    .await
    .unwrap()
}
