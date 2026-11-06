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
            && self.shares_sessions(s.owner).await?
        {
            Ok(s)
        } else {
            Err(AppError::Forbidden)
        }
    }

    async fn shares_sessions(&self, owner: domain::ids::UserId) -> AppResult<bool> {
        Ok(self
            .repos
            .users
            .get(owner)
            .await?
            .is_some_and(|u| u.share_sessions))
    }

    /// "Copy to my log": a session shared with the actor becomes a session of their own, with
    /// their self-companion carrying the score and result of the participant that stood for
    /// them. The owner's notes, location and other participants stay with the owner. The copy
    /// has an id derived from the original, so claiming twice changes nothing.
    ///
    /// # Errors
    /// Not shared with the actor, or the actor's own session.
    pub async fn claim_session(&self, actor: Actor, id: SessionId) -> AppResult<Session> {
        let s = self.session(actor, id).await?;
        if s.owner == actor.user_id {
            return Err(AppError::Conflict("the session is already yours".into()));
        }
        let copy_id = SessionId(uuid::Uuid::new_v5(&s.id.0, actor.user_id.0.as_bytes()));
        if let Some(existing) = self.repos.sessions.get(copy_id).await? {
            return Ok(existing);
        }
        let me = self.user(actor.user_id).await?;
        let mine = self.companion_ids_representing(actor).await?;
        let mut participant = s
            .participants
            .iter()
            .find(|p| mine.contains(&p.companion_id))
            .cloned()
            .found("participant")?;
        participant.companion_id = me.self_companion;
        let copy = Session {
            id: copy_id,
            owner: actor.user_id,
            item_id: s.item_id,
            started_at: s.started_at,
            duration_minutes: s.duration_minutes,
            location: None,
            notes: None,
            participants: vec![participant],
            coop_outcome: s.coop_outcome,
            incomplete: s.incomplete,
        };
        copy.validate()?;
        self.repos.sessions.insert(&copy).await?;
        let kind = self.kind(&self.catalog_item(s.item_id).await?.kind)?;
        let progress = match kind.progress_model {
            ProgressModel::Sessions => Some(Progress::Session {
                session_id: copy.id,
            }),
            ProgressModel::Single => Some(Progress::Finished),
            _ => None,
        };
        if let Some(p) = progress {
            self.log_progress(actor, s.item_id, p, Some(s.started_at), Some(copy_id.0))
                .await?;
        } else {
            self.add_to_library(actor, s.item_id, None).await?;
        }
        self.publish_session(&copy).await?;
        Ok(copy)
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
        let mut hidden = HashSet::new();
        for owner in sessions.iter().map(|s| s.owner).collect::<HashSet<_>>() {
            if !self.shares_sessions(owner).await? {
                hidden.insert(owner);
            }
        }
        sessions.retain(|s| !hidden.contains(&s.owner));
        sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at));
        Ok(sessions)
    }

    /// Sessions the actor logged, newest first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn my_sessions(&self, actor: Actor) -> AppResult<Vec<Session>> {
        let mut sessions = self.repos.sessions.by_owner(actor.user_id).await?;
        sessions.sort_by_key(|s| std::cmp::Reverse(s.started_at));
        Ok(sessions)
    }

    /// Participant names of a session visible to the actor (the owner's companion names),
    /// each with whether it stands for the actor.
    ///
    /// # Errors
    /// Not visible to the actor.
    pub async fn participant_names(
        &self,
        actor: Actor,
        id: SessionId,
    ) -> AppResult<Vec<(String, bool)>> {
        let s = self.session(actor, id).await?;
        let mut mine = self.companion_ids_representing(actor).await?;
        mine.insert(self.user(actor.user_id).await?.self_companion);
        let mut out = Vec::with_capacity(s.participants.len());
        for p in &s.participants {
            let name = self
                .repos
                .companions
                .get(p.companion_id)
                .await?
                .map_or_else(String::new, |c| c.name);
            out.push((name, mine.contains(&p.companion_id)));
        }
        Ok(out)
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
