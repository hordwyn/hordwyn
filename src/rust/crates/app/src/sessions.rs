use std::collections::HashSet;

use domain::Timestamp;
use domain::ids::{CompanionId, ItemId, SessionId};
use domain::kind::ProgressModel;
use domain::progress::Progress;
use domain::session::{CoopOutcome, Participant, Session};
use domain::user::Actor;

use crate::error::OptionExt;
use crate::ports::{AppEvent, Audience, Notification, Subject};
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
    /// Logged as this game group of the actor (design.md 6.7).
    pub game_group: Option<domain::ids::GameGroupId>,
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
            game_group: None,
        }
    }
}

/// The id of the session that a view "watched with" people logs for `user` with the client op
/// id `op` of its progress event (OT-22): the session's own op id is derived from the event's,
/// and [`App::log_session`] derives the session id from that and the user. Undo finds the
/// session of a view with it, and `cli sessions orphaned` the ones an Undo left behind.
#[must_use]
pub fn view_session_id(user: domain::ids::UserId, op: uuid::Uuid) -> SessionId {
    session_id_for(user, view_session_op(op))
}

fn view_session_op(op: uuid::Uuid) -> uuid::Uuid {
    let h = blake3::hash(&[op.as_bytes().as_slice(), b"session"].concat());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&h.as_bytes()[..16]);
    uuid::Uuid::from_bytes(bytes)
}

fn session_id_for(user: domain::ids::UserId, op: uuid::Uuid) -> SessionId {
    SessionId(uuid::Uuid::new_v5(&op, user.0.as_bytes()))
}

impl App {
    /// Log a view of `item_id` "watched with" the companions `with`: a session with the
    /// actor and them, then the progress event. The session's id is derived from the event's
    /// client op id ([`view_session_id`]); without one the server picks the op id, so Undo of
    /// the event always finds the session (OT-22). A replay of a view undone since answers
    /// the undone event and stores no session.
    ///
    /// # Errors
    /// As [`App::check_progress`], [`App::log_session`] and [`App::log_progress`].
    pub async fn log_view_with(
        &self,
        actor: Actor,
        item_id: ItemId,
        progress: Progress,
        occurred_at: Option<Timestamp>,
        client_op_id: Option<uuid::Uuid>,
        with: Vec<CompanionId>,
    ) -> AppResult<domain::progress::ProgressEvent> {
        let op = client_op_id.unwrap_or_else(|| self.uuid());
        if client_op_id.is_some()
            && let Some(e) = self
                .repos
                .progress
                .for_item(actor.user_id, item_id)
                .await?
                .into_iter()
                .find(|e| e.client_op_id == Some(op))
            && self.is_undone(actor, &e).await?
        {
            return Ok(e);
        }
        // the session is logged first: not for a view the log would refuse (a date before
        // the start of a running rewatch run, DEC-591)
        self.check_progress(actor, item_id, &progress, occurred_at, Some(op))
            .await?;
        let me = self.user(actor.user_id).await?;
        let mut participants = vec![Participant::new(me.self_companion)];
        participants.extend(with.into_iter().map(Participant::new));
        let mut session = NewSession::new(item_id, participants);
        session.started_at = occurred_at;
        // idempotency key of the session, derived from the event's (replays are no-ops)
        session.client_op_id = Some(view_session_op(op));
        self.log_session(actor, session).await?;
        self.log_progress(actor, item_id, progress, occurred_at, Some(op))
            .await
    }

    /// The sessions of views "watched with" whose view was undone, which an Undo before
    /// 1.0.1 left behind (OT-22): per account, each session whose id is the
    /// [`view_session_id`] of an undone event of its owner for its item. With `apply` they
    /// are deleted (as Undo deletes them now). Plays are never among them (their Undo has
    /// always deleted the session).
    ///
    /// # Errors
    /// Store errors.
    pub async fn orphaned_view_sessions(&self, apply: bool) -> AppResult<Vec<Session>> {
        let mut found = Vec::new();
        for user in self.repos.users.list().await? {
            let events = self.repos.progress.by_user(user.id).await?;
            let undone: HashSet<domain::ids::EventId> = events
                .iter()
                .filter_map(|e| match e.progress {
                    Progress::Undo { event_id } => Some(event_id),
                    _ => None,
                })
                .collect();
            let gone: HashSet<(SessionId, ItemId)> = events
                .iter()
                .filter(|e| undone.contains(&e.id))
                .filter(|e| !matches!(e.progress, Progress::Session { .. }))
                .filter_map(|e| Some((view_session_id(user.id, e.client_op_id?), e.item_id)))
                .collect();
            if gone.is_empty() {
                continue;
            }
            for s in self.repos.sessions.by_owner(user.id).await? {
                if gone.contains(&(s.id, s.item_id)) {
                    found.push(s);
                }
            }
        }
        if apply {
            for s in &found {
                self.repos.sessions.delete(s.id).await?;
                self.publish_session(s).await?;
            }
        }
        Ok(found)
    }

    /// Log a session (play, gaming session, "watched with"). Participants must be active
    /// companions of the actor. For kinds with the `Sessions` progress model a progress event
    /// is appended as well.
    ///
    /// # Errors
    /// Unknown item, foreign or merged companions, duplicate participants.
    pub async fn log_session(&self, actor: Actor, new: NewSession) -> AppResult<Session> {
        Ok(self.log_session_event(actor, new).await?.0)
    }

    /// Log a play of a kind with the `Sessions` progress model and answer the session with
    /// its progress event (the item page offers Undo on it).
    ///
    /// # Errors
    /// As [`Self::log_session`]; a kind with another progress model is a model mismatch.
    pub async fn log_play(
        &self,
        actor: Actor,
        new: NewSession,
    ) -> AppResult<(Session, domain::progress::ProgressEvent)> {
        let item = self.catalog_item(new.item_id).await?;
        let model = self.kind(&item.kind)?.progress_model;
        if model != ProgressModel::Sessions {
            return Err(domain::DomainError::ProgressModelMismatch {
                model: model.as_str().into(),
                got: ProgressModel::Sessions.as_str().into(),
            }
            .into());
        }
        let (session, event) = self.log_session_event(actor, new).await?;
        let event = event.ok_or(AppError::NotFound("progress event"))?;
        Ok((session, event))
    }

    async fn log_session_event(
        &self,
        actor: Actor,
        new: NewSession,
    ) -> AppResult<(Session, Option<domain::progress::ProgressEvent>)> {
        let item = self.catalog_item(new.item_id).await?;
        let kind = self.kind(&item.kind)?;
        self.check_participants(actor, &new.participants).await?;
        if let Some(g) = new.game_group {
            self.own_game_group(actor, g).await?;
        }
        let op = new.client_op_id;
        // the client op id is the idempotency key of the whole command: a replay names the
        // same session (UX1-3); the user id keeps the ids of two users apart
        let id = op.map_or_else(
            || SessionId(self.uuid()),
            |o| session_id_for(actor.user_id, o),
        );
        let session = Session {
            id,
            owner: actor.user_id,
            item_id: new.item_id,
            started_at: new.started_at.unwrap_or_else(|| self.now()),
            duration_minutes: new.duration_minutes,
            location: new.location,
            notes: new.notes,
            participants: new.participants,
            coop_outcome: new.coop_outcome,
            incomplete: new.incomplete,
            game_group_id: new.game_group,
        };
        session.validate()?;
        let plays = kind.progress_model == ProgressModel::Sessions;
        let play_event = |s: &Session| {
            self.log_progress(
                actor,
                s.item_id,
                Progress::Session { session_id: s.id },
                Some(s.started_at),
                op,
            )
        };
        if op.is_some()
            && let Some(existing) = self.repos.sessions.get(id).await?
            && existing.owner == actor.user_id
        {
            let event = if plays {
                Some(play_event(&existing).await?)
            } else {
                None
            };
            return Ok((existing, event));
        }
        let event = if plays {
            // the event first: a replay of a play undone since (its session deleted) answers
            // the undone event and stores nothing
            let e = play_event(&session).await?;
            if op.is_some() && self.is_undone(actor, &e).await? {
                return Ok((session, Some(e)));
            }
            Some(e)
        } else {
            self.add_to_library(actor, session.item_id, None).await?;
            None
        };
        self.repos.sessions.insert(&session).await?;
        self.publish_session(&session).await?;
        self.announce_session(&session, &item).await?;
        Ok((session, event))
    }

    async fn is_undone(
        &self,
        actor: Actor,
        e: &domain::progress::ProgressEvent,
    ) -> AppResult<bool> {
        Ok(self
            .repos
            .progress
            .for_item(actor.user_id, e.item_id)
            .await?
            .iter()
            .any(|x| matches!(x.progress, Progress::Undo { event_id } if event_id == e.id)))
    }

    /// Tell linked participants that a session was shared with them (unless the owner
    /// keeps sessions private).
    async fn announce_session(
        &self,
        s: &Session,
        item: &domain::catalog::CatalogItem,
    ) -> AppResult<()> {
        let owner = self.user(s.owner).await?;
        if !owner.share_sessions {
            return Ok(());
        }
        let mut users = Vec::new();
        for p in &s.participants {
            if let Some(c) = self.repos.companions.get(p.companion_id).await?
                && let Some(u) = c.linked_user
                && u != s.owner
                && !users.contains(&u)
            {
                users.push(u);
            }
        }
        if users.is_empty() {
            return Ok(());
        }
        self.notify(
            Notification::new(
                "social.session_shared",
                Audience::Users(users),
                format!("session:{}", s.id.0),
            )
            .arg("owner", owner.display_name)
            .arg("title", item.title.clone())
            .subject(Subject {
                kind: Some(item.kind.to_string()),
                item: Some(item.id),
                ..Subject::default()
            })
            .link("/history"),
        )
        .await;
        Ok(())
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
            // the group is the owner's, not the copier's
            game_group_id: None,
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
    pub(crate) async fn companion_ids_representing(
        &self,
        actor: Actor,
    ) -> AppResult<HashSet<CompanionId>> {
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

    pub(crate) async fn publish_session(&self, s: &Session) -> AppResult<()> {
        let mut audience = vec![s.owner];
        // linked participants only hear about it if the owner shares sessions (G2-9)
        let shared = self.shares_sessions(s.owner).await?;
        for p in s.participants.iter().filter(|_| shared) {
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
