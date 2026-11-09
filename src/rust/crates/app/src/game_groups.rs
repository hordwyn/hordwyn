//! Game groups (design.md 6.7): named sets of the owner's people who play together. A play
//! logged as a group carries its id; the People tab counts those plays. Members are read
//! through merges (a member merged into another person shows as that person, and again as
//! themselves after the merge is undone); removed people leave the list; a removed group is
//! hidden and keeps its sessions.

use std::collections::HashMap;

use domain::DomainError;
use domain::game_group::GameGroup;
use domain::ids::{CompanionId, GameGroupId};
use domain::user::Actor;

use crate::error::OptionExt;
use crate::people::{StatItem, roots};
use crate::ports::AppEvent;
use crate::{App, AppError, AppResult};

/// Input for [`App::create_game_group`] and [`App::update_game_group`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameGroupInput {
    pub name: String,
    pub members: Vec<CompanionId>,
}

/// A group as the People tab shows it: members as they stand now, plays logged as the
/// group, the latest of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameGroupView {
    pub group: GameGroup,
    pub plays: u32,
    pub last: Option<StatItem>,
}

impl App {
    /// The actor's active game groups by name, members resolved through merges.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn game_groups(&self, actor: Actor) -> AppResult<Vec<GameGroupView>> {
        let mut groups: Vec<GameGroup> = self
            .repos
            .companions
            .game_groups(actor.user_id)
            .await?
            .into_iter()
            .filter(GameGroup::is_active)
            .collect();
        if groups.is_empty() {
            return Ok(Vec::new());
        }
        let owned = self.repos.companions.by_owner(actor.user_id).await?;
        let root = roots(&owned);
        for g in &mut groups {
            let mut members = Vec::with_capacity(g.members.len());
            for m in &g.members {
                if let Some(r) = root.get(m)
                    && !members.contains(r)
                {
                    members.push(*r);
                }
            }
            g.members = members;
        }
        // one grouped read, not every session of the owner (UX4-17)
        let per: HashMap<GameGroupId, (u32, Option<(domain::Timestamp, domain::ids::ItemId)>)> =
            self.repos
                .sessions
                .group_plays(actor.user_id)
                .await?
                .into_iter()
                .map(|g| (g.group, (g.plays, Some((g.last_at, g.last_item)))))
                .collect();
        let wanted: Vec<_> = per.values().filter_map(|(_, l)| l.map(|x| x.1)).collect();
        let titles: HashMap<_, _> = self
            .repos
            .catalog
            .get_many(&wanted)
            .await?
            .into_iter()
            .map(|i| (i.id, i.title))
            .collect();
        groups.sort_by_key(|g| g.name.to_lowercase());
        Ok(groups
            .into_iter()
            .map(|group| {
                let (plays, last) = per.get(&group.id).copied().unwrap_or_default();
                GameGroupView {
                    last: last.map(|(at, item_id)| StatItem {
                        item_id,
                        title: titles.get(&item_id).cloned().unwrap_or_default(),
                        at,
                    }),
                    plays,
                    group,
                }
            })
            .collect())
    }

    /// The actor's active group `id` (another user's group is not found).
    pub(crate) async fn own_game_group(
        &self,
        actor: Actor,
        id: GameGroupId,
    ) -> AppResult<GameGroup> {
        let g = self
            .repos
            .companions
            .game_group(id)
            .await?
            .found("game group")?;
        if g.owner != actor.user_id {
            return Err(AppError::NotFound("game group"));
        }
        if !g.is_active() {
            return Err(AppError::Conflict("the game group was removed".into()));
        }
        Ok(g)
    }

    /// Members must be active people of the actor, never their own entry.
    async fn check_members(&self, actor: Actor, members: &[CompanionId]) -> AppResult<()> {
        let found = self.repos.companions.get_many(members).await?;
        for m in members {
            let c = found.iter().find(|c| c.id == *m).found("companion")?;
            // someone else's person reads like an unknown id (UX4-17)
            if c.owner != actor.user_id {
                return Err(AppError::NotFound("companion"));
            }
            if c.is_self {
                return Err(
                    DomainError::invalid("members", "you are in every play already").into(),
                );
            }
            c.ensure_active()?;
        }
        Ok(())
    }

    async fn store_game_group(&self, actor: Actor, g: &GameGroup) -> AppResult<()> {
        g.validate()?;
        self.check_members(actor, &g.members).await?;
        self.repos.companions.upsert_game_group(g).await?;
        self.publish(AppEvent::entity_updated(
            "game_group",
            g.id.0,
            vec![actor.user_id],
        ))
        .await;
        Ok(())
    }

    /// # Errors
    /// Invalid name or members; members that are not the actor's active people.
    pub async fn create_game_group(
        &self,
        actor: Actor,
        input: GameGroupInput,
    ) -> AppResult<GameGroup> {
        let g = GameGroup {
            id: GameGroupId(self.uuid()),
            owner: actor.user_id,
            name: input.name.trim().to_string(),
            members: input.members,
            removed_at: None,
            created_at: self.now(),
        };
        self.store_game_group(actor, &g).await?;
        Ok(g)
    }

    /// Rename a group and replace its members.
    ///
    /// # Errors
    /// Not the actor's active group; as [`Self::create_game_group`].
    pub async fn update_game_group(
        &self,
        actor: Actor,
        id: GameGroupId,
        input: GameGroupInput,
    ) -> AppResult<GameGroup> {
        let mut g = self.own_game_group(actor, id).await?;
        g.name = input.name.trim().to_string();
        g.members = input.members;
        self.store_game_group(actor, &g).await?;
        Ok(g)
    }

    /// Remove a group: hidden from People and the play sheet; its plays keep it.
    ///
    /// # Errors
    /// Not the actor's active group.
    pub async fn remove_game_group(&self, actor: Actor, id: GameGroupId) -> AppResult<()> {
        let mut g = self.own_game_group(actor, id).await?;
        g.removed_at = Some(self.now());
        self.repos.companions.upsert_game_group(&g).await?;
        self.publish(AppEvent::entity_updated(
            "game_group",
            g.id.0,
            vec![actor.user_id],
        ))
        .await;
        Ok(())
    }
}
