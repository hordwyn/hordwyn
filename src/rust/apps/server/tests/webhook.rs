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

// ---------------------------------------------------------------- WP3-5 (design.md 5.9, 6.9)

/// A running server for the WP3-5 scenarios.
struct Running {
    base: String,
    http: reqwest::Client,
    stop: CancellationToken,
    web: tokio::task::JoinHandle<()>,
}

async fn running() -> Option<Running> {
    let db_url = store_pg::testing::fresh_db_url().await?;
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
    Some(Running {
        base,
        http: reqwest::Client::new(),
        stop,
        web,
    })
}

impl Running {
    async fn session(&self, name: &str) -> String {
        let v: serde_json::Value = self
            .http
            .post(format!("{}/api/v1/auth/sign-up", self.base))
            .json(&serde_json::json!({
                "username": name, "display_name": name, "password": "correct horse battery"
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        v["token"].as_str().unwrap().to_string()
    }

    async fn hook(&self, token: &str, body: &serde_json::Value) -> (u16, serde_json::Value) {
        let r = self
            .http
            .post(format!("{}/api/v1/hooks/watched", self.base))
            .bearer_auth(token)
            .json(body)
            .send()
            .await
            .unwrap();
        let status = r.status().as_u16();
        (status, r.json().await.unwrap_or_default())
    }

    async fn rest(
        &self,
        method: reqwest::Method,
        path: &str,
        session: &str,
        body: Option<serde_json::Value>,
    ) -> (u16, serde_json::Value) {
        let mut req = self
            .http
            .request(method, format!("{}/api/v1{path}", self.base))
            .bearer_auth(session);
        if let Some(b) = body {
            req = req.json(&b);
        }
        let r = req.send().await.unwrap();
        let status = r.status().as_u16();
        (status, r.json().await.unwrap_or_default())
    }

    async fn integration(&self, session: &str) -> serde_json::Value {
        let (_, list) = self
            .rest(reqwest::Method::GET, "/integrations", session, None)
            .await;
        list[0].clone()
    }

    async fn finish(self) {
        self.stop.cancel();
        let _ = self.web.await;
    }
}

/// Read the SSE stream until `want` matches one `integration.call` event (its JSON payload).
async fn next_call<B: AsRef<[u8]>>(
    sse: &mut (impl futures::Stream<Item = reqwest::Result<B>> + Unpin),
    buf: &mut String,
    want: impl Fn(&serde_json::Value) -> bool,
) -> Option<serde_json::Value> {
    let started = Instant::now();
    loop {
        // complete events end with a blank line
        while let Some(end) = buf.find("\n\n") {
            let block: String = buf.drain(..end + 2).collect();
            if !block.contains("event: integration.call") {
                continue;
            }
            let data: String = block
                .lines()
                .filter_map(|l| l.strip_prefix("data:"))
                .map(str::trim)
                .collect();
            let v: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
            if want(&v["payload"]) {
                return Some(v["payload"].clone());
            }
        }
        if started.elapsed() > Duration::from_secs(10) {
            return None;
        }
        let Ok(Some(Ok(chunk))) = tokio::time::timeout(Duration::from_secs(10), sse.next()).await
        else {
            return None;
        };
        buf.push_str(&String::from_utf8_lossy(chunk.as_ref()));
    }
}

/// R-38 / design.md 6.9: a refused call (422) is attributed to its integration with the
/// offending field and the body it sent, secrets redacted and at most 4 KiB; the user's open
/// pages hear about every attributed call; a 401 is attributed to nothing.
#[tokio::test]
async fn refused_calls_keep_the_field_and_a_redacted_payload() {
    let Some(t) = running().await else {
        return;
    };
    let session = t.session("anna").await;
    let (_, created) = t
        .rest(
            reqwest::Method::POST,
            "/integrations",
            &session,
            Some(serde_json::json!({"name": "Home script"})),
        )
        .await;
    let token = created["token"].as_str().unwrap().to_string();
    let id = created["integration"]["id"].as_str().unwrap().to_string();
    let mut sse = t
        .http
        .get(format!("{}/api/v1/events", t.base))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .bytes_stream();
    let mut buf = String::new();

    // a typo in a field name, with a token and a secret in the body
    let body = serde_json::json!({
        "kind": "episode", "title": "Pilot",
        "series": {"title": "Northern Lights Station", "ids": {"tmdb": "1"}},
        "season": 2, "episode": 4, "episode_title": "Pilot",
        // (sorted after `episode_title`: `json!` sends its keys in order)
        "x_api_token": "tok-should-not-be-kept", "x_auth": {"client_secret": "sec-should-not-be-kept"},
    });
    let (status, err) = t.hook(&token, &body).await;
    assert_eq!(status, 422, "{err}");
    let i = t.integration(&session).await;
    assert_eq!(i["last_status"], 422, "{i}");
    assert_eq!(i["last_error_field"], "episode_title", "{i}");
    let payload = i["last_payload"].as_str().unwrap();
    assert!(payload.contains("episode_title"), "{payload}");
    assert!(!payload.contains("tok-should-not-be-kept"), "{payload}");
    assert!(!payload.contains("sec-should-not-be-kept"), "{payload}");
    let call = next_call(&mut sse, &mut buf, |p| p["status"] == 422).await;
    let call = call.expect("integration.call for the refused call");
    assert_eq!(call["integration_id"], id.as_str());
    assert_eq!(call["field"], "episode_title");
    assert_eq!(call["test"], false);

    // a rule of the format: an episode needs its numbers
    let (status, _) = t
        .hook(
            &token,
            &serde_json::json!({"kind": "episode", "title": "Pilot",
                "series": {"title": "Northern Lights Station", "ids": {"tmdb": "1"}}}),
        )
        .await;
    assert_eq!(status, 422);
    assert_eq!(t.integration(&session).await["last_error_field"], "season");

    // a huge body is kept to 4 KiB
    let (status, _) = t
        .hook(
            &token,
            &serde_json::json!({"kind": "movie", "title": "x".repeat(10_000), "nope": 1}),
        )
        .await;
    assert_eq!(status, 422);
    let i = t.integration(&session).await;
    assert!(i["last_payload"].as_str().unwrap().len() <= 4096, "{i}");

    // an unknown token is refused and attributed to nothing
    let before = t.integration(&session).await;
    let (status, _) = t.hook("itk_unknown", &body).await;
    assert_eq!(status, 401);
    assert_eq!(t.integration(&session).await, before);

    // an accepted call: health, the last recorded item and the event for the open panel
    let play = serde_json::json!({
        "kind": "movie", "title": "Heat", "year": 1995, "ids": {"tmdb": "949"},
        "watched_at": "2026-01-01T21:00:00Z",
    });
    let (status, recorded) = t.hook(&token, &play).await;
    assert_eq!(status, 200, "{recorded}");
    let i = t.integration(&session).await;
    assert_eq!(i["last_status"], 200, "{i}");
    assert_eq!(i["last_error_field"], serde_json::Value::Null);
    assert_eq!(i["last_item"]["title"], "Heat", "{i}");
    assert_eq!(i["last_item"]["id"], recorded["item_id"]);
    let call = next_call(&mut sse, &mut buf, |p| p["status"] == 200).await;
    let call = call.expect("integration.call for the accepted call");
    assert_eq!(call["item_id"], recorded["item_id"]);
    assert_eq!(call["title"], "Heat");
    t.finish().await;
}

/// design.md 5.9: "Send a test call from here" validates and matches the sample but records
/// nothing (no progress, the card keeps its last call) and is marked as a test.
#[tokio::test]
async fn a_test_call_validates_and_matches_but_records_nothing() {
    let Some(t) = running().await else {
        return;
    };
    let session = t.session("anna").await;
    let (_, created) = t
        .rest(
            reqwest::Method::POST,
            "/integrations",
            &session,
            Some(serde_json::json!({"name": "Jellyfin"})),
        )
        .await;
    let token = created["token"].as_str().unwrap().to_string();
    let id = created["integration"]["id"].as_str().unwrap().to_string();
    let play = serde_json::json!({
        "kind": "movie", "title": "Heat", "year": 1995, "ids": {"tmdb": "949"},
        "watched_at": "2026-01-01T21:00:00Z",
    });
    let (_, recorded) = t.hook(&token, &play).await;
    let item_id = recorded["item_id"].as_str().unwrap().to_string();
    let before = t.integration(&session).await;
    let mut sse = t
        .http
        .get(format!("{}/api/v1/events", t.base))
        .bearer_auth(&session)
        .send()
        .await
        .unwrap()
        .bytes_stream();
    let mut buf = String::new();

    let sample = serde_json::json!({
        "kind": "movie", "title": "Heat", "year": 1995, "ids": {"tmdb": "949"},
        "watched_at": "2026-02-01T21:00:00Z",
    });
    let (status, answer) = t
        .rest(
            reqwest::Method::POST,
            &format!("/integrations/{id}/test"),
            &session,
            Some(sample),
        )
        .await;
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["test"], true);
    assert_eq!(answer["matched"]["id"], item_id.as_str(), "{answer}");
    let (_, progress) = t
        .rest(
            reqwest::Method::GET,
            &format!("/items/{item_id}/progress"),
            &session,
            None,
        )
        .await;
    assert_eq!(
        progress.as_array().unwrap().len(),
        1,
        "no new play: {progress}"
    );
    assert_eq!(
        t.integration(&session).await,
        before,
        "the card is unchanged"
    );
    let call = next_call(&mut sse, &mut buf, |p| p["test"] == true).await;
    let call = call.expect("integration.call marked as a test");
    assert_eq!(call["status"], 200);
    assert_eq!(call["integration_id"], id.as_str());

    // a sample without the item's numbers is refused like a real call, with the field
    let (status, err) = t
        .rest(
            reqwest::Method::POST,
            &format!("/integrations/{id}/test"),
            &session,
            Some(serde_json::json!({"kind": "episode", "title": "x",
                "series": {"title": "S", "ids": {"tmdb": "1"}}})),
        )
        .await;
    assert_eq!(status, 422, "{err}");
    assert_eq!(err["field"], "season", "{err}");
    assert_eq!(t.integration(&session).await, before);

    // someone else's sender cannot be tested
    let ben = t.session("ben").await;
    let (status, _) = t
        .rest(
            reqwest::Method::POST,
            &format!("/integrations/{id}/test"),
            &ben,
            Some(play),
        )
        .await;
    assert_eq!(status, 404);
    t.finish().await;
}
