//! Library page and Home queries (design.md 6.1, 6.2): counts, filtered and sorted pages,
//! tags, continue and up next. Every row carries its next step (3.4).

use std::collections::{BTreeMap, HashMap};

use domain::DomainError;
use domain::Timestamp;
use domain::catalog::CatalogItem;
use domain::ids::{EntryId, ItemId};
use domain::kind::{KindDescriptor, KindGroup, KindId, Status};
use domain::library::LibraryEntry;
use domain::progress::{ProgressEvent, ProgressSummary};
use domain::tag::{MAX_TAGS_PER_ENTRY, normalize_tag, tag_key};
use domain::user::Actor;

use crate::error::OptionExt;
use crate::next_step::{NextStep, Rewatch, rewatch, step_in};
use crate::ports::{LibraryCursor, LibraryFilter, LibraryHit, LibrarySort};
use crate::{App, AppResult};

/// A library query as the Library page sends it (its URL state).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryQuery {
    /// Only kinds of this group (combined with `kinds`: both must hold).
    pub group: Option<KindGroup>,
    pub kinds: Vec<KindId>,
    pub statuses: Vec<Status>,
    /// Tags as typed (case does not matter); an entry must carry all.
    pub tags: Vec<String>,
    /// `(attribute, value)` pairs; the attribute must be a facet of a selected kind.
    pub facets: Vec<(String, String)>,
    /// Only these items (from a text search).
    pub items: Option<Vec<ItemId>>,
    pub sort: LibrarySort,
    /// `None`: the sort's natural direction.
    pub descending: Option<bool>,
    /// Continue after this row (the previous page's `next`; keyset paging, DEC-416).
    pub after: Option<LibraryCursor>,
    /// 1 to [`App::MAX_PAGE`]; 0 means the default of 48.
    pub limit: u32,
}

/// One library row: the entry with everything a card or table row shows.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryRow {
    pub entry: LibraryEntry,
    pub item: CatalogItem,
    pub group: KindGroup,
    pub last_activity: Timestamp,
    pub tags: Vec<String>,
    pub summary: ProgressSummary,
    pub next_step: NextStep,
    /// The rewatch run to show: running, or stopped with something watched (DEC-587).
    pub rewatch: Option<Rewatch>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryPage {
    pub rows: Vec<LibraryRow>,
    /// Entries matching the query (all pages).
    pub total: u64,
    /// Where the next page starts, when more rows follow.
    pub next: Option<LibraryCursor>,
}

/// One `(kind, status)` cell of the counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountCell {
    pub kind: KindId,
    pub group: KindGroup,
    pub status: Status,
    pub count: u64,
}

/// Entry counts of the signed-in user for the group switcher, kind chips and status tabs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LibraryCounts {
    pub total: u64,
    /// Watch, Read, Listen, Play always; Other only when not empty.
    pub groups: Vec<(KindGroup, u64)>,
    /// Kinds with entries, sorted by id.
    pub kinds: Vec<(KindId, u64)>,
    /// Statuses with entries, core statuses first in their order, then kind-specific ones.
    pub statuses: Vec<(Status, u64)>,
    /// Every non-empty `(kind, status)` pair, for counts under a group or kind selection.
    pub cells: Vec<CountCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCount {
    pub tag: String,
    pub entries: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpNextReason {
    /// The next part of a series you finished the previous part of.
    NextInSeries,
    /// Planned, most recently added first.
    Planned,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpNext {
    pub row: LibraryRow,
    pub reason: UpNextReason,
}

/// Default page size of the library grid.
pub const DEFAULT_PAGE: u32 = 48;

fn status_rank(s: &Status) -> (usize, String) {
    let core = Status::CORE.iter().position(|c| *c == s.as_str());
    (core.unwrap_or(Status::CORE.len()), s.as_str().to_string())
}

impl App {
    pub(crate) fn group_of(&self, kind: &KindId) -> KindGroup {
        self.kinds.get(kind).map_or(KindGroup::Other, |d| d.group)
    }

    /// Kinds selected by a group and an explicit kind list; `None` when nothing can match.
    pub(crate) fn selected_kinds(
        &self,
        group: Option<KindGroup>,
        kinds: &[KindId],
    ) -> Option<Vec<KindId>> {
        let Some(g) = group else {
            return Some(kinds.to_vec());
        };
        let in_group: Vec<KindId> = self
            .kinds
            .all()
            .into_iter()
            .filter(|d| d.group == g)
            .map(|d| d.id)
            .filter(|k| kinds.is_empty() || kinds.contains(k))
            .collect();
        (!in_group.is_empty()).then_some(in_group)
    }

    fn check_facets(&self, kinds: &[KindId], facets: &[(String, String)]) -> AppResult<()> {
        let descriptors: Vec<KindDescriptor> = if kinds.is_empty() {
            self.kinds.all()
        } else {
            kinds.iter().filter_map(|k| self.kinds.get(k)).collect()
        };
        for (f, _) in facets {
            if !descriptors.iter().any(|d| d.facets.contains(f)) {
                return Err(DomainError::invalid("facet", format!("`{f}` is not a facet")).into());
            }
        }
        Ok(())
    }

    /// Entry counts per group, kind and status.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn library_counts(&self, actor: Actor) -> AppResult<LibraryCounts> {
        let raw = self.repos.library.counts(actor.user_id).await?;
        let mut groups: BTreeMap<KindGroup, u64> =
            KindGroup::ALL[..4].iter().map(|g| (*g, 0)).collect();
        let mut kinds: BTreeMap<KindId, u64> = BTreeMap::new();
        let mut statuses: BTreeMap<(usize, String), (Status, u64)> = BTreeMap::new();
        let mut out = LibraryCounts::default();
        for (kind, status, count) in raw {
            let group = self.group_of(&kind);
            out.total += count;
            *groups.entry(group).or_default() += count;
            *kinds.entry(kind.clone()).or_default() += count;
            statuses
                .entry(status_rank(&status))
                .or_insert_with(|| (status.clone(), 0))
                .1 += count;
            out.cells.push(CountCell {
                kind,
                group,
                status,
                count,
            });
        }
        out.groups = groups.into_iter().collect();
        out.kinds = kinds.into_iter().collect();
        out.statuses = statuses.into_values().collect();
        out.cells.sort_by(|a, b| {
            (&a.kind, status_rank(&a.status)).cmp(&(&b.kind, status_rank(&b.status)))
        });
        Ok(out)
    }

    /// One page of the library for the Library page (filters, sort, paging).
    ///
    /// # Errors
    /// A facet that is not a facet of the selected kinds; a cursor of another sort or
    /// direction; storage errors.
    pub async fn library_query(&self, actor: Actor, q: LibraryQuery) -> AppResult<LibraryPage> {
        let Some(kinds) = self.selected_kinds(q.group, &q.kinds) else {
            return Ok(LibraryPage {
                rows: vec![],
                total: 0,
                next: None,
            });
        };
        let descending = q.descending.unwrap_or(q.sort.descending_by_default());
        if let Some(c) = &q.after
            && (c.sort != q.sort || c.descending != descending || !c.key.fits(q.sort))
        {
            return Err(
                DomainError::invalid("after", "the cursor belongs to another order").into(),
            );
        }
        self.check_facets(&kinds, &q.facets)?;
        let mut tags = Vec::new();
        for t in &q.tags {
            tags.push(tag_key(&normalize_tag(t)?));
        }
        let limit = if q.limit == 0 { DEFAULT_PAGE } else { q.limit };
        let filter = LibraryFilter {
            kinds,
            statuses: q.statuses,
            tags,
            facets: q.facets,
            items: q.items,
            sort: q.sort,
            descending,
            after: q.after,
            limit: limit.clamp(1, Self::MAX_PAGE),
        };
        let page = self.repos.library.query(actor.user_id, &filter).await?;
        Ok(LibraryPage {
            rows: self.rows(actor, page.hits).await?,
            total: page.total,
            next: page.next,
        })
    }

    /// The library row of one item in the actor's library.
    ///
    /// # Errors
    /// `NotFound` if the item is not in the library.
    pub async fn library_row(&self, actor: Actor, item_id: ItemId) -> AppResult<LibraryRow> {
        let entry = self.entry(actor, item_id).await?.found("library entry")?;
        let events = self.progress(actor, item_id).await?;
        let last_activity = domain::progress::effective(&events)
            .iter()
            .map(|e| e.occurred_at)
            .max()
            .unwrap_or(entry.added_at)
            .max(entry.added_at);
        Ok(self
            .rows(
                actor,
                vec![LibraryHit {
                    entry,
                    last_activity,
                }],
            )
            .await?
            .remove(0))
    }

    /// The library row of an entry whose item and events (undo events included or not) the
    /// caller already holds: one read, for the tags (the item page, UX1-10).
    ///
    /// # Errors
    /// Unknown kind; storage errors.
    pub async fn entry_row(
        &self,
        entry: LibraryEntry,
        item: CatalogItem,
        events: &[ProgressEvent],
    ) -> AppResult<LibraryRow> {
        let kind = self.kind(&item.kind)?;
        let last_activity = domain::progress::effective(events)
            .iter()
            .map(|e| e.occurred_at)
            .max()
            .map_or(entry.added_at, |t| t.max(entry.added_at));
        let tags = self
            .repos
            .library
            .tags_of(&[entry.id])
            .await?
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        let summary = ProgressSummary::of(events);
        let status = &entry.status.value;
        let next_step = step_in(status, &kind, &item, events, &summary);
        let rewatch = rewatch(status, &kind, &item, events);
        Ok(LibraryRow {
            rewatch,
            tags,
            group: kind.group,
            entry,
            item,
            last_activity,
            summary,
            next_step,
        })
    }

    /// Build rows for hits: items, tags and progress in three batched reads.
    pub(crate) async fn rows(
        &self,
        actor: Actor,
        hits: Vec<LibraryHit>,
    ) -> AppResult<Vec<LibraryRow>> {
        let item_ids: Vec<ItemId> = hits.iter().map(|h| h.entry.item_id).collect();
        let entry_ids: Vec<EntryId> = hits.iter().map(|h| h.entry.id).collect();
        let items: HashMap<ItemId, CatalogItem> = self
            .repos
            .catalog
            .get_many(&item_ids)
            .await?
            .into_iter()
            .map(|i| (i.id, i))
            .collect();
        let mut tags: HashMap<EntryId, Vec<String>> = HashMap::new();
        for (e, t) in self.repos.library.tags_of(&entry_ids).await? {
            tags.entry(e).or_default().push(t);
        }
        let mut events: HashMap<ItemId, Vec<ProgressEvent>> = HashMap::new();
        for e in self
            .repos
            .progress
            .for_items(actor.user_id, &item_ids)
            .await?
        {
            events.entry(e.item_id).or_default().push(e);
        }
        let mut out = Vec::with_capacity(hits.len());
        for h in hits {
            let item = items.get(&h.entry.item_id).cloned().found("catalog item")?;
            let kind = self.kind(&item.kind)?;
            let events = events.get(&item.id).map_or(&[][..], Vec::as_slice);
            let summary = ProgressSummary::of(events);
            let status = &h.entry.status.value;
            let next_step = step_in(status, &kind, &item, events, &summary);
            out.push(LibraryRow {
                rewatch: rewatch(status, &kind, &item, events),
                tags: tags.remove(&h.entry.id).unwrap_or_default(),
                group: kind.group,
                entry: h.entry,
                item,
                last_activity: h.last_activity,
                summary,
                next_step,
            });
        }
        Ok(out)
    }

    /// Home "Continue": in-progress entries, most recent activity first.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn continue_rows(&self, actor: Actor, limit: u32) -> AppResult<Vec<LibraryRow>> {
        let q = LibraryQuery {
            statuses: vec![Status::in_progress()],
            limit,
            ..LibraryQuery::default()
        };
        Ok(self.library_query(actor, q).await?.rows)
    }

    /// Home "Up next": planned entries, the next part of a series you finished the part
    /// before of first (kinds whose attributes have `series` and `series_position`), then
    /// the most recently added. Bounded: the repository answers at most `limit` series
    /// candidates and `limit` planned rows, so the cost does not grow with the library.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn up_next(&self, actor: Actor, limit: u32) -> AppResult<Vec<UpNext>> {
        let limit = limit.clamp(1, Self::MAX_PAGE);
        let series_kinds = self.series_kinds();
        let in_series = if series_kinds.is_empty() {
            vec![]
        } else {
            self.repos
                .library
                .next_in_series(actor.user_id, &series_kinds, limit)
                .await?
        };
        let planned = |items: Option<Vec<ItemId>>| LibraryQuery {
            statuses: vec![Status::planned()],
            sort: LibrarySort::Added,
            items,
            limit,
            ..LibraryQuery::default()
        };
        let mut out: Vec<UpNext> = Vec::new();
        if !in_series.is_empty() {
            for row in self
                .library_query(actor, planned(Some(in_series)))
                .await?
                .rows
            {
                out.push(UpNext {
                    row,
                    reason: UpNextReason::NextInSeries,
                });
            }
        }
        // one more page than needed covers the rows already listed as next in series
        let mut rest = planned(None);
        rest.limit = limit.saturating_mul(2).min(Self::MAX_PAGE);
        for row in self.library_query(actor, rest).await?.rows {
            if !out.iter().any(|u| u.row.entry.id == row.entry.id) {
                out.push(UpNext {
                    row,
                    reason: UpNextReason::Planned,
                });
            }
        }
        out.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
        Ok(out)
    }

    /// Kinds whose attributes carry a series name and a position (books, for example).
    fn series_kinds(&self) -> Vec<KindId> {
        self.kinds
            .all()
            .into_iter()
            .filter(|d| {
                let props = &d.attributes_schema["properties"];
                props.get("series").is_some() && props.get("series_position").is_some()
            })
            .map(|d| d.id)
            .collect()
    }

    // ------------------------------------------------------------ tags

    async fn entry_for_tags(
        &self,
        actor: Actor,
        item_id: ItemId,
    ) -> AppResult<(LibraryEntry, Vec<String>)> {
        let entry = self.add_to_library(actor, item_id, None).await?;
        let tags = self
            .repos
            .library
            .tags_of(&[entry.id])
            .await?
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        Ok((entry, tags))
    }

    async fn save_tags(
        &self,
        entry: &LibraryEntry,
        mut tags: Vec<String>,
    ) -> AppResult<Vec<String>> {
        tags.sort_by_key(|t| tag_key(t));
        self.repos.library.set_tags(entry, &tags).await?;
        self.publish(crate::ports::AppEvent::entity_updated(
            "library_entry",
            entry.id.0,
            vec![entry.user_id],
        ))
        .await;
        Ok(tags)
    }

    /// Add a tag to the entry of `item_id` (added to the library if needed). Returns the
    /// entry's tags sorted by key; adding a tag it has (in any case) changes nothing.
    ///
    /// # Errors
    /// Invalid tag, too many tags, unknown item.
    pub async fn add_tag(
        &self,
        actor: Actor,
        item_id: ItemId,
        tag: &str,
    ) -> AppResult<Vec<String>> {
        let tag = normalize_tag(tag)?;
        let (entry, mut tags) = self.entry_for_tags(actor, item_id).await?;
        if tags.iter().any(|t| tag_key(t) == tag_key(&tag)) {
            return Ok(tags);
        }
        if tags.len() >= MAX_TAGS_PER_ENTRY {
            return Err(DomainError::invalid("tag", "too many tags on this entry").into());
        }
        tags.push(tag);
        self.save_tags(&entry, tags).await
    }

    /// Remove a tag (any case) from the entry of `item_id`; returns the remaining tags.
    ///
    /// # Errors
    /// Not in the library.
    pub async fn remove_tag(
        &self,
        actor: Actor,
        item_id: ItemId,
        tag: &str,
    ) -> AppResult<Vec<String>> {
        let entry = self.entry(actor, item_id).await?.found("library entry")?;
        let key = tag_key(tag.trim());
        let (_, tags) = self.entry_for_tags(actor, item_id).await?;
        if !tags.iter().any(|t| tag_key(t) == key) {
            return Ok(tags);
        }
        let rest = tags.into_iter().filter(|t| tag_key(t) != key).collect();
        self.save_tags(&entry, rest).await
    }

    /// Tags of the entry of `item_id` (empty when it is not in the library).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn entry_tags(&self, actor: Actor, item_id: ItemId) -> AppResult<Vec<String>> {
        let Some(entry) = self.entry(actor, item_id).await? else {
            return Ok(vec![]);
        };
        Ok(self
            .repos
            .library
            .tags_of(&[entry.id])
            .await?
            .into_iter()
            .map(|(_, t)| t)
            .collect())
    }

    /// Every tag of the actor with its number of entries, sorted by key.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn tags(&self, actor: Actor) -> AppResult<Vec<TagCount>> {
        Ok(self
            .repos
            .library
            .tag_counts(actor.user_id)
            .await?
            .into_iter()
            .map(|(tag, entries)| TagCount { tag, entries })
            .collect())
    }

    /// Rename a tag on every entry of the actor; renaming onto an existing tag merges them.
    /// Returns the number of entries changed.
    ///
    /// # Errors
    /// Invalid new name.
    pub async fn rename_tag(&self, actor: Actor, from: &str, to: &str) -> AppResult<u64> {
        let to = normalize_tag(to)?;
        Ok(self
            .repos
            .library
            .rename_tag(actor.user_id, &tag_key(from.trim()), &to)
            .await?)
    }
}

/// `(series name, position)` of an item with both attributes: the name trimmed and in lower
/// case. The rule [`ports::LibraryRepo::next_in_series`] implements.
///
/// [`ports::LibraryRepo::next_in_series`]: crate::ports::LibraryRepo::next_in_series
#[must_use]
pub fn series_of(item: &CatalogItem) -> Option<(String, f64)> {
    let name = item
        .attributes
        .get("series")?
        .as_str()?
        .trim()
        .to_lowercase();
    let pos = item.attributes.get("series_position")?.as_f64()?;
    (!name.is_empty()).then_some((name, pos))
}

impl LibraryRow {
    /// Regular episodes of an episodic item, when known.
    #[must_use]
    pub fn episodes_total(&self) -> Option<u32> {
        crate::next_step::episode_total(&self.item)
    }

    /// Share done, 0..=1, for the progress bar on a card.
    #[must_use]
    pub fn fraction(&self) -> Option<f32> {
        // a running rewatch counts within itself
        if let Some(Rewatch {
            done,
            total: Some(total),
            running: true,
            ..
        }) = self.rewatch
            && total > 0
        {
            #[allow(clippy::cast_precision_loss)]
            return Some((done as f32 / total as f32).min(1.0));
        }
        crate::next_step::fraction(&self.next_step, &self.summary, &self.item)
    }
}
