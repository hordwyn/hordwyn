#![cfg(feature = "podcastindex")]
//! Podcast Index connector against synthetic fixtures (see docs/connectors/podcastindex.md).

use std::path::Path;
use std::sync::Arc;

use app::ports::Lane;
use catalog_core::{Ctx, Details, DetailsRequest, ImageKind, Search, SearchRequest};
use connectors::podcastindex::{PodcastIndex, signature};
use domain::catalog::ExternalId;
use testkit::ReplayHttp;

#[tokio::test]
async fn search_details_and_signed_requests() {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/podcastindex"),
    ));
    let ctx = Ctx {
        http: http.clone(),
        lane: Lane::Interactive,
    };
    let c =
        PodcastIndex::new(Some("UXKCGDSYGUUEVQJSYDZH:secret".into())).with_clock(|| 1_613_713_388);
    let hits = c
        .search(
            &ctx,
            &SearchRequest {
                kind: "podcast".into(),
                query: "batman university".into(),
                language: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Batman University");
    assert!(
        hits[0]
            .external_ids
            .contains(&ExternalId::new("podcastindex", "75075"))
    );
    assert!(
        hits[0]
            .external_ids
            .contains(&ExternalId::new("itunes", "1441923632"))
    );
    assert_eq!(hits[0].images[0].kind, ImageKind::Cover);
    let d = c
        .details(
            &ctx,
            &DetailsRequest {
                kind: "podcast".into(),
                id: "75075".into(),
                language: None,
            },
        )
        .await
        .unwrap();
    assert!(d.complete);
    assert_eq!(d.attributes["episode_count"], 19);
    assert_eq!(d.attributes["author"], "Tony Sindelar");
    assert_eq!(
        d.attributes["categories"],
        serde_json::json!(["Tv", "Film", "Reviews"])
    );
    kinds_validate(&d.attributes);
    let h = &http.requests()[0].headers;
    assert!(h.contains(&("X-Auth-Key".into(), "UXKCGDSYGUUEVQJSYDZH".into())));
    assert!(h.contains(&("X-Auth-Date".into(), "1613713388".into())));
    assert!(h.contains(&(
        "Authorization".into(),
        signature("UXKCGDSYGUUEVQJSYDZH", "secret", 1_613_713_388)
    )));
}

fn kinds_validate(v: &serde_json::Value) {
    let kinds = kinds::Registry::with_builtins();
    app::ports::KindCatalog::validate_attributes(
        &kinds,
        &domain::kind::KindId::new("podcast").unwrap(),
        v,
    )
    .unwrap();
}
