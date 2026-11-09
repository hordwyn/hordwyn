//! M17 load test: search and the library with 50 000 items (`LOAD_ITEMS` to change), and
//! the phase 1 endpoints (`/home`, `/library/query`, `/stats/totals`, `/stats/series`,
//! `/activity` with and without the History filters, `POST /items/{id}/progress`) over the
//! same library with progress events, book series, board game plays and sessions a linked
//! user shares. Ignored by default (seeding and indexing take a while):
//! `cargo xtask load [--release] [--clients N]`, or
//! `cargo test -p server --test load -- --ignored --nocapture`.
//!
//! Knobs (environment): `LOAD_ITEMS` (50 000 movies), `LOAD_EVENTS` (5 000 finished events),
//! `LOAD_SESSIONS` (500 own plays), `LOAD_SHARED` (50 sessions a linked user shares),
//! `LOAD_CLIENTS` (1 client running the mixed set; more run it concurrently),
//! `LOAD_WRITERS` (0 writers logging progress while the clients read).
//! Prints one line per endpoint (p50 / p95 / max) and fails if a p95 exceeds its budget; with
//! several clients in a debug build the numbers are only reported (WP4-1).

// One end-to-end scenario on purpose: the steps share a seeded database and a server.
#![allow(clippy::too_many_lines)]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

/// p95 budget of every endpoint (debug build on a small VM; release builds are faster).
const BUDGET: Duration = Duration::from_millis(300);
/// The first statistics read after a zone change rebuilds the cache (DEC-404 target 500 ms, not
/// met, about 770 ms); asserted so a regression to seconds is caught (UX4-8).
const ZONE_BUDGET: Duration = Duration::from_millis(1000);
/// Books in series (series of 10; the first 5 of each completed, the rest planned).
const BOOKS: i64 = 500;
/// Board games, each in progress; the plays are spread over them.
const GAMES: i64 = 50;
/// Rounds of the mixed set per client.
const ROUNDS: usize = 10;

fn knob(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[derive(Debug, Clone, Copy)]
struct Cfg {
    items: i64,
    /// Finished events on completed movies, three hours apart (at most the completed movies).
    events: i64,
    /// Own plays (sessions with a progress event each), two hours apart.
    sessions: i64,
    /// Sessions of a linked user with the load user taking part, shared with them.
    shared: i64,
    clients: usize,
    writers: usize,
}

impl Cfg {
    fn from_env() -> Self {
        Self {
            items: knob("LOAD_ITEMS", 50_000),
            events: knob("LOAD_EVENTS", 5_000),
            sessions: knob("LOAD_SESSIONS", 500),
            shared: knob("LOAD_SHARED", 50),
            clients: usize::try_from(knob("LOAD_CLIENTS", 1).max(1)).unwrap_or(1),
            writers: usize::try_from(knob("LOAD_WRITERS", 0).max(0)).unwrap_or(0),
        }
    }

    fn entries(&self) -> i64 {
        // the movies, the books and the games, plus the writers' book
        self.items + BOOKS + GAMES + 1
    }

    /// Budgets are asserted for a single client, and in release builds for any number.
    fn asserted(&self) -> bool {
        self.clients == 1 || !cfg!(debug_assertions)
    }
}

/// One line per endpoint; answers the p95.
fn stats(name: &str, mut v: Vec<Duration>) -> Duration {
    v.sort();
    let at = |pct: usize| v[(v.len() - 1) * pct / 100];
    println!(
        "{name:<30} n={:<4} p50={:>7.1?} p95={:>7.1?} max={:>7.1?}",
        v.len(),
        at(50),
        at(95),
        v[v.len() - 1]
    );
    at(95)
}

/// A signed-in HTTP client of the server under test.
#[derive(Clone)]
struct Client {
    http: reqwest::Client,
    base: String,
    token: String,
}

impl Client {
    async fn send(
        &self,
        req: reqwest::RequestBuilder,
        what: &str,
    ) -> (Duration, serde_json::Value) {
        let t = Instant::now();
        let r = req.bearer_auth(&self.token).send().await.unwrap();
        assert!(r.status().is_success(), "{what}: {}", r.status());
        let body: serde_json::Value = r.json().await.unwrap();
        (t.elapsed(), body)
    }

    async fn get(&self, path: &str) -> (Duration, serde_json::Value) {
        self.send(self.http.get(format!("{}{path}", self.base)), path)
            .await
    }

    async fn post(&self, path: &str, body: serde_json::Value) -> (Duration, serde_json::Value) {
        self.send(
            self.http.post(format!("{}{path}", self.base)).json(&body),
            path,
        )
        .await
    }

    async fn put(&self, path: &str, body: serde_json::Value) -> (Duration, serde_json::Value) {
        self.send(
            self.http.put(format!("{}{path}", self.base)).json(&body),
            path,
        )
        .await
    }
}

async fn sign_up(http: &reqwest::Client, base: &str, name: &str) -> (String, uuid::Uuid) {
    let signup: serde_json::Value = http
        .post(format!("{base}/api/v1/auth/sign-up"))
        .json(&serde_json::json!({
            "username": name, "display_name": name, "password": "correct horse battery"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = signup["token"].as_str().unwrap().to_string();
    let user: uuid::Uuid = signup["user"]["id"].as_str().unwrap().parse().unwrap();
    (token, user)
}

/// What the clients need besides the client itself.
struct Fixture {
    cfg: Cfg,
    /// A companion of the load user on every tenth own play (History person filter).
    anna: uuid::Uuid,
    /// The book the clients and writers log pages of.
    book: uuid::Uuid,
    /// Pages logged so far (forward moves).
    page: AtomicU32,
    /// A year with own activity (History year filter).
    year: i32,
    /// Cursors of the last 100 rows of the library page and of the default library query
    /// (keyset paging, DEC-416), found once by walking the pages.
    last_page: String,
    last_query: String,
}

/// The cursor after the first `skip` rows: walks the pages 500 rows at a time. `query` walks
/// the library query, else the library page.
async fn cursor_after(c: &Client, skip: u64, query: bool) -> String {
    let (mut seen, mut after) = (0u64, None::<String>);
    while seen < skip {
        let limit = (skip - seen).min(500);
        let body = if query {
            let mut q = serde_json::json!({ "limit": limit });
            if let Some(a) = &after {
                q["after"] = a.clone().into();
            }
            c.post("/api/v1/library/query", q).await.1
        } else {
            let a = after
                .as_deref()
                .map(|a| format!("&after={a}"))
                .unwrap_or_default();
            c.get(&format!("/api/v1/library?limit={limit}{a}")).await.1
        };
        let rows = body[if query { "rows" } else { "items" }]
            .as_array()
            .unwrap()
            .len();
        seen += rows as u64;
        after = Some(body["next"].as_str().expect("more pages").to_string());
    }
    after.unwrap_or_default()
}

type Samples = BTreeMap<&'static str, Vec<Duration>>;

/// The mixed set, `ROUNDS` times.
async fn client_rounds(c: Client, fx: Arc<Fixture>) -> Samples {
    let mut out: Samples = BTreeMap::new();
    let mut put = |name: &'static str, d: Duration| out.entry(name).or_default().push(d);
    let cfg = fx.cfg;
    let entries = cfg.entries();
    let queries = [
        "amber+cedar",
        "cedar+ember+4242",
        "granit",
        "juniper+delta+777",
        "indigo",
    ];
    for round in 0..ROUNDS {
        for q in queries.iter().cycle().skip(round).take(3) {
            let (d, body) = c.get(&format!("/api/v1/search?q={q}")).await;
            if *q == "cedar+ember+4242" && cfg.items >= 4242 {
                assert!(
                    body["items"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|x| x["title"] == "Cedar Ember 4242"),
                    "{body}"
                );
            }
            put("search (catalog + library)", d);
        }
        let (d, body) = c.get("/api/v1/library?limit=100").await;
        assert_eq!(body["items"].as_array().unwrap().len(), 100);
        assert_eq!(body["total"].as_i64().unwrap(), entries);
        put("library, first 100", d);
        let (d, body) = c
            .get(&format!("/api/v1/library?limit=100&after={}", fx.last_page))
            .await;
        assert_eq!(body["items"].as_array().unwrap().len(), 100);
        assert!(body["next"].is_null(), "the last page");
        put("library, last 100", d);

        let (d, body) = c.get("/api/v1/home").await;
        assert_eq!(body["continue_rows"].as_array().unwrap().len(), 6, "{body}");
        let up_next = body["up_next"].as_array().unwrap();
        assert_eq!(up_next.len(), 6);
        assert_eq!(up_next[0]["reason"], "next_in_series", "{body}");
        assert_eq!(body["activity"]["rows"].as_array().unwrap().len(), 5);
        put("home", d);
        let (d, body) = c
            .post("/api/v1/library/query", serde_json::json!({"limit": 100}))
            .await;
        assert_eq!(body["rows"].as_array().unwrap().len(), 100);
        assert_eq!(body["total"], entries);
        put("library query, first 100", d);
        let (d, body) = c
            .post(
                "/api/v1/library/query",
                serde_json::json!({"limit": 100, "after": fx.last_query}),
            )
            .await;
        assert_eq!(body["rows"].as_array().unwrap().len(), 100);
        assert!(body["next"].is_null(), "the last page");
        put("library query, last 100", d);
        let (d, body) = c.get("/api/v1/stats/totals").await;
        assert_eq!(body["overall"]["entries"], entries);
        assert_eq!(body["overall"]["amounts"]["finished"], cfg.events, "{body}");
        assert_eq!(body["overall"]["amounts"]["plays"], cfg.sessions, "{body}");
        put("stats totals", d);
        let (d, body) = c.get("/api/v1/stats/series?bucket=month").await;
        assert!(
            body["points"].as_array().is_some_and(|p| p.len() > 1),
            "{body}"
        );
        put("stats series, months", d);
        let (d, body) = c.get("/api/v1/activity?limit=20").await;
        assert_eq!(body["rows"].as_array().unwrap().len(), 20);
        put("activity, 20 rows", d);
        let (d, body) = c
            .get(&format!("/api/v1/activity?limit=20&person={}", fx.anna))
            .await;
        assert_eq!(
            body["rows"].as_array().unwrap().len(),
            usize::try_from((cfg.sessions / 10).min(20)).unwrap(),
            "{body}"
        );
        put("history, person", d);
        let (d, body) = c
            .get(&format!("/api/v1/activity?limit=20&year={}", fx.year))
            .await;
        assert_eq!(body["rows"].as_array().unwrap().len(), 20, "{body}");
        put("history, year", d);
        let (d, _) = log_page(&c, &fx).await;
        put("progress, POST", d);
    }
    out
}

async fn log_page(c: &Client, fx: &Fixture) -> (Duration, serde_json::Value) {
    let page = fx.page.fetch_add(1, Ordering::Relaxed) + 1;
    c.post(
        &format!("/api/v1/items/{}/progress", fx.book),
        serde_json::json!({"progress": {"type": "paged", "page": page}}),
    )
    .await
}

/// Logs pages until `stop`, about 20 a second.
async fn writer(c: Client, fx: Arc<Fixture>, stop: Arc<AtomicBool>) -> Vec<Duration> {
    let mut out = vec![];
    while !stop.load(Ordering::Relaxed) {
        out.push(log_page(&c, &fx).await.0);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "load test: run explicitly"]
async fn search_and_library_with_many_items() {
    let cfg = Cfg::from_env();
    let Some(db_url) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let toml = format!(
        "[database]\nurl = \"{db_url}\"\nmax_connections = 20\n[auth]\nsignup = \"open\"\n\
         secure_cookies = false\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [brand]\ndir = \"{}/../../../../config/brand/default\"\n",
        env!("CARGO_MANIFEST_DIR"),
    );
    let app_cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(
        app_cfg,
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
        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move { stop.cancelled().await })
                .await
                .unwrap();
        })
    };
    let http = reqwest::Client::new();
    let (token, user) = sign_up(&http, &base, "load").await;
    let (_, friend) = sign_up(&http, &base, "friend").await;

    let started = Instant::now();
    seed_movies(&srv.pool, user, cfg.items).await;
    let seeded = seed_phase1(&srv.pool, user, friend, &cfg).await;
    let ix = search::indexer::Indexer::new(srv.app.clone(), srv.pool.clone(), srv.search.clone());
    search::indexer::Indexer::enqueue_all(&srv.pool)
        .await
        .unwrap();
    let indexed = ix.drain().await.unwrap();
    sqlx::query("ANALYZE").execute(&srv.pool).await.unwrap();
    println!(
        "seeded {} items and library entries, indexed {indexed} docs in {:.1?}",
        cfg.items,
        started.elapsed()
    );
    println!(
        "config: items={} events={} sessions={} shared={} clients={} writers={} profile={}",
        cfg.items,
        cfg.events,
        cfg.sessions,
        cfg.shared,
        cfg.clients,
        cfg.writers,
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
    let client = Client { http, base, token };
    let entries = u64::try_from(cfg.entries()).unwrap();
    let mut fx = Fixture {
        cfg,
        anna: seeded.anna,
        book: seeded.book,
        page: AtomicU32::new(0),
        year: seeded.year,
        last_page: String::new(),
        last_query: String::new(),
    };
    // the book the clients log moves to the top of "last activity" before the deep cursors
    // are taken (at the bottom it would leave the last page meanwhile)
    log_page(&client, &fx).await;
    fx.last_page = cursor_after(&client, entries - 100, false).await;
    fx.last_query = cursor_after(&client, entries - 100, true).await;
    let fx = Arc::new(fx);
    // warm up
    client.get("/api/v1/search?q=amber").await;
    client.get("/api/v1/home").await;

    let writing = Arc::new(AtomicBool::new(false));
    let writers: Vec<_> = (0..cfg.writers)
        .map(|_| tokio::spawn(writer(client.clone(), fx.clone(), writing.clone())))
        .collect();
    let clients: Vec<_> = (0..cfg.clients)
        .map(|_| tokio::spawn(client_rounds(client.clone(), fx.clone())))
        .collect();
    let mut all: Samples = BTreeMap::new();
    for c in clients {
        for (name, v) in c.await.unwrap() {
            all.entry(name).or_default().extend(v);
        }
    }
    writing.store(true, Ordering::Relaxed);
    let mut written = vec![];
    for w in writers {
        written.extend(w.await.unwrap());
    }

    // the first statistics read after a zone change (a full rebuild with the cache)
    client
        .put(
            "/api/v1/auth/me/time-zone",
            serde_json::json!({"time_zone": "Pacific/Auckland"}),
        )
        .await;
    let (zone_change, _) = client.get("/api/v1/stats/totals").await;

    println!("--- {} items ---", cfg.items);
    let mut p95s: Vec<(&str, Duration)> = all
        .into_iter()
        .map(|(name, v)| (name, stats(name, v)))
        .collect();
    if !written.is_empty() {
        p95s.push(("progress, writers", stats("progress, writers", written)));
    }
    println!(
        "{:<30} once      {zone_change:>7.1?}",
        "stats totals, zone changed"
    );
    stop.cancel();
    let _ = web.await;
    if cfg.asserted() {
        for (name, p95) in p95s {
            assert!(p95 <= BUDGET, "{name} p95 {p95:?} over {BUDGET:?}");
        }
        assert!(
            zone_change <= ZONE_BUDGET,
            "zone change {zone_change:?} over {ZONE_BUDGET:?}"
        );
    } else {
        println!("(several clients in a debug build: reported only)");
    }
}

async fn seed_movies(pool: &sqlx::PgPool, user: uuid::Uuid, n: i64) {
    // n movies with distinct titles, all in the user's library
    let words = "'{Amber,Basalt,Cedar,Delta,Ember,Fjord,Granite,Harbor,Indigo,Juniper}'";
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO catalog_items (id, kind, title, year, state, updated_at)
         SELECT gen_random_uuid(), 'movie',
                ({words}::text[])[1 + g % 10] || ' ' || ({words}::text[])[1 + (g / 10) % 10]
                    || ' ' || g, 1950 + g % 75, 'partial', now()
         FROM generate_series(1, {n}) AS g"
    )))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO library_entries (id, user_id, item_id, kind, status, status_hlc, rating,
             rating_hlc, notes_hlc, added_at)
         SELECT gen_random_uuid(), $1, id, 'movie', 'completed', '0-0-0', 80, '0-0-0', '0-0-0',
                now() - (row_number() OVER ()) * interval '1 minute'
         FROM catalog_items",
    )
    .bind(user)
    .execute(pool)
    .await
    .unwrap();
}

struct Seeded {
    anna: uuid::Uuid,
    book: uuid::Uuid,
    year: i32,
}

/// Statuses (a tenth planned, a tenth in progress, the rest completed), finished events on
/// completed movies, book series, board game plays (a companion on every tenth), the
/// sessions `friend` shares with the user, and a book in progress for the writers.
async fn seed_phase1(
    pool: &sqlx::PgPool,
    user: uuid::Uuid,
    friend: uuid::Uuid,
    cfg: &Cfg,
) -> Seeded {
    let (events, sessions, shared) = (cfg.events, cfg.sessions, cfg.shared);
    let anna = uuid::Uuid::new_v4();
    let linked = uuid::Uuid::new_v4();
    let book = uuid::Uuid::new_v4();
    let statements = [
        "UPDATE library_entries e SET status = CASE x.rn % 10 WHEN 0 THEN 'planned'
                WHEN 1 THEN 'in_progress' ELSE 'completed' END
         FROM (SELECT id, row_number() OVER (ORDER BY id) AS rn FROM library_entries) x
         WHERE x.id = e.id AND e.user_id = $1"
            .to_string(),
        format!(
            "INSERT INTO progress_events (id, user_id, item_id, occurred_at, progress, recorded_at)
             SELECT gen_random_uuid(), user_id, item_id,
                    now() - (row_number() OVER ()) * interval '3 hours',
                    jsonb_build_object('type', 'finished'), now() - interval '2 days'
             FROM library_entries WHERE status = 'completed' AND user_id = $1 LIMIT {events}"
        ),
        format!(
            "INSERT INTO catalog_items (id, kind, title, year, state, updated_at, attributes)
             SELECT gen_random_uuid(), 'book', 'Saga ' || (g / 10) || ' part ' || (g % 10 + 1),
                    2000, 'partial', now(),
                    jsonb_build_object('series', 'Saga ' || (g / 10),
                                       'series_position', g % 10 + 1)
             FROM generate_series(0, {BOOKS} - 1) AS g"
        ),
        "INSERT INTO library_entries (id, user_id, item_id, kind, status, status_hlc, rating,
             rating_hlc, notes_hlc, added_at)
         SELECT gen_random_uuid(), $1, id, 'book',
                CASE WHEN (attributes ->> 'series_position')::int <= 5 THEN 'completed'
                     ELSE 'planned' END, '0-0-0', NULL, '0-0-0', '0-0-0',
                now() - interval '400 days' - (row_number() OVER ()) * interval '1 minute'
         FROM catalog_items WHERE kind = 'book'"
            .to_string(),
        format!(
            "INSERT INTO catalog_items (id, kind, title, year, state, updated_at)
             VALUES ('{book}', 'book', 'Writers book', 2001, 'partial', now())"
        ),
        format!(
            "INSERT INTO library_entries (id, user_id, item_id, kind, status, status_hlc, rating,
                 rating_hlc, notes_hlc, added_at)
             VALUES (gen_random_uuid(), $1, '{book}', 'book', 'in_progress', '0-0-0', NULL,
                     '0-0-0', '0-0-0', now() - interval '600 days')"
        ),
        format!(
            "INSERT INTO catalog_items (id, kind, title, year, state, updated_at)
             SELECT gen_random_uuid(), 'board_game', 'Game ' || g, 2010, 'partial', now()
             FROM generate_series(1, {GAMES}) AS g"
        ),
        "INSERT INTO library_entries (id, user_id, item_id, kind, status, status_hlc, rating,
             rating_hlc, notes_hlc, added_at)
         SELECT gen_random_uuid(), $1, id, 'board_game', 'in_progress', '0-0-0', NULL, '0-0-0',
                '0-0-0', now() - interval '500 days'
         FROM catalog_items WHERE kind = 'board_game'"
            .to_string(),
        format!(
            "INSERT INTO companions (id, owner_id, name, created_at)
             VALUES ('{anna}', $1, 'Anna', now())"
        ),
        format!(
            "INSERT INTO sessions (id, owner_id, item_id, started_at, duration_minutes)
             SELECT gen_random_uuid(), $1, gm.id,
                    now() - g * interval '2 hours' - interval '1 hour', 60
             FROM generate_series(1, {sessions}) AS g
             JOIN (SELECT id, row_number() OVER (ORDER BY id) - 1 AS rn
                   FROM catalog_items WHERE kind = 'board_game') gm ON gm.rn = g % {GAMES}"
        ),
        "INSERT INTO session_participants (session_id, companion_id, position)
         SELECT s.id, u.self_companion, 0 FROM sessions s JOIN users u ON u.id = s.owner_id
         WHERE s.owner_id = $1"
            .to_string(),
        format!(
            "INSERT INTO session_participants (session_id, companion_id, position)
             SELECT id, '{anna}', 1
             FROM (SELECT id, row_number() OVER (ORDER BY started_at DESC) AS rn
                   FROM sessions WHERE owner_id = $1) s
             WHERE s.rn % 10 = 0"
        ),
        "INSERT INTO progress_events (id, user_id, item_id, occurred_at, progress, recorded_at)
         SELECT gen_random_uuid(), owner_id, item_id, started_at,
                jsonb_build_object('type', 'session', 'session_id', id), now() - interval '2 days'
         FROM sessions WHERE owner_id = $1"
            .to_string(),
        // the friend's companion standing for the user, and the plays they share
        format!(
            "INSERT INTO companions (id, owner_id, name, linked_user, created_at)
             SELECT '{linked}', '{friend}', 'Load', $1, now()"
        ),
        format!(
            "INSERT INTO sessions (id, owner_id, item_id, started_at, duration_minutes)
             SELECT gen_random_uuid(), '{friend}', gm.id,
                    now() - g * interval '20 hours' - interval '30 minutes', 45
             FROM generate_series(1, {shared}) AS g
             JOIN (SELECT id, row_number() OVER (ORDER BY id) - 1 AS rn
                   FROM catalog_items WHERE kind = 'board_game') gm ON gm.rn = g % {GAMES}"
        ),
        format!(
            "INSERT INTO session_participants (session_id, companion_id, position)
             SELECT s.id, u.self_companion, 0 FROM sessions s JOIN users u ON u.id = s.owner_id
             WHERE s.owner_id = '{friend}'
             UNION ALL
             SELECT id, '{linked}', 1 FROM sessions WHERE owner_id = '{friend}'"
        ),
    ];
    for sql in statements {
        let bind = sql.contains("$1");
        let q = sqlx::query(sqlx::AssertSqlSafe(sql));
        let q = if bind { q.bind(user) } else { q };
        q.execute(pool).await.unwrap();
    }
    // a year with own activity: the one before the latest event's
    let year: i32 = sqlx::query_scalar(
        "SELECT (extract(year FROM max(occurred_at)) - 1)::int FROM progress_events
         WHERE user_id = $1",
    )
    .bind(user)
    .fetch_one(pool)
    .await
    .unwrap();
    Seeded { anna, book, year }
}
