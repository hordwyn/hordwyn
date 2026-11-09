#![cfg(feature = "tmdb")]
//! TMDB connector against the fixtures in `fixtures/tmdb` (synthetic, see
//! docs/connectors/tmdb.md).

use std::path::Path;
use std::sync::Arc;

use app::ports::{KindCatalog, Lane};
use catalog_core::{ConnectorError, Ctx, DetailsRequest, ImageKind, Registry, SearchRequest};
use connectors::tmdb::Tmdb;
use domain::catalog::{EntityState, ExternalId};
use domain::kind::KindId;
use testkit::{ReplayHttp, TestEnv};

fn fixtures() -> Arc<ReplayHttp> {
    Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/tmdb"),
    ))
}

fn ctx(http: Arc<ReplayHttp>) -> Ctx {
    Ctx::new(http, Lane::Interactive)
}

fn tmdb() -> Tmdb {
    Tmdb::new(Some("test-token".into()))
}

#[tokio::test]
async fn search_movies() {
    let http = fixtures();
    let req = SearchRequest {
        kind: "movie".into(),
        query: "fight club".into(),
        language: None,
        limit: 10,
    };
    let hits = catalog_core::Search::search(&tmdb(), &ctx(http.clone()), &req)
        .await
        .unwrap();
    assert_eq!(hits.len(), 10, "limit applies");
    let first = &hits[0];
    assert_eq!(first.title, "Fight Club");
    assert_eq!(first.year, Some(1999));
    assert_eq!(first.external_ids, vec![ExternalId::new("tmdb", "550")]);
    assert!(!first.complete);
    assert_eq!(
        first
            .images
            .iter()
            .find(|i| i.kind == ImageKind::Cover)
            .unwrap()
            .url,
        "https://image.tmdb.org/t/p/w780/pB8BM7pdSp6B6Ih7QZ4DrQ3PmJK.jpg"
    );
    let sent = &http.requests()[0];
    assert!(
        sent.headers
            .contains(&("Authorization".into(), "Bearer test-token".into()))
    );
    assert_eq!(sent.connector, "tmdb");
}

#[tokio::test]
async fn movie_details_have_valid_typed_attributes() {
    let req = DetailsRequest {
        kind: "movie".into(),
        id: "11".into(),
        language: None,
    };
    let s = catalog_core::Details::details(&tmdb(), &ctx(fixtures()), &req)
        .await
        .unwrap();
    assert_eq!((s.title.as_str(), s.year), ("Star Wars", Some(1977)));
    assert!(
        s.external_ids
            .contains(&ExternalId::new("imdb", "tt0076759"))
    );
    assert_eq!(s.attributes["runtime_minutes"], 121);
    assert_eq!(s.attributes["genres"][0], "Adventure");
    let kinds = kinds::Registry::with_builtins();
    kinds
        .validate_attributes(&KindId::new("movie").unwrap(), &s.attributes)
        .unwrap();
    assert!(s.images.iter().any(|i| i.kind == ImageKind::Backdrop));
}

#[tokio::test]
async fn show_search_and_details() {
    let http = fixtures();
    let req = SearchRequest {
        kind: "show".into(),
        query: "breaking bad".into(),
        language: None,
        limit: 5,
    };
    let hits = catalog_core::Search::search(&tmdb(), &ctx(http.clone()), &req)
        .await
        .unwrap();
    assert_eq!(hits[0].title, "Breaking Bad");
    assert_eq!(hits[0].year, Some(2008));

    let req = DetailsRequest {
        kind: "show".into(),
        id: "1399".into(),
        language: None,
    };
    let s = catalog_core::Details::details(&tmdb(), &ctx(http), &req)
        .await
        .unwrap();
    assert_eq!(s.title, "Game of Thrones");
    assert_eq!(
        s.attributes["seasons"][1],
        serde_json::json!({"number": 1, "episode_count": 10, "air_date": "2011-04-17"}),
        "WP4-7: each season keeps its air date"
    );
    assert_eq!(s.attributes["airing_status"], "ended");
    kinds::Registry::with_builtins()
        .validate_attributes(&KindId::new("show").unwrap(), &s.attributes)
        .unwrap();
}

#[tokio::test]
async fn missing_token_and_unknown_kind() {
    let req = DetailsRequest {
        kind: "movie".into(),
        id: "11".into(),
        language: None,
    };
    let err = catalog_core::Details::details(&Tmdb::new(None), &ctx(fixtures()), &req)
        .await
        .unwrap_err();
    assert!(matches!(err, ConnectorError::MissingCredentials(_)));
    let req = DetailsRequest {
        kind: "book".into(),
        id: "1".into(),
        language: None,
    };
    assert!(matches!(
        catalog_core::Details::details(&tmdb(), &ctx(fixtures()), &req).await,
        Err(ConnectorError::Unsupported(_))
    ));
}

#[test]
fn info_declares_hosts_and_attribution() {
    let t = tmdb();
    let info = catalog_core::Connector::info(&t);
    assert_eq!(
        info.hosts,
        vec![
            "api.themoviedb.org".to_string(),
            "image.tmdb.org".to_string()
        ]
    );
    assert!(
        info.attribution
            .text
            .contains("not endorsed, certified, or otherwise approved by TMDB")
    );
}

/// Core + one connector + one kind end to end: a stub item is refreshed through the
/// connector, merged, validated by the real kind registry, stored and announced.
#[tokio::test]
async fn refresh_turns_a_stub_into_a_full_item() {
    let env = TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let item = env
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: KindId::new("movie").unwrap(),
            title: "Star Wars".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![ExternalId::new("tmdb", "11")],
            attributes: serde_json::json!({}),
            cover_url: None,
        })
        .await
        .unwrap();
    // updates go only to people the item matters to (DEC-490): someone has opened it
    let user = env
        .app
        .register_user("ana", "Ana", None, domain::user::Role::Member)
        .await
        .unwrap();
    let actor = domain::user::Actor {
        user_id: user.id,
        role: user.role,
    };
    env.app.watch_item(actor, item.id).await.unwrap();
    let mut registry = Registry::default();
    registry.register(Arc::new(tmdb()));
    env.events.clear();

    catalog_core::refresh::refresh_item(
        &env.app,
        &registry,
        fixtures(),
        Lane::Interactive,
        item.id,
    )
    .await
    .unwrap();

    let after = env.app.catalog_item(item.id).await.unwrap();
    assert_eq!(after.state, EntityState::Full);
    assert_eq!(after.year, Some(1977));
    assert_eq!(after.attributes["runtime_minutes"], 121);
    assert_eq!(after.provenance["title"], "tmdb");
    assert!(
        after
            .cover_url
            .as_deref()
            .unwrap()
            .starts_with("https://image.tmdb.org/t/p/w780/")
    );
    assert!(
        after
            .external_ids
            .contains(&ExternalId::new("imdb", "tt0076759"))
    );
    assert!(!after.needs_refresh(env.clock.now_for_tests()));
    assert_eq!(env.events.topics(), vec!["entity.updated".to_string()]);
}

/// Live smoke test; needs `KK_TMDB_API_KEY`.
#[tokio::test]
#[ignore = "talks to the real TMDB API"]
async fn live_search() {
    let Ok(key) = std::env::var(connectors::tmdb::API_KEY_ENV) else {
        panic!("set {} to run", connectors::tmdb::API_KEY_ENV)
    };
    let t = Tmdb::new(Some(key));
    let mut http = catalog_core::GovernedHttp::new("tracker-live-test", None);
    http.add_connector(
        "tmdb",
        catalog_core::ConnectorPolicy::from_info(catalog_core::Connector::info(&t)),
    );
    let ctx = Ctx::new(Arc::new(http), Lane::Interactive);
    let req = SearchRequest {
        kind: "movie".into(),
        query: "fight club".into(),
        language: None,
        limit: 3,
    };
    let hits = catalog_core::Search::search(&t, &ctx, &req).await.unwrap();
    assert!(
        hits.iter()
            .any(|h| h.external_ids.contains(&ExternalId::new("tmdb", "550")))
    );
}

/// design.md 4.2: the key lives in a slot the server updates; a key entered in the admin UI
/// is used by the next request, no restart.
#[tokio::test]
async fn a_new_key_in_the_slot_is_used_by_the_next_request() {
    use catalog_core::Connector;
    let http = fixtures();
    let t = Tmdb::new(None);
    let req = SearchRequest {
        kind: "movie".into(),
        query: "fight club".into(),
        language: None,
        limit: 1,
    };
    assert!(matches!(
        catalog_core::Search::search(&t, &ctx(http.clone()), &req).await,
        Err(ConnectorError::MissingCredentials(_))
    ));
    assert_eq!(t.credentials().instance_fields, &["api_key"]);
    t.key_slot().unwrap().set(Some("from-the-ui".into()));
    catalog_core::Search::search(&t, &ctx(http.clone()), &req)
        .await
        .unwrap();
    assert!(
        http.requests()[0]
            .headers
            .contains(&("Authorization".into(), "Bearer from-the-ui".into()))
    );
}

/// UX2-S11: a malformed base URL from the configuration is an error, not a panic.
#[tokio::test]
async fn a_bad_base_url_is_an_error() {
    let c = Tmdb::with_base_urls(Some("t".into()), "not a url", "not a url either");
    let req = SearchRequest {
        kind: "movie".into(),
        query: "x".into(),
        language: None,
        limit: 1,
    };
    let r = catalog_core::Search::search(&c, &ctx(fixtures()), &req).await;
    assert!(matches!(r, Err(ConnectorError::Parse(_))), "{r:?}");
}

/// WP4-7: `GET /3/tv/{series_id}/season/{season_number}` keeps only the episode number, the
/// air date and the runtime (TV Seasons > Details, docs/connectors/tmdb.md).
#[tokio::test]
async fn season_episodes_keep_number_air_date_and_runtime() {
    let http = fixtures();
    let req = catalog_core::EpisodesRequest {
        kind: "show".into(),
        id: "1399".into(),
        season: 1,
    };
    let eps = catalog_core::Episodes::season_episodes(&tmdb(), &ctx(http.clone()), &req)
        .await
        .unwrap();
    assert_eq!(eps.len(), 10);
    assert_eq!(
        eps[0],
        domain::catalog::CatalogEpisode {
            season: 1,
            number: 1,
            air_date: chrono::NaiveDate::from_ymd_opt(2011, 4, 17),
            runtime_minutes: Some(62),
        }
    );
    assert_eq!(eps[9].number, 10);
    assert_eq!(
        eps[9].air_date,
        chrono::NaiveDate::from_ymd_opt(2011, 6, 19)
    );
    assert_eq!(eps[9].runtime_minutes, Some(53));
    let sent = &http.requests()[0];
    assert_eq!(
        sent.url,
        "https://api.themoviedb.org/3/tv/1399/season/1?language=en-US"
    );
    assert_eq!(sent.op, "season");
    // a movie has no seasons
    let movie = catalog_core::EpisodesRequest {
        kind: "movie".into(),
        id: "11".into(),
        season: 1,
    };
    assert!(matches!(
        catalog_core::Episodes::season_episodes(&tmdb(), &ctx(fixtures()), &movie).await,
        Err(ConnectorError::Unsupported(_))
    ));
}

async fn show_stub(env: &TestEnv) -> domain::catalog::CatalogItem {
    env.app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: KindId::new("show").unwrap(),
            title: "Game of Thrones".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![ExternalId::new("tmdb", "1399")],
            attributes: serde_json::json!({}),
            cover_url: None,
        })
        .await
        .unwrap()
}

fn season_calls(http: &ReplayHttp, season: u32) -> usize {
    let path = format!("/3/tv/1399/season/{season}?");
    http.requests()
        .iter()
        .filter(|r| r.url.contains(&path))
        .count()
}

/// WP4-7: the detail refresh queues the episode fetch on the background lane; that job asks
/// once per season, stores the dates and, on the next run, skips a season whose episode count
/// and air date did not change and whose episodes have all aired.
#[tokio::test]
async fn the_episode_refresh_fetches_each_season_once_and_skips_unchanged_ones() {
    let env = TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let item = show_stub(&env).await;
    let mut registry = Registry::default();
    registry.register(Arc::new(tmdb()));
    let http = fixtures();
    catalog_core::refresh::refresh_item(
        &env.app,
        &registry,
        http.clone(),
        Lane::Interactive,
        item.id,
    )
    .await
    .unwrap();
    // no season fetched by the detail refresh itself: it is queued on the background lane
    assert_eq!(season_calls(&http, 1), 0);
    let queued: Vec<_> = env
        .jobs
        .jobs()
        .into_iter()
        .filter(|j| j.kind == app::EPISODES_JOB)
        .collect();
    assert_eq!(queued.len(), 1, "{queued:?}");
    assert_eq!(queued[0].lane, Lane::BackgroundRefresh);

    let stored = catalog_core::refresh::refresh_episodes(
        &env.app,
        &registry,
        http.clone(),
        Lane::BackgroundRefresh,
        item.id,
    )
    .await
    .unwrap();
    assert_eq!(
        stored, 1,
        "season 1 has a fixture, the others fail and are left out"
    );
    assert_eq!(season_calls(&http, 1), 1);
    assert_eq!(season_calls(&http, 2), 1);
    let eps = env.app.catalog_episodes(item.id).await.unwrap();
    assert_eq!(eps.len(), 10);
    assert_eq!(
        eps[4].air_date,
        chrono::NaiveDate::from_ymd_opt(2011, 5, 15)
    );

    // the same season data again: season 1 is not asked for again, the failed ones are
    env.events.clear();
    let again = catalog_core::refresh::refresh_episodes(
        &env.app,
        &registry,
        http.clone(),
        Lane::BackgroundRefresh,
        item.id,
    )
    .await
    .unwrap();
    assert_eq!(again, 0);
    assert_eq!(season_calls(&http, 1), 1);
    assert_eq!(season_calls(&http, 2), 2);
    assert_eq!(env.app.catalog_episodes(item.id).await.unwrap(), eps);
    assert_eq!(
        env.events.topics().len(),
        0,
        "nothing changed, nothing announced"
    );
}
