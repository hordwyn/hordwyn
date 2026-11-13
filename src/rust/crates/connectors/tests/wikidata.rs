#![cfg(feature = "wikidata")]
//! Wikidata connector (board games) against responses recorded on 2026-11-13 and against
//! entity JSON written by hand in the recorded shape (the tests that say "synthetic"); see
//! docs/connectors/wikidata.md.

use std::path::Path;
use std::sync::Arc;

use app::ports::{HttpError, Lane};
use catalog_core::{
    Capability, Connector, ConnectorError, ContactUse, Ctx, Details, DetailsRequest, Search,
    SearchRequest, Snapshot,
};
use connectors::wikidata::{CLASSES, Wikidata};
use domain::catalog::ExternalId;
use serde_json::{Value, json};
use testkit::ReplayHttp;

fn replay() -> (Arc<ReplayHttp>, Ctx) {
    let http = Arc::new(ReplayHttp::from_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/wikidata"),
    ));
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    (http, ctx)
}

fn query(q: &str, limit: u32, language: Option<&str>) -> SearchRequest {
    SearchRequest {
        kind: "board_game".into(),
        query: q.into(),
        language: language.map(String::from),
        limit,
    }
}

fn game(id: &str, language: Option<&str>) -> DetailsRequest {
    DetailsRequest {
        kind: "board_game".into(),
        id: id.into(),
        language: language.map(String::from),
    }
}

fn valid(s: &Snapshot) {
    let kinds = kinds::Registry::with_builtins();
    app::ports::KindCatalog::validate_attributes(
        &kinds,
        &domain::kind::KindId::new("board_game").unwrap(),
        &s.attributes,
    )
    .unwrap();
}

fn urls(http: &ReplayHttp) -> Vec<String> {
    http.requests().into_iter().map(|r| r.url).collect()
}

// --- synthetic entities (hand-written in the shape of the recorded ones) ---

fn statement(rank: &str, kind: &str, value: &Value) -> Value {
    json!({
        "mainsnak": {"snaktype": "value", "datavalue": {"value": value, "type": kind}},
        "type": "statement",
        "rank": rank,
    })
}

fn quantity(rank: &str, amount: &str, unit: &str) -> Value {
    let unit = if unit == "1" {
        unit.to_string()
    } else {
        format!("http://www.wikidata.org/entity/{unit}")
    };
    statement(rank, "quantity", &json!({"amount": amount, "unit": unit}))
}

fn text(rank: &str, value: &str) -> Value {
    statement(rank, "string", &json!(value))
}

fn year(rank: &str, time: &str, precision: u32) -> Value {
    statement(
        rank,
        "time",
        &json!({"time": time, "timezone": 0, "before": 0, "after": 0, "precision": precision,
            "calendarmodel": "http://www.wikidata.org/entity/Q1985727"}),
    )
}

/// The details of the one synthetic item `Q1` with these labels and claims, asked in English.
async fn synthetic(labels: &Value, claims: &Value) -> Result<Snapshot, ConnectorError> {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    let url = wd
        .entities_url(&["Q1".into()], true, "en", Lane::Interactive)
        .unwrap();
    let body = json!({"entities": {"Q1": {
        "type": "item", "id": "Q1", "labels": labels, "descriptions": {},
        "claims": claims, "sitelinks": {},
    }}, "success": 1});
    http.record("GET", &url, 200, serde_json::to_vec(&body).unwrap());
    let s = wd.details(&ctx, &game("Q1", None)).await?;
    valid(&s);
    Ok(s)
}

fn en(label: &str) -> Value {
    json!({"en": {"language": "en", "value": label}})
}

// --- recorded ---

#[tokio::test]
async fn search_sends_the_class_filter_and_asks_the_hits_in_one_batch() {
    let (http, ctx) = replay();
    let hits = Wikidata::new()
        .search(&ctx, &query("Catan", 3, Some("de")))
        .await
        .unwrap();
    let urls = urls(&http);
    assert_eq!(urls.len(), 2, "one search, one batch of entities");
    assert!(urls[0].contains("action=query&list=search&srnamespace=0&srlimit=3"));
    assert!(
        urls[0].contains("srsearch=Catan+haswbstatement%3AP31%3DQ131436%7CP31%3DQ60474521%7C"),
        "{}",
        urls[0]
    );
    for class in CLASSES {
        assert!(urls[0].contains(&format!("P31%3D{class}")), "{class}");
    }
    assert!(urls[1].contains("action=wbgetentities&ids=Q17271%7CQ105395676%7CQ839962&"));
    // the member's language, then English, then `mul`, then a wider set (WD-2)
    assert!(
        urls[1].contains("languages=de%7Cen%7Cmul%7Cfr%7C"),
        "{}",
        urls[1]
    );

    let ids: Vec<&str> = hits
        .iter()
        .map(|h| h.external_ids[0].value.as_str())
        .collect();
    assert_eq!(
        ids,
        ["Q17271", "Q105395676", "Q839962"],
        "in the order of the search"
    );
    let catan = &hits[0];
    assert_eq!(catan.connector, "wikidata");
    assert_eq!(catan.kind, "board_game");
    assert_eq!(catan.title, "Die Siedler von Catan");
    assert_eq!(
        catan.original_title.as_deref(),
        Some("The Settlers of Catan")
    );
    assert_eq!(catan.description.as_deref(), Some("Brettspiel"));
    assert_eq!(catan.year, Some(1995));
    assert_eq!(
        catan.attributes,
        json!({
            "players_min": 3, "players_max": 4,
            "playtime_min_minutes": 75, "playtime_max_minutes": 75,
            "min_age": 10, "bgg_id": 13, "gtins": ["0688623905027"],
        })
    );
    // the BGG id and the GTINs are reference identifiers, not matching keys (WD-3, DEC-195)
    assert_eq!(catan.external_ids, [ExternalId::new("wikidata", "Q17271")]);
    assert_eq!(catan.popularity, Some(42.0), "the number of sitelinks");
    assert!(!catan.complete);
    // a row with a title and little else is what the source has for most games
    let helpers = &hits[1];
    assert_eq!(helpers.title, "Helfer von Catan");
    assert_eq!(helpers.year, None);
    assert_eq!(helpers.popularity, Some(1.0));
    for h in &hits {
        valid(h);
    }
}

#[tokio::test]
async fn an_entity_maps_players_time_age_year_and_ids() {
    let (http, ctx) = replay();
    let s = Wikidata::new()
        .details(&ctx, &game("Q17271", Some("de-DE")))
        .await
        .unwrap();
    assert_eq!(s.title, "Die Siedler von Catan");
    assert_eq!(s.year, Some(1995));
    assert_eq!(
        s.attributes,
        json!({
            "players_min": 3, "players_max": 4,
            "playtime_min_minutes": 75, "playtime_max_minutes": 75,
            "min_age": 10, "bgg_id": 13, "gtins": ["0688623905027"],
            "categories": ["Eurogame", "Kartenspiel"],
        })
    );
    assert_eq!(s.external_ids, [ExternalId::new("wikidata", "Q17271")]);
    assert!(s.complete);
    valid(&s);
    let urls = urls(&http);
    assert_eq!(
        urls.len(),
        2,
        "the item, then the labels of its linked items"
    );
    assert!(urls[1].contains("ids=Q788553%7CQ142714&props=labels&languages=de%7Cen%7Cmul%7C"));
}

#[tokio::test]
async fn two_duration_statements_give_the_shortest_and_the_longest_time() {
    let (_, ctx) = replay();
    let s = Wikidata::new()
        .details(&ctx, &game("Q65784798", Some("en")))
        .await
        .unwrap();
    assert_eq!(s.title, "Wingspan");
    assert_eq!(s.original_title, None, "the title is the English label");
    assert_eq!(s.year, Some(2019));
    assert_eq!(
        s.attributes,
        json!({
            "players_min": 1, "players_max": 5,
            "playtime_min_minutes": 40, "playtime_max_minutes": 70,
            "min_age": 10, "bgg_id": 266_192,
            "categories": ["card game"],
        })
    );
}

#[tokio::test]
async fn p18_is_never_read() {
    let (http, ctx) = replay();
    let wd = Wikidata::new();
    let s = wd.details(&ctx, &game("Q17271", Some("de"))).await.unwrap();
    assert_eq!(s.images.len(), 0, "the recorded item has a P18 statement");
    let hits = wd
        .search(&ctx, &query("Catan", 3, Some("de")))
        .await
        .unwrap();
    assert_eq!(hits.iter().map(|h| h.images.len()).sum::<usize>(), 0);
    assert!(wd.as_images().is_none());
    assert!(!wd.info().capabilities.contains(&Capability::Images));
    for u in urls(&http) {
        assert!(!u.contains("commons"), "{u}");
    }
}

#[tokio::test]
async fn an_empty_search_asks_no_entities() {
    let (http, ctx) = replay();
    let hits = Wikidata::new()
        .search(&ctx, &query("zzqqxx nope", 5, Some("en")))
        .await
        .unwrap();
    assert_eq!(hits.len(), 0);
    assert_eq!(http.requests().len(), 1);
    // and a blank query asks nothing at all
    let hits = Wikidata::new()
        .search(&ctx, &query("  ", 5, None))
        .await
        .unwrap();
    assert_eq!(hits.len(), 0);
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn an_unknown_item_is_not_found() {
    let (http, ctx) = replay();
    let wd = Wikidata::new();
    // the source answers 200 with an error object
    let gone = wd.details(&ctx, &game("Q99999999999", Some("en"))).await;
    assert_eq!(gone, Err(ConnectorError::NotFound));
    assert_eq!(http.requests().len(), 1);
    // what is no item id is never put into a request
    for bad in ["", "P31", "Q", "Q12|Q13", "q17271", "Q1&maxlag=0", "17271"] {
        let r = wd.details(&ctx, &game(bad, None)).await;
        assert_eq!(r, Err(ConnectorError::NotFound), "{bad:?}");
    }
    assert_eq!(http.requests().len(), 1);
}

#[tokio::test]
async fn every_request_goes_to_www_wikidata_org() {
    let (http, ctx) = replay();
    let wd = Wikidata::new();
    wd.search(&ctx, &query("Catan", 3, Some("de")))
        .await
        .unwrap();
    wd.details(&ctx, &game("Q17271", Some("de"))).await.unwrap();
    let requests = http.requests();
    assert_eq!(requests.len(), 4);
    for r in requests {
        assert!(
            r.url.starts_with("https://www.wikidata.org/w/api.php?"),
            "{}",
            r.url
        );
        assert_eq!(r.method, "GET");
        assert_eq!(r.connector, "wikidata");
        // success answers may be kept for a day (the API's own parameters); errors never are
        assert!(r.url.contains("maxage=86400&smaxage=86400"), "{}", r.url);
    }
    assert_eq!(wd.info().hosts, ["www.wikidata.org"]);
}

#[tokio::test]
async fn background_calls_carry_maxlag_and_interactive_ones_do_not() {
    let wd = Wikidata::new();
    for (lane, expected) in [
        (Lane::Interactive, false),
        (Lane::BackgroundRefresh, true),
        (Lane::UserImport, true),
        (Lane::Prefetch, true),
    ] {
        let http = Arc::new(ReplayHttp::default());
        let ctx = Ctx::new(http.clone(), lane);
        // nothing is recorded: only the requests are looked at
        let _ = wd.search(&ctx, &query("Catan", 3, None)).await;
        let _ = wd.details(&ctx, &game("Q17271", None)).await;
        let urls = urls(&http);
        assert_eq!(urls.len(), 2);
        for u in urls {
            assert_eq!(u.contains("&maxlag=5"), expected, "{lane:?} {u}");
        }
    }
}

#[test]
fn the_source_requires_the_operator_contact_and_serves_board_games_only() {
    let wd = Wikidata::new();
    let info = wd.info();
    assert_eq!(info.id, "wikidata");
    assert_eq!(info.kinds, ["board_game"]);
    assert_eq!(info.contact, ContactUse::Required);
    assert_eq!(info.capabilities, [Capability::Search, Capability::Details]);
    // 120 a minute at most, one at a time (the source allows 200 and three)
    assert_eq!(info.rate_limit.per_second, 2);
    assert_eq!(info.rate_limit.burst, 2);
    assert_eq!(info.rate_limit.max_concurrency, 1);
    assert_eq!(info.cache_ttl_secs, 24 * 3600);
    assert_eq!(info.attribution.text, "Data from Wikidata (CC0).");
    assert_eq!(info.attribution.url, "https://www.wikidata.org/");
    assert_eq!(CLASSES[0], "Q131436");
    assert_eq!(CLASSES[1], "Q60474521");
}

#[tokio::test]
async fn another_kind_is_not_served() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    let mut movie = query("Catan", 3, None);
    movie.kind = "movie".into();
    assert!(matches!(
        wd.search(&ctx, &movie).await,
        Err(ConnectorError::Unsupported(_))
    ));
    let mut details = game("Q17271", None);
    details.kind = "movie".into();
    assert!(matches!(
        wd.details(&ctx, &details).await,
        Err(ConnectorError::Unsupported(_))
    ));
    assert_eq!(http.requests().len(), 0);
}

// --- synthetic ---

#[tokio::test]
async fn the_label_falls_back_from_the_user_language_to_english() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    let entity = |labels: Value, descriptions: Value| {
        json!({"entities": {"Q1": {"type": "item", "id": "Q1", "labels": labels,
            "descriptions": descriptions, "claims": {}, "sitelinks": {}}}, "success": 1})
        .to_string()
    };
    let url = wd
        .entities_url(&["Q1".into()], true, "de", Lane::Interactive)
        .unwrap();
    let de = game("Q1", Some("de"));

    http.record("GET", &url, 200, entity(en("Root"), en("2018 board game")));
    let s = wd.details(&ctx, &de).await.unwrap();
    assert_eq!(s.title, "Root");
    assert_eq!(s.original_title, None);
    assert_eq!(s.description.as_deref(), Some("2018 board game"));

    let mul = json!({"mul": {"language": "mul", "value": "Azul"}});
    http.record("GET", &url, 200, entity(mul, json!({})));
    let s = wd.details(&ctx, &de).await.unwrap();
    assert_eq!(s.title, "Azul");
    assert_eq!(s.description, None);

    // no label in the languages asked: the id stands in on the page of a known item ...
    http.record("GET", &url, 200, entity(json!({}), json!({})));
    assert_eq!(wd.details(&ctx, &de).await.unwrap().title, "Q1");
    // ... and an empty map written as a list is read as empty
    http.record("GET", &url, 200, entity(json!([]), json!([])));
    assert_eq!(wd.details(&ctx, &de).await.unwrap().title, "Q1");
}

#[tokio::test]
async fn a_search_hit_without_a_label_is_left_out() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    let search = wd.search_url("Azul", 5, Lane::Interactive).unwrap();
    http.record(
        "GET",
        &search,
        200,
        json!({"batchcomplete": true, "query": {"search": [
            {"ns": 0, "title": "Q2", "pageid": 2}, {"ns": 0, "title": "Q1", "pageid": 1},
            {"ns": 0, "title": "Q3", "pageid": 3}, {"ns": 120, "title": "Property:P31"},
        ]}})
        .to_string(),
    );
    let entities = wd
        .entities_url(
            &["Q2".into(), "Q1".into(), "Q3".into()],
            true,
            "en",
            Lane::Interactive,
        )
        .unwrap();
    let item = |id: &str, labels: Value| {
        json!({"type": "item", "id": id, "labels": labels, "descriptions": {}, "claims": {},
            "sitelinks": {}})
    };
    http.record(
        "GET",
        &entities,
        200,
        json!({"entities": {
            "Q1": item("Q1", en("Azul")),
            "Q2": item("Q2", json!({})),
            "Q3": {"id": "Q3", "missing": ""},
        }, "success": 1})
        .to_string(),
    );
    let hits = wd.search(&ctx, &query("Azul", 5, None)).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].external_ids, [ExternalId::new("wikidata", "Q1")]);
}

#[tokio::test]
async fn a_preferred_statement_wins_and_a_deprecated_one_is_skipped() {
    let unknown = json!({"mainsnak": {"snaktype": "somevalue"}, "type": "statement",
        "rank": "preferred"});
    let s = synthetic(
        &en("Game"),
        &json!({
            // preferred beats the normal one before it
            "P1872": [quantity("normal", "+1", "1"), quantity("preferred", "+2", "1")],
            // deprecated is skipped, then the first of normal rank
            "P1873": [quantity("deprecated", "+99", "1"), quantity("normal", "+4", "1"),
                quantity("normal", "+6", "1")],
            // "unknown value" is no value, whatever its rank
            "P2899": [unknown, quantity("normal", "+8", "Q24564698")],
            // only deprecated statements: no value
            "P2047": [quantity("deprecated", "+30", "Q7727")],
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        s.attributes,
        json!({"players_min": 2, "players_max": 4, "min_age": 8})
    );
}

#[tokio::test]
async fn playing_time_reads_minutes_hours_and_bounds() {
    let time = |claims: Value| async move {
        let s = synthetic(&en("Game"), &json!({"P2047": claims}))
            .await
            .unwrap();
        (
            s.attributes["playtime_min_minutes"].as_u64(),
            s.attributes["playtime_max_minutes"].as_u64(),
        )
    };
    assert_eq!(
        time(json!([quantity("normal", "+45", "Q7727")])).await,
        (Some(45), Some(45))
    );
    assert_eq!(
        time(json!([quantity("normal", "+2", "Q25235")])).await,
        (Some(120), Some(120)),
        "hours"
    );
    assert_eq!(
        time(json!([quantity("normal", "+1.5", "Q25235")])).await,
        (Some(90), Some(90))
    );
    let bounded = statement(
        "normal",
        "quantity",
        &json!({"amount": "+75", "lowerBound": "+60", "upperBound": "+90",
            "unit": "http://www.wikidata.org/entity/Q7727"}),
    );
    assert_eq!(time(json!([bounded])).await, (Some(60), Some(90)));
    assert_eq!(
        time(json!([
            quantity("normal", "+90", "Q7727"),
            quantity("normal", "+30", "Q7727")
        ]))
        .await,
        (Some(30), Some(90)),
        "two statements"
    );
    assert_eq!(
        time(json!([quantity("normal", "+0", "Q7727")])).await,
        (None, None),
        "no time is no fact"
    );
}

#[tokio::test]
async fn an_unknown_unit_gives_no_time() {
    let s = synthetic(
        &en("Game"),
        &json!({
            // Q11574 (second), a day count, no unit at all: none is read as minutes
            "P2047": [quantity("normal", "+3600", "Q11574"), quantity("normal", "+2", "Q573"),
                quantity("normal", "+60", "1")],
            // an age in months is not an age in years
            "P2899": [quantity("normal", "+18", "Q5151")],
        }),
    )
    .await
    .unwrap();
    assert_eq!(s.attributes, json!({}));
}

#[tokio::test]
async fn a_bgg_id_that_is_not_a_number_is_dropped() {
    let s = synthetic(&en("Game"), &json!({"P2339": [text("normal", "catan-13")]}))
        .await
        .unwrap();
    assert_eq!(s.attributes, json!({}));
    assert_eq!(s.external_ids, [ExternalId::new("wikidata", "Q1")]);

    let s = synthetic(
        &en("Game"),
        &json!({
            "P2339": [text("normal", "822"), text("normal", "823")],
            "P3962": [text("normal", "4002051693602"), text("normal", "not a code"),
                text("normal", "0029877030712"), text("normal", "00000001"),
                text("normal", "00000002"), text("normal", "00000003"),
                text("normal", "00000004")],
        }),
    )
    .await
    .unwrap();
    assert_eq!(
        s.attributes,
        json!({"bgg_id": 822, "gtins": ["4002051693602", "0029877030712", "00000001",
            "00000002", "00000003"]}),
        "one BGG id, at most five codes"
    );
    assert_eq!(s.external_ids, [ExternalId::new("wikidata", "Q1")]);
}

/// WD-3 (DEC-195): only the QID matches a catalogue item. A base game and a second item that
/// carry the same BGG id and GTIN (Wikidata has such duplicates) stay two items, each with
/// its own QID and both reference identifiers.
#[tokio::test]
async fn two_items_that_share_a_bgg_id_stay_separate() {
    let env = testkit::TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()));
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    http.record(
        "GET",
        &wd.search_url("Duo", 5, Lane::Interactive).unwrap(),
        200,
        json!({"query": {"search": [{"ns": 0, "title": "Q1"}, {"ns": 0, "title": "Q2"}]}})
            .to_string(),
    );
    let item = |id: &str, label: &str| {
        json!({"type": "item", "id": id, "labels": en(label), "descriptions": {},
        "sitelinks": {}, "claims": {
            "P2339": [text("normal", "13")],
            "P3962": [text("normal", "4002051693602")],
        }})
    };
    let ids = ["Q1".to_string(), "Q2".to_string()];
    http.record(
        "GET",
        &wd.entities_url(&ids, true, "en", Lane::Interactive)
            .unwrap(),
        200,
        json!({"entities": {"Q1": item("Q1", "Duo"), "Q2": item("Q2", "Duo: Second Edition")}})
            .to_string(),
    );
    let hits = wd.search(&ctx, &query("Duo", 5, None)).await.unwrap();
    assert_eq!(hits.len(), 2);
    let mut stored = Vec::new();
    for h in &hits {
        let new = catalog_core::search::new_item(h).unwrap();
        stored.push(env.app.ensure_catalog_item(new).await.unwrap());
    }
    assert_ne!(stored[0].id, stored[1].id, "two items, not one");
    assert_eq!(stored[0].external_ids, [ExternalId::new("wikidata", "Q1")]);
    assert_eq!(stored[1].external_ids, [ExternalId::new("wikidata", "Q2")]);
    for s in &stored {
        assert_eq!(s.attributes["bgg_id"], 13);
        assert_eq!(s.attributes["gtins"], json!(["4002051693602"]));
    }
}

#[tokio::test]
async fn the_year_is_the_earliest_publication_date() {
    let year_of = |claims: Value| async move {
        synthetic(&en("Game"), &json!({"P577": claims}))
            .await
            .unwrap()
            .year
    };
    assert_eq!(
        year_of(json!([
            year("normal", "+2019-03-08T00:00:00Z", 11),
            year("normal", "+2017-00-00T00:00:00Z", 9),
            year("deprecated", "+1990-00-00T00:00:00Z", 9),
        ]))
        .await,
        Some(2017)
    );
    // a decade or a century is no year
    assert_eq!(
        year_of(json!([year("normal", "+1990-00-00T00:00:00Z", 8)])).await,
        None
    );
    // a year before the common era
    assert_eq!(
        year_of(json!([year("normal", "-2600-00-00T00:00:00Z", 9)])).await,
        Some(-2600)
    );
}

#[tokio::test]
async fn mechanics_and_categories_are_the_labels_of_the_linked_items() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    let item = |id: &str| {
        statement(
            "normal",
            "wikibase-entityid",
            &json!({"entity-type": "item", "id": id}),
        )
    };
    let url = wd
        .entities_url(&["Q1".into()], true, "en", Lane::Interactive)
        .unwrap();
    http.record(
        "GET",
        &url,
        200,
        json!({"entities": {"Q1": {"type": "item", "id": "Q1", "labels": en("Game"),
            "descriptions": {}, "sitelinks": {}, "claims": {
                "P4151": [item("Q10"), item("Q11")],
                "P136": [item("Q12"), item("Q10")],
            }}}, "success": 1})
        .to_string(),
    );
    let labels = wd
        .entities_url(
            &["Q10".into(), "Q11".into(), "Q12".into()],
            false,
            "en",
            Lane::Interactive,
        )
        .unwrap();
    assert!(labels.contains("props=labels&"), "{labels}");
    let label = |id: &str, l: Value| json!({"type": "item", "id": id, "labels": l});
    http.record(
        "GET",
        &labels,
        200,
        json!({"entities": {
            "Q10": label("Q10", en("tile placement")),
            // no label in the languages asked: left out, never shown as an id
            "Q11": label("Q11", json!({})),
            "Q12": label("Q12", en("abstract strategy game")),
        }, "success": 1})
        .to_string(),
    );
    let s = wd.details(&ctx, &game("Q1", None)).await.unwrap();
    assert_eq!(
        s.attributes,
        json!({
            "mechanics": ["tile placement"],
            "categories": ["abstract strategy game", "tile placement"],
        })
    );
    valid(&s);
}

#[tokio::test]
async fn an_error_object_in_a_200_answer_is_an_error() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::BackgroundRefresh);
    let url = wd
        .entities_url(&["Q1".into()], true, "en", Lane::BackgroundRefresh)
        .unwrap();
    // the shape recorded on 2026-11-13 with `maxlag=-1`
    http.record(
        "GET",
        &url,
        200,
        json!({"error": {"code": "maxlag", "info": "Waiting for 10.64.16.215: 0.4 seconds lagged.",
            "host": "10.64.16.215", "lag": 0.4, "type": "db"}, "servedby": "mw"})
        .to_string(),
    );
    let lagged = wd.details(&ctx, &game("Q1", None)).await;
    assert!(
        matches!(
            lagged,
            Err(ConnectorError::Http(HttpError::RateLimited { .. }))
        ),
        "{lagged:?}"
    );
    http.record(
        "GET",
        &url,
        200,
        json!({"error": {"code": "ratelimited", "info": "slow down"}}).to_string(),
    );
    let limited = wd.details(&ctx, &game("Q1", None)).await;
    assert!(matches!(
        limited,
        Err(ConnectorError::Http(HttpError::RateLimited { .. }))
    ));
    http.record(
        "GET",
        &url,
        200,
        json!({"error": {"code": "internal_api_error_X", "info": "boom"}}).to_string(),
    );
    let other = wd.details(&ctx, &game("Q1", None)).await;
    assert!(matches!(other, Err(ConnectorError::Parse(_))), "{other:?}");
}

#[tokio::test]
async fn a_long_or_odd_query_is_sent_as_plain_words() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    let long = "wort ".repeat(100);
    let _ = wd.search(&ctx, &query(&long, 50, None)).await;
    let _ = wd.search(&ctx, &query("  Zug \t um\nZug  ", 0, None)).await;
    let urls = urls(&http);
    let typed = |u: &str| {
        let parsed = url::Url::parse(u).unwrap();
        let (_, v) = parsed.query_pairs().find(|(k, _)| k == "srsearch").unwrap();
        v.split(" haswbstatement:").next().unwrap().to_string()
    };
    assert!(typed(&urls[0]).chars().count() <= 200);
    assert!(urls[0].contains("srlimit=20&"), "at most 20 hits are asked");
    assert_eq!(typed(&urls[1]), "Zug um Zug");
    assert!(urls[1].contains("srlimit=1&"));
}

/// The search syntax of the source (keywords such as `haswbstatement:`, `OR`, a leading `-`,
/// quotes) is not passed on from what a person typed: their words stay words and the class
/// filter stays the only condition.
#[tokio::test]
async fn search_syntax_in_the_typed_text_is_sent_as_words() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    let _ = wd
        .search(
            &ctx,
            &query(
                r#"Catan OR haswbstatement:P31=Q5 -"Seafarers" !x && 7 Wonders: Duel"#,
                5,
                None,
            ),
        )
        .await;
    let u = &urls(&http)[0];
    let parsed = url::Url::parse(u).unwrap();
    let (_, v) = parsed.query_pairs().find(|(k, _)| k == "srsearch").unwrap();
    let (typed, filter) = v.split_once(" haswbstatement:").unwrap();
    assert_eq!(
        typed,
        "Catan or haswbstatement P31=Q5 Seafarers x 7 Wonders Duel"
    );
    assert!(filter.starts_with("P31=Q131436|"), "{filter}");
    assert_eq!(v.matches("haswbstatement:").count(), 1);
}

/// v1-wikidata: the registry asks Wikidata for board games once the operator contact is
/// set, and names it as waiting for the contact until then; its requests take their turn
/// from the budget shared by every process (v1-books, LB-19).
#[test]
fn the_registry_asks_wikidata_for_board_games_once_the_contact_is_set() {
    use catalog_core::{ConnectorPolicy, CredentialState, OperatorContact, Registry};
    let contact = OperatorContact::default();
    let mut r = Registry::default().with_contact(contact.clone());
    r.register(Arc::new(Wikidata::new()));
    assert_eq!(r.for_kind("board_game", Capability::Search).len(), 0);
    assert_eq!(
        r.skipped(&["board_game".into()], Capability::Search),
        [("wikidata".to_string(), CredentialState::NeedsContact)]
    );
    contact.set(Some("games@example.org")).unwrap();
    let found = r.for_kind("board_game", Capability::Search);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].info().id, "wikidata");
    assert_eq!(r.for_kind("board_game", Capability::Details).len(), 1);
    assert_eq!(r.for_kind("board_game", Capability::Images).len(), 0);
    assert_eq!(r.for_kind("movie", Capability::Search).len(), 0);

    let policy = ConnectorPolicy::from_info(Wikidata::new().info());
    assert!(policy.shared_pace);
    assert_eq!(policy.contact, ContactUse::Required);
    assert_eq!(policy.hosts, ["www.wikidata.org"]);
}

/// WD-1: the sibling classes that whole families of common games are filed under (dice games
/// such as Yahtzee, abstract strategy games, chess variants such as shogi, children's games,
/// parlour and party games, connection games, xiangqi variants) are in the filter; measured
/// live on 2026-11-13, the filter keeps 5155 of the 5251 items with a BoardGameGeek id
/// (98.2 %, docs/connectors/wikidata.md). The longest search (200 typed characters and the
/// filter) was accepted live with a filter of 513 characters.
#[test]
fn the_class_list_holds_the_sibling_classes_of_common_games() {
    for class in [
        "Q1515156",   // dice game
        "Q573573",    // abstract strategy game
        "Q299191",    // chess variant
        "Q1509934",   // children's game
        "Q14947863",  // parlour game
        "Q839864",    // party game
        "Q5161688",   // connection game
        "Q133889197", // xiangqi variant
    ] {
        assert!(CLASSES.contains(&class), "{class}");
    }
    // game of skill (Q1368898) is left out on purpose: darts, laser tag, egg-and-spoon race
    assert!(!CLASSES.contains(&"Q1368898"));
    let filter = CLASSES
        .iter()
        .map(|c| format!("P31={c}"))
        .collect::<Vec<_>>()
        .join("|");
    assert!(filter.len() <= 513, "{} characters", filter.len());
}

/// WD-2: without a member language the labels are asked in English, `mul` and a wider set, so
/// an item that has a German label only is shown with it instead of being dropped.
#[tokio::test]
async fn without_a_language_a_hit_with_a_german_label_only_is_kept() {
    let wd = Wikidata::new();
    let http = Arc::new(ReplayHttp::default());
    let ctx = Ctx::new(http.clone(), Lane::Interactive);
    http.record(
        "GET",
        &wd.search_url("Wurfel", 5, Lane::Interactive).unwrap(),
        200,
        json!({"query": {"search": [{"ns": 0, "title": "Q7"}]}}).to_string(),
    );
    let entities = wd
        .entities_url(&["Q7".into()], true, "en", Lane::Interactive)
        .unwrap();
    assert!(
        entities.contains("languages=en%7Cmul%7Cde%7Cfr%7C"),
        "{entities}"
    );
    http.record(
        "GET",
        &entities,
        200,
        json!({"entities": {"Q7": {"type": "item", "id": "Q7", "descriptions": {},
            "claims": {}, "sitelinks": {},
            "labels": {"de": {"language": "de", "value": "Wurfelturm"}}}}})
        .to_string(),
    );
    let hits = wd.search(&ctx, &query("Wurfel", 5, None)).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Wurfelturm");
    assert_eq!(hits[0].original_title, None, "there is no English label");
}

/// WD-4: a `maxlag` or `ratelimited` answer arrives with HTTP 200; the connector asks the
/// governed client to pause the source for the answer's `Retry-After` (else the lag the
/// answer names, else 5 s), so the next call waits instead of being sent at once.
#[tokio::test]
async fn a_maxlag_answer_pauses_the_source() {
    let wd = Wikidata::new();
    let url = wd
        .entities_url(&["Q1".into()], true, "en", Lane::BackgroundRefresh)
        .unwrap();
    let cases = [
        (
            vec![("Retry-After".to_string(), "7".to_string())],
            json!({"error": {"code": "maxlag", "info": "lagged", "lag": 0.4}}),
            7_000,
        ),
        (
            vec![],
            json!({"error": {"code": "maxlag", "info": "lagged", "lag": 12.3}}),
            13_000,
        ),
        (vec![], json!({"error": {"code": "ratelimited"}}), 5_000),
    ];
    for (headers, body, wait_ms) in cases {
        let http = Arc::new(ReplayHttp::default());
        let ctx = Ctx::new(http.clone(), Lane::BackgroundRefresh);
        http.record_with_headers("GET", &url, 200, headers, body.to_string());
        let r = wd.details(&ctx, &game("Q1", None)).await;
        assert_eq!(
            r,
            Err(ConnectorError::Http(HttpError::RateLimited {
                retry_after_ms: wait_ms
            }))
        );
        assert_eq!(
            http.pauses(),
            [(
                "wikidata".to_string(),
                std::time::Duration::from_millis(wait_ms)
            )]
        );
    }
}
