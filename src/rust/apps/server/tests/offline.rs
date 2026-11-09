//! WP4-5 offline queue against the server (DEC-237, design.md 5.4): the commands the browser
//! queues while offline (built by `web::offline`, stamped by its client clock) replay
//! through `POST /sync/commands` once: a second push of the same queue changes nothing.

mod support;

use serde_json::{Value, json};
use server::{Overrides, Server};
use web::offline::{ClientClock, Queued, log_command, status_command};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn start() -> Option<(axum::Router, Server)> {
    let db = store_pg::testing::fresh_db_url().await?;
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [brand]\ndir = \"{}\"\n[auth]\nsecure_cookies = false\n",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, vec![]).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    Some((srv.base_router(), srv))
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let req = axum::http::Request::builder()
        .method(method)
        .uri(format!("/api/v1{uri}"))
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(axum::body::Body::from(
            body.map_or_else(String::new, |b| b.to_string()),
        ))
        .unwrap();
    let resp = support::send(app, req).await;
    let status = resp.status().as_u16();
    (
        status,
        serde_json::from_slice(&resp.into_body()).unwrap_or(Value::Null),
    )
}

#[test]
fn the_client_clock_stamps_what_the_server_parses_in_order() {
    let mut c = ClientClock::new(7);
    let first = c.stamp(1_000);
    let second = c.stamp(1_000);
    // the wall clock went back (another tab, a corrected clock): still later
    let third = c.stamp(900);
    let last = c.stamp(2_000);
    let parsed: Vec<domain::hlc::Hlc> = [&first, &second, &third, &last]
        .iter()
        .map(|s| s.parse().unwrap())
        .collect();
    assert!(parsed.windows(2).all(|w| w[0] < w[1]), "{parsed:?}");
    assert_eq!(parsed[0], domain::hlc::Hlc::new(1_000, 0, 7));
    assert_eq!(parsed[3], domain::hlc::Hlc::new(2_000, 0, 7));
    // the clock survives a reload through its stored last stamp
    let mut again = ClientClock::resume(7, Some(&last));
    assert!(again.stamp(1_500).parse::<domain::hlc::Hlc>().unwrap() > parsed[3]);
}

/// A server, a signed-in person and a show in their library: (router, server, cookie, item id).
async fn signed_in_with_a_show() -> (axum::Router, Server, String, String) {
    let (app, srv) = start().await.unwrap();
    let resp = support::send(
        &app,
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/auth/sign-up")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(
                json!({"username": "olga", "display_name": "Olga", "password": "correct horse battery"})
                    .to_string(),
            ))
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status(), 201);
    let cookie = resp.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();
    let item = srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("show").unwrap(),
            title: "Downstream".into(),
            original_title: None,
            year: Some(2020),
            description: None,
            external_ids: Vec::new(),
            attributes: json!({}),
            cover_url: None,
        })
        .await
        .unwrap()
        .id
        .0
        .to_string();
    let (s, _) = call(
        &app,
        "POST",
        "/library",
        &cookie,
        Some(json!({"item_id": item})),
    )
    .await;
    assert!(s == 200 || s == 201, "{s}");
    (app, srv, cookie, item)
}

#[tokio::test]
async fn a_queue_replayed_twice_changes_nothing() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let (app, _srv, cookie, item) = signed_in_with_a_show().await;

    // offline: an episode mark, then a status change, queued with op ids
    let mut clock = ClientClock::new(42);
    let now = chrono::Utc::now();
    let ms = u64::try_from(now.timestamp_millis()).unwrap();
    let queue = [
        Queued::new(
            log_command(
                &item,
                json!({"type": "episode", "season": 1, "episode": 2}),
                Some(now.to_rfc3339()),
                Some(&clock.stamp(ms)),
            ),
            "Downstream",
            ms,
        ),
        Queued::new(
            status_command(&item, "paused", &clock.stamp(ms)),
            "Downstream",
            ms,
        ),
    ];
    // the flush: one push in the order queued, with the stamps the commands got in the queue
    // (the log's stamp is for the status it implies, so the later status wins over it)
    let batch = web::offline::flush_batch(&queue);
    assert_eq!(batch.commands.len(), 2);
    assert_eq!(
        batch.commands[1].op_id, queue[1].envelope.op_id,
        "the op id is kept"
    );
    let push = |b: &contracts::SyncBatchDto| {
        let (app, cookie, b) = (app.clone(), cookie.clone(), json!(b));
        async move {
            let (s, r) = call(&app, "POST", "/sync/commands", &cookie, Some(b)).await;
            assert_eq!(s, 200);
            assert!(r.as_array().unwrap().iter().all(|r| r["ok"] == true), "{r}");
        }
    };
    push(&batch).await;
    let progress = |v: &Value| v.as_array().unwrap().len();
    let (_, events) = call(
        &app,
        "GET",
        &format!("/items/{item}/progress"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(progress(&events), 1, "{events}");
    let (_, changes) = call(&app, "GET", "/sync/changes?cursor=0", &cookie, None).await;

    // the flush answers were lost (the tab closed): the same batches go again
    push(&batch).await;
    let (_, events) = call(
        &app,
        "GET",
        &format!("/items/{item}/progress"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(progress(&events), 1, "the episode is logged once");
    let (_, after) = call(&app, "GET", "/sync/changes?cursor=0", &cookie, None).await;
    assert_eq!(after, changes, "no new change rows");
    let entry = after["changes"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find_map(|c| c.get("entry").filter(|e| !e.is_null()))
        .unwrap()
        .clone();
    assert_eq!(entry["status"], "paused");

    // a stamp older than the stored one loses (last writer wins on the client's clock)
    let old = Queued::new(
        status_command(&item, "dropped", &ClientClock::new(1).stamp(1)),
        "Downstream",
        ms,
    );
    let (_, r) = call(
        &app,
        "POST",
        "/sync/commands",
        &cookie,
        Some(json!(web::offline::flush_batch(&[old]))),
    )
    .await;
    assert_eq!(r[0]["ok"], true, "{r}");
    let (_, lib) = call(&app, "GET", "/library?limit=5", &cookie, None).await;
    assert_eq!(lib["items"][0]["entry"]["status"], "paused", "{lib}");
}

/// DEC-434 revisited (WP4-8): an offline change counts as made when it was queued, so a change
/// another device made later is not overwritten when the queue syncs; the queued log still
/// cannot override the status queued after it, because the log carries its queue-time stamp
/// for the status it implies.
#[tokio::test]
async fn a_queued_change_loses_to_a_later_change_from_another_device() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let (app, _srv, cookie, item) = signed_in_with_a_show().await;
    // the phone, offline a minute ago: an episode, a rating, a status
    let then = chrono::Utc::now() - chrono::Duration::seconds(60);
    let ms = u64::try_from(then.timestamp_millis()).unwrap();
    let mut phone = ClientClock::new(42);
    let log = log_command(
        &item,
        json!({"type": "episode", "season": 1, "episode": 3}),
        Some(then.to_rfc3339()),
        Some(&phone.stamp(ms)),
    );
    let queue = [
        Queued::new(log, "Downstream", ms),
        Queued::new(
            web::offline::rate_command(&item, Some(60), &phone.stamp(ms)),
            "Downstream",
            ms,
        ),
        Queued::new(
            status_command(&item, "paused", &phone.stamp(ms)),
            "Downstream",
            ms,
        ),
    ];
    // the laptop, online, now: completed and rated 80
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/library/{item}/status"),
        &cookie,
        Some(json!({"status": "completed"})),
    )
    .await;
    assert_eq!(s, 200);
    let (s, _) = call(
        &app,
        "PUT",
        &format!("/library/{item}/rating"),
        &cookie,
        Some(json!({"rating": 80})),
    )
    .await;
    assert_eq!(s, 200);
    // the phone comes back online
    let batch = web::offline::flush_batch(&queue);
    assert_eq!(batch.commands.len(), 3, "one push, in the order queued");
    let (s, r) = call(&app, "POST", "/sync/commands", &cookie, Some(json!(batch))).await;
    assert_eq!(s, 200);
    assert!(r.as_array().unwrap().iter().all(|r| r["ok"] == true), "{r}");
    let (_, lib) = call(&app, "GET", "/library?limit=5", &cookie, None).await;
    let entry = &lib["items"][0]["entry"];
    assert_eq!(entry["status"], "completed", "{lib}");
    assert_eq!(entry["rating"], 80, "{lib}");
    let (_, events) = call(
        &app,
        "GET",
        &format!("/items/{item}/progress"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(
        events.as_array().unwrap().len(),
        1,
        "the log is kept: {events}"
    );
}
