//! UX phase 1 Home, Stats and History pages (design.md 6.1, 6.6): the server renders the
//! sections, the charts with their text equivalents and the day-grouped history with its URL
//! filters for a seeded user.

use server::{Overrides, Server};
use tower::ServiceExt;

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn server() -> Server {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap()
}

fn router(srv: &Server) -> axum::Router {
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build();
    srv.app_router(&options)
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<serde_json::Value>,
) -> (u16, String) {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("accept-language", "en");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let body = match body {
        Some(b) => {
            req = req.header("content-type", "application/json");
            axum::body::Body::from(b.to_string())
        }
        None => axum::body::Body::empty(),
    };
    let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = resp.status().as_u16();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 23)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn sign_up(app: &axum::Router, username: &str) -> String {
    let body = serde_json::json!({
        "username": username,
        "display_name": username,
        "password": "correct horse battery",
    });
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v1/auth/sign-up")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "sign-up");
    let cookie = resp.headers()["set-cookie"].to_str().unwrap();
    cookie.split(';').next().unwrap().to_string()
}

/// Creates a catalog item and puts it in the library with `status` (and `tag`).
async fn seed(
    srv: &Server,
    app: &axum::Router,
    cookie: &str,
    (kind, title, status, tag): (&str, &str, &str, Option<&str>),
) -> String {
    let item = srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new(kind).unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(2020),
            description: None,
            external_ids: Vec::new(),
            // one movie carries a genre for the facet filter
            attributes: if title == "Harbour Lights" {
                serde_json::json!({"genres": ["Drama"]})
            } else {
                serde_json::json!({})
            },
            cover_url: None,
        })
        .await
        .unwrap();
    let id = item.id.0.to_string();
    let body = serde_json::json!({"item_id": id, "status": status});
    let (code, text) = call(app, "POST", "/api/v1/library", Some(cookie), Some(body)).await;
    assert!(code < 300, "add {title}: {code} {text}");
    if let Some(tag) = tag {
        let uri = format!("/api/v1/library/{id}/tags");
        let body = serde_json::json!({ "tag": tag });
        let (code, text) = call(app, "POST", &uri, Some(cookie), Some(body)).await;
        assert!(code < 300, "tag {title}: {code} {text}");
    }
    id
}

fn services() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return false;
    }
    true
}

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// Log `progress` for an item (at `at`, or now), optionally with companions.
async fn log(
    app: &axum::Router,
    cookie: &str,
    item: &str,
    progress: serde_json::Value,
    at: Option<&str>,
    with: &[String],
) -> String {
    let body = serde_json::json!({"progress": progress, "occurred_at": at, "with": with});
    let uri = format!("/api/v1/items/{item}/progress");
    let (code, text) = call(app, "POST", &uri, Some(cookie), Some(body)).await;
    assert_eq!(code, 200, "log: {text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["id"].as_str().unwrap().to_string()
}

async fn companion(app: &axum::Router, cookie: &str, name: &str) -> String {
    let body = serde_json::json!({ "name": name });
    let (code, text) = call(app, "POST", "/api/v1/companions", Some(cookie), Some(body)).await;
    assert!(code < 300, "companion: {code} {text}");
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["id"].as_str().unwrap().to_string()
}

struct Seeded {
    cookie: String,
    anna: String,
}

/// A show in progress (two episodes, the second with Anna), a planned movie, a book in
/// progress (page 40 on 2026-10-15).
async fn seeded(srv: &Server, app: &axum::Router) -> Seeded {
    let cookie = sign_up(app, "mira").await;
    let anna = companion(app, &cookie, "Anna").await;
    let show = seed(
        srv,
        app,
        &cookie,
        ("show", "Downstream", "in_progress", None),
    )
    .await;
    seed(
        srv,
        app,
        &cookie,
        ("movie", "Harbour Lights", "planned", None),
    )
    .await;
    let book = seed(
        srv,
        app,
        &cookie,
        ("book", "The Glass Ferry", "in_progress", None),
    )
    .await;
    let ep = |n: u32| serde_json::json!({"type": "episode", "season": 1, "episode": n});
    log(
        app,
        &cookie,
        &show,
        ep(1),
        Some("2026-10-14T20:00:00Z"),
        &[],
    )
    .await;
    log(
        app,
        &cookie,
        &book,
        serde_json::json!({"type": "paged", "page": 40}),
        Some("2026-10-15T21:00:00Z"),
        &[],
    )
    .await;
    log(
        app,
        &cookie,
        &show,
        ep(2),
        None,
        std::slice::from_ref(&anna),
    )
    .await;
    Seeded { cookie, anna }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn home_shows_continue_up_next_activity_and_stats() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);

    let fresh = sign_up(&app, "noor").await;
    let (status, html) = call(&app, "GET", "/", Some(&fresh), None).await;
    assert_eq!(status, 200);
    assert!(html.contains("Nothing here yet"), "empty state");
    assert!(html.contains(">Find something<"), "one primary action");
    assert!(!html.contains(">Continue<"), "no empty sections");

    let s = seeded(&srv, &app).await;
    let (status, html) = call(&app, "GET", "/", Some(&s.cookie), None).await;
    assert_eq!(status, 200);
    assert!(html.contains("Welcome back, mira"), "greeting");
    for h in [
        ">Continue<",
        ">Up next<",
        ">Recent activity<",
        ">All-time stats<",
    ] {
        assert!(html.contains(h), "section {h}");
    }
    // Continue: the next step of the show, with its progress
    assert!(
        html.contains("aria-label=\"Watched S1 E3: Downstream\""),
        "next step"
    );
    // Up next: the planned movie with its reason
    assert!(html.contains("Harbour Lights"), "up next");
    assert!(
        html.contains("href=\"/library?status=planned\""),
        "all planned"
    );
    // Activity: newest first, at most 5 rows, link to History
    assert!(html.contains("href=\"/history\""), "activity link");
    assert!(html.contains("with Anna"), "participants");
    assert_eq!(count(&html, "data-testid=\"activity-row\""), 3);
    // Stats: one chart per group with entries, an image with a name, and the numbers as text
    assert!(html.contains("href=\"/stats\""), "stats link");
    assert!(html.contains("<svg role=\"img\""), "native svg chart");
    assert!(
        html.contains(
            "aria-label=\"Watch: 1 planned, 1 in progress, 0 completed, 0 paused, 0 dropped\""
        ),
        "chart summary"
    );
    assert!(
        html.contains("data-testid=\"totals-list-read\""),
        "text equivalent"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stats_has_a_tab_per_group_and_a_table_per_chart() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let s = seeded(&srv, &app).await;

    let (status, html) = call(&app, "GET", "/stats", Some(&s.cookie), None).await;
    assert_eq!(status, 200);
    assert!(html.contains("role=\"tablist\""), "tabs");
    for g in ["all", "watch", "read", "listen", "play"] {
        assert!(html.contains(&format!("id=\"st-group-{g}\"")), "tab {g}");
    }
    assert!(
        html.contains("id=\"st-group-all\" role=\"tab\" aria-selected=\"true\""),
        "All selected"
    );
    assert!(html.contains("<table"), "table equivalent");
    assert!(html.contains("Oct 2026"), "month bucket");

    let (_, html) = call(
        &app,
        "GET",
        "/stats?group=read&bucket=year",
        Some(&s.cookie),
        None,
    )
    .await;
    assert!(
        html.contains("id=\"st-group-read\" role=\"tab\" aria-selected=\"true\""),
        "Read selected"
    );
    assert!(html.contains(">Pages<"), "read measures");
    assert!(html.contains(">2026<"), "year bucket");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_groups_by_day_and_keeps_filters_in_the_url() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let s = seeded(&srv, &app).await;

    let (status, html) = call(&app, "GET", "/history", Some(&s.cookie), None).await;
    assert_eq!(status, 200);
    assert!(html.contains(">Today<"), "today heading");
    assert!(html.contains("Thursday, Oct 15"), "date heading");
    assert_eq!(count(&html, "data-testid=\"activity-row\""), 3);
    assert!(html.contains("href=\"/items/"), "item link");
    assert!(html.contains("Undo: Downstream"), "undo on own recent rows");
    assert!(html.contains("data-testid=\"history-side\""), "side stats");
    assert!(html.contains("href=\"/stats\""), "all stats");
    assert!(!html.contains(">Show more<"), "one page only");

    let (_, html) = call(&app, "GET", "/history?group=read", Some(&s.cookie), None).await;
    assert_eq!(
        count(&html, "data-testid=\"activity-row\""),
        1,
        "group filter"
    );
    assert!(html.contains("The Glass Ferry"));

    let uri = format!("/history?person={}", s.anna);
    let (_, html) = call(&app, "GET", &uri, Some(&s.cookie), None).await;
    assert_eq!(
        count(&html, "data-testid=\"activity-row\""),
        1,
        "person filter"
    );
    assert!(
        html.contains("aria-expanded=\"true\""),
        "filters open while set"
    );

    let (_, html) = call(&app, "GET", "/history?year=2025", Some(&s.cookie), None).await;
    assert!(
        html.contains("Nothing logged in this view"),
        "filtered empty state"
    );

    // more than a page: Load more
    let cookie = sign_up(&app, "pia").await;
    let movie = seed(
        &srv,
        &app,
        &cookie,
        ("movie", "Milk Teeth", "planned", None),
    )
    .await;
    for d in 1..=31 {
        let at = format!("2026-08-{d:02}T12:00:00Z");
        log(
            &app,
            &cookie,
            &movie,
            serde_json::json!({"type": "finished"}),
            Some(&at),
            &[],
        )
        .await;
    }
    let (_, html) = call(&app, "GET", "/history", Some(&cookie), None).await;
    assert_eq!(count(&html, "data-testid=\"activity-row\""), 30);
    assert!(html.contains(">Show more<"), "paging");
    second_page(&app, &cookie).await;

    let (_, html) = call(&app, "GET", "/history", Some(&fresh_user(&app).await), None).await;
    assert!(html.contains("Nothing logged yet"), "empty state");
}

async fn fresh_user(app: &axum::Router) -> String {
    sign_up(app, "quinn").await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_you_hub_links_the_stats_page() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "rosa").await;
    let (_, html) = call(&app, "GET", "/you", Some(&cookie), None).await;
    assert!(html.contains("href=\"/stats\""), "You hub");
}

/// "Show more" asks the activity server function for the page after `next`.
async fn second_page(app: &axum::Router, cookie: &str) {
    use leptos::server_fn::ServerFn;
    let path = web::api::ActivityFeed::PATH;
    let body = serde_json::json!({"query": {"limit": 30}});
    let (code, text) = call(app, "POST", path, Some(cookie), Some(body)).await;
    assert_eq!(code, 200, "{text}");
    let first: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(first["rows"].as_array().unwrap().len(), 30);
    let next = first["next"].as_str().expect("a next page").to_string();
    let body = serde_json::json!({"query": {"limit": 30, "before": next}});
    let (code, text) = call(app, "POST", path, Some(cookie), Some(body)).await;
    assert_eq!(code, 200, "{text}");
    let second: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(second["rows"].as_array().unwrap().len(), 1, "{text}");
}
