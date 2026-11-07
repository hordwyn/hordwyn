//! UX phase 1 Library page (design.md 6.2, 3.5): the server renders the group switcher, the
//! kind chips and status tabs with their counts, the cards with their next step and tags,
//! and the state from the URL (filters, status, view) for a seeded user.

use server::{Overrides, Server};
use tower::ServiceExt;

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn server() -> Server {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
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

// multi-threaded like production (thread-local page state aborts the stream there)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_library_renders_groups_chips_and_counts() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);

    let empty = sign_up(&app, "nora").await;
    let (status, html) = call(&app, "GET", "/library", Some(&empty), None).await;
    assert_eq!(status, 200);
    assert!(html.contains("Your library is empty"), "empty state");
    assert!(html.contains(">Find something<"), "empty state action");

    let cookie = sign_up(&app, "lena").await;
    for row in [
        ("movie", "Harbour Lights", "planned", Some("cosy")),
        ("movie", "Ninety Winters", "planned", None),
        ("movie", "Milk Teeth", "completed", None),
        ("show", "Downstream", "in_progress", Some("cosy")),
        ("book", "The Glass Ferry", "planned", None),
        ("board_game", "Meeple Harbour", "planned", None),
    ] {
        seed(&srv, &app, &cookie, row).await;
    }

    let (status, html) = call(&app, "GET", "/library", Some(&cookie), None).await;
    assert_eq!(status, 200);
    assert!(html.contains("6 items"), "count next to the title");
    // the group switcher with every group, All selected
    for g in ["all", "watch", "read", "listen", "play"] {
        assert!(html.contains(&format!("id=\"lib-group-{g}\"")), "group {g}");
    }
    assert!(html.contains("role=\"radiogroup\" aria-label=\"Group\""));
    // kind chips with counts, only kinds with entries
    assert!(html.contains("All kinds"), "all-kinds chip");
    assert!(html.contains("Movie<"), "movie chip");
    assert!(!html.contains("Audiobook<"), "no chip for an empty kind");
    // status tabs with counts under the selection
    assert!(html.contains("Planned 4"), "planned tab");
    assert!(html.contains("In progress 1"), "in-progress tab");
    assert!(html.contains("Completed 1"), "completed tab");
    assert!(html.contains(">All 6<"), "all tab");
    // cards: next-step pill, square art for board games, tags, the quick menu
    assert_eq!(count(&html, "data-testid=\"library-card\""), 6);
    assert!(
        html.contains("aria-label=\"Watched: Harbour Lights\""),
        "one-click pill"
    );
    assert!(
        html.contains("aria-label=\"Log a play: Meeple Harbour\""),
        "play opens the item"
    );
    assert!(html.contains("aspect-square"), "square board game box");
    assert!(html.contains("Actions for Downstream"), "quick menu");
    assert!(html.contains(">cosy<"), "tag on the card");
    assert!(
        html.contains("aria-controls=\"lib-filters\""),
        "one Filters button"
    );

    // the URL state: group, kind, status
    let (_, html) = call(
        &app,
        "GET",
        "/library?group=watch&kind=movie&status=planned",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(count(&html, "data-testid=\"library-card\""), 2);
    assert!(html.contains("Planned 2"), "tab counts follow the kind");
    assert!(!html.contains("Book<"), "only the group's kinds");
    assert!(
        html.contains("id=\"lib-status-planned\" role=\"tab\" aria-selected=\"true\""),
        "selected status tab"
    );

    // tag filter with its removable badge; the list view
    let (_, html) = call(
        &app,
        "GET",
        "/library?tag=cosy&view=list",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(count(&html, "<tr"), 3, "header and two rows");
    assert!(html.contains("Remove filter Tag: cosy"), "badge");

    facet_and_no_match(&app, &cookie).await;
}

/// A facet of the single selected kind, with the values seen as suggestions; and the
/// no-match state.
async fn facet_and_no_match(app: &axum::Router, cookie: &str) {
    let cookie = cookie.to_string();
    let app = app.clone();
    let (_, html) = call(
        &app,
        "GET",
        "/library?kind=movie&facet=genres:Drama",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(
        count(&html, "data-testid=\"library-card\""),
        1,
        "facet filter"
    );
    assert!(html.contains("Remove filter Genre: Drama"), "facet badge");
    assert!(html.contains("for=\"lib-facet-genres\""), "facet field");
    assert!(html.contains("<option value=\"Drama\">"), "seen values");

    // nothing matches: clear filters and the nearest status
    let (_, html) = call(
        &app,
        "GET",
        "/library?kind=show&status=completed",
        Some(&cookie),
        None,
    )
    .await;
    assert!(
        html.contains("Nothing matches these filters"),
        "no-match state"
    );
    assert!(html.contains(">Clear filters<"));
    assert!(html.contains("Show In progress instead"), "nearest status");
}

/// The next-step button logs `{"type": "episode", "season": 1, "episode": 2}`: numbers in
/// nested JSON, so the server function takes a JSON body (URL encoding turns them into
/// strings).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_next_step_logs_an_episode_and_undo_takes_it_back() {
    use leptos::server_fn::ServerFn;
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "otto").await;
    let id = seed(
        &srv,
        &app,
        &cookie,
        ("show", "Downstream", "in_progress", None),
    )
    .await;
    let body = serde_json::json!({
        "item_id": id,
        "progress": {"progress": {"type": "episode", "season": 1, "episode": 2}},
    });
    let (code, event) = call(
        &app,
        "POST",
        web::api::LogProgress::PATH,
        Some(&cookie),
        Some(body),
    )
    .await;
    assert_eq!(code, 200, "{event}");
    let event: serde_json::Value = serde_json::from_str(&event).unwrap();
    let (_, html) = call(&app, "GET", "/library", Some(&cookie), None).await;
    assert!(html.contains("Watched S1 E3: Downstream"), "next episode");
    // undo_progress has a flat argument and keeps the URL encoding
    let form = format!("event_id={}", event["id"].as_str().unwrap());
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(web::api::UndoProgress::PATH)
                .header("cookie", &cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(axum::body::Body::from(form))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "undo");
    let (_, html) = call(&app, "GET", "/library", Some(&cookie), None).await;
    assert!(
        html.contains("Watched S1 E1: Downstream"),
        "back to the first episode (nothing watched before)"
    );
}
