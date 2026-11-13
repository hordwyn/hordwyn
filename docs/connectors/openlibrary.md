# Connector: Open Library (`openlibrary`) - implemented (v1-books)

Date checked: 2026-11-12 (the requests of "Verified live" below; terms, licence and rate
limits as read on the live pages on 2026-11-10). First record: 2026-11-06.

History: the 2026-11-06 version of this file marked the connector "not implemented: KICKOFF
rule 6 conflict" (DEC-106), because the service software is AGPL-3.0 and its API
documentation had therefore not been read. Owner ruling R-P1 (DEC-169, docs/HANDOFF.md 4.1)
superseded that reading: the public API documentation and terms of a GPL-family service may
be read to write an independent HTTP client; the code stays off-limits and nothing is copied.
Only the public documentation and policy pages listed below were read, and the answers of
the public API. The search documentation links to a Solr schema file in the project's source
repository for the full field list: do not open it.

| Item | Value |
|---|---|
| Kinds | book. Audiobooks are not served (v1-plan.md 3.1: the documented fields hold no length and no narrator) |
| Capabilities | search, details, images |
| Code | `src/rust/crates/connectors/src/openlibrary.rs`, cargo feature `openlibrary` (part of `connectors/all` and `server/all-connectors`); on by default, no key |
| Licence of the service software | AGPL-3.0 (github.com/internetarchive/openlibrary; checked from repository metadata on 2026-11-06, not re-checked) |
| API docs | https://openlibrary.org/developers/api ; search https://openlibrary.org/dev/docs/api/search ; works, editions, ISBN https://openlibrary.org/dev/docs/api/books ; covers https://openlibrary.org/dev/docs/api/covers |
| Terms | "Usage Guidelines" and "Rate Limits" on https://openlibrary.org/developers/api ; licensing https://openlibrary.org/developers/licensing |
| Licence | No licence text. "The Internet Archive does not assert any new copyright or other proprietary rights over any of the material in the Open Library database. There may be existing rights issues on some contributions and in some jurisdictions." Licence tag `unspecified-open`: display freely, never contribute these fields to a CC0 hub. Licence of the cover image files: not stated |
| Attribution | Not required. Covers page: "A courtesy link back to Open Library is appreciated, whether it be on each individual book's page [...] or on your About page or in your footer." We show "Book data and covers from Open Library." with a link to https://openlibrary.org/ on item pages that use it and on the attribution page; licence line "No licence stated; the Internet Archive asserts no new rights over the database" |
| Auth | none |
| User-Agent | "please add a HEADER that specifies a User-Agent string with (a) the name of your application and (b) your contact email or phone number"; example `User-Agent: MyLibraryApp (contact@example.org)`. Ours: `<brand ident>/<version> (<operator contact>)` when the instance setting `instance.operator_contact` is set (Admin > Data sources, or `KK_OPERATOR_CONTACT`), else the plain application header. The connector declares `ContactUse::Preferred`: it is asked with and without a contact. The contact is an e-mail address (preferred: the page names "contact email or phone number" and gives the identified rate for "User-Agent and email") or an https URL; whether a URL alone counts as "identified" is not stated (unverified). The header travels along the cover redirects to the `archive.org` hosts (all of them the Internet Archive's). With web and worker as two processes the worker reads the setting when it starts: after an admin removes the contact, restart the worker, or it keeps sending the old one |
| Rate limits | "Default (non-identified requests): 1 request per second"; "Identified requests (with User-Agent and email): 3 requests per second". Policy as built: 1 request per second, burst 1, one request at a time, with or without a contact (one household does not need the identified tier). The limit is per address and the instance can be several processes (the web role, a separate worker, `cli refresh-failed` and `cli match-books`), each with its own token bucket: every process therefore takes its turn at this source from one pace kept in the database (table `source_pace`, `catalog_core::pg::PgPace`; it applies to every connector without a key), so all of them together send at most one request per second. Covers: "The cover access by ids other than CoverID and OLID are rate-limited. Currently only 100 requests/IP are allowed for every 5 minutes."; on excess "403 Forbidden". The connector addresses covers by cover id only |
| Usage rules | "These APIs are not intended to serve as a bulk data backend or high-traffic commercial infrastructure."; also "they are not intended to serve as a data backend for third-party services". Do: "Make useful, time-sensitive requests on behalf of human users", "Cache responses whenever possible", "Identify your application with a User-Agent header and email". Do not: "Scrape HTML pages", "Distribute requests across multiple IPs", "Make hundreds of single-book requests (use search.json for batch results)", "Use Open Library as a backend for high-traffic commercial services". "Violations may result in aggressive rate limiting or blocking." "Please do not use our APIs to bulk download metadata" (monthly dumps exist). Covers: "Please, do not crawl our cover API."; "intended for displaying covers on public facing websites and not for bulk download". The card of the source on Admin > Data sources says that a high-traffic commercial service must not rely on it. `cli refresh-failed` asks for up to 50 works with one search instead of one request per book. The page also asks applications to register their use case (form linked under "Registering Your Application"): owner decision, not done (v1-plan.md Q-E) |
| Caching | no limit; caching is asked for. Responses are cached for 7 days; covers are stored under the storage budget. No retention rule |
| Hosts | `openlibrary.org`, `covers.openlibrary.org`, `archive.org`, `*.archive.org`. A cover request redirects off the covers host: on 2026-11-10 `covers.openlibrary.org/b/id/8739161-M.jpg?default=false` answered 302 to `archive.org/download/...`, which answered 302 to `ia800703.us.archive.org/view_archive.php?...`, which served the JPEG (the host number varies per item; one sample only). Since package sec2 the server follows a redirect only to a host on the connector's list, checked again at every hop (`crates/catalog-core/src/http.rs`): the connector lists `archive.org` and `*.archive.org` (every subdomain, default port; a bare leading dot `.archive.org` is no syntax since the sec2 review, SEC2-11, and covers nothing), which together cover `archive.org` and every `ia*.us.archive.org` host; without the wildcard entry stored covers stop at the second redirect. A redirect to a host not on the list is refused. The browser path (DEC-640: a page loads the cover from the provider until the instance has stored it) is not affected |

## Endpoints used

| Op | Request |
|---|---|
| search | `GET https://openlibrary.org/search.json?q=<q>&fields=key,title,author_name,first_publish_year,cover_i,edition_count,number_of_pages_median[&lang=<xx>]&limit=<1..20>` (`lang` = two-letter ISO 639-1 code, sent when the request names a language; it "influences but doesn't exclude search results") |
| search by ISBN | a query that is an ISBN-10 or ISBN-13 with a valid check digit (hyphens and spaces dropped): `GET https://openlibrary.org/isbn/<isbn as typed, compact>.json` (the live answer is a 302 to `/books/{OLID}.json`), then the row of the edition's work (next line). A 404 is "no result". The result is the work, with the extra external id `("isbn", "<isbn13>")` and the attribute `isbn13` |
| row of one work | `GET https://openlibrary.org/search.json?q=key:/works/{OLID}&fields=<as above>&limit=1`: authors, first year, median page count, cover id, edition count (the work record names its authors by key only; this avoids one request per author) |
| rows of many works | `GET https://openlibrary.org/search.json?q=key:(/works/A OR /works/B ...)&fields=<as above>&limit=50`, at most 50 works per request (`OpenLibrary::works`, used by `cli refresh-failed`) |
| title search | `GET https://openlibrary.org/search.json?title=<title>&fields=<as above>&limit=20` (`OpenLibrary::search_title`, used by `cli match-books`; the recorded fixture asked for 5) |
| work details | `GET https://openlibrary.org/works/{OLID}.json`, then the row of that work |
| edition | `GET https://openlibrary.org/books/{OLID}.json` when details are asked for an edition id (`OL...M`); its `works[0].key` names the work |
| cover | `https://covers.openlibrary.org/b/id/{cover id}-L.jpg?default=false` (404 instead of a blank image when there is none); never the ISBN form |

Ids are checked before they are put into a path: `OL`, digits, `W` (work) or `M` (edition);
anything else is answered "not found" without a request.

## Fields used

- search `docs[]`: `key` (`/works/OL...W`), `title`, `author_name[]`, `first_publish_year`,
  `cover_i`, `edition_count`, `number_of_pages_median`. Every field but `key` may be absent
  and is then simply not set (recorded: a row without `cover_i`, without
  `first_publish_year`, without `number_of_pages_median`). `number_of_pages_median` is not in
  the documented example; the docs say "The schema is not guaranteed to be stable, but most
  common fields (e.g. title, IA ids, etc) should be safe to depend on."
- edition (`/isbn/{isbn}.json`, `/books/{id}.json`): `title`, `works[].key`. Nothing else
  is kept: the item is the work, whatever the edition.
- work (`/works/{OLID}.json`): `title`, `description`, `subjects` (the first 20), `covers`
  (the first id above 0; the list holds `-1` in recorded answers and is read so that `null`
  is skipped too).

Mapping: one catalogue item is one work. `number_of_pages_median` -> `page_count`, shown as
"of N pages (estimate)"; a person can set another total on the item page ("Change"), which
is stored with provenance `user` and survives every refresh. `edition_count` ->
`popularity` (a tie-break between rows, DEC-611). Cover: the work's first cover id, else the
search row's `cover_i`. An item imported with an edition id keeps that id.

## Verified live (2026-11-12)

Ten requests to `openlibrary.org`, one per 1.3 seconds, sent with a generic client name and
no contact (the anonymous tier), from the build host: the eight below, the `key:` search for
`/works/OL45804W` (fixture `search-key-OL45804W.json`; it showed the same as the one for
`OL82563W`), and, as far as the fixtures tell (the list was not written down at the time),
the edition address `/books/OL7353617M.json` that the ISBN address redirects to. What they
showed:

- `search.json?q=dune&fields=...&lang=en&limit=3`: 200; three docs with exactly the fields
  asked; first `/works/OL893414W`, "Dune", 1965, 155 editions, median 608 pages.
- The `key:` query works: `search.json?q=key:/works/OL82563W&fields=...&limit=1` answered
  one doc (`numFound` 1).
- The multi-key form works: `q=key:(/works/OL82563W OR /works/OL45804W OR /works/OL893415W)`
  with `limit=50` answered the two works the index holds; the third key is simply absent
  (no error).
- The type of `description` is not fixed: `/works/OL45804W.json` has a plain string,
  `/works/OL82563W.json` an object `{"type": "/type/text", "value": "..."}`. Both are read.
- `/works/OL45804W.json`: `covers` holds `-1` among 36 ids.
- `/isbn/9780140328721.json`: one redirect to `/books/OL7353617M.json` ("Fantastic Mr.
  Fox", `works` `[{"key": "/works/OL45804W"}]`).
- `/works/OL1W.json`: 404 with `{"error": "notfound", "key": "/works/OL1W"}`.
- `search.json?title=Fantastic Mr Fox&fields=...&limit=5`: 200, five docs, four of them
  with the same title after normalising (so `cli match-books` leaves that title alone).

Verified on 2026-11-10 (not repeated): the quotes of the table above against
`/developers/api`, `/developers/licensing`, `/dev/docs/api/search`, `/dev/docs/api/books`,
`/dev/docs/api/covers`, and the cover redirect chain.

Not verified: whether a URL-only contact in the User-Agent gets the identified rate; the
licence of cover image files (not stated anywhere read); the current licence of the service
software (last checked 2026-11-06); the full set of archive.org hosts that serve covers;
whether `/isbn/` finds an edition by the ISBN-10 form when the record lists only the
ISBN-13 (the connector asks with the form that was typed); what `/works/{id}.json` answers
for a work that was merged into another (a record without `title` is a parse error, the
item's refresh fails and is tried again later).

## Fixtures

`src/rust/crates/connectors/fixtures/openlibrary/*.json`: 14 files. Ten were recorded on
2026-11-12 (search rows untrimmed, because `fields=` already limits them; works and the
edition trimmed to the fields used, lists cut, as each header says), no images. Four are
marked `synthetic` in their header: the ISBN-10 address (`isbn-0140328726`: the recorded
edition under another URL), a 404 for an ISBN nobody holds (`isbn-9780306406157-404`, in the
shape of the recorded 404), and a work whose covers list starts with `-1` and that the
search index does not hold (`work-OL999001W-synthetic` with its empty search answer
`search-key-OL999001W-synthetic`). Real-world text (subjects in
several languages) is kept as recorded.

`cargo xtask record openlibrary` records the non-synthetic ones again: it needs
`KK_OPERATOR_CONTACT` (sent in the User-Agent of the recording requests only; a value with
a space, a parenthesis or a character outside printable ASCII is refused), sends one
request per 1.2 seconds, trims each answer to the top-level fields its fixture holds and
keeps a list no longer than the fixture has it (a list that was cut stays cut).

The browser tests use `src/web/tests/openlibrary-mock.mjs`, which serves these fixtures and
one generated single-colour picture for every cover (no provider image is kept).
