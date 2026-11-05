use std::collections::HashSet;

use domain::Timestamp;
use domain::ids::{CompanionId, ItemId, SessionId};
use domain::kind::ProgressModel;
use domain::progress::Progress;
use domain::session::{CoopOutcome, Participant, Session};
use domain::user::Actor;

use crate::error::OptionExt;
use crate::ports::AppEvent;
use crate::{App, AppError, AppResult};

/// Input for [`App::log_session`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewSession {
    pub item_id: ItemId,
    pub started_at: Option<Timestamp>,
    pub duration_minutes: Option<u32>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub participants: Vec<Participant>,
    pub coop_outcome: Option<CoopOutcome>,
    pub incomplete: bool,
    pub client_op_id: Option<uuid::Uuid>,
}

impl NewSession {
    #[must_use]
    pub fn new(item_id: ItemId, participants: Vec<Participant>) -> Self {
        Self {
            item_id,
            started_at: None,
            duration_minutes: None,
            location: None,
            notes: None,
            participants,
            coop_outcome: None,
            incomplete: false,
            client_op_id: None,
        }
    }
}

impl App {
    /// Log a session (play, gaming session, "watched with"). Participants must be active
    /// companions of the actor. For kinds with the `Sessions` progress model a progress event
    /// is appended as well.
    ///
    /// # Errors
    /// Unknown item, foreign or merged companions, duplicate participants.
    pub async fn log_session(&self, actor: Actor, new: NewSession) -> AppResult<Session> {
        let item = self.catalog_item(new.item_id).await?;
        let kind = self.kind(&item.kind)?;
        self.check_participants(actor, &new.participants).await?;
        let session = Session {
            id: SessionId(self.uuid()),
            owner: actor.user_id,
            item_id: new.item_id,
            started_at: new.started_at.unwrap_or_else(|| self.now()),
            duration_minutes: new.duration_minutes,
            location: new.location,
            notes: new.notes,
            participants: new.participants,
            coop_outcome: new.coop_outcome,
            incomplete: new.incomplete,
        };
        session.validate()?;
        self.repos.sessions.insert(&session).await?;
        if kind.progress_model == ProgressModel::Sessions {
            self.log_progress(
                actor,
                session.item_id,
                Progress::Session {
                    session_id: session.id,
                },
                Some(session.started_at),
                new.client_op_id,
            )
            .await?;
        } else {
            self.add_to_library(actor, session.item_id, None).await?;
        }
        self.publish_session(&session).await?;
        Ok(session)
    }

    async fn check_participants(&self, actor: Actor, ps: &[Participant]) -> AppResult<()> {
        for p in ps {
            let c = self
                .repos
                .companions
                .get(p.companion_id)
                .await?
                .found("companion")?;
            if c.owner != actor.user_id {
                return Err(AppError::Forbidden);
            }
            c.ensure_active()?;
        }
        Ok(())
    }

    /// Read a session owned by the actor or shared with them as a linked participant.
    ///
    /// # Errors
    /// Unknown or not visible.
    pub async fn session(&self, actor: Actor, id: SessionId) -> AppResult<Session> {
        let s = self.repos.sessions.get(id).await?.found("session")?;
        if s.owner == actor.user_id {
            return Ok(s);
        }
        let mine = self.companion_ids_representing(actor).await?;
        if s.participants
            .iter()
            .any(|p| mine.contains(&p.companion_id))
        {
            Ok(s)
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// # Errors
    /// Not the owner.
    pub async fn delete_session(&self, actor: Actor, id: SessionId) -> AppResult<()> {
        let s = self.repos.sessions.get(id).await?.found("session")?;
        if s.owner != actor.user_id {
            return Err(AppError::Forbidden);
        }
        self.repos.sessions.delete(id).await?;
        self.publish_session(&s).await
    }

    /// Sessions of other users in which the actor participated through a linked companion
    /// (read-only, D-9).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn shared_sessions(&self, actor: Actor) -> AppResult<Vec<Session>> {
        let ids: Vec<CompanionId> = self
            .companion_ids_representing(actor)
            .await?
            .into_iter()
            .collect();
        let mut sessions = self.repos.sessions.with_participants(&ids).await?;
        sessions.retain(|s| s.owner != actor.user_id);
        sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at));
        Ok(sessions)
    }

    /// Companions of other users that stand for the actor: linked to them, plus companions
    /// merged into those.
    async fn companion_ids_representing(&self, actor: Actor) -> AppResult<HashSet<CompanionId>> {
        let linked = self.repos.companions.linked_to(actor.user_id).await?;
        let mut ids: HashSet<CompanionId> = HashSet::new();
        for c in linked.into_iter().filter(|c| c.owner != actor.user_id) {
            ids.insert(c.id);
            let owned = self.repos.companions.by_owner(c.owner).await?;
            let mut grew = true;
            while grew {
                grew = false;
                for o in &owned {
                    if let Some(t) = o.merged_into
                        && ids.contains(&t)
                        && ids.insert(o.id)
                    {
                        grew = true;
                    }
                }
            }
        }
        Ok(ids)
    }

    async fn publish_session(&self, s: &Session) -> AppResult<()> {
        let mut audience = vec![s.owner];
        for p in &s.participants {
            if let Some(c) = self.repos.companions.get(p.companion_id).await?
                && let Some(u) = c.linked_user
                && !audience.contains(&u)
            {
                audience.push(u);
            }
        }
        self.publish(AppEvent::entity_updated("session", s.id.0, audience))
            .await;
        Ok(())
    }
}
