//! Postgres adapter of [`ProviderStore`] (migration `0018_oidc.sql`). The client secret is
//! stored in the envelope format of `auth::secrets` (purpose `oidc_client_secret`, row = the
//! provider id, field `client_secret`) and re-wrapped by the rotation job ([`SecretRows`]).

use std::sync::Arc;

use app::ports::{
    PortError, RepoError, RepoResult, RewrapBatch, SecretCipher, SecretRows, SecretSlot,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::provider::{
    Capabilities, ClientSecret, Provenance, Provider, ProviderStore, RoleSource, SECRET_PURPOSE,
    SecretChange,
};
use crate::admin::AuditEntry;

const FIELD: &str = "client_secret";

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("oidc db: {e}"))
}

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn re(e: sqlx::Error) -> RepoError {
    RepoError::Backend(format!("oidc db: {e}"))
}

fn slot(row: &str) -> SecretSlot<'_> {
    SecretSlot {
        purpose: SECRET_PURPOSE,
        row,
        field: FIELD,
    }
}

#[derive(Clone)]
pub struct PgProviderStore {
    pool: PgPool,
    cipher: Arc<dyn SecretCipher>,
}

impl PgProviderStore {
    #[must_use]
    pub fn new(pool: PgPool, cipher: Arc<dyn SecretCipher>) -> Self {
        Self { pool, cipher }
    }

    fn secret(&self, id: Uuid, blob: Option<&[u8]>) -> ClientSecret {
        let Some(blob) = blob else {
            return ClientSecret::None;
        };
        let row = id.to_string();
        match self
            .cipher
            .open(slot(&row), blob)
            .ok()
            .and_then(|p| String::from_utf8(Zeroizing::new(p).to_vec()).ok())
        {
            Some(s) => ClientSecret::Set(Zeroizing::new(s)),
            None => ClientSecret::Locked,
        }
    }

    fn provider(&self, r: Row) -> Provider {
        Provider {
            client_secret: self.secret(r.id, r.client_secret.as_deref()),
            id: r.id,
            slug: r.slug,
            display_name: r.display_name,
            issuer: r.issuer,
            client_id: r.client_id,
            scopes: r.scopes,
            enabled: r.enabled,
            is_default: r.is_default,
            auto_create: r.auto_create,
            groups_claim: r.groups_claim,
            allowed_groups: r.allowed_groups,
            admin_groups: r.admin_groups,
            role_source: RoleSource::parse(&r.role_source).unwrap_or_default(),
            provenance: if r.provenance == "env" {
                Provenance::Env
            } else {
                Provenance::Db
            },
            capabilities: serde_json::from_value(r.capabilities).unwrap_or_default(),
            discovery: r.discovery,
            discovery_fetched_at: r.discovery_fetched_at,
            last_test: r.last_test,
            last_test_at: r.last_test_at,
            last_callback_at: r.last_callback_at,
            last_callback_result: r.last_callback_result,
            created_by: r.created_by.map(UserId),
            updated_by: r.updated_by.map(UserId),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

struct Row {
    id: Uuid,
    slug: String,
    display_name: String,
    issuer: String,
    client_id: String,
    client_secret: Option<Vec<u8>>,
    scopes: Vec<String>,
    enabled: bool,
    is_default: bool,
    auto_create: bool,
    groups_claim: String,
    allowed_groups: Vec<String>,
    admin_groups: Vec<String>,
    role_source: String,
    provenance: String,
    discovery: Option<Value>,
    discovery_fetched_at: Option<DateTime<Utc>>,
    capabilities: Value,
    last_test: Option<Value>,
    last_test_at: Option<DateTime<Utc>>,
    last_callback_at: Option<DateTime<Utc>>,
    last_callback_result: Option<String>,
    created_by: Option<Uuid>,
    updated_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn audit_all(tx: &mut sqlx::PgConnection, audit: &[AuditEntry]) -> Result<(), PortError> {
    for e in audit {
        crate::admin_pg::insert_audit(tx, e).await?;
    }
    Ok(())
}

#[async_trait]
impl ProviderStore for PgProviderStore {
    async fn providers(&self) -> Result<Vec<Provider>, PortError> {
        let rows = sqlx::query_as!(
            Row,
            "SELECT id, slug, display_name, issuer, client_id, client_secret, scopes, enabled,
                    is_default, auto_create, groups_claim, allowed_groups, admin_groups,
                    role_source, provenance, discovery, discovery_fetched_at, capabilities,
                    last_test, last_test_at, last_callback_at, last_callback_result,
                    created_by, updated_by, created_at, updated_at
             FROM oidc_providers ORDER BY display_name, slug"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?;
        Ok(rows.into_iter().map(|r| self.provider(r)).collect())
    }

    async fn save(
        &self,
        p: &Provider,
        secret: SecretChange<'_>,
        audit: &[AuditEntry],
    ) -> Result<bool, PortError> {
        let row = p.id.to_string();
        let (set, blob) = match secret {
            SecretChange::Keep => (false, None),
            SecretChange::Set(s) => (true, Some(self.cipher.seal(slot(&row), s.as_bytes()))),
            SecretChange::Clear => (true, None),
        };
        let mut tx = self.pool.begin().await.map_err(pe)?;
        let taken = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM oidc_providers WHERE slug = $1 AND id <> $2) AS "e!""#,
            p.slug,
            p.id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(pe)?;
        if taken {
            return Ok(false);
        }
        if p.is_default {
            sqlx::query!(
                "UPDATE oidc_providers SET is_default = false WHERE is_default AND id <> $1",
                p.id
            )
            .execute(&mut *tx)
            .await
            .map_err(pe)?;
        }
        sqlx::query!(
            "INSERT INTO oidc_providers (id, slug, display_name, issuer, client_id, client_secret,
                 scopes, enabled, is_default, auto_create, groups_claim, allowed_groups,
                 admin_groups, role_source, provenance, created_by, updated_by, created_at,
                 updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
                     $18, $19)
             ON CONFLICT (id) DO UPDATE SET slug = $2, display_name = $3, issuer = $4,
                 client_id = $5,
                 client_secret = CASE WHEN $20 THEN $6 ELSE oidc_providers.client_secret END,
                 scopes = $7, enabled = $8, is_default = $9, auto_create = $10,
                 groups_claim = $11, allowed_groups = $12, admin_groups = $13,
                 role_source = $14, provenance = $15, updated_by = $17, updated_at = $19",
            p.id,
            p.slug,
            p.display_name,
            p.issuer,
            p.client_id,
            blob,
            &p.scopes,
            p.enabled,
            p.is_default,
            p.auto_create,
            p.groups_claim,
            &p.allowed_groups,
            &p.admin_groups,
            p.role_source.as_str(),
            p.provenance.as_str(),
            p.created_by.map(|u| u.0),
            p.updated_by.map(|u| u.0),
            p.created_at,
            p.updated_at,
            set
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        audit_all(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)?;
        Ok(true)
    }

    async fn delete(&self, id: Uuid, audit: &[AuditEntry]) -> Result<bool, PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        let n = sqlx::query!("DELETE FROM oidc_providers WHERE id = $1", id)
            .execute(&mut *tx)
            .await
            .map_err(pe)?
            .rows_affected();
        if n == 0 {
            return Ok(false);
        }
        audit_all(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)?;
        Ok(true)
    }

    async fn record_discovery(
        &self,
        id: Uuid,
        doc: &Value,
        caps: &Capabilities,
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        let caps = serde_json::to_value(caps).unwrap_or(Value::Null);
        sqlx::query!(
            "UPDATE oidc_providers SET discovery = $2, capabilities = $3, discovery_fetched_at = $4
             WHERE id = $1",
            id,
            doc,
            caps,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn record_test(
        &self,
        id: Uuid,
        result: &Value,
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE oidc_providers SET last_test = $2, last_test_at = $3 WHERE id = $1",
            id,
            result,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn record_callback(
        &self,
        id: Uuid,
        result: &str,
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE oidc_providers SET last_callback_result = $2, last_callback_at = $3
             WHERE id = $1",
            id,
            result,
            at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn claim_once(
        &self,
        key: &str,
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<bool, PortError> {
        sqlx::query!("DELETE FROM oidc_used_states WHERE expires_at <= $1", now)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(sqlx::query!(
            "INSERT INTO oidc_used_states (state_hash, expires_at) VALUES ($1, $2)
             ON CONFLICT (state_hash) DO NOTHING",
            key,
            expires_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected()
            == 1)
    }

    async fn claim_jti(
        &self,
        key: &str,
        expires_at: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<bool, PortError> {
        sqlx::query!("DELETE FROM oidc_used_jti WHERE expires_at <= $1", now)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(sqlx::query!(
            "INSERT INTO oidc_used_jti (jti_key, expires_at) VALUES ($1, $2)
             ON CONFLICT (jti_key) DO NOTHING",
            key,
            expires_at
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?
        .rows_affected()
            == 1)
    }

    async fn release_jti(&self, key: &str) -> Result<(), PortError> {
        sqlx::query!("DELETE FROM oidc_used_jti WHERE jti_key = $1", key)
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }
}

/// The client secret column for the rotation job and the security page.
#[async_trait]
impl SecretRows for PgProviderStore {
    fn purpose(&self) -> &'static str {
        SECRET_PURPOSE
    }

    async fn seal_plaintext(&self, _cipher: &dyn SecretCipher) -> RepoResult<u64> {
        // the column never held plain text (environment secrets are not stored)
        Ok(0)
    }

    async fn version_counts(&self) -> RepoResult<Vec<(u16, u64)>> {
        crate::pg::version_counts(&self.pool, "oidc_providers", "client_secret").await
    }

    async fn rewrap_batch(
        &self,
        cipher: &dyn SecretCipher,
        active: u16,
        after: Option<Uuid>,
        limit: u32,
    ) -> RepoResult<RewrapBatch> {
        let rows = sqlx::query!(
            r#"SELECT id, client_secret AS "blob!" FROM oidc_providers
               WHERE client_secret IS NOT NULL
                 AND ($3::uuid IS NULL OR id > $3)
                 AND CASE WHEN length(client_secret) >= 3
                     THEN get_byte(client_secret, 1) * 256 + get_byte(client_secret, 2) <> $1
                     ELSE false END
               ORDER BY id LIMIT $2"#,
            i32::from(active),
            i64::from(limit),
            after
        )
        .fetch_all(&self.pool)
        .await
        .map_err(re)?;
        let last = rows.last().map(|r| r.id);
        let (mut rewrapped, mut locked) = (0, 0);
        for r in rows {
            let row = r.id.to_string();
            match cipher.rewrap(slot(&row), &r.blob) {
                Ok(Some(new)) => {
                    rewrapped += sqlx::query!(
                        "UPDATE oidc_providers SET client_secret = $3
                         WHERE id = $1 AND client_secret = $2",
                        r.id,
                        r.blob,
                        new
                    )
                    .execute(&self.pool)
                    .await
                    .map_err(re)?
                    .rows_affected();
                }
                Ok(None) => {}
                Err(_) => locked += 1,
            }
        }
        Ok(RewrapBatch {
            rewrapped,
            locked,
            last,
        })
    }
}
