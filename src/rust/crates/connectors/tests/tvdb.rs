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
    // OT-27 (#3): the counts and first air dates come from the aired-order episode list,
    // read page by page; season 3 is only in that list, the DVD season 1 in neither
    assert_eq!(
        d.attributes["seasons"],
        serde_json::json!([
            {"number": 0, "episode_count": 1, "air_date": "2009-02-17"},
            {"number": 1, "episode_count": 3, "air_date": "2008-01-20"},
            {"number": 2, "episode_count": 2, "air_date": "2009-03-08"},
            {"number": 3, "episode_count": 1, "air_date": "2010-03-21"},
        ]),
        "aired order only"
    );
    let pages: Vec<String> = http
        .requests()
        .into_iter()
        .filter(|r| r.op == "episodes")
        .map(|r| r.url)
        .collect();
    assert_eq!(
        pages,
        [
            "https://api4.thetvdb.com/v4/series/81189/episodes/official?page=0",
            "https://api4.thetvdb.com/v4/series/81189/episodes/official?page=1",
        ]
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

/// DEC-286: the token cache is keyed by a hash of the PIN, so the PIN itself is not kept as
/// a map key for the life of the process.
#[tokio::test]
async fn the_token_cache_key_is_not_the_pin() {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tvdb"),
    ));
    let req = SearchRequest {
        kind: "show".into(),
        query: "breaking bad".into(),
        language: None,
        limit: 10,
    };
    let c = Tvdb::new(Some("key:1234".into()));
    for pin in [Some("5678"), None, Some("5678")] {
        let ctx = Ctx::new(http.clone(), Lane::Interactive).with_user_key(pin.map(String::from));
        c.search(&ctx, &req).await.unwrap();
    }
    let slots = c.token_slots();
    assert_eq!(slots.len(), 2, "one token per PIN: {slots:?}");
    for s in &slots {
        assert!(!s.contains("5678") && !s.contains("1234"), "{s}");
        assert_eq!(s.len(), 64, "a blake3 hex digest: {s}");
        // UX4-22: a keyed derivation, not the bare hash of the PIN (no lookup table)
        let bare = blake3::hash(b"5678").to_hex().to_string();
        assert_ne!(*s, bare, "salted");
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()), "{s}");
    }
    let logins = http.requests().iter().filter(|r| r.op == "login").count();
    assert_eq!(logins, 2, "the second search with 5678 reused its token");
}

/// WP4-7: `GET /series/{id}/episodes/official?page=0&season=N` (getSeriesEpisodes, v4 spec)
/// keeps the episode number, `aired` and `runtime` of the asked season only.
#[tokio::test]
async fn season_episodes_keep_number_aired_and_runtime() {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tvdb"),
    ));
    let ctx = Ctx::new(http.clone(), Lane::BackgroundRefresh);
    let c = Tvdb::new(Some("key".into()));
    let eps = catalog_core::Episodes::season_episodes(
        &c,
        &ctx,
        &catalog_core::EpisodesRequest {
            kind: "show".into(),
            id: "81189".into(),
            season: 1,
        },
    )
    .await
    .unwrap();
    let day = chrono::NaiveDate::from_ymd_opt;
    assert_eq!(
        eps.iter()
            .map(|e| (e.season, e.number, e.air_date, e.runtime_minutes))
            .collect::<Vec<_>>(),
        vec![
            (1, 1, day(2008, 1, 20), Some(58)),
            (1, 2, day(2008, 1, 27), None),
            (1, 3, None, Some(47)),
        ],
        "the special of season 0 is left out"
    );
    let sent = http
        .requests()
        .into_iter()
        .find(|r| r.op == "season")
        .unwrap();
    assert_eq!(
        sent.url,
        "https://api4.thetvdb.com/v4/series/81189/episodes/official?page=0&season=1"
    );
}
