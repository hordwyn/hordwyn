//! The CLI escape hatches (addendum-1 3.3, DEC-283): promote, a one-time password reset
//! link used through `POST /auth/reset-password`, reset-totp, disable-sso-only, setup reopen
//! and `secrets status|rotate|prune`; every one writes an audit row with actor `cli`.

use crate::support;

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

/// One server and database for the escape-hatch scenario: an admin `root`, a member `mia`
/// (both signed in) and one encrypted value on key v1.
struct Hatches {
    cfg: config::AppConfig,
    srv: Server,
    app: axum::Router,
    pool: sqlx::PgPool,
    root: String,
    mia: String,
    dir: std::path::PathBuf,
}

async fn hatches(db: &str) -> Hatches {
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
    let (_, _, mia) = call(
        &app,
        "POST",
        "/auth/sign-up",
        None,
        Some(account("mia", "correct horse battery")),
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
    Hatches {
        cfg,
        srv,
        app,
        pool,
        root,
        mia: mia.unwrap(),
        dir,
    }
}

/// promote, then a one-time reset link (a weak password does not use it up; the old
/// sessions end), then reset-totp with none enrolled (still audited).
async fn promote_and_reset(h: &Hatches, e: &Escape) {
    let app = &h.app;
    assert!(e.promote("nobody").await.is_err());
    e.promote("mia").await.unwrap();
    let (_, users, _) = call(app, "GET", "/admin/users", Some(&h.root), None).await;
    let m = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "mia")
        .unwrap();
    assert_eq!(m["role"], "admin");

    let link = e.reset_password("mia").await.unwrap();
    let token = link
        .strip_prefix("https://shelf.example.org/reset-password/")
        .unwrap_or_else(|| panic!("{link}"))
        .to_string();
    let reset = |pw: &str| Some(json!({"token": token, "password": pw}));
    let (code, body, _) = call(app, "POST", "/auth/reset-password", None, reset("short")).await;
    assert_eq!(code, 422, "{body}");
    let (code, body, _) = call(
        app,
        "POST",
        "/auth/reset-password",
        None,
        reset("a brand new passphrase"),
    )
    .await;
    assert_eq!(code, 204, "{body}");
    let (code, body, _) = call(
        app,
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
    let (code, _, _) = call(app, "GET", "/auth/me", Some(&h.mia), None).await;
    assert_eq!(code, 401, "the old sessions ended");
    let (code, _, _) = call(
        app,
        "POST",
        "/auth/sign-in",
        None,
        Some(json!({"username": "mia", "password": "a brand new passphrase"})),
    )
    .await;
    assert_eq!(code, 200);
    assert!(!e.reset_totp("mia").await.unwrap());
}

/// disable-sso-only sets the sign-in page back; setup reopen answers the wizard again.
async fn sso_and_setup(h: &Hatches, e: &Escape) {
    sqlx::query("UPDATE instance_settings SET sign_in_mode = 'auto'")
        .execute(&h.pool)
        .await
        .unwrap();
    assert!(e.disable_sso_only().await.unwrap());
    let mode: Option<String> = sqlx::query_scalar("SELECT sign_in_mode FROM instance_settings")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(mode.as_deref(), Some("page"));
    let (code, _, _) = call(&h.app, "POST", "/setup/complete", Some(&h.root), None).await;
    assert_eq!(code, 204);
    let (code, _, _) = call(&h.app, "GET", "/setup", Some(&h.root), None).await;
    assert_eq!(code, 404);
    e.setup_reopen().await.unwrap();
    let (code, body, _) = call(&h.app, "GET", "/setup", Some(&h.root), None).await;
    assert_eq!(code, 200, "{body}");
}

/// secrets status, rotate (v2 active, rows left on v1) and prune (v1 still in use).
async fn secrets(h: &Hatches, e: &Escape) {
    let st = e.secrets_status().await.unwrap();
    assert_eq!(st.active_version, 1);
    assert!(
        st.versions.iter().any(|v| v.version == 1 && v.rows >= 1),
        "{st:?}"
    );
    assert_eq!(e.secrets_rotate().await.unwrap(), 2);
    let st = Escape::open(&h.cfg)
        .await
        .unwrap()
        .secrets_status()
        .await
        .unwrap();
    assert_eq!(st.active_version, 2);
    assert!(st.rows_to_rewrap >= 1, "{st:?}");
    api::admin::reload_keys(&h.srv.api).await.unwrap();
    let e = Escape::open(&h.cfg).await.unwrap();
    assert_eq!(
        e.secrets_prune().await.unwrap(),
        Vec::<u16>::new(),
        "v1 still in use"
    );
}

#[tokio::test]
async fn the_cli_escape_hatches_work_and_are_audited_as_cli() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let h = Box::pin(hatches(&db)).await;
    let e = Escape::open(&h.cfg).await.unwrap();
    promote_and_reset(&h, &e).await;
    sso_and_setup(&h, &e).await;
    secrets(&h, &e).await;
    assert_eq!(
        cli_rows(&h.pool).await,
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
    std::fs::remove_dir_all(&h.dir).unwrap();
}

/// REL-1: `cli reindex --clear --now` fills the search index without a worker (a staging
/// copy runs `--role=web`, so nothing else would index what `reindex` queued).
#[tokio::test]
async fn reindex_now_fills_the_index_without_a_worker() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let h = Box::pin(hatches(&db)).await;
    h.srv
        .app
        .upsert_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: "Heat".into(),
            original_title: None,
            year: Some(1995),
            description: Some("bank robbery in Los Angeles".into()),
            external_ids: vec![domain::catalog::ExternalId::new("tmdb", "949")],
            attributes: json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap();
    let viewer: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let find = || app::ports::SearchQuery {
        index: "catalog".into(),
        text: "robbery".into(),
        viewer: domain::ids::UserId(viewer),
        filters: vec![],
        limit: 10,
    };
    // `reindex --clear` queues; without a worker nothing reaches the index
    search::pg::PgSearch::new(h.pool.clone())
        .clear("catalog")
        .await
        .unwrap();
    search::indexer::Indexer::enqueue_all(&h.pool)
        .await
        .unwrap();
    assert_eq!(h.srv.search.search(find()).await.unwrap().len(), 0);
    // `--now` indexes in the calling process
    let done = server::escape::index_now(&h.pool, h.srv.search.clone())
        .await
        .unwrap();
    assert!(done > 0);
    assert_eq!(h.srv.search.search(find()).await.unwrap().len(), 1);
    std::fs::remove_dir_all(&h.dir).unwrap();
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

/// DEC-383 (deferred to WP4-8): with the keys from `APP__SECRETS__KEYS` (no key file)
/// `rotate` and `prune` refuse and point to the variable; `promote` of an admin keeps the
/// admin and says so in the audit row; `reset-totp` reports and removes an enrolled
/// authenticator; a reset link of a disabled account sets the password but signs nobody in.
#[tokio::test]
async fn env_keys_an_admin_promoted_an_enrolled_reset_and_a_disabled_reset() {
    let Some(db) = store_pg::testing::fresh_db_url().await else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("cli-env-{}", uuid::Uuid::now_v7()));
    let key_file = dir.join("secrets.key");
    let keys = "v1:".to_string() + &"Q".repeat(43) + "=";
    let cfg = cli_config(&db, &key_file, vec![("APP__SECRETS__KEYS".into(), keys)]);
    let srv = Box::pin(Server::build(
        cfg.clone(),
        config::ServerRole::All,
        Overrides::default(),
    ))
    .await
    .unwrap();
    let app = srv.base_router();
    let (_, _, root) = call(
        &app,
        "POST",
        "/auth/sign-up",
        None,
        Some(account("root", "correct horse battery")),
    )
    .await;
    root.unwrap();
    call(
        &app,
        "POST",
        "/auth/sign-up",
        None,
        Some(account("mia", "correct horse battery")),
    )
    .await;
    let e = Escape::open(&cfg).await.unwrap();
    for err in [
        e.secrets_rotate().await.unwrap_err(),
        e.secrets_prune().await.map(|_| ()).unwrap_err(),
    ] {
        assert!(err.to_string().contains("APP__SECRETS__KEYS"), "{err}");
    }
    assert!(!key_file.exists(), "no key file was written");

    e.promote("root").await.unwrap();
    let detail: Value = sqlx::query_scalar(
        "SELECT detail FROM admin_audit WHERE action = 'role.changed' AND actor_kind = 'cli'",
    )
    .fetch_one(&srv.pool)
    .await
    .unwrap();
    assert_eq!(
        (&detail["from"], &detail["to"]),
        (&json!("admin"), &json!("admin"))
    );

    sqlx::query(
        "UPDATE credentials SET totp_enabled = true
         WHERE user_id = (SELECT id FROM users WHERE username = 'mia')",
    )
    .execute(&srv.pool)
    .await
    .unwrap();
    assert!(e.reset_totp("mia").await.unwrap(), "it was enrolled");
    let enabled: bool = sqlx::query_scalar(
        "SELECT totp_enabled FROM credentials
         WHERE user_id = (SELECT id FROM users WHERE username = 'mia')",
    )
    .fetch_one(&srv.pool)
    .await
    .unwrap();
    assert!(!enabled);
    assert!(!e.reset_totp("mia").await.unwrap(), "nothing left to reset");

    sqlx::query(
        "INSERT INTO user_disabled (user_id, disabled_by, disabled_at)
         SELECT m.id, r.id, now() FROM users m, users r WHERE m.username = 'mia' AND r.username = 'root'",
    )
    .execute(&srv.pool)
    .await
    .unwrap();
    let link = e.reset_password("mia").await.unwrap();
    let token = link.rsplit('/').next().unwrap().to_string();
    let (code, body, _) = call(
        &app,
        "POST",
        "/auth/reset-password",
        None,
        Some(json!({"token": token, "password": "a brand new passphrase"})),
    )
    .await;
    assert_eq!(code, 204, "{body}");
    let (code, body, cookie) = call(
        &app,
        "POST",
        "/auth/sign-in",
        None,
        Some(json!({"username": "mia", "password": "a brand new passphrase"})),
    )
    .await;
    assert_eq!(
        (code, body["code"].as_str(), cookie),
        (403, Some("account_disabled"), None),
        "{body}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
