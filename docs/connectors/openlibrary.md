# Connector: Open Library (`openlibrary`) - not implemented yet (terms recorded)

Date checked: 2026-11-10 (terms, licence, rate limits and sample responses re-read on the live
pages that day, see "Verified live"). First record: 2026-11-06.

History: the 2026-11-06 version of this file marked the connector "not implemented: KICKOFF
rule 6 conflict" (DEC-106), because the service software is AGPL-3.0 and its API
documentation had therefore not been read. Owner ruling R-P1 (DEC-169, docs/HANDOFF.md 4.1)
superseded that reading: the public API documentation and terms of a GPL-family service may
be read to write an independent HTTP client; the code stays off-limits and nothing is copied.
Only the public documentation and policy pages listed below were read. The search
documentation links to a Solr schema file in the project's source repository for the full
field list: do not open it.

| Item | Value |
|---|---|
| Kinds | book (audiobook editions by ISBN later) |
| Capabilities | search, details, images |
| Licence of the service software | AGPL-3.0 (github.com/internetarchive/openlibrary; checked from repository metadata on 2026-11-06, not re-checked) |
| API docs | https://openlibrary.org/developers/api ; search https://openlibrary.org/dev/docs/api/search ; works, editions, ISBN https://openlibrary.org/dev/docs/api/books ; covers https://openlibrary.org/dev/docs/api/covers |
| Terms | "Usage Guidelines" and "Rate Limits" on https://openlibrary.org/developers/api ; licensing https://openlibrary.org/developers/licensing |
| Licence | No licence text. "The Internet Archive does not assert any new copyright or other proprietary rights over any of the material in the Open Library database. There may be existing rights issues on some contributions and in some jurisdictions." Licence tag `unspecified-open`: display freely, never contribute these fields to a CC0 hub. Licence of the cover image files: not stated |
| Attribution | Not required. Covers page: "A courtesy link back to Open Library is appreciated, whether it be on each individual book's page [...] or on your About page or in your footer." We show "Book data and covers from Open Library." with a link to https://openlibrary.org/ on item pages that use it and on the attribution page |
| Auth | none |
| User-Agent | "please add a HEADER that specifies a User-Agent string with (a) the name of your application and (b) your contact email or phone number"; example `User-Agent: MyLibraryApp (contact@example.org)`. Ours: brand ident, version and the operator contact of the instance. Today the app sends a contact only when the brand sets `support_url` (`src/rust/crates/brand/src/lib.rs:133-137`), and that is a URL: whether a URL alone counts as "identified" is not stated (unverified) |
| Rate limits | "Default (non-identified requests): 1 request per second"; "Identified requests (with User-Agent and email): 3 requests per second". Policy: 2/s, burst 2, 2 concurrent when the operator contact is set, else 1/s, burst 1, 1 concurrent. Covers: "The cover access by ids other than CoverID and OLID are rate-limited. Currently only 100 requests/IP are allowed for every 5 minutes."; on excess "403 Forbidden" |
| Usage rules | "These APIs are not intended to serve as a bulk data backend or high-traffic commercial infrastructure."; also "they are not intended to serve as a data backend for third-party services". Do: "Make useful, time-sensitive requests on behalf of human users", "Cache responses whenever possible", "Identify your application with a User-Agent header and email". Do not: "Scrape HTML pages", "Distribute requests across multiple IPs", "Make hundreds of single-book requests (use search.json for batch results)", "Use Open Library as a backend for high-traffic commercial services". "Violations may result in aggressive rate limiting or blocking." "Please do not use our APIs to bulk download metadata" (monthly dumps exist). Covers: "Please, do not crawl our cover API."; "intended for displaying covers on public facing websites and not for bulk download". An operator who runs a high-traffic commercial service must not rely on this connector: say so on the admin page. The page also asks applications to register their use case (form linked under "Registering Your Application"): owner decision, not done |
| Caching | no limit; caching is asked for. Cache responses (TTL 7 days) and store covers under the storage budget. No retention rule |
| Hosts | `openlibrary.org`, `covers.openlibrary.org`. Cover requests redirect off that host: on 2026-11-10 `covers.openlibrary.org/b/id/8739161-M.jpg?default=false` answered 302 to `archive.org/download/...`, which answered 302 to `ia800703.us.archive.org/view_archive.php?...`, which served the JPEG. The allowlist therefore needs `archive.org` and the numbered `ia*.us.archive.org` hosts for covers (the host number varies per item; one sample only), or the browser loads covers directly as for Podcast Index artwork: decide when building |
| Status | planned behind cargo feature `openlibrary` (W5); on by default once built (no key). Depends on the operator-contact setting proposed in `docs/fable/round2/attribution-and-connectors.md` B1 |

## Endpoints used

| Op | Request |
|---|---|
| search | `GET https://openlibrary.org/search.json?q=<q>&fields=key,title,author_name,first_publish_year,cover_i,edition_count,number_of_pages_median&lang=<xx>&limit=20` (`lang` = two-letter ISO 639-1 code; it "influences but doesn't exclude search results") |
| work details | `GET https://openlibrary.org/works/{OLID}.json` |
| edition by ISBN | `GET https://openlibrary.org/isbn/{isbn}.json` (redirects to the edition, `/books/{OLID}.json`) |
| cover | `https://covers.openlibrary.org/b/id/{cover_i}-L.jpg?default=false` (sizes S, M, L; 404 instead of a blank image when there is none); never the ISBN form in a loop (100 per 5 minutes) |

## Fields used

- search `docs[]`: `key` (`/works/OL...W`), `title`, `author_name[]`, `first_publish_year`,
  `cover_i`, `edition_count`, `number_of_pages_median`. The last one is not in the documented
  example; the docs say "The schema is not guaranteed to be stable, but most common fields
  (e.g. title, IA ids, etc) should be safe to depend on."
- edition (`/isbn/{isbn}.json`): `key` (`/books/OL...M`), `title`, `publishers[]`,
  `publish_date` (free text, e.g. "October 1, 1988"), `languages[].key`
  (`/languages/eng`), `number_of_pages`, `isbn_10[]`, `isbn_13[]`, `covers[]` (cover ids;
  can contain `-1`, skip it), `works[].key`, `authors[].key`.
- work (`/works/{OLID}.json`): `key`, `title`, `description`, `subjects`, `covers`,
  `authors`. The type of `description` (plain string or object) was not inspected: check when
  recording.

Mapping: `number_of_pages_median` -> `page_count` of a work-level book item, marked as an
estimate; an edition's `number_of_pages` replaces it when the user picked an edition by
ISBN. The user can always overwrite the page count (stats "pages read", OT-17).

## Verified live (2026-11-10)

Pages opened and compared with the statements above: `/developers/api` (usage guidelines,
rate limits, User-Agent), `/developers/licensing`, `/dev/docs/api/search`,
`/dev/docs/api/books`, `/dev/docs/api/covers`. All quotes above are the pages' wording.

Sample requests (one each):

- `search.json?q=harry+potter+philosopher&fields=...&limit=1`: first doc
  `/works/OL82563W`, "Harry Potter and the Philosopher's Stone", `first_publish_year` 1997,
  `edition_count` 400, `number_of_pages_median` 302, `cover_i` 15155833.
- `/isbn/9780140328721.json`: one redirect to `/books/OL7353617M.json`; "Fantastic Mr. Fox",
  `number_of_pages` 96, `isbn_13` `["9780140328721"]`, `covers` `[15152634, 8739161, -1]`,
  `works` `[{"key": "/works/OL45804W"}]`.
- `/works/OL45804W.json`: top-level keys include `title`, `description`, `subjects`,
  `covers`, `authors`.
- cover by id: redirect chain as described under Hosts.

Not verified: whether a URL-only contact in the User-Agent gets the identified rate; the
licence of cover image files (not stated anywhere read); the current licence of the service
software (last checked 2026-11-06); the full set of archive.org hosts that serve covers.

## Fixtures

`src/rust/crates/connectors/fixtures/openlibrary/*.json`: none yet. To be recorded with
`cargo xtask record openlibrary` (no key needed); small JSON responses only, trimmed to the
fields used, source URL and date in the fixture header; no images. Until recorded, synthetic
fixtures built from the documented examples, marked `synthetic`.
