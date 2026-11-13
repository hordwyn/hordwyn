//! Open Library (books). Terms, limits and attribution: `docs/connectors/openlibrary.md`.
//!
//! No key. The source asks clients to name the application and a contact in the
//! `User-Agent`; the governed client adds the operator contact for this connector
//! ([`ContactUse::Preferred`]). One item is one work (`OL...W`), whatever the edition: an
//! ISBN finds its edition and returns the edition's work. Covers are addressed by cover id
//! only, never by ISBN (that form is limited to 100 requests in 5 minutes per address).
//! Written from the public API documentation alone (DEC-169).

use async_trait::async_trait;
use catalog_core::{
    Attribution, Auth, Capability, Connector, ConnectorError, ConnectorInfo, ContactUse, Ctx,
    Details, DetailsRequest, ImageKind, ImageRef, RateLimit, Search, SearchRequest, Snapshot,
};
use domain::catalog::ExternalId;
use kinds::builtin::BookAttributes;
use serde::Deserialize;

use crate::util::{get_json, non_empty};

pub const ID: &str = "openlibrary";
/// Namespace of the ISBN-13 an item was found by.
pub const ISBN_NAMESPACE: &str = "isbn";
const DEFAULT_API_BASE: &str = "https://openlibrary.org";
const DEFAULT_COVERS_BASE: &str = "https://covers.openlibrary.org";
const KIND: &str = "book";
/// The fields asked of `search.json` (the answer holds nothing else).
pub const FIELDS: &str =
    "key,title,author_name,first_publish_year,cover_i,edition_count,number_of_pages_median";
/// Rows of a search answer.
const SEARCH_LIMIT: u32 = 20;
/// Works asked with one search ([`OpenLibrary::works`]).
pub const BATCH: usize = 50;
/// Subjects kept of a work.
const MAX_SUBJECTS: usize = 20;

pub struct OpenLibrary {
    info: ConnectorInfo,
    api_base: String,
    covers_base: String,
}

fn host_of(base: &str) -> String {
    url::Url::parse(base)
        .ok()
        .and_then(|u| {
            let host = u.host_str()?.to_string();
            Some(u.port().map_or(host.clone(), |p| format!("{host}:{p}")))
        })
        .unwrap_or_default()
}

impl Default for OpenLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenLibrary {
    #[must_use]
    pub fn new() -> Self {
        let mut c = Self::with_base_urls(DEFAULT_API_BASE, DEFAULT_COVERS_BASE);
        // a stored cover is fetched by the server: covers.openlibrary.org answers with a
        // redirect to archive.org, which redirects to a numbered ia*.us.archive.org host
        c.info.hosts.push("archive.org".into());
        c
    }

    /// Other hosts for the API and the covers (tests, a mirror).
    #[must_use]
    pub fn with_base_urls(api_base: &str, covers_base: &str) -> Self {
        let api_base = api_base.trim_end_matches('/').to_string();
        let covers_base = covers_base.trim_end_matches('/').to_string();
        let mut hosts = vec![host_of(&api_base)];
        let covers_host = host_of(&covers_base);
        if !hosts.contains(&covers_host) {
            hosts.push(covers_host);
        }
        Self {
            info: ConnectorInfo {
                id: ID.into(),
                name: "Open Library".into(),
                kinds: vec![KIND.into()],
                capabilities: vec![Capability::Search, Capability::Details, Capability::Images],
                // the source allows 1 request per second without and 3 with a contact; one
                // household needs no more than the lower rate
                rate_limit: RateLimit {
                    per_second: 1,
                    burst: 1,
                    max_concurrency: 1,
                },
                auth: Auth::None,
                attribution: Attribution {
                    text: "Book data and covers from Open Library.".into(),
                    url: "https://openlibrary.org/".into(),
                    licence: "No licence stated; the Internet Archive asserts no new rights \
                              over the database"
                        .into(),
                },
                hosts,
                cache_ttl_secs: 7 * 24 * 3600,
                contact: ContactUse::Preferred,
            },
            api_base,
            covers_base,
        }
    }

    /// # Errors
    /// A base URL from the configuration that does not parse.
    fn url(&self, path: &str, query: &[(&str, &str)]) -> Result<String, ConnectorError> {
        let mut u = url::Url::parse(&format!("{}{path}", self.api_base))
            .map_err(|e| ConnectorError::Parse(format!("base url: {e}")))?;
        if !query.is_empty() {
            u.query_pairs_mut().extend_pairs(query);
        }
        Ok(u.to_string())
    }

    fn cover(&self, id: i64) -> ImageRef {
        ImageRef {
            kind: ImageKind::Cover,
            // `default=false`: 404 instead of a blank picture when the cover is gone
            url: format!("{}/b/id/{id}-L.jpg?default=false", self.covers_base),
            width: None,
            height: None,
        }
    }

    fn snapshot(&self, doc: Doc) -> Option<Snapshot> {
        let id = work_id(&doc.key)?.to_string();
        let attributes = BookAttributes {
            authors: doc.author_name,
            page_count: doc.number_of_pages_median.filter(|p| *p > 0),
            ..BookAttributes::default()
        };
        Some(Snapshot {
            connector: ID.into(),
            kind: KIND.into(),
            external_ids: vec![ExternalId::new(ID, id)],
            title: doc.title,
            original_title: None,
            year: doc.first_publish_year,
            description: None,
            attributes: serde_json::to_value(attributes).unwrap_or_default(),
            images: doc
                .cover_i
                .filter(|c| *c > 0)
                .map(|c| self.cover(c))
                .into_iter()
                .collect(),
            complete: false,
            // a tie-break between rows only (DEC-611)
            popularity: doc.edition_count.map(f64::from),
        })
    }

    async fn search_docs(
        &self,
        ctx: &Ctx,
        op: &str,
        query: &[(&str, &str)],
    ) -> Result<Vec<Doc>, ConnectorError> {
        let url = self.url("/search.json", query)?;
        let page: SearchPage = get_json(ctx, ID, op, &url, headers()).await?;
        Ok(page.docs)
    }

    /// The search row of one work (authors, year, median page count, cover): the work record
    /// itself names its authors by key only.
    async fn work_doc(&self, ctx: &Ctx, work: &str) -> Result<Option<Doc>, ConnectorError> {
        let q = format!("key:/works/{work}");
        let docs = self
            .search_docs(
                ctx,
                "work_row",
                &[("q", q.as_str()), ("fields", FIELDS), ("limit", "1")],
            )
            .await?;
        Ok(docs.into_iter().find(|d| work_id(&d.key) == Some(work)))
    }

    /// Works whose title field matches `title` (not the full text): the candidates for a
    /// title-only book.
    ///
    /// # Errors
    /// As [`Search::search`].
    pub async fn search_title(
        &self,
        ctx: &Ctx,
        title: &str,
        limit: u32,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        let limit = limit.clamp(1, SEARCH_LIMIT).to_string();
        let docs = self
            .search_docs(
                ctx,
                "search_title",
                &[("title", title), ("fields", FIELDS), ("limit", &limit)],
            )
            .await?;
        Ok(docs.into_iter().filter_map(|d| self.snapshot(d)).collect())
    }

    /// The search rows of up to [`BATCH`] works with one request (title, authors, year,
    /// median page count, cover; no description). The source asks for this instead of one
    /// request per book. Ids that are not work ids are left out; a work the index does not
    /// hold is absent from the answer.
    ///
    /// # Errors
    /// As [`Search::search`].
    pub async fn works(&self, ctx: &Ctx, ids: &[String]) -> Result<Vec<Snapshot>, ConnectorError> {
        let keys: Vec<String> = ids
            .iter()
            .filter(|i| is_olid(i, 'W'))
            .take(BATCH)
            .map(|i| format!("/works/{i}"))
            .collect();
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let q = format!("key:({})", keys.join(" OR "));
        let limit = BATCH.to_string();
        let docs = self
            .search_docs(
                ctx,
                "works",
                &[("q", q.as_str()), ("fields", FIELDS), ("limit", &limit)],
            )
            .await?;
        Ok(docs.into_iter().filter_map(|d| self.snapshot(d)).collect())
    }

    /// The work an ISBN belongs to, as the one search result.
    async fn by_isbn(
        &self,
        ctx: &Ctx,
        typed: &str,
        isbn: String,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        let url = self.url(&format!("/isbn/{typed}.json"), &[])?;
        let edition: Edition = match get_json(ctx, ID, "isbn", &url, headers()).await {
            Ok(e) => e,
            Err(ConnectorError::NotFound) => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let Some(work) = edition.work().map(String::from) else {
            return Ok(Vec::new());
        };
        let mut snapshot = match self.work_doc(ctx, &work).await? {
            Some(doc) => self.snapshot(doc),
            None => None,
        }
        // the index does not hold the work (yet): the edition's title is all there is
        .unwrap_or_else(|| Snapshot {
            connector: ID.into(),
            kind: KIND.into(),
            external_ids: vec![ExternalId::new(ID, work)],
            title: edition.title.clone().unwrap_or_default(),
            original_title: None,
            year: None,
            description: None,
            attributes: serde_json::json!({}),
            images: Vec::new(),
            complete: false,
            popularity: None,
        });
        snapshot
            .external_ids
            .push(ExternalId::new(ISBN_NAMESPACE, isbn.clone()));
        if let Some(a) = snapshot.attributes.as_object_mut() {
            a.insert("isbn13".into(), isbn.into());
        }
        Ok(vec![snapshot])
    }
}

fn headers() -> Vec<(String, String)> {
    vec![("Accept".into(), "application/json".into())]
}

/// Is `id` an Open Library id of the given sort (`W` work, `M` edition): `OL`, digits, the
/// letter? Anything else is never put into a request path.
fn is_olid(id: &str, sort: char) -> bool {
    id.strip_prefix("OL")
        .and_then(|r| r.strip_suffix(sort))
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
}

/// `OL45804W` of `/works/OL45804W`, or of the bare `OL45804W`: the documentation of the
/// search shows a row's key in both forms (review BOOK-13); the recorded rows have the prefix.
fn work_id(key: &str) -> Option<&str> {
    let id = key.strip_prefix("/works/").unwrap_or(key);
    Some(id).filter(|id| is_olid(id, 'W'))
}

/// The ISBN-13 of a query that is an ISBN-10 or ISBN-13 with a valid check digit (hyphens
/// and spaces dropped); `None` for anything else, which is then searched as text.
#[must_use]
pub fn isbn13(query: &str) -> Option<String> {
    let compact = compact_isbn(query)?;
    let digit = |c: char| c.to_digit(10);
    match compact.len() {
        13 => {
            if !(compact.starts_with("978") || compact.starts_with("979")) {
                return None;
            }
            let digits: Vec<u32> = compact.chars().map(digit).collect::<Option<_>>()?;
            (ean_check(&digits[..12]) == digits[12]).then_some(compact)
        }
        10 => {
            let mut sum = 0;
            for (i, c) in compact.chars().enumerate() {
                let v = match c {
                    'X' if i == 9 => 10,
                    c => digit(c)?,
                };
                sum += v * (10 - u32::try_from(i).ok()?);
            }
            if sum % 11 != 0 {
                return None;
            }
            let mut digits = vec![9, 7, 8];
            digits.extend(compact.chars().take(9).filter_map(digit));
            let check = ean_check(&digits);
            let mut out: String = digits
                .iter()
                .filter_map(|d| char::from_digit(*d, 10))
                .collect();
            out.push(char::from_digit(check, 10)?);
            Some(out)
        }
        _ => None,
    }
}

/// The query without hyphens and spaces, when what is left could be an ISBN.
fn compact_isbn(query: &str) -> Option<String> {
    let compact: String = query
        .trim()
        .chars()
        .filter(|c| *c != '-' && *c != ' ')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    (matches!(compact.len(), 10 | 13) && compact.chars().all(|c| c.is_ascii_digit() || c == 'X'))
        .then_some(compact)
}

/// The EAN-13 check digit of the first twelve digits.
fn ean_check(first: &[u32]) -> u32 {
    let sum: u32 = first
        .iter()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { *d } else { d * 3 })
        .sum();
    (10 - sum % 10) % 10
}

/// One row of `search.json`. Every field but the key may be absent: the documentation says
/// the schema "is not guaranteed to be stable".
#[derive(Deserialize)]
struct Doc {
    key: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    author_name: Vec<String>,
    first_publish_year: Option<i32>,
    cover_i: Option<i64>,
    edition_count: Option<u32>,
    number_of_pages_median: Option<u32>,
}

#[derive(Deserialize)]
struct SearchPage {
    #[serde(default)]
    docs: Vec<Doc>,
}

#[derive(Deserialize)]
struct KeyRef {
    key: String,
}

/// `/isbn/{isbn}.json` and `/books/{id}.json`.
#[derive(Deserialize)]
struct Edition {
    title: Option<String>,
    #[serde(default)]
    works: Vec<KeyRef>,
}

impl Edition {
    fn work(&self) -> Option<&str> {
        self.works.first().and_then(|w| work_id(&w.key))
    }
}

/// A work's description: a plain string in some records, `{"type": "/type/text", "value":
/// ...}` in others (both recorded on 2026-11-12).
#[derive(Deserialize)]
#[serde(untagged)]
enum Text {
    Plain(String),
    Typed { value: String },
}

impl Text {
    fn into_string(self) -> String {
        match self {
            Self::Plain(s) | Self::Typed { value: s } => s,
        }
    }
}

/// `/works/{id}.json`.
#[derive(Deserialize)]
struct Work {
    title: String,
    description: Option<Text>,
    #[serde(default)]
    subjects: Vec<String>,
    /// Cover ids; the list can hold `-1` and `null`.
    #[serde(default)]
    covers: Vec<Option<i64>>,
}

fn check_kind(kind: &str) -> Result<(), ConnectorError> {
    if kind == KIND {
        Ok(())
    } else {
        Err(ConnectorError::Unsupported(format!("kind `{kind}`")))
    }
}

#[async_trait]
impl Search for OpenLibrary {
    async fn search(
        &self,
        ctx: &Ctx,
        req: &SearchRequest,
    ) -> Result<Vec<Snapshot>, ConnectorError> {
        check_kind(&req.kind)?;
        if let (Some(isbn), Some(typed)) = (isbn13(&req.query), compact_isbn(&req.query)) {
            return self.by_isbn(ctx, &typed, isbn).await;
        }
        let limit = req.limit.clamp(1, SEARCH_LIMIT).to_string();
        let mut query = vec![("q", req.query.as_str()), ("fields", FIELDS)];
        // "influences but doesn't exclude search results"; a two-letter code
        let lang = req
            .language
            .as_deref()
            .and_then(|l| l.get(..2))
            .filter(|l| l.chars().all(|c| c.is_ascii_lowercase()));
        if let Some(lang) = lang {
            query.push(("lang", lang));
        }
        query.push(("limit", &limit));
        let docs = self.search_docs(ctx, "search", &query).await?;
        Ok(docs.into_iter().filter_map(|d| self.snapshot(d)).collect())
    }
}

#[async_trait]
impl Details for OpenLibrary {
    async fn details(&self, ctx: &Ctx, req: &DetailsRequest) -> Result<Snapshot, ConnectorError> {
        check_kind(&req.kind)?;
        let work_of_edition;
        let work_id: &str = if is_olid(&req.id, 'W') {
            &req.id
        } else if is_olid(&req.id, 'M') {
            let url = self.url(&format!("/books/{}.json", req.id), &[])?;
            let edition: Edition = get_json(ctx, ID, "edition", &url, headers()).await?;
            work_of_edition = edition
                .work()
                .map(String::from)
                .ok_or(ConnectorError::NotFound)?;
            &work_of_edition
        } else {
            return Err(ConnectorError::NotFound);
        };
        let url = self.url(&format!("/works/{work_id}.json"), &[])?;
        let work: Work = get_json(ctx, ID, "details", &url, headers()).await?;
        let doc = self.work_doc(ctx, work_id).await?;
        let own_cover = work.covers.iter().flatten().copied().find(|c| *c > 0);
        let (authors, year, pages, row_cover, editions) = doc.map_or_else(
            || (Vec::new(), None, None, None, None),
            |d| {
                (
                    d.author_name,
                    d.first_publish_year,
                    d.number_of_pages_median.filter(|p| *p > 0),
                    d.cover_i.filter(|c| *c > 0),
                    d.edition_count,
                )
            },
        );
        let attributes = BookAttributes {
            authors,
            page_count: pages,
            subjects: work.subjects.into_iter().take(MAX_SUBJECTS).collect(),
            ..BookAttributes::default()
        };
        Ok(Snapshot {
            connector: ID.into(),
            kind: KIND.into(),
            // the id the item was asked by (a work id, or the edition id of an import)
            external_ids: vec![ExternalId::new(ID, req.id.clone())],
            title: work.title,
            original_title: None,
            year,
            description: non_empty(work.description.map(Text::into_string)),
            attributes: serde_json::to_value(attributes).unwrap_or_default(),
            images: own_cover
                .or(row_cover)
                .map(|c| self.cover(c))
                .into_iter()
                .collect(),
            complete: true,
            popularity: editions.map(f64::from),
        })
    }
}

#[async_trait]
impl catalog_core::Images for OpenLibrary {
    async fn images(
        &self,
        ctx: &Ctx,
        req: &DetailsRequest,
    ) -> Result<Vec<ImageRef>, ConnectorError> {
        Ok(Details::details(self, ctx, req).await?.images)
    }
}

impl Connector for OpenLibrary {
    fn info(&self) -> &ConnectorInfo {
        &self.info
    }

    fn as_search(&self) -> Option<&dyn Search> {
        Some(self)
    }

    fn as_details(&self) -> Option<&dyn Details> {
        Some(self)
    }

    fn as_images(&self) -> Option<&dyn catalog_core::Images> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::work_id;

    /// Review BOOK-13: a search row is kept whichever form its key has.
    #[test]
    fn a_work_key_is_read_with_and_without_its_prefix() {
        assert_eq!(work_id("/works/OL45804W"), Some("OL45804W"));
        assert_eq!(work_id("OL45804W"), Some("OL45804W"));
        assert_eq!(work_id("/books/OL7353617M"), None);
        assert_eq!(work_id("OL7353617M"), None);
        assert_eq!(work_id("/works/../x"), None);
        assert_eq!(work_id(""), None);
    }
}
