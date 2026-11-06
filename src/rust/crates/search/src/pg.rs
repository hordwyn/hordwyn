//! Postgres fallback adapter: full-text search (`simple` configuration, weighted by field
//! importance) plus `pg_trgm` similarity, in table `search_docs` (store-pg migration 0007).

use app::ports::{PortError, SearchDoc, SearchHit, SearchIndex, SearchQuery};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;

use crate::{is_private, searchable};

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("search db: {e}"))
}

#[derive(Clone)]
pub struct PgSearch {
    pool: PgPool,
}

impl PgSearch {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Remove every document of an index (full reindex).
    ///
    /// # Errors
    /// Database errors.
    pub async fn clear(&self, index: &str) -> Result<(), PortError> {
        sqlx::query!("DELETE FROM search_docs WHERE index_name = $1", index)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(text_of).collect::<Vec<_>>().join(" "),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Searchable text per weight class: A = first searchable attribute (title), B = second,
/// C = the rest.
fn weighted(index: &str, fields: &Value) -> [String; 3] {
    let attrs = searchable(index);
    let get = |a: &str| fields.get(a).map(text_of).unwrap_or_default();
    [
        attrs.first().map(|a| get(a)).unwrap_or_default(),
        attrs.get(1).map(|a| get(a)).unwrap_or_default(),
        attrs
            .iter()
            .skip(2)
            .map(|a| get(a))
            .collect::<Vec<_>>()
            .join(" "),
    ]
}

#[async_trait]
impl SearchIndex for PgSearch {
    async fn upsert(&self, index: &str, docs: Vec<SearchDoc>) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        for d in docs {
            let [a, b, c] = weighted(index, &d.fields);
            let body = format!("{a} {b}");
            sqlx::query!(
                "INSERT INTO search_docs (index_name, id, owner_id, fields, body, tsv)
                 VALUES ($1, $2, $3, $4, $5,
                     setweight(to_tsvector('simple', $6), 'A')
                     || setweight(to_tsvector('simple', $7), 'B')
                     || setweight(to_tsvector('simple', $8), 'C'))
                 ON CONFLICT (index_name, id) DO UPDATE SET owner_id = EXCLUDED.owner_id,
                     fields = EXCLUDED.fields, body = EXCLUDED.body, tsv = EXCLUDED.tsv",
                index,
                d.id,
                d.owner.map(|o| o.0),
                d.fields,
                body.trim(),
                a,
                b,
                c
            )
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        }
        tx.commit().await.map_err(pe)
    }

    async fn delete(&self, index: &str, ids: Vec<String>) -> Result<(), PortError> {
        sqlx::query!(
            "DELETE FROM search_docs WHERE index_name = $1 AND id = ANY($2)",
            index,
            &ids
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn search(&self, q: SearchQuery) -> Result<Vec<SearchHit>, PortError> {
        crate::check(&q)?;
        let owner = is_private(&q.index).then_some(q.viewer.0);
        // allowlisted keys (checked above) and values as parameters: no SQL is built from input
        let filter = serde_json::Value::Object(
            q.filters
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect(),
        );
        let limit = i64::try_from(q.limit.clamp(1, 100)).unwrap_or(20);
        let rows = sqlx::query!(
            r#"SELECT id, fields AS "fields!"
               FROM search_docs,
                    websearch_to_tsquery('simple', $3) AS query
               WHERE index_name = $1
                 AND ($2::uuid IS NULL OR owner_id = $2)
                 AND (SELECT bool_and(fields ->> f.key = f.value)
                      FROM jsonb_each_text($4::jsonb) AS f) IS NOT FALSE
                 AND ($3 = '' OR tsv @@ query OR body % $3 OR body ILIKE '%' || $3 || '%')
               ORDER BY ts_rank(tsv, query) + similarity(body, $3) DESC, id
               LIMIT $5"#,
            q.index,
            owner,
            q.text.trim(),
            filter,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?;
        Ok(rows
            .into_iter()
            .map(|r| SearchHit {
                id: r.id,
                fields: r.fields,
            })
            .collect())
    }
}
