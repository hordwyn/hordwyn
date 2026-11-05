use domain::companion::{Companion, UserSnapshot};
use domain::ids::{CompanionId, UserId};
use domain::user::{Actor, Role, User, validate_username};

use crate::error::OptionExt;
use crate::ports::{AppEvent, RepoError};
use crate::{App, AppError, AppResult};

impl App {
    /// Create a user and their self-companion. Credentials are handled by `auth`.
    ///
    /// # Errors
    /// Invalid or taken username.
    pub async fn register_user(
        &self,
        username: &str,
        display_name: &str,
        email: Option<String>,
        role: Role,
    ) -> AppResult<User> {
        validate_username(username)?;
        domain::companion::validate_companion_name(display_name)?;
        if self.repos.users.by_username(username).await?.is_some() {
            return Err(AppError::Conflict("username is taken".into()));
        }
        let now = self.now();
        let user_id = UserId(self.uuid());
        let self_companion = Companion {
            id: CompanionId(self.uuid()),
            owner: user_id,
            name: display_name.to_string(),
            avatar_asset: None,
            notes: None,
            linked_user: Some(user_id),
            snapshot: None,
            is_self: true,
            merged_into: None,
            created_at: now,
        };
        let user = User {
            id: user_id,
            username: username.to_string(),
            display_name: display_name.to_string(),
            email,
            role,
            self_companion: self_companion.id,
            created_at: now,
        };
        self.repos
            .users
            .insert_with_self(&user, &self_companion)
            .await
            .map_err(|e| match e {
                RepoError::Duplicate(m) => AppError::Conflict(m),
                e @ RepoError::Backend(_) => AppError::Repo(e),
            })?;
        Ok(user)
    }

    /// # Errors
    /// Unknown user.
    pub async fn user(&self, id: UserId) -> AppResult<User> {
        self.repos.users.get(id).await?.found("user")
    }

    /// Delete an account (admin or the user themself). Companions of other users that were
    /// linked to it are demoted: they keep a snapshot and all history (D-9).
    ///
    /// # Errors
    /// Forbidden or unknown user.
    pub async fn delete_user(&self, actor: Actor, id: UserId) -> AppResult<()> {
        if actor.user_id != id && !actor.is_admin() {
            return Err(AppError::Forbidden);
        }
        let user = self.user(id).await?;
        let snapshot = UserSnapshot {
            display_name: user.display_name.clone(),
            avatar_asset: None,
        };
        let mut demoted = Vec::new();
        for mut c in self.repos.companions.linked_to(id).await? {
            if c.owner == id {
                continue;
            }
            c.demote(snapshot.clone());
            demoted.push(c);
        }
        self.repos.companions.update_many(&demoted).await?;
        self.repos.users.delete(id).await?;
        for c in &demoted {
            self.publish(AppEvent::entity_updated("companion", c.id.0, vec![c.owner]))
                .await;
        }
        Ok(())
    }
}
