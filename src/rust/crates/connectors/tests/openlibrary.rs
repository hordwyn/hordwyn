#![cfg(feature = "openlibrary")]
//! Open Library connector against fixtures recorded on 2026-11-12 (and three synthetic ones,
//! marked in their headers); see docs/connectors/openlibrary.md.

use std::path::Path;
use std::sync::Arc;

use app::ports::Lane;
use catalog_core::{
    ConnectorError, ContactUse, Ctx, Details, DetailsRequest, ImageKind, Search, SearchRequest,
    Snapshot,
};
use connectors::openlibrary::{OpenLibrary, isbn13};
use domain::catalog::ExternalId;
use domain::net::PrivateHosts;
use testkit::ReplayHttp;

fn replay() -> (Arc<ReplayHttp>, Ctx) {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/openlibrary"),
    ));
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    (http, ctx)
}

fn query(q: &str, limit: u32, language: Option<&str>) -> SearchRequest {
    SearchRequest {
        kind: "book".into(),
        query: q.into(),
        language: language.map(String::from),
        limit,
    }
}

fn work(id: &str) -> DetailsRequest {
    DetailsRequest {
        kind: "book".into(),
        id: id.into(),
        language: None,
    }
}

fn valid(s: &Snapshot) {
    let kinds = kinds::Registry::with_builtins();
    app::ports::KindCatalog::validate_attributes(
        &kinds,
        &domain::kind::KindId::new("book").unwrap(),
        &s.attributes,
    )
    .unwrap();
}

fn cover(s: &Snapshot) -> Option<&str> {
    s.images
        .iter()
        .find(|i| i.kind == ImageKind::Cover)
        .map(|i| i.url.as_str())
}

#[tokio::test]
async fn search_maps_title_authors_year_pages_and_cover() {
    let (_, ctx) = replay();
    let hits = OpenLibrary::new()
        .search(&ctx, &query("dune", 3, Some("en")))
        .await
        .unwrap();
    assert_eq!(hits.len(), 3);
    let dune = &hits[0];
    assert_eq!(dune.connector, "openlibrary");
    assert_eq!(dune.kind, "book");
    assert_eq!(
        dune.external_ids,
        [ExternalId::new("openlibrary", "OL893414W")]
    );
    assert_eq!(dune.title, "Dune");
    assert_eq!(dune.year, Some(1965));
    assert_eq!(
        dune.attributes,
        serde_json::json!({"authors": ["Frank Herbert"], "page_count": 608})
    );
    assert_eq!(
        cover(dune),
        Some("https://covers.openlibrary.org/b/id/11481354-L.jpg?default=false")
    );
    assert_eq!(dune.popularity, Some(155.0));
    assert!(!dune.complete);
    assert_eq!(dune.description, None);
    for h in &hits {
        valid(h);
    }
}

#[tokio::test]
async fn a_doc_without_cover_i_has_no_image() {
    let (_, ctx) = replay();
    let hits = OpenLibrary::new()
        .search_title(&ctx, "Fantastic Mr Fox", 5)
        .await
        .unwrap();
    assert_eq!(hits.len(), 5);
    let bare = hits
        .iter()
        .find(|h| h.external_ids[0].value == "OL34465599W")
        .unwrap();
    assert_eq!(bare.images.len(), 0);
    // no median page count and no year either: missing fields are no error
    assert_eq!(bare.attributes, serde_json::json!({"authors": ["Leffler"]}));
    let tie_in = hits
        .iter()
        .find(|h| h.external_ids[0].value == "OL20097961W")
        .unwrap();
    assert_eq!(tie_in.year, None);
    assert!(cover(&hits[0]).is_some());
}

#[test]
fn an_isbn_is_recognised_by_its_check_digit() {
    assert_eq!(isbn13("9780140328721").as_deref(), Some("9780140328721"));
    assert_eq!(
        isbn13("978-0-14-032872-1").as_deref(),
        Some("9780140328721")
    );
    assert_eq!(isbn13("0-14-032872-6").as_deref(), Some("9780140328721"));
    assert_eq!(isbn13("0 14 032872 6").as_deref(), Some("9780140328721"));
    // ISBN-10 with the check character X
    assert_eq!(isbn13("080442957X").as_deref(), Some("9780804429573"));
    assert_eq!(isbn13("080442957x").as_deref(), Some("9780804429573"));
    for not in [
        "9780140328722",
        "0140328727",
        "1984",
        "4006381333931",
        "dune",
        "978014032872X",
        "",
    ] {
        assert_eq!(isbn13(not), None, "{not}");
    }
}

#[tokio::test]
async fn an_isbn_query_asks_the_isbn_endpoint_and_returns_its_work() {
    for typed in ["9780140328721", "978-0-14-032872-1", "0-14-032872-6"] {
        let (http, ctx) = replay();
        let hits = OpenLibrary::new()
            .search(&ctx, &query(typed, 20, Some("en")))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "{typed}");
        let fox = &hits[0];
        assert_eq!(fox.title, "Fantastic Mr Fox");
        assert_eq!(
            fox.external_ids,
            [
                ExternalId::new("openlibrary", "OL45804W"),
                ExternalId::new("isbn", "9780140328721"),
            ]
        );
        assert_eq!(fox.attributes["isbn13"], "9780140328721");
        assert_eq!(fox.attributes["authors"], serde_json::json!(["Roald Dahl"]));
        // work level: the median of the work, not the 96 pages of this one edition (here equal)
        assert_eq!(fox.attributes["page_count"], 96);
        assert_eq!(fox.year, Some(1970));
        valid(fox);
        let urls: Vec<String> = http.requests().into_iter().map(|r| r.url).collect();
        assert_eq!(urls.len(), 2, "{urls:?}");
        assert!(
            urls[0].starts_with("https://openlibrary.org/isbn/"),
            "{urls:?}"
        );
        assert!(urls[1].contains("q=key%3A%2Fworks%2FOL45804W"), "{urls:?}");
    }
    // an ISBN nobody holds: no result, not an error
    let (_, ctx) = replay();
    let none = OpenLibrary::new()
        .search(&ctx, &query("9780306406157", 20, None))
        .await
        .unwrap();
    assert_eq!(none.len(), 0);
}

#[tokio::test]
async fn a_number_with_a_wrong_check_digit_is_a_title_search() {
    let (http, ctx) = replay();
    http.record(
        "GET",
        "https://openlibrary.org/search.json?q=9780140328722&fields=key%2Ctitle%2Cauthor_name%2Cfirst_publish_year%2Ccover_i%2Cedition_count%2Cnumber_of_pages_median&limit=20",
        200,
        r#"{"numFound":0,"docs":[]}"#,
    );
    let hits = OpenLibrary::new()
        .search(&ctx, &query("9780140328722", 20, None))
        .await
        .unwrap();
    assert_eq!(hits.len(), 0);
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn details_reads_a_description_that_is_a_string_or_an_object() {
    let (http, ctx) = replay();
    let c = OpenLibrary::new();
    let fox = c.details(&ctx, &work("OL45804W")).await.unwrap();
    assert!(fox.complete);
    assert_eq!(fox.title, "Fantastic Mr Fox");
    assert!(
        fox.description
            .as_deref()
            .unwrap()
            .starts_with("The main character of Fantastic Mr. Fox"),
        "{:?}",
        fox.description
    );
    assert_eq!(
        fox.external_ids,
        [ExternalId::new("openlibrary", "OL45804W")]
    );
    assert_eq!(fox.year, Some(1970));
    assert_eq!(fox.attributes["authors"], serde_json::json!(["Roald Dahl"]));
    assert_eq!(fox.attributes["page_count"], 96);
    let subjects = fox.attributes["subjects"].as_array().unwrap();
    assert_eq!(subjects.len(), 20, "the first 20 of the 24 in the fixture");
    assert_eq!(subjects[0], "Animals");
    assert_eq!(
        cover(&fox),
        Some("https://covers.openlibrary.org/b/id/6498519-L.jpg?default=false")
    );
    assert_eq!(fox.popularity, Some(161.0));
    valid(&fox);
    assert_eq!(http.requests().len(), 2, "the work and one search for it");

    let potter = c.details(&ctx, &work("OL82563W")).await.unwrap();
    assert!(
        potter
            .description
            .as_deref()
            .unwrap()
            .starts_with("Turning the envelope over"),
        "{:?}",
        potter.description
    );
    assert_eq!(
        potter.attributes["authors"],
        serde_json::json!(["J. K. Rowling"])
    );
    assert_eq!(potter.attributes["page_count"], 302);
    valid(&potter);
}

#[tokio::test]
async fn details_takes_an_edition_id() {
    let (http, ctx) = replay();
    let fox = OpenLibrary::new()
        .details(&ctx, &work("OL7353617M"))
        .await
        .unwrap();
    assert_eq!(fox.title, "Fantastic Mr Fox", "the work's title");
    // the item keeps the id it was asked by
    assert_eq!(
        fox.external_ids,
        [ExternalId::new("openlibrary", "OL7353617M")]
    );
    assert_eq!(fox.attributes["page_count"], 96);
    let urls: Vec<String> = http.requests().into_iter().map(|r| r.url).collect();
    assert_eq!(
        urls[..2],
        [
            "https://openlibrary.org/books/OL7353617M.json",
            "https://openlibrary.org/works/OL45804W.json"
        ]
    );
}

#[tokio::test]
async fn negative_cover_ids_are_skipped() {
    let (_, ctx) = replay();
    // covers [-1, null, 424242, 7]; the search index does not know the work
    let odd = OpenLibrary::new()
        .details(&ctx, &work("OL999001W"))
        .await
        .unwrap();
    assert_eq!(
        cover(&odd),
        Some("https://covers.openlibrary.org/b/id/424242-L.jpg?default=false")
    );
    assert_eq!(odd.title, "A Book With Odd Covers");
    assert_eq!(odd.description, None);
    assert_eq!(odd.attributes, serde_json::json!({}));
    assert!(odd.complete);
}

#[tokio::test]
async fn a_404_is_not_found() {
    let (_, ctx) = replay();
    let r = OpenLibrary::new().details(&ctx, &work("OL1W")).await;
    assert_eq!(r, Err(ConnectorError::NotFound));
    // an id that is neither a work nor an edition id is not sent anywhere
    let (http, ctx) = replay();
    let r = OpenLibrary::new()
        .details(&ctx, &work("../authors/OL1A"))
        .await;
    assert_eq!(r, Err(ConnectorError::NotFound));
    assert_eq!(http.requests().len(), 0);
}

#[tokio::test]
async fn many_works_are_asked_with_one_search() {
    let (http, ctx) = replay();
    let ids = ["OL82563W", "OL45804W", "OL893415W"].map(String::from);
    let found = OpenLibrary::new().works(&ctx, &ids).await.unwrap();
    assert_eq!(http.requests().len(), 1);
    let titles: Vec<(&str, &str)> = found
        .iter()
        .map(|s| (s.external_ids[0].value.as_str(), s.title.as_str()))
        .collect();
    assert_eq!(
        titles,
        [
            ("OL82563W", "Harry Potter and the Philosopher's Stone"),
            ("OL45804W", "Fantastic Mr Fox"),
        ],
        "the id the index does not hold is simply absent"
    );
    assert_eq!(found[0].attributes["page_count"], 302);
    assert!(cover(&found[1]).is_some());
    // nothing to ask, nothing sent; ids that are not work ids are left out of the query
    let (http, ctx) = replay();
    let none = OpenLibrary::new()
        .works(&ctx, &["OL7353617M".to_string(), "x y".to_string()])
        .await
        .unwrap();
    assert_eq!(none.len(), 0);
    assert_eq!(http.requests().len(), 0);
}

#[tokio::test]
async fn every_request_goes_to_an_allowed_host() {
    let (http, ctx) = replay();
    let c = OpenLibrary::new();
    let info = catalog_core::Connector::info(&c).clone();
    assert_eq!(
        info.hosts,
        [
            "openlibrary.org",
            "covers.openlibrary.org",
            "archive.org",
            "*.archive.org"
        ]
    );
    c.search(&ctx, &query("dune", 3, Some("en"))).await.unwrap();
    c.search(&ctx, &query("9780140328721", 20, None))
        .await
        .unwrap();
    c.details(&ctx, &work("OL7353617M")).await.unwrap();
    let mut urls: Vec<String> = http.requests().into_iter().map(|r| r.url).collect();
    for s in c.search(&ctx, &query("dune", 3, Some("en"))).await.unwrap() {
        urls.extend(s.images.into_iter().map(|i| i.url));
    }
    assert!(urls.len() > 6);
    for u in urls {
        let host = url::Url::parse(&u).unwrap().host_str().unwrap().to_string();
        assert!(PrivateHosts::new(&info.hosts).covers(&host, None), "{u}");
        assert!(!u.contains("/b/isbn/"), "covers by cover id only: {u}");
    }
    // the policy the governed client enforces (docs/connectors/openlibrary.md)
    assert_eq!(
        (
            info.rate_limit.per_second,
            info.rate_limit.burst,
            info.rate_limit.max_concurrency
        ),
        (1, 1, 1)
    );
    assert_eq!(info.cache_ttl_secs, 7 * 24 * 3600);
    assert_eq!(info.contact, ContactUse::Preferred);
    // no key: the limit is per address, so every process shares one pace (BOOK-8)
    assert!(catalog_core::ConnectorPolicy::from_info(&info).shared_pace);
    assert_eq!(info.kinds, ["book"]);
    assert_eq!(info.attribution.url, "https://openlibrary.org/");
}

/// A stored cover follows `covers.openlibrary.org` -> `archive.org` -> a numbered
/// `ia*.us.archive.org` host (docs/connectors/openlibrary.md); the governed client checks every
/// hop against this list with the rules of [`PrivateHosts`] (sec2), so the numbered hosts must
/// be covered and an unlisted host must not.
#[test]
fn the_cover_redirect_hosts_are_on_the_list_and_no_others() {
    let info = catalog_core::Connector::info(&OpenLibrary::new()).clone();
    let list = PrivateHosts::new(&info.hosts);
    for host in [
        "covers.openlibrary.org",
        "archive.org",
        "ia800703.us.archive.org",
        "ia601602.us.archive.org",
    ] {
        assert!(list.covers(host, None), "{host}");
    }
    for host in [
        "images.example.net",
        "archive.org.example.net",
        "notarchive.org",
        "openlibrary.org.example.net",
    ] {
        assert!(!list.covers(host, None), "{host}");
    }
    assert!(
        !list.covers("ia800703.us.archive.org", Some(8443)),
        "default port only"
    );
}

#[tokio::test]
async fn another_kind_is_refused() {
    let (http, ctx) = replay();
    let mut q = query("dune", 3, None);
    q.kind = "audiobook".into();
    let r = OpenLibrary::new().search(&ctx, &q).await;
    assert!(matches!(r, Err(ConnectorError::Unsupported(_))), "{r:?}");
    assert_eq!(http.requests().len(), 0);
}
