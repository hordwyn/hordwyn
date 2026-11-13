//! The log of title matches (`cli match-books --apply`, review BOOK-9): what an item was
//! before it got the id of a source by its title alone.

use domain::catalog::CatalogItem;
use domain::ids::ItemId;

use crate::conv::to_json;
use crate::{PgStore, Res, be};

/// One stored match.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogMatch {
    pub at: chrono::DateTime<chrono::Utc>,
    pub item_id: ItemId,
    pub source: String,
    pub external_id: String,
    pub title_before: String,
    pub year_before: Option<i32>,
    /// The whole item as it was (`CatalogItem` as JSON).
    pub before: serde_json::Value,
}

impl PgStore {
    /// Write down that `before` is about to get `external_id` of `source` by a title match.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn log_catalog_match(
        &self,
        before: &CatalogItem,
        source: &str,
        external_id: &str,
    ) -> Res<()> {
        sqlx::query!(
            "INSERT INTO catalog_match_log (item_id, source, external_id, before)
             VALUES ($1, $2, $3, $4)",
            before.id.0,
            source,
            external_id,
            to_json(before)?
        )
        .execute(&self.pool)
        .await
        .map_err(be)?;
        Ok(())
    }

    /// Every stored match, oldest first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn catalog_matches(&self) -> Res<Vec<CatalogMatch>> {
        let rows = sqlx::query!(
            "SELECT matched_at, item_id, source, external_id, before
             FROM catalog_match_log ORDER BY id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        Ok(rows
            .into_iter()
            .map(|r| CatalogMatch {
                at: r.matched_at,
                item_id: ItemId(r.item_id),
                source: r.source,
                external_id: r.external_id,
                title_before: r.before["title"].as_str().unwrap_or_default().to_string(),
                year_before: r.before["year"]
                    .as_i64()
                    .and_then(|y| i32::try_from(y).ok()),
                before: r.before,
            })
            .collect())
    }
}
