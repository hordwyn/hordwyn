//! UX phase 0, form discipline (design.md 5.2, 6.14): the server-rendered sign-up and sign-in
//! pages state each rule before typing, label every field, wire hints with
//! `aria-describedby` and use the right autofill tokens (WCAG 1.3.1, 1.3.5, 3.3.2).

use server::{Overrides, Server};
use tower::ServiceExt;

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn app() -> axum::Router {
    let db = store_pg::testing::fresh_db_url().await.expect("database");
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
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
    srv.app_router(&options)
}

async fn page(app: &axum::Router, uri: &str, lang: &str) -> (u16, String) {
    let req = axum::http::Request::builder()
        .uri(uri)
        .header("accept-language", lang);
    let resp = app
        .clone()
        .oneshot(req.body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let body = axum::body::to_bytes(resp.into_body(), 1 << 22)
        .await
        .unwrap();
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
        "[database]\nurl = \"{db}\"\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
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
