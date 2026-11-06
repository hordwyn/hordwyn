//! Postgres adapters for [`ObjectIndex`] and [`AssetRepo`] (tables from store-pg migration
//! `0004_storage.sql`).

use app::ports::PortError;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::FetchPriority;
use crate::index::{ObjectClass, ObjectIndex, OpCounts, StoredObject, Tier, Usage};
use crate::service::{AssetKind, AssetRecord, AssetRepo};

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("storage db: {e}"))
}

fn corrupt(what: &str, v: &str) -> PortError {
    PortError(format!("storage db: invalid {what} `{v}`"))
}

fn to_i64(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

fn to_u64(v: i64) -> u64 {
    u64::try_from(v).unwrap_or(0)
}

const fn priority_str(p: FetchPriority) -> &'static str {
    match p {
        FetchPriority::Visible => "visible",
        FetchPriority::Library => "library",
        FetchPriority::Related => "related",
        FetchPriority::Catalog => "catalog",
    }
}

fn priority(s: &str) -> Result<FetchPriority, PortError> {
    match s {
        "visible" => Ok(FetchPriority::Visible),
        "library" => Ok(FetchPriority::Library),
        "related" => Ok(FetchPriority::Related),
        "catalog" => Ok(FetchPriority::Catalog),
        _ => Err(corrupt("priority", s)),
    }
}

#[derive(Clone)]
pub struct PgObjectIndex {
    pool: PgPool,
}

impl PgObjectIndex {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

struct ObjRow {
    key: String,
    class: String,
    tier: String,
    bytes: i64,
    owner_id: Option<Uuid>,
    asset_id: Option<Uuid>,
    priority: String,
    last_accessed: DateTime<Utc>,
}

impl TryFrom<ObjRow> for StoredObject {
    type Error = PortError;

    fn try_from(r: ObjRow) -> Result<Self, PortError> {
        Ok(Self {
            class: ObjectClass::parse(&r.class).ok_or_else(|| corrupt("class", &r.class))?,
            tier: Tier::parse(&r.tier).ok_or_else(|| corrupt("tier", &r.tier))?,
            key: r.key,
            bytes: to_u64(r.bytes),
            owner: r.owner_id,
            asset: r.asset_id,
            priority: priority(&r.priority)?,
            last_accessed: r.last_accessed,
        })
    }
}

#[async_trait]
impl ObjectIndex for PgObjectIndex {
    async fn record(&self, o: &StoredObject) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO stored_objects (key, class, tier, bytes, owner_id, asset_id, priority, last_accessed)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (key) DO UPDATE SET class = $2, tier = $3, bytes = $4, owner_id = $5,
                 asset_id = $6, priority = $7, last_accessed = $8",
            o.key,
            o.class.as_str(),
            o.tier.as_str(),
            to_i64(o.bytes),
            o.owner,
            o.asset,
            priority_str(o.priority),
            o.last_accessed
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<StoredObject>, PortError> {
        sqlx::query_as!(
            ObjRow,
            "SELECT key, class, tier, bytes, owner_id, asset_id, priority, last_accessed
             FROM stored_objects WHERE key = $1",
            key
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(StoredObject::try_from)
        .transpose()
    }

    async fn touch(&self, key: &str, at: DateTime<Utc>) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE stored_objects SET last_accessed = $2 WHERE key = $1",
            key,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn remove(&self, keys: &[String]) -> Result<(), PortError> {
        sqlx::query!("DELETE FROM stored_objects WHERE key = ANY($1)", keys)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }

    async fn usage(&self) -> Result<Usage, PortError> {
        let r = sqlx::query!(
            r#"SELECT
                 COALESCE(SUM(bytes), 0)::bigint AS "total!",
                 COALESCE(SUM(bytes) FILTER (WHERE tier = 'pinned'), 0)::bigint AS "pinned!",
                 COALESCE(SUM(bytes) FILTER (WHERE tier = 'cache'), 0)::bigint AS "cache!",
                 COALESCE(SUM(bytes) FILTER (WHERE class = 'http_cache'), 0)::bigint AS "http!",
                 COUNT(*) AS "objects!"
               FROM stored_objects"#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pe)?;
        Ok(Usage {
            total: to_u64(r.total),
            pinned: to_u64(r.pinned),
            cache: to_u64(r.cache),
            http_cache: to_u64(r.http),
            objects: to_u64(r.objects),
        })
    }

    async fn upload_bytes(&self, owner: Uuid) -> Result<u64, PortError> {
        let r = sqlx::query_scalar!(
            r#"SELECT COALESCE(SUM(bytes), 0)::bigint AS "n!" FROM stored_objects
               WHERE class = 'upload' AND owner_id = $1"#,
            owner
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pe)?;
        Ok(to_u64(r))
    }

    async fn eviction_candidates(
        &self,
        class: Option<ObjectClass>,
        now: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<StoredObject>, PortError> {
        // Same ordering as `index::eviction_score`: (age in seconds + 1) * weight.
        sqlx::query_as!(
            ObjRow,
            "SELECT key, class, tier, bytes, owner_id, asset_id, priority, last_accessed
             FROM stored_objects
             WHERE tier = 'cache' AND ($1::text IS NULL OR class = $1)
             ORDER BY (GREATEST(EXTRACT(EPOCH FROM ($2 - last_accessed))::bigint, 0) + 1)
                      * CASE priority WHEN 'related' THEN 2 WHEN 'catalog' THEN 4 ELSE 1 END DESC,
                      key
             LIMIT $3",
            class.map(ObjectClass::as_str),
            now,
            i64::try_from(limit).unwrap_or(i64::MAX)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(StoredObject::try_from)
        .collect()
    }

    async fn set_asset_tier(&self, asset: Uuid, tier: Tier) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE stored_objects SET tier = $2 WHERE asset_id = $1",
            asset,
            tier.as_str()
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn bump_ops(&self, day: NaiveDate, d: OpCounts) -> Result<OpCounts, PortError> {
        let r = sqlx::query!(
            "INSERT INTO storage_ops (day, puts, gets, deletes, egress_bytes) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (day) DO UPDATE SET
                 puts = storage_ops.puts + EXCLUDED.puts,
                 gets = storage_ops.gets + EXCLUDED.gets,
                 deletes = storage_ops.deletes + EXCLUDED.deletes,
                 egress_bytes = storage_ops.egress_bytes + EXCLUDED.egress_bytes
             RETURNING puts, gets, deletes, egress_bytes",
            day,
            to_i64(d.puts),
            to_i64(d.gets),
            to_i64(d.deletes),
            to_i64(d.egress_bytes)
        )
        .fetch_one(&self.pool)
        .await
        .map_err(pe)?;
        Ok(OpCounts {
            puts: to_u64(r.puts),
            gets: to_u64(r.gets),
            deletes: to_u64(r.deletes),
            egress_bytes: to_u64(r.egress_bytes),
        })
    }

    async fn ops(&self, day: NaiveDate) -> Result<OpCounts, PortError> {
        let r = sqlx::query!(
            "SELECT puts, gets, deletes, egress_bytes FROM storage_ops WHERE day = $1",
            day
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.map_or_else(OpCounts::default, |r| OpCounts {
            puts: to_u64(r.puts),
            gets: to_u64(r.gets),
            deletes: to_u64(r.deletes),
            egress_bytes: to_u64(r.egress_bytes),
        }))
    }
}

#[derive(Clone)]
pub struct PgAssetRepo {
    pool: PgPool,
}

impl PgAssetRepo {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

struct AssetRow {
    id: Uuid,
    kind: String,
    source_url: Option<String>,
    connector: Option<String>,
    hash: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    thumbhash: Option<Vec<u8>>,
    dominant_color: Option<String>,
    pins: i32,
    owner_id: Option<Uuid>,
    priority: String,
    failed: bool,
    created_at: DateTime<Utc>,
}

impl TryFrom<AssetRow> for AssetRecord {
    type Error = PortError;

    fn try_from(r: AssetRow) -> Result<Self, PortError> {
        Ok(Self {
            id: r.id,
            kind: AssetKind::parse(&r.kind).ok_or_else(|| corrupt("asset kind", &r.kind))?,
            source_url: r.source_url,
            connector: r.connector,
            hash: r.hash,
            width: r.width.and_then(|v| u32::try_from(v).ok()),
            height: r.height.and_then(|v| u32::try_from(v).ok()),
            thumbhash: r.thumbhash,
            dominant_color: r.dominant_color,
            pins: u32::try_from(r.pins).unwrap_or(0),
            owner: r.owner_id,
            priority: priority(&r.priority)?,
            failed: r.failed,
            created_at: r.created_at,
        })
    }
}

#[async_trait]
impl AssetRepo for PgAssetRepo {
    async fn upsert(&self, a: &AssetRecord) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO assets (id, kind, source_url, connector, hash, width, height, thumbhash,
                                 dominant_color, pins, owner_id, priority, failed, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             ON CONFLICT (id) DO UPDATE SET kind = $2, source_url = $3, connector = $4, hash = $5,
                 width = $6, height = $7, thumbhash = $8, dominant_color = $9, pins = $10,
                 owner_id = $11, priority = $12, failed = $13",
            a.id,
            a.kind.as_str(),
            a.source_url,
            a.connector,
            a.hash,
            a.width.and_then(|v| i32::try_from(v).ok()),
            a.height.and_then(|v| i32::try_from(v).ok()),
            a.thumbhash,
            a.dominant_color,
            i32::try_from(a.pins).unwrap_or(i32::MAX),
            a.owner,
            priority_str(a.priority),
            a.failed,
            a.created_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn get(&self, id: Uuid) -> Result<Option<AssetRecord>, PortError> {
        sqlx::query_as!(AssetRow, "SELECT * FROM assets WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?
            .map(AssetRecord::try_from)
            .transpose()
    }

    async fn by_hash(&self, hash: &str) -> Result<Option<AssetRecord>, PortError> {
        sqlx::query_as!(AssetRow, "SELECT * FROM assets WHERE hash = $1", hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?
            .map(AssetRecord::try_from)
            .transpose()
    }

    async fn by_source(&self, url: &str) -> Result<Option<AssetRecord>, PortError> {
        sqlx::query_as!(AssetRow, "SELECT * FROM assets WHERE source_url = $1", url)
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?
            .map(AssetRecord::try_from)
            .transpose()
    }
}
