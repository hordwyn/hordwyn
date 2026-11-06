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
        "/events",
    ] {
        assert!(doc["paths"].get(p).is_some(), "missing path {p}");
    }
    assert!(doc["components"]["schemas"]["CatalogItemDto"].is_object());
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

/// UX0-4 (design.md 5.2): the lock-out says how long it lasts, in the body and as
/// `Retry-After`, so the client can show the remaining minutes.
#[tokio::test]
async fn lock_out_carries_the_seconds_left() {
    let (s, _) = state();
    let app = api::router(s);
    signed_up(&app).await;
    let wrong = || Some(serde_json::json!({"username": "anna", "password": "wrong password!!"}));
    for _ in 0..auth::MAX_FAILED_SIGN_INS {
        let (status, _, _) = call(&app, "POST", "/api/v1/auth/sign-in", None, wrong()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, headers, err) = call(&app, "POST", "/api/v1/auth/sign-in", None, wrong()).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(err["code"], "too_many_attempts");
    let secs = err["retry_after_secs"].as_u64().expect("retry_after_secs");
    assert!(secs > 0 && secs <= 15 * 60, "{secs}");
    assert_eq!(
        headers[header::RETRY_AFTER].to_str().unwrap(),
        secs.to_string()
    );
    // other errors carry no such field
    let (_, _, err) = call(&app, "GET", "/api/v1/library", None, None).await;
    assert!(err.get("retry_after_secs").is_none(), "{err}");
}
