//! Package v1-ops over HTTP: the reminder to back up the secrets key (OPS-4: on Admin >
//! Overview until an admin confirms it, gated and audited, asked again for a new key
//! version) and `Cache-Control: no-cache` on the unhashed `/pkg` files (OPS-8).

// The short names (t: server, a/m: admin/member cookie, o: overview, r: answer) keep the
// many calls readable, as in `admin.rs`. Every test starts its own server, database and
// key file (review V1OPS-9: one scenario per test, no test over 100 lines).
#![allow(clippy::many_single_char_names)]

use crate::support;

use leptos::server_fn::ServerFn;
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

struct T {
    srv: Server,
    app: axum::Router,
    dir: std::path::PathBuf,
}

/// A server with a real key file (generated at this start) and a site root with one
/// unhashed bundle file.
async fn start() -> T {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let dir = std::env::temp_dir().join(format!("ops-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(dir.join("site/pkg")).unwrap();
    std::fs::write(dir.join("site/pkg/app.js"), "export default 1;").unwrap();
    std::fs::write(dir.join("site/plain.txt"), "plain").unwrap();
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nkey_file = \"{}\"\n[storage]\nbackend = \"memory\"\n\
         [search]\nbackend = \"postgres\"\n[brand]\ndir = \"{}\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n",
        dir.join("secrets.key").display(),
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .site_root(dir.join("site").display().to_string())
        .build();
    let app = srv.app_router(&options);
    T { srv, app, dir }
}

struct Resp {
    status: u16,
    body: Value,
    text: String,
    cookie: Option<String>,
    cache: Option<String>,
}

async fn call(
    t: &T,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    lang: &str,
    body: Option<Value>,
) -> Resp {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("accept-language", lang)
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let body = body.map_or_else(String::new, |b| b.to_string());
    let resp = support::send(&t.app, req.body(axum::body::Body::from(body)).unwrap()).await;
    let status = resp.status().as_u16();
    let header = |name: &str| {
        resp.headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_string())
    };
    let cookie = header("set-cookie").map(|c| c.split(';').next().unwrap().to_string());
    let cache = header("cache-control");
    let bytes = resp.into_body();
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        text: String::from_utf8_lossy(&bytes).into_owned(),
        cookie,
        cache,
    }
}

async fn sign_up(t: &T, name: &str) -> String {
    let r = call(
        t,
        "POST",
        "/api/v1/auth/sign-up",
        None,
        "en",
        Some(json!({"username": name, "display_name": name, "password": "correct horse battery"})),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.text);
    r.cookie.unwrap()
}

const OVERVIEW: &str = "/api/v1/admin/overview";
const CONFIRM: &str = "/api/v1/admin/secrets/backup-confirmed";
const AUDIT: &str = "/api/v1/admin/audit?action=secrets.backup_confirmed";

/// The server, the admin `root` and the member `mia` (their cookies; both proofs are fresh
/// right after the sign-up).
async fn start_with_accounts() -> (T, String, String) {
    let t = start().await;
    let admin = sign_up(&t, "root").await;
    let member = sign_up(&t, "mia").await;
    (t, admin, member)
}

/// The page's server function (what the button calls), as `cookie`.
async fn confirm_fn(t: &T, cookie: &str) -> u16 {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(web::admin_api::ConfirmKeyBackup::PATH)
        .header("content-type", "application/x-www-form-urlencoded")
        .header("accept", "application/json")
        .header("cookie", cookie)
        .body(axum::body::Body::empty())
        .unwrap();
    support::send(&t.app, req).await.status().as_u16()
}

/// OPS-4 (owner ruling 2026-11-12, option a): Admin > Overview carries the reminder while
/// the backup of the active key is not confirmed: in the overview's data and on the page, in
/// both languages, with the path of the key file and one button.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_key_backup_reminder_is_shown_with_the_path_of_the_key() {
    if !services() {
        return;
    }
    let (t, admin, _) = start_with_accounts().await;
    let a = Some(admin.as_str());
    let key_file = t.dir.join("secrets.key");

    let o = call(&t, "GET", OVERVIEW, a, "en", None).await;
    assert_eq!(o.status, 200, "{}", o.text);
    assert_eq!(o.body["key_backup"]["version"], 1, "{}", o.text);
    assert_eq!(
        o.body["key_backup"]["key_file"],
        key_file.display().to_string()
    );

    let page = call(&t, "GET", "/admin", a, "en", None).await;
    assert_eq!(page.status, 200);
    assert!(page.text.contains("data-testid=\"key-backup\""), "banner");
    assert!(page.text.contains("Back up the secrets key"), "en title");
    assert!(page.text.contains(&key_file.display().to_string()), "path");
    assert!(
        page.text.contains(">I have backed up the key<"),
        "en button"
    );
    let page = call(&t, "GET", "/admin", a, "de", None).await;
    assert!(
        page.text.contains("Geheimnisschl\u{fc}ssel sichern"),
        "de title"
    );
    assert!(
        page.text
            .contains(">Ich habe den Schl\u{fc}ssel gesichert<"),
        "de button"
    );
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// The confirmation is admin-only, needs a fresh proof and writes an audit row (the key
/// version and the proof method, never the key); then the reminder is gone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn confirming_the_key_backup_is_gated_and_audited() {
    if !services() {
        return;
    }
    let (t, admin, member) = start_with_accounts().await;
    let (a, m) = (Some(admin.as_str()), Some(member.as_str()));

    // a member cannot confirm; a stale proof is refused and nothing is recorded
    let r = call(&t, "POST", CONFIRM, m, "en", None).await;
    assert_eq!(r.status, 403, "{}", r.text);
    sqlx::query(
        "UPDATE auth_sessions SET reauth_at = now() - interval '2 hours', reauth_slide_at = NULL",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();
    let r = call(&t, "POST", CONFIRM, a, "en", None).await;
    assert_eq!(r.body["code"], "reauth_required", "{}", r.text);
    let o = call(&t, "GET", OVERVIEW, a, "en", None).await;
    assert_eq!(o.body["key_backup"]["version"], 1, "still asked");
    let audit = call(&t, "GET", AUDIT, a, "en", None).await;
    assert_eq!(audit.body["entries"].as_array().unwrap().len(), 0);

    // a fresh proof: confirmed and audited
    let proof = json!({"password": "correct horse battery"});
    let r = call(&t, "POST", "/api/v1/auth/reauth", a, "en", Some(proof)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let admin = r.cookie.unwrap_or(admin);
    let a = Some(admin.as_str());
    let r = call(&t, "POST", CONFIRM, a, "en", None).await;
    assert_eq!(r.status, 204, "{}", r.text);
    let o = call(&t, "GET", OVERVIEW, a, "en", None).await;
    assert_eq!(o.body["key_backup"], Value::Null, "{}", o.text);
    let page = call(&t, "GET", "/admin", a, "en", None).await;
    assert!(!page.text.contains("data-testid=\"key-backup\""), "gone");
    let audit = call(&t, "GET", AUDIT, a, "en", None).await;
    let rows = audit.body["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{}", audit.text);
    assert_eq!(rows[0]["actor_name"], "root");
    assert_eq!(rows[0]["detail"]["key_version"], 1);
    assert_eq!(rows[0]["detail"]["proof"], "password");
    let key = std::fs::read_to_string(t.dir.join("secrets.key")).unwrap();
    let secret = key.lines().find(|l| l.starts_with("v1:")).unwrap();
    assert!(!audit.text.contains(&secret[3..]), "the key is not logged");
    // the audit page names the action in words
    let page = call(&t, "GET", "/admin/audit", a, "en", None).await;
    assert!(
        page.text.contains("Secrets key backup confirmed"),
        "audit label"
    );
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// Confirming again changes nothing and is no error; the page's server function reaches
/// the same operation (and the same gate); a new active key version asks again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_confirmation_changes_nothing_and_a_rotation_asks_again() {
    if !services() {
        return;
    }
    let (t, admin, member) = start_with_accounts().await;
    let a = Some(admin.as_str());
    let key_file = t.dir.join("secrets.key");

    assert_ne!(confirm_fn(&t, &member).await, 200, "member");
    let o = call(&t, "GET", OVERVIEW, a, "en", None).await;
    assert_eq!(o.body["key_backup"]["version"], 1, "{}", o.text);
    assert_eq!(confirm_fn(&t, &admin).await, 200);
    let o = call(&t, "GET", OVERVIEW, a, "en", None).await;
    assert_eq!(o.body["key_backup"], Value::Null, "{}", o.text);
    let r = call(&t, "POST", CONFIRM, a, "en", None).await;
    assert_eq!(r.status, 204, "{}", r.text);
    assert_eq!(confirm_fn(&t, &admin).await, 200);
    let audit = call(&t, "GET", AUDIT, a, "en", None).await;
    assert_eq!(
        audit.body["entries"].as_array().unwrap().len(),
        1,
        "one row"
    );

    // a rotation (new active version, loaded like SIGHUP does) asks again
    assert_eq!(auth::secrets::rotate_key_file(&key_file).unwrap(), 2);
    api::admin::reload_keys(&t.srv.api).await.unwrap();
    let o = call(&t, "GET", OVERVIEW, a, "en", None).await;
    assert_eq!(o.body["key_backup"]["version"], 2, "{}", o.text);
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// OPS-8, the cheap variant: the bundle files under `/pkg` keep their names across
/// releases, so a browser must ask before it reuses one; other answers are left alone.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unhashed_bundle_files_are_revalidated() {
    if !services() {
        return;
    }
    let t = start().await;
    let r = call(&t, "GET", "/pkg/app.js", None, "en", None).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.text, "export default 1;");
    assert_eq!(r.cache.as_deref(), Some("no-cache"));
    // "ask first" must not mean "download again": the validator the answer carries is
    // honoured, so an unchanged file costs one empty 304
    let get = |since: Option<String>| {
        let mut req = axum::http::Request::builder().uri("/pkg/app.js");
        if let Some(s) = since {
            req = req.header("if-modified-since", s);
        }
        support::send(&t.app, req.body(axum::body::Body::empty()).unwrap())
    };
    let first = get(None).await;
    let stamp = first.headers()["last-modified"]
        .to_str()
        .unwrap()
        .to_string();
    let again = get(Some(stamp.clone())).await;
    assert_eq!(again.status(), 304);
    assert_eq!(again.headers()["cache-control"], "no-cache");
    assert_eq!(again.headers()["last-modified"].to_str().unwrap(), stamp);
    assert_eq!(again.body().len(), 0);
    let other = get(Some("Thu, 01 Jan 2004 00:00:00 GMT".into())).await;
    assert_eq!(other.status(), 200, "another release's stamp gets the file");
    assert_eq!(other.body().len(), "export default 1;".len());
    let r = call(&t, "GET", "/pkg/missing.wasm", None, "en", None).await;
    assert_eq!(r.status, 404);
    assert_eq!(r.cache.as_deref(), Some("no-cache"));
    let r = call(&t, "GET", "/plain.txt", None, "en", None).await;
    assert_eq!(r.status, 200);
    assert_eq!(r.cache, None, "only /pkg");
    let r = call(&t, "GET", "/healthz", None, "en", None).await;
    assert_eq!((r.status, r.cache), (200, None));
    std::fs::remove_dir_all(&t.dir).unwrap();
}
