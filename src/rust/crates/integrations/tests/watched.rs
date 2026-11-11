//! M14: the watched webhook records plays once, for the token's user, and says what is wrong.

use std::sync::Arc;

use app::ports::Audience;
use chrono::{DateTime, Utc};
use domain::progress::Progress;
use domain::user::{Actor, Role};
use integrations::{IntegrationError, Integrations, MemIntegrationStore, WatchedEvent};
use testkit::TestEnv;

fn event(json: serde_json::Value) -> WatchedEvent {
    serde_json::from_value(json).unwrap()
}

async fn env() -> (TestEnv, Integrations, Actor, String) {
    let t = TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let u = t
        .app
        .register_user("anna", "Anna", None, Role::Member)
        .await
        .unwrap();
    let actor = Actor {
        user_id: u.id,
        role: u.role,
    };
    let i = Integrations::new(t.app.clone(), Arc::new(MemIntegrationStore::default()));
    let (_, token) = i.create(actor, "Jellyfin").await.unwrap();
    (t, i, actor, token)
}

#[tokio::test]
async fn a_movie_play_is_recorded_once_and_announced() {
    let (t, i, actor, token) = env().await;
    assert!(token.starts_with("itk_"));
    let play = event(serde_json::json!({
        "kind": "movie", "title": "Heat", "year": 1995,
        "ids": {"tmdb": "949", "IMDB": "tt0113277"},
        "watched_at": "2026-01-01T21:00:00Z", "source": "Jellyfin",
    }));
    let r = i.receive(&token, &play).await.unwrap();
    let again = i.receive(&token, &play).await.unwrap();
    assert_eq!(again, r, "the same play twice is one event");
    let events = t.app.progress(actor, r.item_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].progress, Progress::Finished);
    let at: DateTime<Utc> = "2026-01-01T21:00:00Z".parse().unwrap();
    assert_eq!(events[0].occurred_at, at);
    let entry = t.app.entry(actor, r.item_id).await.unwrap().unwrap();
    assert_eq!(entry.status.value.as_str(), "completed");
    // a stub item is created and refreshed in the background
    assert!(t.jobs.jobs().iter().any(|j| j.kind == app::REFRESH_JOB));
    let item = t.app.catalog_item(r.item_id).await.unwrap();
    assert!(item.external_ids.iter().any(|x| x.namespace == "imdb"));
    let n = t
        .notifications
        .all()
        .into_iter()
        .find(|n| n.event == "integration.play_recorded")
        .unwrap();
    assert_eq!(n.audience, Audience::Users(vec![actor.user_id]));
    assert_eq!(n.args["title"], "Heat");
    assert_eq!(n.args["source"], "Jellyfin");
}

#[tokio::test]
async fn episodes_go_to_the_series() {
    let (t, i, actor, token) = env().await;
    let ep = |n: u32| {
        event(serde_json::json!({
            "kind": "episode", "title": "Good News About Hell",
            "series": {"title": "Severance", "year": 2022, "ids": {"tmdb": "95396"}},
            "season": 1, "episode": n, "watched_at": "2026-01-02T20:00:00Z",
        }))
    };
    let a = i.receive(&token, &ep(1)).await.unwrap();
    let b = i.receive(&token, &ep(2)).await.unwrap();
    assert_eq!(a.item_id, b.item_id, "one series item");
    let events = t.app.progress(actor, a.item_id).await.unwrap();
    assert_eq!(events.len(), 2);
    assert!(events.iter().any(|e| e.progress
        == Progress::Episode {
            season: 1,
            episode: 2
        }));
    assert_eq!(
        t.app.catalog_item(a.item_id).await.unwrap().kind.as_str(),
        "show"
    );
}

#[tokio::test]
async fn bad_tokens_and_events_are_refused() {
    let (_, i, actor, token) = env().await;
    let movie =
        event(serde_json::json!({"kind": "movie", "title": "Heat", "ids": {"tmdb": "949"}}));
    assert!(matches!(
        i.receive("itk_wrong", &movie).await,
        Err(IntegrationError::Unauthorized)
    ));
    let no_ids = event(serde_json::json!({"kind": "movie", "title": "Heat"}));
    assert!(matches!(
        i.receive(&token, &no_ids).await,
        Err(IntegrationError::Refused { field: Some(f), .. }) if f == "ids"
    ));
    let no_episode = event(serde_json::json!({
        "kind": "episode", "title": "x", "series": {"title": "S", "ids": {"tmdb": "1"}}
    }));
    assert!(matches!(
        i.receive(&token, &no_episode).await,
        Err(IntegrationError::Refused { field: Some(f), .. }) if f == "season"
    ));
    // the last error is kept for the settings page
    let listed = i.list(actor).await.unwrap();
    assert!(listed[0].last_error.as_deref().unwrap().contains("season"));
    assert!(listed[0].last_seen_at.is_some());
    // unknown fields are refused (typos in templates show up)
    assert!(
        serde_json::from_value::<WatchedEvent>(serde_json::json!({
            "kind": "movie", "title": "Heat", "tmdb": "949"
        }))
        .is_err()
    );
    // revoking stops the token
    i.delete(actor, listed[0].id).await.unwrap();
    assert!(matches!(
        i.receive(&token, &movie).await,
        Err(IntegrationError::Unauthorized)
    ));
}

#[tokio::test]
async fn retries_without_a_time_are_one_play() {
    let (t, i, actor, token) = env().await;
    let movie =
        event(serde_json::json!({"kind": "movie", "title": "Heat", "ids": {"tmdb": "949"}}));
    let first = i.receive(&token, &movie).await.unwrap();
    t.clock.advance(chrono::Duration::seconds(30));
    let retry = i.receive(&token, &movie).await.unwrap();
    assert_eq!(first, retry, "a retry 30 s later is the same play");
    // a sender id decides on its own
    let with_id = |id: &str| {
        event(
            serde_json::json!({"kind": "movie", "title": "Heat", "ids": {"tmdb": "949"}, "id": id}),
        )
    };
    let p1 = i.receive(&token, &with_id("p1")).await.unwrap();
    t.clock.advance(chrono::Duration::hours(2));
    assert_eq!(i.receive(&token, &with_id("p1")).await.unwrap(), p1);
    assert_ne!(i.receive(&token, &with_id("p2")).await.unwrap(), p1);
    assert_eq!(t.app.progress(actor, first.item_id).await.unwrap().len(), 3);
}

/// Review ot2e, OT2E-1 (DEC-591 (6)): while a rewatch run is running a person cannot date a
/// view before its start, but a media server dates its plays itself. A late play is
/// recorded (not counted in the run) instead of answered with an error; one dated now is
/// the run's.
#[tokio::test]
async fn a_late_play_during_a_rewatch_run_is_recorded() {
    let (t, i, actor, token) = env().await;
    let ep = |n: u32, at: DateTime<Utc>| {
        event(serde_json::json!({
            "kind": "episode", "title": "Good News About Hell",
            "series": {"title": "Severance", "year": 2022, "ids": {"tmdb": "95396"}},
            "season": 1, "episode": n, "watched_at": at.to_rfc3339(),
        }))
    };
    let now = t.clock.now_for_tests();
    let first = i
        .receive(&token, &ep(1, now - chrono::Duration::days(30)))
        .await
        .unwrap();
    let show = first.item_id;
    let mut item = t.app.catalog_item(show).await.unwrap();
    item.attributes = serde_json::json!({"seasons": [{"number": 1, "episode_count": 2}]});
    t.store.state().catalog.insert(item.id, item);
    t.app
        .set_status(actor, show, domain::kind::Status::completed(), None)
        .await
        .unwrap();
    t.app
        .log_progress(actor, show, Progress::Rewatch, None, None)
        .await
        .unwrap();
    // watched last night, delivered now
    i.receive(&token, &ep(1, now - chrono::Duration::hours(12)))
        .await
        .unwrap();
    let row = t.app.library_row(actor, show).await.unwrap();
    assert_eq!(row.rewatch.as_ref().map(|r| r.done), Some(0));
    assert_eq!(row.summary.episode_views, 2);
    t.clock.advance(chrono::Duration::minutes(5));
    i.receive(&token, &ep(2, t.clock.now_for_tests()))
        .await
        .unwrap();
    let row = t.app.library_row(actor, show).await.unwrap();
    assert_eq!(row.rewatch.as_ref().map(|r| r.done), Some(1));
}
