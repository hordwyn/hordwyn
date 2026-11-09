//! WP4-7: per-episode release dates of a show (`catalog_episodes`): which seasons a refresh
//! asks for, storing them idempotently, and the background job that fetches them.

mod common;

use app::{SeasonFetch, SeasonKey};
use chrono::NaiveDate;
use domain::catalog::CatalogEpisode;
use testkit::TestEnv;

fn day(y: i32, m: u32, d: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(y, m, d)
}

fn ep(season: u32, number: u32, air_date: Option<NaiveDate>) -> CatalogEpisode {
    CatalogEpisode {
        season,
        number,
        air_date,
        runtime_minutes: Some(50),
    }
}

async fn show(env: &TestEnv, seasons: serde_json::Value) -> domain::catalog::CatalogItem {
    let mut item = common::item(&env.app, "show", "Harbour Watch").await;
    item.attributes = serde_json::json!({ "seasons": seasons });
    env.store.state().catalog.insert(item.id, item.clone());
    item
}

fn fetch(key: SeasonKey, episodes: Vec<CatalogEpisode>) -> SeasonFetch {
    SeasonFetch {
        key,
        connector: "tmdb".into(),
        episodes,
    }
}

#[tokio::test]
async fn season_keys_come_from_the_regular_seasons() {
    let keys = app::season_keys(&serde_json::json!({"seasons": [
        {"number": 0, "episode_count": 3},
        {"number": 2, "episode_count": 4},
        {"number": 1, "episode_count": 2, "air_date": "2025-03-01"},
    ]}));
    assert_eq!(
        keys,
        vec![
            SeasonKey {
                number: 1,
                episode_count: 2,
                air_date: day(2025, 3, 1)
            },
            SeasonKey {
                number: 2,
                episode_count: 4,
                air_date: None
            },
        ]
    );
    assert_eq!(app::season_keys(&serde_json::json!({})).len(), 0);
}

#[tokio::test]
async fn storing_a_season_is_idempotent_and_skips_it_next_time() {
    let env = TestEnv::new();
    let item = show(
        &env,
        serde_json::json!([
            {"number": 1, "episode_count": 2, "air_date": "2025-03-01"},
            {"number": 2, "episode_count": 2, "air_date": "2025-09-01"},
        ]),
    )
    .await;
    // someone has the show, so a change of it is announced (to them, DEC-490)
    let (_, viewer) = common::user(&env, "alice").await;
    env.app.add_to_library(viewer, item.id, None).await.unwrap();
    let all = env.app.seasons_to_fetch(item.id).await.unwrap();
    assert_eq!(all.iter().map(|k| k.number).collect::<Vec<_>>(), [1, 2]);

    let s1 = fetch(
        all[0].clone(),
        vec![ep(1, 2, day(2025, 3, 8)), ep(1, 1, day(2025, 3, 1))],
    );
    // season 2: its second episode airs after today (2026-01-01 on the test clock)
    let s2 = fetch(
        all[1].clone(),
        vec![ep(2, 1, day(2025, 9, 1)), ep(2, 2, day(2026, 2, 1))],
    );
    env.events.clear();
    assert_eq!(
        env.app
            .store_episodes(item.id, &[s1.clone(), s2.clone()])
            .await
            .unwrap(),
        2
    );
    assert_eq!(env.events.topics(), vec!["entity.updated".to_string()]);
    let stored = env.app.catalog_episodes(item.id).await.unwrap();
    assert_eq!(
        stored
            .iter()
            .map(|e| (e.season, e.number))
            .collect::<Vec<_>>(),
        [(1, 1), (1, 2), (2, 1), (2, 2)],
        "in order"
    );

    // the same data again changes nothing and announces nothing
    env.events.clear();
    assert_eq!(
        env.app
            .store_episodes(item.id, std::slice::from_ref(&s1))
            .await
            .unwrap(),
        0
    );
    assert_eq!(env.app.catalog_episodes(item.id).await.unwrap(), stored);
    assert_eq!(env.events.topics().len(), 0);

    // season 1 is complete and aired: skipped; season 2 has an episode still to air: asked
    let next = env.app.seasons_to_fetch(item.id).await.unwrap();
    assert_eq!(next.iter().map(|k| k.number).collect::<Vec<_>>(), [2]);

    // a season whose episode count changed is asked for again, and a fetch replaces it
    let mut grown = all[0].clone();
    grown.episode_count = 3;
    let mut changed = item.clone();
    changed.attributes = serde_json::json!({"seasons": [
        {"number": 1, "episode_count": 3, "air_date": "2025-03-01"},
        {"number": 2, "episode_count": 2, "air_date": "2025-09-01"},
    ]});
    env.store.state().catalog.insert(item.id, changed);
    let next = env.app.seasons_to_fetch(item.id).await.unwrap();
    assert_eq!(next.iter().map(|k| k.number).collect::<Vec<_>>(), [1, 2]);
    env.app
        .store_episodes(item.id, &[fetch(grown, vec![ep(1, 3, day(2025, 3, 15))])])
        .await
        .unwrap();
    let after = env.app.catalog_episodes(item.id).await.unwrap();
    assert_eq!(
        after
            .iter()
            .map(|e| (e.season, e.number))
            .collect::<Vec<_>>(),
        [(1, 3), (2, 1), (2, 2)],
        "a fetched season replaces what was stored for it"
    );
}

#[tokio::test]
async fn the_episode_job_is_queued_on_the_background_lane_once() {
    let env = TestEnv::new();
    let item = show(&env, serde_json::json!([{"number": 1, "episode_count": 1}])).await;
    assert!(env.app.queue_episode_refresh(item.id).await.unwrap());
    let jobs = env.jobs.jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].kind, app::EPISODES_JOB);
    assert_eq!(jobs[0].lane, app::ports::Lane::BackgroundRefresh);
    assert_eq!(
        jobs[0].dedupe_key.as_deref(),
        Some(format!("{}:{}", app::EPISODES_JOB, item.id).as_str())
    );
    // nothing left to ask for: no job
    let key = env.app.seasons_to_fetch(item.id).await.unwrap().remove(0);
    env.app
        .store_episodes(item.id, &[fetch(key, vec![ep(1, 1, day(2025, 1, 1))])])
        .await
        .unwrap();
    assert!(!env.app.queue_episode_refresh(item.id).await.unwrap());
    // a movie has no seasons
    let movie = common::item(&env.app, "movie", "Harbour Lights").await;
    assert!(!env.app.queue_episode_refresh(movie.id).await.unwrap());
    assert_eq!(env.jobs.jobs().len(), 1);
}
