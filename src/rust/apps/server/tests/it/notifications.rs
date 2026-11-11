//! WP3-5 (design.md 6.8, 5.8): channel health from the delivery log, "Mark all read", and the
//! inbox grouped by the day in the user's time zone. Over HTTP against a running server with
//! its job worker.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

struct T {
    base: String,
    http: reqwest::Client,
    srv: Server,
}

async fn start() -> Option<T> {
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
    let router = srv.base_router();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Some(T {
        base,
        http: reqwest::Client::new(),
        srv,
    })
}

impl T {
    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        session: &str,
        body: Option<Value>,
    ) -> (u16, Value) {
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

    async fn sign_up(&self, name: &str) -> String {
        let v: Value = self
            .http
            .post(format!("{}/api/v1/auth/sign-up", self.base))
            .json(&json!({"username": name, "display_name": name,
                "password": "correct horse battery"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        v["token"].as_str().unwrap().to_string()
    }
}

/// A channel whose last delivery failed says so (with the channel's error), one that never
/// delivered says "never", and a channel's health ignores the other channels' deliveries.
#[tokio::test]
async fn channel_health_comes_from_the_delivery_log() {
    let Some(t) = start().await else {
        return;
    };
    // the first account is the admin: it hears about sign-ups
    let anna = t.sign_up("anna").await;
    let (status, hook) = t
        .call(
            reqwest::Method::POST,
            "/notifications/channels",
            &anna,
            Some(json!({"name": "Automation",
                "config": {"kind": "webhook", "url": "http://127.0.0.1:9/in", "secret": ""}})),
        )
        .await;
    assert_eq!(status, 201, "{hook}");
    let (status, ntfy) = t
        .call(
            reqwest::Method::POST,
            "/notifications/channels",
            &anna,
            Some(json!({"name": "Phone",
                "config": {"kind": "ntfy", "server": "https://ntfy.example.org", "topic": "t"}})),
        )
        .await;
    assert_eq!(status, 201, "{ntfy}");
    let hook_id = hook["id"].as_str().unwrap().to_string();
    let (status, sub) = t
        .call(
            reqwest::Method::PUT,
            "/notifications/subscriptions",
            &anna,
            Some(json!({"event": "admin.signup", "targets": {"type": "only", "ids": [hook_id]}})),
        )
        .await;
    assert_eq!(status, 200, "{sub}");

    // before anything was sent: both channels have never delivered
    let (_, channels) = t
        .call(reqwest::Method::GET, "/notifications/channels", &anna, None)
        .await;
    for c in channels.as_array().unwrap() {
        assert_eq!(c["health"]["state"], "never", "{c}");
    }

    t.sign_up("ben").await;
    let stop = CancellationToken::new();
    let worker = tokio::spawn(t.srv.worker().run(stop.clone()));
    let mut channels = Value::Null;
    for _ in 0..100 {
        (_, channels) = t
            .call(reqwest::Method::GET, "/notifications/channels", &anna, None)
            .await;
        if channels[0]["health"]["state"] == "failing" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stop.cancel();
    let _ = worker.await;
    let by_name = |n: &str| {
        channels
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == n)
            .cloned()
            .unwrap()
    };
    let automation = by_name("Automation");
    assert_eq!(automation["health"]["state"], "failing", "{automation}");
    assert!(
        automation["health"]["error"]
            .as_str()
            .is_some_and(|e| !e.is_empty()),
        "{automation}"
    );
    assert!(automation["health"]["at"].is_string(), "{automation}");
    assert_eq!(by_name("Phone")["health"]["state"], "never");
}

/// "Mark all read" marks every unread inbox row of the caller (and nobody else's); the inbox
/// rows carry their day and time in the user's time zone (DEC-306).
#[tokio::test]
async fn mark_all_read_and_local_days() {
    let Some(t) = start().await else {
        return;
    };
    let anna = t.sign_up("anna").await;
    // three sign-ups (her own included) -> three unread rows for the admin
    let ben = t.sign_up("ben").await;
    t.sign_up("carl").await;
    let (status, _) = t
        .call(
            reqwest::Method::PUT,
            "/auth/me/time-zone",
            &anna,
            Some(json!({"time_zone": "Pacific/Kiritimati"})),
        )
        .await;
    assert_eq!(status, 200);
    let (_, inbox) = t
        .call(reqwest::Method::GET, "/notifications/inbox", &anna, None)
        .await;
    let rows = inbox.as_array().unwrap();
    assert_eq!(rows.len(), 3, "{inbox}");
    assert!(rows.iter().all(|r| r["read"] == false));
    // UTC+14: the local day and time follow the zone, not UTC
    let at = chrono::DateTime::parse_from_rfc3339(rows[0]["created_at"].as_str().unwrap())
        .unwrap()
        .with_timezone(&chrono::Utc);
    let local = at + chrono::Duration::hours(14);
    assert_eq!(rows[0]["local_day"], local.format("%Y-%m-%d").to_string());
    assert_eq!(rows[0]["local_time"], local.format("%H:%M").to_string());

    let (status, _) = t
        .call(
            reqwest::Method::POST,
            "/notifications/inbox/read-all",
            &ben,
            None,
        )
        .await;
    assert_eq!(status, 204, "an empty inbox is fine");
    let (status, _) = t
        .call(
            reqwest::Method::POST,
            "/notifications/inbox/read-all",
            &anna,
            None,
        )
        .await;
    assert_eq!(status, 204);
    let (_, inbox) = t
        .call(reqwest::Method::GET, "/notifications/inbox", &anna, None)
        .await;
    assert!(
        inbox.as_array().unwrap().iter().all(|r| r["read"] == true),
        "{inbox}"
    );
    let status = t
        .http
        .post(format!("{}/api/v1/notifications/inbox/read-all", t.base))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status.as_u16(), 401, "signed out");
}
