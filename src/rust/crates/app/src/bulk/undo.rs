//! Undoing a bulk change: each kind of change has its own inverse that touches only what the
//! change did, so a change made in between stays (UX4-5). Every read is one batch, and an
//! undo writes in one transaction (or saves one list).

use std::collections::{HashMap, HashSet};

use domain::DomainError;
use domain::catalog::CatalogItem;
use domain::hlc::Lww;
use domain::ids::{EntryId, ItemId, ListId};
use domain::kind::Status;
use domain::library::{LibraryEntry, Rating};
use domain::tag::{MAX_TAGS_PER_ENTRY, normalize_tag, tag_key};
use domain::user::Actor;

use super::{BulkUndo, EntrySnapshot, MAX_BULK, StatusBack};
use crate::error::OptionExt;
use crate::ports::AppEvent;
use crate::{App, AppError, AppResult};

type Write = (LibraryEntry, Option<Vec<String>>);

impl App {
    /// Undo a bulk change in the actor's own library. Statuses go back only where they still
    /// carry the change's stamp; a tag comes off (or back) only on the items the change
    /// touched; removed entries come back (with their id and when they were added, unless the
    /// id is taken) only when the item was not added again; a list loses only the items it
    /// gained, and a list deleted since is nothing to undo.
    ///
    /// # Errors
    /// An undo that does not fit (unknown item, a status the kind does not have, an invalid
    /// rating or tag, too many entries); a list someone else owns; storage errors.
    pub async fn undo_bulk(&self, actor: Actor, undo: &BulkUndo) -> AppResult<()> {
        let writes = match undo {
            BulkUndo::Nothing => return Ok(()),
            BulkUndo::ListGained { list, items } => {
                return self.undo_list(actor, *list, items).await;
            }
            BulkUndo::Status { entries } => self.undo_status(actor, entries).await?,
            BulkUndo::TagAdded { tag, items } => {
                let touched: Vec<(ItemId, String)> =
                    items.iter().map(|i| (*i, tag.clone())).collect();
                self.undo_tag(actor, &touched, false).await?
            }
            BulkUndo::TagRemoved { items } => self.undo_tag(actor, items, true).await?,
            BulkUndo::Restore { entries } => self.undo_restore(actor, entries).await?,
        };
        if let Some((first, _)) = writes.first() {
            let id = first.id;
            self.repos.library.write_many(&writes).await?;
            self.publish(AppEvent::entity_updated(
                "library_entry",
                id.0,
                vec![actor.user_id],
            ))
            .await;
        }
        Ok(())
    }

    async fn current(
        &self,
        actor: Actor,
        items: &[ItemId],
    ) -> AppResult<HashMap<ItemId, LibraryEntry>> {
        if items.len() > MAX_BULK {
            return Err(DomainError::invalid("undo", format!("at most {MAX_BULK} ids")).into());
        }
        Ok(self
            .repos
            .library
            .entries_for(actor.user_id, items)
            .await?
            .into_iter()
            .map(|e| (e.item_id, e))
            .collect())
    }

    async fn undo_status(&self, actor: Actor, back: &[StatusBack]) -> AppResult<Vec<Write>> {
        let ids: Vec<ItemId> = back.iter().map(|b| b.item_id).collect();
        let mut current = self.current(actor, &ids).await?;
        let mut writes = Vec::new();
        for b in back {
            let Some(mut e) = current.remove(&b.item_id) else {
                continue;
            };
            // changed again since the bulk change: the newer change stays
            if e.status.stamp != b.set {
                continue;
            }
            let before = Status::new(&b.before);
            self.kind(&e.kind)?.check_status(&before)?;
            e.status = Lww::new(before, self.stamp(None));
            writes.push((e, None));
        }
        Ok(writes)
    }

    /// Take a tag off (`put_back` false) or put it back on the items the change touched,
    /// each with its own spelling.
    async fn undo_tag(
        &self,
        actor: Actor,
        touched: &[(ItemId, String)],
        put_back: bool,
    ) -> AppResult<Vec<Write>> {
        let ids: Vec<ItemId> = touched.iter().map(|(i, _)| *i).collect();
        let current = self.current(actor, &ids).await?;
        let entry_ids: Vec<EntryId> = current.values().map(|e| e.id).collect();
        let mut tags: HashMap<EntryId, Vec<String>> = HashMap::new();
        for (e, t) in self.repos.library.tags_of(&entry_ids).await? {
            tags.entry(e).or_default().push(t);
        }
        let mut writes = Vec::new();
        for (item, tag) in touched {
            let Some(e) = current.get(item) else {
                continue;
            };
            let tag = normalize_tag(tag)?;
            let key = tag_key(&tag);
            let mut next = tags.remove(&e.id).unwrap_or_default();
            let has = next.iter().any(|x| tag_key(x) == key);
            if put_back && !has && next.len() < MAX_TAGS_PER_ENTRY {
                next.push(tag);
                next.sort_by_key(|x| tag_key(x));
            } else if !put_back && has {
                next.retain(|x| tag_key(x) != key);
            } else {
                continue;
            }
            writes.push((e.clone(), Some(next)));
        }
        Ok(writes)
    }

    async fn undo_list(&self, actor: Actor, list: ListId, items: &[ItemId]) -> AppResult<()> {
        if items.len() > MAX_BULK {
            return Err(DomainError::invalid("undo", format!("at most {MAX_BULK} ids")).into());
        }
        let mut list = match self.owned_list(actor, list).await {
            Ok(l) => l,
            // deleted since: nothing left to take out
            Err(AppError::NotFound(_)) => return Ok(()),
            Err(e) => return Err(e),
        };
        let mut changed = false;
        for i in items {
            changed |= list.remove(*i);
        }
        if changed {
            self.save_list(list).await?;
        }
        Ok(())
    }

    async fn undo_restore(&self, actor: Actor, removed: &[EntrySnapshot]) -> AppResult<Vec<Write>> {
        let ids: Vec<ItemId> = removed.iter().map(|s| s.item_id).collect();
        let current = self.current(actor, &ids).await?;
        // added again since the removal: that entry stays as it is
        let gone: Vec<&EntrySnapshot> = removed
            .iter()
            .filter(|s| !current.contains_key(&s.item_id))
            .collect();
        let wanted: Vec<ItemId> = gone.iter().map(|s| s.item_id).collect();
        let items: HashMap<ItemId, CatalogItem> = self
            .repos
            .catalog
            .get_many(&wanted)
            .await?
            .into_iter()
            .map(|i| (i.id, i))
            .collect();
        let old: Vec<EntryId> = gone.iter().map(|s| s.entry_id).collect();
        let taken: HashSet<EntryId> = self
            .repos
            .library
            .existing_entry_ids(&old)
            .await?
            .into_iter()
            .collect();
        let mut writes = Vec::with_capacity(gone.len());
        for s in gone {
            let item = items.get(&s.item_id).found("catalog item")?;
            let id = if taken.contains(&s.entry_id) {
                EntryId(self.uuid())
            } else {
                s.entry_id
            };
            writes.push(self.restored(actor, s, item, id)?);
        }
        Ok(writes)
    }

    /// The entry a snapshot describes, checked against the item's kind, with new stamps (it
    /// is a new entry) and never added later than now.
    fn restored(
        &self,
        actor: Actor,
        s: &EntrySnapshot,
        item: &CatalogItem,
        id: EntryId,
    ) -> AppResult<Write> {
        let status = Status::new(&s.status);
        self.kind(&item.kind)?.check_status(&status)?;
        let rating = s.rating.map(Rating::new).transpose()?;
        let mut tags: Vec<String> = Vec::new();
        for t in &s.tags {
            let t = normalize_tag(t)?;
            if !tags.iter().any(|x| tag_key(x) == tag_key(&t)) {
                tags.push(t);
            }
        }
        if tags.len() > MAX_TAGS_PER_ENTRY {
            return Err(DomainError::invalid("tags", "too many tags").into());
        }
        tags.sort_by_key(|x| tag_key(x));
        let entry = LibraryEntry {
            id,
            user_id: actor.user_id,
            item_id: item.id,
            kind: item.kind.clone(),
            status: Lww::new(status, self.stamp(None)),
            rating: Lww::new(rating, self.stamp(None)),
            notes: Lww::new(s.notes.clone(), self.stamp(None)),
            added_at: s.added_at.min(self.now()),
        };
        Ok((entry, Some(tags)))
    }
}
