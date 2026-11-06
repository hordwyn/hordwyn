#![cfg(feature = "tvdb")]
//! `TheTVDB` connector against synthetic fixtures (see docs/connectors/tvdb.md).

use std::path::Path;
use std::sync::Arc;

use app::ports::Lane;
use catalog_core::{Ctx, Details, DetailsRequest, Search, SearchRequest};
use connectors::tvdb::Tvdb;
use domain::catalog::ExternalId;
use testkit::ReplayHttp;

#[tokio::test]
async fn search_details_and_login_once() {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tvdb"),
    ));
    let ctx = Ctx {
        http: http.clone(),
        lane: Lane::Interactive,
    };
    let c = Tvdb::new(Some("key:1234".into()));
    let hits = c
        .search(
            &ctx,
            &SearchRequest {
                kind: "show".into(),
                query: "breaking bad".into(),
                language: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits[0].title, "Breaking Bad");
    assert_eq!(hits[0].year, Some(2008));
    assert_eq!(hits[0].external_ids, vec![ExternalId::new("tvdb", "81189")]);
    let d = c
        .details(
            &ctx,
            &DetailsRequest {
                kind: "show".into(),
                id: "81189".into(),
                language: None,
            },
        )
        .await
        .unwrap();
    assert!(
        d.external_ids
            .contains(&ExternalId::new("imdb", "tt0903747"))
    );
    assert!(d.external_ids.contains(&ExternalId::new("tmdb", "1396")));
    assert_eq!(d.attributes["network"], "AMC");
    assert_eq!(d.attributes["airing_status"], "ended");
    assert_eq!(
        d.attributes["seasons"].as_array().unwrap().len(),
        3,
        "aired order only"
    );
    let kinds = kinds::Registry::with_builtins();
    app::ports::KindCatalog::validate_attributes(
        &kinds,
        &domain::kind::KindId::new("show").unwrap(),
        &d.attributes,
    )
    .unwrap();
    let reqs = http.requests();
    assert_eq!(reqs.iter().filter(|r| r.op == "login").count(), 1);
    let login = reqs.iter().find(|r| r.op == "login").unwrap();
    let body: serde_json::Value = serde_json::from_slice(login.body.as_deref().unwrap()).unwrap();
    assert_eq!(body, serde_json::json!({"apikey": "key", "pin": "1234"}));
}
