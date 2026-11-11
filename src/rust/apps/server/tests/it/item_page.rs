//! UX phase 1 item page (design.md 6.3, 3.4, 5.4; addendum-1 1.3 Item row): the server renders
//! the facts card, the action bar per progress model, the rating/tags/lists row, the tabs
//! with the episode list and the private note, for seeded items of several kinds.

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
    // the action bar: next episode first, "Watched with...", the overflow menu
    let bar = part(&html, "action-bar");
    assert!(bar.contains(">Mark S1 E3 watched<"), "next step first");
    assert!(
        bar.contains("aria-label=\"Watched with...: Northern Lights\""),
        "Watched with..."
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
    // completed by hand, nothing logged: the first watch is not a rewatch (DEC-559)
    assert!(bar.contains(">Mark as watched<"), "completed movie");
    assert!(!bar.contains("Log a rewatch"), "no rewatch before a watch");
    assert!(
        bar.contains("aria-label=\"Watched with...: Harbour Lights\""),
        "Watched with..."
    );
}

/// The words of the status pill in the header.
fn pill(html: &str) -> &str {
    let rest = part(html, "ip-status");
    let from = rest.find('>').map_or(0, |i| i + 1);
    rest[from..].split('<').next().unwrap_or_default()
}

/// ot2e (DEC-537 model A, DEC-587): a rewatch run of a Completed show in the rendered page:
/// In progress with the run's line, next step and episode counts while it runs; Completed
/// with the count and where it stopped afterwards.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rewatch_run_shows_in_the_header_the_next_step_and_the_list() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "runa").await;
    let seasons = serde_json::json!({"seasons": [{"number": 1, "episode_count": 3}]});
    let show = seed(
        &srv,
        &app,
        &cookie,
        ("show", "Tideline", "completed", seasons),
    )
    .await;
    let ep = |e: u32| serde_json::json!({"type": "episode", "season": 1, "episode": e});
    for e in 1..=3 {
        log(&app, &cookie, &show, ep(e)).await;
    }
    let page = format!("/items/{show}");
    let (status, html) = call(&app, "GET", &page, Some(&cookie), None).await;
    assert_eq!(status, 200);
    assert_eq!(pill(&html), "Completed");
    let fact = part(&html, "item-fact-line");
    assert!(
        fact.contains(">Watched once, finished on "),
        "{}",
        &fact[..120]
    );
    assert!(html.contains(">3/3<"), "every episode watched");

    log(&app, &cookie, &show, serde_json::json!({"type": "rewatch"})).await;
    log(&app, &cookie, &show, ep(1)).await;
    let (_, html) = call(&app, "GET", &page, Some(&cookie), None).await;
    assert_eq!(pill(&html), "In progress");
    let fact = part(&html, "item-fact-line");
    assert!(
        fact.contains(">Rewatch: 1 of 3 episodes. Watched once before.<"),
        "{}",
        &fact[..160]
    );
    let bar = part(&html, "action-bar");
    assert!(bar.contains(">Mark S1 E2 watched<"), "the run's next step");
    assert!(bar.contains(">1 of 3 episodes<"), "the run's progress");
    assert!(html.contains(">1/3<"), "the list shows the run");
    assert!(!html.contains(">3/3<"), "not every view");
    // a run is logged now: no backfill dates
    assert!(!html.contains("Spread over a date range"));
    // the start is a row of the item's history
    assert!(html.contains("Started again from the beginning"));

    // a view of yesterday while the run runs is refused (review OT2E-1)
    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    let uri = format!("/api/v1/items/{show}/progress");
    let body = serde_json::json!({"progress": ep(2), "occurred_at": yesterday});
    let (code, text) = call(&app, "POST", &uri, Some(&cookie), Some(body)).await;
    assert_eq!(code, 422, "{text}");
    assert!(text.contains("\"invalid\""), "{text}");

    // the stop is a marker (DEC-590): Completed, and a row of the history
    let stop = serde_json::json!({"type": "rewatch_stop"});
    log(&app, &cookie, &show, stop).await;
    let (_, html) = call(&app, "GET", &page, Some(&cookie), None).await;
    assert_eq!(pill(&html), "Completed");
    assert!(html.contains("Stopped before the end"));
    let fact = part(&html, "item-fact-line");
    assert!(
        fact.contains(">Watched once; rewatch stopped after 1 of 3 episodes<"),
        "{}",
        &fact[..160]
    );
    assert!(html.contains(">3/3<"), "every view again");
    assert!(html.contains("Spread over a date range"));
}

/// Once a watch is logged, the next step of a movie is the rewatch (DEC-554, DEC-559).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_watched_movie_offers_the_rewatch() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "lea").await;
    let movie = seed(
        &srv,
        &app,
        &cookie,
        ("movie", "Harbour Lights", "planned", serde_json::json!({})),
    )
    .await;
    log(
        &app,
        &cookie,
        &movie,
        serde_json::json!({"type": "finished"}),
    )
    .await;
    let (status, html) = call(&app, "GET", &format!("/items/{movie}"), Some(&cookie), None).await;
    assert_eq!(status, 200);
    let bar = part(&html, "action-bar");
    assert!(bar.contains(">Log a rewatch<"), "watched movie");
    assert!(!bar.contains(">Mark as watched<"), "one next step");
    assert!(
        bar.contains("aria-label=\"Watched with...: Harbour Lights\""),
        "Watched with..."
    );
}

/// Not in the library yet: the first step logs it (and adds it), the second button adds it
/// as planned (DEC-554), and there is no note.
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
    let bar = part(&html, "action-bar");
    assert!(bar.contains(">Mark as watched<"), "the first step");
    assert!(bar.contains(">Want to watch<"), "the add as planned");
    assert!(!bar.contains("Add to library"), "a movie has its verb");
    assert!(!bar.contains("Watched with..."), "no sheet before an entry");
    assert!(
        !html.contains("id=\"item-tab-notes\""),
        "no note without an entry"
    );
}

/// ot2f (OT-11, OT-18): a game's rendered page has the "Plays" tab with its strip (plays,
/// time played, win rate) and a row per play; a co-op play without a result reads "Played
/// together" (never "Lost together"), and the line under the title ends with the time.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_game_page_has_the_plays_tab_with_time_and_the_win_rate() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "playa").await;
    let none = serde_json::json!({});
    let game = seed(
        &srv,
        &app,
        &cookie,
        ("video_game", "Brick Wizards", "in_progress", none),
    )
    .await;
    let page = format!("/items/{game}");
    // no play yet: the tab is there with its count, and it says so
    let (status, html) = call(&app, "GET", &page, Some(&cookie), None).await;
    assert_eq!(status, 200);
    assert!(html.contains(">No plays logged yet.<"), "empty tab");
    assert!(!html.contains("data-testid=\"ip-plays-strip\""));

    let uri = format!("/api/v1/items/{game}/plays");
    let (_, me) = call(&app, "GET", "/api/v1/auth/me", Some(&cookie), None).await;
    let me: serde_json::Value = serde_json::from_str(&me).unwrap();
    let me = me["self_companion"].clone();
    for body in [
        serde_json::json!({"duration_minutes": 90, "coop": "none"}),
        serde_json::json!({"duration_minutes": 30,
            "players": [{"companion_id": me, "score": 12, "winner": true}]}),
    ] {
        let (code, text) = call(&app, "POST", &uri, Some(&cookie), Some(body)).await;
        assert_eq!(code, 200, "{text}");
    }
    let (_, html) = call(&app, "GET", &page, Some(&cookie), None).await;
    let strip = part(&html, "ip-plays-strip");
    for text in [
        ">Plays<",
        ">2<",
        ">Time played<",
        ">2 hours<",
        ">Win rate<",
        ">100%<",
        ">1 of 1 play with a result in your own log<",
    ] {
        assert!(strip.contains(text), "{text}");
    }
    let plays = part(&html, "ip-plays");
    assert!(plays.contains(">Played together<"), "the co-op play");
    assert!(plays.contains(">1 h 30 min<"));
    assert!(plays.contains(">You won<"));
    assert!(!html.contains("Lost together"), "OT-18");
    let fact = part(&html, "item-fact-line");
    assert!(fact.contains(", 2 hours in total<"), "the fact line");
}

/// Review OT2F-2: the "Plays" tab counts a play as the line under the title does, from its
/// history event. A history that came without the plays themselves (an export file carries
/// the events only) is counted, and the tab says what is missing instead of "No plays logged
/// yet.".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plays_without_details_are_counted_on_the_plays_tab() {
    if !services() {
        return;
    }
    let srv = server().await;
    let app = router(&srv);
    let cookie = sign_up(&app, "bareplays").await;
    let none = serde_json::json!({});
    let game = seed(
        &srv,
        &app,
        &cookie,
        ("board_game", "Echo Harbour", "in_progress", none),
    )
    .await;
    for _ in 0..3 {
        let id = uuid::Uuid::now_v7();
        log(
            &app,
            &cookie,
            &game,
            serde_json::json!({"type": "session", "session_id": id}),
        )
        .await;
    }
    let (status, html) = call(&app, "GET", &format!("/items/{game}"), Some(&cookie), None).await;
    assert_eq!(status, 200);
    let fact = part(&html, "item-fact-line");
    assert!(fact.contains("Played 3 times"), "the line under the title");
    let strip = part(&html, "ip-plays-strip");
    for text in [">3<", ">not logged<", ">no results yet<"] {
        assert!(strip.contains(text), "{text}");
    }
    let bare = part(&html, "ip-plays-bare");
    assert!(
        bare.contains(
            ">3 plays have no details (players, result, duration). They count as plays, not in \
             the time or the win rate.<"
        ),
        "{bare}"
    );
    assert!(!html.contains("No plays logged yet."));
}
