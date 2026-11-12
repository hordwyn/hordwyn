//! One behaviour suite, run against Meilisearch and the Postgres fallback (M08 acceptance):
//! ranking basics, filters, deletes, and tenant isolation (user A never sees B's documents).

// The suite is one scenario on purpose: each step builds on the documents of the previous one.
#![allow(clippy::too_many_lines)]

use std::sync::Arc;

use app::ports::{SearchDoc, SearchIndex, SearchQuery};
use domain::ids::UserId;
use search::meili::MeiliIndex;
use search::pg::PgSearch;
use serde_json::json;

fn user() -> UserId {
    UserId(uuid::Uuid::now_v7())
}

fn q(index: &str, text: &str, viewer: UserId, filters: &[(&str, &str)]) -> SearchQuery {
    SearchQuery {
        index: index.into(),
        text: text.into(),
        viewer,
        filters: filters
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
        limit: 20,
    }
}

async fn ids(ix: &dyn SearchIndex, query: SearchQuery) -> Vec<String> {
    ix.search(query)
        .await
        .unwrap()
        .into_iter()
        .map(|h| h.id)
        .collect()
}

/// OT-3: among the documents that hold all the words, the one whose title is the query comes
/// first and titles that start with it come before titles that hold it later. The derivative
/// titles are indexed first and repeat the words in their original title, so neither the
/// insertion order nor a text score that counts the words can give this order. `c4` ("Harry
/// Potter and the Philosopher's Stone") is already indexed.
async fn title_ties(ix: &dyn SearchIndex, viewer: UserId) {
    let doc = |id: &str, title: &str, original: &str| SearchDoc {
        id: id.into(),
        owner: None,
        fields: json!({"title": title, "original_title": original, "kind": "movie"}),
    };
    ix.upsert(
        "catalog",
        vec![
            doc(
                "d4",
                "The Making of Harry Potter",
                "The Making of Harry Potter",
            ),
            doc(
                "d2",
                "Harry Potter and the Order of the Homeless",
                "Harry Potter and the Order of the Homeless",
            ),
            doc("d3", "Harry Potter", "Hari Pottar aur Paras Patthar"),
        ],
    )
    .await
    .unwrap();
    let found = ids(ix, q("catalog", "harry potter", viewer, &[])).await;
    assert_eq!(found.len(), 4, "{found:?}");
    assert_eq!(found[0], "d3", "the title that is the query: {found:?}");
    assert_eq!(found[3], "d4", "a title that starts otherwise: {found:?}");
    let upper = ids(ix, q("catalog", "HARRY POTTER", viewer, &[])).await;
    assert_eq!(upper[0], "d3", "case does not matter: {upper:?}");
    let mut typo = ids(ix, q("catalog", "harry poter", viewer, &[])).await;
    typo.sort();
    assert_eq!(typo, ["c4", "d2", "d3", "d4"], "a typo still finds them");
    ix.delete("catalog", vec!["d2".into(), "d3".into(), "d4".into()])
        .await
        .unwrap();
}

/// OT1B-3, the owner's pair: "Harry Potter and the Order of the Homeless" must not stand
/// above the first Harry Potter film. Both titles start with the query, so the title rules
/// tie; the source's popularity decides (TMDB `vote_count`; the numbers have the size TMDB
/// shows for a film of that standing and for a fan film, they are not recorded answers).
/// The derivative is indexed first, has the shorter text and repeats the words in its
/// description, which is what the text score rewards. A title without a value comes after
/// the ones that have one.
async fn popularity_breaks_title_ties(ix: &dyn SearchIndex, viewer: UserId) {
    let doc = |id: &str, title: &str, description: &str, popularity: Option<f64>| SearchDoc {
        id: id.into(),
        owner: None,
        fields: json!({"title": title, "original_title": title, "kind": "movie",
                       "description": description,
                       // as the indexer writes it: 0 when the source gives none
                       "popularity": popularity.unwrap_or(0.0)}),
    };
    ix.upsert(
        "catalog",
        vec![
            doc(
                "p2",
                "Harry Potter and the Order of the Homeless",
                "Harry Potter. A parody of Harry Potter.",
                Some(3.0),
            ),
            doc(
                "p3",
                "Harry Potter and the Forbidden Journey",
                "Harry Potter ride film.",
                None,
            ),
            doc(
                "p1",
                "Harry Potter and the Philosopher's Stone",
                "Harry Potter has lived under the stairs at his aunt and uncle's house his \
                 whole life. But on his 11th birthday, he learns he's a powerful wizard, with \
                 a place waiting for him at the Hogwarts School of Witchcraft and Wizardry.",
                Some(28000.0),
            ),
            doc(
                "p4",
                "Harry Potter and the Chamber of Secrets",
                "Cars fly, trees fight back, and a mysterious house-elf comes to warn Harry \
                 Potter at the start of his second year at Hogwarts.",
                Some(22500.0),
            ),
        ],
    )
    .await
    .unwrap();
    for text in ["harry potter", "Harry Potter", "harry"] {
        let found = ids(ix, q("catalog", text, viewer, &[("kind", "movie")])).await;
        assert_eq!(
            found,
            ["p1", "p4", "p2", "p3"],
            "{text}: the original first, the derivative below it"
        );
    }
    ix.delete(
        "catalog",
        vec!["p1".into(), "p2".into(), "p3".into(), "p4".into()],
    )
    .await
    .unwrap();
}

/// OT1B-4: a leading article (en: the, a, an; de: der, die, das, ein, eine) does not defeat
/// the title rules. `articles` says whether the backend skips them: the Postgres fallback
/// does (DEC-612); Meilisearch ranks by the position of the word in the title, so "The
/// Avengers" comes after the titles that start with the word (recorded, not changed).
async fn leading_articles(ix: &dyn SearchIndex, viewer: UserId, articles: bool) {
    let doc = |id: &str, title: &str| SearchDoc {
        id: id.into(),
        owner: None,
        fields: json!({"title": title, "kind": "show"}),
    };
    ix.upsert(
        "catalog",
        vec![
            doc("a2", "Avengers: Endgame"),
            doc("a3", "Avengers Grimm"),
            doc("a4", "Lego Avengers"),
            doc("a1", "The Avengers"),
            doc("a5", "Der Pate"),
            doc("a6", "Pate wider Willen"),
            doc("a7", "Eine Insel"),
            doc("a8", "Insel der Hunde"),
            doc("a9", "The Thing"),
            doc("a10", "The Thing from Another World"),
        ],
    )
    .await
    .unwrap();
    let show = [("kind", "show")];
    let found = ids(ix, q("catalog", "avengers", viewer, &show)).await;
    assert_eq!(found.len(), 4, "{found:?}");
    let the = ids(ix, q("catalog", "the thing", viewer, &show)).await;
    assert_eq!(the[0], "a9", "a query with its article: {the:?}");
    let pate = ids(ix, q("catalog", "pate", viewer, &show)).await;
    let insel = ids(ix, q("catalog", "insel", viewer, &show)).await;
    if articles {
        assert_eq!(
            found[0], "a1",
            "the title that is the query but for its article"
        );
        assert_eq!(found[3], "a4", "a title that holds the word later");
        assert_eq!(pate, ["a5", "a6"], "der");
        assert_eq!(insel, ["a7", "a8"], "eine");
    } else {
        // Meilisearch 1.56: "The Avengers" and "Lego Avengers" hold the word at the same
        // position and follow the titles that start with it
        let mut head = found[..2].to_vec();
        head.sort();
        assert_eq!(head, ["a2", "a3"], "{found:?}");
        assert_eq!(pate, ["a6", "a5"], "{pate:?}");
        assert_eq!(insel, ["a8", "a7"], "{insel:?}");
    }
    ix.delete("catalog", (1..=10).map(|n| format!("a{n}")).collect())
        .await
        .unwrap();
}

async fn suite(ix: Arc<dyn SearchIndex>, articles: bool) {
    let a = user();
    let b = user();
    ix.upsert(
        "catalog",
        vec![
            SearchDoc {
                id: "c1".into(),
                owner: None,
                fields: json!({"title": "Blade Runner", "kind": "movie", "year": 1982}),
            },
            SearchDoc {
                id: "c2".into(),
                owner: None,
                fields: json!({"title": "Blade Runner 2049", "kind": "movie", "year": 2017}),
            },
            SearchDoc {
                id: "c3".into(),
                owner: None,
                fields: json!({"title": "Dune", "kind": "book", "year": 1965,
                               "description": "desert planet"}),
            },
        ],
    )
    .await
    .unwrap();

    let mut blade = ids(ix.as_ref(), q("catalog", "blade", a, &[])).await;
    blade.sort();
    assert_eq!(blade, ["c1", "c2"]);
    assert_eq!(
        ids(ix.as_ref(), q("catalog", "dune", a, &[("kind", "book")])).await,
        ["c3"]
    );
    assert!(
        ids(ix.as_ref(), q("catalog", "dune", a, &[("kind", "movie")]))
            .await
            .is_empty(),
        "filter applies"
    );
    assert!(
        ids(ix.as_ref(), q("catalog", "blade runer", a, &[]))
            .await
            .contains(&"c1".to_string()),
        "tolerates typos"
    );
    ix.upsert(
        "catalog",
        vec![SearchDoc {
            id: "c4".into(),
            owner: None,
            fields: json!({"title": "Harry Potter and the Philosopher's Stone", "kind": "book"}),
        }],
    )
    .await
    .unwrap();
    assert_eq!(
        ids(ix.as_ref(), q("catalog", "harry poter", a, &[])).await,
        ["c4"],
        "a typo in a long title (DEC-493)"
    );
    assert_eq!(
        ids(ix.as_ref(), q("catalog", "desert", b, &[])).await,
        ["c3"],
        "catalog is shared and searches descriptions"
    );
    title_ties(ix.as_ref(), a).await;
    popularity_breaks_title_ties(ix.as_ref(), a).await;
    leading_articles(ix.as_ref(), a, articles).await;

    ix.upsert(
        "library",
        vec![
            SearchDoc {
                id: "la".into(),
                owner: Some(a),
                fields: json!({"title": "Secret Diary", "notes": "only for anna",
                               "status": "completed", "kind": "book"}),
            },
            SearchDoc {
                id: "lb".into(),
                owner: Some(b),
                fields: json!({"title": "Secret Diary", "notes": "only for ben",
                               "status": "planned", "kind": "book"}),
            },
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        ids(ix.as_ref(), q("library", "secret", a, &[])).await,
        ["la"]
    );
    assert_eq!(
        ids(ix.as_ref(), q("library", "secret", b, &[])).await,
        ["lb"]
    );
    assert!(
        ids(ix.as_ref(), q("library", "ben", a, &[]))
            .await
            .is_empty(),
        "A cannot find B's notes"
    );
    assert!(
        ids(ix.as_ref(), q("library", "", a, &[("status", "planned")]))
            .await
            .is_empty(),
        "filters never widen the owner scope"
    );
    assert!(
        ix.search(q("library", "secret", a, &[("owner", &b.0.to_string())]))
            .await
            .is_err(),
        "owner is not a client filter"
    );

    ix.delete("catalog", vec!["c2".into()]).await.unwrap();
    assert_eq!(
        ids(ix.as_ref(), q("catalog", "blade", a, &[])).await,
        ["c1"]
    );
}

#[tokio::test]
async fn postgres_fallback_passes_the_suite() {
    let Some(pool) = store_pg::testing::fresh_db().await else {
        return;
    };
    suite(Arc::new(PgSearch::new(pool)), true).await;
}

#[tokio::test]
async fn meilisearch_passes_the_suite() {
    let (Ok(url), Ok(key)) = (
        std::env::var("TEST_MEILI_URL"),
        std::env::var("TEST_MEILI_KEY"),
    ) else {
        assert!(
            std::env::var_os("REQUIRE_SERVICES").is_none(),
            "REQUIRE_SERVICES is set but TEST_MEILI_URL/TEST_MEILI_KEY are missing"
        );
        eprintln!("skipping: TEST_MEILI_URL not set (run `cargo xtask services up`)");
        return;
    };
    let prefix = format!("t{}_", uuid::Uuid::now_v7().simple());
    let ix = MeiliIndex::connect(&url, &key, &prefix).await.unwrap();
    suite(Arc::new(ix), false).await;
}
