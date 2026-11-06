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
    let t = testkit::TestEnv::new();
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
    assert_eq!(lib[0]["item"]["title"], "Heat");
    assert_eq!(lib[0]["entry"]["status"], "completed");
    assert_eq!(
        lib[0]["item"]["cover"]["url"],
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
