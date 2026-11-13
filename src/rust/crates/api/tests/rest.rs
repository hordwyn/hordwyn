//! REST API against in-memory adapters (testkit): auth flow, errors, library, OpenAPI.

use std::sync::Arc;

use api::{ApiState, Naming};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use catalog_core::Registry;
use catalog_core::search::SearchFanout;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn state() -> (ApiState, testkit::TestEnv) {
    let mut t = testkit::TestEnv::new();
    let store = Arc::new(notify::MemNotifyStore::default());
    let engine = Arc::new(notify::Engine::new(
        store,
        t.jobs.clone(),
        t.events.clone(),
        t.clock.clone(),
        t.ids.clone(),
        "Shelf",
        "en",
        notify::template::Overrides::new(),
    ));
    t.app.notifications = engine.clone();
    let deliverer = Arc::new(notify::Deliverer::new(
        engine.clone(),
        Arc::new(notify::channels::Channels::default()),
        t.clock.clone(),
        "https://shelf.example.org",
        3,
    ));
    let auth = auth::Auth::new(
        t.app.clone(),
        Arc::new(auth::MemAuthStore::default()),
        auth::AuthSettings {
            signup: auth::SignupMode::Open,
            ..auth::AuthSettings::default()
        },
    );
    let s = ApiState {
        app: t.app.clone(),
        auth,
        oidc: None,
        bus: realtime::Bus::local(),
        search: Arc::new(testkit::fakes::MemSearch::default()),
        fanout: SearchFanout::new(
            t.app.clone(),
            Registry::default(),
            Arc::new(testkit::fakes::ReplayHttp::default()),
        ),
        registry: Registry::default(),
        assets: None,
        importer: None,
        notify: Some(api::NotifyApi {
            engine,
            deliverer,
            web_push_key: None,
        }),
        integrations: Some(Arc::new(integrations::Integrations::new(
            t.app.clone(),
            Arc::new(integrations::MemIntegrationStore::default()),
        ))),
        public_url: "https://shelf.example.org".into(),
        changes: None,
        naming: Naming::from_prefix("shelf", "Shelf"),
        secure_cookies: true,
        default_kinds: vec!["movie".into()],
        admin: None,
    };
    (s, t)
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let req = match body {
        Some(v) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => b.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, headers, json)
}

async fn signed_up(app: &axum::Router) -> String {
    let (status, headers, body) = call(
        app,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        Some(serde_json::json!({
            "username": "anna", "display_name": "Anna", "password": "correct horse battery"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let cookie = headers[header::SET_COOKIE].to_str().unwrap();
    assert!(
        cookie.starts_with("shelf_session="),
        "brand-derived name: {cookie}"
    );
    assert!(cookie.contains("HttpOnly") && cookie.contains("Secure"));
    assert_eq!(body["user"]["role"], "admin", "first account");
    body["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn sign_up_sign_in_and_me() {
    let (s, _) = state();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let (status, _, me) = call(&app, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["username"], "anna");

    let (status, _, err) = call(
        &app,
        "POST",
        "/api/v1/auth/sign-in",
        None,
        Some(serde_json::json!({"username": "anna", "password": "wrong password!!"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(err["code"], "invalid_credentials");

    let (status, _, _) = call(&app, "POST", "/api/v1/auth/sign-out", Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, err) = call(&app, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(err["code"], "unauthorized");
}

/// UX phase 0: each sign-up rule answers with its own code; the availability check
/// normalises like sign-up.
#[tokio::test]
async fn sign_up_errors_name_the_field_and_rule() {
    let (s, _) = state();
    let app = api::router(s);
    let sign_up = |username: &str, password: &str| {
        Some(serde_json::json!({
            "username": username, "display_name": "", "password": password
        }))
    };
    for (name, pw, status, code) in [
        ("ab", "correct horse battery", 422, "username_too_short"),
        ("anna k", "correct horse battery", 422, "username_space"),
        (
            "me@x.org",
            "correct horse battery",
            422,
            "username_invalid_chars",
        ),
        ("anna", "short", 422, "password_too_short"),
    ] {
        let (st, _, err) = call(
            &app,
            "POST",
            "/api/v1/auth/sign-up",
            None,
            sign_up(name, pw),
        )
        .await;
        assert_eq!(st.as_u16(), status, "{name}: {err}");
        assert_eq!(err["code"], code);
    }
    signed_up(&app).await;
    let (st, _, err) = call(
        &app,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        sign_up(" Anna ", "correct horse battery"),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(err["code"], "username_taken");

    let (st, _, v) = call(
        &app,
        "POST",
        "/api/v1/auth/username-available",
        None,
        Some(serde_json::json!({"username": "ANNA"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(
        v,
        serde_json::json!({
            "status": "taken", "username": "anna", "suggestions": ["anna2", "anna3", "anna4"]
        })
    );
    let (_, _, v) = call(
        &app,
        "POST",
        "/api/v1/auth/username-available",
        None,
        Some(serde_json::json!({"username": "Ben"})),
    )
    .await;
    assert_eq!(
        v,
        serde_json::json!({"status": "available", "username": "ben"})
    );
}

#[tokio::test]
async fn library_flow_with_a_companion() {
    let (s, t) = state();
    let item = t
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: None,
            external_ids: vec![domain::catalog::ExternalId::new("tmdb", "949")],
            attributes: serde_json::json!({}),
            cover_url: Some("https://image.tmdb.org/t/p/w780/heat.jpg".into()),
            popularity: None,
        })
        .await
        .unwrap();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let (status, _, c) = call(
        &app,
        "POST",
        "/api/v1/companions",
        Some(&token),
        Some(serde_json::json!({"name": "Ben"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = item.id.0.to_string();
    let (status, _, p) = call(
        &app,
        "POST",
        &format!("/api/v1/items/{id}/progress"),
        Some(&token),
        Some(serde_json::json!({
            "progress": {"type": "finished"},
            "with": [c["id"]],
            "client_op_id": "01900000-0000-7000-8000-000000000001"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{p}");
    let (_, _, lib) = call(&app, "GET", "/api/v1/library", Some(&token), None).await;
    assert_eq!(lib["items"][0]["item"]["title"], "Heat");
    assert_eq!(lib["items"][0]["entry"]["status"], "completed");
    assert_eq!(
        lib["items"][0]["item"]["cover"]["url"],
        "https://image.tmdb.org/t/p/w780/heat.jpg"
    );
    // replaying the same command is a no-op
    let (status, _, again) = call(
        &app,
        "POST",
        &format!("/api/v1/items/{id}/progress"),
        Some(&token),
        Some(serde_json::json!({
            "progress": {"type": "finished"},
            "with": [c["id"]],
            "client_op_id": "01900000-0000-7000-8000-000000000001"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["id"], p["id"]);
    let (_, _, rated) = call(
        &app,
        "PUT",
        &format!("/api/v1/library/{id}/rating"),
        Some(&token),
        Some(serde_json::json!({"rating": 90})),
    )
    .await;
    assert_eq!(rated["rating"], 90);
}

#[tokio::test]
async fn errors_have_stable_codes() {
    let (s, _) = state();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let (status, _, e) = call(&app, "GET", "/api/v1/items/not-a-uuid", Some(&token), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(e["code"], "bad_request");
    let (status, _, e) = call(
        &app,
        "GET",
        "/api/v1/items/01900000-0000-7000-8000-000000000009",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(e["code"], "not_found");
    let (status, _, _) = call(&app, "GET", "/api/v1/library", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn openapi_document_uses_the_brand_and_lists_paths() {
    let (s, _) = state();
    let app = api::router(s);
    let (status, _, doc) = call(&app, "GET", "/api/v1/openapi.json", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(doc["info"]["title"], "Shelf API");
    for p in [
        "/auth/sign-up",
        "/items/{id}",
        "/search",
        "/library",
        "/items/{id}/progress",
        "/items",
        "/items/titled",
        "/events",
    ] {
        assert!(doc["paths"].get(p).is_some(), "missing path {p}");
    }
    assert!(doc["components"]["schemas"]["CatalogItemDto"].is_object());
    // v1-manual: an item entered by hand, and its details
    assert!(doc["paths"]["/items"]["post"].is_object());
    assert!(doc["paths"]["/items/{id}"]["patch"].is_object());
    assert!(
        doc["paths"]["/items/{id}"]["get"].is_object(),
        "still there"
    );
    assert!(doc["components"]["schemas"]["NewItemDto"].is_object());
    assert!(doc["components"]["schemas"]["ItemDetailsDto"].is_object());
}

#[tokio::test]
async fn export_then_import_moves_a_library_to_another_account() {
    let t = testkit::TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let importer = Arc::new(importers::Importer::new(
        t.app.clone(),
        Arc::new(importers::MemImportStore::default()),
        Arc::new(testkit::fakes::MemBlobs::default()),
        t.jobs.clone(),
    ));
    let (mut s, _) = state();
    s.app = t.app.clone();
    s.auth = auth::Auth::new(
        t.app.clone(),
        Arc::new(auth::MemAuthStore::default()),
        auth::AuthSettings {
            signup: auth::SignupMode::Open,
            ..auth::AuthSettings::default()
        },
    );
    s.importer = Some(importer.clone());
    let item = t
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: None,
            external_ids: vec![domain::catalog::ExternalId::new("tmdb", "949")],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let app = api::router(s);
    let anna = signed_up(&app).await;
    call(
        &app,
        "POST",
        &format!("/api/v1/items/{}/progress", item.id.0),
        Some(&anna),
        Some(serde_json::json!({"progress": {"type": "finished"}})),
    )
    .await;
    let req = Request::builder()
        .uri("/api/v1/export")
        .header(header::AUTHORIZATION, format!("Bearer {anna}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.headers()["content-type"], "application/x-ndjson");
    let file = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(file.to_vec()).unwrap();
    assert!(text.lines().next().unwrap().contains("library-export"));
    assert!(text.contains("\"tmdb\":\"949\""));

    let (_, _, ben) = call(
        &app,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        Some(serde_json::json!({"username": "ben", "display_name": "Ben", "password": "correct horse battery"})),
    )
    .await;
    let ben = ben["token"].as_str().unwrap().to_string();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/imports?format=generic-jsonl")
        .header(header::AUTHORIZATION, format!("Bearer {ben}"))
        .body(Body::from(text))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let started: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let id = uuid::Uuid::parse_str(started["id"].as_str().unwrap()).unwrap();
    // the worker would run the queued job; run it directly here
    importer.run(id).await.unwrap();
    let (_, _, report) = call(
        &app,
        "GET",
        &format!("/api/v1/imports/{id}"),
        Some(&ben),
        None,
    )
    .await;
    assert_eq!(report["state"], "done");
    assert_eq!(
        report["matched"], 1,
        "matched the existing item by its TMDB id"
    );
    let (_, _, lib) = call(&app, "GET", "/api/v1/library", Some(&ben), None).await;
    assert_eq!(lib["items"][0]["item"]["id"], item.id.0.to_string());
    assert_eq!(lib["items"][0]["entry"]["status"], "completed");
    // anna cannot read ben's import
    let (status, _, _) = call(
        &app,
        "GET",
        &format!("/api/v1/imports/{id}"),
        Some(&anna),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// M15 over REST: promote by invite, the new user sees and claims the shared session, link
/// requests, demote, merge, privacy, and a link-shared list read without an account.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn sharing_and_companion_flows() {
    let (s, t) = state();
    let item = t
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: None,
            external_ids: vec![domain::catalog::ExternalId::new("tmdb", "949")],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let id = item.id.0.to_string();
    let app = api::router(s);
    let anna = signed_up(&app).await;
    let (_, _, ben) = call(
        &app,
        "POST",
        "/api/v1/companions",
        Some(&anna),
        Some(serde_json::json!({"name": "Ben"})),
    )
    .await;
    let ben_id = ben["id"].as_str().unwrap().to_string();
    call(
        &app,
        "POST",
        &format!("/api/v1/items/{id}/progress"),
        Some(&anna),
        Some(serde_json::json!({"progress": {"type": "finished"}, "with": [ben_id]})),
    )
    .await;
    let (_, _, before) = call(&app, "GET", "/api/v1/sessions", Some(&anna), None).await;
    assert_eq!(before.as_array().unwrap().len(), 1);

    // promote: invite, then sign up with it
    let (status, _, inv) = call(
        &app,
        "POST",
        &format!("/api/v1/companions/{ben_id}/invite"),
        Some(&anna),
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{inv}");
    let invite = inv["token"].as_str().unwrap();
    assert_eq!(inv["path"], format!("/invite/{invite}"));
    let (status, _, signed) = call(
        &app,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        Some(
            serde_json::json!({"username": "ben", "display_name": "Ben B.",
            "password": "correct horse battery", "invite": invite}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{signed}");
    let bent = signed["token"].as_str().unwrap().to_string();

    // the new user sees the shared session and copies it to their log, once
    let (_, _, shared) = call(&app, "GET", "/api/v1/sessions/shared", Some(&bent), None).await;
    assert_eq!(shared[0]["item_title"], "Heat");
    assert_eq!(shared[0]["owner_name"], "Anna");
    assert_eq!(shared[0]["mine"], false);
    let me_rows: Vec<_> = shared[0]["participants"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["is_me"] == true)
        .collect();
    assert_eq!(me_rows.len(), 1);
    assert_eq!(me_rows[0]["name"], "Ben");
    let sid = shared[0]["id"].as_str().unwrap();
    let (status, _, copy) = call(
        &app,
        "POST",
        &format!("/api/v1/sessions/{sid}/claim"),
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    assert_eq!(copy["mine"], true);
    let (_, _, again) = call(
        &app,
        "POST",
        &format!("/api/v1/sessions/{sid}/claim"),
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(again["id"], copy["id"]);
    let (_, _, lib) = call(&app, "GET", "/api/v1/library", Some(&bent), None).await;
    assert_eq!(lib["items"][0]["entry"]["status"], "completed");

    // privacy: the owner stops sharing sessions
    let (_, _, me) = call(
        &app,
        "PUT",
        "/api/v1/auth/me/privacy",
        Some(&anna),
        Some(serde_json::json!({"share_sessions": false})),
    )
    .await;
    assert_eq!(me["share_sessions"], false);
    let (_, _, hidden) = call(&app, "GET", "/api/v1/sessions/shared", Some(&bent), None).await;
    assert_eq!(hidden, serde_json::json!([]));
    call(
        &app,
        "PUT",
        "/api/v1/auth/me/privacy",
        Some(&anna),
        Some(serde_json::json!({"share_sessions": true})),
    )
    .await;

    // demote: history is unchanged
    let (_, _, demoted) = call(
        &app,
        "POST",
        &format!("/api/v1/companions/{ben_id}/unlink"),
        Some(&anna),
        None,
    )
    .await;
    assert_eq!(demoted["linked_user"], serde_json::Value::Null);
    assert_eq!(demoted["name"], "Ben");
    let (_, _, after) = call(&app, "GET", "/api/v1/sessions", Some(&anna), None).await;
    assert_eq!(after, before);

    // link request by username, accepted by the other user
    let (status, _, req) = call(
        &app,
        "POST",
        &format!("/api/v1/companions/{ben_id}/link-requests"),
        Some(&anna),
        Some(serde_json::json!({"username": "ben"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{req}");
    let (_, _, pending) = call(&app, "GET", "/api/v1/link-requests", Some(&bent), None).await;
    assert_eq!(pending[0]["from_name"], "Anna");
    assert_eq!(pending[0]["companion_name"], "Ben");
    let rid = pending[0]["id"].as_str().unwrap();
    let (status, _, linked) = call(
        &app,
        "POST",
        &format!("/api/v1/link-requests/{rid}/accept"),
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{linked}");
    assert_eq!(linked["linked_user"], signed["user"]["id"]);

    // merge a duplicate: history unchanged
    let (_, _, dup) = call(
        &app,
        "POST",
        "/api/v1/companions",
        Some(&anna),
        Some(serde_json::json!({"name": "Benny"})),
    )
    .await;
    let (status, _, kept) = call(
        &app,
        "POST",
        &format!("/api/v1/companions/{ben_id}/merge"),
        Some(&anna),
        Some(serde_json::json!({"remove": dup["id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{kept}");
    let (_, _, cs) = call(&app, "GET", "/api/v1/companions", Some(&anna), None).await;
    assert!(cs.as_array().unwrap().iter().all(|c| c["name"] != "Benny"));
    let (_, _, after) = call(&app, "GET", "/api/v1/sessions", Some(&anna), None).await;
    assert_eq!(after, before);

    // a list shared by link is readable without an account
    let (_, _, list) = call(
        &app,
        "POST",
        "/api/v1/lists",
        Some(&anna),
        Some(serde_json::json!({"name": "Crime"})),
    )
    .await;
    let lid = list["id"].as_str().unwrap();
    call(
        &app,
        "POST",
        &format!("/api/v1/lists/{lid}/items"),
        Some(&anna),
        Some(serde_json::json!({"item_id": id})),
    )
    .await;
    let (_, _, link) = call(
        &app,
        "POST",
        &format!("/api/v1/lists/{lid}/share"),
        Some(&anna),
        None,
    )
    .await;
    let path = link["path"].as_str().unwrap();
    let (status, _, public) = call(&app, "GET", &format!("/api/v1{path}"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{public}");
    assert_eq!(public["owner_name"], "Anna");
    assert_eq!(public["items"][0]["title"], "Heat");
    let (status, _, _) = call(&app, "GET", "/api/v1/shared/lists/nope", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// M13 over REST: a link request lands in the target's inbox; channels keep their secrets;
/// subscriptions are validated; deliveries are logged; only admins edit defaults and texts.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn notifications_inbox_channels_and_subscriptions() {
    let (s, t) = state();
    let app = api::router(s);
    let anna = signed_up(&app).await;
    let (_, _, ben) = call(
        &app,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        Some(serde_json::json!({"username": "ben", "display_name": "Ben",
            "password": "correct horse battery"})),
    )
    .await;
    let bent = ben["token"].as_str().unwrap().to_string();

    // ben subscribes to social events on a signed webhook
    let (status, _, ch) = call(
        &app,
        "POST",
        "/api/v1/notifications/channels",
        Some(&bent),
        Some(serde_json::json!({"name": "Hook",
            "config": {"kind": "webhook", "url": "https://hooks.example.org/x", "secret": ""}})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{ch}");
    assert!(ch["secret"].as_str().unwrap().starts_with("whsec_"));
    assert_eq!(ch["config"]["secret"], "********");
    let (_, _, list) = call(
        &app,
        "GET",
        "/api/v1/notifications/channels",
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(list[0]["secret"], serde_json::Value::Null);
    let (status, _, err) = call(
        &app,
        "PUT",
        "/api/v1/notifications/subscriptions",
        Some(&bent),
        Some(serde_json::json!({"event": "nope.*"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
    let (status, _, err) = call(
        &app,
        "PUT",
        "/api/v1/notifications/subscriptions",
        Some(&bent),
        Some(serde_json::json!({"event": "social.*",
            "mode": {"mode": "digest", "cron": "every day"}})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
    let (status, _, sub) = call(
        &app,
        "PUT",
        "/api/v1/notifications/subscriptions",
        Some(&bent),
        Some(serde_json::json!({"event": "social.*", "timezone": "Europe/Berlin"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sub}");
    assert_eq!(sub["mode"]["mode"], "immediate");

    // anna asks ben to confirm a companion
    let (_, _, c) = call(
        &app,
        "POST",
        "/api/v1/companions",
        Some(&anna),
        Some(serde_json::json!({"name": "B."})),
    )
    .await;
    call(
        &app,
        "POST",
        &format!(
            "/api/v1/companions/{}/link-requests",
            c["id"].as_str().unwrap()
        ),
        Some(&anna),
        Some(serde_json::json!({"username": "ben"})),
    )
    .await;
    let (_, _, inbox) = call(
        &app,
        "GET",
        "/api/v1/notifications/inbox",
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(inbox[0]["title"], "Anna says you are \"B.\"");
    assert_eq!(inbox[0]["read"], false);
    let id = inbox[0]["id"].as_str().unwrap();
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/api/v1/notifications/inbox/{id}/read"),
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, _, inbox) = call(
        &app,
        "GET",
        "/api/v1/notifications/inbox",
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(inbox[0]["read"], true);
    let (_, _, log) = call(
        &app,
        "GET",
        "/api/v1/notifications/deliveries",
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(log[0]["state"], "queued");
    assert_eq!(log[0]["event"], "social.link_request");
    assert!(t.jobs.jobs().iter().any(|j| j.kind == notify::DELIVER_JOB));

    // only admins edit defaults and texts (the first account is admin)
    let (status, _, _) = call(
        &app,
        "PUT",
        "/api/v1/notifications/subscriptions",
        Some(&bent),
        Some(serde_json::json!({"event": "*", "instance_default": true})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _, d) = call(
        &app,
        "PUT",
        "/api/v1/notifications/subscriptions",
        Some(&anna),
        Some(serde_json::json!({"event": "admin.*", "instance_default": true, "locked": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{d}");
    assert_eq!(
        (d["instance_default"].clone(), d["locked"].clone()),
        (true.into(), true.into())
    );
    let (_, _, subs) = call(
        &app,
        "GET",
        "/api/v1/notifications/subscriptions",
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(subs.as_array().unwrap().len(), 2);
    let (status, _, _) = call(
        &app,
        "PUT",
        "/api/v1/notifications/templates",
        Some(&anna),
        Some(
            serde_json::json!({"locale": "en", "key": "social-link_request-title",
            "text": "Link request from { $from }"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, _, c2) = call(
        &app,
        "POST",
        "/api/v1/companions",
        Some(&anna),
        Some(serde_json::json!({"name": "Ben"})),
    )
    .await;
    call(
        &app,
        "POST",
        &format!(
            "/api/v1/companions/{}/link-requests",
            c2["id"].as_str().unwrap()
        ),
        Some(&anna),
        Some(serde_json::json!({"username": "ben"})),
    )
    .await;
    let (_, _, inbox) = call(
        &app,
        "GET",
        "/api/v1/notifications/inbox",
        Some(&bent),
        None,
    )
    .await;
    let titles: Vec<_> = inbox
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["title"].clone())
        .collect();
    assert!(
        titles.contains(&"Link request from Anna".into()),
        "{titles:?}"
    );
    let (_, _, events) = call(
        &app,
        "GET",
        "/api/v1/notifications/events",
        Some(&bent),
        None,
    )
    .await;
    assert!(
        events
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["audience"] == "user")
    );
    // a test send reports what the channel answered (no adapters in this build)
    let (status, _, err) = call(
        &app,
        "POST",
        &format!(
            "/api/v1/notifications/channels/{}/test",
            ch["id"].as_str().unwrap()
        ),
        Some(&bent),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(err["code"], "channel_error");
}

#[tokio::test]
async fn json_bodies_are_limited_but_imports_may_be_large() {
    let (s, _) = state();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let big = "x".repeat(3 << 20);
    let (status, _, _) = call(
        &app,
        "POST",
        "/api/v1/companions",
        Some(&token),
        Some(serde_json::json!({ "name": big })),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    // a 3 MiB import passes the size check (it fails later: no importer in this state)
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/imports?format=generic-jsonl")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(big))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_ne!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

/// Per-user language (owner ruling DEC-171, DEC-201): stored on the account, only supported
/// locales, `null` goes back to the automatic choice.
#[tokio::test]
async fn language_preference_is_stored_and_validated() {
    let (s, _t) = state();
    let app = api::router(s);
    let anna = signed_up(&app).await;
    let (_, _, me) = call(&app, "GET", "/api/v1/auth/me", Some(&anna), None).await;
    assert_eq!(
        me["locale"],
        serde_json::Value::Null,
        "automatic by default"
    );
    let put = |body: serde_json::Value| {
        let (app, anna) = (app.clone(), anna.clone());
        async move {
            call(
                &app,
                "PUT",
                "/api/v1/auth/me/locale",
                Some(&anna),
                Some(body),
            )
            .await
        }
    };
    let (status, _, me) = put(serde_json::json!({"locale": "de"})).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["locale"], "de");
    let (_, _, me) = call(&app, "GET", "/api/v1/auth/me", Some(&anna), None).await;
    assert_eq!(me["locale"], "de", "stored");
    let (status, _, err) = put(serde_json::json!({"locale": "fr"})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
    assert_eq!(err["code"], "invalid");
    let (status, _, me) = put(serde_json::json!({"locale": null})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["locale"], serde_json::Value::Null);
}

/// UX0-4 (design.md 5.2), SEC-5: the wait after repeated failures says how long it lasts, in
/// the body and as `Retry-After`, so the client can show the remaining time.
#[tokio::test]
async fn lock_out_carries_the_seconds_left() {
    let (s, _) = state();
    let app = api::router(s);
    signed_up(&app).await;
    let wrong = || Some(serde_json::json!({"username": "anna", "password": "wrong password!!"}));
    for _ in 0..=auth::ThrottleSettings::default().free_failures {
        let (status, _, _) = call(&app, "POST", "/api/v1/auth/sign-in", None, wrong()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, headers, err) = call(&app, "POST", "/api/v1/auth/sign-in", None, wrong()).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(err["code"], "too_many_attempts");
    let secs = err["retry_after_secs"].as_u64().expect("retry_after_secs");
    assert_eq!(secs, 1, "the first wait");
    assert_eq!(
        headers[header::RETRY_AFTER].to_str().unwrap(),
        secs.to_string()
    );
    // other errors carry no such field
    let (_, _, err) = call(&app, "GET", "/api/v1/library", None, None).await;
    assert!(err.get("retry_after_secs").is_none(), "{err}");
}

/// UX phase 1: kinds and groups, counts, the library query with a text filter, tags,
/// notes, undo, the activity feed, statistics and Home.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn phase1_library_home_history_and_stats() {
    let (s, t) = state();
    let mk = |kind: &str, title: &str, attributes: serde_json::Value| app::NewCatalogItem {
        kind: domain::kind::KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2000),
        description: None,
        external_ids: vec![domain::catalog::ExternalId::new("test", title)],
        attributes,
        cover_url: None,
        popularity: None,
    };
    let show = t
        .app
        .upsert_catalog_item(mk(
            "show",
            "Dark",
            serde_json::json!({"seasons": [{"number": 1, "episode_count": 10}]}),
        ))
        .await
        .unwrap();
    let movie = t
        .app
        .upsert_catalog_item(mk(
            "movie",
            "Heat",
            serde_json::json!({"genres": ["Crime"], "runtime_minutes": 170}),
        ))
        .await
        .unwrap();
    let search = s.search.clone();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let tk = Some(token.as_str());

    let (st, _, kinds) = call(&app, "GET", "/api/v1/kinds", tk, None).await;
    assert_eq!(st, StatusCode::OK, "{kinds}");
    assert_eq!(kinds["groups"][0]["id"], "watch");
    assert_eq!(kinds["groups"][0]["label_key"], "kind-group-watch");
    let movie_kind = kinds["kinds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["id"] == "movie")
        .unwrap();
    assert_eq!(movie_kind["group"], "watch");
    assert_eq!(movie_kind["label_key"], "kind-movie");
    assert_eq!(movie_kind["facets"][0], "genres");

    let show_id = show.id.0.to_string();
    let movie_id = movie.id.0.to_string();
    let (st, _, ev) = call(
        &app,
        "POST",
        &format!("/api/v1/items/{show_id}/progress"),
        tk,
        Some(serde_json::json!({"progress": {"type": "episode", "season": 1, "episode": 1}})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{ev}");
    let (_, _, ev2) = call(
        &app,
        "POST",
        &format!("/api/v1/items/{show_id}/progress"),
        tk,
        Some(serde_json::json!({"progress": {"type": "episode", "season": 1, "episode": 2}})),
    )
    .await;
    let (st, _, undo) = call(
        &app,
        "POST",
        &format!("/api/v1/progress/{}/undo", ev2["id"].as_str().unwrap()),
        tk,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{undo}");
    assert_eq!(undo["progress"]["type"], "undo");
    let (_, _, add) = call(
        &app,
        "POST",
        "/api/v1/library",
        tk,
        Some(serde_json::json!({"item_id": movie_id})),
    )
    .await;
    assert_eq!(add["status"], "planned");

    let (st, _, counts) = call(&app, "GET", "/api/v1/library/counts", tk, None).await;
    assert_eq!(st, StatusCode::OK, "{counts}");
    assert_eq!(counts["total"], 2);
    assert_eq!(
        counts["groups"][0],
        serde_json::json!({"key": "watch", "count": 2})
    );

    let (st, _, rows) = call(
        &app,
        "POST",
        "/api/v1/library/query",
        tk,
        Some(serde_json::json!({"group": "watch", "statuses": ["in_progress"]})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{rows}");
    assert_eq!(rows["total"], 1);
    let row = &rows["rows"][0];
    assert_eq!(row["item"]["title"], "Dark");
    assert_eq!(row["next_step"]["action"], "episode");
    assert_eq!(row["next_step"]["episode"], 2);
    assert_eq!(row["next_step"]["label_key"], "next-step-episode");
    assert_eq!(row["summary"]["episodes_watched"], 1);
    assert!((row["summary"]["fraction"].as_f64().unwrap() - 0.1).abs() < 1e-6);
    let (st, _, bad) = call(
        &app,
        "POST",
        "/api/v1/library/query",
        tk,
        Some(serde_json::json!({"sort": "nope"})),
    )
    .await;
    assert_eq!(
        (st, bad["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("bad_request"))
    );
    // keyset paging (DEC-416): `next` leads to the second and last page; a made-up cursor
    // is refused, and so is a cursor of another order
    let (_, _, one) = call(
        &app,
        "POST",
        "/api/v1/library/query",
        tk,
        Some(serde_json::json!({"sort": "title", "limit": 1})),
    )
    .await;
    let next = one["next"].as_str().expect("a cursor").to_string();
    let (st, _, two) = call(
        &app,
        "POST",
        "/api/v1/library/query",
        tk,
        Some(serde_json::json!({"sort": "title", "limit": 1, "after": next})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{two}");
    assert_eq!(
        (two["total"].as_u64(), two["next"].is_null()),
        (Some(2), true)
    );
    assert_ne!(two["rows"][0]["item"]["id"], one["rows"][0]["item"]["id"]);
    for (body, code) in [
        (serde_json::json!({"after": "50"}), StatusCode::BAD_REQUEST),
        (
            serde_json::json!({"sort": "year", "after": next}),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        let (st, _, e) = call(&app, "POST", "/api/v1/library/query", tk, Some(body)).await;
        assert_eq!(st, code, "{e}");
    }
    let (_, _, p1) = call(&app, "GET", "/api/v1/library?limit=1", tk, None).await;
    let after = p1["next"].as_str().expect("a cursor");
    let (st, _, p2) = call(
        &app,
        "GET",
        &format!("/api/v1/library?limit=1&after={after}"),
        tk,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{p2}");
    assert_eq!(
        (p2["items"].as_array().map(Vec::len), p2["next"].is_null()),
        (Some(1), true)
    );
    assert_ne!(p2["items"][0]["entry"]["id"], p1["items"][0]["entry"]["id"]);
    let (st, _, _) = call(&app, "GET", "/api/v1/library?after=x", tk, None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (_, _, facet) = call(
        &app,
        "POST",
        "/api/v1/library/query",
        tk,
        Some(serde_json::json!({"facets": [{"attribute": "genres", "value": "Crime"}]})),
    )
    .await;
    assert_eq!(
        facet["rows"][0]["next_step"]["label_key"],
        "next-step-watched"
    );

    // text filter: through the library search index (indexed by hand here)
    let me = t.app.user_by_username("anna").await.unwrap().unwrap();
    search
        .upsert(
            "library",
            vec![app::ports::SearchDoc {
                id: "x".into(),
                owner: Some(me.id),
                fields: serde_json::json!({"item_id": movie_id, "title": "Heat"}),
            }],
        )
        .await
        .unwrap();
    let (_, _, text) = call(
        &app,
        "POST",
        "/api/v1/library/query",
        tk,
        Some(serde_json::json!({"q": "hea"})),
    )
    .await;
    assert_eq!(text["total"], 1, "{text}");
    assert_eq!(text["rows"][0]["item"]["title"], "Heat");

    // tags and the private note
    let (st, _, tags) = call(
        &app,
        "POST",
        &format!("/api/v1/library/{movie_id}/tags"),
        tk,
        Some(serde_json::json!({"tag": "Cosy"})),
    )
    .await;
    assert_eq!(
        (st, tags.clone()),
        (StatusCode::OK, serde_json::json!(["Cosy"]))
    );
    let (_, _, all) = call(&app, "GET", "/api/v1/tags", tk, None).await;
    assert_eq!(all, serde_json::json!([{"tag": "Cosy", "entries": 1}]));
    let (st, _, n) = call(
        &app,
        "PUT",
        "/api/v1/tags/cosy",
        tk,
        Some(serde_json::json!({"name": "warm"})),
    )
    .await;
    assert_eq!((st, n), (StatusCode::OK, serde_json::json!(1)));
    let (st, _, left) = call(
        &app,
        "DELETE",
        &format!("/api/v1/library/{movie_id}/tags/WARM"),
        tk,
        None,
    )
    .await;
    assert_eq!((st, left), (StatusCode::OK, serde_json::json!([])));
    let (st, _, noted) = call(
        &app,
        "PUT",
        &format!("/api/v1/library/{movie_id}/note"),
        tk,
        Some(serde_json::json!({"note": "with dad"})),
    )
    .await;
    assert_eq!(
        (st, noted["notes"].as_str()),
        (StatusCode::OK, Some("with dad"))
    );

    let (st, _, feed) = call(&app, "GET", "/api/v1/activity?limit=5", tk, None).await;
    assert_eq!(st, StatusCode::OK, "{feed}");
    assert_eq!(feed["rows"].as_array().unwrap().len(), 1);
    assert_eq!(feed["rows"][0]["event_id"], ev["id"]);
    assert_eq!(feed["rows"][0]["undoable"], true);
    assert_eq!(feed["next"], serde_json::Value::Null);
    let (st, _, _) = call(&app, "GET", "/api/v1/activity?group=play", tk, None).await;
    assert_eq!(st, StatusCode::OK);
    // DEC-383 (deferred to WP4-8): History's `year` filter over REST (the test clock is in
    // 2026, so the one row is in 2026 and none in 2025)
    for (year, rows) in [(2026, 1), (2025, 0)] {
        let uri = format!("/api/v1/activity?limit=5&year={year}");
        let (st, _, feed) = call(&app, "GET", &uri, tk, None).await;
        assert_eq!(st, StatusCode::OK, "{feed}");
        assert_eq!(
            feed["rows"].as_array().unwrap().len(),
            rows,
            "{year}: {feed}"
        );
    }

    let (st, _, totals) = call(&app, "GET", "/api/v1/stats/totals", tk, None).await;
    assert_eq!(st, StatusCode::OK, "{totals}");
    assert_eq!(totals["overall"]["group"], "all");
    assert_eq!(totals["groups"][0]["group"], "watch");
    assert_eq!(totals["groups"][0]["amounts"]["episodes"], 1);
    // OT-17: the amounts per kind for the time tiles
    let show = totals["kinds"]
        .as_array()
        .and_then(|k| k.iter().find(|k| k["kind"] == "show"))
        .unwrap_or_else(|| panic!("kinds: {totals}"));
    assert_eq!(show["amounts"]["episodes"], 1);
    let (st, _, series) = call(
        &app,
        "GET",
        "/api/v1/stats/series?group=watch&bucket=year",
        tk,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{series}");
    assert_eq!(series["points"][0]["bucket"], "2026");
    assert_eq!(series["bucket"], "year");

    let (st, _, home) = call(&app, "GET", "/api/v1/home", tk, None).await;
    assert_eq!(st, StatusCode::OK, "{home}");
    assert_eq!(home["library_total"], 2);
    assert_eq!(home["continue_rows"][0]["item"]["title"], "Dark");
    assert_eq!(home["up_next"][0]["row"]["item"]["title"], "Heat");
    assert_eq!(home["up_next"][0]["reason"], "planned");
    assert_eq!(home["activity"]["rows"].as_array().unwrap().len(), 1);

    let (st, _, item) = call(
        &app,
        "GET",
        &format!("/api/v1/items/{show_id}/page"),
        tk,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{item}");
    assert_eq!(
        item["progress"].as_array().unwrap().len(),
        1,
        "undone event hidden"
    );
    assert_eq!(item["next_step"]["episode"], 2);
    assert_eq!(item["summary"]["episodes_watched"], 1);
}

/// OT-11: the feed row and the item page carry the play (duration, scores, winner).
async fn play_is_read(
    app: &axum::Router,
    tk: Option<&str>,
    gid: &str,
    session: &serde_json::Value,
) {
    let (_, _, feed) = call(app, "GET", "/api/v1/activity", tk, None).await;
    let row = &feed["rows"][0];
    assert_eq!(
        row["play"],
        serde_json::json!({"duration_minutes": 45, "coop": null}),
        "{row}"
    );
    let scores: Vec<_> = row["participants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["is_me"].as_bool(),
                p["score"].as_f64(),
                p["winner"].as_bool(),
            )
        })
        .collect();
    assert_eq!(
        scores,
        vec![
            (Some(true), Some(31.0), Some(false)),
            (Some(false), Some(42.0), Some(true))
        ]
    );
    let (_, _, page) = call(app, "GET", &format!("/api/v1/items/{gid}/page"), tk, None).await;
    let plays = page["plays"].as_array().unwrap();
    assert_eq!(plays.len(), 1, "{page}");
    assert_eq!(&plays[0]["session_id"], session);
    assert_eq!(plays[0]["play"]["duration_minutes"], 45);
    assert_eq!(plays[0]["participants"][1]["name"], "Ben");
    assert_eq!(plays[0]["participants"][1]["score"], 42.0);
    assert_eq!(plays[0]["participants"][1]["winner"], true);
}

/// ot2f: the item page and the Stats page carry the play totals (plays, minutes, the
/// caller's results); a co-op play without a result (OT-18, `coop: "none"`) is a play that
/// decides nothing, and an unknown outcome word is refused.
async fn play_totals_are_read(app: &axum::Router, tk: Option<&str>, gid: &str) {
    let page_uri = format!("/api/v1/items/{gid}/page");
    let (_, _, page) = call(app, "GET", &page_uri, tk, None).await;
    assert_eq!(
        page["play_totals"],
        serde_json::json!({"plays": 1, "minutes": 45, "decided": 1, "wins": 0}),
        "{page}"
    );
    let uri = format!("/api/v1/items/{gid}/plays");
    let coop = |word: &str| serde_json::json!({"duration_minutes": 30, "coop": word});
    let (st, _, err) = call(app, "POST", &uri, tk, Some(coop("draw"))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{err}");
    let (st, _, ev) = call(app, "POST", &uri, tk, Some(coop("none"))).await;
    assert_eq!(st, StatusCode::OK, "{ev}");
    let (_, _, page) = call(app, "GET", &page_uri, tk, None).await;
    assert_eq!(page["plays"][0]["play"]["coop"], "none", "{page}");
    assert_eq!(
        page["play_totals"],
        serde_json::json!({"plays": 2, "minutes": 75, "decided": 1, "wins": 0})
    );
    let (st, _, compare) = call(app, "GET", "/api/v1/stats/compare", tk, None).await;
    assert_eq!(st, StatusCode::OK, "{compare}");
    assert_eq!(
        compare["play"],
        serde_json::json!({"plays": 2, "minutes": 75, "decided": 1, "wins": 0}),
        "{compare}"
    );
    let undo = format!("/api/v1/progress/{}/undo", ev["id"].as_str().unwrap());
    let (st, _, _) = call(app, "POST", &undo, tk, None).await;
    assert_eq!(st, StatusCode::OK);
}

/// The item page's play sheet (design.md 5.4): a play with players, scores and a winner is
/// one call; it answers the progress event (for Undo) and shows up in the sessions with the
/// scores. A kind without the sessions model refuses it.
#[tokio::test]
async fn a_play_with_players_scores_and_a_winner() {
    let (s, t) = state();
    let mk = |kind: &str, title: &str| app::NewCatalogItem {
        kind: domain::kind::KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2017),
        description: None,
        external_ids: vec![domain::catalog::ExternalId::new("test", title)],
        attributes: serde_json::json!({}),
        cover_url: None,
        popularity: None,
    };
    let game = t
        .app
        .upsert_catalog_item(mk("board_game", "Harbour"))
        .await
        .unwrap();
    let movie = t
        .app
        .upsert_catalog_item(mk("movie", "Heat"))
        .await
        .unwrap();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let tk = Some(token.as_str());
    let (_, _, me) = call(&app, "GET", "/api/v1/auth/me", tk, None).await;
    let (_, _, ben) = call(
        &app,
        "POST",
        "/api/v1/companions",
        tk,
        Some(serde_json::json!({"name": "Ben"})),
    )
    .await;
    let play = serde_json::json!({
        "started_at": "2025-12-01T19:00:00Z",
        "duration_minutes": 45,
        "players": [
            {"companion_id": me["self_companion"], "score": 31, "winner": false},
            {"companion_id": ben["id"], "score": 42, "winner": true},
        ],
        "client_op_id": "01900000-0000-7000-8000-0000000000a1",
    });
    let gid = game.id.0.to_string();
    let uri = format!("/api/v1/items/{gid}/plays");
    let (st, _, ev) = call(&app, "POST", &uri, tk, Some(play.clone())).await;
    assert_eq!(st, StatusCode::OK, "{ev}");
    assert_eq!(ev["progress"]["type"], "session");
    assert!(
        ev["occurred_at"]
            .as_str()
            .unwrap()
            .starts_with("2025-12-01T19:00:00")
    );
    let (_, _, sessions) = call(&app, "GET", "/api/v1/sessions", tk, None).await;
    let ps = &sessions[0]["participants"];
    assert_eq!(sessions.as_array().unwrap().len(), 1);
    let benp = ps
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "Ben")
        .unwrap();
    assert_eq!(benp["score"], 42.0);
    assert_eq!(benp["winner"], true);
    let (_, _, lib) = call(&app, "GET", "/api/v1/library", tk, None).await;
    assert_eq!(lib["items"][0]["entry"]["status"], "in_progress");
    play_is_read(&app, tk, &gid, &ev["progress"]["session_id"]).await;
    play_totals_are_read(&app, tk, &gid).await;
    // the play's event can be undone like any other
    let undo = format!("/api/v1/progress/{}/undo", ev["id"].as_str().unwrap());
    let (st, _, _) = call(&app, "POST", &undo, tk, None).await;
    assert_eq!(st, StatusCode::OK);
    // a movie has no plays
    let uri = format!("/api/v1/items/{}/plays", movie.id.0);
    let (st, _, err) = call(&app, "POST", &uri, tk, Some(play)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
}

#[tokio::test]
async fn a_replayed_play_or_watched_with_stores_one_session() {
    // UX1-3: the client op id is the idempotency key of the whole command, session included
    let (s, t) = state();
    let mk = |kind: &str, title: &str| app::NewCatalogItem {
        kind: domain::kind::KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2017),
        description: None,
        external_ids: vec![domain::catalog::ExternalId::new("test", title)],
        attributes: serde_json::json!({}),
        cover_url: None,
        popularity: None,
    };
    let game = t
        .app
        .upsert_catalog_item(mk("board_game", "Harbour"))
        .await
        .unwrap();
    let movie = t
        .app
        .upsert_catalog_item(mk("movie", "Heat"))
        .await
        .unwrap();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let tk = Some(token.as_str());
    let (_, _, ben) = call(
        &app,
        "POST",
        "/api/v1/companions",
        tk,
        Some(serde_json::json!({"name": "Ben"})),
    )
    .await;
    let play = serde_json::json!({
        "started_at": "2025-12-01T19:00:00Z",
        "players": [{"companion_id": ben["id"], "score": 42, "winner": true}],
        "client_op_id": "01900000-0000-7000-8000-0000000000b1",
    });
    let uri = format!("/api/v1/items/{}/plays", game.id.0);
    let (st, _, first) = call(&app, "POST", &uri, tk, Some(play.clone())).await;
    assert_eq!(st, StatusCode::OK, "{first}");
    let (st, _, again) = call(&app, "POST", &uri, tk, Some(play)).await;
    assert_eq!(st, StatusCode::OK, "{again}");
    assert_eq!(
        first["id"], again["id"],
        "the same event answers the replay"
    );
    assert_eq!(
        first["progress"], again["progress"],
        "naming the same session"
    );
    let (_, _, sessions) = call(&app, "GET", "/api/v1/sessions", tk, None).await;
    assert_eq!(sessions.as_array().unwrap().len(), 1, "{sessions}");

    let watched = serde_json::json!({
        "progress": {"type": "finished"},
        "occurred_at": "2025-12-02T19:00:00Z",
        "with": [ben["id"]],
        "client_op_id": "01900000-0000-7000-8000-0000000000b2",
    });
    let uri = format!("/api/v1/items/{}/progress", movie.id.0);
    let (st, _, first) = call(&app, "POST", &uri, tk, Some(watched.clone())).await;
    assert_eq!(st, StatusCode::OK, "{first}");
    let (st, _, again) = call(&app, "POST", &uri, tk, Some(watched)).await;
    assert_eq!(st, StatusCode::OK, "{again}");
    assert_eq!(first["id"], again["id"]);
    let (_, _, sessions) = call(&app, "GET", "/api/v1/sessions", tk, None).await;
    assert_eq!(sessions.as_array().unwrap().len(), 2, "{sessions}");
}

#[tokio::test]
async fn progress_and_plays_refuse_the_future_and_oversized_fields() {
    // UX1-5: the test clock stands at 2026-01-01T00:00:00Z; five minutes of clock skew pass
    let (s, t) = state();
    let mk = |kind: &str, title: &str| app::NewCatalogItem {
        kind: domain::kind::KindId::new(kind).unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2017),
        description: None,
        external_ids: vec![domain::catalog::ExternalId::new("test", title)],
        attributes: serde_json::json!({}),
        cover_url: None,
        popularity: None,
    };
    let game = t
        .app
        .upsert_catalog_item(mk("board_game", "Harbour"))
        .await
        .unwrap();
    let movie = t
        .app
        .upsert_catalog_item(mk("movie", "Heat"))
        .await
        .unwrap();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let tk = Some(token.as_str());
    let progress = format!("/api/v1/items/{}/progress", movie.id.0);
    let plays = format!("/api/v1/items/{}/plays", game.id.0);
    let watched =
        |at: &str| serde_json::json!({"progress": {"type": "finished"}, "occurred_at": at});
    for (uri, body, field) in [
        (&progress, watched("2026-01-01T00:06:00Z"), "occurred_at"),
        (
            &plays,
            serde_json::json!({"started_at": "2026-01-02T00:00:00Z"}),
            "started_at",
        ),
        (
            &plays,
            serde_json::json!({"duration_minutes": 10_081}),
            "duration_minutes",
        ),
        (
            &plays,
            serde_json::json!({"location": "x".repeat(201)}),
            "location",
        ),
        (
            &plays,
            serde_json::json!({"notes": "x".repeat(20_001)}),
            "notes",
        ),
    ] {
        let (st, _, err) = call(&app, "POST", uri, tk, Some(body)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{field}: {err}");
        assert_eq!(err["code"], "invalid", "{err}");
        assert!(err["message"].as_str().unwrap().contains(field), "{err}");
    }
    let (st, _, ok) = call(
        &app,
        "POST",
        &progress,
        tk,
        Some(watched("2026-01-01T00:04:00Z")),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "within the clock skew: {ok}");
    let fine = serde_json::json!({
        "duration_minutes": 10_080, "location": "x".repeat(200), "notes": "x".repeat(20_000)
    });
    let (st, _, ok) = call(&app, "POST", &plays, tk, Some(fine)).await;
    assert_eq!(st, StatusCode::OK, "at the limits: {ok}");
}

/// A search backend that is down.
struct SearchDown;

#[async_trait::async_trait]
impl app::ports::SearchIndex for SearchDown {
    async fn upsert(
        &self,
        _: &str,
        _: Vec<app::ports::SearchDoc>,
    ) -> Result<(), app::ports::PortError> {
        Err(app::ports::PortError("down".into()))
    }
    async fn delete(&self, _: &str, _: Vec<String>) -> Result<(), app::ports::PortError> {
        Err(app::ports::PortError("down".into()))
    }
    async fn search(
        &self,
        _: app::ports::SearchQuery,
    ) -> Result<Vec<app::ports::SearchHit>, app::ports::PortError> {
        Err(app::ports::PortError("down".into()))
    }
}

#[tokio::test]
async fn the_library_text_filter_says_when_search_is_down() {
    // UX1-10: not "nothing matches", but 503 so the page offers Retry
    let (mut s, _t) = state();
    s.search = Arc::new(SearchDown);
    let app = api::router(s);
    let token = signed_up(&app).await;
    let tk = Some(token.as_str());
    let q = serde_json::json!({"q": "harbour"});
    let (st, _, err) = call(&app, "POST", "/api/v1/library/query", tk, Some(q)).await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "{err}");
    assert_eq!(err["code"], "search_unavailable");
    let (st, _, _) = call(
        &app,
        "POST",
        "/api/v1/library/query",
        tk,
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "without text the search is not asked");
}

async fn a_movie(t: &testkit::TestEnv, title: &str) -> domain::catalog::CatalogItem {
    t.app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(1995),
            description: None,
            external_ids: vec![],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap()
}

/// OT-4, OT-7 (DEC-553; review OT2A-16): the item page of an item that is not in the library
/// carries its first step and a count of zero; the first log adds it and counts once.
#[tokio::test]
async fn the_item_page_has_a_first_step_and_the_count_without_an_entry() {
    let (s, t) = state();
    let item = a_movie(&t, "Heat").await;
    let app = api::router(s);
    let token = signed_up(&app).await;
    let page = format!("/api/v1/items/{}/page", item.id.0);
    let (status, _, p) = call(&app, "GET", &page, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert!(p["entry"].is_null(), "{p}");
    assert!(p["summary"].is_null(), "{p}");
    assert_eq!(p["times"], 0);
    assert_eq!(p["next_step"]["action"], "finish");
    assert_eq!(p["next_step"]["again"], false);
    assert_eq!(p["next_step"]["label_key"], "next-step-watched");

    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/api/v1/items/{}/progress", item.id.0),
        Some(&token),
        Some(serde_json::json!({"progress": {"type": "finished"}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, _, p) = call(&app, "GET", &page, Some(&token), None).await;
    assert_eq!(p["entry"]["status"], "completed");
    assert_eq!(p["times"], 1);
    assert_eq!(p["next_step"]["again"], true);
    assert_eq!(p["summary"]["finished"], 1);
}

/// Review OT2A, "not verified": a stored item whose kind this deployment no longer has (the
/// kind was switched off, D-10) still has a page outside the library, as before the first
/// step was added: no step, no count, no error.
#[tokio::test]
async fn the_item_page_of_a_switched_off_kind_still_answers() {
    let (s, t) = state();
    let mut item = a_movie(&t, "Hoerspiel").await;
    item.kind = domain::kind::KindId::new("radio_play").unwrap();
    t.app.repos.catalog.upsert(&item).await.unwrap();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let page = format!("/api/v1/items/{}/page", item.id.0);
    let (status, _, p) = call(&app, "GET", &page, Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{p}");
    assert_eq!(p["item"]["kind"], "radio_play");
    assert!(p["entry"].is_null(), "{p}");
    assert!(p["next_step"].is_null(), "{p}");
    assert_eq!(p["times"], 0);
}

/// ot2e (DEC-587): the item page of a show carries its rewatch run (running, then stopped)
/// and the day of the pass the count names; the run is started with a plain progress log
/// and stopped with the status write.
#[tokio::test]
async fn the_item_page_carries_the_rewatch_run() {
    let (s, t) = state();
    let mut item = a_movie(&t, "Harbour Watch").await;
    item.kind = domain::kind::KindId::new("show").unwrap();
    item.attributes = serde_json::json!({"seasons": [{"number": 1, "episode_count": 3}]});
    t.app.repos.catalog.upsert(&item).await.unwrap();
    let app = api::router(s);
    let token = signed_up(&app).await;
    let page = format!("/api/v1/items/{}/page", item.id.0);
    let progress = format!("/api/v1/items/{}/progress", item.id.0);
    let ep =
        |e: u32| serde_json::json!({"progress": {"type": "episode", "season": 1, "episode": e}});
    for e in 1..=3 {
        t.clock.advance(chrono::Duration::days(1));
        let (status, _, _) = call(&app, "POST", &progress, Some(&token), Some(ep(e))).await;
        assert_eq!(status, StatusCode::OK);
    }
    let status_url = format!("/api/v1/library/{}/status", item.id.0);
    let completed = serde_json::json!({"status": "completed"});
    let (status, _, b) = call(
        &app,
        "PUT",
        &status_url,
        Some(&token),
        Some(completed.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{b}");
    let (_, _, p) = call(&app, "GET", &page, Some(&token), None).await;
    assert_eq!(p["times"], 1);
    assert!(p["summary"]["rewatch"].is_null(), "{p}");
    let third_day = p["progress"][2]["occurred_at"].clone();
    assert_eq!(p["times_at"], third_day);

    // one favourite episode again two days later: the day of the pass stays (OT2A-7)
    t.clock.advance(chrono::Duration::days(2));
    call(&app, "POST", &progress, Some(&token), Some(ep(1))).await;
    let (_, _, p) = call(&app, "GET", &page, Some(&token), None).await;
    assert_eq!(p["times_at"], third_day);
    assert_ne!(p["summary"]["last_at"], third_day);

    t.clock.advance(chrono::Duration::days(1));
    let start = serde_json::json!({"progress": {"type": "rewatch"}});
    let (status, _, started) = call(&app, "POST", &progress, Some(&token), Some(start)).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    // the answer of a log names the status it moved the entry to
    assert_eq!(started["status"], "in_progress");
    t.clock.advance(chrono::Duration::minutes(1));
    let (_, _, first) = call(&app, "POST", &progress, Some(&token), Some(ep(1))).await;
    assert!(first.get("status").is_none(), "{first}");
    let (_, _, p) = call(&app, "GET", &page, Some(&token), None).await;
    assert_eq!(p["entry"]["status"], "in_progress");
    assert_eq!(
        p["summary"]["rewatch"],
        serde_json::json!({"start": started["id"], "done": 1, "running": true})
    );
    assert_eq!(p["summary"]["episodes_total"], 3);
    assert_eq!(p["summary"]["episodes_watched"], 3);
    assert_eq!(p["times"], 1);
    assert_eq!(p["next_step"]["action"], "episode");
    assert_eq!(p["next_step"]["episode"], 2);
    let third = p["summary"]["fraction"].as_f64().unwrap();
    assert!((third - 1.0 / 3.0).abs() < 0.01, "{third}");

    let (status, _, _) = call(&app, "PUT", &status_url, Some(&token), Some(completed)).await;
    assert_eq!(status, StatusCode::OK);
    let (_, _, p) = call(&app, "GET", &page, Some(&token), None).await;
    assert_eq!(p["entry"]["status"], "completed");
    assert_eq!(p["summary"]["rewatch"]["running"], false);
    assert_eq!(p["summary"]["rewatch"]["done"], 1);
    assert_eq!(p["times"], 1);
    assert_eq!(p["next_step"]["action"], "caught_up");
}

/// A show with one season of `episodes` episodes and a TMDB id.
async fn a_show(t: &testkit::TestEnv, title: &str, tmdb: &str, episodes: u32) -> String {
    let item = t
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("show").unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(2019),
            description: None,
            external_ids: vec![domain::catalog::ExternalId::new("tmdb", tmdb)],
            attributes: serde_json::json!({"seasons": [{"number": 1, "episode_count": episodes}]}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    item.id.0.to_string()
}

/// Log `steps` on an item one minute apart: a number is that episode of season 1, a word is
/// a marker (`rewatch`, `rewatch_stop`) or `completed` (the status set by hand). Answers
/// the events.
async fn logged(
    app: &axum::Router,
    t: &testkit::TestEnv,
    token: &str,
    item: &str,
    steps: &[&str],
) -> Vec<serde_json::Value> {
    let mut events = Vec::new();
    for step in steps {
        t.clock.advance(chrono::Duration::minutes(1));
        if *step == "completed" {
            let uri = format!("/api/v1/library/{item}/status");
            let body = serde_json::json!({"status": "completed"});
            let (status, _, b) = call(app, "PUT", &uri, Some(token), Some(body)).await;
            assert_eq!(status, StatusCode::OK, "{b}");
            continue;
        }
        let progress = match step.parse::<u32>() {
            Ok(e) => serde_json::json!({"type": "episode", "season": 1, "episode": e}),
            Err(_) => serde_json::json!({"type": step}),
        };
        let uri = format!("/api/v1/items/{item}/progress");
        let body = serde_json::json!({"progress": progress});
        let (status, _, event) = call(app, "POST", &uri, Some(token), Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{step}: {event}");
        events.push(event);
    }
    events
}

/// Review OT2E-7 a, OT2E-8, OT2E-2 c: the export of a library with rewatch runs (two stopped
/// ones in a row, a finished one, one that is running) and with undone logs is a file the
/// import takes, and the other account reads the same runs.
#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn export_then_import_keeps_rewatch_runs_and_leaves_undone_logs_out() {
    let t = testkit::TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let importer = Arc::new(importers::Importer::new(
        t.app.clone(),
        Arc::new(importers::MemImportStore::default()),
        Arc::new(testkit::fakes::MemBlobs::default()),
        t.jobs.clone(),
    ));
    let (mut s, _) = state();
    s.app = t.app.clone();
    s.auth = auth::Auth::new(
        t.app.clone(),
        Arc::new(auth::MemAuthStore::default()),
        auth::AuthSettings {
            signup: auth::SignupMode::Open,
            ..auth::AuthSettings::default()
        },
    );
    s.importer = Some(importer.clone());
    let stopped = a_show(&t, "Harbour Watch", "101", 3).await;
    let finished = a_show(&t, "Tideline", "102", 2).await;
    let running = a_show(&t, "Saltmarsh", "103", 2).await;
    let app = api::router(s);
    let anna = signed_up(&app).await;
    // two runs stopped in a row, one episode each
    let story = [
        "1",
        "2",
        "3",
        "completed",
        "rewatch",
        "1",
        "rewatch_stop",
        "rewatch",
        "2",
        "rewatch_stop",
    ];
    logged(&app, &t, &anna, &stopped, &story).await;
    // a whole run, then one more view that is taken back (an undo event is stored)
    let story = ["1", "2", "completed", "rewatch", "1", "2", "1"];
    let events = logged(&app, &t, &anna, &finished, &story).await;
    let undo = format!(
        "/api/v1/progress/{}/undo",
        events.last().unwrap()["id"].as_str().unwrap()
    );
    let (status, _, b) = call(&app, "POST", &undo, Some(&anna), None).await;
    assert_eq!(status, StatusCode::OK, "{b}");
    // a run that is running when the file is written
    logged(
        &app,
        &t,
        &anna,
        &running,
        &["1", "2", "completed", "rewatch", "1"],
    )
    .await;

    let read = async |token: &str| {
        let mut out = Vec::new();
        for item in [&stopped, &finished, &running] {
            let uri = format!("/api/v1/items/{item}/page");
            let (_, _, p) = call(&app, "GET", &uri, Some(token), None).await;
            let run = &p["summary"]["rewatch"];
            out.push(serde_json::json!({
                "status": p["entry"]["status"], "times": p["times"],
                "done": run["done"], "running": run["running"],
                "views": p["summary"]["episode_views"], "events": p["progress"].as_array().unwrap().len(),
            }));
        }
        out
    };
    let before = read(&anna).await;
    assert_eq!(
        before,
        [
            serde_json::json!({"status": "completed", "times": 1, "done": 1, "running": false, "views": 5, "events": 9}),
            serde_json::json!({"status": "completed", "times": 2, "done": null, "running": null, "views": 4, "events": 5}),
            serde_json::json!({"status": "in_progress", "times": 1, "done": 1, "running": true, "views": 3, "events": 4}),
        ]
    );

    let req = Request::builder()
        .uri("/api/v1/export")
        .header(header::AUTHORIZATION, format!("Bearer {anna}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let file = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(file.to_vec()).unwrap();
    assert!(!text.contains("\"undo\""), "no undo events in the file");
    assert!(text.contains("\"rewatch_stop\""));

    t.clock.advance(chrono::Duration::days(3));
    let (_, _, ben) = call(
        &app,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        Some(serde_json::json!({"username": "ben", "display_name": "Ben", "password": "correct horse battery"})),
    )
    .await;
    let ben = ben["token"].as_str().unwrap().to_string();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/imports?format=generic-jsonl")
        .header(header::AUTHORIZATION, format!("Bearer {ben}"))
        .body(Body::from(text))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let started: serde_json::Value =
        serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let id = uuid::Uuid::parse_str(started["id"].as_str().unwrap()).unwrap();
    importer.run(id).await.unwrap();
    let uri = format!("/api/v1/imports/{id}");
    let (_, _, report) = call(&app, "GET", &uri, Some(&ben), None).await;
    assert_eq!(report["state"], "done", "{report}");
    assert_eq!(report["matched"], 3, "{report}");
    assert_eq!(read(&ben).await, before, "{report}");
}

/// Review OT2E-1 and OT2E-12 over REST: a start that starts nothing is a conflict, dates a
/// run cannot take are `invalid`, and "watched with" logs no session for a refused view.
#[tokio::test]
async fn a_rewatch_run_refuses_what_it_could_not_count() {
    let (s, t) = state();
    let show = a_show(&t, "Harbour Watch", "101", 3).await;
    let app = api::router(s);
    let token = signed_up(&app).await;
    let tk = Some(token.as_str());
    let uri = format!("/api/v1/items/{show}/progress");
    let post = async |body: serde_json::Value| {
        let (status, _, b) = call(&app, "POST", &uri, tk, Some(body)).await;
        (status, b["code"].as_str().unwrap_or("").to_string())
    };
    let start = serde_json::json!({"progress": {"type": "rewatch"}});
    // not in the library, then In progress: nothing to start, and nothing is added
    assert_eq!(post(start.clone()).await.0, StatusCode::CONFLICT);
    let (_, _, lib) = call(&app, "GET", "/api/v1/library", tk, None).await;
    assert_eq!(lib["items"].as_array().unwrap().len(), 0);
    logged(&app, &t, &token, &show, &["1", "2", "3"]).await;
    assert_eq!(post(start.clone()).await.0, StatusCode::CONFLICT);
    assert_eq!(
        post(serde_json::json!({"progress": {"type": "rewatch_stop"}}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    logged(&app, &t, &token, &show, &["completed"]).await;
    t.clock.advance(chrono::Duration::days(2));
    let now = t.clock.now_for_tests();
    let earlier = (now - chrono::Duration::days(30)).to_rfc3339();
    // a start dated before the first run's views
    let dated = serde_json::json!({"progress": {"type": "rewatch"}, "occurred_at": earlier});
    assert_eq!(
        post(dated).await,
        (StatusCode::UNPROCESSABLE_ENTITY, "invalid".to_string())
    );
    assert_eq!(post(start).await.0, StatusCode::OK);
    // a view of yesterday while the run runs, alone and "watched with"
    let (_, _, ben) = call(
        &app,
        "POST",
        "/api/v1/companions",
        tk,
        Some(serde_json::json!({"name": "Ben"})),
    )
    .await;
    let yesterday = (now - chrono::Duration::days(1)).to_rfc3339();
    let view = serde_json::json!({
        "progress": {"type": "episode", "season": 1, "episode": 1}, "occurred_at": yesterday,
    });
    assert_eq!(
        post(view.clone()).await,
        (StatusCode::UNPROCESSABLE_ENTITY, "invalid".to_string())
    );
    let mut with = view;
    with["with"] = serde_json::json!([ben["id"]]);
    assert_eq!(post(with).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, _, sessions) = call(&app, "GET", "/api/v1/sessions", tk, None).await;
    assert_eq!(sessions.as_array().unwrap().len(), 0, "{sessions}");
    let (_, _, p) = call(&app, "GET", &format!("/api/v1/items/{show}/page"), tk, None).await;
    assert_eq!(p["summary"]["rewatch"]["done"], 0);
    assert_eq!(p["progress"].as_array().unwrap().len(), 4);
}

/// OT-3: the search answer says which local hits are in the searcher's library (the palette
/// leads with them), and nobody else's entry counts.
#[tokio::test]
async fn search_names_the_hits_in_the_searchers_library() {
    let (s, t) = state();
    let mk = |title: &str| app::NewCatalogItem {
        kind: domain::kind::KindId::new("movie").unwrap(),
        title: title.into(),
        original_title: None,
        year: Some(2001),
        description: None,
        external_ids: vec![domain::catalog::ExternalId::new("test", title)],
        attributes: serde_json::json!({}),
        cover_url: None,
        popularity: None,
    };
    let mut ids = Vec::new();
    for title in ["Heat", "Heat 2", "Heatwave"] {
        let item = t.app.upsert_catalog_item(mk(title)).await.unwrap();
        s.search
            .upsert(
                "catalog",
                vec![app::ports::SearchDoc {
                    id: item.id.0.to_string(),
                    owner: None,
                    fields: serde_json::json!({"title": title, "kind": "movie"}),
                }],
            )
            .await
            .unwrap();
        ids.push(item.id.0.to_string());
    }
    let app = api::router(s);
    let token = signed_up(&app).await;
    let tk = Some(token.as_str());
    let (st, _, add) = call(
        &app,
        "POST",
        "/api/v1/library",
        tk,
        Some(serde_json::json!({"item_id": ids[1]})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{add}");
    let (st, _, found) = call(&app, "GET", "/api/v1/search?q=heat", tk, None).await;
    assert_eq!(st, StatusCode::OK, "{found}");
    assert_eq!(found["items"].as_array().unwrap().len(), 3, "{found}");
    assert_eq!(found["in_library"], serde_json::json!([ids[1]]), "{found}");
    // another account has no entry
    let (_, _, other) = call(
        &app,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        Some(serde_json::json!({
            "username": "ben", "display_name": "Ben", "password": "correct horse battery"
        })),
    )
    .await;
    let ben = other["token"].as_str().expect("a token").to_string();
    let (_, _, found) = call(&app, "GET", "/api/v1/search?q=heat", Some(&ben), None).await;
    assert_eq!(found["in_library"], serde_json::json!([]), "{found}");
}

/// An integration store whose every call fails with the text of a database error.
struct BrokenIntegrationStore;

const STORE_FAILURE: &str = "error returned from database: relation \"integrations\" does not \
                             exist at postgres://app@db.internal:5432/app";

#[async_trait::async_trait]
impl integrations::IntegrationStore for BrokenIntegrationStore {
    async fn insert(&self, _: &integrations::Integration) -> Result<(), app::ports::PortError> {
        Err(app::ports::PortError(STORE_FAILURE.into()))
    }
    async fn by_token_hash(
        &self,
        _: &str,
    ) -> Result<Option<integrations::Integration>, app::ports::PortError> {
        Err(app::ports::PortError(STORE_FAILURE.into()))
    }
    async fn by_user(
        &self,
        _: domain::ids::UserId,
    ) -> Result<Vec<integrations::Integration>, app::ports::PortError> {
        Err(app::ports::PortError(STORE_FAILURE.into()))
    }
    async fn delete(&self, _: uuid::Uuid) -> Result<(), app::ports::PortError> {
        Err(app::ports::PortError(STORE_FAILURE.into()))
    }
    async fn touch(
        &self,
        _: uuid::Uuid,
        _: &integrations::CallRecord,
    ) -> Result<(), app::ports::PortError> {
        Err(app::ports::PortError(STORE_FAILURE.into()))
    }
}

/// sec0 (SEC-9): a webhook sender whose call meets a storage failure gets the generic 500,
/// not the store's message; the same for the member's own integration routes.
#[tokio::test]
async fn a_storage_failure_behind_the_webhook_answers_with_the_generic_text() {
    let (mut s, t) = state();
    s.integrations = Some(Arc::new(integrations::Integrations::new(
        t.app.clone(),
        Arc::new(BrokenIntegrationStore),
    )));
    let app = api::router(s);
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/hooks/watched")
        .header(header::AUTHORIZATION, "Bearer itk_anything")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({"kind": "movie", "title": "Heat", "year": 1995}).to_string(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["code"], "internal", "{text}");
    assert_eq!(json["message"], "internal error", "{text}");
    for leaked in ["relation", "database", "postgres", "db.internal"] {
        assert!(!text.contains(leaked), "{leaked} in the answer: {text}");
    }

    let token = signed_up(&app).await;
    let (status, _, body) = call(&app, "GET", "/api/v1/integrations", Some(&token), None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["message"], "internal error", "{body}");
}

/// A notification store whose every call fails with the text of a database error.
struct BrokenNotifyStore;

const NOTIFY_FAILURE: &str = "notifications db: error returned from database: relation \
                              \"notify_channels\" does not exist at postgres://app@db.internal";

/// Every method of the store, failing.
macro_rules! broken {
    ($($name:ident($($arg:ty),*) -> $out:ty;)*) => {
        #[async_trait::async_trait]
        impl notify::NotifyStore for BrokenNotifyStore {
            $(async fn $name(&self $(, _: $arg)*) -> notify::store::StoreResult<$out> {
                Err(app::ports::PortError(NOTIFY_FAILURE.into()))
            })*
        }
    };
}

type At = chrono::DateTime<chrono::Utc>;
type User = domain::ids::UserId;

broken! {
    admins() -> Vec<User>;
    user_locale(User) -> Option<String>;
    channels(User) -> Vec<notify::model::Channel>;
    channel(uuid::Uuid) -> Option<notify::model::Channel>;
    put_channel(&notify::model::Channel) -> ();
    delete_channel(uuid::Uuid) -> ();
    subscriptions(User) -> Vec<notify::model::Subscription>;
    defaults() -> Vec<notify::model::Subscription>;
    subscription(uuid::Uuid) -> Option<notify::model::Subscription>;
    put_subscription(&notify::model::Subscription) -> ();
    delete_subscription(uuid::Uuid) -> ();
    insert_inbox(&notify::model::InboxItem) -> bool;
    inbox(User, u32) -> Vec<notify::model::InboxItem>;
    count_unread(User) -> u32;
    mark_read(User, uuid::Uuid, At) -> ();
    mark_all_read(User, At) -> u32;
    insert_delivery(&notify::model::Delivery) -> bool;
    delivery(uuid::Uuid) -> Option<notify::model::Delivery>;
    update_delivery(&notify::model::Delivery) -> ();
    deliveries(User, u32) -> Vec<notify::model::Delivery>;
    latest_deliveries(User) -> Vec<notify::model::Delivery>;
    due_digest(uuid::Uuid, At) -> Vec<notify::model::Delivery>;
    count_since(uuid::Uuid, &str, At) -> u32;
    template_overrides() -> notify::template::Overrides;
    set_template_override(&str, &str, Option<&str>) -> ();
}

/// Review of sec0 (SEC0-1): the notifications routes answer a storage failure with the
/// generic 500 as well; the store's text reaches no member, whatever the route.
#[tokio::test]
async fn a_storage_failure_behind_the_notifications_routes_answers_with_the_generic_text() {
    let (mut s, t) = state();
    let engine = Arc::new(notify::Engine::new(
        Arc::new(BrokenNotifyStore),
        t.jobs.clone(),
        t.events.clone(),
        t.clock.clone(),
        t.ids.clone(),
        "Shelf",
        "en",
        notify::template::Overrides::new(),
    ));
    let deliverer = Arc::new(notify::Deliverer::new(
        engine.clone(),
        Arc::new(notify::channels::Channels::default()),
        t.clock.clone(),
        "https://shelf.example.org",
        3,
    ));
    s.notify = Some(api::NotifyApi {
        engine,
        deliverer,
        web_push_key: None,
    });
    let app = api::router(s);
    let token = signed_up(&app).await;
    let channel = serde_json::json!({
        "name": "Phone",
        "config": {"kind": "ntfy", "server": "https://ntfy.sh", "topic": "t", "token": null},
        "enabled": true,
    });
    let rule = serde_json::json!({"event": "social.link_request", "targets": {"type": "all"}});
    let none = uuid::Uuid::nil();
    let routes = [
        ("GET", "/api/v1/notifications/inbox".to_string(), None),
        (
            "POST",
            "/api/v1/notifications/inbox/read-all".to_string(),
            None,
        ),
        (
            "POST",
            format!("/api/v1/notifications/inbox/{none}/read"),
            None,
        ),
        ("GET", "/api/v1/notifications/channels".to_string(), None),
        (
            "POST",
            "/api/v1/notifications/channels".to_string(),
            Some(channel),
        ),
        (
            "DELETE",
            format!("/api/v1/notifications/channels/{none}"),
            None,
        ),
        (
            "GET",
            "/api/v1/notifications/subscriptions".to_string(),
            None,
        ),
        (
            "PUT",
            "/api/v1/notifications/subscriptions".to_string(),
            Some(rule),
        ),
        ("GET", "/api/v1/notifications/deliveries".to_string(), None),
    ];
    for (method, path, body) in routes {
        let (status, _, body) = call(&app, method, &path, Some(&token), body).await;
        let text = body.to_string();
        assert_eq!(
            status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "{method} {path}: {text}"
        );
        assert_eq!(body["code"], "internal", "{method} {path}: {text}");
        assert_eq!(body["message"], "internal error", "{method} {path}: {text}");
        for leaked in ["relation", "database", "notify_channels", "db.internal"] {
            assert!(
                !text.contains(leaked),
                "{method} {path}: {leaked} in {text}"
            );
        }
    }
}

// ---------------------------------------------------------------- v1-manual

/// A source that answers for movies in the namespace `src`.
struct MovieSource(catalog_core::ConnectorInfo);

impl MovieSource {
    fn new() -> Self {
        Self(catalog_core::ConnectorInfo {
            id: "src".into(),
            name: "Source".into(),
            kinds: vec!["movie".into()],
            capabilities: vec![catalog_core::Capability::Details],
            rate_limit: catalog_core::RateLimit {
                per_second: 1,
                burst: 1,
                max_concurrency: 1,
            },
            auth: catalog_core::Auth::None,
            attribution: catalog_core::Attribution {
                text: String::new(),
                url: String::new(),
                licence: String::new(),
            },
            hosts: vec![],
            cache_ttl_secs: 0,
            contact: catalog_core::ContactUse::None,
        })
    }
}

impl catalog_core::Connector for MovieSource {
    fn info(&self) -> &catalog_core::ConnectorInfo {
        &self.0
    }
}

/// `POST /items`: 201 with the item, 422 with the field, 401 without a session; the item is
/// found by its title and its page offers "Edit details" and queues no refresh.
#[tokio::test]
async fn an_item_is_entered_by_hand_over_http() {
    let (s, t) = state();
    let app = api::router(s);
    let tk = signed_up(&app).await;
    let tk = Some(tk.as_str());
    let (status, _, body) = call(
        &app,
        "POST",
        "/api/v1/items",
        None,
        Some(serde_json::json!({"kind": "board_game", "title": "Marsh"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");

    let (status, _, item) = call(
        &app,
        "POST",
        "/api/v1/items",
        tk,
        Some(serde_json::json!({
            "kind": "board_game", "title": " Zine of the Marsh ", "year": 2019,
            "cover_url": "https://img.example/box.jpg"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{item}");
    assert_eq!(item["title"], "Zine of the Marsh");
    assert_eq!(item["kind"], "board_game");
    assert_eq!(item["year"], 2019);
    assert_eq!(item["state"], "full");
    assert_eq!(item["external_ids"], serde_json::json!([]));
    assert_eq!(item["cover"]["url"], "https://img.example/box.jpg");
    assert_eq!(
        item["provenance"],
        serde_json::json!({"cover_url": "user", "title": "user", "year": "user"})
    );
    let id = item["id"].as_str().unwrap().to_string();

    refused_entries_name_their_field(&app, tk).await;

    // found by its title before a second one is entered
    let (status, _, found) = call(
        &app,
        "GET",
        "/api/v1/items/titled?kind=board_game&title=%20zine%20OF%20the%20marsh",
        tk,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(found.as_array().unwrap().len(), 1);
    assert_eq!(found[0]["id"], id.as_str());
    let (_, _, none) = call(
        &app,
        "GET",
        "/api/v1/items/titled?kind=book&title=Zine%20of%20the%20Marsh",
        tk,
        None,
    )
    .await;
    assert_eq!(none, serde_json::json!([]), "another kind");
    let (status, _, _) = call(
        &app,
        "GET",
        "/api/v1/items/titled?kind=board_game&title=x",
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // its page and the plain read queue no refresh (B2), and the page offers "Edit details"
    let (status, _, page) = call(&app, "GET", &format!("/api/v1/items/{id}/page"), tk, None).await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert_eq!(page["details_editable"], true);
    let (status, _, _) = call(&app, "GET", &format!("/api/v1/items/{id}"), tk, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(t.jobs.jobs().len(), 0, "nobody could answer a refresh");
}

/// `PATCH /items/{id}`: the three details of an item no source answers for; 409 for an item
/// of a registered source, whose page does not offer the edit and still queues its refresh.
#[tokio::test]
async fn details_are_edited_over_http_unless_a_source_answers() {
    let (mut s, t) = state();
    let mut registry = Registry::default();
    registry.register(Arc::new(MovieSource::new()));
    s.registry = registry;
    let app = api::router(s);
    let tk = signed_up(&app).await;
    let tk = Some(tk.as_str());
    let (_, _, item) = call(
        &app,
        "POST",
        "/api/v1/items",
        tk,
        Some(serde_json::json!({"kind": "movie", "title": "Home video", "year": 2001})),
    )
    .await;
    let id = item["id"].as_str().unwrap().to_string();
    let path = format!("/api/v1/items/{id}");

    let (status, _, _) = call(
        &app,
        "PATCH",
        &path,
        None,
        Some(serde_json::json!({"title": "X"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, edited) = call(
        &app,
        "PATCH",
        &path,
        tk,
        Some(serde_json::json!({
            "title": "Home video, summer", "year": 2001,
            "cover_url": "https://img.example/summer.jpg"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["title"], "Home video, summer");
    assert_eq!(edited["cover"]["url"], "https://img.example/summer.jpg");
    assert_eq!(edited["year"], 2001);
    // year and cover left out: none
    let (status, _, edited) = call(
        &app,
        "PATCH",
        &path,
        tk,
        Some(serde_json::json!({"title": "Home video, summer"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["year"], serde_json::Value::Null);
    assert_eq!(edited["cover"], serde_json::Value::Null);
    let (status, _, e) = call(
        &app,
        "PATCH",
        &path,
        tk,
        Some(serde_json::json!({"title": "A", "cover_url": "ftp://img.example/a"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{e}");
    assert_eq!(e["field"], "cover_url");
    let (status, _, e) = call(
        &app,
        "PATCH",
        &format!("/api/v1/items/{}", uuid::Uuid::from_u128(77)),
        tk,
        Some(serde_json::json!({"title": "A"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{e}");

    a_sourced_item_refuses_the_edit(&app, &t, tk).await;
}

/// Review MAN-2: a second item of a kind and title is a choice, not an accident. `POST /items`
/// answers 409 `conflict` on the field `title`, naming the items, unless the body says
/// `allow_duplicate`; the check comes after the field checks.
#[tokio::test]
async fn a_title_that_is_there_is_refused_unless_a_second_one_is_asked_for() {
    let (s, _t) = state();
    let app = api::router(s);
    let tk = signed_up(&app).await;
    let tk = Some(tk.as_str());
    let new = |title: &str, allow: Option<bool>| {
        let mut b = serde_json::json!({"kind": "board_game", "title": title});
        if let Some(a) = allow {
            b["allow_duplicate"] = a.into();
        }
        b
    };
    let (status, _, first) =
        call(&app, "POST", "/api/v1/items", tk, Some(new("Catan", None))).await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let first_id = first["id"].as_str().unwrap().to_string();

    // the same title but for case and blanks: refused, the items named
    let (status, _, e) = call(
        &app,
        "POST",
        "/api/v1/items",
        tk,
        Some(new(" catan ", None)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{e}");
    assert_eq!(e["code"], "conflict");
    assert_eq!(e["field"], "title");
    assert!(e["message"].as_str().unwrap().contains(&first_id), "{e}");
    let (status, _, e) = call(
        &app,
        "POST",
        "/api/v1/items",
        tk,
        Some(new("Catan", Some(false))),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{e}");
    // a wrong field is still named first
    let mut bad = new("Catan", None);
    bad["year"] = 0.into();
    let (status, _, e) = call(&app, "POST", "/api/v1/items", tk, Some(bad)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{e}");
    assert_eq!(e["field"], "year");
    // another kind of that title is no conflict
    let (status, _, e) = call(
        &app,
        "POST",
        "/api/v1/items",
        tk,
        Some(serde_json::json!({"kind": "book", "title": "Catan"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{e}");

    // asked for: a second one
    let (status, _, second) = call(
        &app,
        "POST",
        "/api/v1/items",
        tk,
        Some(new("Catan", Some(true))),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{second}");
    assert_ne!(second["id"], first["id"]);
    let (_, _, found) = call(
        &app,
        "GET",
        "/api/v1/items/titled?kind=board_game&title=Catan",
        tk,
        None,
    )
    .await;
    assert_eq!(found.as_array().unwrap().len(), 2, "{found}");
}

/// What `POST /items` refuses, each with its field.
async fn refused_entries_name_their_field(app: &axum::Router, tk: Option<&str>) {
    for (body, field) in [
        (
            serde_json::json!({"kind": "board_game", "title": "  "}),
            "title",
        ),
        (
            serde_json::json!({"kind": "board_game", "title": "x".repeat(501)}),
            "title",
        ),
        (
            serde_json::json!({"kind": "board_game", "title": "A", "year": 0}),
            "year",
        ),
        (
            serde_json::json!({"kind": "board_game", "title": "A", "year": 9999}),
            "year",
        ),
        (
            serde_json::json!({"kind": "board_game", "title": "A",
                "cover_url": "http://img.example/a.jpg"}),
            "cover_url",
        ),
        (
            serde_json::json!({"kind": "board_game", "title": "A",
                "cover_url": "https://me:pw@img.example/a.jpg"}),
            "cover_url",
        ),
        (serde_json::json!({"kind": "zine", "title": "A"}), "kind"),
        (serde_json::json!({"kind": "No Kind", "title": "A"}), "kind"),
    ] {
        let (status, _, e) = call(app, "POST", "/api/v1/items", tk, Some(body.clone())).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}: {e}");
        assert_eq!(e["code"], "invalid", "{body}");
        assert_eq!(e["field"], field, "{body}: {e}");
    }
}

/// An item the registered source answers for: no edit, no offer, and its refresh is queued.
async fn a_sourced_item_refuses_the_edit(
    app: &axum::Router,
    t: &testkit::TestEnv,
    tk: Option<&str>,
) {
    // an item the registered source answers for
    let sourced = t
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: None,
            external_ids: vec![domain::catalog::ExternalId::new("src", "949")],
            attributes: serde_json::json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let (status, _, e) = call(
        app,
        "PATCH",
        &format!("/api/v1/items/{}", sourced.id.0),
        tk,
        Some(serde_json::json!({"title": "Mine"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{e}");
    assert_eq!(e["code"], "conflict");
    assert_eq!(t.jobs.jobs().len(), 0, "nothing was queued so far");
    let (_, _, page) = call(
        app,
        "GET",
        &format!("/api/v1/items/{}/page", sourced.id.0),
        tk,
        None,
    )
    .await;
    assert_eq!(page["details_editable"], false);
    assert_eq!(page["item"]["title"], "Heat");
    assert_eq!(t.jobs.jobs().len(), 1, "its source is asked");
}
