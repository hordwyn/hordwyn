use std::collections::HashMap;

use app::ports::{ListRepo, RepoError};
use async_trait::async_trait;
use domain::ids::{ItemId, ListId, UserId};
use domain::list::{List, ListItem};
use sqlx::{Postgres, Transaction};

use crate::conv::{json, to_json};
use crate::tx::{Op, record};
use crate::{PgStore, Res, be};

struct ListRow {
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
    name: String,
    description: Option<String>,
    visibility: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl PgStore {
    async fn load_lists(&self, rows: Vec<ListRow>) -> Res<Vec<List>> {
        let ids: Vec<uuid::Uuid> = rows.iter().map(|r| r.id).collect();
        let mut items: HashMap<uuid::Uuid, Vec<ListItem>> = HashMap::new();
        for r in sqlx::query!(
            "SELECT list_id, item_id, note, added_at FROM list_items
             WHERE list_id = ANY($1) ORDER BY list_id, position",
            &ids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        {
            items.entry(r.list_id).or_default().push(ListItem {
                item_id: ItemId(r.item_id),
                note: r.note,
                added_at: r.added_at,
            });
        }
        rows.into_iter()
            .map(|r| {
                Ok(List {
                    id: ListId(r.id),
                    owner: UserId(r.owner_id),
                    name: r.name,
                    description: r.description,
                    visibility: json("visibility", r.visibility)?,
                    items: items.remove(&r.id).unwrap_or_default(),
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
            })
            .collect::<Result<_, RepoError>>()
    }
}

async fn write_list(tx: &mut Transaction<'static, Postgres>, l: &List) -> Res<()> {
    sqlx::query!(
        "INSERT INTO lists (id, owner_id, name, description, visibility, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, description = EXCLUDED.description,
             visibility = EXCLUDED.visibility, updated_at = EXCLUDED.updated_at",
        l.id.0,
        l.owner.0,
        l.name,
        l.description,
        to_json(&l.visibility)?,
        l.created_at,
        l.updated_at
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    sqlx::query!("DELETE FROM list_items WHERE list_id = $1", l.id.0)
        .execute(&mut **tx)
        .await
        .map_err(be)?;
    for (pos, it) in l.items.iter().enumerate() {
        let pos = i32::try_from(pos).map_err(|_| RepoError::Backend("list too long".into()))?;
        sqlx::query!(
            "INSERT INTO list_items (list_id, item_id, position, note, added_at) VALUES ($1, $2, $3, $4, $5)",
            l.id.0,
            it.item_id.0,
            pos,
            it.note,
            it.added_at
        )
        .execute(&mut **tx)
        .await
        .map_err(be)?;
    }
    record(tx, "list", l.id.0, Op::Upsert, &[l.owner]).await
}

#[async_trait]
impl ListRepo for PgStore {
    async fn insert(&self, l: &List) -> Res<()> {
        ListRepo::update(self, l).await
    }

    async fn get(&self, id: ListId) -> Res<Option<List>> {
        let rows = sqlx::query_as!(ListRow, "SELECT * FROM lists WHERE id = $1", id.0)
            .fetch_all(&self.pool)
            .await
            .map_err(be)?;
        Ok(self.load_lists(rows).await?.pop())
    }

    async fn update(&self, l: &List) -> Res<()> {
        let mut tx = self.begin().await?;
        write_list(&mut tx, l).await?;
        tx.commit().await.map_err(be)
    }

    async fn delete(&self, id: ListId) -> Res<()> {
        let mut tx = self.begin().await?;
        let owner = sqlx::query_scalar!("DELETE FROM lists WHERE id = $1 RETURNING owner_id", id.0)
            .fetch_optional(&mut *tx)
            .await
            .map_err(be)?;
        let users: Vec<UserId> = owner.map(UserId).into_iter().collect();
        record(&mut tx, "list", id.0, Op::Delete, &users).await?;
        tx.commit().await.map_err(be)
    }

    async fn by_owner(&self, owner: UserId) -> Res<Vec<List>> {
        let rows = sqlx::query_as!(
            ListRow,
            "SELECT * FROM lists WHERE owner_id = $1 ORDER BY created_at, id",
            owner.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        self.load_lists(rows).await
    }
}
