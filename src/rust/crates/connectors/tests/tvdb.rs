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
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
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

/// The user-supported model (design.md 4.1, DEC-246): a member's own subscriber PIN logs in
/// for that member's requests; each PIN keeps its own token, and requests without a member
/// PIN use the instance key (with its PIN, if any).
#[tokio::test]
async fn a_members_own_pin_logs_in_for_their_requests() {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tvdb"),
    ));
    let ctx = |pin: Option<&str>| {
        Ctx::new(http.clone(), Lane::Interactive).with_user_key(pin.map(String::from))
    };
    let req = SearchRequest {
        kind: "show".into(),
        query: "breaking bad".into(),
        language: None,
        limit: 10,
    };
    let c = Tvdb::new(Some("key:1234".into()));
    for pin in [Some("5678"), Some("5678"), None, Some("9999"), None] {
        let hits = c.search(&ctx(pin), &req).await.unwrap();
        assert_eq!(hits[0].title, "Breaking Bad");
    }
    let bodies: Vec<serde_json::Value> = http
        .requests()
        .iter()
        .filter(|r| r.op == "login")
        .map(|r| serde_json::from_slice(r.body.as_deref().unwrap()).unwrap())
        .collect();
    assert_eq!(
        bodies,
        vec![
            serde_json::json!({"apikey": "key", "pin": "5678"}),
            serde_json::json!({"apikey": "key", "pin": "1234"}),
            serde_json::json!({"apikey": "key", "pin": "9999"}),
        ],
        "one login per PIN, tokens kept apart"
    );

    // a project key without a PIN of its own: the member's PIN is the only one
    let c = Tvdb::new(Some("key".into()));
    c.search(&ctx(Some("4321")), &req).await.unwrap();
    let last = http
        .requests()
        .into_iter()
        .rfind(|r| r.op == "login")
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(last.body.as_deref().unwrap()).unwrap();
    assert_eq!(body, serde_json::json!({"apikey": "key", "pin": "4321"}));
}

/// UX2-S2: a member's wrong PIN is their problem: the error says so, and every request made
/// with it is marked so the governed client does not record the rejection for the instance.
#[tokio::test]
async fn a_rejected_member_pin_is_a_user_error_and_not_recorded() {
    let http = Arc::new(ReplayHttp::default());
    http.record("POST", "https://api4.thetvdb.com/v4/login", 401, "{}");
    let req = SearchRequest {
        kind: "show".into(),
        query: "breaking bad".into(),
        language: None,
        limit: 10,
    };
    let c = Tvdb::new(Some("key".into()));
    let member = Ctx::new(http.clone(), Lane::Interactive).with_user_key(Some("0000".into()));
    let err = c.search(&member, &req).await.unwrap_err();
    assert_eq!(
        err,
        catalog_core::ConnectorError::UserCredentialRejected { status: 401 }
    );
    assert!(http.requests().iter().all(|r| r.skip_auth_record));
    // the instance key alone: an ordinary rejection, recorded as usual
    let err = c
        .search(&Ctx::new(http.clone(), Lane::Interactive), &req)
        .await
        .unwrap_err();
    assert_eq!(err, catalog_core::ConnectorError::Status { status: 401 });
    assert!(!http.requests().last().unwrap().skip_auth_record);
}

/// UX2-S11: a malformed base URL from the configuration is an error, not a panic.
#[tokio::test]
async fn a_bad_base_url_is_an_error() {
    let http = Arc::new(ReplayHttp::default());
    http.record("POST", "not a url/login", 200, r#"{"data":{"token":"t"}}"#);
    let c = Tvdb::with_base_url(Some("key".into()), "not a url");
    let r = c
        .search(
            &Ctx::new(http, Lane::Interactive),
            &SearchRequest {
                kind: "show".into(),
                query: "x".into(),
                language: None,
                limit: 1,
            },
        )
        .await;
    assert!(
        matches!(r, Err(catalog_core::ConnectorError::Parse(_))),
        "{r:?}"
    );
}
