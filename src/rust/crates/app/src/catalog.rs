use domain::DomainError;
use domain::catalog::{CatalogItem, EntityState, ExternalId};
use domain::ids::ItemId;
use domain::kind::{KindDescriptor, KindId};

use crate::error::OptionExt;
use crate::ports::{AppEvent, ItemAudience};
use crate::{App, AppResult};

/// Catalog data as delivered by a connector or importer.
#[derive(Debug, Clone, PartialEq)]
pub struct NewCatalogItem {
    pub kind: KindId,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub external_ids: Vec<ExternalId>,
    pub attributes: serde_json::Value,
    /// Provider cover URL (redirect fallback, registered as an asset by the refresh job).
    pub cover_url: Option<String>,
    /// See `CatalogItem::popularity`.
    pub popularity: Option<f64>,
}

impl App {
    /// # Errors
    /// Unknown kind.
    pub fn kind(&self, id: &KindId) -> AppResult<KindDescriptor> {
        self.kinds
            .get(id)
            .ok_or_else(|| DomainError::invalid("kind", format!("unknown kind `{id}`")).into())
    }

    /// # Errors
    /// Unknown item.
    pub async fn catalog_item(&self, id: ItemId) -> AppResult<CatalogItem> {
        self.repos.catalog.get(id).await?.found("catalog item")
    }
}

/// Job kind that refreshes one catalog item from its connectors.
pub const REFRESH_JOB: &str = "catalog.refresh";

impl App {
    /// Item detail for a page: returns what is stored right now and, if the item is a stub,
    /// partial or stale, enqueues a background refresh (deduplicated per item). Never waits
    /// on a provider (D-6.1); the refresh publishes `entity.updated` when it is done. For
    /// callers that already watch the item (the item page); others use
    /// [`App::item_detail_for`], or nobody may hear about the refresh (DEC-490).
    ///
    /// # Errors
    /// Unknown item. A failing queue is logged by the adapter and does not fail the read.
    pub async fn item_detail(&self, id: ItemId) -> AppResult<CatalogItem> {
        self.item_detail_where(id, None, &has_external_id).await
    }

    /// [`App::item_detail`] on behalf of `actor` (`GET /items/{id}`): when a refresh is
    /// queued, the actor watches the item first, so its `entity.updated` reaches them even
    /// for a search result nobody has (DEC-492). A fresh item writes nothing.
    ///
    /// # Errors
    /// Unknown item.
    pub async fn item_detail_for(
        &self,
        actor: domain::user::Actor,
        id: ItemId,
    ) -> AppResult<CatalogItem> {
        self.item_detail_where(id, Some(actor), &has_external_id)
            .await
    }

    /// The read behind [`App::item_detail`] and [`App::item_detail_for`] (`watcher`) for a
    /// caller that knows the sources: a refresh is queued only when `answerable` says a
    /// source could answer for the item. An item nobody answers for (entered by hand, a
    /// title-only stub of an import, ids of sources this instance does not have) was queued
    /// on every view and marked failed every hour (v1-plan.md 7, B2). Without the sources
    /// at hand the two callers above ask for any external id.
    ///
    /// # Errors
    /// Unknown item.
    pub async fn item_detail_where(
        &self,
        id: ItemId,
        watcher: Option<domain::user::Actor>,
        answerable: &(dyn Fn(&CatalogItem) -> bool + Sync),
    ) -> AppResult<CatalogItem> {
        let item = self.catalog_item(id).await?;
        if item.needs_refresh(self.now()) && answerable(&item) {
            // before the refresh is queued, so its event cannot come first; best effort
            if let Some(a) = watcher {
                let _ = self.watch_item(a, id).await;
            }
            // A full queue must not break reads; the next view enqueues again.
            let _ = self
                .jobs
                .enqueue(crate::ports::NewJob {
                    lane: crate::ports::Lane::Interactive,
                    kind: REFRESH_JOB.into(),
                    payload: serde_json::json!({ "item_id": id }),
                    dedupe_key: Some(format!("{REFRESH_JOB}:{id}")),
                    priority: 0,
                    run_at: None,
                })
                .await;
        }
        Ok(item)
    }
}

/// The rule of [`App::item_detail`]: without an external id no source can know the item.
fn has_external_id(item: &CatalogItem) -> bool {
    !item.external_ids.is_empty()
}

/// The fields of `item` a person set ([`USER_PROVENANCE`]): no writer of connector data
/// replaces them.
fn fields_by_user(item: &CatalogItem) -> std::collections::BTreeSet<String> {
    item.provenance
        .iter()
        .filter(|(_, source)| source.as_str() == USER_PROVENANCE)
        .map(|(field, _)| field.clone())
        .collect()
}

/// Result of refreshing an item from its connectors (already merged).
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogRefresh {
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub attributes: serde_json::Value,
    pub external_ids: Vec<ExternalId>,
    pub cover_url: Option<String>,
    pub provenance: std::collections::BTreeMap<String, String>,
    /// Seconds until the item is considered stale again.
    pub fresh_for_secs: i64,
    /// See `CatalogItem::popularity`; `None` keeps the stored value.
    pub popularity: Option<f64>,
}

impl App {
    /// Store refreshed data, mark the item `full` and publish `entity.updated`. Attributes
    /// that fail validation are dropped (old ones kept) rather than failing the refresh.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn apply_catalog_refresh(
        &self,
        id: ItemId,
        refresh: CatalogRefresh,
    ) -> AppResult<CatalogItem> {
        self.store_refresh(id, refresh, true).await
    }

    /// The rules shared by a full refresh and a partial fill. A field whose provenance is
    /// [`USER_PROVENANCE`] keeps its value and its provenance (v1-plan.md 7, B3).
    async fn store_refresh(
        &self,
        id: ItemId,
        refresh: CatalogRefresh,
        complete: bool,
    ) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        let now = self.now();
        let by_user = fields_by_user(&item);
        let kept = |field: &str| by_user.contains(field);
        let attributes = keep_user_attributes(refresh.attributes, &item.attributes, &by_user);
        match self.kinds.validate_attributes(&item.kind, &attributes) {
            Ok(attrs) => item.attributes = attrs,
            Err(reason) => {
                // Provider data that does not fit the schema must not break the page.
                item.provenance.insert("attributes.invalid".into(), reason);
            }
        }
        if !refresh.title.trim().is_empty() && !kept("title") {
            item.title = refresh.title;
        }
        if !kept("original_title") {
            item.original_title = refresh.original_title.or(item.original_title);
        }
        if !kept("year") {
            item.year = refresh.year.or(item.year);
        }
        if !kept("description") {
            item.description = refresh.description.or(item.description);
        }
        if !kept("cover_url") {
            item.cover_url = refresh.cover_url.or(item.cover_url);
        }
        item.popularity = refresh.popularity.or(item.popularity);
        for ext in refresh.external_ids {
            if !item.external_ids.contains(&ext) {
                item.external_ids.push(ext);
            }
        }
        item.provenance.extend(
            refresh
                .provenance
                .into_iter()
                .filter(|(field, _)| !by_user.contains(field)),
        );
        item.fetched_at = Some(now);
        if complete {
            item.state = EntityState::Full;
            item.fresh_until = Some(now + chrono::Duration::seconds(refresh.fresh_for_secs));
        } else {
            // the rest (description, subjects) comes with the refresh a page view queues
            item.state = EntityState::Partial;
            item.fresh_until = None;
        }
        self.repos.catalog.upsert(&item).await?;
        self.publish_item(item.id).await;
        Ok(item)
    }

    /// Record a failed refresh: stubs become `failed`; anything else stays as it is and is
    /// retried after `retry_in_secs`.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn mark_catalog_refresh_failed(
        &self,
        id: ItemId,
        retry_in_secs: i64,
    ) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        if item.state == EntityState::Stub {
            item.state = EntityState::Failed;
        }
        item.fresh_until = Some(self.now() + chrono::Duration::seconds(retry_in_secs));
        self.repos.catalog.upsert(&item).await?;
        self.publish_item(item.id).await;
        Ok(item)
    }
}

/// Size limits for catalog data from users (G2-10).
pub const MAX_TITLE_CHARS: usize = 500;
pub const MAX_EXTERNAL_IDS: usize = 10;

fn check_untrusted(new: &NewCatalogItem) -> AppResult<()> {
    let bad =
        |m: &str| -> AppResult<()> { Err(DomainError::invalid("item", m.to_string()).into()) };
    if new.title.chars().count() > MAX_TITLE_CHARS {
        return bad("title is longer than 500 characters");
    }
    if new.external_ids.len() > MAX_EXTERNAL_IDS {
        return bad("more than 10 external ids");
    }
    let ns_ok = |ns: &str| {
        (1..=32).contains(&ns.len())
            && ns
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    if new
        .external_ids
        .iter()
        .any(|e| !ns_ok(&e.namespace) || e.value.is_empty() || e.value.chars().count() > 200)
    {
        return bad(
            "external ids need a namespace of a-z, 0-9, _ (up to 32) and a value up to 200 characters",
        );
    }
    Ok(())
}

impl App {
    /// Like [`App::upsert_catalog_item`], but an item that already exists (same external id)
    /// is returned unchanged except for new external ids and a missing cover URL. Search
    /// results use this so a sparse result never overwrites a refreshed item.
    ///
    /// # Errors
    /// Unknown kind, invalid attributes or a blank title (for new items).
    pub async fn ensure_catalog_item(&self, new: NewCatalogItem) -> AppResult<CatalogItem> {
        self.put_catalog_item(new, OnExisting::Fill).await
    }

    /// For data from users (imports, webhooks), not from connectors: an existing item (same
    /// external id) is returned unchanged, so nobody can attach ids or covers to a shared
    /// item that a refresh would then follow (G2-2). New items are size-checked.
    ///
    /// # Errors
    /// Unknown kind, a blank or too long title, too many or too long ids.
    pub async fn find_or_create_catalog_item(&self, new: NewCatalogItem) -> AppResult<CatalogItem> {
        check_untrusted(&new)?;
        self.put_catalog_item(new, OnExisting::Keep).await
    }

    /// Attach the processed cover asset to an item.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn set_cover_asset(&self, id: ItemId, asset: String) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        item.cover_asset = Some(asset);
        self.repos.catalog.upsert(&item).await?;
        self.publish_item(item.id).await;
        Ok(item)
    }

    /// # Errors
    /// Storage errors.
    pub async fn item_in_any_library(&self, id: ItemId) -> AppResult<bool> {
        Ok(self.repos.library.any_for_item(id).await?)
    }
}

/// What happens to an item that already holds one of the new data's external ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnExisting {
    /// Connector data: replace the fields (`upsert_catalog_item`).
    Merge,
    /// Search results: add missing ids and a missing cover only (`ensure_catalog_item`).
    Fill,
    /// Data from users: return it unchanged (`find_or_create_catalog_item`, G2-2).
    Keep,
}

impl App {
    /// Insert a catalog item, or update the existing one that shares an external id.
    /// Attributes are validated against the kind's schema. Concurrent calls with the same
    /// external id end with one item (DEC-445).
    ///
    /// # Errors
    /// Unknown kind, invalid attributes or a blank title.
    pub async fn upsert_catalog_item(&self, new: NewCatalogItem) -> AppResult<CatalogItem> {
        self.put_catalog_item(new, OnExisting::Merge).await
    }

    async fn existing_item(&self, new: &NewCatalogItem) -> AppResult<Option<CatalogItem>> {
        for ext in &new.external_ids {
            if let Some(found) = self.repos.catalog.by_external_id(&new.kind, ext).await? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    async fn put_catalog_item(
        &self,
        new: NewCatalogItem,
        mode: OnExisting,
    ) -> AppResult<CatalogItem> {
        if let Some(item) = self.existing_item(&new).await? {
            return self.update_existing(item, new, mode).await;
        }
        self.kind(&new.kind)?;
        if new.title.trim().is_empty() {
            return Err(DomainError::invalid("title", "title must not be empty").into());
        }
        let attributes = self
            .kinds
            .validate_attributes(&new.kind, &new.attributes)
            .map_err(|r| DomainError::invalid("attributes", r))?;
        let item = CatalogItem {
            id: ItemId(self.uuid()),
            kind: new.kind.clone(),
            title: new.title.clone(),
            original_title: new.original_title.clone(),
            year: new.year,
            description: new.description.clone(),
            cover_asset: None,
            cover_url: new.cover_url.clone(),
            external_ids: new.external_ids.clone(),
            attributes,
            provenance: std::collections::BTreeMap::new(),
            state: EntityState::Partial,
            fetched_at: None,
            fresh_until: None,
            popularity: new.popularity,
        };
        // A concurrent call may claim one of the ids between the lookup and the insert; the
        // insert then writes nothing and names the winner, which is updated instead.
        let mut attempt = 0;
        loop {
            match self.repos.catalog.insert_new(&item, self.now()).await {
                // a new item is on nobody's page yet: there is nobody to tell (DEC-490)
                Ok(None) => return Ok(item),
                Ok(Some(winner)) => {
                    let existing = self.catalog_item(winner).await?;
                    return self.update_existing(existing, new, mode).await;
                }
                // only the lost race is retried; any other error is returned at once (UX4-18)
                Err(crate::ports::RepoError::Backend(m))
                    if m == crate::ports::CLAIM_RACE && attempt < 2 =>
                {
                    attempt += 1;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    async fn update_existing(
        &self,
        mut item: CatalogItem,
        new: NewCatalogItem,
        mode: OnExisting,
    ) -> AppResult<CatalogItem> {
        let mut changed = false;
        // what a person set stays, whoever writes (review BOOK-10 of v1-books)
        let by_user = fields_by_user(&item);
        let kept = |field: &str| by_user.contains(field);
        match mode {
            OnExisting::Keep => return Ok(item),
            OnExisting::Fill => {
                for e in &new.external_ids {
                    if !item.external_ids.contains(e) {
                        item.external_ids.push(e.clone());
                        changed = true;
                    }
                }
                // an expired item (its title gone with the source's data) gets its data back
                // through a refetch (UX4-9), not from a search result; a cover a person
                // removed is not filled in again. An expired item that kept a title a
                // person set (LC-24) is not recognised here and takes the cover and the
                // popularity below; that write is stamped as source data (`upsert`, the
                // `title <> ''` branch of `provider_data_at`), so the next sweep purges
                // it again and the retention limit holds (review MAN-8)
                if item.cover_url.is_none()
                    && new.cover_url.is_some()
                    && !item.title_unavailable()
                    && !kept("cover_url")
                {
                    item.cover_url = new.cover_url;
                    changed = true;
                }
                // a kept copy from before the value was read gets it; a stored one is left
                // to the refresh, so a search writes nothing for it (DEC-611)
                if item.popularity.is_none()
                    && new.popularity.is_some()
                    && !item.title_unavailable()
                {
                    item.popularity = new.popularity;
                    changed = true;
                }
                if changed {
                    self.repos.catalog.upsert(&item).await?;
                }
                return Ok(item);
            }
            OnExisting::Merge => {}
        }
        self.kind(&new.kind)?;
        if new.title.trim().is_empty() {
            return Err(DomainError::invalid("title", "title must not be empty").into());
        }
        let attributes = keep_user_attributes(new.attributes, &item.attributes, &by_user);
        item.attributes = self
            .kinds
            .validate_attributes(&new.kind, &attributes)
            .map_err(|r| DomainError::invalid("attributes", r))?;
        if !kept("title") {
            item.title = new.title;
        }
        if !kept("original_title") {
            item.original_title = new.original_title.or(item.original_title);
        }
        if !kept("year") {
            item.year = new.year.or(item.year);
        }
        if !kept("description") {
            item.description = new.description.or(item.description);
        }
        for ext in new.external_ids {
            if !item.external_ids.contains(&ext) {
                item.external_ids.push(ext);
            }
        }
        if !kept("cover_url") {
            item.cover_url = new.cover_url.or(item.cover_url);
        }
        item.popularity = new.popularity.or(item.popularity);
        self.repos.catalog.upsert(&item).await?;
        self.publish_item(item.id).await;
        Ok(item)
    }
}

impl App {
    /// Items with an id in one of `namespaces` whose connector data was obtained before
    /// `before`, oldest first (catalog retention, DEC-453).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn stale_provider_data(
        &self,
        namespaces: &[String],
        before: domain::Timestamp,
        limit: u32,
    ) -> AppResult<Vec<(ItemId, domain::Timestamp)>> {
        Ok(self
            .repos
            .catalog
            .provider_data_before(namespaces, before, limit)
            .await?)
    }

    /// Drop the connector data of an item, its title and year included (only the external
    /// ids stay; the person's own data is untouched, and so is every field of the item a
    /// person set, [`USER_PROVENANCE`]; it becomes an expired stub, fetched again once its
    /// source can answer, UX4-9) and publish `entity.updated`. Returns the cover asset it
    /// referenced, for the caller to delete.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn purge_provider_data(&self, id: ItemId) -> AppResult<Option<String>> {
        let cover = self
            .repos
            .catalog
            .purge_provider_data(id, self.now())
            .await?;
        self.publish_item(id).await;
        Ok(cover)
    }

    /// Delete an item nothing refers to (catalog retention, UX4-9).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn delete_unreferenced_item(&self, id: ItemId) -> AppResult<crate::ports::Removal> {
        Ok(self.repos.catalog.delete_unreferenced(id).await?)
    }

    /// Purged items with an id in one of `namespaces` that may be fetched again at `now`.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn expired_provider_data(
        &self,
        namespaces: &[String],
        now: domain::Timestamp,
        limit: u32,
    ) -> AppResult<Vec<ItemId>> {
        Ok(self
            .repos
            .catalog
            .expired_provider_data(namespaces, now, limit)
            .await?)
    }

    /// A refetch of a purged item was queued: the next one not before `next`.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn touch_expired(&self, id: ItemId, next: domain::Timestamp) -> AppResult<()> {
        Ok(self.repos.catalog.touch_expired(id, next).await?)
    }
}

/// How long an opened item page hears about changes of its item when nothing else ties the
/// viewer to it (DEC-490).
pub const ITEM_WATCH_SECS: i64 = 3600;

impl App {
    /// Publish a change of a catalog item to the users it matters to (DEC-490), never to the
    /// whole instance unless the item is on a list the instance can see. Best effort like
    /// every event: a failing audience query publishes nothing.
    pub(crate) async fn publish_item(&self, id: ItemId) {
        let audience = match self.repos.catalog.audience(id, self.now()).await {
            Ok(ItemAudience::Everyone) => Vec::new(),
            Ok(ItemAudience::Users(users)) if !users.is_empty() => users,
            Ok(ItemAudience::Users(_)) | Err(_) => return,
        };
        self.publish(AppEvent::entity_updated("catalog_item", id.0, audience))
            .await;
    }

    /// The actor opened the item page: for [`ITEM_WATCH_SECS`] changes of the item reach
    /// them even when it is not in their library (a stub that is being fetched, DEC-490).
    ///
    /// # Errors
    /// Storage errors.
    pub async fn watch_item(&self, actor: domain::user::Actor, id: ItemId) -> AppResult<()> {
        let now = self.now();
        self.repos
            .catalog
            .watch(
                id,
                actor.user_id,
                now + chrono::Duration::seconds(ITEM_WATCH_SECS),
                now,
            )
            .await?;
        Ok(())
    }
}

/// Provenance of a field a person set on the shared item: a refresh keeps its value
/// (v1-plan.md 7, B3).
pub const USER_PROVENANCE: &str = "user";
/// Largest page total a person may set.
pub const MAX_PAGE_COUNT: u32 = 100_000;
/// The attribute that holds the page total of a paged item.
const PAGE_COUNT: &str = "page_count";

/// `fresh` with the attributes a person set taken from `stored`: kept when the source gives
/// another value, and when it gives none.
fn keep_user_attributes(
    fresh: serde_json::Value,
    stored: &serde_json::Value,
    by_user: &std::collections::BTreeSet<String>,
) -> serde_json::Value {
    let mut names = by_user
        .iter()
        .filter_map(|f| f.strip_prefix("attributes."))
        .peekable();
    if names.peek().is_none() {
        return fresh;
    }
    let mut out = match fresh {
        serde_json::Value::Object(map) => map,
        _ => serde_json::Map::new(),
    };
    for name in names {
        match stored.get(name) {
            Some(v) => out.insert(name.to_string(), v.clone()),
            None => out.remove(name),
        };
    }
    serde_json::Value::Object(out)
}

impl App {
    /// Does an item of `kind` hold a page total: a paged kind whose attributes have
    /// `page_count` (book, comic; a manga counts chapters and has none). The item page
    /// offers "Set page total" only then (review BOOK-5).
    #[must_use]
    pub fn takes_page_total(&self, kind: &KindId) -> bool {
        self.kinds.get(kind).is_some_and(|k| {
            k.progress_model == domain::kind::ProgressModel::Paged && k.has_attribute(PAGE_COUNT)
        })
    }

    /// A person corrects the page total of a paged item (the median of a work is an
    /// estimate, v1-plan.md 3.1.3). The value is marked as theirs, so a refresh keeps it.
    /// A change of the shared item: who may do it is the caller's rule.
    ///
    /// # Errors
    /// Unknown item, a kind that holds no page total ([`App::takes_page_total`]), a total
    /// outside 1..=[`MAX_PAGE_COUNT`].
    pub async fn set_item_page_count(&self, id: ItemId, pages: u32) -> AppResult<CatalogItem> {
        let mut item = self.catalog_item(id).await?;
        if !self.takes_page_total(&item.kind) {
            return Err(DomainError::invalid("page_count", "this kind has no pages").into());
        }
        if !(1..=MAX_PAGE_COUNT).contains(&pages) {
            return Err(DomainError::invalid("page_count", "1 to 100000 pages").into());
        }
        let mut attributes = match std::mem::take(&mut item.attributes) {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        attributes.insert(PAGE_COUNT.into(), pages.into());
        item.attributes = self
            .kinds
            .validate_attributes(&item.kind, &serde_json::Value::Object(attributes))
            .map_err(|r| DomainError::invalid("page_count", r))?;
        item.provenance
            .insert(format!("attributes.{PAGE_COUNT}"), USER_PROVENANCE.into());
        self.repos.catalog.upsert(&item).await?;
        self.publish_item(item.id).await;
        Ok(item)
    }

    /// Store what a batch answer holds for an item (title, authors, year, cover, page count)
    /// and leave it due for its full refresh, which brings the rest when its page is opened.
    ///
    /// # Errors
    /// Unknown item or storage errors.
    pub async fn apply_catalog_partial(
        &self,
        id: ItemId,
        refresh: CatalogRefresh,
    ) -> AppResult<CatalogItem> {
        self.store_refresh(id, refresh, false).await
    }

    /// Give the items that never got their data another refresh (v1-plan.md 7, B1): every
    /// item [`App::unfetched_items`] names (of `kind`, when given) that holds an external id
    /// in one of `namespaces` (the ids of the registered sources) is due again, a failed one
    /// is a stub again, and each gets a refresh job on the background lane. Returns them.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn requeue_failed(
        &self,
        kind: Option<&KindId>,
        namespaces: &[String],
    ) -> AppResult<Vec<CatalogItem>> {
        let mut again = Vec::new();
        for mut item in self.unfetched_items(kind).await? {
            if !item
                .external_ids
                .iter()
                .any(|e| namespaces.contains(&e.namespace))
            {
                continue;
            }
            if item.state == EntityState::Failed {
                item.state = EntityState::Stub;
            }
            item.fresh_until = None;
            self.repos.catalog.upsert(&item).await?;
            self.jobs
                .enqueue(crate::ports::NewJob {
                    lane: crate::ports::Lane::BackgroundRefresh,
                    kind: REFRESH_JOB.into(),
                    payload: serde_json::json!({ "item_id": item.id }),
                    dedupe_key: Some(format!("{REFRESH_JOB}:{}", item.id)),
                    priority: 0,
                    run_at: None,
                })
                .await
                .map_err(|e| crate::ports::RepoError::Backend(e.to_string()))?;
            self.publish_item(item.id).await;
            again.push(item);
        }
        Ok(again)
    }

    /// The items in somebody's library that hold an external id in `namespace` (the id of a
    /// source), by title; search results nobody added are left out. With `apply` each is due again and gets a refresh job on the background lane
    /// (`cli refresh --source`, for a release whose connector reads more than before, such
    /// as the `TheTVDB` episode counts of 1.0.1, #3). Returns them.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn requeue_source(
        &self,
        namespace: &str,
        apply: bool,
    ) -> AppResult<Vec<CatalogItem>> {
        let mut found = Vec::new();
        let ids = self.repos.catalog.library_items_from(namespace).await?;
        for mut item in self.repos.catalog.get_many(&ids).await? {
            if apply {
                item.fresh_until = None;
                self.repos.catalog.upsert(&item).await?;
                self.jobs
                    .enqueue(crate::ports::NewJob {
                        lane: crate::ports::Lane::BackgroundRefresh,
                        kind: REFRESH_JOB.into(),
                        payload: serde_json::json!({ "item_id": item.id }),
                        dedupe_key: Some(format!("{REFRESH_JOB}:{}", item.id)),
                        priority: 0,
                        run_at: None,
                    })
                    .await
                    .map_err(|e| crate::ports::RepoError::Backend(e.to_string()))?;
            }
            found.push(item);
        }
        Ok(found)
    }

    /// The items that never got their data (of `kind`, when given): in state `failed` (a
    /// stub whose refresh failed is never refreshed again on its own), or never fetched and
    /// in somebody's library (an import stub, which is `partial` and only retried when its
    /// page is opened). Search results nobody added are not among them.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn unfetched_items(&self, kind: Option<&KindId>) -> AppResult<Vec<CatalogItem>> {
        let ids = self.repos.catalog.unfetched_items(kind).await?;
        Ok(self.repos.catalog.get_many(&ids).await?)
    }

    /// The items of `kind` that hold no external id in `namespace`, by title.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn items_without_id_from(
        &self,
        kind: &KindId,
        namespace: &str,
    ) -> AppResult<Vec<CatalogItem>> {
        let ids = self
            .repos
            .catalog
            .items_without_namespace(kind, namespace)
            .await?;
        let mut items = self.repos.catalog.get_many(&ids).await?;
        items.sort_by_key(|i| ids.iter().position(|id| *id == i.id));
        Ok(items)
    }
}
