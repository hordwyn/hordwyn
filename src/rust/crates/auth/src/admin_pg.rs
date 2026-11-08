//! Postgres adapter for [`AdminStore`] (store-pg migration `0016_admin.sql`).

use std::collections::BTreeMap;
use std::sync::Arc;

use app::ports::{
    PortError, RepoError, RepoResult, RewrapBatch, SecretCipher, SecretLocked, SecretRows,
    SecretSlot,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::ids::UserId;
use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::admin::{
    ActorKind, AdminStore, AuditEntry, AuditFilter, CREDENTIAL_FIELD, CREDENTIAL_PURPOSE,
    LicenceRecord, MAIL_PURPOSE, StoredCredential, StoredMail, StoredSettings,
};

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn pe(e: sqlx::Error) -> PortError {
    PortError(format!("admin db: {e}"))
}

// Takes the error by value so it can be passed to `map_err` directly.
#[allow(clippy::needless_pass_by_value)]
fn re(e: sqlx::Error) -> RepoError {
    RepoError::Backend(format!("admin db: {e}"))
}

fn slot(row: &str) -> SecretSlot<'_> {
    SecretSlot {
        purpose: CREDENTIAL_PURPOSE,
        row,
        field: CREDENTIAL_FIELD,
    }
}

#[derive(Clone)]
pub struct PgAdminStore {
    pool: PgPool,
    cipher: Arc<dyn SecretCipher>,
}

impl PgAdminStore {
    #[must_use]
    pub fn new(pool: PgPool, cipher: Arc<dyn SecretCipher>) -> Self {
        Self { pool, cipher }
    }

    fn open(&self, id: Uuid, blob: &[u8]) -> StoredFields {
        let row = id.to_string();
        self.cipher.open(slot(&row), blob).and_then(|plain| {
            let plain = Zeroizing::new(plain);
            serde_json::from_slice(&plain).map_err(|_| app::ports::SecretLocked {
                version: crate::secrets::blob_version(blob),
            })
        })
    }

    fn row(&self, r: CredRow) -> StoredCredential {
        StoredCredential {
            fields: self.open(r.id, &r.secret),
            id: r.id,
            connector: r.connector_id,
            user: r.user_id.map(UserId),
            created_by: r.created_by.map(UserId),
            created_at: r.created_at,
            last_tested_at: r.last_tested_at,
            last_result: r.last_result,
        }
    }
}

type StoredFields = Result<BTreeMap<String, String>, app::ports::SecretLocked>;

/// Insert one audit row on `conn`, usually the transaction of the change it records
/// (addendum-1 3.4: "each row is written in the same transaction as the change").
///
/// # Errors
/// Database errors (the caller's transaction is then rolled back).
pub async fn insert_audit(conn: &mut sqlx::PgConnection, e: &AuditEntry) -> Result<(), PortError> {
    sqlx::query!(
        "INSERT INTO admin_audit (id, at, actor_id, actor_kind, actor_name, action, target_type,
                                  target_id, detail, client_addr, request_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        e.id,
        e.at,
        e.actor_id.map(|u| u.0),
        e.actor_kind.as_str(),
        e.actor_name,
        e.action,
        e.target_type,
        e.target_id,
        e.detail,
        e.client_addr,
        e.request_id
    )
    .execute(conn)
    .await
    .map_err(pe)?;
    Ok(())
}

async fn audit_opt(
    tx: &mut sqlx::PgConnection,
    audit: Option<&AuditEntry>,
) -> Result<(), PortError> {
    match audit {
        Some(e) => insert_audit(tx, e).await,
        None => Ok(()),
    }
}

/// The slot of the SMTP password (one row: the instance).
fn mail_slot() -> SecretSlot<'static> {
    SecretSlot {
        purpose: MAIL_PURPOSE,
        row: "instance",
        field: "password",
    }
}

/// Does any encrypted column hold a value (start-up: refuse to generate a new key then)?
///
/// # Errors
/// Database errors.
pub async fn has_encrypted_rows(pool: &PgPool) -> Result<bool, PortError> {
    sqlx::query_scalar!(
        r#"SELECT (EXISTS (SELECT 1 FROM credentials WHERE totp_secret_enc IS NOT NULL)
                OR EXISTS (SELECT 1 FROM notify_channels WHERE secret IS NOT NULL)
                OR EXISTS (SELECT 1 FROM connector_credentials)
                OR EXISTS (SELECT 1 FROM instance_settings WHERE mail_password IS NOT NULL)
                OR EXISTS (SELECT 1 FROM oidc_providers WHERE client_secret IS NOT NULL))
               AS "e!""#
    )
    .fetch_one(pool)
    .await
    .map_err(pe)
}

struct CredRow {
    id: Uuid,
    connector_id: String,
    user_id: Option<Uuid>,
    secret: Vec<u8>,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    last_tested_at: Option<DateTime<Utc>>,
    last_result: Option<String>,
}

struct AuditRow {
    id: Uuid,
    at: DateTime<Utc>,
    actor_id: Option<Uuid>,
    actor_kind: String,
    actor_name: Option<String>,
    action: String,
    target_type: Option<String>,
    target_id: Option<String>,
    detail: serde_json::Value,
    client_addr: Option<String>,
    request_id: Option<String>,
}

impl From<AuditRow> for AuditEntry {
    fn from(r: AuditRow) -> Self {
        Self {
            id: r.id,
            at: r.at,
            actor_id: r.actor_id.map(UserId),
            actor_kind: ActorKind::parse(&r.actor_kind),
            actor_name: r.actor_name,
            action: r.action,
            target_type: r.target_type,
            target_id: r.target_id,
            detail: r.detail,
            client_addr: r.client_addr,
            request_id: r.request_id,
        }
    }
}

#[async_trait]
impl AdminStore for PgAdminStore {
    async fn settings(&self) -> Result<StoredSettings, PortError> {
        let r = sqlx::query!(
            r#"SELECT hosting, signup, session_days, secure_cookies, storage_max_total_bytes,
                    storage_high_watermark, storage_low_watermark, setup_completed_at, setup_step,
                    sign_in_mode, mail_host, mail_port, mail_security, mail_username, mail_from,
                    mail_password IS NOT NULL AS "mail_password_set!", mail_last_test_at,
                    mail_last_test_result, updated_at, updated_by
             FROM instance_settings WHERE id"#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?;
        Ok(r.map(|r| StoredSettings {
            hosting: r.hosting,
            signup: r.signup,
            session_days: r.session_days.and_then(|d| u32::try_from(d).ok()),
            secure_cookies: r.secure_cookies,
            storage_max_total_bytes: r
                .storage_max_total_bytes
                .and_then(|b| u64::try_from(b).ok()),
            storage_high_watermark: r.storage_high_watermark,
            storage_low_watermark: r.storage_low_watermark,
            setup_completed_at: r.setup_completed_at,
            setup_step: r.setup_step,
            sign_in_mode: r.sign_in_mode,
            mail: StoredMail {
                host: r.mail_host,
                port: r.mail_port.and_then(|p| u16::try_from(p).ok()),
                security: r.mail_security,
                username: r.mail_username,
                from: r.mail_from,
                password_set: r.mail_password_set,
                last_test_at: r.mail_last_test_at,
                last_test_result: r.mail_last_test_result,
            },
            updated_at: r.updated_at,
            updated_by: r.updated_by.map(UserId),
        })
        .unwrap_or_default())
    }

    async fn put_settings(
        &self,
        s: &StoredSettings,
        audit: &[AuditEntry],
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "INSERT INTO instance_settings (id, hosting, signup, session_days, secure_cookies,
                 storage_max_total_bytes, storage_high_watermark, storage_low_watermark,
                 setup_completed_at, setup_step, updated_at, updated_by, sign_in_mode, mail_host,
                 mail_port, mail_security, mail_username, mail_from)
             VALUES (true, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                     $17)
             ON CONFLICT (id) DO UPDATE SET hosting = $1, signup = $2, session_days = $3,
                 secure_cookies = $4, storage_max_total_bytes = $5, storage_high_watermark = $6,
                 storage_low_watermark = $7, setup_completed_at = $8, setup_step = $9,
                 updated_at = $10, updated_by = $11, sign_in_mode = $12, mail_host = $13,
                 mail_port = $14, mail_security = $15, mail_username = $16, mail_from = $17",
            s.hosting,
            s.signup,
            s.session_days.and_then(|d| i32::try_from(d).ok()),
            s.secure_cookies,
            s.storage_max_total_bytes
                .and_then(|b| i64::try_from(b).ok()),
            s.storage_high_watermark,
            s.storage_low_watermark,
            s.setup_completed_at,
            s.setup_step,
            s.updated_at,
            s.updated_by.map(|u| u.0),
            s.sign_in_mode,
            s.mail.host,
            s.mail.port.map(i32::from),
            s.mail.security,
            s.mail.username,
            s.mail.from
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        for e in audit {
            insert_audit(&mut tx, e).await?;
        }
        tx.commit().await.map_err(pe)
    }

    async fn audit(&self, e: &AuditEntry) -> Result<(), PortError> {
        let mut conn = self.pool.acquire().await.map_err(pe)?;
        insert_audit(&mut conn, e).await
    }

    async fn audit_query(&self, f: &AuditFilter) -> Result<Vec<AuditEntry>, PortError> {
        let (exact, prefix) = match f.action.as_deref() {
            Some(a) if a.ends_with('.') => (None, Some(format!("{a}%"))),
            Some(a) => (Some(a.to_string()), None),
            None => (None, None),
        };
        Ok(sqlx::query_as!(
            AuditRow,
            "SELECT id, at, actor_id, actor_kind, actor_name, action, target_type, target_id,
                    detail, client_addr, request_id
             FROM admin_audit
             WHERE ($1::uuid IS NULL OR actor_id = $1)
               AND ($2::text IS NULL OR action = $2)
               AND ($3::text IS NULL OR action LIKE $3)
               AND ($4::text IS NULL OR target_type = $4)
               AND ($5::timestamptz IS NULL OR at < $5)
               AND ($6::timestamptz IS NULL OR at >= $6)
             ORDER BY at DESC, id DESC LIMIT $7",
            f.actor.map(|a| a.0),
            exact,
            prefix,
            f.target_type,
            f.before,
            f.after,
            i64::from(f.limit)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(AuditEntry::from)
        .collect())
    }

    async fn audit_purge(&self, before: DateTime<Utc>) -> Result<u64, PortError> {
        Ok(
            sqlx::query!("DELETE FROM admin_audit WHERE at < $1", before)
                .execute(&self.pool)
                .await
                .map_err(pe)?
                .rows_affected(),
        )
    }

    async fn credential(
        &self,
        connector: &str,
        user: Option<UserId>,
    ) -> Result<Option<StoredCredential>, PortError> {
        Ok(sqlx::query_as!(
            CredRow,
            "SELECT id, connector_id, user_id, secret, created_by, created_at, last_tested_at,
                    last_result
             FROM connector_credentials
             WHERE connector_id = $1 AND user_id IS NOT DISTINCT FROM $2",
            connector,
            user.map(|u| u.0)
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(pe)?
        .map(|r| self.row(r)))
    }

    async fn instance_credentials(&self) -> Result<Vec<StoredCredential>, PortError> {
        Ok(sqlx::query_as!(
            CredRow,
            "SELECT id, connector_id, user_id, secret, created_by, created_at, last_tested_at,
                    last_result
             FROM connector_credentials WHERE scope = 'instance' ORDER BY connector_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(|r| self.row(r))
        .collect())
    }

    async fn user_credentials(&self, user: UserId) -> Result<Vec<StoredCredential>, PortError> {
        Ok(sqlx::query_as!(
            CredRow,
            "SELECT id, connector_id, user_id, secret, created_by, created_at, last_tested_at,
                    last_result
             FROM connector_credentials WHERE user_id = $1 ORDER BY connector_id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(|r| self.row(r))
        .collect())
    }

    async fn put_credential(
        &self,
        connector: &str,
        user: Option<UserId>,
        fields: &BTreeMap<String, String>,
        by: Option<UserId>,
        at: DateTime<Utc>,
        audit: Option<&AuditEntry>,
    ) -> Result<StoredCredential, PortError> {
        let id = self
            .credential(connector, user)
            .await?
            .map_or_else(Uuid::now_v7, |c| c.id);
        let plain =
            Zeroizing::new(serde_json::to_vec(fields).map_err(|e| PortError(e.to_string()))?);
        let row = id.to_string();
        let blob = self.cipher.seal(slot(&row), &plain);
        let scope = if user.is_some() { "user" } else { "instance" };
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "INSERT INTO connector_credentials (id, connector_id, scope, user_id, secret,
                                                created_by, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id) DO UPDATE SET secret = $5, created_by = $6, created_at = $7,
                 last_tested_at = NULL, last_result = NULL",
            id,
            connector,
            scope,
            user.map(|u| u.0),
            blob,
            by.map(|u| u.0),
            at
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)?;
        Ok(StoredCredential {
            id,
            connector: connector.to_string(),
            user,
            fields: Ok(fields.clone()),
            created_by: by,
            created_at: at,
            last_tested_at: None,
            last_result: None,
        })
    }

    async fn delete_credential(
        &self,
        connector: &str,
        user: Option<UserId>,
        audit: Option<&AuditEntry>,
    ) -> Result<bool, PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        let n = sqlx::query!(
            "DELETE FROM connector_credentials
             WHERE connector_id = $1 AND user_id IS NOT DISTINCT FROM $2",
            connector,
            user.map(|u| u.0)
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?
        .rows_affected();
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)?;
        Ok(n > 0)
    }

    async fn record_test(
        &self,
        id: Uuid,
        at: DateTime<Utc>,
        result: &str,
    ) -> Result<(), PortError> {
        sqlx::query!(
            "UPDATE connector_credentials SET last_tested_at = $2, last_result = $3 WHERE id = $1",
            id,
            at,
            result
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn licences(&self) -> Result<Vec<LicenceRecord>, PortError> {
        Ok(sqlx::query!(
            "SELECT connector_id, agreement_on_file, recorded_by, recorded_at FROM source_licences
             ORDER BY connector_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(pe)?
        .into_iter()
        .map(|r| LicenceRecord {
            connector: r.connector_id,
            agreement_on_file: r.agreement_on_file,
            recorded_by: r.recorded_by.map(UserId),
            recorded_at: r.recorded_at,
        })
        .collect())
    }

    async fn put_licence(
        &self,
        l: &LicenceRecord,
        audit: Option<&AuditEntry>,
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "INSERT INTO source_licences (connector_id, agreement_on_file, recorded_by, recorded_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (connector_id) DO UPDATE SET agreement_on_file = $2, recorded_by = $3,
                 recorded_at = $4",
            l.connector,
            l.agreement_on_file,
            l.recorded_by.map(|u| u.0),
            l.recorded_at
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)
    }

    async fn source_switches(&self) -> Result<Vec<(String, bool)>, PortError> {
        Ok(
            sqlx::query!("SELECT connector_id, enabled FROM source_switches ORDER BY connector_id")
                .fetch_all(&self.pool)
                .await
                .map_err(pe)?
                .into_iter()
                .map(|r| (r.connector_id, r.enabled))
                .collect(),
        )
    }

    async fn set_source_enabled(
        &self,
        connector: &str,
        enabled: bool,
        by: Option<UserId>,
        at: DateTime<Utc>,
        audit: Option<&AuditEntry>,
    ) -> Result<(), PortError> {
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "INSERT INTO source_switches (connector_id, enabled, changed_by, changed_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (connector_id) DO UPDATE SET enabled = $2, changed_by = $3,
                 changed_at = $4",
            connector,
            enabled,
            by.map(|u| u.0),
            at
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)
    }

    async fn mail_password(&self) -> Result<Option<Result<String, SecretLocked>>, PortError> {
        let blob = sqlx::query_scalar!("SELECT mail_password FROM instance_settings WHERE id")
            .fetch_optional(&self.pool)
            .await
            .map_err(pe)?
            .flatten();
        Ok(blob.map(|b| {
            self.cipher.open(mail_slot(), &b).and_then(|plain| {
                let plain = Zeroizing::new(plain);
                String::from_utf8(plain.to_vec()).map_err(|_| SecretLocked {
                    version: crate::secrets::blob_version(&b),
                })
            })
        }))
    }

    async fn put_mail_password(
        &self,
        password: Option<&str>,
        audit: Option<&AuditEntry>,
    ) -> Result<(), PortError> {
        let blob = password.map(|p| self.cipher.seal(mail_slot(), p.as_bytes()));
        let mut tx = self.pool.begin().await.map_err(pe)?;
        sqlx::query!(
            "INSERT INTO instance_settings (id, mail_password) VALUES (true, $1)
             ON CONFLICT (id) DO UPDATE SET mail_password = $1",
            blob
        )
        .execute(&mut *tx)
        .await
        .map_err(pe)?;
        audit_opt(&mut tx, audit).await?;
        tx.commit().await.map_err(pe)
    }

    async fn record_mail_test(&self, at: DateTime<Utc>, result: &str) -> Result<(), PortError> {
        sqlx::query!(
            "INSERT INTO instance_settings (id, mail_last_test_at, mail_last_test_result)
             VALUES (true, $1, $2)
             ON CONFLICT (id) DO UPDATE SET mail_last_test_at = $1, mail_last_test_result = $2",
            at,
            result
        )
        .execute(&self.pool)
        .await
        .map_err(pe)?;
        Ok(())
    }

    async fn key_versions(&self) -> Result<Vec<(u16, DateTime<Utc>)>, PortError> {
        Ok(
            sqlx::query!("SELECT version, first_seen_at FROM secrets_keys ORDER BY version")
                .fetch_all(&self.pool)
                .await
                .map_err(pe)?
                .into_iter()
                .filter_map(|r| Some((u16::try_from(r.version).ok()?, r.first_seen_at)))
                .collect(),
        )
    }

    async fn record_key_versions(
        &self,
        versions: &[u16],
        at: DateTime<Utc>,
    ) -> Result<(), PortError> {
        for v in versions {
            sqlx::query!(
                "INSERT INTO secrets_keys (version, first_seen_at) VALUES ($1, $2)
                 ON CONFLICT (version) DO NOTHING",
                i32::from(*v),
                at
            )
            .execute(&self.pool)
            .await
            .map_err(pe)?;
        }
        Ok(())
    }

    async fn ping(&self) -> Result<(), PortError> {
        sqlx::query_scalar!(r#"SELECT 1 AS "one!""#)
            .fetch_one(&self.pool)
            .await
            .map_err(pe)?;
        Ok(())
    }
}

/// The credential column for the rotation job (no plain-text predecessor exists).
#[async_trait]
impl SecretRows for PgAdminStore {
    fn purpose(&self) -> &'static str {
        CREDENTIAL_PURPOSE
    }

    async fn seal_plaintext(&self, _cipher: &dyn SecretCipher) -> RepoResult<u64> {
        Ok(0)
    }

    async fn version_counts(&self) -> RepoResult<Vec<(u16, u64)>> {
        crate::pg::version_counts(&self.pool, "connector_credentials", "secret").await
    }

    async fn rewrap_batch(
        &self,
        cipher: &dyn SecretCipher,
        active: u16,
        after: Option<Uuid>,
        limit: u32,
    ) -> RepoResult<RewrapBatch> {
        let rows = sqlx::query!(
            "SELECT id, secret FROM connector_credentials
             WHERE ($3::uuid IS NULL OR id > $3)
               AND CASE WHEN length(secret) >= 3
                   THEN get_byte(secret, 1) * 256 + get_byte(secret, 2) <> $1
                   ELSE false END
             ORDER BY id LIMIT $2",
            i32::from(active),
            i64::from(limit),
            after
        )
        .fetch_all(&self.pool)
        .await
        .map_err(re)?;
        let last = rows.last().map(|r| r.id);
        let (mut done, mut locked) = (0, 0);
        for r in rows {
            let row = r.id.to_string();
            match cipher.rewrap(slot(&row), &r.secret) {
                Ok(Some(new)) => {
                    done += sqlx::query!(
                        "UPDATE connector_credentials SET secret = $3 WHERE id = $1 AND secret = $2",
                        r.id,
                        r.secret,
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
            rewrapped: done,
            locked,
            last,
        })
    }
}

/// The encrypted SMTP password (purpose `smtp_password`, one row) for the rotation job and
/// the security page.
pub struct PgMailSecret {
    pool: PgPool,
}

impl PgMailSecret {
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SecretRows for PgMailSecret {
    fn purpose(&self) -> &'static str {
        MAIL_PURPOSE
    }

    async fn seal_plaintext(&self, _cipher: &dyn SecretCipher) -> RepoResult<u64> {
        Ok(0)
    }

    async fn version_counts(&self) -> RepoResult<Vec<(u16, u64)>> {
        crate::pg::version_counts(&self.pool, "instance_settings", "mail_password").await
    }

    async fn rewrap_batch(
        &self,
        cipher: &dyn SecretCipher,
        _active: u16,
        after: Option<Uuid>,
        _limit: u32,
    ) -> RepoResult<RewrapBatch> {
        // one row: the first call handles it, the paging call after it ends the loop
        if after.is_some() {
            return Ok(RewrapBatch::default());
        }
        let Some(blob) =
            sqlx::query_scalar!("SELECT mail_password FROM instance_settings WHERE id")
                .fetch_optional(&self.pool)
                .await
                .map_err(re)?
                .flatten()
        else {
            return Ok(RewrapBatch::default());
        };
        let (mut rewrapped, mut locked) = (0, 0);
        match cipher.rewrap(mail_slot(), &blob) {
            Ok(Some(new)) => {
                rewrapped = sqlx::query!(
                    "UPDATE instance_settings SET mail_password = $2
                     WHERE id AND mail_password = $1",
                    blob,
                    new
                )
                .execute(&self.pool)
                .await
                .map_err(re)?
                .rows_affected();
            }
            Ok(None) => {}
            Err(_) => locked = 1,
        }
        Ok(RewrapBatch {
            rewrapped,
            locked,
            // a sentinel id so the caller asks once more and stops
            last: Some(Uuid::nil()),
        })
    }
}
