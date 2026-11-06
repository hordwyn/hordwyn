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

async fn suite(ix: Arc<dyn SearchIndex>) {
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
    assert_eq!(
        ids(ix.as_ref(), q("catalog", "desert", b, &[])).await,
        ["c3"],
        "catalog is shared and searches descriptions"
    );

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
    suite(Arc::new(PgSearch::new(pool))).await;
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
    suite(Arc::new(ix)).await;
}
