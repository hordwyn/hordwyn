//! WP3-7 imports over HTTP (design.md 6.11, R-35): a dry run writes nothing and returns its
//! preview, its file runs for real afterwards, a cancel stops the import, an unmatched row is
//! matched by hand and imported, `GET /imports` lists the user's own imports newest first,
//! and the export downloads as JSON Lines or CSV.

use crate::support;

use std::fmt::Write as _;

use serde_json::{Value, json};
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

fn services() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return false;
    }
    true
}

fn legacy_fixture() -> Vec<u8> {
    std::fs::read(repo(
        "src/rust/crates/testkit/fixtures/import/legacy/export.json",
    ))
    .unwrap()
}

fn trakt_history() -> Vec<u8> {
    std::fs::read(repo(
        "src/rust/crates/importers/fixtures/trakt/history.json",
    ))
    .unwrap()
}

struct T {
    srv: Server,
    app: axum::Router,
}

impl T {
    async fn new() -> Self {
        let db = store_pg::testing::fresh_db_url().await.expect("database");
        let toml = format!(
            "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
             [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n",
            repo("config/brand/default")
        );
        let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
        let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
            .await
            .unwrap();
        let options = leptos::prelude::LeptosOptions::builder()
            .output_name("app")
            .build();
        let app = srv.app_router(&options);
        Self { srv, app }
    }

    async fn raw(
        &self,
        method: &str,
        uri: &str,
        cookie: &str,
        body: axum::body::Body,
        json: bool,
    ) -> axum::http::Response<axum::body::Bytes> {
        let mut req = axum::http::Request::builder()
            .method(method)
            .uri(format!("/api/v1{uri}"))
            .header("cookie", cookie);
        if json {
            req = req.header("content-type", "application/json");
        }
        support::send(&self.app, req.body(body).unwrap()).await
    }

    async fn call(
        &self,
        method: &str,
        uri: &str,
        cookie: &str,
        body: Option<Value>,
    ) -> (u16, Value) {
        let json = body.is_some();
        let body = body.map_or_else(axum::body::Body::empty, |b| {
            axum::body::Body::from(b.to_string())
        });
        let resp = self.raw(method, uri, cookie, body, json).await;
        let status = resp.status().as_u16();
        let bytes = resp.into_body();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn sign_up(&self, username: &str) -> String {
        let body = json!({"username": username, "display_name": username,
            "password": "correct horse battery"});
        let resp = self
            .raw(
                "POST",
                "/auth/sign-up",
                "",
                axum::body::Body::from(body.to_string()),
                true,
            )
            .await;
        assert_eq!(resp.status(), 201, "sign-up");
        let cookie = resp.headers()["set-cookie"].to_str().unwrap();
        cookie.split(';').next().unwrap().to_string()
    }

    /// Upload a file; returns the new import's id.
    async fn upload(&self, cookie: &str, format: &str, dry_run: bool, bytes: Vec<u8>) -> String {
        let resp = self
            .raw(
                "POST",
                &format!("/imports?format={format}&dry_run={dry_run}"),
                cookie,
                axum::body::Body::from(bytes),
                false,
            )
            .await;
        assert_eq!(resp.status(), 202);
        let bytes = resp.into_body();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().to_string()
    }

    /// Run the queued import job (the worker's handler).
    async fn run(&self, id: &str) {
        let importer = self.srv.api.importer.as_ref().unwrap();
        let _ = importer.run(id.parse().unwrap()).await;
    }

    async fn library_len(&self, cookie: &str) -> usize {
        let (s, v) = self.call("GET", "/library", cookie, None).await;
        assert_eq!(s, 200, "{v}");
        usize::try_from(v["total"].as_u64().unwrap()).unwrap()
    }
}

#[tokio::test]
async fn a_dry_run_writes_nothing_and_returns_the_preview_then_runs_for_real() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let dry = t.upload(&anna, "legacy-json", true, legacy_fixture()).await;
    t.run(&dry).await;
    let (s, r) = t.call("GET", &format!("/imports/{dry}"), &anna, None).await;
    assert_eq!(s, 200, "{r}");
    assert_eq!(r["state"], "done");
    assert_eq!(r["dry_run"], true);
    assert_eq!(r["total"], 21);
    let p = &r["preview"];
    assert_eq!(p["kinds"]["movie"], 8);
    assert_eq!(p["kinds"]["show"], 3);
    assert_eq!(p["events"], 82);
    assert_eq!(p["samples"].as_array().unwrap().len(), 5);
    assert!(
        p["lost"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["code"] == "estimated_dates" && l["count"] == 31),
        "{p}"
    );
    assert_eq!(t.library_len(&anna).await, 0, "a dry run writes nothing");
    let (s, lists) = t.call("GET", "/lists", &anna, None).await;
    assert_eq!((s, lists.as_array().map(Vec::len)), (200, Some(0)));

    // the preview's file, for real
    let (s, real) = t
        .call("POST", &format!("/imports/{dry}/run"), &anna, None)
        .await;
    assert_eq!(s, 202, "{real}");
    assert_eq!(real["source_import"], dry.as_str());
    let real = real["id"].as_str().unwrap().to_string();
    t.run(&real).await;
    let (_, r) = t
        .call("GET", &format!("/imports/{real}"), &anna, None)
        .await;
    assert_eq!(
        (r["state"].as_str(), r["imported"].as_u64()),
        (Some("done"), Some(21))
    );
    assert_eq!(t.library_len(&anna).await, 21);
}

#[tokio::test]
async fn a_cancel_stops_the_import_and_a_finished_one_cannot_be_cancelled() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let mut file = String::from("{\"format\":\"library-export\",\"version\":1}\n");
    for i in 0..60 {
        let _ = writeln!(
            file,
            "{{\"source_id\":\"r{i}\",\"kind\":\"movie\",\"title\":\"Film {i}\"}}"
        );
    }
    let id = t
        .upload(&anna, "generic-jsonl", false, file.into_bytes())
        .await;
    let (s, _) = t
        .call("POST", &format!("/imports/{id}/cancel"), &anna, None)
        .await;
    assert_eq!(s, 202);
    t.run(&id).await;
    let (_, r) = t.call("GET", &format!("/imports/{id}"), &anna, None).await;
    assert_eq!(r["state"], "cancelled");
    assert_eq!(r["imported"], 0);
    assert_eq!(t.library_len(&anna).await, 0);
    let (s, e) = t
        .call("POST", &format!("/imports/{id}/cancel"), &anna, None)
        .await;
    assert_eq!((s, e["code"].as_str()), (409, Some("import_finished")));
    // someone else's import is not theirs to stop
    let bob = t.sign_up("bob").await;
    let (s, _) = t
        .call("POST", &format!("/imports/{id}/cancel"), &bob, None)
        .await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn an_unmatched_row_is_matched_by_hand_and_imported() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let id = t.upload(&anna, "trakt-json", false, trakt_history()).await;
    t.run(&id).await;
    let (_, r) = t.call("GET", &format!("/imports/{id}"), &anna, None).await;
    assert_eq!(r["imported"], 2, "{r}");
    // UX3-26: the storage key of the file and the owner id are internal
    assert!(
        r.get("source_key").is_none() && r.get("user_id").is_none(),
        "{r}"
    );
    let unmatched = r["unmatched"].as_array().unwrap();
    assert_eq!(unmatched.len(), 1);
    assert_eq!(unmatched[0]["row"]["title"], "A Home Movie");
    let source_id = unmatched[0]["row"]["source_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(t.library_len(&anna).await, 2);

    let item = t
        .srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "A Home Movie".into(),
            original_title: None,
            year: Some(2013),
            description: None,
            external_ids: Vec::new(),
            attributes: json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let show = t
        .srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("show").unwrap(),
            title: "Not a movie".into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: Vec::new(),
            attributes: json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let path = format!("/imports/{id}/match");
    let (s, e) = t
        .call(
            "POST",
            &path,
            &anna,
            Some(json!({"source_id": source_id, "item_id": show.id.0.to_string()})),
        )
        .await;
    assert_eq!((s, e["code"].as_str()), (409, Some("import_kind_mismatch")));
    let (s, r) = t
        .call(
            "POST",
            &path,
            &anna,
            Some(json!({"source_id": source_id, "item_id": item.id.0.to_string()})),
        )
        .await;
    assert_eq!(s, 200, "{r}");
    assert_eq!(r["imported"], 3);
    assert_eq!(r["unmatched"].as_array().map(Vec::len), Some(0));
    assert_eq!(t.library_len(&anna).await, 3);
    let (s, events) = t
        .call(
            "GET",
            &format!("/items/{}/progress", item.id.0),
            &anna,
            None,
        )
        .await;
    assert_eq!(
        (s, events.as_array().map(Vec::len)),
        (200, Some(1)),
        "{events}"
    );
    // the row is gone from the list: a second match is refused
    let (s, _) = t
        .call(
            "POST",
            &path,
            &anna,
            Some(json!({"source_id": source_id, "item_id": item.id.0.to_string()})),
        )
        .await;
    assert_eq!(s, 404);
}

#[tokio::test]
async fn imports_are_listed_newest_first_for_their_owner_only() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let bob = t.sign_up("bob").await;
    let first = t.upload(&anna, "legacy-json", true, legacy_fixture()).await;
    let second = t.upload(&anna, "trakt-json", true, trakt_history()).await;
    let (s, list) = t.call("GET", "/imports", &anna, None).await;
    assert_eq!(s, 200, "{list}");
    let ids: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![second.as_str(), first.as_str()]);
    assert!(list[0].get("source_key").is_none(), "UX3-26: {list}");
    let (_, theirs) = t.call("GET", "/imports", &bob, None).await;
    assert_eq!(theirs.as_array().map(Vec::len), Some(0));
    let (s, _) = t
        .call("GET", &format!("/imports/{first}"), &bob, None)
        .await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn the_export_downloads_as_json_lines_or_csv() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let id = t
        .upload(&anna, "legacy-json", false, legacy_fixture())
        .await;
    t.run(&id).await;
    for (format, kind, first_line) in [
        (
            "jsonl",
            "application/x-ndjson",
            "{\"format\":\"library-export\",\"version\":1}",
        ),
        (
            "csv",
            "text/csv; charset=utf-8",
            "source_id,kind,title,year,status,rating,notes,external_ids,finished_dates,lists,cover_url",
        ),
    ] {
        let resp = t
            .raw(
                "GET",
                &format!("/export?format={format}"),
                &anna,
                axum::body::Body::empty(),
                false,
            )
            .await;
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.headers()["content-type"], kind);
        assert_eq!(
            resp.headers()["content-disposition"],
            format!("attachment; filename=\"library-export.{format}\"").as_str()
        );
        let body = resp.into_body();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(text.lines().next(), Some(first_line));
        // a header and one line per library entry (CSV quotes no newlines here)
        assert_eq!(text.lines().count(), 22, "{format}");
        assert!(text.contains("Fast and Furious"), "lists are exported");
    }
    let (s, _) = t.call("GET", "/export?format=xml", &anna, None).await;
    assert_eq!(s, 400);
}

/// DEC-383 (deferred to WP4-8): another user can neither run nor match someone's import, and
/// a file over the 64 MiB body limit is refused before anything is stored.
#[tokio::test]
async fn another_users_run_and_match_are_refused_and_a_huge_file_too() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let bob = t.sign_up("bob").await;
    let dry = t.upload(&anna, "trakt-json", true, trakt_history()).await;
    let (s, _) = t
        .call("POST", &format!("/imports/{dry}/run"), &bob, None)
        .await;
    assert_eq!(s, 403, "bob runs anna's previewed import");
    let id = t.upload(&anna, "trakt-json", false, trakt_history()).await;
    t.run(&id).await;
    let (_, r) = t.call("GET", &format!("/imports/{id}"), &anna, None).await;
    let source_id = r["unmatched"][0]["row"]["source_id"]
        .as_str()
        .unwrap()
        .to_string();
    let item = t
        .srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "A Home Movie".into(),
            original_title: None,
            year: Some(2013),
            description: None,
            external_ids: Vec::new(),
            attributes: json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let body = json!({"source_id": source_id, "item_id": item.id.0.to_string()});
    let (s, _) = t
        .call("POST", &format!("/imports/{id}/match"), &bob, Some(body))
        .await;
    assert_eq!(s, 403, "bob matches a row of anna's import");
    assert_eq!(t.library_len(&bob).await, 0);
    let (_, still) = t.call("GET", &format!("/imports/{id}"), &anna, None).await;
    assert_eq!(
        still["unmatched"].as_array().map(Vec::len),
        Some(1),
        "{still}"
    );

    let resp = t
        .raw(
            "POST",
            "/imports?format=trakt-json&dry_run=true",
            &anna,
            axum::body::Body::from(vec![b' '; (64 << 20) + 1]),
            false,
        )
        .await;
    assert_eq!(resp.status(), 413);
    let (_, list) = t.call("GET", "/imports", &anna, None).await;
    assert_eq!(
        list.as_array().map(Vec::len),
        Some(2),
        "nothing stored: {list}"
    );
}
