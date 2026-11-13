//! v1-books over HTTP: the page total of a book is said to be an estimate, a person
//! corrects it and it survives a refresh (v1-plan.md 3.1.3, 7 B3); who may do that depends
//! on the hosting mode; the operator contact is an instance setting that reaches Open
//! Library in the `User-Agent` and no other source (3.1.1, 7 B4).

// One scenario per test on one server and database; the short names (a/m: admin and
// member cookie, r: answer) keep the many calls readable.
#![allow(clippy::many_single_char_names)]

use crate::support;

use serde_json::{Value, json};
use server::{Overrides, Server};

fn repo(p: &str) -> String {
    format!("{}/../../../../{p}", env!("CARGO_MANIFEST_DIR"))
}

async fn server(extra: &str) -> Option<(Server, axum::Router)> {
    let db = store_pg::testing::fresh_db_url().await?;
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [auth]\nsignup = \"open\"\nsecure_cookies = false\n[brand]\ndir = \"{}\"\n{extra}",
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
    Some((srv, app))
}

struct Resp {
    status: u16,
    body: Value,
    text: String,
    cookie: Option<String>,
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> Resp {
    let mut req = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("accept-language", "en")
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
    Resp {
        status,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        text: String::from_utf8_lossy(&bytes).into_owned(),
        cookie,
    }
}

async fn sign_up(app: &axum::Router, name: &str) -> String {
    let body = json!({"username": name, "display_name": name, "password": "correct horse battery"});
    call(app, "POST", "/api/v1/auth/sign-up", None, Some(body))
        .await
        .cookie
        .expect("sign-up")
}

fn setting<'a>(page: &'a Value, key: &str) -> &'a Value {
    page["settings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("setting {key}"))
}

fn from_source(pages: u32) -> app::CatalogRefresh {
    app::CatalogRefresh {
        title: "Dune".into(),
        original_title: None,
        year: Some(1965),
        description: Some("Sand.".into()),
        attributes: json!({"authors": ["Frank Herbert"], "page_count": pages}),
        external_ids: vec![],
        cover_url: None,
        provenance: [
            ("attributes.authors", "openlibrary"),
            ("attributes.page_count", "openlibrary"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
        fresh_for_secs: 3600,
        popularity: None,
    }
}

async fn item(srv: &Server, kind: &str, title: &str, id: (&str, &str)) -> String {
    srv.app
        .find_or_create_catalog_item(app::NewCatalogItem {
            kind: domain::kind::KindId::new(kind).unwrap(),
            title: title.into(),
            original_title: None,
            year: None,
            description: None,
            external_ids: vec![domain::catalog::ExternalId::new(id.0, id.1)],
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

/// The part of the page from the element with `data-testid="<id>"` on.
fn part<'a>(html: &'a str, id: &str) -> &'a str {
    let start = html
        .find(&format!("data-testid=\"{id}\""))
        .unwrap_or_else(|| panic!("{id} missing"));
    let rest = &html[start..];
    &rest[..rest.find("id=\"item-tabs\"").unwrap_or(rest.len())]
}

// multi-threaded like production (thread-local page state aborts the stream there)
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_page_total_is_an_estimate_until_a_person_sets_it_and_then_it_stays() {
    let Some((srv, app)) = server("").await else {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        return;
    };
    let admin = sign_up(&app, "root").await;
    let member = sign_up(&app, "mia").await;
    let (a, m) = (Some(admin.as_str()), Some(member.as_str()));
    let book = item(&srv, "book", "dune", ("openlibrary", "OL893414W")).await;
    let id = domain::ids::ItemId(book.parse().unwrap());
    srv.app
        .apply_catalog_refresh(id, from_source(608))
        .await
        .unwrap();
    let add = json!({"item_id": book, "status": "in_progress"});
    assert!(
        call(&app, "POST", "/api/v1/library", m, Some(add))
            .await
            .status
            < 300
    );

    // the median of the source is said to be one, with the way to change it
    let html = call(&app, "GET", &format!("/items/{book}"), m, None)
        .await
        .text;
    let bar = part(&html, "action-bar");
    assert!(bar.contains("of 608 pages (estimate)"), "{bar}");
    assert!(bar.contains(">Change<"), "{bar}");
    assert!(
        bar.contains("aria-label=\"Change the page total\""),
        "{bar}"
    );
    let page = call(&app, "GET", &format!("/api/v1/items/{book}/page"), m, None).await;
    assert_eq!(page.body["can_edit"], true, "a home instance: every member");
    assert_eq!(page.body["page_total_editable"], true);

    // a member sets the total of their copy
    let uri = format!("/api/v1/items/{book}/page-count");
    for bad in [json!({"pages": 0}), json!({"pages": 100_001})] {
        let r = call(&app, "PUT", &uri, m, Some(bad)).await;
        assert_eq!(r.status, 422, "{}", r.text);
    }
    let r = call(&app, "PUT", &uri, m, Some(json!({"pages": 600}))).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["attributes"]["page_count"], 600);
    assert_eq!(r.body["provenance"]["attributes.page_count"], "user");
    assert_eq!(r.body["attributes"]["authors"], json!(["Frank Herbert"]));
    let html = call(&app, "GET", &format!("/items/{book}"), m, None)
        .await
        .text;
    let bar = part(&html, "action-bar");
    assert!(bar.contains("of 600 pages"), "{bar}");
    assert!(!bar.contains("(estimate)"), "{bar}");

    // the next refresh from the source keeps it
    srv.app
        .apply_catalog_refresh(id, from_source(612))
        .await
        .unwrap();
    let page = call(&app, "GET", &format!("/api/v1/items/{book}/page"), m, None).await;
    assert_eq!(page.body["item"]["attributes"]["page_count"], 600);
    assert_eq!(page.body["next_step"]["pages"], 600);

    // hosted: the item is everybody's, so only an admin changes it
    let hosted = json!({"values": {"instance.hosting": "hosted"}});
    let r = call(&app, "PUT", "/api/v1/admin/settings", a, Some(hosted)).await;
    assert_eq!(r.status, 200, "{}", r.text);
    let r = call(&app, "PUT", &uri, m, Some(json!({"pages": 500}))).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (403, Some("forbidden"))
    );
    let page = call(&app, "GET", &format!("/api/v1/items/{book}/page"), m, None).await;
    assert_eq!(page.body["can_edit"], false);
    assert_eq!(page.body["page_total_editable"], false);
    assert_eq!(page.body["item"]["attributes"]["page_count"], 600);
    let html = call(&app, "GET", &format!("/items/{book}"), m, None)
        .await
        .text;
    let bar = part(&html, "action-bar");
    assert!(bar.contains("of 600 pages"), "{bar}");
    assert!(!bar.contains(">Change<"), "no Change for a member: {bar}");
    let r = call(&app, "PUT", &uri, a, Some(json!({"pages": 590}))).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(r.body["attributes"]["page_count"], 590);
}

/// Review BOOK-5: "Set page total" is offered only where the kind holds one. A manga is
/// paged and counts chapters: no button, and the route answers with the rule. A film has
/// no pages at all; nobody sets a total signed out.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kind_without_a_page_count_offers_no_page_total() {
    let Some((srv, app)) = server("").await else {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        return;
    };
    let member = sign_up(&app, "mia").await;
    let m = Some(member.as_str());
    let manga = item(&srv, "manga", "Akira", ("anilist", "30664")).await;
    let book = item(&srv, "book", "Emma", ("openlibrary", "OL66W")).await;
    for id in [&manga, &book] {
        let add = json!({"item_id": id, "status": "in_progress"});
        let r = call(&app, "POST", "/api/v1/library", m, Some(add)).await;
        assert!(r.status < 300, "{}", r.text);
    }
    // the book without a total offers to set one, the manga does not
    let html = call(&app, "GET", &format!("/items/{book}"), m, None).await;
    assert!(part(&html.text, "action-bar").contains(">Set page total<"));
    let html = call(&app, "GET", &format!("/items/{manga}"), m, None).await;
    let bar = part(&html.text, "action-bar");
    assert!(bar.contains("id=\"pages-at\""), "the page field: {bar}");
    assert!(!bar.contains("Set page total"), "{bar}");
    assert!(!bar.contains("pages-total-open"), "{bar}");
    let page = call(&app, "GET", &format!("/api/v1/items/{manga}/page"), m, None).await;
    assert_eq!(page.body["can_edit"], true);
    assert_eq!(page.body["page_total_editable"], false);
    let uri = format!("/api/v1/items/{manga}/page-count");
    let r = call(&app, "PUT", &uri, m, Some(json!({"pages": 180}))).await;
    assert_eq!(r.status, 422, "{}", r.text);
    assert!(r.text.contains("this kind has no pages"), "{}", r.text);

    // a film has no pages; nobody sets them signed out
    let film = item(&srv, "movie", "Dune", ("tmdb", "438631")).await;
    let uri = format!("/api/v1/items/{film}/page-count");
    let r = call(&app, "PUT", &uri, m, Some(json!({"pages": 100}))).await;
    assert_eq!(r.status, 422, "{}", r.text);
    let uri = format!("/api/v1/items/{book}/page-count");
    let r = call(&app, "PUT", &uri, None, Some(json!({"pages": 100}))).await;
    assert_eq!(r.status, 401);
}

/// Open Library and TMDB as local mocks, and the configuration that points at them.
#[cfg(feature = "openlibrary")]
async fn sources() -> (wiremock::MockServer, wiremock::MockServer, String) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let ol = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "numFound": 1,
            "docs": [{"key": "/works/OL893414W", "title": "Dune", "author_name": ["Frank Herbert"],
                      "first_publish_year": 1965, "cover_i": 11_481_354, "edition_count": 155,
                      "number_of_pages_median": 608}]
        })))
        .mount(&ol)
        .await;
    let tmdb = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results": []})))
        .mount(&tmdb)
        .await;
    let extra = format!(
        "[connectors.openlibrary]\nbase_url = \"{}\"\n[connectors.tmdb]\nbase_url = \"{}\"\napi_key = \"k\"\n",
        ol.uri(),
        tmdb.uri()
    );
    (ol, tmdb, extra)
}

#[cfg(feature = "openlibrary")]
fn contact(v: &Value) -> Value {
    json!({ "values": { "instance.operator_contact": v } })
}

/// The operator contact is an instance setting: an e-mail address or an https URL, set by
/// an admin; Open Library works without it.
#[cfg(feature = "openlibrary")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_operator_contact_is_a_checked_setting_of_the_admin() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        return;
    }
    let (_ol, _tmdb, extra) = sources().await;
    let (_srv, app) = server(&extra).await.unwrap();
    let admin = sign_up(&app, "root").await;
    let member = sign_up(&app, "mia").await;
    let (a, m) = (Some(admin.as_str()), Some(member.as_str()));
    let put = |v: Value| Some(contact(&v));

    // not set: Open Library needs no key and is asked anyway
    let page = call(&app, "GET", "/api/v1/admin/settings", a, None)
        .await
        .body;
    let c = setting(&page, "instance.operator_contact");
    assert_eq!(
        (c["value"].clone(), c["editable"].as_bool()),
        (Value::Null, Some(true))
    );
    assert_eq!(c["env_name"], "APP__INSTANCE__OPERATOR_CONTACT");
    let sources = call(&app, "GET", "/api/v1/admin/sources", a, None)
        .await
        .body;
    let source = sources
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["status"]["id"] == "openlibrary")
        .unwrap_or_else(|| panic!("{sources}"));
    assert_eq!(source["status"]["state"], "working", "{source}");
    assert_eq!(source["status"]["kinds"], json!(["book"]));
    assert_eq!(source["fields"], json!([]), "no key to ask for");
    assert_eq!(
        source["attribution"],
        "Book data and covers from Open Library."
    );

    // what is not an address or an https URL is refused, and a member may not set it
    for bad in [
        json!("ops@example.org\r\nX-Injected: 1"),
        json!("http://example.org"),
        json!("just me"),
        json!(7),
    ] {
        let r = call(&app, "PUT", "/api/v1/admin/settings", a, put(bad.clone())).await;
        assert_eq!(
            (r.status, r.body["code"].as_str()),
            (422, Some("invalid_contact")),
            "{bad}: {}",
            r.text
        );
    }
    let r = call(
        &app,
        "PUT",
        "/api/v1/admin/settings",
        m,
        put(json!("mia@example.org")),
    )
    .await;
    assert_eq!(r.status, 403);
    let r = call(
        &app,
        "PUT",
        "/api/v1/admin/settings",
        a,
        put(json!(" ops@example.org ")),
    )
    .await;
    assert_eq!(r.status, 200, "{}", r.text);
    let c = setting(&r.body, "instance.operator_contact");
    assert_eq!(
        (c["value"].as_str(), c["source"].as_str()),
        (Some("ops@example.org"), Some("db"))
    );

    // cleared again: an empty text is "no contact"
    let r = call(&app, "PUT", "/api/v1/admin/settings", a, put(json!(""))).await;
    assert_eq!(r.status, 200, "{}", r.text);
    assert_eq!(
        setting(&r.body, "instance.operator_contact")["value"],
        Value::Null
    );
}

/// The contact reaches Open Library in the `User-Agent` and no other source.
#[cfg(feature = "openlibrary")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_operator_contact_goes_to_open_library_and_nowhere_else() {
    if std::env::var_os("TEST_DATABASE_URL").is_none() {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        return;
    }
    let (ol, tmdb, extra) = sources().await;
    let (_srv, app) = server(&extra).await.unwrap();
    let admin = sign_up(&app, "root").await;
    let member = sign_up(&app, "mia").await;
    let (a, m) = (Some(admin.as_str()), Some(member.as_str()));
    let set = Some(contact(&json!("ops@example.org")));
    let r = call(&app, "PUT", "/api/v1/admin/settings", a, set).await;
    assert_eq!(r.status, 200, "{}", r.text);

    // a book search and a film search: the contact goes to Open Library only
    let agent = |r: &wiremock::Request| r.headers["user-agent"].to_str().unwrap().to_string();
    assert_eq!(
        call(&app, "GET", "/api/v1/search?q=dune&kind=book", m, None)
            .await
            .status,
        200
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/search?q=dune&kind=movie", m, None)
            .await
            .status,
        200
    );
    let mut seen = (Vec::new(), Vec::new());
    for _ in 0..200 {
        seen = (
            ol.received_requests().await.unwrap(),
            tmdb.received_requests().await.unwrap(),
        );
        if !seen.0.is_empty() && !seen.1.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(!seen.0.is_empty(), "Open Library was asked");
    assert!(!seen.1.is_empty(), "TMDB was asked");
    let to_books = agent(&seen.0[0]);
    assert!(to_books.ends_with(" (ops@example.org)"), "{to_books}");
    assert!(
        !to_books.contains('+'),
        "the product and the contact, nothing else: {to_books}"
    );
    let to_films = agent(&seen.1[0]);
    assert!(!to_films.contains("example.org"), "{to_films}");
}

/// Set in the configuration, the contact is read-only in the admin area, and a value that
/// is no contact stops the start.
#[tokio::test]
async fn a_configured_operator_contact_is_read_only() {
    let Some((_srv, app)) =
        server("[instance]\noperator_contact = \"https://example.org/contact\"\n").await
    else {
        assert!(std::env::var_os("REQUIRE_SERVICES").is_none());
        return;
    };
    let admin = sign_up(&app, "root").await;
    let a = Some(admin.as_str());
    let page = call(&app, "GET", "/api/v1/admin/settings", a, None)
        .await
        .body;
    let c = setting(&page, "instance.operator_contact");
    assert_eq!(c["value"], "https://example.org/contact");
    assert_eq!(
        (c["source"].as_str(), c["editable"].as_bool()),
        (Some("file"), Some(false))
    );
    let patch = json!({"values": {"instance.operator_contact": "ops@example.org"}});
    let r = call(&app, "PUT", "/api/v1/admin/settings", a, Some(patch)).await;
    assert_eq!(
        (r.status, r.body["code"].as_str()),
        (409, Some("fixed_by_config"))
    );

    let db = store_pg::testing::fresh_db_url().await.unwrap();
    let toml = format!(
        "[database]\nurl = \"{db}\"\n[secrets]\nephemeral = true\n[storage]\nbackend = \"memory\"\n[search]\nbackend = \"postgres\"\n\
         [brand]\ndir = \"{}\"\n[instance]\noperator_contact = \"me at home\"\n",
        repo("config/brand/default")
    );
    let cfg = config::AppConfig::from_sources(&toml, Vec::new()).unwrap();
    let refused = Server::build(cfg, config::ServerRole::All, Overrides::default())
        .await
        .err()
        .expect("an unusable contact stops the start")
        .to_string();
    assert!(refused.contains("instance.operator_contact"), "{refused}");
}
