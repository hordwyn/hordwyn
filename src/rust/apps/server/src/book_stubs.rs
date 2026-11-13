//! `cli refresh-failed` and `cli match-books` (v1-books, v1-plan.md 3.1.4 and 7 B1): the
//! items that never got their data are asked again, and title-only books without an Open
//! Library id are matched by title. Both open the database directly, like the escape
//! hatches; the refresh jobs they queue are run by the server's worker.
//!
//! Open Library asks clients not to make "hundreds of single-book requests (use search.json
//! for batch results)" (docs/connectors/openlibrary.md): books with a work id are therefore
//! filled from one search per 50 works, and get their description with the normal refresh
//! when their page is opened.

use std::sync::Arc;

use anyhow::Context;
use app::App;
use app::ports::{HttpFetch, Lane};
use catalog_core::{Ctx, Snapshot};
use config::AppConfig;
use domain::catalog::CatalogItem;
#[cfg(feature = "openlibrary")]
use domain::catalog::ExternalId;
use domain::ids::ItemId;
use domain::kind::KindId;

/// The kind both commands treat specially.
const BOOK: &str = "book";

/// What `refresh-failed` did, or without `--apply` what it would do.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefreshReport {
    /// `false`: the dry run. Nothing was asked and nothing was written.
    pub applied: bool,
    /// Books filled from a batch answer (title, authors, year, cover, page count); in a dry
    /// run the books that would be asked for in batches.
    pub filled: Vec<(ItemId, String)>,
    /// Items that got a refresh job (dry run: that would get one).
    pub queued: Vec<(ItemId, String)>,
    /// One line per batch search that failed, with the reason; its books went to the jobs.
    pub failed: Vec<String>,
    /// Items that never got their data and hold no id of a registered source.
    pub unknown: usize,
}

/// One book `match-books` looked at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchRow {
    pub item: ItemId,
    pub title: String,
    pub year: Option<i32>,
    pub outcome: MatchOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchOutcome {
    /// Exactly one work has the same title (and nothing speaks against it).
    Matched {
        id: String,
        title: String,
        authors: Vec<String>,
        year: Option<i32>,
    },
    /// Left as it is, and why.
    Unmatched(String),
}

/// A title as it is compared: lower case, punctuation as spaces, one space between words,
/// a leading article dropped ("The Hobbit" and "hobbit, the" are not the same; nothing is
/// guessed).
#[must_use]
pub fn normalise_title(title: &str) -> String {
    const ARTICLES: [&str; 14] = [
        "the", "a", "an", "der", "die", "das", "ein", "eine", "le", "la", "les", "el", "los", "las",
    ];
    let folded: String = title
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut words: Vec<&str> = folded.split_whitespace().collect();
    if words.len() > 1 && ARTICLES.contains(&words[0]) {
        words.remove(0);
    }
    words.join(" ")
}

/// How many rows `match-books` asks for per title: a work is taken only when it is the one
/// row among all of these with the same title (review BOOK-9: among five rows a common
/// title looked unique too easily). One request either way.
pub const MATCH_ROWS: u32 = 20;

/// Did a person enter `item` by hand ("Add by hand" marks its title as theirs)? Such an item
/// is no candidate of `match-books` (review MAN-4 of v1-manual).
#[cfg(feature = "openlibrary")]
fn entered_by_hand(item: &CatalogItem) -> bool {
    item.provenance.get("title").map(String::as_str) == Some(app::USER_PROVENANCE)
}

fn authors_of(attributes: &serde_json::Value) -> Vec<String> {
    attributes
        .get("authors")
        .and_then(|a| serde_json::from_value(a.clone()).ok())
        .unwrap_or_default()
}

/// The one candidate that may be taken for a stub with `title`, `year` and, when it has
/// one, the first author `author`. Every row of the answer is looked at.
///
/// # Errors
/// The reason when there is none: no result, no or several results with the same title, a
/// year that differs, or a first author the result does not name.
pub fn pick<'a>(
    title: &str,
    year: Option<i32>,
    author: Option<&str>,
    candidates: &'a [Snapshot],
) -> Result<&'a Snapshot, String> {
    if candidates.is_empty() {
        return Err("no result".into());
    }
    let want = normalise_title(title);
    let same: Vec<&Snapshot> = candidates
        .iter()
        .filter(|c| normalise_title(&c.title) == want)
        .collect();
    let one = match same.as_slice() {
        [] => return Err("no result with the same title".into()),
        [one] => *one,
        many => return Err(format!("{} results with the same title", many.len())),
    };
    match (year, one.year) {
        (Some(have), Some(theirs)) if have != theirs => {
            return Err(format!("the year differs (here {have}, there {theirs})"));
        }
        (Some(have), None) => {
            return Err(format!("the year differs (here {have}, there none)"));
        }
        _ => {}
    }
    // names are compared like titles; a result without authors says nothing against it
    let theirs = authors_of(&one.attributes);
    let named = |a: &str| {
        theirs
            .iter()
            .any(|t| normalise_title(t) == normalise_title(a))
    };
    match author.filter(|a| !theirs.is_empty() && !named(a)) {
        Some(have) => Err(format!(
            "the author differs (here {have}, there {})",
            theirs.join(", ")
        )),
        None => Ok(one),
    }
}

/// Where `match-books --apply` writes down what it changed (review BOOK-9): the item as it
/// was and the id it got, so a wrong match can be found and put back.
#[async_trait::async_trait]
pub trait MatchLog: Send + Sync {
    /// # Errors
    /// Storage errors; the match is not stored then.
    async fn matched(&self, before: &CatalogItem, id: &str) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
impl MatchLog for store_pg::PgStore {
    async fn matched(&self, before: &CatalogItem, id: &str) -> anyhow::Result<()> {
        self.log_catalog_match(before, BOOK_SOURCE, id)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}

/// The source id the match log names (the connector's id, also when it is not compiled in).
const BOOK_SOURCE: &str = "openlibrary";

/// The refresh of an item from one search row.
#[cfg(feature = "openlibrary")]
fn row_refresh(row: &Snapshot) -> Option<app::CatalogRefresh> {
    use catalog_core::merge::{FieldPriority, merge};
    let priority = FieldPriority {
        default: vec![row.connector.clone()],
        ..FieldPriority::default()
    };
    let m = merge(std::slice::from_ref(row), &priority)?;
    Some(app::CatalogRefresh {
        title: m.title,
        original_title: m.original_title,
        year: m.year,
        description: m.description,
        attributes: m.attributes,
        external_ids: m.external_ids,
        cover_url: m.cover_url,
        provenance: m.provenance,
        // a partial fill is due at once: the description comes with the full refresh
        fresh_for_secs: 0,
        popularity: m.popularity,
    })
}

fn named(item: &CatalogItem) -> (ItemId, String) {
    (item.id, item.title.clone())
}

#[cfg(feature = "openlibrary")]
mod books {
    use connectors::openlibrary::{BATCH, ID, OpenLibrary};

    use super::{
        App, BOOK, CatalogItem, Ctx, ExternalId, ItemId, KindId, MATCH_ROWS, MatchLog,
        MatchOutcome, MatchRow, authors_of, entered_by_hand, named, pick, row_refresh,
    };

    /// The Open Library work id of an item, if it holds one.
    fn work_id(item: &CatalogItem) -> Option<&str> {
        item.external_ids
            .iter()
            .find(|e| e.namespace == ID && e.value.starts_with("OL") && e.value.ends_with('W'))
            .map(|e| e.value.as_str())
    }

    /// The books among `items` that hold a work id: what [`fill`] asks for in batches.
    pub(super) fn batched(items: &[CatalogItem]) -> Vec<(&CatalogItem, &str)> {
        let books = items.iter().filter(|i| i.kind.as_str() == BOOK);
        books.filter_map(|i| Some((i, work_id(i)?))).collect()
    }

    /// Fill the books among `items` that hold a work id from one search per [`BATCH`] works.
    /// A batch that fails is a line of the second answer, with the reason (review BOOK-7);
    /// its books are left to the refresh jobs.
    pub(super) async fn fill(
        app: &App,
        ol: &OpenLibrary,
        ctx: &Ctx,
        items: &[CatalogItem],
    ) -> anyhow::Result<(Vec<(ItemId, String)>, Vec<String>)> {
        let books = batched(items);
        let (mut filled, mut failed) = (Vec::new(), Vec::new());
        for chunk in books.chunks(BATCH) {
            let ids: Vec<String> = chunk.iter().map(|(_, id)| (*id).to_string()).collect();
            let rows = match ol.works(ctx, &ids).await {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!(error = %e, works = ids.len(), "batch of works failed");
                    let n = ids.len();
                    failed.push(format!(
                        "batch of {n} works failed: {e}; left to the refresh jobs"
                    ));
                    continue;
                }
            };
            for row in rows {
                let Some((item, _)) = chunk.iter().find(|(_, id)| {
                    row.external_ids
                        .iter()
                        .any(|e| e.namespace == ID && e.value == *id)
                }) else {
                    continue;
                };
                let Some(refresh) = row_refresh(&row) else {
                    continue;
                };
                let stored = app.apply_catalog_partial(item.id, refresh).await?;
                filled.push(named(&stored));
            }
        }
        Ok((filled, failed))
    }

    /// See [`super::match_books`].
    pub(super) async fn match_books(
        app: &App,
        ol: &OpenLibrary,
        ctx: &Ctx,
        apply: Option<&dyn MatchLog>,
    ) -> anyhow::Result<Vec<MatchRow>> {
        let kind = KindId::new(BOOK).map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut taken: Vec<String> = Vec::new();
        let mut rows = Vec::new();
        for item in app.items_without_id_from(&kind, ID).await? {
            // a book a person entered by hand is theirs: no source knew it (LC-48)
            if entered_by_hand(&item) {
                continue;
            }
            let outcome = if item.title_unavailable() {
                MatchOutcome::Unmatched("no title".into())
            } else {
                let authors = authors_of(&item.attributes);
                let author = authors.first().map(String::as_str);
                match ol.search_title(ctx, &item.title, MATCH_ROWS).await {
                    Err(e) => MatchOutcome::Unmatched(format!("the source did not answer: {e}")),
                    Ok(found) => match pick(&item.title, item.year, author, &found) {
                        Err(reason) => MatchOutcome::Unmatched(reason),
                        Ok(hit) => take(app, &kind, &item, hit, &mut taken, apply).await?,
                    },
                }
            };
            rows.push(MatchRow {
                item: item.id,
                title: item.title.clone(),
                year: item.year,
                outcome,
            });
        }
        Ok(rows)
    }

    /// Take `hit` for `item` unless its work is held already; with `apply` write the item
    /// as it was into the log, then store the match.
    async fn take(
        app: &App,
        kind: &KindId,
        item: &CatalogItem,
        hit: &catalog_core::Snapshot,
        taken: &mut Vec<String>,
        apply: Option<&dyn MatchLog>,
    ) -> anyhow::Result<MatchOutcome> {
        let Some(id) = hit
            .external_ids
            .iter()
            .find(|e| e.namespace == ID)
            .map(|e| e.value.clone())
        else {
            return Ok(MatchOutcome::Unmatched("the result has no work id".into()));
        };
        let ext = ExternalId::new(ID, id.clone());
        if let Some(other) = app.repos.catalog.by_external_id(kind, &ext).await? {
            return Ok(MatchOutcome::Unmatched(format!(
                "another item holds {id}: {}",
                other.title
            )));
        }
        if taken.contains(&id) {
            return Ok(MatchOutcome::Unmatched(format!(
                "another book of this run matches {id}"
            )));
        }
        taken.push(id.clone());
        if let (Some(log), Some(refresh)) = (apply, row_refresh(hit)) {
            log.matched(item, &id).await?;
            app.apply_catalog_partial(item.id, refresh).await?;
        }
        Ok(MatchOutcome::Matched {
            id,
            title: hit.title.clone(),
            authors: authors_of(&hit.attributes),
            year: hit.year,
        })
    }
}

/// The book source of the two commands.
#[cfg(feature = "openlibrary")]
pub type BookSource = connectors::openlibrary::OpenLibrary;
/// The book source of the two commands (not compiled in).
#[cfg(not(feature = "openlibrary"))]
pub type BookSource = ();

/// `refresh-failed`: give every item that never got its data (state `failed`, or never
/// fetched and in somebody's library) and that a registered source knows (`sources`: their
/// ids) another refresh. Books with an Open Library work id are filled from batch searches
/// first (`books`: the source and its HTTP context); everything else, and what a batch did
/// not answer, gets a refresh job on the background lane. Without `apply` nothing is asked
/// and nothing is written: the report lists what would be done (review BOOK-7).
///
/// # Errors
/// Storage errors, an unknown kind.
pub async fn refresh_failed(
    app: &App,
    sources: &[String],
    books: Option<(&BookSource, &Ctx)>,
    kind: Option<&str>,
    apply: bool,
) -> anyhow::Result<RefreshReport> {
    let kind = kind
        .map(|k| {
            let id = KindId::new(k).map_err(|e| anyhow::anyhow!("{e}"))?;
            app.kind(&id)
                .with_context(|| format!("unknown kind `{k}`"))?;
            Ok::<_, anyhow::Error>(id)
        })
        .transpose()?;
    let items = app.unfetched_items(kind.as_ref()).await?;
    let known = |i: &&CatalogItem| {
        i.external_ids
            .iter()
            .any(|e| sources.contains(&e.namespace))
    };
    let unknown = items.len() - items.iter().filter(known).count();
    #[cfg(feature = "openlibrary")]
    let books = books.filter(|_| sources.iter().any(|s| s == connectors::openlibrary::ID));
    if !apply {
        #[cfg(feature = "openlibrary")]
        let batched: Vec<ItemId> = match books {
            Some(_) => books::batched(&items).iter().map(|(i, _)| i.id).collect(),
            None => Vec::new(),
        };
        #[cfg(not(feature = "openlibrary"))]
        let batched: Vec<ItemId> = {
            let _ = books;
            Vec::new()
        };
        let (filled, queued): (Vec<&CatalogItem>, Vec<&CatalogItem>) = items
            .iter()
            .filter(known)
            .partition(|i| batched.contains(&i.id));
        return Ok(RefreshReport {
            applied: false,
            filled: filled.into_iter().map(named).collect(),
            queued: queued.into_iter().map(named).collect(),
            failed: Vec::new(),
            unknown,
        });
    }
    #[cfg(feature = "openlibrary")]
    let (filled, failed) = match books {
        Some((ol, ctx)) => books::fill(app, ol, ctx, &items).await?,
        None => (Vec::new(), Vec::new()),
    };
    #[cfg(not(feature = "openlibrary"))]
    let (filled, failed) = {
        let _ = books;
        (Vec::new(), Vec::new())
    };
    let queued = app
        .requeue_failed(kind.as_ref(), sources)
        .await?
        .iter()
        .map(named)
        .collect();
    Ok(RefreshReport {
        applied: true,
        filled,
        queued,
        failed,
        unknown,
    })
}

/// `match-books`: for every book without an Open Library id, one title search; a work is
/// taken only when exactly one of the [`MATCH_ROWS`] results has the same normalised title,
/// the year agrees (or the book has none), the book's first author (when it has one) is
/// among the work's, and no other item holds that work. Without `apply` nothing is
/// written. With it the item as it was goes into the log, then the book gets the id and
/// what the search row holds (title, authors, year, cover, page count); nothing is merged
/// and nothing is guessed on a tie.
///
/// # Errors
/// Storage errors. A search that fails is that row's reason, not an error.
#[cfg(feature = "openlibrary")]
pub async fn match_books(
    app: &App,
    ol: &BookSource,
    ctx: &Ctx,
    apply: Option<&dyn MatchLog>,
) -> anyhow::Result<Vec<MatchRow>> {
    books::match_books(app, ol, ctx, apply).await
}

/// Most titles a dry run of `refresh-failed` lists per group.
const DRY_LINES: usize = 20;

/// The lines `cli refresh-failed` prints.
#[must_use]
pub fn refresh_text(r: &RefreshReport) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    if !r.applied {
        for (word, rows) in [("would ask ", &r.filled), ("would queue", &r.queued)] {
            for (_, title) in rows.iter().take(DRY_LINES) {
                let _ = writeln!(out, "{word}  {title}");
            }
            if rows.len() > DRY_LINES {
                let _ = writeln!(out, "             ... and {} more", rows.len() - DRY_LINES);
            }
        }
        let _ = write!(
            out,
            "{} books would be asked from Open Library in batches, {} items would be queued \
             for the worker, {} without an id of a registered source (left as they are). \
             Nothing was asked and nothing was changed; run it again with --apply.",
            r.filled.len(),
            r.queued.len(),
            r.unknown
        );
        return out;
    }
    for (_, title) in &r.filled {
        let _ = writeln!(out, "filled   {title}");
    }
    for (_, title) in &r.queued {
        let _ = writeln!(out, "queued   {title}");
    }
    for line in &r.failed {
        let _ = writeln!(out, "failed   {line}");
    }
    let _ = write!(
        out,
        "{} filled from Open Library (the description follows when the page is opened), {} \
         queued for the worker, {} without an id of a registered source (left as they are)",
        r.filled.len(),
        r.queued.len(),
        r.unknown
    );
    out
}

/// The table `cli match-books` prints.
#[must_use]
pub fn match_text(rows: &[MatchRow], applied: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let mut matched = 0;
    for r in rows {
        let year = r.year.map_or_else(String::new, |y| format!(" ({y})"));
        match &r.outcome {
            MatchOutcome::Matched {
                id,
                title,
                authors,
                year: theirs,
            } => {
                matched += 1;
                let theirs = theirs.map_or_else(String::new, |y| format!(" ({y})"));
                let _ = writeln!(
                    out,
                    "match    {}{year}\n         -> {id}  {title}{theirs}  {}",
                    r.title,
                    authors.join(", ")
                );
            }
            MatchOutcome::Unmatched(reason) => {
                let _ = writeln!(out, "no match {}{year}\n         {reason}", r.title);
            }
        }
    }
    let _ = write!(
        out,
        "{} books without an Open Library id, {matched} with exactly one match. {}",
        rows.len(),
        if applied {
            "The matches were stored."
        } else {
            "Nothing was changed; run it again with --apply to store the matches."
        }
    );
    out
}

/// The Open Library connector as the server registers it (`connectors.openlibrary.base_url`
/// moves it to another host), unless it is not among the registered `sources`.
#[cfg(feature = "openlibrary")]
fn book_source(cfg: &AppConfig, sources: &[String]) -> Option<BookSource> {
    let id = connectors::openlibrary::ID;
    if !sources.iter().any(|s| s == id) {
        return None;
    }
    Some(
        match cfg.connectors.get(id).and_then(|c| c.base_url.as_deref()) {
            Some(base) => BookSource::with_base_urls(base, base),
            None => BookSource::new(),
        },
    )
}

#[cfg(not(feature = "openlibrary"))]
fn book_source(_: &AppConfig, _: &[String]) -> Option<BookSource> {
    None
}

/// The database, the registered sources and the governed client of the two commands.
pub struct Catalog {
    app: App,
    /// Ids of the registered sources that are not switched off.
    sources: Vec<String>,
    http: Arc<dyn HttpFetch>,
    /// Open Library, when it is compiled in and not switched off.
    books: Option<BookSource>,
    /// The log of what `match-books --apply` changed.
    store: store_pg::PgStore,
}

impl Catalog {
    /// Connect with the server's configuration and apply pending migrations.
    ///
    /// # Errors
    /// Unreachable database, an operator contact that is not usable.
    pub async fn open(cfg: &AppConfig) -> anyhow::Result<Self> {
        use auth::admin::AdminStore;
        let pool = store_pg::connect(&cfg.database.url, 2)
            .await
            .context("connect to Postgres")?;
        store_pg::migrate(&pool).await.context("migrations")?;
        let app = crate::escape::shell_app(&pool);
        let (registry, _) = crate::default_registry(cfg);
        // nothing is decrypted here: the settings and the switches are plain columns
        let cipher: Arc<dyn app::ports::SecretCipher> =
            Arc::new(auth::secrets::KeyRing::generate().0);
        let admin = auth::admin_pg::PgAdminStore::new(pool.clone(), cipher);
        for (id, enabled) in admin
            .source_switches()
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?
        {
            registry.set_enabled(&id, enabled);
        }
        let stored = admin.settings().await.map_err(|e| anyhow::anyhow!("{e}"))?;
        // the configuration wins over the value an admin stored, as in the server
        let contact = catalog_core::OperatorContact::default();
        contact
            .set(
                cfg.instance
                    .operator_contact
                    .as_deref()
                    .or(stored.operator_contact.as_deref()),
            )
            .map_err(|e| anyhow::anyhow!("instance.operator_contact: {e}"))?;
        let brand = brand::Brand::load(std::path::Path::new(&cfg.brand.dir))
            .with_context(|| format!("brand directory {}", cfg.brand.dir))?;
        let version = env!("CARGO_PKG_VERSION");
        let mut http = catalog_core::GovernedHttp::new(&brand.user_agent(version), None);
        for info in registry.infos() {
            http.add_connector(&info.id, catalog_core::ConnectorPolicy::from_info(&info));
        }
        // the server may be running: both take their turns at Open Library from one pace
        let http = http
            .with_contact(&format!("{}/{version}", brand.ident()), contact)
            .with_shared_pace(Arc::new(catalog_core::pg::PgPace::new(pool.clone())));
        let sources: Vec<String> = registry
            .infos()
            .into_iter()
            .map(|i| i.id)
            .filter(|id| registry.is_enabled(id))
            .collect();
        Ok(Self {
            app,
            books: book_source(cfg, &sources),
            store: store_pg::PgStore::new(pool),
            sources,
            http: Arc::new(http),
        })
    }

    fn ctx(&self) -> Ctx {
        Ctx::new(self.http.clone(), Lane::BackgroundRefresh)
    }

    /// `cli refresh-failed [--kind <kind>] [--apply]`.
    ///
    /// # Errors
    /// See [`refresh_failed`].
    pub async fn refresh_failed(
        &self,
        kind: Option<&str>,
        apply: bool,
    ) -> anyhow::Result<RefreshReport> {
        let ctx = self.ctx();
        let books = self.books.as_ref().map(|s| (s, &ctx));
        refresh_failed(&self.app, &self.sources, books, kind, apply).await
    }

    /// `cli match-books [--apply]`.
    ///
    /// # Errors
    /// Open Library is not compiled in or switched off; storage errors.
    pub async fn match_books(&self, apply: bool) -> anyhow::Result<Vec<MatchRow>> {
        #[cfg(feature = "openlibrary")]
        {
            let Some(ol) = &self.books else {
                anyhow::bail!("Open Library is switched off (Admin > Data sources)");
            };
            let log = apply.then_some(&self.store as &dyn MatchLog);
            match_books(&self.app, ol, &self.ctx(), log).await
        }
        #[cfg(not(feature = "openlibrary"))]
        {
            let _ = apply;
            anyhow::bail!("this build has no Open Library connector")
        }
    }

    /// `cli match-books --applied`: every match `--apply` ever stored, oldest first, with
    /// the title and the year the book had before.
    ///
    /// # Errors
    /// Storage errors.
    pub async fn applied_matches(&self) -> anyhow::Result<String> {
        use std::fmt::Write;
        let rows = self.store.catalog_matches().await;
        let rows = rows.map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut out = String::new();
        for m in &rows {
            let year = m
                .year_before
                .map_or_else(String::new, |y| format!(" ({y})"));
            let _ = writeln!(
                out,
                "{}  {}  {}{year}\n         -> {} {}",
                m.at.format("%Y-%m-%d %H:%M"),
                m.item_id,
                m.title_before,
                m.source,
                m.external_id
            );
        }
        let _ = write!(
            out,
            "{} matches stored by match-books --apply. The log (table catalog_match_log) \
             holds each item as it was before.",
            rows.len()
        );
        Ok(out)
    }
}

#[cfg(all(test, feature = "openlibrary"))]
mod tests {
    use std::path::Path;

    use app::NewCatalogItem;
    use app::ports::Clock;
    use connectors::openlibrary::FIELDS;
    use domain::catalog::EntityState;
    use testkit::{ReplayHttp, TestEnv};

    use super::*;

    fn env() -> TestEnv {
        TestEnv::with_kinds(Arc::new(kinds::Registry::with_builtins()))
    }

    async fn stub(env: &TestEnv, title: &str, year: Option<i32>, id: (&str, &str)) -> CatalogItem {
        env.app
            .find_or_create_catalog_item(NewCatalogItem {
                kind: KindId::new(if id.0 == "tmdb" { "movie" } else { BOOK }).unwrap(),
                title: title.into(),
                original_title: None,
                year,
                description: None,
                external_ids: vec![ExternalId::new(id.0, id.1)],
                attributes: serde_json::json!({}),
                cover_url: None,
                popularity: None,
            })
            .await
            .unwrap()
    }

    async fn fail(env: &TestEnv, id: ItemId) {
        let mut item = env.app.catalog_item(id).await.unwrap();
        item.state = EntityState::Failed;
        env.app.repos.catalog.upsert(&item).await.unwrap();
    }

    fn title_search(title: &str) -> String {
        let mut u = url::Url::parse("https://openlibrary.org/search.json").unwrap();
        u.query_pairs_mut()
            .extend_pairs([("title", title), ("fields", FIELDS), ("limit", "20")]);
        u.to_string()
    }

    fn docs(rows: &[(&str, &str, Option<i32>)]) -> String {
        let docs: Vec<serde_json::Value> = rows
            .iter()
            .map(|(key, title, year)| {
                serde_json::json!({
                    "key": format!("/works/{key}"), "title": title, "author_name": ["An Author"],
                    "first_publish_year": year, "cover_i": 77, "edition_count": 3,
                    "number_of_pages_median": 250
                })
            })
            .collect();
        serde_json::json!({ "numFound": docs.len(), "docs": docs }).to_string()
    }

    fn fixtures() -> Arc<ReplayHttp> {
        Arc::new(ReplayHttp::from_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../crates/connectors/fixtures/openlibrary"),
        ))
    }

    /// What `--apply` wrote down, in memory.
    #[derive(Default)]
    struct Log(std::sync::Mutex<Vec<(CatalogItem, String)>>);

    #[async_trait::async_trait]
    impl MatchLog for Log {
        async fn matched(&self, before: &CatalogItem, id: &str) -> anyhow::Result<()> {
            self.0
                .lock()
                .unwrap()
                .push((before.clone(), id.to_string()));
            Ok(())
        }
    }

    #[test]
    fn titles_are_compared_without_case_punctuation_and_a_leading_article() {
        assert_eq!(normalise_title("The Dune"), "dune");
        assert_eq!(normalise_title("  DUNE! "), "dune");
        assert_eq!(normalise_title("Fantastic Mr. Fox"), "fantastic mr fox");
        assert_eq!(
            normalise_title("Die unendliche Geschichte"),
            "unendliche geschichte"
        );
        assert_eq!(normalise_title("A"), "a", "a one-word title keeps its word");
        assert_ne!(normalise_title("Dune Messiah"), normalise_title("Dune"));
    }

    /// Six books without an Open Library id, one that holds a work, and what the source
    /// answers for each title.
    struct Shelf {
        env: TestEnv,
        ctx: Ctx,
        dune: CatalogItem,
        fox: CatalogItem,
        emma: CatalogItem,
        old: CatalogItem,
        none: CatalogItem,
        twin: CatalogItem,
        /// Every item as it was stored, `dune` first.
        before: Vec<CatalogItem>,
    }

    async fn shelf() -> Shelf {
        let env = env();
        let http = fixtures();
        let dune = stub(&env, "Dune.", None, ("google_books", "g1")).await;
        let fox = stub(&env, "Fantastic Mr Fox", None, ("google_books", "g2")).await;
        let emma = stub(&env, "Emma", None, ("google_books", "g3")).await;
        let old = stub(&env, "Ulysses", Some(1922), ("google_books", "g4")).await;
        let none = stub(&env, "Nothing Here", None, ("google_books", "g5")).await;
        let twin = stub(&env, "the dune", None, ("google_books", "g6")).await;
        let held = stub(&env, "Emma (held)", None, ("openlibrary", "OL66W")).await;
        let dunes = [
            ("OL893415W", "Dune", Some(1965)),
            ("OL2W", "Dune Messiah", Some(1969)),
        ];
        // three works called exactly like the book, a fourth far down the answer
        let foxes = [
            ("OL45804W", "Fantastic Mr Fox", Some(1970)),
            ("OL7W", "Fantastic Mr. Fox", Some(2009)),
            ("OL8W", "The Fantastic Mr Fox", Some(1988)),
            ("OL9W", "Fantastic Mr Fox: the play", Some(1991)),
            ("OL10W", "Fantastic Mr Fox and other stories", Some(1995)),
            ("OL11W", "The Making of Fantastic Mr Fox", Some(2009)),
            ("OL12W", "fantastic mr fox", None),
        ];
        for (title, answer) in [
            ("Dune.", docs(&dunes)),
            ("the dune", docs(&dunes[..1])),
            ("Emma", docs(&[("OL66W", "Emma", Some(1815))])),
            ("Ulysses", docs(&[("OL5W", "Ulysses", Some(1999))])),
            ("Nothing Here", docs(&[])),
            ("Fantastic Mr Fox", docs(&foxes)),
        ] {
            http.record("GET", &title_search(title), 200, answer);
        }
        let mut before = Vec::new();
        for i in [&dune, &fox, &emma, &old, &none, &twin, &held] {
            before.push(env.app.catalog_item(i.id).await.unwrap());
        }
        env.jobs.drain();
        Shelf {
            ctx: Ctx::new(http, Lane::BackgroundRefresh),
            env,
            dune,
            fox,
            emma,
            old,
            none,
            twin,
            before,
        }
    }

    /// The cli test of v1-plan.md 3.3: one exact hit is taken, two equal titles are not, a
    /// hit whose id another item holds is not; without `--apply` nothing is written.
    #[tokio::test]
    async fn match_books_takes_one_exact_hit_and_writes_nothing_unless_asked() {
        let s = shelf().await;
        let rows = match_books(&s.env.app, &BookSource::new(), &s.ctx, None)
            .await
            .unwrap();
        let of = |id: ItemId| rows.iter().find(|r| r.item == id).unwrap().outcome.clone();
        let unmatched = |reason: &str| MatchOutcome::Unmatched(reason.into());
        assert_eq!(
            rows.len(),
            6,
            "the book that has an Open Library id is not asked"
        );
        assert_eq!(
            of(s.dune.id),
            MatchOutcome::Matched {
                id: "OL893415W".into(),
                title: "Dune".into(),
                authors: vec!["An Author".into()],
                year: Some(1965),
            }
        );
        // four works with the same title among the rows (the fourth is the seventh row: all
        // rows of the answer are looked at, review BOOK-9)
        assert_eq!(of(s.fox.id), unmatched("4 results with the same title"));
        assert_eq!(
            of(s.emma.id),
            unmatched("another item holds OL66W: Emma (held)")
        );
        assert_eq!(
            of(s.old.id),
            unmatched("the year differs (here 1922, there 1999)")
        );
        assert_eq!(of(s.none.id), unmatched("no result"));
        assert_eq!(
            of(s.twin.id),
            unmatched("another book of this run matches OL893415W")
        );
        // the dry run wrote nothing
        for b in &s.before {
            assert_eq!(&s.env.app.catalog_item(b.id).await.unwrap(), b);
        }
        assert_eq!(s.env.jobs.drain().len(), 0);
        let text = match_text(&rows, false);
        assert!(
            text.contains("-> OL893415W  Dune (1965)  An Author"),
            "{text}"
        );
        assert!(text.contains("6 books without an Open Library id, 1 with exactly one match"));
        assert!(text.contains("Nothing was changed"), "{text}");
    }

    #[tokio::test]
    async fn match_books_stores_the_one_match_when_asked() {
        let s = shelf().await;
        let ol = BookSource::new();
        let log = Log::default();
        let rows = match_books(&s.env.app, &ol, &s.ctx, Some(&log))
            .await
            .unwrap();
        assert_eq!(rows.len(), 6);
        assert!(match_text(&rows, true).contains("The matches were stored."));
        // what was changed is written down: the item as it was, and the id it got
        assert_eq!(
            *log.0.lock().unwrap(),
            [(s.before[0].clone(), "OL893415W".to_string())]
        );
        let stored = s.env.app.catalog_item(s.dune.id).await.unwrap();
        assert!(
            stored
                .external_ids
                .contains(&ExternalId::new("openlibrary", "OL893415W"))
        );
        assert!(
            stored
                .external_ids
                .contains(&ExternalId::new("google_books", "g1")),
            "the earlier id stays"
        );
        assert_eq!(stored.title, "Dune");
        assert_eq!(stored.year, Some(1965));
        assert_eq!(
            stored.attributes["authors"],
            serde_json::json!(["An Author"])
        );
        assert_eq!(stored.attributes["page_count"], 250);
        assert_eq!(
            stored.cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/77-L.jpg?default=false")
        );
        assert_eq!(stored.state, EntityState::Partial);
        assert!(
            stored.needs_refresh(s.env.clock.now()),
            "the description follows"
        );
        for b in &s.before[1..] {
            assert_eq!(&s.env.app.catalog_item(b.id).await.unwrap(), b);
        }
        // a second run no longer asks for the matched book, and its twin stays unmatched
        let again = match_books(&s.env.app, &ol, &s.ctx, Some(&log))
            .await
            .unwrap();
        assert_eq!(again.len(), 5);
        assert!(
            again
                .iter()
                .all(|r| matches!(r.outcome, MatchOutcome::Unmatched(_)))
        );
        assert_eq!(log.0.lock().unwrap().len(), 1);
    }

    /// LC-48 (review MAN-4): a book a person entered by hand (title provenance `user`) is
    /// not a candidate: no source knew it, and a match would turn it into a sourced item.
    #[tokio::test]
    async fn match_books_leaves_out_books_entered_by_hand() {
        let env = env();
        let http = fixtures();
        http.record(
            "GET",
            &title_search("Dune"),
            200,
            docs(&[("OL893415W", "Dune", Some(1965))]),
        );
        let by_hand = env
            .app
            .create_manual_item(
                KindId::new(BOOK).unwrap(),
                app::ItemDetails {
                    title: "Dune".into(),
                    year: None,
                    cover_url: None,
                },
                true,
            )
            .await
            .unwrap();
        let before = env.app.catalog_item(by_hand.id).await.unwrap();
        let ctx = Ctx::new(http, Lane::BackgroundRefresh);
        let log = Log::default();
        let rows = match_books(&env.app, &BookSource::new(), &ctx, Some(&log))
            .await
            .unwrap();
        assert!(rows.is_empty(), "{rows:?}");
        assert!(log.0.lock().unwrap().is_empty());
        assert_eq!(env.app.catalog_item(by_hand.id).await.unwrap(), before);
    }

    /// Review BOOK-9: a book that names its author is matched only to a work of that
    /// author; a work without authors, or a book without one, says nothing against it.
    #[test]
    fn the_first_author_is_compared_when_the_book_has_one() {
        let row = |authors: &[&str]| Snapshot {
            connector: "openlibrary".into(),
            kind: BOOK.into(),
            external_ids: vec![ExternalId::new("openlibrary", "OL1W")],
            title: "Persuasion".into(),
            original_title: None,
            year: None,
            description: None,
            attributes: serde_json::json!({ "authors": authors }),
            images: Vec::new(),
            complete: false,
            popularity: None,
        };
        let austen = [row(&["Jane Austen", "A. Editor"])];
        assert!(pick("Persuasion", None, Some("jane austen"), &austen).is_ok());
        assert!(pick("Persuasion", None, None, &austen).is_ok());
        assert_eq!(
            pick("Persuasion", None, Some("Study Guides Inc"), &austen).unwrap_err(),
            "the author differs (here Study Guides Inc, there Jane Austen, A. Editor)"
        );
        assert!(pick("Persuasion", None, Some("Jane Austen"), &[row(&[])]).is_ok());
    }

    /// Review BOOK-7: without `--apply` nothing is asked and nothing is written, and a
    /// batch that fails is a line of the report with its reason.
    #[tokio::test]
    async fn refresh_failed_is_a_dry_run_unless_asked_and_names_a_failed_batch() {
        let env = env();
        // a source that answers nothing: every request fails
        let http = Arc::new(ReplayHttp::default());
        let ctx = Ctx::new(http.clone(), Lane::BackgroundRefresh);
        let ol = BookSource::new();
        let book = stub(&env, "a book", None, ("openlibrary", "OL82563W")).await;
        let film = stub(&env, "Film", None, ("tmdb", "11")).await;
        let other = stub(&env, "Other", None, ("google_books", "g1")).await;
        for i in [&book, &film, &other] {
            fail(&env, i.id).await;
        }
        env.jobs.drain();
        let sources = ["tmdb".to_string(), "openlibrary".to_string()];

        let dry = refresh_failed(&env.app, &sources, Some((&ol, &ctx)), None, false)
            .await
            .unwrap();
        assert!(!dry.applied);
        assert_eq!(dry.filled, [(book.id, "a book".to_string())]);
        assert_eq!(dry.queued, [(film.id, "Film".to_string())]);
        assert_eq!((dry.unknown, dry.failed.len()), (1, 0));
        assert_eq!(http.requests().len(), 0, "nothing was asked");
        assert_eq!(env.jobs.drain().len(), 0, "nothing was queued");
        for i in [&book, &film, &other] {
            let now = env.app.catalog_item(i.id).await.unwrap();
            assert_eq!(now.state, EntityState::Failed, "{}", i.title);
        }
        let text = refresh_text(&dry);
        assert!(text.contains("would ask   a book"), "{text}");
        assert!(text.contains("would queue  Film"), "{text}");
        assert!(text.contains("1 books would be asked"), "{text}");
        assert!(text.contains("run it again with --apply"), "{text}");

        let r = refresh_failed(&env.app, &sources, Some((&ol, &ctx)), None, true)
            .await
            .unwrap();
        assert!(r.applied);
        assert_eq!(r.filled, []);
        assert_eq!(r.failed.len(), 1, "{:?}", r.failed);
        assert!(r.failed[0].starts_with("batch of 1 works failed: "));
        assert_eq!(r.queued.len(), 2, "the book of the failed batch is queued");
        let text = refresh_text(&r);
        assert!(
            text.contains("failed   batch of 1 works failed: "),
            "{text}"
        );
    }

    #[tokio::test]
    async fn refresh_failed_fills_books_with_one_search_and_queues_the_rest() {
        let env = env();
        let http = fixtures();
        let ctx = Ctx::new(http.clone(), Lane::BackgroundRefresh);
        let ol = BookSource::new();
        // in the order of the recorded batch request
        let potter = stub(&env, "hp 1", None, ("openlibrary", "OL82563W")).await;
        let fox = stub(&env, "fox", None, ("openlibrary", "OL45804W")).await;
        let absent = stub(&env, "not in the index", None, ("openlibrary", "OL893415W")).await;
        let film = stub(&env, "Film", None, ("tmdb", "11")).await;
        let other = stub(&env, "Other", None, ("google_books", "g1")).await;
        for i in [&potter, &fox, &absent, &film, &other] {
            fail(&env, i.id).await;
        }
        env.jobs.drain();
        let sources = ["tmdb".to_string(), "openlibrary".to_string()];

        let r = refresh_failed(&env.app, &sources, Some((&ol, &ctx)), None, true)
            .await
            .unwrap();
        assert_eq!(http.requests().len(), 1, "one search for all three works");
        assert_eq!(r.failed, Vec::<String>::new());
        assert_eq!(
            r.filled,
            [
                (
                    potter.id,
                    "Harry Potter and the Philosopher's Stone".to_string()
                ),
                (fox.id, "Fantastic Mr Fox".to_string()),
            ]
        );
        let mut queued: Vec<ItemId> = r.queued.iter().map(|(id, _)| *id).collect();
        queued.sort_by_key(|i| i.0);
        let mut want = vec![absent.id, film.id];
        want.sort_by_key(|i| i.0);
        assert_eq!(queued, want);
        assert_eq!(r.unknown, 1);
        let stored = env.app.catalog_item(fox.id).await.unwrap();
        assert_eq!(stored.state, EntityState::Partial);
        assert_eq!(
            stored.attributes["authors"],
            serde_json::json!(["Roald Dahl"])
        );
        assert_eq!(stored.attributes["page_count"], 96);
        assert_eq!(stored.year, Some(1970));
        assert!(stored.cover_url.is_some());
        assert_eq!(stored.provenance["attributes.page_count"], "openlibrary");
        assert!(stored.needs_refresh(env.clock.now()));
        let jobs = env.jobs.drain();
        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().all(|j| j.lane == Lane::BackgroundRefresh));
        assert_eq!(
            env.app.catalog_item(other.id).await.unwrap().state,
            EntityState::Failed
        );
        let text = refresh_text(&r);
        assert!(text.contains("filled   Fantastic Mr Fox"), "{text}");
        assert!(text.contains("2 filled from Open Library"), "{text}");
        assert!(
            text.contains("2 queued for the worker, 1 without an id"),
            "{text}"
        );

        // nothing is left to do; an unknown kind is refused
        let again = refresh_failed(&env.app, &sources, Some((&ol, &ctx)), Some("book"), true)
            .await
            .unwrap();
        assert_eq!((again.filled.len(), again.queued.len()), (0, 0));
        assert!(
            refresh_failed(&env.app, &sources, None, Some("nope"), false)
                .await
                .is_err()
        );
    }
}
