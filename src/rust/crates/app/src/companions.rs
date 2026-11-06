use domain::companion::{
    Companion, Invite, LinkRequest, RequestState, UserSnapshot, invite_expiry, merge,
    validate_companion_name,
};
use domain::ids::{CompanionId, InviteId, LinkRequestId, UserId};
use domain::user::{Actor, Role, User};

use crate::error::OptionExt;
use crate::ports::{AppEvent, Audience, Notification, Subject};
use crate::{App, AppError, AppResult};

impl App {
    /// # Errors
    /// Invalid name.
    pub async fn create_companion(&self, actor: Actor, name: &str) -> AppResult<Companion> {
        validate_companion_name(name)?;
        let c = Companion {
            id: CompanionId(self.uuid()),
            owner: actor.user_id,
            name: name.trim().to_string(),
            avatar_asset: None,
            notes: None,
            linked_user: None,
            snapshot: None,
            is_self: false,
            merged_into: None,
            created_at: self.now(),
        };
        self.repos.companions.insert(&c).await?;
        self.publish_companion(&c).await;
        Ok(c)
    }

    /// A companion owned by the actor (merged ones included).
    ///
    /// # Errors
    /// Unknown or not owned.
    pub async fn companion(&self, actor: Actor, id: CompanionId) -> AppResult<Companion> {
        let c = self.repos.companions.get(id).await?.found("companion")?;
        if c.owner == actor.user_id {
            Ok(c)
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Active companions of the actor, self first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn companions(&self, actor: Actor) -> AppResult<Vec<Companion>> {
        let mut cs = self.repos.companions.by_owner(actor.user_id).await?;
        cs.retain(Companion::is_active);
        cs.sort_by(|a, b| b.is_self.cmp(&a.is_self).then(a.name.cmp(&b.name)));
        Ok(cs)
    }

    /// Follow `merged_into` to the companion that now stands for `id`.
    ///
    /// # Errors
    /// Unknown or not owned.
    pub async fn resolve_companion(&self, actor: Actor, id: CompanionId) -> AppResult<Companion> {
        let mut c = self.companion(actor, id).await?;
        // merge chains are short; the bound protects against corrupt cycles
        for _ in 0..32 {
            match c.merged_into {
                Some(next) => c = self.companion(actor, next).await?,
                None => return Ok(c),
            }
        }
        Err(AppError::Conflict("companion merge chain too long".into()))
    }

    /// # Errors
    /// Not owned, merged, or invalid name.
    pub async fn update_companion(
        &self,
        actor: Actor,
        id: CompanionId,
        name: Option<String>,
        notes: Option<Option<String>>,
    ) -> AppResult<Companion> {
        let mut c = self.companion(actor, id).await?;
        c.ensure_active()?;
        if let Some(n) = name {
            validate_companion_name(&n)?;
            c.name = n.trim().to_string();
        }
        if let Some(n) = notes {
            c.notes = n;
        }
        self.repos.companions.update(&c).await?;
        self.publish_companion(&c).await;
        Ok(c)
    }

    /// Delete an unused companion. Companions referenced by sessions can only be merged.
    ///
    /// # Errors
    /// Self-companion, referenced, or not owned.
    pub async fn delete_companion(&self, actor: Actor, id: CompanionId) -> AppResult<()> {
        let c = self.companion(actor, id).await?;
        if c.is_self {
            return Err(AppError::Conflict(
                "the self-companion cannot be deleted".into(),
            ));
        }
        let mut referenced = vec![id];
        referenced.extend(
            self.repos
                .companions
                .by_owner(actor.user_id)
                .await?
                .into_iter()
                .filter(|o| o.merged_into == Some(id))
                .map(|o| o.id),
        );
        if !self
            .repos
            .sessions
            .with_participants(&referenced)
            .await?
            .is_empty()
        {
            return Err(AppError::Conflict(
                "companion is part of logged sessions; merge it instead".into(),
            ));
        }
        self.repos.companions.delete(id).await?;
        self.publish_companion(&c).await;
        Ok(())
    }

    /// Ask `target` to accept that this companion stands for them.
    ///
    /// # Errors
    /// Not owned, self/merged/already linked companion, unknown target, or the owner already
    /// has a companion linked to the target.
    pub async fn request_link(
        &self,
        actor: Actor,
        id: CompanionId,
        target: UserId,
    ) -> AppResult<LinkRequest> {
        let c = self.companion(actor, id).await?;
        c.ensure_active()?;
        if c.is_self || c.linked_user.is_some() {
            return Err(AppError::Conflict("companion is already linked".into()));
        }
        if target == actor.user_id {
            return Err(AppError::Conflict("use the self-companion instead".into()));
        }
        self.user(target).await?;
        let owned = self.repos.companions.by_owner(actor.user_id).await?;
        if owned.iter().any(|o| o.linked_user == Some(target)) {
            return Err(AppError::Conflict(
                "another companion is already linked to this user".into(),
            ));
        }
        let req = LinkRequest {
            id: LinkRequestId(self.uuid()),
            companion_id: id,
            from: actor.user_id,
            to: target,
            state: RequestState::Pending,
            created_at: self.now(),
        };
        self.repos.link_requests.insert(&req).await?;
        self.publish(AppEvent::entity_updated(
            "link_request",
            req.id.0,
            vec![req.from, req.to],
        ))
        .await;
        let from = self.user(actor.user_id).await?;
        self.notify(
            Notification::new(
                "social.link_request",
                Audience::Users(vec![target]),
                format!("link-request:{}", req.id.0),
            )
            .arg("from", from.display_name)
            .arg("companion", c.name.clone())
            .subject(Subject {
                companion: Some(c.id),
                ..Subject::default()
            })
            .link("/people"),
        )
        .await;
        Ok(req)
    }

    /// Pending link requests addressed to the actor.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn link_requests(&self, actor: Actor) -> AppResult<Vec<LinkRequest>> {
        Ok(self.repos.link_requests.pending_for(actor.user_id).await?)
    }

    async fn pending_request_for(&self, actor: Actor, id: LinkRequestId) -> AppResult<LinkRequest> {
        let req = self
            .repos
            .link_requests
            .get(id)
            .await?
            .found("link request")?;
        if req.to != actor.user_id {
            return Err(AppError::Forbidden);
        }
        if req.state != RequestState::Pending {
            return Err(AppError::Conflict("request is no longer pending".into()));
        }
        Ok(req)
    }

    /// # Errors
    /// Not addressed to the actor, not pending, or the companion changed meanwhile.
    pub async fn accept_link(&self, actor: Actor, id: LinkRequestId) -> AppResult<Companion> {
        let mut req = self.pending_request_for(actor, id).await?;
        let mut c = self
            .repos
            .companions
            .get(req.companion_id)
            .await?
            .found("companion")?;
        c.ensure_active()?;
        if c.linked_user.is_some() {
            return Err(AppError::Conflict("companion is already linked".into()));
        }
        c.linked_user = Some(actor.user_id);
        c.snapshot = None;
        self.repos.companions.update(&c).await?;
        req.state = RequestState::Accepted;
        self.repos.link_requests.update(&req).await?;
        self.publish_companion(&c).await;
        let me = self.user(actor.user_id).await?;
        self.notify(
            Notification::new(
                "social.link_accepted",
                Audience::Users(vec![req.from]),
                format!("link-accepted:{}", req.id.0),
            )
            .arg("user", me.display_name)
            .arg("companion", c.name.clone())
            .subject(Subject {
                companion: Some(c.id),
                ..Subject::default()
            })
            .link("/people"),
        )
        .await;
        Ok(c)
    }

    /// # Errors
    /// Not addressed to the actor or not pending.
    pub async fn decline_link(&self, actor: Actor, id: LinkRequestId) -> AppResult<()> {
        let mut req = self.pending_request_for(actor, id).await?;
        req.state = RequestState::Declined;
        self.repos.link_requests.update(&req).await?;
        Ok(())
    }

    /// End a link (demotion). Allowed for the owner and for the linked user. The companion
    /// keeps a snapshot of the user's name; history is untouched.
    ///
    /// # Errors
    /// Neither owner nor linked user, or the self-companion.
    pub async fn unlink_companion(&self, actor: Actor, id: CompanionId) -> AppResult<Companion> {
        let mut c = self.repos.companions.get(id).await?.found("companion")?;
        let Some(linked) = c.linked_user else {
            return Ok(c);
        };
        if c.owner != actor.user_id && linked != actor.user_id {
            return Err(AppError::Forbidden);
        }
        if c.is_self {
            return Err(AppError::Conflict("the self-companion stays linked".into()));
        }
        let user = self.user(linked).await?;
        c.demote(UserSnapshot {
            display_name: user.display_name,
            avatar_asset: None,
        });
        self.repos.companions.update(&c).await?;
        self.publish_companion(&c).await;
        Ok(c)
    }

    /// Merge the duplicate `remove` into `keep`.
    ///
    /// # Errors
    /// Not owned or a merge rule is violated (see [`domain::companion::merge`]).
    pub async fn merge_companions(
        &self,
        actor: Actor,
        keep: CompanionId,
        remove: CompanionId,
    ) -> AppResult<Companion> {
        let mut k = self.companion(actor, keep).await?;
        let mut r = self.companion(actor, remove).await?;
        merge(&mut k, &mut r)?;
        self.repos
            .companions
            .update_many(&[k.clone(), r.clone()])
            .await?;
        self.publish_companion(&k).await;
        self.publish_companion(&r).await;
        Ok(k)
    }

    /// Promote a companion to a user: create a single-use invite. Returns the invite and the
    /// plain token (only its hash is stored).
    ///
    /// # Errors
    /// Not the owner (or admin), or a self/merged/linked companion.
    pub async fn promote_companion(
        &self,
        actor: Actor,
        id: CompanionId,
        email: Option<String>,
    ) -> AppResult<(Invite, String)> {
        let c = self.repos.companions.get(id).await?.found("companion")?;
        if c.owner != actor.user_id && !actor.is_admin() {
            return Err(AppError::Forbidden);
        }
        c.ensure_active()?;
        if c.is_self || c.linked_user.is_some() {
            return Err(AppError::Conflict("companion is already linked".into()));
        }
        let token = self.ids.new_token();
        let now = self.now();
        let invite = Invite {
            id: InviteId(self.uuid()),
            companion_id: id,
            invited_by: actor.user_id,
            email,
            token_hash: self.ids.hash_token(&token),
            state: RequestState::Pending,
            expires_at: invite_expiry(now),
            created_at: now,
        };
        self.repos.invites.insert(&invite).await?;
        Ok((invite, token))
    }

    /// Accept an invite: create the account and link the companion. The new user immediately
    /// sees shared sessions they took part in.
    ///
    /// # Errors
    /// Unknown, used or expired token; invalid or taken username.
    pub async fn accept_invite(
        &self,
        token: &str,
        username: &str,
        display_name: &str,
    ) -> AppResult<User> {
        let mut invite = self
            .repos
            .invites
            .by_token_hash(&self.ids.hash_token(token))
            .await?
            .found("invite")?;
        if invite.state != RequestState::Pending || invite.expires_at <= self.now() {
            return Err(AppError::Conflict("invite is no longer valid".into()));
        }
        let mut c = self
            .repos
            .companions
            .get(invite.companion_id)
            .await?
            .found("companion")?;
        c.ensure_active()?;
        if c.linked_user.is_some() {
            return Err(AppError::Conflict("companion is already linked".into()));
        }
        let user = self
            .register_user(username, display_name, invite.email.clone(), Role::Member)
            .await?;
        c.linked_user = Some(user.id);
        self.repos.companions.update(&c).await?;
        invite.state = RequestState::Accepted;
        self.repos.invites.update(&invite).await?;
        self.publish_companion(&c).await;
        Ok(user)
    }

    /// Would [`App::accept_invite`] accept this token now (pending, not expired, companion
    /// active and not linked)? Lets sign-up refuse a bad invite before anything else.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn invite_is_valid(&self, token: &str) -> AppResult<bool> {
        let Some(invite) = self
            .repos
            .invites
            .by_token_hash(&self.ids.hash_token(token))
            .await?
        else {
            return Ok(false);
        };
        if invite.state != RequestState::Pending || invite.expires_at <= self.now() {
            return Ok(false);
        }
        let c = self.repos.companions.get(invite.companion_id).await?;
        Ok(c.is_some_and(|c| c.is_active() && c.linked_user.is_none()))
    }

    async fn publish_companion(&self, c: &Companion) {
        let mut audience = vec![c.owner];
        audience.extend(c.linked_user.filter(|u| *u != c.owner));
        self.publish(AppEvent::entity_updated("companion", c.id.0, audience))
            .await;
    }
}
