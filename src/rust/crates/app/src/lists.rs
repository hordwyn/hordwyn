use std::collections::HashSet;

use domain::ids::{ItemId, ListId, UserId};
use domain::list::{List, ListItem, Visibility, VisibilityChoice, validate_list_name};
use domain::user::Actor;

use crate::error::OptionExt;
use crate::ports::AppEvent;
use crate::{App, AppError, AppResult};

impl App {
    /// # Errors
    /// Invalid name.
    pub async fn create_list(
        &self,
        actor: Actor,
        name: &str,
        description: Option<String>,
        visibility: Visibility,
    ) -> AppResult<List> {
        validate_list_name(name)?;
        let now = self.now();
        let list = List {
            id: ListId(self.uuid()),
            owner: actor.user_id,
            name: name.trim().to_string(),
            description,
            visibility,
            items: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        self.repos.lists.insert(&list).await?;
        self.publish_list(&list).await;
        Ok(list)
    }

    /// Read a list the actor may see (owner, or visibility `instance`). Link-shared lists are
    /// read through the share token, not here.
    ///
    /// # Errors
    /// Unknown list or not visible to the actor.
    pub async fn list(&self, actor: Actor, id: ListId) -> AppResult<List> {
        let list = self.repos.lists.get(id).await?.found("list")?;
        if list.owner == actor.user_id || list.visibility == Visibility::Instance {
            Ok(list)
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// Share a list by link: a new random token replaces any earlier one, so sharing again
    /// revokes the old link. Returns the token.
    ///
    /// # Errors
    /// Unknown list or not the owner.
    pub async fn share_list(&self, actor: Actor, id: ListId) -> AppResult<String> {
        let mut list = self.owned_list(actor, id).await?;
        let token = self.ids.new_token();
        list.visibility = Visibility::Link {
            token: token.clone(),
        };
        self.save_list(list).await?;
        Ok(token)
    }

    /// Read a link-shared list without an account.
    ///
    /// # Errors
    /// Unknown or revoked token.
    pub async fn list_by_token(&self, token: &str) -> AppResult<List> {
        self.repos.lists.by_share_token(token).await?.found("list")
    }

    /// # Errors
    /// Storage errors.
    pub async fn lists(&self, actor: Actor) -> AppResult<Vec<List>> {
        Ok(self.repos.lists.by_owner(actor.user_id).await?)
    }

    /// The share dialog (design.md 5.10): private, everyone on this instance, or anyone with
    /// the link. Choosing the link again keeps the link people have unless `new_link` asks
    /// for a new one, which stops the old one; any other choice revokes the link (DEC-131).
    ///
    /// # Errors
    /// Unknown list or not the owner.
    pub async fn set_list_visibility(
        &self,
        actor: Actor,
        id: ListId,
        choice: VisibilityChoice,
        new_link: bool,
    ) -> AppResult<List> {
        let mut list = self.owned_list(actor, id).await?;
        list.visibility = match (choice, &list.visibility) {
            (VisibilityChoice::Private, _) => Visibility::Private,
            (VisibilityChoice::Instance, _) => Visibility::Instance,
            (VisibilityChoice::Link, Visibility::Link { token }) if !new_link => Visibility::Link {
                token: token.clone(),
            },
            (VisibilityChoice::Link, _) => Visibility::Link {
                token: self.ids.new_token(),
            },
        };
        self.save_list(list).await
    }

    /// People linked with the actor, either way: the accounts the actor's companions stand
    /// for, and the owners of companions that stand for the actor.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn linked_people(&self, actor: Actor) -> AppResult<HashSet<UserId>> {
        let mut out: HashSet<UserId> = self
            .repos
            .companions
            .by_owner(actor.user_id)
            .await?
            .into_iter()
            .filter(|c| c.is_active() && !c.is_self)
            .filter_map(|c| c.linked_user)
            .collect();
        out.extend(
            self.repos
                .companions
                .linked_to(actor.user_id)
                .await?
                .into_iter()
                .filter(|c| c.is_active() && c.owner != actor.user_id)
                .map(|c| c.owner),
        );
        out.remove(&actor.user_id);
        Ok(out)
    }

    /// "On this instance": every list of others with visibility `instance`, newest first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn lists_on_instance(&self, actor: Actor) -> AppResult<Vec<List>> {
        let mut lists = self.repos.lists.instance_visible().await?;
        lists.retain(|l| l.owner != actor.user_id);
        lists.sort_by_key(|l| (std::cmp::Reverse(l.updated_at), l.id.0));
        Ok(lists)
    }

    /// "Shared with me": the instance-visible lists of linked people.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn lists_shared_with_me(&self, actor: Actor) -> AppResult<Vec<List>> {
        let linked = self.linked_people(actor).await?;
        let mut lists = self.lists_on_instance(actor).await?;
        lists.retain(|l| linked.contains(&l.owner));
        Ok(lists)
    }

    /// The instance-visible lists of `owner` (a person page's "Lists shared by them").
    ///
    /// # Errors
    /// Storage errors.
    pub async fn lists_shared_by(&self, actor: Actor, owner: UserId) -> AppResult<Vec<List>> {
        let mut lists = self.lists_on_instance(actor).await?;
        lists.retain(|l| l.owner == owner);
        Ok(lists)
    }

    /// "Add all to my library" on a list the actor may read: returns how many were added and
    /// how many were there already.
    ///
    /// # Errors
    /// Unknown list or not visible to the actor.
    pub async fn add_list_to_library(&self, actor: Actor, id: ListId) -> AppResult<(u32, u32)> {
        let list = self.list(actor, id).await?;
        self.add_all(actor, &list).await
    }

    /// [`App::add_list_to_library`] for a link-shared list, by its token.
    ///
    /// # Errors
    /// Unknown or revoked token.
    pub async fn add_shared_list_to_library(
        &self,
        actor: Actor,
        token: &str,
    ) -> AppResult<(u32, u32)> {
        let list = self.list_by_token(token).await?;
        self.add_all(actor, &list).await
    }

    async fn add_all(&self, actor: Actor, list: &List) -> AppResult<(u32, u32)> {
        let (mut added, mut there) = (0, 0);
        for i in &list.items {
            if self
                .repos
                .library
                .get(actor.user_id, i.item_id)
                .await?
                .is_some()
            {
                there += 1;
            } else {
                self.add_to_library(actor, i.item_id, None).await?;
                added += 1;
            }
        }
        Ok((added, there))
    }

    async fn owned_list(&self, actor: Actor, id: ListId) -> AppResult<List> {
        let list = self.repos.lists.get(id).await?.found("list")?;
        if list.owner == actor.user_id {
            Ok(list)
        } else {
            Err(AppError::Forbidden)
        }
    }

    async fn save_list(&self, mut list: List) -> AppResult<List> {
        list.updated_at = self.now();
        self.repos.lists.update(&list).await?;
        self.publish_list(&list).await;
        Ok(list)
    }

    async fn publish_list(&self, list: &List) {
        self.publish(AppEvent::entity_updated(
            "list",
            list.id.0,
            vec![list.owner],
        ))
        .await;
    }

    /// # Errors
    /// Not the owner or invalid name.
    pub async fn update_list(
        &self,
        actor: Actor,
        id: ListId,
        name: Option<String>,
        description: Option<Option<String>>,
        visibility: Option<Visibility>,
    ) -> AppResult<List> {
        let mut list = self.owned_list(actor, id).await?;
        if let Some(n) = name {
            validate_list_name(&n)?;
            list.name = n.trim().to_string();
        }
        if let Some(d) = description {
            list.description = d;
        }
        if let Some(v) = visibility {
            list.visibility = v;
        }
        self.save_list(list).await
    }

    /// # Errors
    /// Not the owner.
    pub async fn delete_list(&self, actor: Actor, id: ListId) -> AppResult<()> {
        let list = self.owned_list(actor, id).await?;
        self.repos.lists.delete(id).await?;
        self.publish_list(&list).await;
        Ok(())
    }

    /// Append an item (no-op if present).
    ///
    /// # Errors
    /// Not the owner or unknown item.
    pub async fn add_to_list(
        &self,
        actor: Actor,
        id: ListId,
        item_id: ItemId,
        note: Option<String>,
    ) -> AppResult<List> {
        let mut list = self.owned_list(actor, id).await?;
        self.catalog_item(item_id).await?;
        let added = list.add(ListItem {
            item_id,
            note,
            added_at: self.now(),
        });
        if added {
            self.save_list(list).await
        } else {
            Ok(list)
        }
    }

    /// # Errors
    /// Not the owner.
    pub async fn remove_from_list(
        &self,
        actor: Actor,
        id: ListId,
        item_id: ItemId,
    ) -> AppResult<List> {
        let mut list = self.owned_list(actor, id).await?;
        if list.remove(item_id) {
            self.save_list(list).await
        } else {
            Ok(list)
        }
    }

    /// # Errors
    /// Not the owner or item not in the list.
    pub async fn move_in_list(
        &self,
        actor: Actor,
        id: ListId,
        item_id: ItemId,
        to: usize,
    ) -> AppResult<List> {
        let mut list = self.owned_list(actor, id).await?;
        list.move_item(item_id, to)?;
        self.save_list(list).await
    }
}
