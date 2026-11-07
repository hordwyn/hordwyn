//! Users, companions, link requests, invites.

use app::ports::{CompanionRepo, InviteRepo, LinkRequestRepo, UserRepo};
use async_trait::async_trait;
use domain::companion::{Companion, Invite, LinkRequest, UserSnapshot};
use domain::ids::{CompanionId, InviteId, LinkRequestId, UserId};
use domain::user::User;
use sqlx::{Postgres, Transaction};

use crate::conv::{json, request_state, request_state_str, role, role_str, to_json};
use crate::tx::{Op, record};
use crate::{PgStore, Res, be};

struct UserRow {
    id: uuid::Uuid,
    username: String,
    display_name: String,
    email: Option<String>,
    role: String,
    self_companion: uuid::Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    share_sessions: bool,
    locale: Option<String>,
}

impl TryFrom<UserRow> for User {
    type Error = app::ports::RepoError;
    fn try_from(r: UserRow) -> Result<Self, Self::Error> {
        Ok(User {
            id: UserId(r.id),
            username: r.username,
            display_name: r.display_name,
            email: r.email,
            role: role(&r.role)?,
            self_companion: CompanionId(r.self_companion),
            share_sessions: r.share_sessions,
            locale: r.locale,
            created_at: r.created_at,
        })
    }
}

struct CompanionRow {
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
    name: String,
    avatar_asset: Option<String>,
    notes: Option<String>,
    linked_user: Option<uuid::Uuid>,
    snapshot: Option<serde_json::Value>,
    is_self: bool,
    merged_into: Option<uuid::Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<CompanionRow> for Companion {
    type Error = app::ports::RepoError;
    fn try_from(r: CompanionRow) -> Result<Self, Self::Error> {
        Ok(Companion {
            id: CompanionId(r.id),
            owner: UserId(r.owner_id),
            name: r.name,
            avatar_asset: r.avatar_asset,
            notes: r.notes,
            linked_user: r.linked_user.map(UserId),
            snapshot: r
                .snapshot
                .map(|s| json::<UserSnapshot>("snapshot", s))
                .transpose()?,
            is_self: r.is_self,
            merged_into: r.merged_into.map(CompanionId),
            created_at: r.created_at,
        })
    }
}

fn companion_audience(c: &Companion) -> Vec<UserId> {
    let mut v = vec![c.owner];
    v.extend(c.linked_user.filter(|u| *u != c.owner));
    v
}

async fn upsert_companion(tx: &mut Transaction<'static, Postgres>, c: &Companion) -> Res<()> {
    let snapshot = c.snapshot.as_ref().map(to_json).transpose()?;
    sqlx::query!(
        "INSERT INTO companions (id, owner_id, name, avatar_asset, notes, linked_user, snapshot,
                                 is_self, merged_into, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, avatar_asset = EXCLUDED.avatar_asset,
             notes = EXCLUDED.notes, linked_user = EXCLUDED.linked_user,
             snapshot = EXCLUDED.snapshot, merged_into = EXCLUDED.merged_into",
        c.id.0,
        c.owner.0,
        c.name,
        c.avatar_asset,
        c.notes,
        c.linked_user.map(|u| u.0),
        snapshot,
        c.is_self,
        c.merged_into.map(|m| m.0),
        c.created_at
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    record(tx, "companion", c.id.0, Op::Upsert, &companion_audience(c)).await
}

#[async_trait]
impl UserRepo for PgStore {
    async fn insert_with_self(&self, user: &User, self_companion: &Companion) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO users (id, username, display_name, email, role, self_companion, created_at,
                                share_sessions, locale)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
            user.id.0,
            user.username,
            user.display_name,
            user.email,
            role_str(user.role),
            user.self_companion.0,
            user.created_at,
            user.share_sessions,
            user.locale
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        record(&mut tx, "user", user.id.0, Op::Upsert, &[user.id]).await?;
        upsert_companion(&mut tx, self_companion).await?;
        tx.commit().await.map_err(be)
    }

    async fn get(&self, id: UserId) -> Res<Option<User>> {
        sqlx::query_as!(UserRow, "SELECT * FROM users WHERE id = $1", id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(be)?
            .map(User::try_from)
            .transpose()
    }

    async fn by_username(&self, username: &str) -> Res<Option<User>> {
        sqlx::query_as!(UserRow, "SELECT * FROM users WHERE username = $1", username)
            .fetch_optional(&self.pool)
            .await
            .map_err(be)?
            .map(User::try_from)
            .transpose()
    }

    async fn count(&self) -> Res<u64> {
        let n = sqlx::query_scalar!(r#"SELECT COUNT(*) AS "n!" FROM users"#)
            .fetch_one(&self.pool)
            .await
            .map_err(be)?;
        Ok(u64::try_from(n).unwrap_or(0))
    }

    async fn list(&self) -> Res<Vec<User>> {
        sqlx::query_as!(UserRow, "SELECT * FROM users ORDER BY created_at, id")
            .fetch_all(&self.pool)
            .await
            .map_err(be)?
            .into_iter()
            .map(User::try_from)
            .collect()
    }

    async fn update(&self, user: &User) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "UPDATE users SET username = $2, display_name = $3, email = $4, role = $5,
                              share_sessions = $6, locale = $7 WHERE id = $1",
            user.id.0,
            user.username,
            user.display_name,
            user.email,
            role_str(user.role),
            user.share_sessions,
            user.locale
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        record(&mut tx, "user", user.id.0, Op::Upsert, &[user.id]).await?;
        tx.commit().await.map_err(be)
    }

    async fn delete(&self, id: UserId) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!("DELETE FROM users WHERE id = $1", id.0)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        sqlx::query!("DELETE FROM user_changes WHERE user_id = $1", id.0)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        sqlx::query!("DELETE FROM user_change_seq WHERE user_id = $1", id.0)
            .execute(&mut *tx)
            .await
            .map_err(be)?;
        record(&mut tx, "user", id.0, Op::Delete, &[]).await?;
        tx.commit().await.map_err(be)
    }
}

#[async_trait]
impl CompanionRepo for PgStore {
    async fn insert(&self, c: &Companion) -> Res<()> {
        let mut tx = self.begin().await?;
        upsert_companion(&mut tx, c).await?;
        tx.commit().await.map_err(be)
    }

    async fn get(&self, id: CompanionId) -> Res<Option<Companion>> {
        sqlx::query_as!(CompanionRow, "SELECT * FROM companions WHERE id = $1", id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(be)?
            .map(Companion::try_from)
            .transpose()
    }

    async fn update(&self, c: &Companion) -> Res<()> {
        self.update_many(std::slice::from_ref(c)).await
    }

    async fn update_many(&self, cs: &[Companion]) -> Res<()> {
        let mut tx = self.begin().await?;
        for c in cs {
            upsert_companion(&mut tx, c).await?;
        }
        tx.commit().await.map_err(be)
    }

    async fn delete(&self, id: CompanionId) -> Res<()> {
        let mut tx = self.begin().await?;
        let owner = sqlx::query_scalar!(
            "DELETE FROM companions WHERE id = $1 RETURNING owner_id",
            id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(be)?;
        let users: Vec<UserId> = owner.map(UserId).into_iter().collect();
        record(&mut tx, "companion", id.0, Op::Delete, &users).await?;
        tx.commit().await.map_err(be)
    }

    async fn by_owner(&self, owner: UserId) -> Res<Vec<Companion>> {
        sqlx::query_as!(
            CompanionRow,
            "SELECT * FROM companions WHERE owner_id = $1 ORDER BY created_at, id",
            owner.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(Companion::try_from)
        .collect()
    }

    async fn linked_to(&self, user: UserId) -> Res<Vec<Companion>> {
        sqlx::query_as!(
            CompanionRow,
            "SELECT * FROM companions WHERE linked_user = $1 ORDER BY created_at, id",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(Companion::try_from)
        .collect()
    }
}

struct LinkRequestRow {
    id: uuid::Uuid,
    companion_id: uuid::Uuid,
    from_user: uuid::Uuid,
    to_user: uuid::Uuid,
    state: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<LinkRequestRow> for LinkRequest {
    type Error = app::ports::RepoError;
    fn try_from(r: LinkRequestRow) -> Result<Self, Self::Error> {
        Ok(LinkRequest {
            id: LinkRequestId(r.id),
            companion_id: CompanionId(r.companion_id),
            from: UserId(r.from_user),
            to: UserId(r.to_user),
            state: request_state(&r.state)?,
            created_at: r.created_at,
        })
    }
}

#[async_trait]
impl LinkRequestRepo for PgStore {
    async fn insert(&self, r: &LinkRequest) -> Res<()> {
        LinkRequestRepo::update(self, r).await
    }

    async fn get(&self, id: LinkRequestId) -> Res<Option<LinkRequest>> {
        sqlx::query_as!(
            LinkRequestRow,
            "SELECT * FROM link_requests WHERE id = $1",
            id.0
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(LinkRequest::try_from)
        .transpose()
    }

    async fn update(&self, r: &LinkRequest) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO link_requests (id, companion_id, from_user, to_user, state, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (id) DO UPDATE SET state = EXCLUDED.state",
            r.id.0,
            r.companion_id.0,
            r.from.0,
            r.to.0,
            request_state_str(r.state),
            r.created_at
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        record(&mut tx, "link_request", r.id.0, Op::Upsert, &[r.from, r.to]).await?;
        tx.commit().await.map_err(be)
    }

    async fn pending_for(&self, user: UserId) -> Res<Vec<LinkRequest>> {
        sqlx::query_as!(
            LinkRequestRow,
            "SELECT * FROM link_requests WHERE to_user = $1 AND state = 'pending' ORDER BY created_at",
            user.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        .into_iter()
        .map(LinkRequest::try_from)
        .collect()
    }
}

struct InviteRow {
    id: uuid::Uuid,
    companion_id: uuid::Uuid,
    invited_by: uuid::Uuid,
    email: Option<String>,
    token_hash: String,
    state: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl TryFrom<InviteRow> for Invite {
    type Error = app::ports::RepoError;
    fn try_from(r: InviteRow) -> Result<Self, Self::Error> {
        Ok(Invite {
            id: InviteId(r.id),
            companion_id: CompanionId(r.companion_id),
            invited_by: UserId(r.invited_by),
            email: r.email,
            token_hash: r.token_hash,
            state: request_state(&r.state)?,
            expires_at: r.expires_at,
            created_at: r.created_at,
        })
    }
}

#[async_trait]
impl InviteRepo for PgStore {
    async fn insert(&self, i: &Invite) -> Res<()> {
        InviteRepo::update(self, i).await
    }

    async fn get(&self, id: InviteId) -> Res<Option<Invite>> {
        sqlx::query_as!(InviteRow, "SELECT * FROM invites WHERE id = $1", id.0)
            .fetch_optional(&self.pool)
            .await
            .map_err(be)?
            .map(Invite::try_from)
            .transpose()
    }

    async fn by_token_hash(&self, hash: &str) -> Res<Option<Invite>> {
        sqlx::query_as!(
            InviteRow,
            "SELECT * FROM invites WHERE token_hash = $1",
            hash
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(be)?
        .map(Invite::try_from)
        .transpose()
    }

    async fn update(&self, i: &Invite) -> Res<()> {
        let mut tx = self.begin().await?;
        sqlx::query!(
            "INSERT INTO invites (id, companion_id, invited_by, email, token_hash, state, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (id) DO UPDATE SET state = EXCLUDED.state",
            i.id.0,
            i.companion_id.0,
            i.invited_by.0,
            i.email,
            i.token_hash,
            request_state_str(i.state),
            i.expires_at,
            i.created_at
        )
        .execute(&mut *tx)
        .await
        .map_err(be)?;
        record(&mut tx, "invite", i.id.0, Op::Upsert, &[i.invited_by]).await?;
        tx.commit().await.map_err(be)
    }
}
