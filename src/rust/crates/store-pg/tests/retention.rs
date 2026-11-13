//! Catalog retention on Postgres (v1 baseline migration, DEC-453): when connector data was obtained,
//! which items are old, and what a purge keeps.

mod common;

use app::{CatalogRefresh, NewCatalogItem, SeasonFetch};
use chrono::{Duration, TimeZone, Utc};
use domain::catalog::{CatalogEpisode, EntityState, ExternalId};
use domain::kind::KindId;

fn show(title: &str, ns: &str) -> NewCatalogItem {
    NewCatalogItem {
        kind: KindId::new("show").unwrap(),
        title: title.into(),
        original_title: Some("Original".into()),
        year: Some(2011),
        description: Some("From the source.".into()),
        external_ids: vec![ExternalId::new(ns, title)],
        attributes: serde_json::json!({"seasons": [{"number": 1, "episode_count": 1}]}),
        cover_url: Some("https://image.example/a.jpg".into()),
        popularity: Some(1200.0),
    }
}

fn refresh() -> CatalogRefresh {
    CatalogRefresh {
        title: "Harbour".into(),
        original_title: None,
        year: Some(2011),
        description: Some("Fresh.".into()),
        attributes: serde_json::json!({"seasons": [{"number": 1, "episode_count": 1}]}),
        external_ids: vec![],
        cover_url: None,
        provenance: std::collections::BTreeMap::new(),
        fresh_for_secs: 60,
        popularity: Some(1500.0),
    }
}

/// The stored popularity of an item (OT1B-3).
async fn popularity(app: &app::App, id: domain::ids::ItemId) -> Option<f64> {
    app.catalog_item(id).await.unwrap().popularity
}

#[tokio::test]
async fn provider_data_is_stamped_found_and_purged_on_postgres() {
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let ns = vec!["tmdb".to_string()];
    // FixedClock starts at 2026-01-01
    let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let a = app
        .upsert_catalog_item(show("Harbour", "tmdb"))
        .await
        .unwrap();
    let other = app
        .upsert_catalog_item(show("Ronin", "test"))
        .await
        .unwrap();
    assert_eq!(
        app.stale_provider_data(&ns, t0 + Duration::seconds(1), 10)
            .await
            .unwrap(),
        vec![(a.id, t0)]
    );
    assert_eq!(app.stale_provider_data(&ns, t0, 10).await.unwrap(), vec![]);

    // a failed refresh keeps the stamp, a successful one moves it
    app.mark_catalog_refresh_failed(a.id, 60).await.unwrap();
    app.set_cover_asset(a.id, "asset-1".into()).await.unwrap();
    assert_eq!(
        app.stale_provider_data(&ns, t0 + Duration::seconds(1), 10)
            .await
            .unwrap()
            .len(),
        1
    );
    let clock = testkit::FixedClock::default();
    clock.set(t0 + Duration::days(10));
    let later = app::App {
        clock: std::sync::Arc::new(clock),
        ..app.clone()
    };
    assert_eq!(popularity(app, a.id).await, Some(1200.0), "stored");
    later.apply_catalog_refresh(a.id, refresh()).await.unwrap();
    assert_eq!(popularity(app, a.id).await, Some(1500.0), "refreshed");
    assert_eq!(
        app.stale_provider_data(&ns, t0 + Duration::days(11), 10)
            .await
            .unwrap(),
        vec![(a.id, t0 + Duration::days(10))]
    );

    // episodes, title and year go with the purge; the ids stay (UX4-9)
    let key = app.seasons_to_fetch(a.id).await.unwrap().remove(0);
    app.store_episodes(
        a.id,
        &[SeasonFetch {
            key,
            connector: "tmdb".into(),
            episodes: vec![CatalogEpisode {
                season: 1,
                number: 1,
                air_date: None,
                runtime_minutes: Some(50),
            }],
        }],
    )
    .await
    .unwrap();
    assert_eq!(
        app.purge_provider_data(a.id).await.unwrap().as_deref(),
        Some("asset-1")
    );
    let purged = app.catalog_item(a.id).await.unwrap();
    assert!(purged.title_unavailable());
    assert_eq!(purged.year, None);
    assert_eq!(purged.popularity, None, "it is the source's data too");
    assert_eq!(
        purged.external_ids,
        vec![ExternalId::new("tmdb", "Harbour")]
    );
    assert_eq!(purged.description, None);
    assert_eq!(purged.original_title, None);
    assert_eq!(purged.cover_url, None);
    assert_eq!(purged.cover_asset, None);
    assert_eq!(purged.attributes, serde_json::json!({}));
    assert_eq!(purged.state, EntityState::Stub);
    assert_eq!(purged.fetched_at, None);
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM catalog_episodes").await,
        0
    );
    assert_eq!(
        app.stale_provider_data(&ns, t0 + Duration::days(400), 10)
            .await
            .unwrap(),
        vec![]
    );
    // the other source's item is never listed for the tmdb namespace
    assert_eq!(
        app.stale_provider_data(&["test".into()], t0 + Duration::days(400), 10)
            .await
            .unwrap(),
        vec![(other.id, t0)]
    );
}

#[tokio::test]
async fn unreferenced_items_are_deleted_and_purged_ones_are_refetched_on_postgres() {
    // UX4-9: the delete checks every reference in its own statement; a purged item waits for
    // its refetch time, and a refresh that brings the title back ends the wait
    let Some(env) = common::env().await else {
        return;
    };
    let app = &env.app;
    let ns = vec!["tmdb".to_string()];
    let t0 = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let (_, me) = common::user(app, "alice").await;
    let lone = app.upsert_catalog_item(show("Lone", "tmdb")).await.unwrap();
    let owned = app
        .upsert_catalog_item(show("Owned", "tmdb"))
        .await
        .unwrap();
    let played = app
        .upsert_catalog_item(show("Played", "tmdb"))
        .await
        .unwrap();
    app.add_to_library(me, owned.id, None).await.unwrap();
    let ep = domain::progress::Progress::Episode {
        season: 1,
        episode: 1,
    };
    app.log_progress(me, played.id, ep, None, None)
        .await
        .unwrap();
    app.set_cover_asset(lone.id, "asset-9".into())
        .await
        .unwrap();
    for id in [owned.id, played.id] {
        assert_eq!(
            app.delete_unreferenced_item(id).await.unwrap(),
            app::ports::Removal::Kept
        );
    }
    assert_eq!(
        app.delete_unreferenced_item(lone.id).await.unwrap(),
        app::ports::Removal::Deleted {
            cover_asset: Some("asset-9".into())
        }
    );
    assert!(app.catalog_item(lone.id).await.is_err());
    assert_eq!(
        common::count(
            &env.store,
            &format!(
                "SELECT count(*) FROM outbox WHERE entity_id = '{}' AND op = 'delete'",
                lone.id.0
            )
        )
        .await,
        1
    );

    app.purge_provider_data(owned.id).await.unwrap();
    assert_eq!(
        app.expired_provider_data(&ns, t0, 10).await.unwrap(),
        vec![owned.id]
    );
    app.touch_expired(owned.id, t0 + Duration::days(1))
        .await
        .unwrap();
    assert_eq!(
        app.expired_provider_data(&ns, t0, 10).await.unwrap(),
        vec![]
    );
    assert_eq!(
        app.expired_provider_data(&ns, t0 + Duration::days(1), 10)
            .await
            .unwrap(),
        vec![owned.id]
    );
    // the refetch: title back, stamped again, no longer waiting
    app.apply_catalog_refresh(owned.id, refresh())
        .await
        .unwrap();
    assert_eq!(
        app.expired_provider_data(&ns, t0 + Duration::days(9), 10)
            .await
            .unwrap(),
        vec![]
    );
    assert_eq!(app.catalog_item(owned.id).await.unwrap().title, "Harbour");
    assert_eq!(
        app.stale_provider_data(&ns, t0 + Duration::seconds(1), 10)
            .await
            .unwrap()
            .iter()
            .filter(|(id, _)| *id == owned.id)
            .count(),
        1
    );
}
