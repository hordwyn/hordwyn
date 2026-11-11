//! WP3-6 lists and people over HTTP (design.md 6.5, 6.7, 5.10): a list everyone here may see
//! is readable by another member and listed under "On this instance" (and "Shared with me"
//! once they are linked), a private one is neither; the order set by "move" persists; the
//! share choices make, keep and revoke the public link; the People page carries the stats and
//! the person page the lists the person shares.

use crate::support;

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

struct T {
    srv: Server,
    app: axum::Router,
}

impl T {
    async fn new() -> Self {
        let srv = server().await;
        let options = leptos::prelude::LeptosOptions::builder()
            .output_name("app")
            .build();
        let app = srv.app_router(&options);
        Self { srv, app }
    }

    async fn call(
        &self,
        method: &str,
        uri: &str,
        cookie: &str,
        body: Option<Value>,
    ) -> (u16, Value) {
        let mut req = axum::http::Request::builder()
            .method(method)
            .uri(format!("/api/v1{uri}"))
            .header("cookie", cookie);
        let body = match body {
            Some(b) => {
                req = req.header("content-type", "application/json");
                axum::body::Body::from(b.to_string())
            }
            None => axum::body::Body::empty(),
        };
        let resp = support::send(&self.app, req.body(body).unwrap()).await;
        let status = resp.status().as_u16();
        let bytes = resp.into_body();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// A server-rendered page (signed out).
    async fn page(&self, uri: &str) -> (u16, String) {
        let req = axum::http::Request::builder()
            .uri(uri)
            .header("accept-language", "en")
            .body(axum::body::Body::empty())
            .unwrap();
        let resp = support::send(&self.app, req).await;
        let status = resp.status().as_u16();
        let bytes = resp.into_body();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn sign_up(&self, username: &str) -> String {
        let body = json!({"username": username, "display_name": username,
            "password": "correct horse battery"});
        let resp = support::send(
            &self.app,
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

    async fn item(&self, kind: &str, title: &str) -> String {
        let item = self
            .srv
            .app
            .find_or_create_catalog_item(app::NewCatalogItem {
                kind: domain::kind::KindId::new(kind).unwrap(),
                title: title.into(),
                original_title: None,
                year: Some(2020),
                description: None,
                external_ids: Vec::new(),
                attributes: json!({}),
                cover_url: None,
            })
            .await
            .unwrap();
        item.id.0.to_string()
    }

    async fn visibility(&self, cookie: &str, id: &str, body: Value) -> (u16, Value) {
        self.call(
            "PUT",
            &format!("/lists/{id}/visibility"),
            cookie,
            Some(body),
        )
        .await
    }

    async fn list(&self, cookie: &str, name: &str, items: &[&str]) -> String {
        let (s, l) = self
            .call("POST", "/lists", cookie, Some(json!({"name": name})))
            .await;
        assert_eq!(s, 201, "{l}");
        let id = l["id"].as_str().unwrap().to_string();
        for i in items {
            let (s, v) = self
                .call(
                    "POST",
                    &format!("/lists/{id}/items"),
                    cookie,
                    Some(json!({"item_id": i})),
                )
                .await;
            assert_eq!(s, 200, "{v}");
        }
        id
    }
}

fn names(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn an_instance_list_is_readable_by_others_and_a_private_one_is_not() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let bob = t.sign_up("bob").await;
    let heat = t.item("movie", "Heat").await;
    let open = t.list(&anna, "Open picks", &[&heat]).await;
    let closed = t.list(&anna, "Closed picks", &[&heat]).await;
    let (s, l) = t
        .call(
            "PUT",
            &format!("/lists/{open}/visibility"),
            &anna,
            Some(json!({"visibility": "instance"})),
        )
        .await;
    assert_eq!(
        (s, l["visibility"].as_str()),
        (200, Some("instance")),
        "{l}"
    );

    let (s, page) = t.call("GET", &format!("/lists/{open}"), &bob, None).await;
    assert_eq!(s, 200, "{page}");
    assert_eq!(page["owner_name"], "anna");
    assert_eq!(page["mine"], false);
    assert_eq!(page["items"][0]["item"]["title"], "Heat");
    assert!(page["share_path"].is_null(), "only the owner sees a link");
    let (s, _) = t.call("GET", &format!("/lists/{closed}"), &bob, None).await;
    assert_eq!(s, 403);
    // others cannot change it
    let (s, _) = t
        .call(
            "PATCH",
            &format!("/lists/{open}"),
            &bob,
            Some(json!({"name": "Mine now"})),
        )
        .await;
    assert_eq!(s, 403);

    let (_, ov) = t.call("GET", "/lists/overview", &bob, None).await;
    assert_eq!(names(&ov["on_instance"]), vec!["Open picks"]);
    assert_eq!(
        names(&ov["shared_with_me"]),
        Vec::<String>::new(),
        "not linked yet"
    );
    assert_eq!(ov["on_instance"][0]["count"], 1);
    assert_eq!(ov["on_instance"][0]["kinds"], json!(["movie"]));
    let (_, mine) = t.call("GET", "/lists/overview", &anna, None).await;
    assert_eq!(names(&mine["mine"]), vec!["Open picks", "Closed picks"]);
    assert_eq!(names(&mine["on_instance"]), Vec::<String>::new());

    // once Bob confirms that Anna's "Bob" is him, the list is shared with him too
    let (_, c) = t
        .call("POST", "/companions", &anna, Some(json!({"name": "Bob"})))
        .await;
    let cid = c["id"].as_str().unwrap();
    let (s, _) = t
        .call(
            "POST",
            &format!("/companions/{cid}/link-requests"),
            &anna,
            Some(json!({"username": "bob"})),
        )
        .await;
    assert_eq!(s, 201);
    let (_, people) = t.call("GET", "/people", &bob, None).await;
    let req = people["requests"][0]["id"].as_str().unwrap().to_string();
    let (_, sent) = t.call("GET", "/people", &anna, None).await;
    assert_eq!(sent["people"][0]["link_state"], "requested", "{sent}");
    let (s, _) = t
        .call("POST", &format!("/link-requests/{req}/accept"), &bob, None)
        .await;
    assert_eq!(s, 200);
    let (_, ov) = t.call("GET", "/lists/overview", &bob, None).await;
    assert_eq!(names(&ov["shared_with_me"]), vec!["Open picks"]);
    let (_, person) = t.call("GET", &format!("/people/{cid}"), &anna, None).await;
    assert_eq!(person["person"]["link_state"], "linked", "{person}");
    assert_eq!(person["person"]["linked_username"], "bob");

    // "Add all to my library"
    let (s, added) = t
        .call("POST", &format!("/lists/{open}/library"), &bob, None)
        .await;
    assert_eq!(
        (s, added["added"].as_u64(), added["already"].as_u64()),
        (200, Some(1), Some(0))
    );
}

#[tokio::test]
async fn moving_an_item_persists_the_order_and_the_link_is_made_kept_and_revoked() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let alpha = t.item("movie", "Alpha").await;
    let bravo = t.item("book", "Bravo").await;
    let charlie = t.item("board_game", "Charlie").await;
    let id = t.list(&anna, "Order", &[&alpha, &bravo, &charlie]).await;

    let (s, l) = t
        .call(
            "PUT",
            &format!("/lists/{id}/items/{charlie}/position"),
            &anna,
            Some(json!({"to": 0})),
        )
        .await;
    assert_eq!(s, 200, "{l}");
    let (s, l) = t
        .call(
            "PUT",
            &format!("/lists/{id}/items/{alpha}/position"),
            &anna,
            Some(json!({"to": 2})),
        )
        .await;
    assert_eq!(s, 200, "{l}");
    let (_, page) = t.call("GET", &format!("/lists/{id}"), &anna, None).await;
    let titles: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["item"]["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Charlie", "Bravo", "Alpha"]);

    let (s, l) = t
        .call(
            "PATCH",
            &format!("/lists/{id}"),
            &anna,
            Some(json!({"name": "Ordered", "description": "In this order"})),
        )
        .await;
    assert_eq!((s, l["name"].as_str()), (200, Some("Ordered")), "{l}");
    let (s, _) = t
        .call("DELETE", &format!("/lists/{id}/items/{bravo}"), &anna, None)
        .await;
    assert_eq!(s, 200);
    let (_, page) = t.call("GET", &format!("/lists/{id}"), &anna, None).await;
    assert_eq!(page["description"], "In this order");
    assert_eq!(page["items"].as_array().unwrap().len(), 2);

    // the link: made, kept on a second save, replaced on request, revoked by "private"
    let share = |new_link: bool| json!({"visibility": "link", "new_link": new_link});
    let (_, l) = t.visibility(&anna, &id, share(false)).await;
    let first = l["share_path"].as_str().unwrap().to_string();
    let (_, l) = t.visibility(&anna, &id, share(false)).await;
    assert_eq!(l["share_path"].as_str(), Some(first.as_str()));
    let (_, l) = t.visibility(&anna, &id, share(true)).await;
    let second = l["share_path"].as_str().unwrap().to_string();
    assert_ne!(first, second);
    let anon = "x=y";
    let (s, _) = t.call("GET", &first, anon, None).await;
    assert_eq!(s, 404, "the old link stopped");
    // the public page answers 404 too, so link checkers drop it (UX3-16)
    let (s, html) = t.page(&first).await;
    assert_eq!(s, 404, "replaced link page");
    assert!(
        html.contains("This list is not shared any more"),
        "missing page"
    );
    let (s, html) = t.page(&second).await;
    assert_eq!(s, 200, "shared page");
    assert!(
        !html.contains("This list is not shared any more"),
        "shared page body"
    );
    let (s, shared) = t.call("GET", &second, anon, None).await;
    assert_eq!(s, 200);
    assert_eq!(shared["items"].as_array().unwrap().len(), 2);
    let (_, l) = t
        .visibility(&anna, &id, json!({"visibility": "private"}))
        .await;
    assert!(l["share_path"].is_null());
    let (s, gone) = t.call("GET", &second, anon, None).await;
    assert_eq!((s, gone["code"].as_str()), (404, Some("not_found")));

    let (s, _) = t.call("DELETE", &format!("/lists/{id}"), &anna, None).await;
    assert_eq!(s, 204);
    let (s, _) = t.call("GET", &format!("/lists/{id}"), &anna, None).await;
    assert_eq!(s, 404);
}

#[tokio::test]
async fn the_people_page_has_stats_suggestions_and_undoable_merges() {
    if !services() {
        return;
    }
    let t = T::new().await;
    let anna = t.sign_up("anna").await;
    let game = t.item("board_game", "Azul").await;
    let mut ids = Vec::new();
    for name in ["Lee", "lee.", "Mara"] {
        let (s, c) = t
            .call(
                "POST",
                "/companions",
                &anna,
                Some(json!({"name": name, "notes": "met at club"})),
            )
            .await;
        assert_eq!(s, 201, "{c}");
        ids.push(c["id"].as_str().unwrap().to_string());
    }
    let body = json!({"players": [
        {"companion_id": ids[0], "winner": true},
        {"companion_id": ids[2], "winner": false}
    ]});
    let (s, v) = t
        .call("POST", &format!("/items/{game}/plays"), &anna, Some(body))
        .await;
    assert!(s < 300, "{s} {v}");

    let (s, page) = t.call("GET", "/people", &anna, None).await;
    assert_eq!(s, 200, "{page}");
    let lee = page["people"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Lee")
        .unwrap();
    assert_eq!(
        (lee["together"].as_u64(), lee["plays"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(
        (lee["wins"].as_u64(), lee["decided"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(lee["last"]["title"], "Azul");
    assert_eq!(lee["notes"], "met at club");
    assert_eq!(page["suggestions"][0]["keep_id"], ids[0].as_str(), "{page}");
    assert_eq!(page["suggestions"][0]["remove_id"], ids[1].as_str());

    let (s, _) = t
        .call(
            "POST",
            &format!("/companions/{}/merge", ids[0]),
            &anna,
            Some(json!({"remove": ids[1]})),
        )
        .await;
    assert_eq!(s, 200);
    let (_, person) = t
        .call("GET", &format!("/people/{}", ids[0]), &anna, None)
        .await;
    assert_eq!(person["merged"][0]["id"], ids[1].as_str(), "{person}");
    let (s, back) = t
        .call(
            "POST",
            &format!("/companions/{}/unmerge", ids[1]),
            &anna,
            None,
        )
        .await;
    assert_eq!(
        (s, back["merged_into"].clone()),
        (200, Value::Null),
        "{back}"
    );

    // rename, then delete: Lee is named in a play, so the history keeps him
    let (s, c) = t
        .call(
            "PATCH",
            &format!("/companions/{}", ids[0]),
            &anna,
            Some(json!({"name": "Lee K."})),
        )
        .await;
    assert_eq!((s, c["name"].as_str()), (200, Some("Lee K.")));
    let (s, _) = t
        .call("DELETE", &format!("/companions/{}", ids[0]), &anna, None)
        .await;
    assert_eq!(s, 204);
    let (_, page) = t.call("GET", "/people", &anna, None).await;
    assert_eq!(page["people"].as_array().unwrap().len(), 2, "{page}");
    let (_, feed) = t.call("GET", "/activity", &anna, None).await;
    assert!(feed.to_string().contains("Lee K."), "{feed}");
}
