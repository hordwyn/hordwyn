//! sec1 (SEC-1, DEC-536): every answer of the server carries the security headers, pages a
//! content security policy whose nonce is on every inline script and style of the shell,
//! and answers that depend on the session are not stored by a cache.

use crate::support;

use leptos::server_fn::ServerFn;
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
    app: axum::Router,
    dir: std::path::PathBuf,
}

/// A server whose `[server]` table is `server` and whose site root has one bundle file and
/// one plain file.
async fn start(server: &str) -> T {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let dir = std::env::temp_dir().join(format!("sec1-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(dir.join("site/pkg")).unwrap();
    std::fs::write(dir.join("site/pkg/app.js"), "export default 1;").unwrap();
    std::fs::write(dir.join("site/plain.txt"), "plain").unwrap();
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n\
         [search]\nbackend = \"postgres\"\n[brand]\ndir = \"{}\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[server]\n{server}\n",
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
    T {
        app: srv.app_router(&options),
        dir,
    }
}

struct Resp {
    status: u16,
    headers: axum::http::HeaderMap,
    text: String,
}

impl Resp {
    fn header(&self, name: &str) -> Option<&str> {
        let mut all = self.headers.get_all(name).iter();
        let first = all.next()?;
        assert!(all.next().is_none(), "{name} is sent once");
        Some(first.to_str().unwrap())
    }

    fn policy(&self) -> &str {
        self.header("content-security-policy")
            .unwrap_or_else(|| panic!("no policy on {} {}", self.status, self.text))
    }

    /// The nonce the policy allows scripts with.
    fn nonce(&self) -> String {
        let p = self.policy();
        let at = p.find("'nonce-").expect("a nonce in the policy") + "'nonce-".len();
        p[at..at + p[at..].find('\'').unwrap()].to_string()
    }
}

async fn call(t: &T, method: &str, uri: &str, extra: &[(&str, &str)], body: &str) -> Resp {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("accept-language", "en");
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let resp = support::send(
        &t.app,
        req.body(axum::body::Body::from(body.to_string())).unwrap(),
    )
    .await;
    let status = resp.status().as_u16();
    let headers = resp.headers().clone();
    Resp {
        status,
        headers,
        text: String::from_utf8_lossy(&resp.into_body()).into_owned(),
    }
}

/// The head of an answer whose body never ends (the event stream): the headers are sent
/// before the first event, so the body is not read.
async fn head(t: &T, uri: &str, cookie: &str) -> Resp {
    use tower::ServiceExt;
    let req = axum::http::Request::builder()
        .uri(uri)
        .header("cookie", cookie)
        .body(axum::body::Body::empty())
        .unwrap();
    let limit = std::time::Duration::from_secs(30);
    let resp = tokio::time::timeout(limit, t.app.clone().oneshot(req))
        .await
        .expect("the head of the answer arrives")
        .unwrap();
    Resp {
        status: resp.status().as_u16(),
        headers: resp.headers().clone(),
        text: String::new(),
    }
}

async fn get(t: &T, uri: &str, cookie: Option<&str>) -> Resp {
    match cookie {
        Some(c) => call(t, "GET", uri, &[("cookie", c)], "").await,
        None => call(t, "GET", uri, &[], "").await,
    }
}

async fn sign_up(t: &T, name: &str) -> String {
    let body = format!(
        r#"{{"username":"{name}","display_name":"{name}","password":"correct horse battery"}}"#
    );
    let r = call(
        t,
        "POST",
        "/api/v1/auth/sign-up",
        &[("content-type", "application/json")],
        &body,
    )
    .await;
    assert_eq!(r.status, 201, "{}", r.text);
    let cookie = r.header("set-cookie").unwrap();
    cookie.split(';').next().unwrap().to_string()
}

fn fn_path<F: ServerFn>() -> String {
    F::PATH.to_string()
}

/// Every opening tag `<name ...>` of `html`, attributes included.
fn tags<'a>(html: &'a str, name: &str) -> Vec<&'a str> {
    let open = format!("<{name}");
    let mut found = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find(&open) {
        let tail = &rest[at..];
        let end = tail.find('>').expect("the tag ends");
        // `<script` also starts `<scripted`: the name ends at a space or the bracket
        if matches!(tail.as_bytes()[open.len()], b' ' | b'>') {
            found.push(&tail[..=end]);
        }
        rest = &tail[end..];
    }
    found
}

/// The headers that do not depend on the kind of answer.
fn assert_fixed_headers(r: &Resp, what: &str) {
    assert_eq!(
        r.header("x-content-type-options"),
        Some("nosniff"),
        "{what}"
    );
    assert_eq!(r.header("x-frame-options"), Some("DENY"), "{what}");
    assert_eq!(r.header("referrer-policy"), Some("same-origin"), "{what}");
    let permissions = r.header("permissions-policy").unwrap_or_default();
    for feature in ["camera=()", "geolocation=()", "microphone=()", "payment=()"] {
        assert!(permissions.contains(feature), "{what}: {permissions}");
    }
    assert!(
        r.policy().contains("frame-ancestors 'none'"),
        "{what}: {}",
        r.policy()
    );
    assert_eq!(
        r.header("content-security-policy-report-only"),
        None,
        "{what}"
    );
    assert_eq!(
        r.header("strict-transport-security"),
        None,
        "{what}: the public URL is plain http"
    );
}

/// A page, a REST answer, a server function, an asset of the bundle, a plain file, the
/// health probe, the 404 page and the refusals that never reach a handler.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_answer_carries_the_security_headers() {
    if !services() {
        return;
    }
    let t = start("").await;
    // review SEC1-9: a redirect (before the first account every page leads to the wizard)
    let to_setup = get(&t, "/", None).await;
    assert_eq!(to_setup.header("location"), Some("/setup"));
    let anna = sign_up(&t, "anna").await;
    let current_user = fn_path::<web::api::CurrentUser>();
    let json = [("content-type", "application/json")];
    let cross = [
        ("origin", "https://evil.example"),
        ("sec-fetch-site", "cross-site"),
    ];
    let huge = [("content-length", "99999999")];
    let answers = [
        ("redirect", to_setup, 303),
        ("event stream", head(&t, "/api/v1/events", &anna).await, 200),
        ("page", get(&t, "/", Some(&anna)).await, 200),
        ("signed-out page", get(&t, "/sign-in", None).await, 200),
        ("404 page", get(&t, "/no/such/page", None).await, 404),
        ("rest", get(&t, "/api/v1/auth/me", Some(&anna)).await, 200),
        ("rest 401", get(&t, "/api/v1/auth/me", None).await, 401),
        ("rest 404", get(&t, "/api/v1/nothing", None).await, 404),
        (
            "server function",
            call(&t, "POST", &current_user, &[("cookie", &anna)], "").await,
            200,
        ),
        ("bundle", get(&t, "/pkg/app.js", None).await, 200),
        ("plain file", get(&t, "/plain.txt", None).await, 200),
        ("health", get(&t, "/healthz", None).await, 200),
        (
            "cross-site refusal",
            call(&t, "POST", "/api/v1/auth/sign-in", &cross, "{}").await,
            403,
        ),
        (
            "too large",
            call(&t, "POST", "/sign-in", &huge, "").await,
            413,
        ),
        (
            "wrong method",
            call(&t, "DELETE", "/healthz", &json, "").await,
            405,
        ),
    ];
    for (what, r, status) in &answers {
        assert_eq!(r.status, *status, "{what}: {}", r.text);
        assert_fixed_headers(r, what);
    }
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// The policy of a page: scripts and styles from this origin and with the nonce of this
/// answer only, WebAssembly may be compiled, pictures from any https host (DEC-640), no
/// plugins, no `<base>`, forms post to this origin.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_page_gets_the_page_policy_and_other_answers_a_closed_one() {
    if !services() {
        return;
    }
    let t = start("").await;
    let anna = sign_up(&t, "anna").await;
    let page = get(&t, "/", Some(&anna)).await;
    let nonce = page.nonce();
    assert!(nonce.len() >= 22, "128 bits, base64: {nonce}");
    let directives: Vec<&str> = page.policy().split("; ").collect();
    for d in [
        "default-src 'self'".to_string(),
        format!("script-src 'self' 'nonce-{nonce}' 'wasm-unsafe-eval'"),
        format!("style-src 'self' 'nonce-{nonce}'"),
        "style-src-attr 'unsafe-inline'".to_string(),
        // the test instance runs on plain http, where a picture may come over http too
        "img-src 'self' data: https: http:".to_string(),
        "font-src 'self'".to_string(),
        "connect-src 'self'".to_string(),
        "object-src 'none'".to_string(),
        "base-uri 'none'".to_string(),
        "form-action 'self'".to_string(),
        "frame-ancestors 'none'".to_string(),
    ] {
        assert!(directives.contains(&d.as_str()), "{d} in {directives:?}");
    }
    for never in ["'unsafe-eval'", "script-src 'self' 'unsafe-inline'", " *"] {
        assert!(!page.policy().contains(never), "{never}: {}", page.policy());
    }
    // a nonce is used once
    let again = get(&t, "/", Some(&anna)).await;
    assert_ne!(again.nonce(), nonce);
    // nothing but a document runs scripts: other answers allow nothing at all
    for uri in ["/api/v1/auth/me", "/pkg/app.js", "/healthz"] {
        let r = get(&t, uri, Some(&anna)).await;
        assert_eq!(
            r.policy(),
            "default-src 'none'; frame-ancestors 'none'",
            "{uri}"
        );
    }
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// "Every inline script of the rendered shell carries the response's nonce" (and every
/// inline style): signed out, signed in (with the data the page streams after the shell)
/// and on the 404 page.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_inline_script_and_style_carries_the_nonce_of_the_answer() {
    if !services() {
        return;
    }
    let t = start("").await;
    let anna = sign_up(&t, "anna").await;
    for (uri, cookie) in [
        ("/sign-in", None),
        ("/", Some(anna.as_str())),
        ("/library", Some(anna.as_str())),
        ("/stats", Some(anna.as_str())),
        ("/admin", Some(anna.as_str())),
        ("/no/such/page", None),
    ] {
        let r = get(&t, uri, cookie).await;
        let attr = format!("nonce=\"{}\"", r.nonce());
        let scripts = tags(&r.text, "script");
        assert!(
            scripts.len() >= 3,
            "{uri}: theme, config, hydration: {scripts:?}"
        );
        let mut with_nonce = 0;
        for tag in &scripts {
            // a data block is not run and needs no nonce
            if tag.contains("type=\"application/json\"") {
                assert!(!tag.contains("nonce"), "{uri}: {tag}");
                continue;
            }
            assert!(tag.contains(&attr), "{uri}: {tag} lacks {attr}");
            with_nonce += 1;
        }
        assert!(
            with_nonce >= 2,
            "{uri}: theme and hydration script: {scripts:?}"
        );
        let styles = tags(&r.text, "style");
        assert_eq!(styles.len(), 1, "{uri}: the brand theme: {styles:?}");
        assert!(
            styles[0].contains(&attr),
            "{uri}: {} lacks {attr}",
            styles[0]
        );
        // no handler attributes and no script addresses
        for never in [" onclick=", " onload=", " onerror=", "javascript:"] {
            assert!(!r.text.contains(never), "{uri}: {never}");
        }
    }
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// HSTS only when the public URL is https, with the configured age; `0` switches it off.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hsts_follows_the_public_url() {
    if !services() {
        return;
    }
    let https = "public_url = \"https://tracker.example.org\"";
    let t = start(https).await;
    // with an account: before the first one every page leads to the setup wizard
    sign_up(&t, "anna").await;
    for uri in ["/sign-in", "/api/v1/auth/me", "/healthz", "/pkg/app.js"] {
        let r = get(&t, uri, None).await;
        assert_eq!(
            r.header("strict-transport-security"),
            Some("max-age=31536000"),
            "{uri}"
        );
    }
    // over https a picture comes over https only (a browser upgrades the others itself)
    let page = get(&t, "/sign-in", None).await;
    assert!(
        page.policy().contains("img-src 'self' data: https:;"),
        "{}",
        page.policy()
    );
    std::fs::remove_dir_all(&t.dir).unwrap();

    let t = start(&format!("{https}\nhsts_max_age_secs = 600")).await;
    let r = get(&t, "/healthz", None).await;
    assert_eq!(r.header("strict-transport-security"), Some("max-age=600"));
    std::fs::remove_dir_all(&t.dir).unwrap();

    let t = start(&format!("{https}\nhsts_max_age_secs = 0")).await;
    let r = get(&t, "/healthz", None).await;
    assert_eq!(r.header("strict-transport-security"), None);
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// `server.csp_report_only`: the same policy under the header a browser only reports on;
/// the other headers stay (the frame rule then rests on `X-Frame-Options`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_policy_can_be_sent_report_only() {
    if !services() {
        return;
    }
    let t = start("csp_report_only = true").await;
    sign_up(&t, "anna").await;
    for uri in ["/sign-in", "/api/v1/auth/me"] {
        let r = get(&t, uri, None).await;
        assert_eq!(r.header("content-security-policy"), None, "{uri}");
        let policy = r.header("content-security-policy-report-only").unwrap();
        assert!(policy.contains("frame-ancestors 'none'"), "{uri}: {policy}");
        assert_eq!(r.header("x-frame-options"), Some("DENY"), "{uri}");
        assert_eq!(r.header("x-content-type-options"), Some("nosniff"), "{uri}");
    }
    let page = get(&t, "/sign-in", None).await;
    let policy = page
        .header("content-security-policy-report-only")
        .unwrap()
        .to_string();
    let at = policy.find("'nonce-").unwrap() + "'nonce-".len();
    let nonce = &policy[at..at + policy[at..].find('\'').unwrap()];
    assert!(page.text.contains(&format!("nonce=\"{nonce}\"")));
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// What depends on the session is not stored: pages, the REST API, server functions.
/// Static files keep their own directive.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn answers_that_depend_on_the_session_are_not_stored() {
    if !services() {
        return;
    }
    let t = start("").await;
    let anna = sign_up(&t, "anna").await;
    let current_user = fn_path::<web::api::CurrentUser>();
    let cache = |r: &Resp| r.header("cache-control").map(str::to_string);
    for (uri, cookie) in [
        ("/", Some(anna.as_str())),
        ("/sign-in", None),
        ("/no/such/page", None),
        ("/api/v1/auth/me", Some(anna.as_str())),
        ("/api/v1/auth/me", None),
        ("/api/v1/nothing", None),
    ] {
        let r = get(&t, uri, cookie).await;
        assert_eq!(cache(&r).as_deref(), Some("no-store"), "{uri}");
    }
    let r = call(&t, "POST", &current_user, &[("cookie", &anna)], "").await;
    assert_eq!((r.status, cache(&r).as_deref()), (200, Some("no-store")));
    let r = get(&t, "/plain.txt", None).await;
    assert_eq!(
        (r.status, cache(&r)),
        (200, None),
        "a file of the site root"
    );
    let r = get(&t, "/healthz", None).await;
    assert_eq!((r.status, cache(&r)), (200, None));
    // review SEC1-9: the event stream answers for one session too. It keeps the directive
    // its handler sets (axum's `Sse`): a stream that never ends is not stored either way
    let r = head(&t, "/api/v1/events", &anna).await;
    assert_eq!(r.header("content-type"), Some("text/event-stream"));
    assert_eq!(cache(&r).as_deref(), Some("no-cache"));
    assert_eq!(r.policy(), "default-src 'none'; frame-ancestors 'none'");
    std::fs::remove_dir_all(&t.dir).unwrap();
}

/// OPS-8, DEC-536: a bundle file whose name carries the content hash cargo-leptos gives it
/// (`app.<22 characters>.js`) never changes under that name, so a browser keeps it for good;
/// a name without a hash is still asked for again (DEC-623).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hashed_bundle_files_are_kept_for_good() {
    if !services() {
        return;
    }
    let t = start("").await;
    let pkg = t.dir.join("site/pkg");
    for name in [
        "app.1QptwknKfgfUS09SPHUB1w.js",
        "app.agqKqpkWxk7RmjHr_KCTQQ.wasm",
        "app.xG-Am-_6XUfA88Tp5ujsCg.css",
    ] {
        std::fs::write(pkg.join(name), "x").unwrap();
        let r = get(&t, &format!("/pkg/{name}"), None).await;
        assert_eq!(r.status, 200, "{name}");
        assert_eq!(
            r.header("cache-control"),
            Some("public, max-age=31536000, immutable"),
            "{name}"
        );
        assert_eq!(
            r.header("x-content-type-options"),
            Some("nosniff"),
            "{name}"
        );
    }
    // not a hash: too short, or no name in front of it
    for name in ["app.js", "app.min.js", "1QptwknKfgfUS09SPHUB1w.js"] {
        std::fs::write(pkg.join(name), "x").unwrap();
        let r = get(&t, &format!("/pkg/{name}"), None).await;
        assert_eq!(
            (r.status, r.header("cache-control")),
            (200, Some("no-cache")),
            "{name}"
        );
    }
    // a name that does not exist is answered with the 404 page: a page, with a nonce and,
    // for a signed-in visitor, their name in the shell, so it is not stored at all
    // (DEC-658, review SEC1-4)
    let anna = sign_up(&t, "anna").await;
    for (name, cookie) in [
        ("app.AAAAAAAAAAAAAAAAAAAAAA.js", None),
        ("app.AAAAAAAAAAAAAAAAAAAAAA.js", Some(anna.as_str())),
        ("missing.wasm", None),
    ] {
        let r = get(&t, &format!("/pkg/{name}"), cookie).await;
        assert_eq!(r.status, 404, "{name}");
        let kind = r.header("content-type").unwrap_or_default();
        assert!(kind.starts_with("text/html"), "{name}: {kind}");
        assert_eq!(r.header("cache-control"), Some("no-store"), "{name}");
    }
    std::fs::remove_dir_all(&t.dir).unwrap();
}
