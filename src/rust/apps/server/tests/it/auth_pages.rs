//! UX phase 0, form discipline (design.md 5.2, 6.14): the server-rendered sign-up and sign-in
//! pages state each rule before typing, label every field, wire hints with
//! `aria-describedby` and use the right autofill tokens (WCAG 1.3.1, 1.3.5, 3.3.2).

use crate::support;

use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn app() -> axum::Router {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build();
    let app = srv.app_router(&options);
    first_account(&app).await;
    app
}

/// The first account: before it exists every page leads to `/setup` (design.md 5.1).
async fn first_account(app: &axum::Router) {
    let body = r#"{"username":"owner","display_name":"Owner","password":"correct horse battery"}"#;
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/sign-up")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body))
        .unwrap();
    let resp = support::send(app, req).await;
    assert_eq!(resp.status(), 201, "first account");
}

async fn page(app: &axum::Router, uri: &str, lang: &str) -> (u16, String) {
    let req = axum::http::Request::builder()
        .uri(uri)
        .header("accept-language", lang);
    let resp = support::send(app, req.body(axum::body::Body::empty()).unwrap()).await;
    let status = resp.status().as_u16();
    let body = resp.into_body();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

fn services() -> bool {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return false;
    }
    true
}

/// The opening tag of the element with `id`.
fn tag<'a>(html: &'a str, id: &str) -> &'a str {
    let at = html
        .find(&format!("id=\"{id}\""))
        .unwrap_or_else(|| panic!("no element with id {id}"));
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    &html[start..=end]
}

#[tokio::test]
async fn sign_up_states_the_rules_and_labels_every_field() {
    if !services() {
        return;
    }
    let app = app().await;
    for uri in ["/sign-up", "/invite/some-token"] {
        let (status, html) = page(&app, uri, "en").await;
        assert_eq!(status, 200, "{uri}");
        // hints before typing (design.md 2.4)
        assert!(
            html.contains("Used in links and for sign-in. 3 to 32 characters."),
            "{uri}: username hint"
        );
        assert!(
            html.contains("At least 10 characters. No other rules; a long phrase is best."),
            "{uri}: password hint"
        );
        // the live checklist, neutral before typing
        for rule in ["3 to 32 characters", "No spaces", "Only a-z, 0-9 and _ . -"] {
            assert!(html.contains(rule), "{uri}: rule {rule}");
        }
        assert!(
            html.contains("data-state=\"pending\""),
            "{uri}: neutral rules"
        );
        // labelled fields with autofill tokens and descriptions
        for (id, label) in [
            ("username", "Username"),
            ("display_name", "Display name (optional)"),
            ("email", "Email (optional)"),
            ("password", "Password"),
        ] {
            assert!(
                html.contains(&format!("for=\"{id}\"")) && html.contains(&format!(">{label}<")),
                "{uri}: label {label}"
            );
        }
        let user = tag(&html, "username");
        assert!(user.contains("autocomplete=\"username\""), "{user}");
        assert!(
            user.contains("aria-describedby=\"username-description username-rules"),
            "{user}"
        );
        let pass = tag(&html, "password");
        assert!(pass.contains("type=\"password\""), "{pass}");
        assert!(pass.contains("autocomplete=\"new-password\""), "{pass}");
        assert!(
            pass.contains("aria-describedby=\"password-description"),
            "{pass}"
        );
        assert!(tag(&html, "email").contains("autocomplete=\"email\""));
        assert!(tag(&html, "display_name").contains("autocomplete=\"nickname\""));
        assert!(html.contains(">Show<"), "{uri}: show password button");
        assert!(!html.contains("check the fields"), "{uri}");
    }
    let (_, html) = page(&app, "/sign-up", "de").await;
    assert!(html.contains("F\u{fc}r Links und zum Anmelden. 3 bis 32 Zeichen."));
    assert!(html.contains("Keine Leerzeichen"));
}

#[tokio::test]
async fn sign_in_uses_current_password_and_no_rules() {
    if !services() {
        return;
    }
    let app = app().await;
    let (status, html) = page(&app, "/sign-in", "en").await;
    assert_eq!(status, 200);
    let user = tag(&html, "username");
    assert!(user.contains("autocomplete=\"username\""), "{user}");
    let pass = tag(&html, "password");
    assert!(pass.contains("autocomplete=\"current-password\""), "{pass}");
    assert!(html.contains(">Show<"), "show password button");
    // no rules on sign-in: they would hint at which accounts exist
    assert!(!html.contains("No spaces"));
    assert!(!html.contains("data-state=\"pending\""));
}

/// UX0-5 (design.md 6.14): the auth card has the brand mark above it and a footer with the
/// brand's legal links.
#[tokio::test]
async fn auth_pages_show_the_brand_mark_and_legal_links() {
    if !services() {
        return;
    }
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n",
        repo("src/web/tests/brands/sample")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build();
    let app = srv.app_router(&options);
    first_account(&app).await;
    for uri in ["/sign-in", "/sign-up"] {
        let (status, html) = page(&app, uri, "en").await;
        assert_eq!(status, 200, "{uri}");
        assert!(
            html.contains("data-testid=\"brand-mark\""),
            "{uri}: brand mark"
        );
        assert!(html.contains(">Reading Room<"), "{uri}: brand name");
        let footer = &html[html.find("<footer").expect("footer")..];
        assert!(
            footer.contains("href=\"https://example.org/imprint\"") && footer.contains(">Imprint<"),
            "{uri}: imprint link"
        );
        assert!(footer.contains(">Privacy<"), "{uri}: privacy link");
    }
}

// ---------------------------------------------------------------- phase 3 (WP3-4)

/// A request with an optional cookie and JSON body; answers status, `Set-Cookie` values and
/// the body.
async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<&str>,
) -> (u16, Vec<String>, String) {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("accept-language", "en");
    if let Some(c) = cookie {
        req = req.header("cookie", c);
    }
    if body.is_some() {
        req = req.header("content-type", "application/json");
    }
    let body = body.map_or_else(axum::body::Body::empty, |b| {
        axum::body::Body::from(b.to_string())
    });
    let resp = support::send(app, req.body(body).unwrap()).await;
    let status = resp.status().as_u16();
    let cookies = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|v| v.to_str().unwrap().to_string())
        .collect();
    let bytes = resp.into_body();
    (status, cookies, String::from_utf8(bytes.to_vec()).unwrap())
}

/// An app whose first account (the admin) is signed in: the router and its session cookie.
async fn app_with_admin() -> (axum::Router, String) {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let srv = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .unwrap();
    let options = leptos::prelude::LeptosOptions::builder()
        .output_name("app")
        .build();
    let app = srv.app_router(&options);
    let body = r#"{"username":"owner","display_name":"Owner","password":"correct horse battery"}"#;
    let (status, cookies, _) = call(&app, "POST", "/api/v1/auth/sign-up", None, Some(body)).await;
    assert_eq!(status, 201, "first account");
    let cookie = cookies[0].split(';').next().unwrap().to_string();
    (app, cookie)
}

/// A provider "Pocket ID" (slug `pocket`); creating it needs no discovery.
async fn add_provider(app: &axum::Router, admin: &str) {
    let body = r#"{"slug":"pocket","display_name":"Pocket ID","issuer":"https://id.example.org","client_id":"shelf","client_secret":"s3cret","is_default":true}"#;
    let (status, _, text) = call(
        app,
        "POST",
        "/api/v1/admin/oidc/providers",
        Some(admin),
        Some(body),
    )
    .await;
    assert_eq!(status, 201, "{text}");
}

/// The opening `<details>` tag of the password disclosure, if the page has one.
fn password_disclosure(html: &str) -> Option<&str> {
    let at = html.find("data-testid=\"password-disclosure\"")?;
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    Some(&html[start..=end])
}

/// Addendum-1 4.3, 4.5 and design.md 5.2: without a provider the password form is open and
/// there is no disclosure; with one the provider buttons come first (the admin's display
/// name, no logo), the password form sits in a closed `details`, and `next` travels along.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_puts_the_providers_first_and_the_password_behind_a_disclosure() {
    if !services() {
        return;
    }
    let (app, admin) = app_with_admin().await;
    let (_, html) = page(&app, "/sign-in", "en").await;
    assert!(password_disclosure(&html).is_none(), "no disclosure");
    assert!(html.contains("id=\"password\""), "password form open");
    assert!(!html.contains("Continue with"), "no provider button");

    add_provider(&app, &admin).await;
    let (status, html) = page(&app, "/sign-in?next=/history", "en").await;
    assert_eq!(status, 200);
    assert!(
        html.contains(">Continue with Pocket ID<"),
        "provider button"
    );
    assert!(
        html.contains("href=\"/api/v1/auth/oidc/start?provider=pocket&amp;next=%2Fhistory\""),
        "button starts the flow with next"
    );
    // a full page load: the client router must not take the API address as a page
    assert!(
        html.contains("rel=\"external\" href=\"/api/v1/auth/oidc/start?provider=pocket"),
        "external link"
    );
    let details = password_disclosure(&html).expect("password disclosure");
    assert!(details.starts_with("<details"), "{details}");
    assert!(!details.contains(" open"), "closed: {details}");
    assert!(html.contains("Sign in with a password instead"), "summary");
    let button = html.find(">Continue with Pocket ID<").unwrap();
    assert!(
        button < html.find("data-testid=\"password-disclosure\"").unwrap(),
        "providers first"
    );
    // German
    let (_, html) = page(&app, "/sign-in", "de").await;
    assert!(html.contains(">Weiter mit Pocket ID<"), "de button");
}

/// DEC-326: the `?error=` states. The outage is an alert with Try again and the break-glass
/// link (and no Try again after the retry cap); refusals say why; the password form opens.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_explains_the_provider_errors() {
    if !services() {
        return;
    }
    let (app, admin) = app_with_admin().await;
    add_provider(&app, &admin).await;
    let (_, html) = page(&app, "/sign-in?error=oidc_unavailable", "en").await;
    let alert = &html[html.find("role=\"alert\"").expect("alert")..];
    assert!(alert.contains("Pocket ID is not answering"), "outage alert");
    assert!(html.contains(">Try again<"), "retry");
    assert!(
        html.contains("href=\"/sign-in/local\""),
        "break-glass link on the outage page"
    );
    assert!(
        password_disclosure(&html).is_some_and(|d| d.contains(" open")),
        "password form open"
    );
    let (_, html) = page(&app, "/sign-in?error=oidc_unavailable&retry=0", "en").await;
    assert!(html.contains("Pocket ID is not answering"));
    assert!(!html.contains(">Try again<"), "no retry after the cap");
    assert!(html.contains("stopped trying"), "says why");
    for (code, text) in [
        (
            "group_rule",
            "Your account is not allowed here (group rule)",
        ),
        ("oidc_denied", "Pocket ID did not allow this sign-in"),
        (
            "no_account",
            "No account for this sign-in. Ask your admin for an invite.",
        ),
        ("account_disabled", "This account is disabled."),
        ("oidc_failed", "The sign-in did not work."),
        ("not_found", "The sign-in did not work."),
    ] {
        let (status, html) = page(&app, &format!("/sign-in?error={code}"), "en").await;
        assert_eq!(status, 200, "{code}");
        assert!(html.contains("role=\"alert\""), "{code}: alert");
        assert!(html.contains(text), "{code}: {text}");
    }
    // the normal flow never links the break-glass page
    let (_, html) = page(&app, "/sign-in", "en").await;
    assert!(
        !html.contains("href=\"/sign-in/local\""),
        "no break-glass link"
    );
}

/// Addendum-1 4.4: "Your session ended" only when the browser still sent a session cookie
/// that is no longer valid.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_says_when_the_session_ended() {
    if !services() {
        return;
    }
    let (app, admin) = app_with_admin().await;
    let name = admin.split('=').next().unwrap();
    let stale = format!("{name}=no-such-session");
    let (_, _, html) = call(&app, "GET", "/sign-in", Some(&stale), None).await;
    assert!(html.contains("Your session ended"), "ended");
    let (_, html) = page(&app, "/sign-in", "en").await;
    assert!(!html.contains("Your session ended"), "fresh visit");
}

/// Addendum-1 4.3, 4.5, 6.14; DEC-301: the new auth pages render in the auth frame, each
/// with one H1; `/signed-out` never signs anyone in.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_new_auth_pages_render() {
    if !services() {
        return;
    }
    let app = app().await;
    let (status, html) = page(&app, "/signed-out", "en").await;
    assert_eq!(status, 200);
    assert!(html.contains(">You are signed out<"), "signed-out heading");
    assert!(html.contains("href=\"/sign-in\""), "sign-in link");
    assert!(html.contains("data-testid=\"brand-mark\""), "auth frame");
    let (_, html) = page(&app, "/signed-out", "de").await;
    assert!(html.contains(">Du bist abgemeldet<"), "de");

    let (status, html) = page(&app, "/sign-in/local", "en").await;
    assert_eq!(status, 200);
    assert!(html.contains(">Local sign-in<"), "local heading");
    assert!(tag(&html, "username").contains("autocomplete=\"username\""));
    assert!(tag(&html, "password").contains("autocomplete=\"current-password\""));
    assert!(html.contains("This sign-in is recorded in the audit log."));

    let (status, html) = page(&app, "/sign-in/link", "en").await;
    assert_eq!(status, 200);
    assert!(html.contains(">Is this your account?<"), "link heading");

    let (status, html) = page(&app, "/reset-password/some-token", "en").await;
    assert_eq!(status, 200);
    assert!(html.contains(">Choose a new password<"), "reset heading");
    assert!(tag(&html, "password").contains("autocomplete=\"new-password\""));
    for uri in [
        "/signed-out",
        "/sign-in/local",
        "/sign-in/link",
        "/reset-password/x",
    ] {
        let (_, html) = page(&app, uri, "en").await;
        assert_eq!(html.matches("<h1").count(), 1, "{uri}: one H1");
    }
}
