//! UX phase 1 Library page (design.md 6.2, 3.5): the server renders the group switcher, the
//! kind chips and status tabs with their counts, the cards with their next step and tags,
//! and the state from the URL (filters, status, view) for a seeded user.

use crate::support;

use server::{Overrides, Server};

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
    let resp = support::send(app, req.body(body).unwrap()).await;
    let status = resp.status().as_u16();
    let bytes = resp.into_body();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn sign_up(app: &axum::Router, username: &str) -> String {
    let body = serde_json::json!({
        "username": username,
        "display_name": username,
        "password": "correct horse battery",
    });
    let resp = support::send(
        app,
        axum::http::Request::builder()
            .method("POST")
            .uri("/api/v1/auth/sign-up")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string()))
            .unwrap(),
    )
    .await;
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
            popularity: None,
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
        html.contains("aria-label=\"Mark as watched: Harbour Lights\""),
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
    assert!(
        html.contains("Mark S1 E3 watched: Downstream"),
        "next episode"
    );
    // undo_progress has a flat argument and keeps the URL encoding
    let form = format!("event_id={}", event["id"].as_str().unwrap());
    let resp = support::send(
        &app,
        axum::http::Request::builder()
            .method("POST")
            .uri(web::api::UndoProgress::PATH)
            .header("cookie", &cookie)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(axum::body::Body::from(form))
            .unwrap(),
    )
    .await;
    assert_eq!(resp.status(), 200, "undo");
    let (_, html) = call(&app, "GET", "/library", Some(&cookie), None).await;
    assert!(
        html.contains("Mark S1 E1 watched: Downstream"),
        "back to the first episode (nothing watched before)"
    );
}

/// Bulk edits over HTTP (WP4-3): 500 ids in one request with one result each and an undo
/// that puts them back; 501 ids are refused with 400; removing one item answers an undo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bulk_edits_500_items_and_refuses_more() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "bulky").await;
    let user: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username = 'bulky'")
        .fetch_one(&srv.pool)
        .await
        .unwrap();
    let ids: Vec<uuid::Uuid> = sqlx::query_scalar(
        "INSERT INTO catalog_items (id, kind, title, year, state, updated_at)
         SELECT gen_random_uuid(), 'movie', 'Bulk ' || g, 2000, 'partial', now()
         FROM generate_series(1, 500) AS g RETURNING id",
    )
    .fetch_all(&srv.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_entries (id, user_id, item_id, kind, status, status_hlc, rating,
             rating_hlc, notes_hlc, added_at)
         SELECT gen_random_uuid(), $1, id, 'movie', 'planned', '0-0-0', NULL, '0-0-0', '0-0-0',
                now() FROM unnest($2::uuid[]) AS id",
    )
    .bind(user)
    .bind(&ids)
    .execute(&srv.pool)
    .await
    .unwrap();
    let item_ids: Vec<String> = ids.iter().map(ToString::to_string).collect();

    let body = serde_json::json!({"item_ids": item_ids, "op": "add_tag", "tag": "Cosy"});
    let (code, text) = call(
        &app,
        "POST",
        "/api/v1/library/bulk",
        Some(&cookie),
        Some(body),
    )
    .await;
    assert_eq!(code, 200, "{text}");
    let out: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(out["done"], 500);
    assert_eq!(out["results"].as_array().unwrap().len(), 500);
    let (_, tags) = call(&app, "GET", "/api/v1/tags", Some(&cookie), None).await;
    assert!(tags.contains(r#""entries":500"#), "{tags}");

    let undo = serde_json::json!({"undo": out["undo"]});
    let (code, text) = call(
        &app,
        "POST",
        "/api/v1/library/bulk/undo",
        Some(&cookie),
        Some(undo),
    )
    .await;
    assert_eq!(code, 204, "{text}");
    let (_, tags) = call(&app, "GET", "/api/v1/tags", Some(&cookie), None).await;
    assert_eq!(tags, "[]");

    let mut over = item_ids.clone();
    over.push(uuid::Uuid::new_v4().to_string());
    let body = serde_json::json!({"item_ids": over, "op": "remove"});
    let (code, text) = call(
        &app,
        "POST",
        "/api/v1/library/bulk",
        Some(&cookie),
        Some(body),
    )
    .await;
    assert_eq!(code, 400, "{text}");
    let (code, _) = call(
        &app,
        "POST",
        "/api/v1/library/bulk/undo",
        Some(&cookie),
        Some(serde_json::json!({"undo": "garbage"})),
    )
    .await;
    assert_eq!(code, 400);

    // one item: removed, then back
    let uri = format!("/api/v1/library/{}", item_ids[0]);
    let (code, text) = call(&app, "DELETE", &uri, Some(&cookie), None).await;
    assert_eq!(code, 200, "{text}");
    let out: serde_json::Value = serde_json::from_str(&text).unwrap();
    let (_, counts) = call(&app, "GET", "/api/v1/library/counts", Some(&cookie), None).await;
    assert!(counts.contains(r#""total":499"#), "{counts}");
    let (code, _) = call(&app, "DELETE", &uri, Some(&cookie), None).await;
    assert_eq!(code, 404);
    let undo = serde_json::json!({"undo": out["undo"]});
    let (code, _) = call(
        &app,
        "POST",
        "/api/v1/library/bulk/undo",
        Some(&cookie),
        Some(undo),
    )
    .await;
    assert_eq!(code, 204);
    let (_, counts) = call(&app, "GET", "/api/v1/library/counts", Some(&cookie), None).await;
    assert!(counts.contains(r#""total":500"#), "{counts}");
}

/// UX4-5 / UX4-12 over HTTP: the undo token is signed and bound to the person it was
/// handed to; an edited or someone else's token is refused with 400 and changes nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_undo_token_is_signed_and_bound_to_its_user() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let alice = sign_up(&app, "alice").await;
    let bob = sign_up(&app, "bob").await;
    let user: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE username = 'alice'")
        .fetch_one(&srv.pool)
        .await
        .unwrap();
    let item: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO catalog_items (id, kind, title, year, state, updated_at)
         VALUES (gen_random_uuid(), 'movie', 'Heat', 1995, 'partial', now()) RETURNING id",
    )
    .fetch_one(&srv.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_entries (id, user_id, item_id, kind, status, status_hlc, rating,
             rating_hlc, notes_hlc, added_at)
         VALUES (gen_random_uuid(), $1, $2, 'movie', 'planned', '0-0-0', NULL, '0-0-0', '0-0-0',
                 now())",
    )
    .bind(user)
    .bind(item)
    .execute(&srv.pool)
    .await
    .unwrap();
    let (code, text) = call(
        &app,
        "DELETE",
        &format!("/api/v1/library/{item}"),
        Some(&alice),
        None,
    )
    .await;
    assert_eq!(code, 200, "{text}");
    let out: serde_json::Value = serde_json::from_str(&text).unwrap();
    let token = out["undo"].as_str().unwrap().to_string();
    let undo = |t: &str| Some(serde_json::json!({ "undo": t }));
    let (body, tag) = token.rsplit_once('.').unwrap();
    let forged = format!("{body}A.{tag}");
    for (who, t) in [(&alice, forged.as_str()), (&bob, token.as_str())] {
        let (code, text) = call(
            &app,
            "POST",
            "/api/v1/library/bulk/undo",
            Some(who),
            undo(t),
        )
        .await;
        assert_eq!(code, 400, "{text}");
    }
    let (_, counts) = call(&app, "GET", "/api/v1/library/counts", Some(&bob), None).await;
    assert!(counts.contains(r#""total":0"#), "{counts}");
    let (code, text) = call(
        &app,
        "POST",
        "/api/v1/library/bulk/undo",
        Some(&alice),
        undo(&token),
    )
    .await;
    assert_eq!(code, 204, "{text}");
    let (_, counts) = call(&app, "GET", "/api/v1/library/counts", Some(&alice), None).await;
    assert!(counts.contains(r#""total":1"#), "{counts}");
}
