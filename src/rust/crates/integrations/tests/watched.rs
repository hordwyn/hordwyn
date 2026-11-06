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
        Err(IntegrationError::Invalid(_))
    ));
    let no_episode = event(serde_json::json!({
        "kind": "episode", "title": "x", "series": {"title": "S", "ids": {"tmdb": "1"}}
    }));
    assert!(matches!(
        i.receive(&token, &no_episode).await,
        Err(IntegrationError::Invalid(_))
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
