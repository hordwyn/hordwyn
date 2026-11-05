//! Transaction helper: outbox + per-user change sequence.

use domain::ids::UserId;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{PgStore, Res, be};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Upsert,
    Delete,
}

impl Op {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
        }
    }
}

/// One row of a user's change feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub seq: i64,
    pub entity: String,
    pub entity_id: Uuid,
    pub op: String,
}

impl PgStore {
    pub(crate) async fn begin(&self) -> Res<Transaction<'static, Postgres>> {
        self.pool.begin().await.map_err(be)
    }

    /// Changes of `user` after `cursor` (exclusive), oldest first.
    ///
    /// # Errors
    /// Database errors.
    pub async fn changes_since(&self, user: UserId, cursor: i64, limit: i64) -> Res<Vec<Change>> {
        sqlx::query_as!(
            Change,
            "SELECT seq, entity, entity_id, op FROM user_changes
             WHERE user_id = $1 AND seq > $2 ORDER BY seq LIMIT $3",
            user.0,
            cursor,
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)
    }
}

/// Record a change inside `tx`: one outbox row, then one change-feed row per affected user.
pub(crate) async fn record(
    tx: &mut Transaction<'static, Postgres>,
    entity: &str,
    id: Uuid,
    op: Op,
    users: &[UserId],
) -> Res<()> {
    sqlx::query!(
        "INSERT INTO outbox (entity, entity_id, op, user_id) VALUES ($1, $2, $3, $4)",
        entity,
        id,
        op.as_str(),
        users.first().map(|u| u.0)
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    for user in users {
        let seq = sqlx::query_scalar!(
            "INSERT INTO user_change_seq (user_id, last_seq) VALUES ($1, 1)
             ON CONFLICT (user_id) DO UPDATE SET last_seq = user_change_seq.last_seq + 1
             RETURNING last_seq",
            user.0
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(be)?;
        sqlx::query!(
            "INSERT INTO user_changes (user_id, seq, entity, entity_id, op) VALUES ($1, $2, $3, $4, $5)",
            user.0,
            seq,
            entity,
            id,
            op.as_str()
        )
        .execute(&mut **tx)
        .await
        .map_err(be)?;
    }
    Ok(())
}
