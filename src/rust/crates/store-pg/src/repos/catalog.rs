use app::ports::CatalogRepo;
use async_trait::async_trait;
use domain::catalog::{CatalogItem, ExternalId};
use domain::ids::ItemId;
use domain::kind::KindId;

use crate::conv::{corrupt, entity_state, entity_state_str};
use crate::tx::{Op, record};
use crate::{PgStore, Res, be};

struct ItemRow {
    id: uuid::Uuid,
    kind: String,
    title: String,
    original_title: Option<String>,
    year: Option<i32>,
    description: Option<String>,
    cover_asset: Option<String>,
    attributes: serde_json::Value,
    state: String,
    fetched_at: Option<chrono::DateTime<chrono::Utc>>,
    fresh_until: Option<chrono::DateTime<chrono::Utc>>,
}

impl PgStore {
    async fn load_item(&self, row: ItemRow) -> Res<CatalogItem> {
        let external_ids = sqlx::query!(
            "SELECT namespace, value FROM external_ids WHERE item_id = $1 ORDER BY namespace, value",
            row.id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(|r| ExternalId::new(r.namespace, r.value))
        .collect();
        Ok(CatalogItem {
            id: ItemId(row.id),
            kind: KindId::new(&row.kind).map_err(|_| corrupt("kind", &row.kind))?,
            title: row.title,
            original_title: row.original_title,
            year: row.year,
            description: row.description,
            cover_asset: row.cover_asset,
            external_ids,
            attributes: row.attributes,
            state: entity_state(&row.state)?,
            fetched_at: row.fetched_at,
            fresh_until: row.fresh_until,
        })
    }
}

#[async_trait]
impl CatalogRepo for PgStore {
    async fn upsert(&self, item: &CatalogItem) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO catalog_items (id, kind, title, original_title, year, description, cover_asset,
                                        attributes, state, fetched_at, fresh_until, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now())
             ON CONFLICT (id) DO UPDATE SET kind = EXCLUDED.kind, title = EXCLUDED.title,
                 original_title = EXCLUDED.original_title, year = EXCLUDED.year,
                 description = EXCLUDED.description, cover_asset = EXCLUDED.cover_asset,
                 attributes = EXCLUDED.attributes, state = EXCLUDED.state,
                 fetched_at = EXCLUDED.fetched_at, fresh_until = EXCLUDED.fresh_until,
                 updated_at = now()",
            item.id.0,
            item.kind.as_str(),
            item.title,
            item.original_title,
            item.year,
            item.description,
            item.cover_asset,
            item.attributes,
            entity_state_str(item.state),
            item.fetched_at,
            item.fresh_until
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        for ext in &item.external_ids {
            // first item to claim an external id keeps it
            sqlx::query!(
                "INSERT INTO external_ids (item_id, kind, namespace, value) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (kind, namespace, value) DO NOTHING",
                item.id.0,
                item.kind.as_str(),
                ext.namespace,
                ext.value
            )
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        }
        record(&mut tx, "catalog_item", item.id.0, Op::Upsert, &[]).await?;
        tx.commit().await.map_err(be)
    }

    async fn get(&self, id: ItemId) -> Res<Option<CatalogItem>> {
        let row = sqlx::query_as!(
            ItemRow,
            "SELECT id, kind, title, original_title, year, description, cover_asset, attributes,
                    state, fetched_at, fresh_until
             FROM catalog_items WHERE id = $1",
            id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?;
        match row {
            Some(r) => Ok(Some(self.load_item(r).await?)),
            None => Ok(None),
        }
    }

    async fn by_external_id(&self, kind: &KindId, id: &ExternalId) -> Res<Option<CatalogItem>> {
        let row = sqlx::query_as!(
            ItemRow,
            "SELECT c.id, c.kind, c.title, c.original_title, c.year, c.description, c.cover_asset,
                    c.attributes, c.state, c.fetched_at, c.fresh_until
             FROM external_ids e JOIN catalog_items c ON c.id = e.item_id
             WHERE e.kind = $1 AND e.namespace = $2 AND e.value = $3",
            kind.as_str(),
            id.namespace,
            id.value
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?;
        match row {
            Some(r) => Ok(Some(self.load_item(r).await?)),
            None => Ok(None),
        }
    }
}
