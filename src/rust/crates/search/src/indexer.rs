//! Outbox -> search index (D-13). Rows are claimed in batches with `FOR UPDATE SKIP LOCKED`
//! inside one transaction and marked processed only after the index accepted the batch, so a
//! crash re-indexes the batch instead of losing it. Documents are rebuilt from the current
//! rows (never from the outbox payload), which makes re-processing harmless.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use app::App;
use app::ports::{PortError, SearchDoc, SearchIndex};
use domain::ids::{CompanionId, EntryId, ItemId, UserId};
use serde_json::json;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{CATALOG, LIBRARY, PEOPLE};

fn pe(e: impl std::fmt::Display) -> PortError {
    PortError(format!("indexer: {e}"))
}

pub struct Indexer {
    app: App,
    pool: PgPool,
    index: Arc<dyn SearchIndex>,
    batch: i64,
}

#[derive(Default)]
struct Batch {
    upserts: BTreeMap<&'static str, Vec<SearchDoc>>,
    deletes: BTreeMap<&'static str, Vec<String>>,
}

impl Batch {
    fn upsert(&mut self, index: &'static str, doc: SearchDoc) {
        self.upserts.entry(index).or_default().push(doc);
    }

    fn delete(&mut self, index: &'static str, id: String) {
        self.deletes.entry(index).or_default().push(id);
    }
}

impl Indexer {
    #[must_use]
    pub fn new(app: App, pool: PgPool, index: Arc<dyn SearchIndex>) -> Self {
        Self {
            app,
            pool,
            index,
            batch: 200,
        }
    }

    /// Process one batch; returns the number of outbox rows handled.
    ///
    /// # Errors
    /// Database or index errors (the batch stays pending and is retried).
    pub async fn run_once(&self) -> Result<usize, PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        let rows = sqlx::query!(
            "SELECT id, entity, entity_id, op FROM outbox
             WHERE processed_at IS NULL ORDER BY id LIMIT $1 FOR UPDATE SKIP LOCKED",
            self.batch
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(pe)?;
        if rows.is_empty() {
            return Ok(0);
        }
        let mut b = Batch::default();
        for r in &rows {
            self.collect(&mut b, &r.entity, r.entity_id, &r.op).await?;
        }
        for (index, docs) in b.upserts {
            self.index.upsert(index, docs).await?;
        }
        for (index, ids) in b.deletes {
            self.index.delete(index, ids).await?;
        }
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        sqlx::query!(
            "UPDATE outbox SET processed_at = now() WHERE id = ANY($1)",
            &ids
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        tx.commit().await.map_err(pe)?;
        Ok(rows.len())
    }

    /// Process until the outbox is empty.
    ///
    /// # Errors
    /// See [`Indexer::run_once`].
    pub async fn drain(&self) -> Result<usize, PortError> {
        let mut total = 0;
        loop {
            let n = self.run_once().await?;
            if n == 0 {
                return Ok(total);
            }
            total += n;
        }
    }

    /// Poll the outbox until `stop` is cancelled.
    pub async fn run(&self, stop: CancellationToken) {
        loop {
            let pause = match self.run_once().await {
                Ok(0) => Duration::from_millis(500),
                Ok(_) => Duration::ZERO,
                Err(e) => {
                    tracing::warn!(error = %e, "indexing batch failed, retrying");
                    Duration::from_secs(5)
                }
            };
            tokio::select! {
                () = stop.cancelled() => return,
                () = tokio::time::sleep(pause) => {}
            }
        }
    }

    /// Queue every catalog item, library entry and companion for indexing (`cli reindex`).
    ///
    /// # Errors
    /// Database errors.
    pub async fn enqueue_all(pool: &PgPool) -> Result<u64, PortError> {
        let mut n = 0;
        for sql in [
            "INSERT INTO outbox (entity, entity_id, op) SELECT 'catalog_item', id, 'upsert' FROM catalog_items",
            "INSERT INTO outbox (entity, entity_id, op, user_id) SELECT 'library_entry', id, 'upsert', user_id FROM library_entries",
            "INSERT INTO outbox (entity, entity_id, op, user_id) SELECT 'companion', id, 'upsert', owner_id FROM companions",
        ] {
            n += sqlx::query(sql)
                .execute(pool)
                .await
                .map_err(pe)?
                .rows_affected();
        }
        Ok(n)
    }

    async fn collect(
        &self,
        b: &mut Batch,
        entity: &str,
        id: Uuid,
        op: &str,
    ) -> Result<(), PortError> {
        match entity {
            "catalog_item" => match self.app.repos.catalog.get(ItemId(id)).await.map_err(pe)? {
                Some(i) if op != "delete" => b.upsert(
                    CATALOG,
                    SearchDoc {
                        id: id.to_string(),
                        owner: None,
                        fields: json!({
                            "title": i.title,
                            "original_title": i.original_title,
                            "description": i.description,
                            "kind": i.kind.as_str(),
                            "year": i.year,
                            // always a number: Meilisearch leaves the order of documents
                            // without the attribute of a custom ranking rule undefined
                            "popularity": i.popularity.unwrap_or(0.0),
                        }),
                    },
                ),
                _ => b.delete(CATALOG, id.to_string()),
            },
            "library_entry" => self.library_doc(b, EntryId(id)).await?,
            "review" => {
                // a review changes its library entry's document
                let r = sqlx::query!(
                    "SELECT e.id FROM reviews r JOIN library_entries e
                       ON e.user_id = r.user_id AND e.item_id = r.item_id WHERE r.id = $1",
                    id
                )
                .fetch_optional(&self.pool)
                .await
                .map_err(pe)?;
                if let Some(r) = r {
                    self.library_doc(b, EntryId(r.id)).await?;
                }
            }
            "companion" => {
                match self
                    .app
                    .repos
                    .companions
                    .get(CompanionId(id))
                    .await
                    .map_err(pe)?
                {
                    Some(c) if op != "delete" && c.merged_into.is_none() => b.upsert(
                        PEOPLE,
                        SearchDoc {
                            id: id.to_string(),
                            owner: Some(c.owner),
                            fields: json!({
                                "name": c.name,
                                "notes": c.notes,
                                "source": "companion",
                                "is_self": c.is_self,
                            }),
                        },
                    ),
                    _ => b.delete(PEOPLE, id.to_string()),
                }
            }
            // other entities have no search documents
            _ => {}
        }
        Ok(())
    }

    async fn library_doc(&self, b: &mut Batch, id: EntryId) -> Result<(), PortError> {
        let Some(e) = self.app.repos.library.get_by_id(id).await.map_err(pe)? else {
            b.delete(LIBRARY, id.0.to_string());
            return Ok(());
        };
        let Some(item) = self.app.repos.catalog.get(e.item_id).await.map_err(pe)? else {
            b.delete(LIBRARY, id.0.to_string());
            return Ok(());
        };
        let review = self
            .app
            .repos
            .reviews
            .get(e.user_id, e.item_id)
            .await
            .map_err(pe)?
            .map(|r| r.body);
        b.upsert(
            LIBRARY,
            SearchDoc {
                id: id.0.to_string(),
                owner: Some(UserId(e.user_id.0)),
                fields: json!({
                    "item_id": e.item_id.0.to_string(),
                    "title": item.title,
                    "original_title": item.original_title,
                    "kind": e.kind.as_str(),
                    "status": e.status.value.as_str(),
                    "rating": e.rating.value.map(domain::library::Rating::value),
                    "notes": e.notes.value,
                    "review": review,
                    "year": item.year,
                }),
            },
        );
        Ok(())
    }
}
