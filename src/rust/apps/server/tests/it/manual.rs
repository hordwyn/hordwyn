//! v1-manual through the real worker and Postgres: the refresh job leaves an item entered by
//! hand alone, and the server never fetches a cover address a member typed (v1-plan.md 7,
//! B2 and B5).

use std::sync::Arc;
use std::time::Duration;

use app::ItemDetails;
use app::ports::{JobQueue, Lane, NewJob};
use domain::kind::KindId;
use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

async fn start(http: Arc<testkit::fakes::ReplayHttp>) -> Option<Server> {
    let db_url = store_pg::testing::fresh_db_url().await?;
    let toml = format!(
        "[database]\nurl = \"{db_url}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n\
         [brand]\ndir = \"{}/../../../../config/brand/default\"\n",
        env!("CARGO_MANIFEST_DIR"),
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    Some(
        Server::build(
            cfg,
            config::ServerRole::All,
            Overrides {
                registry: Some(catalog_core::Registry::default()),
                http: Some(http),
                search: None,
            },
        )
        .await
        .unwrap(),
    )
}

#[tokio::test]
async fn the_asset_job_never_fetches_a_cover_a_member_typed() {
    let http = Arc::new(testkit::fakes::ReplayHttp::default());
    let Some(s) = start(http.clone()).await else {
        return;
    };
    let item = s
        .app
        .create_manual_item(
            KindId::new("board_game").unwrap(),
            ItemDetails {
                title: "Zine of the Marsh".into(),
                year: Some(2019),
                cover_url: Some("https://img.example/box.jpg".into()),
            },
            true,
        )
        .await
        .unwrap();
    // a view queues nothing for it
    s.app.item_detail(item.id).await.unwrap();
    let queued: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = $1")
        .bind(app::REFRESH_JOB)
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(queued, 0);
    // a refresh that is queued anyway (an older client, a webhook) runs through the job
    s.queue
        .enqueue(NewJob {
            lane: Lane::Interactive,
            kind: app::REFRESH_JOB.into(),
            payload: serde_json::json!({ "item_id": item.id }),
            dedupe_key: None,
            priority: 0,
            run_at: None,
        })
        .await
        .unwrap();
    let stop = CancellationToken::new();
    let worker = tokio::spawn(s.worker().run(stop.clone()));
    let mut left = 1_i64;
    for _ in 0..100 {
        left = sqlx::query_scalar(
            "SELECT count(*) FROM jobs WHERE kind = $1 AND state IN ('queued', 'running')",
        )
        .bind(app::REFRESH_JOB)
        .fetch_one(&s.pool)
        .await
        .unwrap();
        if left == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    stop.cancel();
    let _ = worker.await;
    assert_eq!(left, 0, "the job ran");
    let failed: i64 =
        sqlx::query_scalar("SELECT count(*) FROM jobs WHERE kind = $1 AND state <> 'done'")
            .bind(app::REFRESH_JOB)
            .fetch_one(&s.pool)
            .await
            .unwrap();
    assert_eq!(failed, 0, "and did not fail");
    let after = s.app.catalog_item(item.id).await.unwrap();
    assert_eq!(
        after, item,
        "untouched: not failed, no retry time, no asset"
    );
    let assets: i64 = sqlx::query_scalar("SELECT count(*) FROM assets")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(assets, 0, "the address was not registered for a fetch");
    assert_eq!(http.requests().len(), 0, "and nothing was requested");
}

// ---------------------------------------------------------------- hosted mode over HTTP

struct Answer {
    status: u16,
    body: serde_json::Value,
    cookie: Option<String>,
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<serde_json::Value>,
) -> Answer {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(format!("/api/v1{uri}"))
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let body = body.map_or_else(String::new, |b| b.to_string());
    let resp = crate::support::send(app, req.body(axum::body::Body::from(body)).unwrap()).await;
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    Answer {
        status: resp.status().as_u16(),
        body: serde_json::from_slice(resp.body()).unwrap_or(serde_json::Value::Null),
        cookie,
    }
}

fn account(name: &str) -> serde_json::Value {
    serde_json::json!({"username": name, "display_name": name, "password": "correct horse battery"})
}

/// On a hosted instance the item is everybody's: a member may add one by hand, but not
/// choose an address other members' browsers load, and not edit details; an admin may do
/// both. On a home instance (the same server before the switch) the member may do all of it.
#[tokio::test]
async fn hosted_mode_keeps_cover_addresses_and_edits_for_admins() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n\
         [search]\nbackend = \"postgres\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n\
         [brand]\ndir = \"{}/../../../../config/brand/default\"\n",
        env!("CARGO_MANIFEST_DIR"),
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    // the first account is the admin
    let r = call(&app, "POST", "/auth/sign-up", None, Some(account("root"))).await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert_eq!(r.body["user"]["role"], "admin");
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    let r = call(&app, "POST", "/auth/sign-up", None, Some(account("member"))).await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert_eq!(r.body["user"]["role"], "member");
    let member = r.cookie.unwrap();
    let m = Some(member.as_str());
    let with_cover = |title: &str| {
        Some(serde_json::json!({
            "kind": "board_game", "title": title, "cover_url": "https://img.example/box.jpg"
        }))
    };

    on_a_home_instance_the_member_may(&app, m).await;

    // hosted
    let r = call(
        &app,
        "PUT",
        "/admin/settings",
        a,
        Some(serde_json::json!({"values": {"instance.hosting": "hosted"}})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = call(&app, "POST", "/items", m, with_cover("Hosted game")).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("forbidden")),
        "{}",
        r.body
    );
    let r = call(
        &app,
        "POST",
        "/items",
        m,
        Some(serde_json::json!({"kind": "board_game", "title": "Hosted game"})),
    )
    .await;
    assert_eq!(
        r.status, 201,
        "without an address the member may: {}",
        r.body
    );
    let hosted_item = r.body["id"].as_str().unwrap().to_string();
    let r = call(&app, "GET", &format!("/items/{hosted_item}/page"), m, None).await;
    assert_eq!(r.body["details_editable"], false, "{}", r.body);
    let r = call(
        &app,
        "PATCH",
        &format!("/items/{hosted_item}"),
        m,
        Some(serde_json::json!({"title": "Mine now"})),
    )
    .await;
    assert_eq!(r.status, 403, "{}", r.body);
    // the admin may do both
    let r = call(&app, "GET", &format!("/items/{hosted_item}/page"), a, None).await;
    assert_eq!(r.body["details_editable"], true, "{}", r.body);
    let r = call(
        &app,
        "PATCH",
        &format!("/items/{hosted_item}"),
        a,
        Some(serde_json::json!({
            "title": "Hosted game", "cover_url": "https://img.example/hosted.jpg"
        })),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["cover"]["url"], "https://img.example/hosted.jpg");
    let r = call(&app, "POST", "/items", a, with_cover("Admin game")).await;
    assert_eq!(r.status, 201, "{}", r.body);
}

/// Home (the default): a member sets a cover address and edits details.
async fn on_a_home_instance_the_member_may(app: &axum::Router, m: Option<&str>) {
    let home_game = Some(serde_json::json!({
        "kind": "board_game", "title": "Home game", "cover_url": "https://img.example/box.jpg"
    }));
    // home (the default): the member sets a cover address and edits
    let r = call(app, "POST", "/items", m, home_game).await;
    assert_eq!(r.status, 201, "{}", r.body);
    let home_item = r.body["id"].as_str().unwrap().to_string();
    let r = call(app, "GET", &format!("/items/{home_item}/page"), m, None).await;
    assert_eq!(r.body["details_editable"], true, "{}", r.body);
    let r = call(
        app,
        "PATCH",
        &format!("/items/{home_item}"),
        m,
        Some(serde_json::json!({"title": "Home game, corrected"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
}
