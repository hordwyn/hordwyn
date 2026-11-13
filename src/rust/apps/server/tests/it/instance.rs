//! UX phase 3, instance server (WP3-1) over HTTP: audit rows in the change's transaction,
//! client address and request id behind trusted proxies, SMTP in the database, the source
//! Disabled switch, the wizard's done flags and the per-user time zone.

use crate::support;

use std::net::SocketAddr;

use serde_json::{Value, json};
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

struct T {
    app: axum::Router,
    srv: Server,
    db: String,
}

async fn start_on(db: String, extra: &str, env: Vec<(String, String)>) -> T {
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [brand]\ndir = \"{}\"\n{extra}",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, env).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    T {
        app: srv.base_router(),
        srv,
        db,
    }
}

async fn start(extra: &str, env: Vec<(String, String)>) -> Option<T> {
    let db = store_pg::testing::fresh_db_url().await?;
    Some(start_on(db, extra, env).await)
}

struct Resp {
    status: u16,
    body: Value,
    cookie: Option<String>,
    text: String,
    request_id: Option<String>,
}

#[derive(Default)]
struct Opts<'a> {
    cookie: Option<&'a str>,
    body: Option<Value>,
    peer: Option<&'a str>,
    headers: Vec<(&'a str, &'a str)>,
}

async fn send(t: &T, method: &str, uri: &str, o: Opts<'_>) -> Resp {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(format!("/api/v1{uri}"))
        .header("content-type", "application/json");
    if let Some(c) = o.cookie {
        req = req.header("cookie", c);
    }
    for (k, v) in &o.headers {
        req = req.header(*k, *v);
    }
    let body = o.body.map_or_else(String::new, |b| b.to_string());
    let mut req = req.body(axum::body::Body::from(body)).unwrap();
    if let Some(p) = o.peer {
        let addr: SocketAddr = p.parse().unwrap();
        req.extensions_mut()
            .insert(axum::extract::ConnectInfo(addr));
    }
    let resp = support::send(&t.app, req).await;
    let status = resp.status().as_u16();
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let request_id = resp
        .headers()
        .get("x-request-id")
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = resp.into_body();
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
        text: String::from_utf8_lossy(&bytes).into_owned(),
        request_id,
    }
}

async fn call(t: &T, method: &str, uri: &str, cookie: Option<&str>, body: Option<Value>) -> Resp {
    send(
        t,
        method,
        uri,
        Opts {
            cookie,
            body,
            ..Opts::default()
        },
    )
    .await
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

async fn sign_up(t: &T, name: &str) -> String {
    let r = call(t, "POST", "/auth/sign-up", None, Some(account(name))).await;
    assert_eq!(r.status, 201, "{}", r.text);
    r.cookie.unwrap()
}

fn setting<'a>(page: &'a Value, key: &str) -> &'a Value {
    page["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("no setting {key} in {page}"))
}

async fn audit_rows(t: &T, a: Option<&str>, action: &str) -> Vec<Value> {
    let r = call(t, "GET", &format!("/admin/audit?action={action}"), a, None).await;
    assert_eq!(r.status, 200, "{}", r.text);
    r.body["entries"].as_array().unwrap().clone()
}

fn health<'a>(overview: &'a Value, id: &str) -> &'a Value {
    overview["health"]
        .as_array()
        .unwrap()
        .iter()
        .find(|h| h["id"] == id)
        .unwrap()
}

/// The id of the user called `name` (from the admin's user list).
async fn user_id(t: &T, admin: Option<&str>, name: &str) -> String {
    let users = call(t, "GET", "/admin/users", admin, None).await.body;
    users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == name)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// A trigger that refuses every audit insert (dropped with `DROP TRIGGER refuse_audit`).
async fn refuse_audit_writes(t: &T) {
    sqlx::raw_sql(
        "CREATE FUNCTION refuse_audit() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'audit refused'; END $$;
         CREATE TRIGGER refuse_audit BEFORE INSERT ON admin_audit
             FOR EACH ROW EXECUTE FUNCTION refuse_audit();",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();
}

/// A catalog movie without external ids; returns its id.
async fn movie(t: &T, title: &str) -> String {
    t.srv
        .app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new("movie").unwrap(),
            title: title.into(),
            original_title: None,
            year: Some(2020),
            description: None,
            external_ids: Vec::new(),
            attributes: json!({}),
            cover_url: None,
            popularity: None,
        })
        .await
        .unwrap()
        .id
        .0
        .to_string()
}

/// addendum-1 3.4 / DEC-245: "each row is written in the same transaction as the change".
/// A trigger that refuses every audit insert stands in for a failing audit write: the
/// change must not happen either.
#[tokio::test]
async fn an_audit_row_that_cannot_be_written_rolls_its_change_back() {
    if skip() {
        return;
    }
    let t = start(
        "[auth]\nsignup = \"open\"\nsecure_cookies = false\n",
        vec![],
    )
    .await
    .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    let _ = sign_up(&t, "mia").await;
    let mia = user_id(&t, a, "mia").await;
    refuse_audit_writes(&t).await;

    let put = json!({"values": {"instance.hosting": "hosted"}});
    let r = call(&t, "PUT", "/admin/settings", a, Some(put.clone())).await;
    assert_eq!(r.status, 500, "{}", r.text);
    let page = call(&t, "GET", "/admin/settings", a, None).await.body;
    assert_eq!(setting(&page, "instance.hosting")["value"], "home");

    let r = call(
        &t,
        "PUT",
        "/admin/sources/tmdb/credential",
        a,
        Some(json!({"fields": {"api_key": "k-123"}})),
    )
    .await;
    assert_eq!(r.status, 500, "{}", r.text);
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM connector_credentials")
        .fetch_one(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(n, 0, "credential rolled back");

    let r = call(
        &t,
        "PUT",
        "/admin/sources/tmdb/licence",
        a,
        Some(json!({"agreement_on_file": true})),
    )
    .await;
    assert_eq!(r.status, 500, "{}", r.text);
    let r = call(&t, "POST", "/admin/invites", a, Some(json!({}))).await;
    assert_eq!(r.status, 500, "{}", r.text);
    let invites = call(&t, "GET", "/admin/invites", a, None).await.body;
    assert_eq!(invites.as_array().unwrap().len(), 0, "{invites}");
    let r = call(
        &t,
        "PUT",
        &format!("/admin/users/{mia}/disabled"),
        a,
        Some(json!({"disabled": true})),
    )
    .await;
    assert_eq!(r.status, 500, "{}", r.text);
    let users = call(&t, "GET", "/admin/users", a, None).await.body;
    assert!(
        users
            .as_array()
            .unwrap()
            .iter()
            .all(|u| u["disabled"] == false),
        "{users}"
    );
    let r = call(
        &t,
        "PUT",
        "/admin/sources/tmdb/enabled",
        a,
        Some(json!({"enabled": false})),
    )
    .await;
    assert_eq!(r.status, 500, "{}", r.text);
    let switches: i64 = sqlx::query_scalar("SELECT count(*) FROM source_switches")
        .fetch_one(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(switches, 0);

    // with the audit log working again the same change goes through, with its row
    sqlx::raw_sql("DROP TRIGGER refuse_audit ON admin_audit")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let r = call(&t, "PUT", "/admin/settings", a, Some(put)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let rows = audit_rows(&t, a, "setting.changed").await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    // DEC-281: before and after values
    assert_eq!(
        (&rows[0]["detail"]["from"], &rows[0]["detail"]["to"]),
        (&json!("home"), &json!("hosted"))
    );
}

/// DEC-284: the socket address, or `X-Forwarded-For` / `X-Request-Id` only from a trusted
/// proxy; both land in the audit row and in the CSV.
#[tokio::test]
async fn client_address_and_request_id_only_through_a_trusted_proxy() {
    if skip() {
        return;
    }
    let t = start(
        "[auth]\nsignup = \"open\"\nsecure_cookies = false\n[server]\ntrusted_proxies = [\"10.0.0.0/8\", \"::1/128\"]\n",
        vec![],
    )
    .await
    .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    let put = |hosting: &str| Some(json!({"values": {"instance.hosting": hosting}}));

    // an untrusted peer: its own address, a fresh request id, the headers ignored
    let r = send(
        &t,
        "PUT",
        "/admin/settings",
        Opts {
            cookie: a,
            body: put("hosted"),
            peer: Some("192.0.2.7:5555"),
            headers: vec![
                ("x-forwarded-for", "198.51.100.9"),
                ("x-request-id", "spoofed-id"),
            ],
        },
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let own_id = r.request_id.expect("every answer names its request id");
    assert_ne!(own_id, "spoofed-id");
    let row = audit_rows(&t, a, "setting.changed").await[0].clone();
    assert_eq!(row["client_addr"], "192.0.2.7", "{row}");
    assert_eq!(row["request_id"], own_id.as_str());

    // a trusted proxy: the first untrusted address from the right, its request id
    let r = send(
        &t,
        "PUT",
        "/admin/settings",
        Opts {
            cookie: a,
            body: put("home"),
            peer: Some("10.1.2.3:443"),
            headers: vec![
                ("x-forwarded-for", "203.0.113.5, 198.51.100.9, 10.9.9.9"),
                ("x-request-id", "req-abc_123"),
            ],
        },
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.request_id.as_deref(), Some("req-abc_123"));
    let row = audit_rows(&t, a, "setting.changed").await[0].clone();
    assert_eq!(row["client_addr"], "198.51.100.9", "{row}");
    assert_eq!(row["request_id"], "req-abc_123");

    // a request id that is not a short token is replaced
    let r = send(
        &t,
        "GET",
        "/admin/settings",
        Opts {
            cookie: a,
            peer: Some("10.1.2.3:443"),
            headers: vec![("x-request-id", "bad id with spaces")],
            ..Opts::default()
        },
    )
    .await;
    assert_ne!(r.request_id.as_deref(), Some("bad id with spaces"));

    let csv = call(&t, "GET", "/admin/audit.csv", a, None).await.text;
    let header = csv.lines().next().unwrap();
    assert_eq!(
        header,
        "at,actor,action,target_type,target_id,detail,client_addr,request_id"
    );
    assert!(
        csv.lines()
            .nth(1)
            .unwrap()
            .ends_with(",198.51.100.9,req-abc_123"),
        "{csv}"
    );
}

/// A minimal SMTP server: answers every command, counts the messages it accepts.
async fn fake_smtp() -> (u16, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let sent = std::sync::Arc::new(AtomicUsize::new(0));
    let count = sent.clone();
    tokio::spawn(async move {
        while let Ok((sock, _)) = listener.accept().await {
            let count = count.clone();
            tokio::spawn(async move {
                let (r, mut w) = sock.into_split();
                let mut lines = BufReader::new(r).lines();
                w.write_all(b"220 test ESMTP\r\n").await.unwrap();
                let mut data = false;
                while let Ok(Some(l)) = lines.next_line().await {
                    let reply: &[u8] = if data {
                        if l == "." {
                            data = false;
                            count.fetch_add(1, Ordering::SeqCst);
                            b"250 queued\r\n"
                        } else {
                            continue;
                        }
                    } else if l.starts_with("EHLO") {
                        b"250-test\r\n250 AUTH PLAIN LOGIN\r\n"
                    } else if l.starts_with("DATA") {
                        data = true;
                        b"354 go on\r\n"
                    } else if l.starts_with("AUTH") {
                        b"235 ok\r\n"
                    } else if l.starts_with("QUIT") {
                        let _ = w.write_all(b"221 bye\r\n").await;
                        break;
                    } else {
                        b"250 ok\r\n"
                    };
                    if w.write_all(reply).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    (port, sent)
}

/// addendum-1 3.2 / DEC-245: SMTP lives in the database with an encrypted password, the
/// mailer changes without a restart and "send a test" is recorded for the Overview row.
#[tokio::test]
async fn smtp_in_the_database_with_an_encrypted_password_and_a_recorded_test() {
    use std::sync::atomic::Ordering;
    if skip() {
        return;
    }
    let (port, sent) = fake_smtp().await;
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    let o = call(&t, "GET", "/admin/overview", a, None).await.body;
    assert_eq!(health(&o, "mail")["status"], "off");

    let values = json!({"values": {
        "mail.host": "127.0.0.1", "mail.port": port, "mail.security": "none",
        "mail.username": "relay", "mail.from": "Shelf <noreply@example.org>"
    }});
    let r = call(&t, "PUT", "/admin/settings", a, Some(values)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = call(
        &t,
        "PUT",
        "/admin/mail/password",
        a,
        Some(json!({"password": "s3cret-pw"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert!(!r.text.contains("s3cret-pw"), "never returned: {}", r.text);
    let mail = call(&t, "GET", "/admin/mail", a, None).await;
    assert!(!mail.text.contains("s3cret-pw"), "{}", mail.text);
    assert_eq!(mail.body["password_set"], true, "{}", mail.text);
    assert_eq!(setting(&mail.body, "mail.host")["source"], "db");
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT mail_password FROM instance_settings WHERE id")
            .fetch_one(&t.srv.pool)
            .await
            .unwrap();
    assert_eq!(stored[0], 0x01, "envelope format");
    assert!(!String::from_utf8_lossy(&stored).contains("s3cret-pw"));
    let rows = audit_rows(&t, a, "secret.replaced").await;
    assert_eq!(rows[0]["detail"]["purpose"], "smtp_password", "{rows:?}");
    assert!(!rows[0].to_string().contains("s3cret-pw"));

    // the stored settings are used at once (no restart): the test mail arrives
    let r = call(
        &t,
        "POST",
        "/admin/mail/test",
        a,
        Some(json!({"to": "root@example.org"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["ok"], true, "{}", r.text);
    assert_eq!(sent.load(Ordering::SeqCst), 1);
    let mail = call(&t, "GET", "/admin/mail", a, None).await.body;
    assert_eq!(mail["last_test_result"], "ok");
    assert!(mail["last_test_at"].is_string());
    let o = call(&t, "GET", "/admin/overview", a, None).await.body;
    assert_eq!(
        (&health(&o, "mail")["status"], &health(&o, "mail")["detail"]),
        (&json!("ok"), &json!("tested"))
    );

    // a server that is not there: the failure is recorded and shown
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        Some(json!({"values": {"mail.port": 1}})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = call(
        &t,
        "POST",
        "/admin/mail/test",
        a,
        Some(json!({"to": "root@example.org"})),
    )
    .await;
    assert_eq!(
        (r.status, &r.body["ok"]),
        (200, &json!(false)),
        "{}",
        r.text
    );
    let o = call(&t, "GET", "/admin/overview", a, None).await.body;
    assert_eq!(health(&o, "mail")["status"], "error", "{o}");

    // the password can be removed again
    let r = call(
        &t,
        "PUT",
        "/admin/mail/password",
        a,
        Some(json!({"password": null})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["password_set"], false);
}

/// DEC-397 (UX3-18): a stolen admin session must not redirect reset and notification mail,
/// so the SMTP settings and the password need a fresh proof; the test mail does not.
#[tokio::test]
async fn smtp_changes_need_a_fresh_proof() {
    if skip() {
        return;
    }
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    sqlx::query(
        "UPDATE auth_sessions SET reauth_at = now() - interval '2 hours', reauth_slide_at = NULL",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();

    let host = json!({"values": {"mail.host": "evil.example.org"}});
    let r = call(&t, "PUT", "/admin/settings", a, Some(host.clone())).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("reauth_required")),
        "{}",
        r.text
    );
    let pw = json!({"password": "stolen-pw"});
    let r = call(&t, "PUT", "/admin/mail/password", a, Some(pw.clone())).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("reauth_required")),
        "{}",
        r.text
    );
    let mail = call(&t, "GET", "/admin/mail", a, None).await.body;
    assert_eq!(setting(&mail, "mail.host")["value"], Value::Null, "{mail}");
    assert_eq!(mail["password_set"], false, "{mail}");
    assert_eq!(setting(&mail, "mail.host")["needs_reauth"], true, "{mail}");
    // sending a test stays ungated (it changes nothing)
    let r = call(
        &t,
        "POST",
        "/admin/mail/test",
        a,
        Some(json!({"to": "root@example.org"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);

    let r = call(
        &t,
        "POST",
        "/auth/reauth",
        a,
        Some(json!({"password": "correct horse battery"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let fresh = r.cookie.unwrap();
    let a = Some(fresh.as_str());
    let r = call(&t, "PUT", "/admin/settings", a, Some(host)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = call(&t, "PUT", "/admin/mail/password", a, Some(pw)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let mail = call(&t, "GET", "/admin/mail", a, None).await.body;
    assert_eq!(setting(&mail, "mail.host")["value"], "evil.example.org");
    assert_eq!(mail["password_set"], true, "{mail}");
}

/// The environment's `[notify.smtp]` wins and is read-only (addendum-1 3.2).
#[tokio::test]
async fn smtp_from_the_environment_is_read_only() {
    if skip() {
        return;
    }
    let env = vec![
        (
            "APP__NOTIFY__SMTP__HOST".to_string(),
            "mail.example.org".to_string(),
        ),
        (
            "APP__NOTIFY__SMTP__FROM".to_string(),
            "x@example.org".to_string(),
        ),
        (
            "APP__NOTIFY__SMTP__PASSWORD".to_string(),
            "env-pw".to_string(),
        ),
    ];
    let t = start("[auth]\nsecure_cookies = false\n", env)
        .await
        .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    let mail = call(&t, "GET", "/admin/mail", a, None).await;
    assert!(!mail.text.contains("env-pw"), "{}", mail.text);
    let host = setting(&mail.body, "mail.host");
    assert_eq!(
        (&host["source"], &host["editable"], &host["value"]),
        (&json!("env"), &json!(false), &json!("mail.example.org"))
    );
    assert_eq!(host["env_var"], "APP__NOTIFY__SMTP__HOST");
    assert_eq!(mail.body["password_source"], "env");
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        Some(json!({"values": {"mail.host": "other.example.org"}})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("fixed_by_config"))
    );
    let r = call(
        &t,
        "PUT",
        "/admin/mail/password",
        a,
        Some(json!({"password": "x"})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("fixed_by_config"))
    );
}

/// DEC-289: the Disabled switch on a source (audited; the registry honours it at once and
/// after a restart).
#[tokio::test]
async fn a_source_is_switched_off_and_on_again() {
    if skip() {
        return;
    }
    let extra = "[auth]\nsignup = \"open\"\nsecure_cookies = false\n";
    let t = start(extra, vec![]).await.unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    let member = sign_up(&t, "mia").await;
    let m = Some(member.as_str());
    let off = Some(json!({"enabled": false}));
    let r = call(&t, "PUT", "/admin/sources/tmdb/enabled", m, off.clone()).await;
    assert_eq!(r.status, 403);
    let r = call(&t, "PUT", "/admin/sources/tmdb/enabled", a, off).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["status"]["state"], "disabled", "{}", r.text);
    assert!(!t.srv.registry.is_enabled("tmdb"));
    let r = call(&t, "GET", "/search?q=alien&kind=movie", m, None).await;
    assert!(
        !r.body["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "tmdb"),
        "{}",
        r.text
    );
    let rows = audit_rows(&t, a, "source.disabled").await;
    assert_eq!(rows[0]["target_id"], "tmdb", "{rows:?}");

    // the stored switch survives a restart
    let again = start_on(t.db.clone(), extra, vec![]).await;
    assert!(!again.srv.registry.is_enabled("tmdb"));
    let r = call(
        &again,
        "PUT",
        "/admin/sources/tmdb/enabled",
        a,
        Some(json!({"enabled": true})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_ne!(r.body["status"]["state"], "disabled");
    assert!(again.srv.registry.is_enabled("tmdb"));
    assert_eq!(audit_rows(&again, a, "source.enabled").await.len(), 1);
}

/// DEC-292: the wizard's checklist rows carry done flags from the server.
#[tokio::test]
async fn the_wizard_checklist_says_what_is_done() {
    if skip() {
        return;
    }
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let r = call(&t, "POST", "/setup/account", None, Some(account("root"))).await;
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    let st = call(&t, "GET", "/setup", a, None).await.body;
    assert_eq!(st["done"], json!([]), "{st}");
    let r = call(&t, "POST", "/admin/invites", a, Some(json!({}))).await;
    assert_eq!(r.status, 201, "{}", r.text);
    let st = call(&t, "GET", "/setup", a, None).await.body;
    assert_eq!(st["done"], json!(["invite"]), "{st}");
}

/// DEC-239: a user's time zone moves days, times and month buckets: an event at 23:30 UTC
/// on 31 January is 1 February in Europe/Berlin.
#[tokio::test]
async fn the_time_zone_moves_days_in_history_activity_and_stats() {
    if skip() {
        return;
    }
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let cookie = sign_up(&t, "root").await;
    let c = Some(cookie.as_str());
    let id = movie(&t, "Night Train").await;
    let r = call(
        &t,
        "POST",
        "/library",
        c,
        Some(json!({"item_id": id, "status": "in_progress"})),
    )
    .await;
    assert!(r.status < 300, "{}", r.text);
    let r = call(
        &t,
        "POST",
        &format!("/items/{id}/progress"),
        c,
        Some(json!({"progress": {"type": "finished"}, "occurred_at": "2026-01-31T23:30:00Z"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);

    // UTC first
    let feed = call(&t, "GET", "/activity", c, None).await.body;
    let row = &feed["rows"][0];
    assert_eq!(
        (&row["local_day"], &row["local_time"]),
        (&json!("2026-01-31"), &json!("23:30")),
        "{feed}"
    );
    let series = call(&t, "GET", "/stats/series?bucket=month", c, None)
        .await
        .body;
    assert!(series.to_string().contains("2026-01"), "{series}");

    let r = call(
        &t,
        "PUT",
        "/auth/me/time-zone",
        c,
        Some(json!({"time_zone": "Mars/Olympus"})),
    )
    .await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (422, Some("invalid_time_zone"))
    );
    let r = call(
        &t,
        "PUT",
        "/auth/me/time-zone",
        c,
        Some(json!({"time_zone": "Europe/Berlin"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["time_zone"], "Europe/Berlin");
    let me = call(&t, "GET", "/auth/me", c, None).await.body;
    assert_eq!(me["time_zone"], "Europe/Berlin", "{me}");

    let feed = call(&t, "GET", "/activity", c, None).await.body;
    let row = &feed["rows"][0];
    assert_eq!(
        (&row["local_day"], &row["local_time"]),
        (&json!("2026-02-01"), &json!("00:30")),
        "{feed}"
    );
    let series = call(&t, "GET", "/stats/series?bucket=month", c, None)
        .await
        .body;
    let text = series.to_string();
    assert!(
        text.contains("2026-02") && !text.contains("2026-01"),
        "{series}"
    );

    // back to automatic
    let r = call(
        &t,
        "PUT",
        "/auth/me/time-zone",
        c,
        Some(json!({"time_zone": null})),
    )
    .await;
    assert_eq!((r.status, &r.body["time_zone"]), (200, &Value::Null));
}

// ---------------------------------------------------------------- DEC-281 deferred tests

fn totp_code(secret_base32: &str) -> String {
    let secret = totp_rs::Secret::try_from_base32(secret_base32).unwrap();
    let totp = totp_rs::Builder::new()
        .with_secret(secret)
        .with_account_name("root")
        .build()
        .unwrap();
    totp.generate_current().to_string()
}

/// DEC-281: `POST /auth/reauth` takes a recovery code once, and wrong passwords are
/// throttled like sign-in (DEC-155).
#[tokio::test]
async fn reauth_takes_a_recovery_code_once_and_is_throttled() {
    if skip() {
        return;
    }
    let t = start(
        "[auth]\nsignup = \"open\"\nsecure_cookies = false\n",
        vec![],
    )
    .await
    .unwrap();
    let cookie = sign_up(&t, "root").await;
    let c = Some(cookie.as_str());
    let r = call(&t, "POST", "/auth/totp", c, None).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let code = totp_code(r.body["secret_base32"].as_str().unwrap());
    let r = call(
        &t,
        "POST",
        "/auth/totp/confirm",
        c,
        Some(json!({"code": code})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let recovery = r.body["codes"][0].as_str().unwrap().to_string();

    let proof = |totp: &str| Some(json!({"password": "correct horse battery", "totp": totp}));
    let r = call(&t, "POST", "/auth/reauth", c, proof(&recovery)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["fresh"], true);
    let cookie = r.cookie.expect("the session id is regenerated");
    let c = Some(cookie.as_str());
    let r = call(&t, "POST", "/auth/reauth", c, proof(&recovery)).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (401, Some("invalid_totp")),
        "a recovery code works once: {}",
        r.text
    );

    let wrong = Some(json!({"password": "not the password"}));
    let mut throttled = false;
    for _ in 0..12 {
        let r = call(&t, "POST", "/auth/reauth", c, wrong.clone()).await;
        if r.status == 429 {
            assert_eq!(r.body["code"], "too_many_attempts");
            throttled = true;
            break;
        }
        assert_eq!(r.status, 401, "{}", r.text);
    }
    assert!(throttled, "wrong passwords are throttled");
    let right = Some(json!({"password": "correct horse battery"}));
    let r = call(&t, "POST", "/auth/reauth", c, right).await;
    assert_eq!(r.status, 429, "the right password waits too: {}", r.text);
}

/// DEC-281: the wizard refuses members and signed-out visitors once an account exists, and
/// every step answers 404 after completion.
#[tokio::test]
async fn the_wizard_refuses_members_and_is_closed_after_completion() {
    if skip() {
        return;
    }
    let t = start(
        "[auth]\nsignup = \"open\"\nsecure_cookies = false\n",
        vec![],
    )
    .await
    .unwrap();
    let r = call(&t, "POST", "/setup/account", None, Some(account("root"))).await;
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    let member = sign_up(&t, "mia").await;
    let m = Some(member.as_str());
    assert_eq!(call(&t, "GET", "/setup", None, None).await.status, 401);
    for (method, path, body) in [
        ("GET", "/setup", None),
        ("POST", "/setup/hosting", Some(json!({"hosting": "hosted"}))),
        ("POST", "/setup/sources", None),
        ("POST", "/setup/complete", None),
    ] {
        let r = call(&t, method, path, m, body).await;
        assert_eq!(r.status, 403, "{method} {path} as a member: {}", r.text);
    }
    assert_eq!(
        call(&t, "POST", "/setup/complete", a, None).await.status,
        204
    );
    for (method, path, cookie, body) in [
        ("GET", "/setup", a, None),
        ("POST", "/setup/sources", a, None),
        ("POST", "/setup/complete", a, None),
        ("POST", "/setup/account", None, Some(account("eve"))),
    ] {
        let r = call(&t, method, path, cookie, body).await;
        assert_eq!(
            (r.status, r.body["code"].as_str()),
            (404, Some("setup_done")),
            "{method} {path} after completion"
        );
    }
}

/// DEC-281: settings apply without a restart, a value shadowed by env is shown, and
/// `secret.replaced` names purpose and key version, never the value.
#[tokio::test]
async fn settings_apply_at_once_shadowed_values_and_the_secret_detail() {
    if skip() {
        return;
    }
    let env = vec![(
        "APP__STORAGE__HIGH_WATERMARK".to_string(),
        "0.85".to_string(),
    )];
    let t = start("[auth]\nsecure_cookies = false\n", env)
        .await
        .unwrap();
    let r = call(&t, "POST", "/setup/account", None, Some(account("root"))).await;
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    let put = |v: Value| Some(json!({ "values": v }));
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"auth.signup": "open"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = call(&t, "POST", "/auth/sign-up", None, Some(account("mia"))).await;
    assert_eq!(r.status, 201, "open at once: {}", r.text);
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        put(json!({"auth.signup": "closed"})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = call(&t, "POST", "/auth/sign-up", None, Some(account("eve"))).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("signup_closed")),
        "closed at once"
    );

    sqlx::query("UPDATE instance_settings SET storage_high_watermark = 0.7")
        .execute(&t.srv.pool)
        .await
        .unwrap();
    let page = call(&t, "GET", "/admin/settings", a, None).await.body;
    let high = setting(&page, "storage.high_watermark");
    assert_eq!(
        (&high["value"], &high["shadowed_value"], &high["source"]),
        (&json!(0.85), &json!(0.7), &json!("env"))
    );

    let r = call(
        &t,
        "PUT",
        "/admin/sources/tmdb/credential",
        a,
        Some(json!({"fields": {"api_key": "tmdb-key-value"}})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let row = audit_rows(&t, a, "secret.replaced").await[0].clone();
    assert_eq!(
        row["detail"],
        json!({"purpose": "connector_credential", "key_version": 1})
    );
    assert!(!row.to_string().contains("tmdb-key-value"));
}

/// DEC-281: the worker registers the cron entries of the admin jobs and their handlers run
/// (audit retention deletes rows past 365 days).
#[tokio::test]
async fn the_admin_jobs_are_scheduled_and_their_handlers_run() {
    if skip() {
        return;
    }
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let r = call(&t, "POST", "/setup/account", None, Some(account("root"))).await;
    let admin = r.cookie.unwrap();
    let a = Some(admin.as_str());
    t.srv.schedule_admin_jobs().await;
    let page = call(&t, "GET", "/admin/storage-jobs", a, None).await.body;
    let names: Vec<&str> = page["cron"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"admin.audit_retention") && names.contains(&"secrets.rewrap"),
        "{page}"
    );

    sqlx::query(
        "INSERT INTO admin_audit (id, at, actor_kind, action) VALUES
         (gen_random_uuid(), now() - interval '400 days', 'system', 'old.row')",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();
    // UX3-12: expired reset links go with the same daily job
    sqlx::query(
        "INSERT INTO password_resets (id, user_id, token_hash, created_at, expires_at)
         SELECT gen_random_uuid(), id, 'expired-hash', now() - interval '2 hours',
                now() - interval '1 hour' FROM users LIMIT 1",
    )
    .execute(&t.srv.pool)
    .await
    .unwrap();
    for kind in ["admin.audit_retention", "secrets.rewrap"] {
        t.srv
            .jobs()
            .enqueue(app::ports::NewJob {
                lane: app::ports::Lane::BackgroundRefresh,
                kind: kind.into(),
                payload: json!({}),
                dedupe_key: None,
                priority: 0,
                run_at: None,
            })
            .await
            .unwrap();
    }
    let stop = tokio_util::sync::CancellationToken::new();
    let worker = tokio::spawn(t.srv.worker().run(stop.clone()));
    let mut gone = false;
    for _ in 0..100 {
        let n: i64 =
            sqlx::query_scalar("SELECT count(*) FROM admin_audit WHERE action = 'old.row'")
                .fetch_one(&t.srv.pool)
                .await
                .unwrap();
        let open: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM jobs WHERE kind IN ('admin.audit_retention', 'secrets.rewrap')
               AND state <> 'done'",
        )
        .fetch_one(&t.srv.pool)
        .await
        .unwrap_or(1);
        if n == 0 && open == 0 {
            gone = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    stop.cancel();
    let _ = worker.await;
    assert!(
        gone,
        "the retention job deleted the old row and both jobs finished"
    );
    let resets: i64 = sqlx::query_scalar("SELECT count(*) FROM password_resets")
        .fetch_one(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(resets, 0, "expired reset links are pruned");
}

/// UX3-18: settings that store fine but give no usable mail transport answer with a warning
/// (mail is off), instead of a silent 200.
#[tokio::test]
async fn a_mail_transport_that_cannot_be_built_is_reported() {
    if skip() {
        return;
    }
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    let values = json!({"values": {
        "mail.host": "127.0.0.1", "mail.port": 2525, "mail.security": "none",
        "mail.from": "Shelf <noreply@@example.org>"
    }});
    let r = call(&t, "PUT", "/admin/settings", a, Some(values)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let warnings: Vec<&str> = r.body["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(warnings.contains(&"mail_transport_unusable"), "{}", r.text);
    // a good sender clears it
    let values = json!({"values": {"mail.from": "Shelf <noreply@example.org>"}});
    let r = call(&t, "PUT", "/admin/settings", a, Some(values)).await;
    assert!(!r.text.contains("mail_transport_unusable"), "{}", r.text);
}

/// DEC-383 (deferred to WP4-8): `mail.*` values are checked (host, port, security, sender),
/// a refused value changes nothing, and a test mail without an address to an admin who has
/// none is `no_recipient`.
#[tokio::test]
async fn mail_settings_are_validated_and_a_test_needs_a_recipient() {
    if skip() {
        return;
    }
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    for (key, value) in [
        ("mail.host", json!("smtp example.org")),
        ("mail.port", json!(0)),
        ("mail.port", json!(70_000)),
        ("mail.port", json!("25")),
        ("mail.security", json!("ssl")),
        ("mail.from", json!("nobody")),
    ] {
        let r = call(
            &t,
            "PUT",
            "/admin/settings",
            a,
            Some(json!({"values": {key: value}})),
        )
        .await;
        assert_eq!(
            (r.status, r.body["code"].as_str()),
            (422, Some("invalid_setting")),
            "{key} = {value}: {}",
            r.text
        );
    }
    let mail = call(&t, "GET", "/admin/mail", a, None).await.body;
    for key in ["mail.host", "mail.port", "mail.security", "mail.from"] {
        assert_ne!(
            setting(&mail, key)["source"],
            "db",
            "{key} unchanged: {mail}"
        );
    }
    for body in [json!({}), json!({"to": "  "})] {
        let r = call(&t, "POST", "/admin/mail/test", a, Some(body)).await;
        assert_eq!(
            (r.status, r.body["code"].as_str()),
            (422, Some("no_recipient")),
            "{}",
            r.text
        );
    }
}

async fn mail_test(t: &T, cookie: Option<&str>) -> Resp {
    call(
        t,
        "POST",
        "/admin/mail/test",
        cookie,
        Some(json!({"to": "root@example.org"})),
    )
    .await
}

async fn mail_at(t: &T, cookie: Option<&str>, port: u16) {
    let values = json!({"values": {
        "mail.host": "127.0.0.1", "mail.port": port, "mail.security": "none",
        "mail.from": "Shelf <noreply@example.org>"
    }});
    let r = call(t, "PUT", "/admin/settings", cookie, Some(values)).await;
    assert_eq!(r.status, 200, "{}", r.text);
}

/// sec2 (SEC-4, DEC-534): a mail server on a private address is used on a home instance and
/// refused on a hosted one, and a failed test answers with its category only.
#[tokio::test]
async fn a_mail_server_on_a_private_address_follows_the_hosting_mode() {
    use std::sync::atomic::Ordering;
    if skip() {
        return;
    }
    let (port, sent) = fake_smtp().await;
    let t = start("[auth]\nsecure_cookies = false\n", vec![])
        .await
        .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    mail_at(&t, a, port).await;
    let r = mail_test(&t, a).await;
    assert_eq!(r.body["ok"], true, "home: {}", r.text);
    assert_eq!(sent.load(Ordering::SeqCst), 1);

    let hosted = json!({"values": {"instance.hosting": "hosted"}});
    let r = call(&t, "PUT", "/admin/settings", a, Some(hosted)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = mail_test(&t, a).await;
    assert_eq!(
        (&r.body["ok"], &r.body["result"]),
        (&json!(false), &json!("failed: private_target")),
        "hosted: {}",
        r.text
    );
    assert_eq!(sent.load(Ordering::SeqCst), 1, "not connected to");
    let mail = call(&t, "GET", "/admin/mail", a, None).await.body;
    assert_eq!(mail["last_test_result"], "failed: private_target");

    // back at home, a server that is not there: the category, not the transport's text
    let home = json!({"values": {"instance.hosting": "home", "mail.port": 1}});
    let r = call(&t, "PUT", "/admin/settings", a, Some(home)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = mail_test(&t, a).await;
    assert_eq!(r.body["result"], "failed: connection", "{}", r.text);
    assert!(!r.text.contains("127.0.0.1"), "{}", r.text);
}

/// sec2 (DEC-534): `notify.allow_private_targets` overrides the hosting mode in both
/// directions.
#[tokio::test]
async fn the_private_target_switch_overrides_the_hosting_mode() {
    use std::sync::atomic::Ordering;
    if skip() {
        return;
    }
    let (port, sent) = fake_smtp().await;
    let t = start(
        "[auth]\nsecure_cookies = false\n[instance]\nhosting = \"hosted\"\n[notify]\nallow_private_targets = true\n",
        vec![],
    )
    .await
    .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    mail_at(&t, a, port).await;
    let r = mail_test(&t, a).await;
    assert_eq!(r.body["ok"], true, "hosted, allowed: {}", r.text);
    assert_eq!(sent.load(Ordering::SeqCst), 1);

    let t = start(
        "[auth]\nsecure_cookies = false\n[notify]\nallow_private_targets = false\n",
        vec![],
    )
    .await
    .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    mail_at(&t, a, port).await;
    let r = mail_test(&t, a).await;
    assert_eq!(
        r.body["result"], "failed: private_target",
        "home, refused: {}",
        r.text
    );
    assert_eq!(sent.load(Ordering::SeqCst), 1);
}

async fn sign_in_from(
    t: &T,
    peer: &str,
    forwarded: Option<&str>,
    uri: &str,
    name: &str,
    password: &str,
) -> Resp {
    send(
        t,
        "POST",
        uri,
        Opts {
            body: Some(json!({"username": name, "password": password})),
            peer: Some(peer),
            headers: forwarded
                .map(|f| ("x-forwarded-for", f))
                .into_iter()
                .collect(),
            ..Opts::default()
        },
    )
    .await
}

/// sec2 (SEC-5, DEC-535): the wait belongs to the account and the client address behind the
/// trusted proxy; the local sign-in stays open for the owner while someone guesses at the
/// admin name from elsewhere, and an address over its budget is refused for every name.
#[tokio::test]
async fn sign_in_is_slowed_per_account_and_client_address() {
    if skip() {
        return;
    }
    let t = start(
        "[auth]\nsignup = \"open\"\nsecure_cookies = false\n[auth.throttle]\naddress_failures = 6\n\
         [server]\ntrusted_proxies = [\"10.0.0.0/8\"]\n",
        vec![],
    )
    .await
    .unwrap();
    let _ = sign_up(&t, "root").await;
    let proxy = "10.0.0.1:4000";
    let attacker = Some("198.51.100.9");
    let owner = Some("203.0.113.7");
    let local = "/auth/sign-in/local";
    let good = "correct horse battery";

    // three free failures, then the pair waits: 429 with the seconds, also for the right
    // password, on both doors
    for _ in 0..4 {
        let r = sign_in_from(&t, proxy, attacker, local, "root", "wrong password!").await;
        assert_eq!(r.status, 401, "{}", r.text);
    }
    for uri in [local, "/auth/sign-in"] {
        let r = sign_in_from(&t, proxy, attacker, uri, "root", good).await;
        assert_eq!(r.status, 429, "{uri}: {}", r.text);
        assert_eq!(r.body["code"], "too_many_attempts");
        assert_eq!(r.body["retry_after_secs"], 1, "{}", r.text);
    }
    // the owner, behind the same proxy but from another address, is not slowed
    let r = sign_in_from(&t, proxy, owner, local, "root", good).await;
    assert_eq!(r.status, 200, "{}", r.text);
    // and the break-glass sign-in is still recorded
    let rows = audit_rows(&t, r.cookie.as_deref(), "auth.local_sign_in").await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["client_addr"], "203.0.113.7");

    // the attacker's address runs out of budget (6 failures) across other names
    for n in 0..2 {
        let name = format!("guess{n}");
        let r = sign_in_from(
            &t,
            proxy,
            attacker,
            "/auth/sign-in",
            &name,
            "wrong password!",
        )
        .await;
        assert_eq!(r.status, 401, "{}", r.text);
    }
    let r = sign_in_from(
        &t,
        proxy,
        attacker,
        "/auth/sign-in",
        "another",
        "wrong password!",
    )
    .await;
    assert_eq!(r.status, 429, "{}", r.text);
    assert!(
        r.body["retry_after_secs"].as_u64().unwrap() > 60,
        "{}",
        r.text
    );
    // an address that names itself in the header without coming through the proxy is the
    // socket address: it cannot borrow someone else's budget or escape its own
    let r = sign_in_from(&t, "198.51.100.9:999", owner, "/auth/sign-in", "root", good).await;
    assert_eq!(r.status, 429, "{}", r.text);
    let r = sign_in_from(&t, proxy, owner, "/auth/sign-in", "root", good).await;
    assert_eq!(r.status, 200, "{}", r.text);
}
