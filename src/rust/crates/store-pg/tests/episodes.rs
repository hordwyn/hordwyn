//! WP4-7 on Postgres (v1 baseline migration): a season's episodes are stored with what they were
//! fetched for, replaced as a whole, kept idempotent, and go with their item.

mod common;

use app::{SeasonFetch, SeasonKey};
use chrono::NaiveDate;
use domain::catalog::CatalogEpisode;

fn ep(season: u32, number: u32, air: Option<NaiveDate>, runtime: Option<u32>) -> CatalogEpisode {
    CatalogEpisode {
        season,
        number,
        air_date: air,
        runtime_minutes: runtime,
    }
}

#[tokio::test]
async fn season_episodes_round_trip_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let mut show = common::item(app, "show", "Harbour Watch").await;
    show.attributes = serde_json::json!({"seasons": [
        {"number": 1, "episode_count": 2, "air_date": "2025-03-01"},
        {"number": 2, "episode_count": 1},
    ]});
    app.repos.catalog.upsert(&show).await.unwrap();
    let keys = app.seasons_to_fetch(show.id).await.unwrap();
    assert_eq!(keys.iter().map(|k| k.number).collect::<Vec<_>>(), [1, 2]);
    let day = NaiveDate::from_ymd_opt;
    let s1 = SeasonFetch {
        key: keys[0].clone(),
        connector: "tmdb".into(),
        episodes: vec![
            ep(1, 2, day(2025, 3, 8), None),
            ep(1, 1, day(2025, 3, 1), Some(45)),
        ],
    };
    let s2 = SeasonFetch {
        key: keys[1].clone(),
        connector: "tmdb".into(),
        episodes: vec![ep(2, 1, None, Some(50))],
    };
    assert_eq!(
        app.store_episodes(show.id, &[s1.clone(), s2])
            .await
            .unwrap(),
        2
    );
    let stored = app.catalog_episodes(show.id).await.unwrap();
    assert_eq!(
        stored,
        vec![
            ep(1, 1, day(2025, 3, 1), Some(45)),
            ep(1, 2, day(2025, 3, 8), None),
            ep(2, 1, None, Some(50)),
        ]
    );
    let seasons = app.repos.catalog.episode_seasons(show.id).await.unwrap();
    assert_eq!(seasons[0].air_date, day(2025, 3, 1));
    assert_eq!((seasons[0].episode_count, seasons[1].air_date), (2, None));
    // season 1 is complete and aired; season 2 has an episode without a date
    let next: Vec<u32> = app
        .seasons_to_fetch(show.id)
        .await
        .unwrap()
        .iter()
        .map(|k| k.number)
        .collect();
    assert_eq!(next, [2]);

    // the same answer again: nothing changes
    assert_eq!(app.store_episodes(show.id, &[s1]).await.unwrap(), 0);
    assert_eq!(app.catalog_episodes(show.id).await.unwrap(), stored);

    // a new answer replaces the season as a whole
    let fewer = SeasonFetch {
        key: SeasonKey {
            episode_count: 1,
            ..keys[0].clone()
        },
        connector: "tvdb".into(),
        episodes: vec![ep(1, 1, day(2025, 3, 2), None)],
    };
    assert_eq!(app.store_episodes(show.id, &[fewer]).await.unwrap(), 1);
    let after = app.catalog_episodes(show.id).await.unwrap();
    assert_eq!(
        after,
        vec![ep(1, 1, day(2025, 3, 2), None), ep(2, 1, None, Some(50))]
    );
    let seasons = app.repos.catalog.episode_seasons(show.id).await.unwrap();
    assert_eq!(
        (seasons[0].connector.as_str(), seasons[0].episode_count),
        ("tvdb", 1)
    );
}
