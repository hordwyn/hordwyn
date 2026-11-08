//! UX phase 1 item page (design.md 6.3, 3.4, 5.4; addendum-1 1.3 Item row): the server renders
//! the facts card, the action bar per progress model, the rating/tags/lists row, the tabs
//! with the episode list and the private note, for seeded items of several kinds.

mod support;

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

/// Creates a catalog item with `attributes` and puts it in the library with `status`.
async fn seed(
    srv: &Server,
    app: &axum::Router,
    cookie: &str,
    (kind, title, status, attributes): (&str, &str, &str, serde_json::Value),
) -> String {
    let item = srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new(kind).unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(2021),
            description: Some(format!("About {title}.")),
            external_ids: Vec::new(),
            attributes,
            cover_url: None,
        })
        .await
        .unwrap();
    let id = item.id.0.to_string();
    let body = serde_json::json!({"item_id": id, "status": status});
    let (code, text) = call(app, "POST", "/api/v1/library", Some(cookie), Some(body)).await;
    assert!(code < 300, "add {title}: {code} {text}");
    id
}

async fn log(app: &axum::Router, cookie: &str, id: &str, progress: serde_json::Value) {
    let uri = format!("/api/v1/items/{id}/progress");
    let body = serde_json::json!({ "progress": progress });
    let (code, text) = call(app, "POST", &uri, Some(cookie), Some(body)).await;
    assert!(code < 300, "log: {code} {text}");
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

/// The part of the page inside the element with `data-testid="<id>"` (up to the next test
/// id is enough for these checks).
fn part<'a>(html: &'a str, id: &str) -> &'a str {
    let start = html
        .find(&format!("data-testid=\"{id}\""))
        .unwrap_or_else(|| panic!("{id} missing"));
    &html[start..]
}

// multi-threaded like production (thread-local page state aborts the stream there)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_show_has_its_next_episode_seasons_episodes_tags_and_note() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "ines").await;
    let seasons = serde_json::json!({"seasons": [
        {"number": 1, "episode_count": 3},
        {"number": 2, "episode_count": 4},
    ]});
    let show = seed(
        &srv,
        &app,
        &cookie,
        ("show", "Northern Lights", "planned", seasons),
    )
    .await;
    for e in [1, 2] {
        log(
            &app,
            &cookie,
            &show,
            serde_json::json!({"type": "episode", "season": 1, "episode": e}),
        )
        .await;
    }
    let uri = format!("/api/v1/library/{show}/tags");
    let (code, _) = call(
        &app,
        "POST",
        &uri,
        Some(&cookie),
        Some(serde_json::json!({"tag": "cosy"})),
    )
    .await;
    assert!(code < 300);
    let uri = format!("/api/v1/library/{show}/note");
    let note = serde_json::json!({"note": "Ask Anna about season 2"});
    let (code, _) = call(&app, "PUT", &uri, Some(&cookie), Some(note)).await;
    assert!(code < 300);

    let (status, html) = call(&app, "GET", &format!("/items/{show}"), Some(&cookie), None).await;
    assert_eq!(status, 200);
    assert!(html.contains(">Northern Lights</h1>"), "title");
    // facts card: status, progress, rating; the rest behind "More facts"
    let facts = part(&html, "item-facts");
    assert!(facts.contains("In progress"), "status in the facts");
    assert!(facts.contains("2 of 7 episodes"), "progress in the facts");
    assert!(html.contains("More facts"), "facts disclosure");
    // the action bar: next episode first, "With...", the overflow menu
    let bar = part(&html, "action-bar");
    assert!(bar.contains(">Watched S1 E3<"), "next step first");
    assert!(
        bar.contains("aria-label=\"Log with details: Northern Lights\""),
        "With..."
    );
    assert!(bar.contains("More actions for Northern Lights"), "overflow");
    // rating, tags, lists in one row
    assert!(html.contains("role=\"radiogroup\""), "rating");
    assert!(html.contains(">cosy<"), "tag");
    assert!(html.contains(">Edit tags<"), "tag editor");
    // tabs: episodes first, then about, history, notes
    for t in ["episodes", "about", "history", "notes"] {
        assert!(html.contains(&format!("id=\"item-tab-{t}\"")), "tab {t}");
    }
    assert!(
        html.contains(
            "id=\"item-tab-episodes\" aria-controls=\"item-panel-episodes\" aria-selected=\"true\""
        ),
        "episodes selected"
    );
    // season chips with counts, the season of the next episode open, its episodes
    assert!(html.contains("Season 1"), "season chip");
    assert!(html.contains(">2/3<"), "season count");
    assert!(html.contains(">0/4<"), "second season count");
    assert_eq!(
        count(&html, "data-testid=\"episode-row\""),
        3,
        "season 1 rows"
    );
    assert!(
        html.contains("aria-label=\"Mark S1 E3 watched\""),
        "row action"
    );
    assert!(html.contains(">Mark season watched<"), "bulk action");
    assert!(
        html.contains("data-next=\"true\""),
        "next-up row highlighted"
    );
    // history (effective, newest first) and the private note
    assert_eq!(
        count(&html, "data-testid=\"activity-row\""),
        2,
        "history rows"
    );
    assert!(html.contains("id=\"entry-note\""), "note field");
    assert!(html.contains("Ask Anna about season 2"), "the note");
    // the attribution footer stays
    assert!(html.contains("data-testid=\"attribution\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_action_bar_follows_the_progress_model() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "jon").await;
    let none = serde_json::json!({});
    let book = seed(
        &srv,
        &app,
        &cookie,
        (
            "book",
            "The Glass Ferry",
            "planned",
            serde_json::json!({"page_count": 410}),
        ),
    )
    .await;
    log(
        &app,
        &cookie,
        &book,
        serde_json::json!({"type": "paged", "page": 212}),
    )
    .await;
    let audio = seed(
        &srv,
        &app,
        &cookie,
        (
            "audiobook",
            "Long Road",
            "planned",
            serde_json::json!({"duration_seconds": 42000}),
        ),
    )
    .await;
    let game = seed(
        &srv,
        &app,
        &cookie,
        ("board_game", "Meeple Harbour", "planned", none.clone()),
    )
    .await;
    let movie = seed(
        &srv,
        &app,
        &cookie,
        ("movie", "Harbour Lights", "completed", none),
    )
    .await;

    let (_, html) = call(&app, "GET", &format!("/items/{book}"), Some(&cookie), None).await;
    let bar = part(&html, "action-bar");
    assert!(bar.contains("id=\"pages-at\""), "page field");
    assert!(bar.contains("value=\"212\""), "current page");
    assert!(bar.contains("of 410"), "total pages");
    for b in [">Update<", ">+10<", ">+25<", ">Finished<"] {
        assert!(bar.contains(b), "{b}");
    }
    assert!(
        !html.contains("id=\"item-tab-episodes\""),
        "no episodes tab for a book"
    );
    assert!(html.contains(
        "id=\"item-tab-about\" aria-controls=\"item-panel-about\" aria-selected=\"true\""
    ));

    let (_, html) = call(&app, "GET", &format!("/items/{audio}"), Some(&cookie), None).await;
    let bar = part(&html, "action-bar");
    assert!(bar.contains("id=\"position-at\""), "position field");
    assert!(bar.contains("of 11:40:00"), "total length");
    assert!(bar.contains(">+15 min<"), "shortcut");

    let (_, html) = call(&app, "GET", &format!("/items/{game}"), Some(&cookie), None).await;
    let bar = part(&html, "action-bar");
    assert!(bar.contains(">Log a play<"), "play sheet opener");

    let (_, html) = call(&app, "GET", &format!("/items/{movie}"), Some(&cookie), None).await;
    let bar = part(&html, "action-bar");
    assert!(bar.contains(">Watched again<"), "completed movie");
    assert!(bar.contains("Log with details: Harbour Lights"), "With...");
}

/// Not in the library yet: the primary action adds it, and there is no note.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_item_outside_the_library_offers_to_add_it() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "kai").await;
    let other = srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Elsewhere".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: Vec::new(),
            attributes: serde_json::json!({}),
            cover_url: None,
        })
        .await
        .unwrap();
    let uri = format!("/items/{}", other.id.0);
    let (status, html) = call(&app, "GET", &uri, Some(&cookie), None).await;
    assert_eq!(status, 200);
    assert!(part(&html, "action-bar").contains(">Add to library<"));
    assert!(
        !html.contains("id=\"item-tab-notes\""),
        "no note without an entry"
    );
}
