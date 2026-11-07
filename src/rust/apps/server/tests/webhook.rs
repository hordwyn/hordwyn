//! M14 acceptance (end to end over HTTP): posting a watched event for a Jellyfin play creates a
//! progress event and an SSE update for the user.

// One end-to-end scenario on purpose: the steps share a running server and an SSE stream.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn a_jellyfin_play_becomes_a_progress_event_and_an_sse_update() {
    let Some(db_url) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let toml = format!(
        "[database]\nurl = \"{db_url}\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n\
         [secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[server]\npublic_url = \"https://shelf.example.org\"\n\
         [brand]\ndir = \"{}/../../../../config/brand/default\"\n",
        env!("CARGO_MANIFEST_DIR"),
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(
        cfg,
        config::ServerRole::All,
        Overrides {
            registry: Some(catalog_core::Registry::default()),
            http: Some(Arc::new(testkit::fakes::ReplayHttp::default())),
            search: None,
        },
    )
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let stop = CancellationToken::new();
    let web = {
        let router = srv.base_router();
        let stop = stop.clone();
        let bus = srv.bus.clone();
        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    stop.cancelled().await;
                    bus.shutdown();
                })
                .await
                .unwrap();
        })
    };
    let http = reqwest::Client::new();
    let signup: serde_json::Value = http
        .post(format!("{base}/api/v1/auth/sign-up"))
        .json(&serde_json::json!({
            "username": "anna", "display_name": "Anna", "password": "correct horse battery"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let session = signup["token"].as_str().unwrap().to_string();

    let created: serde_json::Value = http
        .post(format!("{base}/api/v1/integrations"))
        .bearer_auth(&session)
        .json(&serde_json::json!({"name": "Jellyfin"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        created["webhook_url"],
        "https://shelf.example.org/api/v1/hooks/watched"
    );
    let hook_token = created["token"].as_str().unwrap().to_string();

    let mut sse = http
        .get(format!("{base}/api/v1/events"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .bytes_stream();

    // what a Jellyfin webhook template (written by the user, DEC-149) posts after a movie
    let play = serde_json::json!({
        "kind": "movie", "title": "Heat", "year": 1995,
        "ids": {"tmdb": "949", "imdb": "tt0113277"},
        "watched_at": "2026-01-01T21:00:00Z", "source": "Jellyfin",
    });
    let refused = http
        .post(format!("{base}/api/v1/hooks/watched"))
        .bearer_auth(&session)
        .json(&play)
        .send()
        .await
        .unwrap();
    assert_eq!(
        refused.status(),
        401,
        "a session token is not an integration token"
    );
    let r = http
        .post(format!("{base}/api/v1/hooks/watched"))
        .bearer_auth(&hook_token)
        .json(&play)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200);
    let recorded: serde_json::Value = r.json().await.unwrap();
    let item_id = recorded["item_id"].as_str().unwrap().to_string();

    let progress: serde_json::Value = http
        .get(format!("{base}/api/v1/items/{item_id}/progress"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(progress.as_array().unwrap().len(), 1, "{progress}");
    assert_eq!(progress[0]["progress"]["type"], "finished");

    // the user's open pages hear about it
    let started = Instant::now();
    let mut buf = String::new();
    let mut seen = (false, false);
    while !(seen.0 && seen.1) && started.elapsed() < Duration::from_secs(10) {
        let Ok(Some(Ok(chunk))) = tokio::time::timeout(Duration::from_secs(10), sse.next()).await
        else {
            break;
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));
        seen.0 |= buf.contains("entity.updated") && buf.contains(&item_id);
        seen.1 |= buf.contains("notification.new");
    }
    assert!(seen.0, "entity.updated for the item over SSE: {buf}");
    assert!(seen.1, "notification.new over SSE: {buf}");

    // a retry of the same play changes nothing
    let again: serde_json::Value = http
        .post(format!("{base}/api/v1/hooks/watched"))
        .bearer_auth(&hook_token)
        .json(&play)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(again, recorded);
    let list: serde_json::Value = http
        .get(format!("{base}/api/v1/integrations"))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list[0]["last_seen_at"].is_string());
    assert_eq!(list[0]["last_error"], serde_json::Value::Null);
    stop.cancel();
    let _ = web.await;
}
