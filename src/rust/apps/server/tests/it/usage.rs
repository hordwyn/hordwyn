//! WP4-5 logging funnel on the server (design.md 9, research R-54, addendum-1 D16): the
//! beacon `POST /usage/funnel` is aggregated per day without any user or item, refused when
//! signed out, rate limited, dropped while the instance setting is off; the daily job emits
//! one `metrics` event per flow and deletes days past the retention; the admin report and its
//! CSV.

// End-to-end scenarios on purpose: the steps share one server and database; the short
// names (t: server, a/m: admin/member cookie, r: answer) keep the many calls readable.
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

use crate::support;

use serde_json::{Value, json};
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

struct T {
    app: axum::Router,
    srv: Server,
}

async fn start(env: Vec<(String, String)>) -> Option<T> {
    let db = store_pg::testing::fresh_db_url().await?;
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [brand]\ndir = \"{}\"\n[auth]\nsignup = \"open\"\nsecure_cookies = false\n",
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
    let resp = support::send(&t.app, req.body(axum::body::Body::from(body)).unwrap()).await;
    let status = resp.status().as_u16();
    let cookie = resp
        .headers()
        .get("set-cookie")
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string());
    let bytes = resp.into_body();
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        cookie,
        text: String::from_utf8_lossy(&bytes).into_owned(),
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

async fn sign_up(t: &T, name: &str) -> String {
    let r = call(
        t,
        "POST",
        "/auth/sign-up",
        None,
        Some(json!({"username": name, "display_name": name, "password": "correct horse battery"})),
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.body);
    r.cookie.unwrap()
}

// the body as `call` takes it
#[allow(clippy::unnecessary_wraps)]
fn beacon(events: Value) -> Option<Value> {
    let mut body = serde_json::Map::new();
    body.insert("events".into(), events);
    Some(Value::Object(body))
}

/// Every stored aggregate row, ordered.
async fn rows(t: &T) -> Vec<(String, String, String, String, String, i64)> {
    sqlx::query_as::<_, (String, String, String, String, String, i64)>(
        "SELECT flow, subject, outcome, taps_bucket, ms_bucket, n FROM usage_funnel_daily
         ORDER BY flow, subject, outcome, taps_bucket, ms_bucket",
    )
    .fetch_all(&t.srv.pool)
    .await
    .unwrap()
}

fn row(
    flow: &str,
    subject: &str,
    outcome: &str,
    taps: &str,
    ms: &str,
    n: i64,
) -> (String, String, String, String, String, i64) {
    (
        flow.into(),
        subject.into(),
        outcome.into(),
        taps.into(),
        ms.into(),
        n,
    )
}

#[tokio::test]
async fn the_funnel_is_counted_per_day_without_users_or_items() {
    if skip() {
        return;
    }
    let t = start(vec![]).await.unwrap();
    let admin = sign_up(&t, "root").await;
    let member = sign_up(&t, "mia").await;
    let (a, m) = (Some(admin.as_str()), Some(member.as_str()));

    // the table has no column that could name a person or an item
    let cols: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns
         WHERE table_name = 'usage_funnel_daily' ORDER BY ordinal_position",
    )
    .fetch_all(&t.srv.pool)
    .await
    .unwrap();
    assert_eq!(
        cols,
        [
            "day",
            "flow",
            "subject",
            "outcome",
            "taps_bucket",
            "ms_bucket",
            "n"
        ],
        "exactly the aggregate columns"
    );
    for c in &cols {
        assert!(
            !c.contains("user") && !c.contains("item") && !c.contains("ip"),
            "{c}"
        );
    }

    // signed out: refused
    let one = json!([{"flow": "next_step", "outcome": "logged", "taps": "1", "time": "lt2s"}]);
    assert_eq!(
        call(&t, "POST", "/usage/funnel", None, beacon(one.clone()))
            .await
            .status,
        401
    );

    // two people, the same buckets: one row with n = 2 and nothing about who
    let r = call(&t, "POST", "/usage/funnel", m, beacon(one.clone())).await;
    assert_eq!(r.status, 204, "{}", r.text);
    let r = call(
        &t,
        "POST",
        "/usage/funnel",
        a,
        beacon(json!([
            {"flow": "next_step", "outcome": "logged", "taps": "1", "time": "lt2s"},
            {"flow": "sheet", "outcome": "abandoned", "taps": "5+", "time": "60s+"},
            {"flow": "disclosure", "outcome": "open_at_save", "subject": "item/log-more"},
            {"flow": "disclosure", "outcome": "opened_early", "subject": "item/log-more"}
        ])),
    )
    .await;
    assert_eq!(r.status, 204, "{}", r.text);
    assert_eq!(
        rows(&t).await,
        vec![
            row("disclosure", "item/log-more", "open_at_save", "", "", 1),
            row("disclosure", "item/log-more", "opened_early", "", "", 1),
            row("next_step", "", "logged", "1", "lt2s", 2),
            row("sheet", "", "abandoned", "5+", "60s+", 1),
        ]
    );
    let day: chrono::NaiveDate = sqlx::query_scalar("SELECT DISTINCT day FROM usage_funnel_daily")
        .fetch_one(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(
        day,
        chrono::Utc::now().date_naive(),
        "the server's day (UTC)"
    );

    // what is refused: an item id, a free subject, unknown flows, outcomes and buckets
    for bad in [
        json!([{"flow": "next_step", "outcome": "logged", "taps": "1", "time": "lt2s", "item_id": "x"}]),
        json!([{"flow": "disclosure", "outcome": "opened", "subject": "item/0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"}]),
        json!([{"flow": "disclosure", "outcome": "opened", "subject": "Harbour Lights"}]),
        // a well-formed section nobody has (UX4-14: a fixed list bounds the rows)
        json!([{"flow": "disclosure", "outcome": "opened", "subject": "made/up"}]),
        json!([{"flow": "next_step", "outcome": "logged", "taps": "1", "time": "lt2s", "subject": "item/log-more"}]),
        json!([{"flow": "search", "outcome": "logged", "taps": "1", "time": "lt2s"}]),
        json!([{"flow": "next_step", "outcome": "opened", "taps": "1", "time": "lt2s"}]),
        json!([{"flow": "next_step", "outcome": "logged", "taps": "7", "time": "lt2s"}]),
        json!([{"flow": "next_step", "outcome": "logged", "taps": "1", "time": "812ms"}]),
    ] {
        let r = call(&t, "POST", "/usage/funnel", m, beacon(bad.clone())).await;
        assert!(
            r.status == 400 || r.status == 422,
            "{bad} -> {} {}",
            r.status,
            r.text
        );
    }
    let many: Vec<Value> = (0..51).map(|_| one[0].clone()).collect();
    assert_eq!(
        call(&t, "POST", "/usage/funnel", m, beacon(Value::Array(many)))
            .await
            .status,
        422,
        "at most 50 events per beacon"
    );
    assert_eq!(rows(&t).await.len(), 4, "refused beacons store nothing");

    // the admin report and its CSV; members get neither
    let r = call(&t, "GET", "/admin/usage", a, None).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["enabled"], true);
    assert_eq!(r.body["retention_days"], 180);
    assert_eq!(r.body["rows"].as_array().unwrap().len(), 4, "{}", r.body);
    assert!(
        r.body.to_string().find("mia").is_none() && r.body.to_string().find("root").is_none(),
        "no names in the report"
    );
    let r = call(&t, "GET", "/admin/usage.csv", a, None).await;
    assert_eq!(r.status, 200);
    let mut lines = r.text.lines();
    assert_eq!(
        lines.next(),
        Some("day,flow,subject,outcome,taps_bucket,ms_bucket,n")
    );
    assert_eq!(lines.count(), 4);
    assert_eq!(call(&t, "GET", "/admin/usage", m, None).await.status, 403);
    assert_eq!(
        call(&t, "GET", "/admin/usage.csv", m, None).await.status,
        403
    );
}

#[tokio::test]
async fn beacons_are_rate_limited_and_dropped_while_the_setting_is_off() {
    if skip() {
        return;
    }
    let t = start(vec![]).await.unwrap();
    let admin = sign_up(&t, "root").await;
    let member = sign_up(&t, "mia").await;
    let (a, m) = (Some(admin.as_str()), Some(member.as_str()));
    let one = json!([{"flow": "palette", "outcome": "abandoned", "taps": "2", "time": "lt5s"}]);

    // a person may send 30 beacons a minute; the next one is refused, others still count
    for i in 0..30 {
        let r = call(&t, "POST", "/usage/funnel", m, beacon(one.clone())).await;
        assert_eq!(r.status, 204, "beacon {i}: {}", r.text);
    }
    let r = call(&t, "POST", "/usage/funnel", m, beacon(one.clone())).await;
    assert_eq!(r.status, 429, "{}", r.text);
    assert_eq!(r.body["code"], "too_many_attempts");
    assert_eq!(
        call(&t, "POST", "/usage/funnel", a, beacon(one.clone()))
            .await
            .status,
        204
    );
    assert_eq!(
        rows(&t).await,
        vec![row("palette", "", "abandoned", "2", "lt5s", 31)]
    );

    // the setting: on by default, editable, then off -> answered and dropped
    let r = call(&t, "GET", "/admin/settings", a, None).await;
    let s = r.body["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "usage.funnel")
        .cloned()
        .expect("usage.funnel in the settings");
    assert_eq!(
        (&s["value"], &s["source"], &s["editable"]),
        (&json!(true), &json!("default"), &json!(true))
    );
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        Some(json!({"values": {"usage.funnel": false}})),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = call(&t, "POST", "/usage/funnel", a, beacon(one.clone())).await;
    assert_eq!(r.status, 204, "{}", r.text);
    assert_eq!(
        rows(&t).await,
        vec![row("palette", "", "abandoned", "2", "lt5s", 31)]
    );
    assert_eq!(
        call(&t, "GET", "/admin/usage", a, None).await.body["enabled"],
        false
    );
    // the storage page carries the switch with its provenance
    let r = call(&t, "GET", "/admin/storage-jobs", a, None).await;
    assert!(
        r.body["settings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["key"] == "usage.funnel"),
        "{}",
        r.body
    );
}

#[tokio::test]
async fn the_environment_fixes_the_setting() {
    if skip() {
        return;
    }
    let t = start(vec![("APP__USAGE__FUNNEL".into(), "false".into())])
        .await
        .unwrap();
    let admin = sign_up(&t, "root").await;
    let a = Some(admin.as_str());
    let r = call(&t, "GET", "/admin/settings", a, None).await;
    let s = r.body["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "usage.funnel")
        .cloned()
        .unwrap();
    assert_eq!(
        (&s["value"], &s["source"], &s["env_var"], &s["editable"]),
        (
            &json!(false),
            &json!("env"),
            &json!("APP__USAGE__FUNNEL"),
            &json!(false)
        )
    );
    let r = call(
        &t,
        "PUT",
        "/admin/settings",
        a,
        Some(json!({"values": {"usage.funnel": true}})),
    )
    .await;
    assert_eq!(r.status, 409, "{}", r.text);
    let one = json!([{"flow": "bulk", "outcome": "logged", "taps": "3", "time": "lt10s"}]);
    assert_eq!(
        call(&t, "POST", "/usage/funnel", a, beacon(one))
            .await
            .status,
        204
    );
    assert_eq!(rows(&t).await.len(), 0, "off by env: nothing stored");
}

#[tokio::test]
async fn the_daily_job_reports_each_flow_and_keeps_180_days() {
    if skip() {
        return;
    }
    let t = start(vec![]).await.unwrap();
    let today = chrono::Utc::now().date_naive();
    for (days, flow, outcome, n) in [
        (1_i64, "next_step", "logged", 7_i64),
        (1, "next_step", "abandoned", 2),
        (1, "sheet", "logged", 3),
        (180, "palette", "logged", 1),
        (181, "palette", "logged", 1),
        (400, "bulk", "logged", 1),
    ] {
        sqlx::query(
            "INSERT INTO usage_funnel_daily (day, flow, subject, outcome, taps_bucket, ms_bucket, n)
             VALUES ($1, $2, '', $3, '1', 'lt2s', $4)",
        )
        .bind(today - chrono::Duration::days(days))
        .bind(flow)
        .bind(outcome)
        .bind(n)
        .execute(&t.srv.pool)
        .await
        .unwrap();
    }
    let report = api::usage::daily(&t.srv.api, today).await.unwrap();
    // one line per flow of yesterday, outcomes summed over the buckets
    assert_eq!(
        report.flows,
        vec![
            api::usage::FlowTotals {
                flow: "next_step".into(),
                subject: String::new(),
                outcomes: vec![("abandoned".into(), 2), ("logged".into(), 7)],
            },
            api::usage::FlowTotals {
                flow: "sheet".into(),
                subject: String::new(),
                outcomes: vec![("logged".into(), 3)],
            },
        ]
    );
    assert_eq!(report.deleted, 2, "the days past 180 are gone");
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM usage_funnel_daily")
        .fetch_one(&t.srv.pool)
        .await
        .unwrap();
    assert_eq!(left, 4);
    // the cron entry exists
    t.srv.schedule_admin_jobs().await;
    let crons: Vec<String> = sqlx::query_scalar("SELECT name FROM cron_schedules")
        .fetch_all(&t.srv.pool)
        .await
        .unwrap();
    assert!(crons.iter().any(|c| c == "usage.funnel_daily"), "{crons:?}");
}
