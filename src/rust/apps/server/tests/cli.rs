//! The CLI escape hatches (addendum-1 3.3, DEC-283): promote, a one-time password reset
//! link used through `POST /auth/reset-password`, reset-totp, disable-sso-only, setup reopen
//! and `secrets status|rotate|prune`; every one writes an audit row with actor `cli`.

// One scenario on purpose: the steps share one server and database.
#![allow(clippy::too_many_lines)]

mod support;

use serde_json::{Value, json};
use server::escape::Escape;
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (u16, Value, Option<String>) {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(format!("/api/v1{uri}"))
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    let body = body.map_or_else(String::new, |b| b.to_string());
    let resp = support::send(app, req.body(axum::body::Body::from(body)).unwrap()).await;
    let status = resp.status().as_u16();
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = resp.into_body();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
    )
}

fn account(name: &str, password: &str) -> Value {
    json!({"username": name, "display_name": name, "password": password})
}

async fn cli_rows(pool: &sqlx::PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM admin_audit WHERE actor_kind = 'cli' ORDER BY at, id")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_cli_escape_hatches_work_and_are_audited_as_cli() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("cli-{}", uuid::Uuid::now_v7()));
    let key_file = dir.join("secrets.key");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nkey_file = \"{}\"\n[storage]\nbackend = \"memory\"\n\
         [search]\nbackend = \"postgres\"\n[brand]\ndir = \"{}\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[server]\npublic_url = \"https://shelf.example.org\"\n",
        key_file.display(),
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg.clone(), config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    let pool = srv.pool.clone();
    let (_, _, root) = call(
        &app,
        "POST",
        "/auth/sign-up",
        None,
        Some(account("root", "correct horse battery")),
    )
    .await;
    let root = root.unwrap();
    let (_, _, mia) = call(
        &app,
        "POST",
        "/auth/sign-up",
        None,
        Some(account("mia", "correct horse battery")),
    )
    .await;
    let mia = mia.unwrap();
    // one encrypted value on key v1
    let (code, _, _) = call(
        &app,
        "PUT",
        "/admin/sources/tmdb/credential",
        Some(&root),
        Some(json!({"fields": {"api_key": "k-1"}})),
    )
    .await;
    assert_eq!(code, 200);

    let e = Escape::open(&cfg).await.unwrap();

    // promote
    assert!(e.promote("nobody").await.is_err());
    e.promote("mia").await.unwrap();
    let (_, users, _) = call(&app, "GET", "/admin/users", Some(&root), None).await;
    let m = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "mia")
        .unwrap();
    assert_eq!(m["role"], "admin");

    // reset-password: a one-time link, a weak password does not use it up
    let link = e.reset_password("mia").await.unwrap();
    let token = link
        .strip_prefix("https://shelf.example.org/reset-password/")
        .unwrap_or_else(|| panic!("{link}"))
        .to_string();
    let reset = |pw: &str| Some(json!({"token": token, "password": pw}));
    let (code, body, _) = call(&app, "POST", "/auth/reset-password", None, reset("short")).await;
    assert_eq!(code, 422, "{body}");
    let (code, body, _) = call(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        reset("a brand new passphrase"),
    )
    .await;
    assert_eq!(code, 204, "{body}");
    let (code, body, _) = call(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        reset("another passphrase!"),
    )
    .await;
    assert_eq!(
        (code, body["code"].as_str()),
        (404, Some("reset_link_invalid"))
    );
    let (code, _, _) = call(&app, "GET", "/auth/me", Some(&mia), None).await;
    assert_eq!(code, 401, "the old sessions ended");
    let (code, _, _) = call(
        &app,
        "POST",
        "/auth/sign-in",
        None,
        Some(json!({"username": "mia", "password": "a brand new passphrase"})),
    )
    .await;
    assert_eq!(code, 200);

    // reset-totp (none enrolled: still fine, still audited)
    assert!(!e.reset_totp("mia").await.unwrap());

    // disable-sso-only defines auth.sign_in_mode and sets it back to the sign-in page
    sqlx::query("UPDATE instance_settings SET sign_in_mode = 'auto'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(e.disable_sso_only().await.unwrap());
    let mode: Option<String> = sqlx::query_scalar("SELECT sign_in_mode FROM instance_settings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mode.as_deref(), Some("page"));

    // setup reopen: the wizard answers again after completion
    let (code, _, _) = call(&app, "POST", "/setup/complete", Some(&root), None).await;
    assert_eq!(code, 204);
    let (code, _, _) = call(&app, "GET", "/setup", Some(&root), None).await;
    assert_eq!(code, 404);
    e.setup_reopen().await.unwrap();
    let (code, body, _) = call(&app, "GET", "/setup", Some(&root), None).await;
    assert_eq!(code, 200, "{body}");

    // secrets status, rotate, prune
    let st = e.secrets_status().await.unwrap();
    assert_eq!(st.active_version, 1);
    assert!(
        st.versions.iter().any(|v| v.version == 1 && v.rows >= 1),
        "{st:?}"
    );
    assert_eq!(e.secrets_rotate().await.unwrap(), 2);
    let st = Escape::open(&cfg)
        .await
        .unwrap()
        .secrets_status()
        .await
        .unwrap();
    assert_eq!(st.active_version, 2);
    assert!(st.rows_to_rewrap >= 1, "{st:?}");
    api::admin::reload_keys(&srv.api).await.unwrap();
    let e = Escape::open(&cfg).await.unwrap();
    assert_eq!(
        e.secrets_prune().await.unwrap(),
        Vec::<u16>::new(),
        "v1 still in use"
    );

    assert_eq!(
        cli_rows(&pool).await,
        [
            "role.changed",
            "password_reset.created",
            "totp.reset",
            "setting.changed",
            "setup.reopened",
            "secrets.rotated",
            "secrets.pruned",
        ]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// DEC-283: SIGHUP re-reads the key file; a set without a version still in use is refused
/// (the loaded ring stays), a set that keeps it is taken, and the reload is audited.
#[tokio::test]
async fn a_key_reload_keeps_versions_in_use() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("reload-{}", uuid::Uuid::now_v7()));
    let key_file = dir.join("secrets.key");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nkey_file = \"{}\"\n[storage]\nbackend = \"memory\"\n\
         [search]\nbackend = \"postgres\"\n[brand]\ndir = \"{}\"\n[auth]\nsecure_cookies = false\n",
        key_file.display(),
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg.clone(), config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    let (_, _, root) = call(
        &app,
        "POST",
        "/setup/account",
        None,
        Some(account("root", "correct horse battery")),
    )
    .await;
    let root = root.unwrap();
    let (code, _, _) = call(
        &app,
        "PUT",
        "/admin/sources/tmdb/credential",
        Some(&root),
        Some(json!({"fields": {"api_key": "k-1"}})),
    )
    .await;
    assert_eq!(code, 200);
    let v1 = std::fs::read_to_string(&key_file).unwrap();
    let v1_line = v1
        .lines()
        .find(|l| l.starts_with("v1:"))
        .unwrap()
        .to_string();

    // a new file with only v2: refused, v1 still loaded and the value still readable
    let other = "v2:".to_string() + &"A".repeat(43) + "=";
    std::fs::write(&key_file, format!("{other}\n")).unwrap();
    let err = api::admin::reload_keys(&srv.api).await.unwrap_err();
    assert!(err.contains("[1]"), "{err}");
    let (_, st, _) = call(&app, "GET", "/admin/security", Some(&root), None).await;
    assert_eq!(st["secrets"]["active_version"], 1, "{st}");

    // v2 first, v1 kept: taken; new values use v2
    std::fs::write(&key_file, format!("{other}\n{v1_line}\n")).unwrap();
    assert_eq!(api::admin::reload_keys(&srv.api).await.unwrap(), vec![2, 1]);
    let (_, st, _) = call(&app, "GET", "/admin/security", Some(&root), None).await;
    assert_eq!(st["secrets"]["active_version"], 2, "{st}");
    let (code, _, _) = call(
        &app,
        "PUT",
        "/admin/sources/igdb/credential",
        Some(&root),
        Some(json!({"fields": {"client_id": "c", "client_secret": "s"}})),
    )
    .await;
    assert_eq!(code, 200);
    let versions: Vec<i32> = sqlx::query_scalar(
        "SELECT get_byte(secret, 1) * 256 + get_byte(secret, 2) FROM connector_credentials
         ORDER BY connector_id",
    )
    .fetch_all(&srv.pool)
    .await
    .unwrap();
    assert_eq!(versions, [2, 1], "igdb on v2, tmdb still on v1");
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM admin_audit WHERE action = 'secrets.reloaded'")
            .fetch_one(&srv.pool)
            .await
            .unwrap();
    assert_eq!(n, 1);
    std::fs::remove_dir_all(&dir).unwrap();
}

fn cli_config(
    db: &str,
    key_file: &std::path::Path,
    env: Vec<(String, String)>,
) -> config::AppConfig {
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nkey_file = \"{}\"\n[storage]\nbackend = \"memory\"\n\
         [search]\nbackend = \"postgres\"\n[brand]\ndir = \"{}\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n",
        key_file.display(),
        repo("config/brand/default")
    );
    config::AppConfig::from_sources(&toml, env).unwrap()
}

/// UX3-1: `secrets status` and `prune` see every encrypted purpose; a version that only an
/// OIDC client secret uses is counted and kept.
#[tokio::test]
async fn prune_keeps_a_version_only_an_oidc_client_secret_uses() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("cli-oidc-{}", uuid::Uuid::now_v7()));
    let cfg = cli_config(&db, &dir.join("secrets.key"), Vec::new());
    let srv = Server::build(cfg.clone(), config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    let (_, _, root) = call(
        &app,
        "POST",
        "/setup/account",
        None,
        Some(account("root", "correct horse battery")),
    )
    .await;
    let root = root.unwrap();
    let (code, body, _) = call(
        &app,
        "POST",
        "/admin/oidc/providers",
        Some(&root),
        Some(json!({"slug": "pocket", "display_name": "Pocket ID",
            "issuer": "https://id.example.org", "client_id": "shelf", "client_secret": "s3cret"})),
    )
    .await;
    assert_eq!(code, 201, "{body}");

    let e = Escape::open(&cfg).await.unwrap();
    let st = e.secrets_status().await.unwrap();
    assert!(
        st.versions.iter().any(|v| v.version == 1 && v.rows >= 1),
        "the client secret is counted: {st:?}"
    );
    assert_eq!(e.secrets_rotate().await.unwrap(), 2);
    let e = Escape::open(&cfg).await.unwrap();
    // UX3-20: the server still seals with v1 until it reloads; prune waits for that
    let err = e.secrets_prune().await.unwrap_err().to_string();
    assert!(err.contains("SIGHUP"), "{err}");
    api::admin::reload_keys(&srv.api).await.unwrap();
    assert_eq!(
        e.secrets_prune().await.unwrap(),
        Vec::<u16>::new(),
        "v1 still protects the client secret"
    );
    let keys = std::fs::read_to_string(dir.join("secrets.key")).unwrap();
    assert!(keys.lines().any(|l| l.starts_with("v1:")));
    // UX3-24: once the nightly job moved every value to v2, prune drops v1
    api::admin::rewrap_all(srv.api.admin.as_ref().unwrap())
        .await
        .unwrap();
    assert_eq!(e.secrets_prune().await.unwrap(), vec![1]);
    let keys = std::fs::read_to_string(dir.join("secrets.key")).unwrap();
    assert!(
        !keys.lines().any(|l| l.starts_with("v1:")),
        "{}",
        keys.len()
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// UX3-2: with the mode fixed by the environment the hatch changes nothing and says where to
/// change it instead of reporting success.
#[tokio::test]
async fn disable_sso_only_refuses_when_the_environment_fixes_auto() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("cli-mode-{}", uuid::Uuid::now_v7()));
    let env = vec![("APP__AUTH__SIGN_IN_MODE".to_string(), "auto".to_string())];
    let cfg = cli_config(&db, &dir.join("secrets.key"), env);
    let e = Escape::open(&cfg).await.unwrap();
    sqlx::query("UPDATE instance_settings SET sign_in_mode = 'auto'")
        .execute(e.pool())
        .await
        .unwrap();
    let err = e.disable_sso_only().await.unwrap_err().to_string();
    assert!(err.contains("APP__AUTH__SIGN_IN_MODE"), "{err}");
    let mode: Option<String> = sqlx::query_scalar("SELECT sign_in_mode FROM instance_settings")
        .fetch_one(e.pool())
        .await
        .unwrap();
    assert_eq!(mode.as_deref(), Some("auto"), "nothing changed");
    let _ = std::fs::remove_dir_all(&dir);
}

/// UX3-20: a hatch changes first and audits second; when the audit row cannot be written the
/// error says the change was made.
#[tokio::test]
async fn a_failed_audit_row_still_says_the_change_was_made() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("cli-audit-{}", uuid::Uuid::now_v7()));
    let cfg = cli_config(&db, &dir.join("secrets.key"), Vec::new());
    let srv = Server::build(cfg.clone(), config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    call(
        &app,
        "POST",
        "/setup/account",
        None,
        Some(account("root", "correct horse battery")),
    )
    .await;
    call(
        &app,
        "POST",
        "/auth/sign-up",
        None,
        Some(account("mia", "correct horse battery")),
    )
    .await;
    sqlx::raw_sql(
        "CREATE FUNCTION refuse_audit() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'audit refused'; END $$;
         CREATE TRIGGER refuse_audit BEFORE INSERT ON admin_audit
             FOR EACH ROW EXECUTE FUNCTION refuse_audit();",
    )
    .execute(&srv.pool)
    .await
    .unwrap();
    let e = Escape::open(&cfg).await.unwrap();
    let err = e.promote("mia").await.unwrap_err().to_string();
    assert!(err.contains("done") && err.contains("audit row"), "{err}");
    let role: String = sqlx::query_scalar("SELECT role FROM users WHERE username = 'mia'")
        .fetch_one(&srv.pool)
        .await
        .unwrap();
    assert_eq!(role, "admin", "the promotion happened");
    let _ = std::fs::remove_dir_all(&dir);
}

/// UX3-12: a new reset link revokes the older open one, and a storage error after the claim
/// leaves the link usable (claim and password change are one transaction).
#[tokio::test]
async fn reset_links_are_atomic_and_only_the_newest_is_open() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("cli-reset-{}", uuid::Uuid::now_v7()));
    let cfg = cli_config(&db, &dir.join("secrets.key"), Vec::new());
    let srv = Server::build(cfg.clone(), config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let app = srv.base_router();
    call(
        &app,
        "POST",
        "/setup/account",
        None,
        Some(account("root", "correct horse battery")),
    )
    .await;
    let e = Escape::open(&cfg).await.unwrap();
    let token = |link: String| link.rsplit('/').next().unwrap().to_string();
    let first = token(e.reset_password("root").await.unwrap());
    let second = token(e.reset_password("root").await.unwrap());
    let reset = |t: &str, pw: &str| Some(json!({"token": t, "password": pw}));
    let (code, body, _) = call(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        reset(&first, "a brand new passphrase"),
    )
    .await;
    assert_eq!(
        (code, body["code"].as_str()),
        (404, Some("reset_link_invalid")),
        "the older link was revoked"
    );

    sqlx::raw_sql(
        "CREATE FUNCTION refuse_creds() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'credentials refused'; END $$;
         CREATE TRIGGER refuse_creds BEFORE INSERT OR UPDATE ON credentials
             FOR EACH ROW EXECUTE FUNCTION refuse_creds();",
    )
    .execute(&srv.pool)
    .await
    .unwrap();
    let (code, _, _) = call(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        reset(&second, "a brand new passphrase"),
    )
    .await;
    assert_eq!(code, 500);
    sqlx::raw_sql("DROP TRIGGER refuse_creds ON credentials")
        .execute(&srv.pool)
        .await
        .unwrap();
    let (code, body, _) = call(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        reset(&second, "a brand new passphrase"),
    )
    .await;
    assert_eq!(
        code, 204,
        "the failed attempt did not use the link up: {body}"
    );
    let (code, _, _) = call(
        &app,
        "POST",
        "/auth/sign-in",
        None,
        Some(json!({"username": "root", "password": "a brand new passphrase"})),
    )
    .await;
    assert_eq!(code, 200);
    let _ = std::fs::remove_dir_all(&dir);
}
