use std::collections::HashMap;

use app::ports::{RepoError, SessionRepo};
use async_trait::async_trait;
use domain::ids::{CompanionId, ItemId, SessionId, UserId};
use domain::session::{Participant, Session};
use sqlx::{Postgres, Transaction};

use crate::conv::{coop, coop_str, to_i32, to_u32};
use crate::tx::{Op, record};
use crate::{PgStore, Res, be};

struct SessionRow {
    id: uuid::Uuid,
    owner_id: uuid::Uuid,
    item_id: uuid::Uuid,
    started_at: chrono::DateTime<chrono::Utc>,
    duration_minutes: Option<i32>,
    location: Option<String>,
    notes: Option<String>,
    coop_outcome: Option<String>,
    incomplete: bool,
}

impl PgStore {
    async fn load_sessions(&self, rows: Vec<SessionRow>) -> Res<Vec<Session>> {
        let ids: Vec<uuid::Uuid> = rows.iter().map(|r| r.id).collect();
        let mut parts: HashMap<uuid::Uuid, Vec<Participant>> = HashMap::new();
        for r in sqlx::query!(
            "SELECT session_id, companion_id, score, winner, rank, team, faction, starting_player, first_play
             FROM session_participants WHERE session_id = ANY($1) ORDER BY session_id, position",
            &ids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?
        {
            parts.entry(r.session_id).or_default().push(Participant {
                companion_id: CompanionId(r.companion_id),
                score: r.score,
                winner: r.winner,
                rank: to_u32(r.rank)?,
                team: r.team,
                faction: r.faction,
                starting_player: r.starting_player,
                first_play: r.first_play,
            });
        }
        rows.into_iter()
            .map(|r| {
                Ok(Session {
                    id: SessionId(r.id),
                    owner: UserId(r.owner_id),
                    item_id: ItemId(r.item_id),
                    started_at: r.started_at,
                    duration_minutes: to_u32(r.duration_minutes)?,
                    location: r.location,
                    notes: r.notes,
                    participants: parts.remove(&r.id).unwrap_or_default(),
                    coop_outcome: r.coop_outcome.as_deref().map(coop).transpose()?,
                    incomplete: r.incomplete,
                })
            })
            .collect::<Result<_, RepoError>>()
    }

    async fn sessions_where_ids(&self, ids: &[uuid::Uuid]) -> Res<Vec<Session>> {
        let rows = sqlx::query_as!(
            SessionRow,
            "SELECT * FROM sessions WHERE id = ANY($1) ORDER BY started_at DESC, id",
            ids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        self.load_sessions(rows).await
    }
}

async fn write_session(tx: &mut Transaction<'static, Postgres>, s: &Session) -> Res<()> {
    sqlx::query!(
        "INSERT INTO sessions (id, owner_id, item_id, started_at, duration_minutes, location, notes,
                               coop_outcome, incomplete)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (id) DO UPDATE SET item_id = EXCLUDED.item_id, started_at = EXCLUDED.started_at,
             duration_minutes = EXCLUDED.duration_minutes, location = EXCLUDED.location,
             notes = EXCLUDED.notes, coop_outcome = EXCLUDED.coop_outcome,
             incomplete = EXCLUDED.incomplete",
        s.id.0,
        s.owner.0,
        s.item_id.0,
        s.started_at,
        to_i32(s.duration_minutes)?,
        s.location,
        s.notes,
        s.coop_outcome.map(coop_str),
        s.incomplete
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    sqlx::query!(
        "DELETE FROM session_participants WHERE session_id = $1",
        s.id.0
    )
    .execute(&mut **tx)
    .await
    .map_err(be)?;
    for (pos, p) in s.participants.iter().enumerate() {
        let pos =
            i32::try_from(pos).map_err(|_| RepoError::Backend("too many participants".into()))?;
        sqlx::query!(
            "INSERT INTO session_participants (session_id, companion_id, position, score, winner, rank,
                                               team, faction, starting_player, first_play)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            s.id.0,
            p.companion_id.0,
            pos,
            p.score,
            p.winner,
            to_i32(p.rank)?,
            p.team,
            p.faction,
            p.starting_player,
            p.first_play
        )
        .execute(&mut **tx)
        .await
        .map_err(be)?;
    }
    record(tx, "session", s.id.0, Op::Upsert, &[s.owner]).await
}

#[async_trait]
impl SessionRepo for PgStore {
    async fn insert(&self, s: &Session) -> Res<()> {
        SessionRepo::update(self, s).await
    }

    async fn get(&self, id: SessionId) -> Res<Option<Session>> {
        Ok(self.sessions_where_ids(&[id.0]).await?.pop())
    }

    async fn update(&self, s: &Session) -> Res<()> {
        let mut tx = self.begin().await?;
        write_session(&mut tx, s).await?;
        tx.commit().await.map_err(be)
    }

    async fn delete(&self, id: SessionId) -> Res<()> {
        let mut tx = self.begin().await?;
        let owner = sqlx::query_scalar!(
            "DELETE FROM sessions WHERE id = $1 RETURNING owner_id",
            id.0
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(be)?;
        let users: Vec<UserId> = owner.map(UserId).into_iter().collect();
        record(&mut tx, "session", id.0, Op::Delete, &users).await?;
        tx.commit().await.map_err(be)
    }

    async fn by_owner(&self, owner: UserId) -> Res<Vec<Session>> {
        let rows = sqlx::query_as!(
            SessionRow,
            "SELECT * FROM sessions WHERE owner_id = $1 ORDER BY started_at DESC, id",
            owner.0
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        self.load_sessions(rows).await
    }

    async fn with_participants(&self, companions: &[CompanionId]) -> Res<Vec<Session>> {
        let cids: Vec<uuid::Uuid> = companions.iter().map(|c| c.0).collect();
        let ids = sqlx::query_scalar!(
            "SELECT DISTINCT session_id FROM session_participants WHERE companion_id = ANY($1)",
            &cids
        )
        .fetch_all(&self.pool)
        .await
        .map_err(be)?;
        self.sessions_where_ids(&ids).await
    }
}
