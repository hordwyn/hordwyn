//! M17 load test: search and the library with 50 000 items (`LOAD_ITEMS` to change).
//! Ignored by default (seeding and indexing take a while):
//! `cargo test -p server --test load -- --ignored --nocapture`.
//! Prints p50 / p95 / max per request and fails if a p95 exceeds its budget.

// One end-to-end scenario on purpose: the steps share a seeded database and a server.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use server::{Overrides, Server};
use tokio_util::sync::CancellationToken;

/// p95 budgets (debug build on a small CI VM; release builds are several times faster).
const SEARCH_BUDGET: Duration = Duration::from_millis(300);
const LIBRARY_PAGE_BUDGET: Duration = Duration::from_millis(300);

fn stats(name: &str, mut v: Vec<Duration>) -> Duration {
    v.sort();
    let at = |pct: usize| v[(v.len() - 1) * pct / 100];
    println!(
        "{name:<28} n={:<3} p50={:>7.1?} p95={:>7.1?} max={:>7.1?}",
        v.len(),
        at(50),
        at(95),
        v[v.len() - 1]
    );
    at(95)
}

#[tokio::test]
#[ignore = "load test: run explicitly"]
async fn search_and_library_with_many_items() {
    let n: i64 = std::env::var("LOAD_ITEMS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50_000);
    let Some(db_url) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let toml = format!(
        "[database]\nurl = \"{db_url}\"\nmax_connections = 20\n[auth]\nsignup = \"open\"\n\
         secure_cookies = false\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
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
        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move { stop.cancelled().await })
                .await
                .unwrap();
        })
    };
    let http = reqwest::Client::new();
    let signup: serde_json::Value = http
        .post(format!("{base}/api/v1/auth/sign-up"))
        .json(&serde_json::json!({
            "username": "load", "display_name": "Load", "password": "correct horse battery"
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = signup["token"].as_str().unwrap().to_string();
    let user: uuid::Uuid = signup["user"]["id"].as_str().unwrap().parse().unwrap();

    // seed: n movies with distinct titles, all in the user's library
    let started = Instant::now();
    let words = "'{Amber,Basalt,Cedar,Delta,Ember,Fjord,Granite,Harbor,Indigo,Juniper}'";
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "INSERT INTO catalog_items (id, kind, title, year, state, updated_at)
         SELECT gen_random_uuid(), 'movie',
                ({words}::text[])[1 + g % 10] || ' ' || ({words}::text[])[1 + (g / 10) % 10]
                    || ' ' || g, 1950 + g % 75, 'partial', now()
         FROM generate_series(1, {n}) AS g"
    )))
    .execute(&srv.pool)
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
    .execute(&srv.pool)
    .await
    .unwrap();
    let ix = search::indexer::Indexer::new(srv.app.clone(), srv.pool.clone(), srv.search.clone());
    search::indexer::Indexer::enqueue_all(&srv.pool)
        .await
        .unwrap();
    let indexed = ix.drain().await.unwrap();
    sqlx::query("ANALYZE").execute(&srv.pool).await.unwrap();
    println!(
        "seeded {n} items and library entries, indexed {indexed} docs in {:.1?}",
        started.elapsed()
    );

    let get = |path: String| {
        let http = http.clone();
        let token = token.clone();
        let base = base.clone();
        async move {
            let t = Instant::now();
            let r = http
                .get(format!("{base}{path}"))
                .bearer_auth(&token)
                .send()
                .await
                .unwrap();
            assert!(r.status().is_success(), "{path}: {}", r.status());
            let body: serde_json::Value = r.json().await.unwrap();
            (t.elapsed(), body)
        }
    };

    // warm up
    get("/api/v1/search?q=amber".into()).await;
    let mut search = vec![];
    for (i, q) in [
        "amber cedar",
        "cedar ember 4242",
        "granit",
        "juniper delta 777",
        "indigo",
    ]
    .iter()
    .cycle()
    .take(30)
    .enumerate()
    {
        let (d, body) = get(format!("/api/v1/search?q={}", q.replace(' ', "+"))).await;
        if i == 1 {
            assert!(
                body["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|x| x["title"] == "Cedar Ember 4242"),
                "{body}"
            );
        }
        search.push(d);
    }
    let mut first_page = vec![];
    let mut deep_page = vec![];
    let mut total = 0;
    for _ in 0..10 {
        let (d, body) = get("/api/v1/library?limit=100".into()).await;
        assert_eq!(body["items"].as_array().unwrap().len(), 100);
        total = body["total"].as_u64().unwrap();
        first_page.push(d);
        let (d, body) = get(format!("/api/v1/library?limit=100&offset={}", n - 100)).await;
        assert_eq!(body["items"].as_array().unwrap().len(), 100);
        deep_page.push(d);
    }
    assert_eq!(total, u64::try_from(n).unwrap());

    println!("--- {n} items ---");
    let search_p95 = stats("search (catalog + library)", search);
    let first_p95 = stats("library, first 100", first_page);
    let deep_p95 = stats("library, last 100", deep_page);
    stop.cancel();
    let _ = web.await;
    assert!(
        search_p95 <= SEARCH_BUDGET,
        "search p95 {search_p95:?} over {SEARCH_BUDGET:?}"
    );
    assert!(
        first_p95 <= LIBRARY_PAGE_BUDGET,
        "library p95 {first_p95:?} over {LIBRARY_PAGE_BUDGET:?}"
    );
    assert!(
        deep_p95 <= LIBRARY_PAGE_BUDGET,
        "deep library p95 {deep_p95:?} over {LIBRARY_PAGE_BUDGET:?}"
    );
}
