use domain::ids::{ItemId, ListId};
use domain::list::{List, ListItem, Visibility, validate_list_name};
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

    /// # Errors
    /// Storage errors.
    pub async fn lists(&self, actor: Actor) -> AppResult<Vec<List>> {
        Ok(self.repos.lists.by_owner(actor.user_id).await?)
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
