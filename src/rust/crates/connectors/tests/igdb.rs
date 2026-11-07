#![cfg(feature = "igdb")]
//! IGDB connector against synthetic fixtures (see docs/connectors/igdb.md).

use std::path::Path;
use std::sync::Arc;

use app::ports::Lane;
use catalog_core::{
    ConnectorError, Ctx, Details, DetailsRequest, ImageKind, Search, SearchRequest,
};
use connectors::igdb::Igdb;
use domain::catalog::ExternalId;
use testkit::ReplayHttp;

fn ctx() -> (Ctx, Arc<ReplayHttp>) {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/igdb"),
    ));
    (Ctx::new(http.clone(), Lane::Interactive), http)
}

fn igdb() -> Igdb {
    Igdb::new(Some("test-client:test-secret".into()))
}

#[tokio::test]
async fn search_details_and_images() {
    let (ctx, http) = ctx();
    let c = igdb();
    let hits = c
        .search(
            &ctx,
            &SearchRequest {
                kind: "video_game".into(),
                query: "halo".into(),
                language: None,
                limit: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].title, "Halo: Combat Evolved");
    assert_eq!(hits[0].year, Some(2001));
    assert_eq!(hits[0].external_ids, vec![ExternalId::new("igdb", "740")]);
    let cover = hits[0]
        .images
        .iter()
        .find(|i| i.kind == ImageKind::Cover)
        .unwrap();
    assert_eq!(
        cover.url,
        "https://images.igdb.com/igdb/image/upload/t_cover_big_2x/co2r2r.jpg"
    );
    let d = c
        .details(
            &ctx,
            &DetailsRequest {
                kind: "video_game".into(),
                id: "740".into(),
                language: None,
            },
        )
        .await
        .unwrap();
    assert!(d.complete);
    assert_eq!(d.attributes["developers"], serde_json::json!(["Bungie"]));
    assert_eq!(
        d.attributes["publishers"],
        serde_json::json!(["Microsoft Game Studios"])
    );
    assert_eq!(d.attributes["release_date"], "2001-11-15");
    // one token request, reused for both calls; headers as documented
    let reqs = http.requests();
    assert_eq!(reqs.iter().filter(|r| r.op == "token").count(), 1);
    // UX2-S1: the client secret goes in a form body, never in the URL (Twitch docs, client
    // credentials grant flow)
    let token = reqs.iter().find(|r| r.op == "token").unwrap();
    assert_eq!(token.url, "https://id.twitch.tv/oauth2/token");
    assert!(token.headers.contains(&(
        "Content-Type".into(),
        "application/x-www-form-urlencoded".into()
    )));
    assert_eq!(
        token.body.as_deref(),
        Some(&b"client_id=test-client&client_secret=test-secret&grant_type=client_credentials"[..])
    );
    let search = reqs.iter().find(|r| r.op == "search").unwrap();
    assert!(
        search
            .headers
            .contains(&("Client-ID".into(), "test-client".into()))
    );
    assert!(
        search
            .headers
            .contains(&("Authorization".into(), "Bearer test-access-token".into()))
    );
    assert_eq!(search.method, "POST");
}

#[tokio::test]
async fn validates_against_the_kind_schema() {
    let (ctx, _) = ctx();
    let d = igdb()
        .details(
            &ctx,
            &DetailsRequest {
                kind: "video_game".into(),
                id: "740".into(),
                language: None,
            },
        )
        .await
        .unwrap();
    let kinds = kinds::Registry::with_builtins();
    app::ports::KindCatalog::validate_attributes(
        &kinds,
        &domain::kind::KindId::new("video_game").unwrap(),
        &d.attributes,
    )
    .unwrap();
}

#[tokio::test]
async fn missing_credentials_and_wrong_kind() {
    let (ctx, _) = ctx();
    let req = SearchRequest {
        kind: "video_game".into(),
        query: "halo".into(),
        language: None,
        limit: 5,
    };
    assert!(matches!(
        Igdb::new(None).search(&ctx, &req).await,
        Err(ConnectorError::MissingCredentials(_))
    ));
    let movie = SearchRequest {
        kind: "movie".into(),
        ..req
    };
    assert!(matches!(
        igdb().search(&ctx, &movie).await,
        Err(ConnectorError::Unsupported(_))
    ));
}
