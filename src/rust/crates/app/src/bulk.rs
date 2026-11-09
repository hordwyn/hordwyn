//! Remove from library and bulk edits with one undo (design.md 3.5, 6.2, 6.3; WP4-3).
//!
//! Removing deletes the entry and its tags only: progress events, sessions, reviews and list
//! memberships stay ("delete keeps history", DEC-348), so statistics keep counting the
//! history and re-adding the item shows it again. Every bulk operation returns a
//! [`BulkUndo`]: the inverse of what that change did (the statuses it set, the tag it added or
//! took off, the items a list gained, the entries it removed), which the server signs into an
//! opaque token the client sends back to undo (DEC-417, UX4-5).

use std::collections::{HashMap, HashSet};

use domain::DomainError;
use domain::Timestamp;
use domain::hlc::Hlc;
use domain::ids::{EntryId, ItemId, ListId};
use domain::kind::Status;
use domain::library::{LibraryEntry, Rating};
use domain::list::{List, ListItem};
use domain::tag::{MAX_TAGS_PER_ENTRY, normalize_tag, tag_key};
use domain::user::Actor;

use crate::ports::AppEvent;
use crate::{App, AppError, AppResult};

mod undo;

/// Most ids one bulk request may name.
pub const MAX_BULK: usize = 500;

/// Largest undo a bulk request may hand out, as JSON: the signed token (base64, about 4/3 of
/// it) must fit the 2 MiB request body limit of the undo route (UX4-12).
pub const MAX_UNDO_BYTES: usize = 1 << 20;

/// What a bulk request does to every named item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BulkOp {
    SetStatus(Status),
    AddTag(String),
    RemoveTag(String),
    AddToList(ListId),
    Remove,
}

/// Why one id of a bulk request was left alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkFailure {
    /// Not in the caller's library (unknown, or someone else's).
    NotInLibrary,
    /// The item's kind does not have that status.
    StatusNotAllowed,
    /// The entry carries the most tags it may.
    TooManyTags,
}

impl BulkFailure {
    /// The error code clients show (`library-bulk-error-<code>`).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotInLibrary => "not_in_library",
            Self::StatusNotAllowed => "status_not_allowed",
            Self::TooManyTags => "too_many_tags",
        }
    }
}

/// One entry as it was before it was removed (the undo of a removal brings it back).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EntrySnapshot {
    pub entry_id: EntryId,
    pub item_id: ItemId,
    pub status: String,
    pub rating: Option<u8>,
    pub notes: Option<String>,
    pub added_at: Timestamp,
    pub tags: Vec<String>,
}

/// A status a bulk change set, and the one it replaced.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StatusBack {
    pub item_id: ItemId,
    pub before: String,
    /// The stamp the change wrote: the undo applies only while the status still carries it.
    pub set: Hlc,
}

/// The inverse of one bulk change: only what that change did is put back, so a change made in
/// between (the item page, another device, an import) stays (UX4-5).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BulkUndo {
    /// The change changed nothing.
    #[default]
    Nothing,
    /// Statuses go back where they still carry the change's stamp.
    Status { entries: Vec<StatusBack> },
    /// The change added `tag` to these items: it comes off again where still there.
    TagAdded { tag: String, items: Vec<ItemId> },
    /// The change took a tag off these items (as each spelled it): it goes back where missing.
    TagRemoved { items: Vec<(ItemId, String)> },
    /// The list gained these items: they leave it again (a deleted list: nothing to do).
    ListGained { list: ListId, items: Vec<ItemId> },
    /// The removed entries come back, unless the item was added again meanwhile.
    Restore { entries: Vec<EntrySnapshot> },
}

/// The outcome of a bulk request: every distinct id in request order with `None` when it
/// was changed (or already as asked), and the undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOutcome {
    pub results: Vec<(ItemId, Option<BulkFailure>)>,
    pub undo: BulkUndo,
}

impl App {
    /// Remove an item from the actor's library: the entry and its tags go, history stays.
    /// Returns the undo.
    ///
    /// # Errors
    /// `NotFound` when the item is not in the library; storage errors.
    pub async fn remove_from_library(&self, actor: Actor, item_id: ItemId) -> AppResult<BulkUndo> {
        let out = self.bulk(actor, &[item_id], BulkOp::Remove).await?;
        match out.results.first() {
            Some((_, None)) => Ok(out.undo),
            _ => Err(AppError::NotFound("library entry")),
        }
    }

    /// Apply `op` to every distinct id: entry writes in one transaction, removals in one, the
    /// list in one. Ids not in the caller's library (or not fitting the op) fail alone.
    ///
    /// # Errors
    /// More than [`MAX_BULK`] ids or an invalid tag (`Domain`); an undo larger than
    /// [`MAX_UNDO_BYTES`] (`Domain`, nothing changed); a list the actor does not own; storage
    /// errors.
    pub async fn bulk(&self, actor: Actor, ids: &[ItemId], op: BulkOp) -> AppResult<BulkOutcome> {
        if ids.len() > MAX_BULK {
            return Err(DomainError::invalid("item_ids", format!("at most {MAX_BULK} ids")).into());
        }
        let mut seen = HashSet::new();
        let ids: Vec<ItemId> = ids.iter().copied().filter(|i| seen.insert(*i)).collect();
        let tag = match &op {
            BulkOp::AddTag(t) | BulkOp::RemoveTag(t) => Some(normalize_tag(t)?),
            _ => None,
        };
        let mut list = match &op {
            BulkOp::AddToList(id) => Some(self.owned_list(actor, *id).await?),
            _ => None,
        };
        let found: HashMap<ItemId, LibraryEntry> = self
            .repos
            .library
            .entries_for(actor.user_id, &ids)
            .await?
            .into_iter()
            .map(|e| (e.item_id, e))
            .collect();
        let entry_ids: Vec<EntryId> = found.values().map(|e| e.id).collect();
        let mut tags: HashMap<EntryId, Vec<String>> = HashMap::new();
        for (e, t) in self.repos.library.tags_of(&entry_ids).await? {
            tags.entry(e).or_default().push(t);
        }
        let mut plan = Plan::default();
        let mut results = Vec::with_capacity(ids.len());
        for id in &ids {
            let failure = match found.get(id) {
                None => Some(BulkFailure::NotInLibrary),
                Some(e) => {
                    let had = tags.remove(&e.id).unwrap_or_default();
                    self.plan_one(&op, tag.as_deref(), list.as_mut(), e, &had, &mut plan)?
                }
            };
            results.push((*id, failure));
        }
        let undo = plan.undo_of(&op, tag, list.as_ref().map(|l| l.id));
        if serde_json::to_vec(&undo).map_or(usize::MAX, |b| b.len()) > MAX_UNDO_BYTES {
            return Err(DomainError::invalid(
                "item_ids",
                "too much to undo at once; change fewer items at a time",
            )
            .into());
        }
        self.run_plan(actor, plan, list, results, undo).await
    }

    /// What `op` does to one entry, added to `plan`; `Some` when it fails for this entry.
    fn plan_one(
        &self,
        op: &BulkOp,
        tag: Option<&str>,
        list: Option<&mut List>,
        e: &LibraryEntry,
        had: &[String],
        plan: &mut Plan,
    ) -> AppResult<Option<BulkFailure>> {
        match op {
            BulkOp::SetStatus(s) => {
                if self.kind(&e.kind)?.check_status(s).is_err() {
                    return Ok(Some(BulkFailure::StatusNotAllowed));
                }
                let mut changed = e.clone();
                let stamp = self.stamp(None);
                if changed.status.set(s.clone(), stamp) {
                    plan.status.push(StatusBack {
                        item_id: e.item_id,
                        before: e.status.value.as_str().to_string(),
                        set: stamp,
                    });
                    plan.writes.push((changed, None));
                }
            }
            BulkOp::AddTag(_) | BulkOp::RemoveTag(_) => {
                let t = tag.unwrap_or_default();
                let key = tag_key(t);
                let was = had.iter().find(|x| tag_key(x) == key);
                let mut next = had.to_vec();
                if matches!(op, BulkOp::AddTag(_)) {
                    if was.is_some() {
                        return Ok(None);
                    }
                    if had.len() >= MAX_TAGS_PER_ENTRY {
                        return Ok(Some(BulkFailure::TooManyTags));
                    }
                    next.push(t.to_string());
                    next.sort_by_key(|x| tag_key(x));
                    plan.items.push(e.item_id);
                } else if let Some(was) = was {
                    plan.untagged.push((e.item_id, was.clone()));
                    next.retain(|x| tag_key(x) != key);
                } else {
                    return Ok(None);
                }
                plan.writes.push((e.clone(), Some(next)));
            }
            BulkOp::AddToList(_) => {
                if let Some(l) = list {
                    let added = l.add(ListItem {
                        item_id: e.item_id,
                        note: None,
                        added_at: self.now(),
                    });
                    if added {
                        plan.items.push(e.item_id);
                    }
                }
            }
            BulkOp::Remove => {
                plan.removed.push(snapshot(e, had));
                plan.removes.push(e.id);
            }
        }
        Ok(None)
    }

    async fn run_plan(
        &self,
        actor: Actor,
        plan: Plan,
        list: Option<List>,
        results: Vec<(ItemId, Option<BulkFailure>)>,
        undo: BulkUndo,
    ) -> AppResult<BulkOutcome> {
        let first = plan
            .writes
            .first()
            .map(|(e, _)| e.id)
            .or_else(|| plan.removes.first().copied());
        if !plan.writes.is_empty() {
            self.repos.library.write_many(&plan.writes).await?;
        }
        if !plan.removes.is_empty() {
            self.repos
                .library
                .remove_many(actor.user_id, &plan.removes)
                .await?;
        }
        // the list is saved only when it gained something
        if let Some(l) = list
            && matches!(undo, BulkUndo::ListGained { .. })
        {
            self.save_list(l).await?;
        }
        // one event for the whole change (each entry would refresh open pages once each)
        if let Some(id) = first {
            self.publish(AppEvent::entity_updated(
                "library_entry",
                id.0,
                vec![actor.user_id],
            ))
            .await;
        }
        Ok(BulkOutcome { results, undo })
    }
}

/// The writes a bulk request makes, and what its undo needs.
#[derive(Default)]
struct Plan {
    writes: Vec<(LibraryEntry, Option<Vec<String>>)>,
    removes: Vec<EntryId>,
    status: Vec<StatusBack>,
    /// Items that gained or lost the tag, or that the list gained.
    items: Vec<ItemId>,
    /// Items that lost the tag, with the spelling they had.
    untagged: Vec<(ItemId, String)>,
    removed: Vec<EntrySnapshot>,
}

impl Plan {
    fn undo_of(&mut self, op: &BulkOp, tag: Option<String>, list: Option<ListId>) -> BulkUndo {
        let items = std::mem::take(&mut self.items);
        match (op, tag, list) {
            (BulkOp::SetStatus(_), _, _) if !self.status.is_empty() => BulkUndo::Status {
                entries: std::mem::take(&mut self.status),
            },
            (BulkOp::AddTag(_), Some(tag), _) if !items.is_empty() => {
                BulkUndo::TagAdded { tag, items }
            }
            (BulkOp::RemoveTag(_), _, _) if !self.untagged.is_empty() => BulkUndo::TagRemoved {
                items: std::mem::take(&mut self.untagged),
            },
            (BulkOp::AddToList(_), _, Some(list)) if !items.is_empty() => {
                BulkUndo::ListGained { list, items }
            }
            (BulkOp::Remove, _, _) if !self.removed.is_empty() => BulkUndo::Restore {
                entries: std::mem::take(&mut self.removed),
            },
            _ => BulkUndo::Nothing,
        }
    }
}

fn snapshot(e: &LibraryEntry, tags: &[String]) -> EntrySnapshot {
    EntrySnapshot {
        entry_id: e.id,
        item_id: e.item_id,
        status: e.status.value.as_str().to_string(),
        rating: e.rating.value.map(Rating::value),
        notes: e.notes.value.clone(),
        added_at: e.added_at,
        tags: tags.to_vec(),
    }
}
