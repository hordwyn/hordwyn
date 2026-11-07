//! UX phase 2 server side over HTTP: setup wizard, admin-only authorisation, settings with
//! provenance, re-authentication (DEC-191), lock-out guards, data-source credentials and
//! status, "Ask admin", the audit log and the search reason lines.

// End-to-end scenarios on purpose: the steps share one server and database; the short
// names (t: server, a/m: admin/member cookie, r: answer) keep the many calls readable.
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

use serde_json::{Value, json};
use server::{Overrides, Server};
use tower::ServiceExt;

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

struct T {
    app: axum::Router,
    srv: Server,
}

async fn start(extra: &str, env: Vec<(String, String)>) -> Option<T> {
    let db = store_pg::testing::fresh_db_url().await?;
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [brand]\ndir = \"{}\"\n{extra}",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, env).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    Some(T {
        app: srv.base_router(),
        srv,
    })
}

struct Resp {
    status: u16,
    body: Value,
    cookie: Option<String>,
    text: String,
    disposition: Option<String>,
}

async fn call(t: &T, method: &str, uri: &str, cookie: Option<&str>, body: Option<Value>) -> Resp {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(format!("/api/v1{uri}"))
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let body = body.map_or_else(String::new, |b| b.to_string());
    let resp = t
        .app
        .clone()
        .oneshot(req.body(axum::body::Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let disposition = resp
        .headers()
        .get("content-disposition")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .unwrap();
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
        text: String::from_utf8_lossy(&bytes).into_owned(),
        disposition,
    }
}

fn skip() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return true;
    }
    false
}

fn account(name: &str) -> Value {
    json!({"username": name, "display_name": name, "password": "correct horse battery"})
}

/// Expire the fresh proof of every session (instead of waiting an hour).
async fn age_proofs(t: &T) {
    sqlx::query(
        "UPDATE auth_sessions SET reauth_at = now() - interval '2 hours', reauth_slide_at = NULL",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();
}

fn setting<'a>(page: &'a Value, key: &str) -> &'a Value {
    page["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("no setting {key} in {page}"))
}

#[tokio::test]
async fn wizard_then_locked_and_admin_routes_are_admin_only() {
    if skip() {
        return;
    }
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let r = call(&t, "GET", "/setup", None, None).await;
    assert_eq!(
        (r.status, r.body["step"].as_str()),
        (200, Some("account")),
        "{}",
        r.body
    );
    assert_eq!(r.body["hosting"], "home", "default hosting mode (DEC-171)");
    let r = call(&t, "POST", "/setup/account", None, Some(account("root"))).await;
    assert_eq!(r.status, 201, "{}", r.body);
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    assert_eq!(
        call(&t, "POST", "/setup/account", None, Some(account("eve")))
            .await
            .status,
        409
    );
    assert_eq!(
        call(&t, "GET", "/setup", None, None).await.status,
        401,
        "needs the admin now"
    );
    let r = call(&t, "GET", "/setup", a, None).await;
    assert_eq!(r.body["step"], "hosting");
    let r = call(
        &t,
        "POST",
        "/setup/hosting",
        a,
        Some(json!({"hosting": "hosted"})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["step"].as_str()),
        (200, Some("sources")),
        "{}",
        r.body
    );
    let r = call(&t, "POST", "/setup/sources", a, None).await;
    assert_eq!(r.body["step"], "done");
    assert!(
        r.body["checklist"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "invite")
    );
    assert_eq!(
        call(&t, "POST", "/setup/complete", a, None).await.status,
        204
    );
    let r = call(&t, "GET", "/setup", a, None).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (404, Some("setup_done"))
    );
    assert_eq!(
        call(
            &t,
            "POST",
            "/setup/hosting",
            a,
            Some(json!({"hosting": "home"}))
        )
        .await
        .status,
        404
    );

    // an admin invite lets a member in (sign-up is invite-only by default)
    let r = call(
        &t,
        "POST",
        "/admin/invites",
        a,
        Some(json!({"email": "m@example.org"})),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    assert!(r.body["url"].as_str().unwrap().contains("/sign-up?invite="));
    let mut b = account("member");
    b["invite"] = r.body["token"].clone();
    let r = call(&t, "POST", "/auth/sign-up", None, Some(b)).await;
    assert_eq!(r.status, 201, "{}", r.body);
    let member = r.cookie.unwrap();
    let m = Some(member.as_str());
    let routes = [
        ("GET", "/admin/overview"),
        ("GET", "/admin/settings"),
        ("PUT", "/admin/settings"),
        ("GET", "/admin/users"),
        ("GET", "/admin/invites"),
        ("POST", "/admin/invites"),
        ("GET", "/admin/sources"),
        ("PUT", "/admin/sources/tmdb/credential"),
        ("POST", "/admin/sources/tmdb/test"),
        ("PUT", "/admin/sources/tmdb/licence"),
        ("GET", "/admin/storage-jobs"),
        ("POST", "/admin/storage/evict"),
        ("GET", "/admin/notification-defaults"),
        ("GET", "/admin/brand"),
        ("GET", "/admin/security"),
        ("POST", "/admin/secrets/rewrap"),
        ("GET", "/admin/audit"),
        ("GET", "/admin/audit.csv"),
    ];
    let bodies = json!({"values": {}, "fields": {}, "agreement_on_file": true});
    for (method, path) in routes {
        let r = call(&t, method, path, m, Some(bodies.clone())).await;
        assert_eq!(r.status, 403, "{method} {path} as a member: {}", r.text);
        assert_eq!(
            call(&t, method, path, None, Some(bodies.clone()))
                .await
                .status,
            401,
            "{path}"
        );
        assert_ne!(
            call(&t, method, path, a, Some(bodies.clone())).await.status,
            403,
            "{path} as admin"
        );
    }
    let r = call(&t, "GET", "/admin/overview", a, None).await;
    assert_eq!(r.body["health"].as_array().unwrap().len(), 8, "{}", r.body);
    assert_eq!(r.body["hosting"], "hosted");
    assert_eq!(r.body["hosted_banner"], true, "no licence recorded yet");
    let r = call(&t, "GET", "/admin/audit?action=wizard.completed", a, None).await;
    assert_eq!(r.body["entries"].as_array().unwrap().len(), 1, "{}", r.body);
}

#[tokio::test]
async fn settings_provenance_reauth_and_lock_out_guards() {
    if skip() {
        return;
    }
    let env = vec![(
        "APP__STORAGE__HIGH_WATERMARK".to_string(),
        "0.85".to_string(),
    )];
    let t = start("[auth]\nsignup = \"open\"\n", env).await.unwrap();
    let admin = call(&t, "POST", "/auth/sign-up", None, Some(account("root")))
        .await
        .cookie
        .unwrap();
    let a = Some(admin.as_str());
    let ben = call(&t, "POST", "/auth/sign-up", None, Some(account("ben")))
        .await
        .cookie
        .unwrap();

    let page = call(&t, "GET", "/admin/settings", a, None).await.body;
    let signup = setting(&page, "auth.signup");
    assert_eq!(
        (signup["source"].as_str(), signup["editable"].as_bool()),
        (Some("file"), Some(false))
    );
    let high = setting(&page, "storage.high_watermark");
    assert_eq!(high["source"], "env");
    assert_eq!(high["env_var"], "APP__STORAGE__HIGH_WATERMARK");
    assert_eq!(setting(&page, "auth.session_days")["source"], "default");
    assert!(
        page["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w == "public_url_not_https")
    );

    let put = |v: Value| Some(json!({ "values": v }));
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"auth.signup": "closed"})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("fixed_by_config"))
    );
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"storage.max_total_bytes": "1GiB"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    let max = setting(&r.body, "storage.max_total_bytes");
    assert_eq!(
        (max["source"].as_str(), max["value"].as_u64()),
        (Some("db"), Some(1 << 30))
    );
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"storage.low_watermark": 0.95})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (422, Some("invalid_setting")),
        "low above high"
    );
    // secure cookies over plain http would lock everyone out (OD-11)
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"auth.secure_cookies": true})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("secure_cookies_need_https"))
    );

    // DEC-191: sign-in related changes need a fresh proof
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"auth.session_days": 7})),
    )
    .await;
    assert_eq!(r.status, 200, "signing up was the proof: {}", r.body);
    age_proofs(&t).await;
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"auth.session_days": 8})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("reauth_required"))
    );
    let r = call(&t, "GET", "/auth/reauth", a, None).await;
    assert_eq!(
        (r.body["fresh"].as_bool(), r.body["has_password"].as_bool()),
        (Some(false), Some(true))
    );
    let r = call(
        &t,
        "POST",
        "/auth/reauth",
        a,
        Some(json!({"password": "wrong password!"})),
    )
    .await;
    assert_eq!(r.status, 401);
    let r = call(
        &t,
        "POST",
        "/auth/reauth",
        a,
        Some(json!({"password": "correct horse battery"})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["fresh"].as_bool()),
        (200, Some(true)),
        "{}",
        r.body
    );
    let fresh = r.cookie.unwrap();
    assert_ne!(fresh, admin, "session id regenerated");
    assert_eq!(
        call(&t, "GET", "/auth/me", a, None).await.status,
        401,
        "old id is gone"
    );
    let a = Some(fresh.as_str());
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"auth.session_days": 8})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    let r = call(
        &t,
        "POST",
        "/auth/sign-in",
        None,
        Some(json!({"username": "ben", "password": "correct horse battery"})),
    )
    .await;
    let raw = r.text;
    assert_eq!(r.status, 200, "{raw}");

    // lock-out guards (addendum-1 3.3)
    let users = call(&t, "GET", "/admin/users", a, None).await.body;
    let id_of = |name: &str| {
        users
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["username"] == name)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (root_id, ben_id) = (id_of("root"), id_of("ben"));
    let role = |r: &str| Some(json!({ "role": r }));
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{root_id}/role"),
        a,
        role("member"),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("self_role_change"))
    );
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{root_id}/disabled"),
        a,
        Some(json!({"disabled": true})),
    )
    .await;
    assert_eq!(r.body["code"], "self_disable");
    let r = call(&t, "DELETE", &format!("/admin/users/{root_id}"), a, None).await;
    assert_eq!(r.body["code"], "self_delete");
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{ben_id}/role"),
        a,
        role("admin"),
    )
    .await;
    assert_eq!(
        (r.status, r.body["role"].as_str()),
        (200, Some("admin")),
        "{}",
        r.body
    );
    // the acting admin cannot sign in any more (disabled elsewhere): ben is the last admin
    sqlx::query("INSERT INTO user_disabled (user_id, disabled_at) VALUES ($1::uuid, now())")
        .bind(&root_id)
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{ben_id}/role"),
        a,
        role("member"),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("last_admin"))
    );
    sqlx::query("DELETE FROM user_disabled")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{ben_id}/disabled"),
        a,
        Some(json!({"disabled": true})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["disabled"].as_bool()),
        (200, Some(true)),
        "{}",
        r.body
    );
    assert_eq!(
        call(&t, "GET", "/auth/me", Some(ben.as_str()), None)
            .await
            .status,
        401,
        "sessions revoked"
    );
    let r = call(
        &t,
        "POST",
        "/auth/sign-in",
        None,
        Some(json!({"username": "ben", "password": "correct horse battery"})),
    )
    .await;
    assert_eq!(r.body["code"], "account_disabled");

    let audit = call(&t, "GET", "/admin/audit?action=user.", a, None)
        .await
        .body;
    assert_eq!(audit["entries"][0]["action"], "user.disabled", "{audit}");
    assert_eq!(audit["entries"][0]["detail"]["proof"], "password");
    let csv = call(&t, "GET", "/admin/audit.csv", a, None).await.text;
    assert!(
        csv.starts_with("at,actor,action,") && csv.contains("role.changed"),
        "{csv}"
    );
    let r = call(&t, "GET", "/admin/security", a, None).await;
    assert_eq!(r.body["secrets"]["key_source"], "ephemeral", "{}", r.body);
    assert_eq!(r.body["reauth_slide_minutes"], 15);
}

/// UX2-12: every re-auth gate refuses a stale proof over HTTP (DEC-191); the last-admin
/// guard also holds for disable and delete; a refused guard does not slide the window
/// (UX2-11); the audit CSV downloads as a file (UX2-9).
#[tokio::test]
async fn every_reauth_gate_and_the_last_admin_guards_over_http() {
    if skip() {
        return;
    }
    let t = start("", vec![]).await.unwrap();
    let admin = call(&t, "POST", "/auth/sign-up", None, Some(account("root")))
        .await
        .cookie
        .unwrap();
    let a = Some(admin.as_str());
    let inv = call(&t, "POST", "/admin/invites", a, Some(json!({}))).await;
    assert_eq!(inv.status, 201, "{}", inv.text);
    let mut b = account("ben");
    b["invite"] = inv.body["token"].clone();
    assert_eq!(
        call(&t, "POST", "/auth/sign-up", None, Some(b))
            .await
            .status,
        201
    );
    let users = call(&t, "GET", "/admin/users", a, None).await.body;
    let id_of = |name: &str| {
        users
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["username"] == name)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let (root_id, ben_id) = (id_of("root"), id_of("ben"));

    // UX2-11: a refused lock-out guard does not extend the proof
    sqlx::query("UPDATE auth_sessions SET reauth_slide_at = NULL")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{root_id}/role"),
        a,
        Some(json!({"role": "member"})),
    )
    .await;
    assert_eq!(r.body["code"], "self_role_change", "{}", r.text);
    let (slid,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM auth_sessions WHERE reauth_slide_at IS NOT NULL")
            .fetch_one(&t.srv.pool)
            .await
            .unwrap();
    assert_eq!(slid, 0, "the refused change did not slide the window");

    // the last-admin guard on disable and delete: ben is the only admin who can sign in
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{ben_id}/role"),
        a,
        Some(json!({"role": "admin"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    sqlx::query("INSERT INTO user_disabled (user_id, disabled_at) VALUES ($1::uuid, now())")
        .bind(&root_id)
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{ben_id}/disabled"),
        a,
        Some(json!({"disabled": true})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("last_admin")),
        "{}",
        r.text
    );
    let r = call(&t, "DELETE", &format!("/admin/users/{ben_id}"), a, None).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("last_admin")),
        "{}",
        r.text
    );
    sqlx::query("DELETE FROM user_disabled")
        .execute(&t.srv.pool)
        .await
        .unwrap();

    // every gate with a stale proof: 403 reauth_required, nothing changed
    age_proofs(&t).await;
    let put = |v: Value| Some(json!({ "values": v }));
    let gates: Vec<(&str, String, Option<Value>)> = vec![
        (
            "PUT",
            "/admin/settings".into(),
            put(json!({"auth.signup": "open"})),
        ),
        (
            "PUT",
            "/admin/settings".into(),
            put(json!({"auth.session_days": 9})),
        ),
        (
            "PUT",
            "/admin/settings".into(),
            put(json!({"auth.secure_cookies": false})),
        ),
        (
            "PUT",
            format!("/admin/users/{ben_id}/role"),
            Some(json!({"role": "member"})),
        ),
        (
            "PUT",
            format!("/admin/users/{ben_id}/disabled"),
            Some(json!({"disabled": true})),
        ),
        ("DELETE", format!("/admin/users/{ben_id}"), None),
        ("POST", "/auth/totp".into(), None),
    ];
    for (method, path, body) in gates {
        let r = call(&t, method, &path, a, body).await;
        assert_eq!(
            (r.status, r.body["code"].as_str()),
            (403, Some("reauth_required")),
            "{method} {path}: {}",
            r.text
        );
    }
    let r = call(&t, "GET", "/admin/users", a, None).await.body;
    let ben = r
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "ben")
        .unwrap()
        .clone();
    assert_eq!(
        (ben["role"].as_str(), ben["disabled"].as_bool()),
        (Some("admin"), Some(false)),
        "{ben}"
    );
    let page = call(&t, "GET", "/admin/settings", a, None).await.body;
    assert_eq!(setting(&page, "auth.signup")["source"], "default");

    // UX2-9: the audit CSV is a download
    let r = call(&t, "GET", "/admin/audit.csv", a, None).await;
    assert_eq!(r.status, 200);
    assert_eq!(
        r.disposition.as_deref(),
        Some("attachment; filename=\"audit.csv\""),
        "{}",
        r.text
    );
}

#[cfg(feature = "tmdb")]
#[tokio::test]
async fn credentials_status_ask_admin_and_search_reasons() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    if skip() {
        return;
    }
    let tmdb = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_string("{}"))
        .mount(&tmdb)
        .await;
    let extra = format!(
        "[auth]\nsignup = \"open\"\nsecure_cookies = false\n[connectors.tmdb]\nbase_url = \"{}\"\n",
        tmdb.uri()
    );
    let t = start(&extra, vec![]).await.unwrap();
    let admin = call(&t, "POST", "/auth/sign-up", None, Some(account("root")))
        .await
        .cookie
        .unwrap();
    let a = Some(admin.as_str());
    let member = call(&t, "POST", "/auth/sign-up", None, Some(account("mia")))
        .await
        .cookie
        .unwrap();
    let m = Some(member.as_str());

    // registered without a key: "not set up", and a search says so
    let st = call(&t, "GET", "/sources/status", m, None).await.body;
    let tm = st["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "tmdb")
        .unwrap()
        .clone();
    assert_eq!(tm["state"], "not_set_up", "{st}");
    assert_eq!(tm["credential"], Value::Null, "no detail for members");
    assert!(
        st["groups_without_source"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g == "play")
    );
    let r = call(&t, "GET", "/search?q=alien&kind=movie", m, None).await;
    assert_eq!(r.body["skipped"][0]["id"], "tmdb", "{}", r.body);
    assert_eq!(r.body["skipped"][0]["state"], "not_set_up");
    assert_eq!(r.body["groups_without_source"][0], "watch");

    // Ask admin -> inbox of the admin, once per source and day
    for _ in 0..2 {
        let r = call(
            &t,
            "POST",
            "/sources/ask-admin",
            m,
            Some(json!({"group": "play"})),
        )
        .await;
        assert_eq!(r.status, 204, "{}", r.text);
    }
    let inbox = call(&t, "GET", "/notifications/inbox", a, None).await.body;
    let asked: Vec<&Value> = inbox
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["event"] == "admin.source_requested")
        .collect();
    assert_eq!(asked.len(), 1, "{inbox}");
    assert_eq!(
        asked[0]["link"], "/admin/sources",
        "the admin lands on Admin > Data sources (UX2-2)"
    );

    // Test a candidate key that the provider rejects: reported, nothing stored
    let r = call(
        &t,
        "POST",
        "/admin/sources/tmdb/test",
        a,
        Some(json!({"fields": {"api_key": "bad"}})),
    )
    .await;
    assert_eq!(
        (r.body["ok"].as_bool(), r.body["result"].as_str()),
        (Some(false), Some("key_rejected")),
        "{}",
        r.body
    );
    let r = call(
        &t,
        "PUT",
        "/admin/sources/tmdb/credential",
        a,
        Some(json!({"fields": {}})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (422, Some("missing_field"))
    );
    let r = call(
        &t,
        "PUT",
        "/admin/sources/tmdb/credential",
        a,
        Some(json!({"fields": {"api_key": "tmdb-key-from-ui"}})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.body);
    assert_eq!(r.body["credential"]["source"], "db");
    assert_eq!(r.body["credential"]["set_by"], "root");
    assert_eq!(
        r.body["status"]["state"], "working",
        "used at once: {}",
        r.body
    );
    assert!(!r.text.contains("tmdb-key-from-ui"), "never returned");
    let (blob,): (Vec<u8>,) = sqlx::query_as("SELECT secret FROM connector_credentials")
        .fetch_one(&t.srv.pool)
        .await
        .unwrap();
    assert!(
        !blob.windows(7).any(|w| w == b"from-ui"),
        "encrypted at rest"
    );
    let st = call(&t, "GET", "/sources/status", m, None).await.body;
    assert!(
        st["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "tmdb" && s["state"] == "working")
    );
    // the stored key is rejected by the provider: "key rejected" for admins
    let r = call(&t, "POST", "/admin/sources/tmdb/test", a, Some(json!({}))).await;
    assert_eq!(r.body["result"], "key_rejected");
    let r = call(&t, "GET", "/admin/sources", a, None).await;
    let tm = r
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["status"]["id"] == "tmdb")
        .unwrap()
        .clone();
    assert_eq!(tm["status"]["state"], "key_rejected", "{tm}");
    assert_eq!(tm["credential"]["last_result"], "key_rejected");
    assert_eq!(tm["status"]["last_auth_status"], 401);
    // UX2-6: the wizard calls a rejected key typed in the UI rejected, not "set by env"
    let r = call(&t, "GET", "/setup", a, None).await;
    let src = r.body["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "tmdb")
        .unwrap()
        .clone();
    assert_eq!(src["credential"], "invalid", "{}", r.body);
    assert_eq!(r.body["sources_read_only"], false);
    let st = call(&t, "GET", "/sources/status", m, None).await.body;
    assert!(
        st["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "tmdb" && s["state"] == "unavailable")
    );
    let r = call(
        &t,
        "PUT",
        "/admin/sources/tmdb/licence",
        a,
        Some(json!({"agreement_on_file": true})),
    )
    .await;
    assert_eq!(r.body["agreement_on_file"], true);
    let r = call(&t, "DELETE", "/admin/sources/tmdb/credential", a, None).await;
    assert_eq!(r.body["status"]["state"], "not_set_up");
    let audit = call(&t, "GET", "/admin/audit?target_type=source", a, None)
        .await
        .body;
    let actions: Vec<&str> = audit["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert_eq!(
        actions,
        vec!["secret.cleared", "licence.recorded", "secret.replaced"]
    );
    assert!(!audit.to_string().contains("tmdb-key-from-ui"));
}
