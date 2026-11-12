use app::ports::CatalogRepo;
use async_trait::async_trait;
use domain::catalog::{CatalogItem, EpisodeSeason, ExternalId};
use domain::ids::{ItemId, UserId};
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
    cover_url: Option<String>,
    attributes: serde_json::Value,
    provenance: serde_json::Value,
    state: String,
    fetched_at: Option<chrono::DateTime<chrono::Utc>>,
    fresh_until: Option<chrono::DateTime<chrono::Utc>>,
    popularity: Option<f64>,
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
        item_from(row, external_ids)
    }
}

fn item_from(row: ItemRow, external_ids: Vec<ExternalId>) -> Res<CatalogItem> {
    Ok(CatalogItem {
        id: ItemId(row.id),
        kind: KindId::new(&row.kind).map_err(|_| corrupt("kind", &row.kind))?,
        title: row.title,
        original_title: row.original_title,
        year: row.year,
        description: row.description,
        cover_asset: row.cover_asset,
        cover_url: row.cover_url,
        external_ids,
        attributes: row.attributes,
        provenance: crate::conv::json("provenance", row.provenance)?,
        state: entity_state(&row.state)?,
        fetched_at: row.fetched_at,
        fresh_until: row.fresh_until,
        popularity: row.popularity,
    })
}

#[async_trait]
impl CatalogRepo for PgStore {
    async fn upsert(&self, item: &CatalogItem) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO catalog_items (id, kind, title, original_title, year, description, cover_asset,
                                        attributes, state, fetched_at, fresh_until, updated_at,
                                        cover_url, provenance, provider_data_at, popularity)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), $12, $13, $14, $15)
             ON CONFLICT (id) DO UPDATE SET kind = EXCLUDED.kind,
                 popularity = EXCLUDED.popularity,
                 provider_data_at = CASE
                     WHEN EXCLUDED.fetched_at IS NOT NULL
                          AND EXCLUDED.fetched_at IS DISTINCT FROM catalog_items.fetched_at
                     THEN EXCLUDED.fetched_at
                     -- data written to a purged item is stamped again (UX4-9)
                     WHEN catalog_items.provider_data_at IS NULL AND EXCLUDED.title <> ''
                     THEN EXCLUDED.provider_data_at
                     ELSE catalog_items.provider_data_at END,
                 provider_refetch_after = CASE WHEN EXCLUDED.title <> '' THEN NULL
                     ELSE catalog_items.provider_refetch_after END,
                 title = EXCLUDED.title,
                 original_title = EXCLUDED.original_title, year = EXCLUDED.year,
                 description = EXCLUDED.description, cover_asset = EXCLUDED.cover_asset,
                 attributes = EXCLUDED.attributes, state = EXCLUDED.state,
                 fetched_at = EXCLUDED.fetched_at, fresh_until = EXCLUDED.fresh_until,
                 updated_at = now(), cover_url = EXCLUDED.cover_url, provenance = EXCLUDED.provenance",
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
            item.fresh_until,
            item.cover_url,
            crate::conv::to_json(&item.provenance)?,
            item.fetched_at.unwrap_or_else(chrono::Utc::now),
            item.popularity
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

    async fn insert_new(
        &self,
        item: &CatalogItem,
        obtained_at: domain::Timestamp,
    ) -> Res<Option<ItemId>> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO catalog_items (id, kind, title, original_title, year, description, cover_asset,
                                        attributes, state, fetched_at, fresh_until, updated_at,
                                        cover_url, provenance, provider_data_at, popularity)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now(), $12, $13, $14, $15)",
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
            item.fresh_until,
            item.cover_url,
            crate::conv::to_json(&item.provenance)?,
            item.fetched_at.unwrap_or(obtained_at),
            item.popularity
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        // One order for every inserter, so two of them never wait on each other's ids.
        let mut ids: Vec<&ExternalId> = item.external_ids.iter().collect();
        ids.sort_by(|a, b| (&a.namespace, &a.value).cmp(&(&b.namespace, &b.value)));
        ids.dedup();
        for ext in ids {
            // waits for a concurrent uncommitted claim of the same id, then skips if it won
            let claimed = sqlx::query_scalar!(
                "INSERT INTO external_ids (item_id, kind, namespace, value) VALUES ($1, $2, $3, $4)
                 ON CONFLICT (kind, namespace, value) DO NOTHING RETURNING item_id",
                item.id.0,
                item.kind.as_str(),
                ext.namespace,
                ext.value
            )
            .fetch_optional(&mut *tx)
            .await
            .map_err(be)?;
            if claimed.is_none() {
                let owner = sqlx::query_scalar!(
                    "SELECT item_id FROM external_ids WHERE kind = $1 AND namespace = $2 AND value = $3",
                    item.kind.as_str(),
                    ext.namespace,
                    ext.value
                )
                .fetch_optional(&mut *tx)
                .await
                .map_err(be)?;
                tx.rollback().await.map_err(be)?;
                // the owner may have been deleted in between; the caller looks again
                return match owner {
                    Some(o) => Ok(Some(ItemId(o))),
                    None => Err(app::ports::RepoError::Backend(
                        app::ports::CLAIM_RACE.into(),
                    )),
                };
            }
        }
        record(&mut tx, "catalog_item", item.id.0, Op::Upsert, &[]).await?;
        tx.commit().await.map_err(be)?;
        Ok(None)
    }

    async fn provider_data_before(
        &self,
        namespaces: &[String],
        before: domain::Timestamp,
        limit: u32,
    ) -> Res<Vec<(ItemId, domain::Timestamp)>> {
        let rows = sqlx::query!(
            r#"SELECT c.id, c.provider_data_at AS "at!"
               FROM catalog_items c
               WHERE c.provider_data_at < $2
                 AND EXISTS (SELECT 1 FROM external_ids e
                             WHERE e.item_id = c.id AND e.namespace = ANY($1))
               ORDER BY c.provider_data_at, c.id
               LIMIT $3"#,
            namespaces,
            before,
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        Ok(rows.into_iter().map(|r| (ItemId(r.id), r.at)).collect())
    }

    async fn purge_provider_data(&self, id: ItemId, at: domain::Timestamp) -> Res<Option<String>> {
        let mut tx = self.begin().await?;
        let cover = sqlx::query_scalar!(
            "SELECT cover_asset FROM catalog_items WHERE id = $1 FOR UPDATE",
            id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(be)?;
        let Some(cover) = cover else {
            return Ok(None);
        };
        sqlx::query!(
            "UPDATE catalog_items SET title = '', year = NULL, description = NULL,
                 original_title = NULL, attributes = '{}'::jsonb, cover_url = NULL,
                 cover_asset = NULL, provenance = '{}'::jsonb, state = 'stub', fetched_at = NULL,
                 fresh_until = NULL, provider_data_at = NULL, provider_refetch_after = $2,
                 popularity = NULL,
                 updated_at = now()
             WHERE id = $1",
            id.0,
            at
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        sqlx::query!(
            "DELETE FROM catalog_episode_seasons WHERE item_id = $1",
            id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        record(&mut tx, "catalog_item", id.0, Op::Upsert, &[]).await?;
        // the library documents in the search index carry the title too
        sqlx::query!(
            "INSERT INTO outbox (entity, entity_id, op)
             SELECT 'library_entry', e.id, 'upsert' FROM library_entries e WHERE e.item_id = $1",
            id.0
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        tx.commit().await.map_err(be)?;
        Ok(cover)
    }

    async fn delete_unreferenced(&self, id: ItemId) -> Res<app::ports::Removal> {
        let mut tx = self.begin().await?;
        // the references are NO ACTION foreign keys: one inserted concurrently fails the
        // delete (kept, the next run looks again) instead of being lost
        let deleted = sqlx::query_scalar!(
            r#"DELETE FROM catalog_items c WHERE c.id = $1
                 AND NOT EXISTS (SELECT 1 FROM library_entries x WHERE x.item_id = c.id)
                 AND NOT EXISTS (SELECT 1 FROM list_items x WHERE x.item_id = c.id)
                 AND NOT EXISTS (SELECT 1 FROM sessions x WHERE x.item_id = c.id)
                 AND NOT EXISTS (SELECT 1 FROM progress_events x WHERE x.item_id = c.id)
                 AND NOT EXISTS (SELECT 1 FROM reviews x WHERE x.item_id = c.id)
               RETURNING c.cover_asset"#,
            id.0
        )
        .fetch_optional(&mut *tx)
        .await;
        let cover = match deleted {
            Ok(Some(cover)) => cover,
            Ok(None) => return Ok(app::ports::Removal::Kept),
            Err(sqlx::Error::Database(e)) if e.is_foreign_key_violation() => {
                return Ok(app::ports::Removal::Kept);
            }
            Err(e) => return Err(be(e)),
        };
        record(&mut tx, "catalog_item", id.0, Op::Delete, &[]).await?;
        tx.commit().await.map_err(be)?;
        Ok(app::ports::Removal::Deleted { cover_asset: cover })
    }

    async fn expired_provider_data(
        &self,
        namespaces: &[String],
        before: domain::Timestamp,
        limit: u32,
    ) -> Res<Vec<ItemId>> {
        let rows = sqlx::query_scalar!(
            "SELECT c.id FROM catalog_items c
             WHERE c.provider_refetch_after <= $2
               AND EXISTS (SELECT 1 FROM external_ids e
                           WHERE e.item_id = c.id AND e.namespace = ANY($1))
             ORDER BY c.provider_refetch_after, c.id
             LIMIT $3",
            namespaces,
            before,
            i64::from(limit)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        Ok(rows.into_iter().map(ItemId).collect())
    }

    async fn touch_expired(&self, id: ItemId, at: domain::Timestamp) -> Res<()> {
        sqlx::query!(
            "UPDATE catalog_items SET provider_refetch_after = $2
             WHERE id = $1 AND provider_refetch_after IS NOT NULL",
            id.0,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(be)?;
        Ok(())
    }

    async fn get(&self, id: ItemId) -> Res<Option<CatalogItem>> {
        let row = sqlx::query_as!(
            ItemRow,
            "SELECT id, kind, title, original_title, year, description, cover_asset, attributes,
                    state, fetched_at, fresh_until, cover_url, provenance, popularity
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
                    c.attributes, c.state, c.fetched_at, c.fresh_until, c.cover_url, c.provenance,
                    c.popularity
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

    async fn get_many(&self, ids: &[ItemId]) -> Res<Vec<CatalogItem>> {
        let ids: Vec<uuid::Uuid> = ids.iter().map(|i| i.0).collect();
        let rows = sqlx::query_as!(
            ItemRow,
            "SELECT id, kind, title, original_title, year, description, cover_asset, attributes,
                    state, fetched_at, fresh_until, cover_url, provenance, popularity
             FROM catalog_items WHERE id = ANY($1)",
            &ids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        let mut external: std::collections::HashMap<uuid::Uuid, Vec<ExternalId>> =
            std::collections::HashMap::new();
        for r in sqlx::query!(
            "SELECT item_id, namespace, value FROM external_ids WHERE item_id = ANY($1)
             ORDER BY item_id, namespace, value",
            &ids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        {
            external
                .entry(r.item_id)
                .or_default()
                .push(ExternalId::new(r.namespace, r.value));
        }
        rows.into_iter()
            .map(|row| {
                let ext = external.remove(&row.id).unwrap_or_default();
                item_from(row, ext)
            })
            .collect()
    }

    async fn episode_seasons(&self, item: ItemId) -> Res<Vec<EpisodeSeason>> {
        episodes::seasons(self, item).await
    }

    async fn put_episode_seasons(&self, item: ItemId, seasons: &[EpisodeSeason]) -> Res<()> {
        episodes::put(self, item, seasons).await
    }

    async fn audience(&self, id: ItemId, now: domain::Timestamp) -> Res<app::ports::ItemAudience> {
        // one statement per change (DEC-490); every lookup is by an index on the item
        let row = sqlx::query!(
            r#"SELECT EXISTS (SELECT 1 FROM list_items li JOIN lists l ON l.id = li.list_id
                              WHERE li.item_id = $1 AND l.visibility ->> 'type' = 'instance') AS "everyone!",
                      ARRAY(
                          SELECT user_id FROM library_entries WHERE item_id = $1
                          UNION SELECT user_id FROM progress_events WHERE item_id = $1
                          UNION SELECT user_id FROM reviews WHERE item_id = $1
                          UNION SELECT l.owner_id FROM list_items li JOIN lists l ON l.id = li.list_id
                                WHERE li.item_id = $1
                          UNION SELECT owner_id FROM sessions WHERE item_id = $1
                          UNION SELECT c.linked_user FROM sessions s
                                JOIN users o ON o.id = s.owner_id AND o.share_sessions
                                JOIN session_participants p ON p.session_id = s.id
                                JOIN companions c ON c.id = p.companion_id
                                WHERE s.item_id = $1 AND c.linked_user IS NOT NULL
                          UNION SELECT user_id FROM catalog_watchers WHERE item_id = $1 AND until > $2
                      ) AS "users!: Vec<uuid::Uuid>""#,
            id.0,
            now
        )
        .fetch_one(&self.pool)
        .await
        .map_err(be)?;
        Ok(if row.everyone {
            app::ports::ItemAudience::Everyone
        } else {
            app::ports::ItemAudience::Users(row.users.into_iter().map(UserId).collect())
        })
    }

    async fn watch(
        &self,
        id: ItemId,
        user: UserId,
        until: domain::Timestamp,
        now: domain::Timestamp,
    ) -> Res<()> {
        // the user's ended watches go with the new one, so the table stays at the pages
        // opened within the last watch time
        sqlx::query!(
            "WITH ended AS (DELETE FROM catalog_watchers WHERE user_id = $2 AND item_id <> $1 AND until < $4)
             INSERT INTO catalog_watchers (item_id, user_id, until) VALUES ($1, $2, $3)
             ON CONFLICT (item_id, user_id) DO UPDATE SET until = EXCLUDED.until",
            id.0,
            user.0,
            until,
            now
        )
        .execute(&self.pool)
        .await
        .map_err(be)?;
        Ok(())
    }
}

/// Per-episode release dates (WP4-7, migration 0027).
mod episodes {
    use domain::catalog::{CatalogEpisode, EpisodeSeason};
    use domain::ids::ItemId;

    use crate::conv::{to_i32, to_u32};
    use crate::{PgStore, Res, be};

    fn int(v: u32) -> Res<i32> {
        Ok(to_i32(Some(v))?.unwrap_or_default())
    }

    fn uint(v: i32) -> Res<u32> {
        Ok(to_u32(Some(v))?.unwrap_or_default())
    }

    pub(super) async fn seasons(store: &PgStore, item: ItemId) -> Res<Vec<EpisodeSeason>> {
        let rows = sqlx::query!(
            "SELECT season, connector, episode_count, air_date, fetched_at
             FROM catalog_episode_seasons WHERE item_id = $1 ORDER BY season",
            item.0
        )
        .fetch_all(&store.pool)
        .await
        .map_err(be)?;
        let eps = sqlx::query!(
            "SELECT season, number, air_date, runtime_minutes
             FROM catalog_episodes WHERE item_id = $1 ORDER BY season, number",
            item.0
        )
        .fetch_all(&store.pool)
        .await
        .map_err(be)?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let season = uint(r.season)?;
            let mut episodes = Vec::new();
            for e in eps.iter().filter(|e| e.season == r.season) {
                episodes.push(CatalogEpisode {
                    season,
                    number: uint(e.number)?,
                    air_date: e.air_date,
                    runtime_minutes: to_u32(e.runtime_minutes)?,
                });
            }
            out.push(EpisodeSeason {
                season,
                connector: r.connector,
                episode_count: uint(r.episode_count)?,
                air_date: r.air_date,
                fetched_at: r.fetched_at,
                episodes,
            });
        }
        Ok(out)
    }

    pub(super) async fn put(store: &PgStore, item: ItemId, seasons: &[EpisodeSeason]) -> Res<()> {
        let mut tx = store.begin().await?;
        for s in seasons {
            let season = int(s.season)?;
            sqlx::query!(
                "INSERT INTO catalog_episode_seasons
                     (item_id, season, connector, episode_count, air_date, fetched_at)
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (item_id, season) DO UPDATE SET connector = EXCLUDED.connector,
                     episode_count = EXCLUDED.episode_count, air_date = EXCLUDED.air_date,
                     fetched_at = EXCLUDED.fetched_at",
                item.0,
                season,
                s.connector,
                int(s.episode_count)?,
                s.air_date,
                s.fetched_at
            )
            .execute(&mut *tx)
            .await
            .map_err(be)?;
            sqlx::query!(
                "DELETE FROM catalog_episodes WHERE item_id = $1 AND season = $2",
                item.0,
                season
            )
            .execute(&mut *tx)
            .await
            .map_err(be)?;
            let mut numbers = Vec::with_capacity(s.episodes.len());
            let mut dates = Vec::with_capacity(s.episodes.len());
            let mut runtimes = Vec::with_capacity(s.episodes.len());
            for e in &s.episodes {
                numbers.push(int(e.number)?);
                dates.push(e.air_date);
                runtimes.push(to_i32(e.runtime_minutes)?);
            }
            sqlx::query!(
                "INSERT INTO catalog_episodes (item_id, season, number, air_date, runtime_minutes)
                 SELECT $1, $2, n, d, r
                 FROM UNNEST($3::integer[], $4::date[], $5::integer[]) AS t (n, d, r)
                 ON CONFLICT DO NOTHING",
                item.0,
                season,
                &numbers,
                &dates as &[Option<chrono::NaiveDate>],
                &runtimes as &[Option<i32>]
            )
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        }
        tx.commit().await.map_err(be)
    }
}
