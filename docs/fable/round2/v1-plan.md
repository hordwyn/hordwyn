# v1 plan: implementation specs for the remaining packages (2026-11-12)

Status: COMPLETE (a plan; the orchestrator decides the cut). Docs only: no code was changed,
nothing was built, run or fetched for this file. Claims about the code are cited by
`file:line` (working tree of `main`, e34ae74); what was not checked is in section 9.
Paths: `crates/` = `src/rust/crates/`, `apps/` = `src/rust/apps/`, browser tests in
`src/web/tests/`.

Input: the owner's rulings of 2026-11-12 (docs/HANDOFF.md 4.8), `v1-status.md`,
`next-packages.md` (sec0, d1, d2, d5, h1, W3), `docs/research/board-game-sources.md`,
DEC-192 to DEC-198, DEC-420, DEC-530, DEC-531, DEC-608, `docs/connectors/openlibrary.md`,
`attribution-and-connectors.md` (A1 Wikidata and Commons, B1, B2), the owner's answers on
HUB-0 and HUB-7, and the crates `connectors`, `catalog-core`, `kinds`, `importers`, `app`,
`assets`, `web` (palette, widgets, item page).

## 0. Summary and build order

The owner's ruling makes v1 "home instance, six kinds with a source": movies, shows, video
games, podcasts, books, board games. Board games come from Wikidata without BGG and without
the hub; anything that has no source can be entered by hand.

| # | Package | Content | Size | Builds | Migrations |
|---|---|---|---|---|---|
| 0 | v1-ops | in flight (another agent); not specified here | M | yes | its own |
| 1 | sec0 | body limit on server functions, generic webhook error text | S (1 day) | yes | none |
| 2 | v1-polish | the small visible defects, covers by one rule, "What v1 covers" page | S-M (2 days) | yes | none |
| 3 | v1-books | operator contact, Open Library connector, page totals, the kept Ryot book stubs | M (5 days) | yes | none expected |
| 4a | v1-manual | manual item entry for every kind, "Edit details" | M (3 days) | yes | none expected |
| 4b | v1-wikidata | Wikidata connector for board games | M (4 days) | yes | none |
| 5 | v1-acceptance | the owner's run, then tag `v1` | owner, one evening | no | - |

Sum: about 15 working days of one agent after v1-ops, test-first with review. 4a and 4b
are the two halves of "v1-boardgames"; 4a goes first because it makes every kind usable
even where a source finds nothing (the normal case for a hobbyist's shelf: Wikidata knows
about 5,000 games, board-game-sources.md section 6).

Rules that hold for every package (next-packages.md section 0): one building package at a
time; test first, each new test seen red; every string in en and de; ASCII; no AI
attribution; no GPL code read or copied (rule 6; the public API documentation and terms of
a GPL-family service may be read, DEC-169); a connector's `docs/connectors/<id>.md` is
written or updated before its code (KICKOFF section 5); migrations take the next free
number when the package starts (`0033_catalog_popularity.sql` is the last on `main`; v1-ops
may add one); each package ends with `cargo xtask ci` and `cargo xtask e2e` by the
orchestrator. No release load run is asked for: none of the packages changes a measured
read path (the palette gets no new statement).

Decisions recorded with this plan: DEC-640 (covers by one rule, reverses DEC-608's
stored-only default), DEC-641 (books and board games in v1, an instance-level Wikidata
connector before the hub, amends DEC-531 and the cut of v1-status.md), DEC-642 (v1 board
games without Commons images and without "Suggest to Wikidata", defers DEC-194 and
DEC-196; a plan decision the owner may reverse).

---

## 1. sec0 - body limit and webhook errors (SEC-2, SEC-9)

As next-packages.md "sec0", with the places confirmed in the code.

**Scope**

1. SEC-2: requests to server functions get the JSON body limit of the REST router
   (`JSON_BODY_LIMIT`, `crates/api/src/routes.rs:125`; 2 MiB). Today the limit is a layer
   on the REST router only; the pages router mounts `/_fn/{*fn_name}`
   (`apps/server/src/lib.rs:806-811`) without one, so `POST /_fn/sign_in` accepts a body of
   any size without a session.
2. SEC-9: a webhook sender gets a generic text for a storage error. Today
   `IntegrationError::Port(e)` answers 500 with `e.0`, the storage error text
   (`crates/api/src/hooks.rs:48-50`). New: status 500, code `internal`, text "internal
   error"; the detail goes to the log with the request id. Check the neighbouring arm
   `IntegrationError::App(e) => e.into()` the same way: no SQL or path text in the body.

**Files and crates**: `apps/server/src/lib.rs` (the layer on the pages router, before
`guarded`), `crates/api/src/hooks.rs`, `crates/api/src/error.rs` (only if the App arm
leaks), `crates/integrations` (test).

**Tests first**

- `apps/server` test: `POST /_fn/sign_in` with `Content-Length` = limit + 1 answers 413
  and the handler never runs; a body of exactly the limit passes the layer.
- The same with a chunked body and no `Content-Length`: reading stops at the limit and the
  answer is not 2xx. `leptos_axum::handle_server_fns_with_context` takes the raw request,
  so axum's `DefaultBodyLimit` (which acts through extractors) may not apply to it: the
  test decides whether `DefaultBodyLimit` is enough or the layer has to wrap the body
  (`tower_http::limit::RequestBodyLimitLayer`, already allowed if `tower-http` is in the
  tree; confirm).
- `crates/api` test: a webhook whose store fails answers 500 with the generic text and the
  body does not contain the store's message.
- A guard test that the import upload (REST, `IMPORT_BODY_LIMIT`) still takes a file above
  2 MiB.

**Migrations**: none. **Size**: S.

**Risks**: a server function that legitimately posts more than 2 MiB. None is known
(imports upload through REST); grep the `#[server]` functions for byte or list arguments
before choosing the number and name the largest in the DEC.

**Acceptance (owner)**

- `curl -X POST -H 'Content-Type: application/x-www-form-urlencoded' --data-binary @big.bin
  https://<host>/_fn/sign_in` with a 3 MiB file answers 413 at once.
- Sign-in, logging and an import of the Ryot file still work.

---

## 2. v1-polish - visible defects, covers by one rule, "What v1 covers"

**Scope**: v1-status.md section 5 package 3, plus the covers ruling.

| # | Item | Where | Source |
|---|---|---|---|
| P1 | `/stats` marks "Home" as the current section | `crates/web/src/shell.rs:43` | v1-status 2.3 |
| P2 | History year filter uses the person's time zone | `crates/web/src/history_state.rs`, `crates/app/src/activity.rs`, `crates/app/src/time_zone.rs` | G9, C4, DEC-601 |
| P3 | Podcast episode rows say "Listened" / "Mark S1 E1 listened" (de: "Gehoert"); the toast of a video game session reads "Logged: session" not "Logged: Play" | `crates/web/src/item_words.rs`, `episode_list.rs`, `locales/*` | DEC-564, ot2a-progress.md:122 |
| P4 | Palette at 390 px: the key hints of the active row are hidden below `sm`, the title may wrap | `crates/ui/src/command.rs` | ot2d-progress.md:270 |
| P5 | A cover that fails to load shows no alt text under the card's button | `crates/web/src/widgets.rs` (`Cover`), card callers | DEC-586 |
| P6 | The bulk question says "as a rewatch" only where a rewatch run is started; on an In progress show it says "as further views" | `crates/web/src/episode_plan.rs`, `locales/*` | ot2e-progress.md "Noticed" |
| P7 | Covers by one rule (below) | `crates/web/src/palette.rs:258-267` (`thumb`), `crates/ui/src/command.rs:374` | owner ruling 2026-11-12, DEC-640 |
| P8 | Docs page "What v1 covers" | `docs/modules/ROOT/pages/` (new page, linked from the nav and from operations.adoc) | G1 |

**P7, the rule (DEC-640)**: every place that shows a cover uses the variant this instance
has stored when there is one, else the provider's address, loaded by the viewer's browser.
No place refuses the provider's address. That is what `widgets::Cover` already does for
the item page, Library and Home cards (`crates/api/src/dto.rs:78-85` hands out the
provider URL while no asset is stored); the palette row is the one place that does not
(`palette::thumb` returns only stored variants). Change: `thumb` falls back to
`cover.url`. The `img` keeps `loading="lazy"`, `decoding="async"`,
`referrerpolicy="no-referrer"` and `alt=""`; the kind icon stays under the picture and
shows while it loads or when it fails. Recent items in the idle list get the same leading
slot (v1-status 2.3, "no cover box"). The second defect of that line (a row does not get
its cover when it is stored while the palette is open) loses its effect: the row shows the
provider's picture from the start.

What the rule does not cover: Wikimedia Commons files (not used in v1, DEC-642) and a
cover address typed by a member (v1-manual; loaded by the browser, never by the server).

Follow-ups written down, not built here: the operations page gets one paragraph "Which
hosts a browser contacts for pictures" (the image hosts of TMDB, TheTVDB, IGDB, Open
Library and archive.org, podcast hosts) and how `assets.fetch_policy = eager` reduces it;
package sec1 must write its CSP with `img-src 'self' https: data:` (one rule means no host
list).

**P8, the page**: which kinds have a source (after this plan: six), which do not (anime,
manga, comics, audiobooks, music, visual novels) and that those are added by hand
(v1-manual) or by a file in our own format; that the export carries no plays (G2), so the
backup is the database dump of the runbook; that show hours need a series runtime (G3);
that a list named "Owned" stands in for the owned flag (section 5.3). v1-books, v1-manual
and v1-wikidata each update their lines; the page is final with package 4b.

**Tests first**

- `shell` unit test: the section of `/stats` is Stats (P1).
- `history_state` / `app::activity` test: a play on 2025-12-31 23:30 in `Europe/Berlin`
  (22:30 UTC) is in year 2025 for a Berlin user and a play on 2026-01-01 00:30 Berlin time
  (2025-12-31 23:30 UTC) is in 2026 (P2). Seen red on today's code.
- `item_words` tests for the podcast labels and the session toast in en and de (P3); the
  xtask locale parity test guards the pairs.
- `palette::tests::thumb_falls_back_to_the_provider_address` and
  `thumb_prefers_the_smallest_stored_variant` (P7); `ui/tests/wasm.rs`: a command row
  whose picture fails keeps the icon and shows no text (P5 shares the mechanism).
- Playwright `shell.spec.ts` "palette: ... rows lead with a cover box": extended, a search
  result that is in nobody's library shows an `img` whose `src` is the source's address;
  at 390 px the active row's title is fully visible (P4).
- Playwright `rewatch.spec.ts`: the bulk question's text on an In progress show (P6).

**Migrations**: none. **Size**: S-M (P2 is the only item with logic).

**Risks**

- P7 sends up to 20 picture requests per typing pause to a provider's image host. Lazy
  loading limits it to the visible rows (about eight); the picture is whatever size the
  connector put into `cover_url` (not a thumbnail), so a palette on a slow line loads
  more bytes than it shows. Accepted for v1; a per-connector thumbnail address is a later
  item.
- P2 touches a query the History page uses on every visit; the change is the bounds of
  the year, not a new statement. No load run.
- P3 changes texts the owner approved for shows; the podcast wording is new (not in
  ux-owner-test.md): the DEC of the package names it for the owner.

**Acceptance (owner)**

- Type a title that is not in your library into the palette: the rows show covers.
- On `/stats` the top bar highlights nothing (it used to highlight Home; Stats is not one
  of its five sections). The account menu, opened there, shows Stats as the current entry
  (heavier, tinted); on the phone the tab bar marks You. (Rewritten after review POL-1; the
  line read "`/stats` highlights Stats in the top bar". Deviation from the plan as first
  written: LC-1, LC-10 in v1-polish-decisions.md.)
- A podcast's episode list says "Listened"; logging a video game session shows a toast
  that does not say "Play".
- History filtered to a year shows what you logged late on New Year's Eve in that year.
- On the phone the palette's active row is readable.
- The docs have a page "What v1 covers".

---

## 3. v1-books - Open Library for books

Moves d5 forward, without DNB (later; its MARC parsing is a package of its own).

### 3.1 Scope

1. **Operator contact** (attribution-and-connectors.md B1; needed by this package and by
   4b): an instance setting `instance.operator_contact`, an e-mail address or an https
   URL, at most 200 characters, no control characters (it goes into a header). Set on
   Admin > Data sources (one field with the sentence "Sent to Open Library and Wikidata
   with every request so they can reach the operator; it is your contact, not a
   member's"), seeded from the environment (`KK_OPERATOR_CONTACT` in
   `config/.env.example`). Stored like `instance.hosting`
   (`crates/api/src/admin/mod.rs:84,425,464`). The governed HTTP client builds one
   User-Agent at start (`apps/server/src/lib.rs:623`, `brand.user_agent`), so: a connector
   declares `contact: ContactUse` in its `ConnectorInfo` (`None`, `Preferred`,
   `Required`), `GovernedHttp` holds a shared handle to the contact (as it does for
   `hosting`, `crates/catalog-core/src/http.rs:250`) and sets
   `User-Agent: <ident>/<version> (<contact>)` on the requests of connectors that are not
   `None`. TMDB, TheTVDB, IGDB and Podcast Index keep today's header and never get the
   contact. A `Required` connector without a contact is not asked; its state on the
   sources page and in the palette footer is the new word `needs_contact` ("operator
   contact missing", with the Set up action for admins,
   `crates/web/src/source_hints.rs`). Open Library is `Preferred` (it answers without, at
   the anonymous rate); Wikidata is `Required` (without a contact Wikimedia allows 10
   requests per minute and may answer 403).
2. **Connector `openlibrary`** (cargo feature `openlibrary`, part of `all`), kind `book`,
   capabilities search, details, images. Registered in `default_registry`
   (`apps/server/src/lib.rs:176-215`); no key. `kinds::builtin::Book` already names it
   first (`crates/kinds/src/builtin/read.rs:45`).
   - Search: `GET /search.json?q=<q>&fields=key,title,author_name,first_publish_year,
     cover_i,edition_count,number_of_pages_median&lang=<xx>&limit=20`. Snapshot per doc:
     external id `("openlibrary", "OL82563W")`, title, `year` = `first_publish_year`,
     attributes `authors`, `page_count` = `number_of_pages_median`, cover
     `https://covers.openlibrary.org/b/id/<cover_i>-L.jpg?default=false` (only when
     `cover_i` is present and positive), `popularity` = `edition_count` (a tie-break only,
     DEC-611), `complete: false`.
   - ISBN: a query that is an ISBN-10 or ISBN-13 with a valid check digit (hyphens and
     spaces dropped) asks `GET /isbn/<isbn>.json` instead, follows `works[0].key` and
     returns that work as the single result, with the extra external id
     `("isbn", "<isbn13>")` and attribute `isbn13`. The item stays work-level (one item
     per book, whatever the edition); the edition's own page count is not kept (see
     3.1.3 for the person's correction).
   - Details: accepts a work id (`OL...W`) or an edition id (`OL...M`, resolved through
     `/books/<id>.json` to its work). `GET /works/<id>.json` for `description` (a string
     or an object with `value`: both), `subjects` (first 20), `covers` (first positive
     id); then one `search.json?q=key:/works/<id>&fields=...&limit=1` for authors, year,
     median page count and `cover_i` (author names are not in the work record; this
     avoids one request per author). If the `key:` query is not supported when the
     fixtures are recorded: author requests `/authors/<id>.json`, at most three per book.
   - Policy: 1 request per second, burst 1, one at a time, with or without a contact (the
     identified tier allows 3 per second; one household does not need it and the lower
     rate is right in both cases). Cache 7 days. Hosts `openlibrary.org`,
     `covers.openlibrary.org`, `archive.org` (the cover redirect chain,
     openlibrary.md "Hosts"). Covers by cover id only, never by ISBN.
   - Attribution: "Book data and covers from Open Library." linking
     https://openlibrary.org/ ; licence text "No licence stated; the Internet Archive
     asserts no new rights over the database". The item page footer and the sources page
     show it through the existing mechanism (`crates/api/src/ops.rs:344-346`). The admin
     sources page adds the sentence that a high-traffic commercial service must not rely
     on this source.
   - `docs/connectors/openlibrary.md`: status "implemented", the endpoints as built, the
     `key:` query and the type of `description` as recorded, the date.
3. **Page totals and "pages read" (OT-17)**:
   - The item page already hands the total to the page field
     (`crates/app/src/next_step.rs:370`, `page_count`); with the connector it is filled.
   - "Finished" on a book with a known total logs the page position `page = page_count`
     and then sets Completed, so finishing counts the remaining pages (G16). Without a
     total it behaves as today and the stats footnote says "books finished without a page
     count add no pages".
   - The person can correct the total: "of 302 pages (estimate) - Change" next to the
     page field; saving writes `attributes.page_count` with provenance `user`. It is a
     change to the shared catalogue item: any member may do it in home mode, admins only
     in hosted mode.
   - A refresh keeps what a person set: `App::apply_catalog_refresh`
     (`crates/app/src/catalog.rs:123-159`) replaces the attributes as a whole today; new
     rule: a field whose provenance is `user` keeps its value and its provenance.
     v1-manual extends the same rule to title, year and cover address.
4. **The kept Ryot book stubs.** A Ryot row of kind book became a title-only item with
   its `openlibrary` or `google_books` id and a refresh nobody could answer
   (`crates/importers/src/legacy.rs:127-128`, `runner.rs:471-503`). Such a stub is now in
   state `failed` (`App::mark_catalog_refresh_failed`, `crates/app/src/catalog.rs:173`)
   and a failed item is never refreshed again (`CatalogItem::needs_refresh`,
   `crates/domain/src/catalog.rs:103`). So the new connector alone changes nothing for
   them. Two steps, both `cli` commands (the image has `cli` after v1-ops), both listed
   in the upgrade note:
   - `cli refresh-failed [--kind book]`: every item in state `failed` that has an
     external id in the namespace of a registered connector goes back to `stub` and gets
     a refresh job on the background lane. General, not only for books (see section 7,
     B1). For Open Library the job is batched, because its policy says "not hundreds of
     single-book requests (use search.json for batch results)": the command asks
     `search.json?q=key:(/works/A OR /works/B ...)&fields=...&limit=50` for up to 50 ids
     per request and stores title, authors, year, cover and page count; description and
     subjects follow when the item's page is opened (the normal refresh). 300 books are
     six requests.
   - `cli match-books [--apply]`: for book items without an `openlibrary` id (the
     `google_books` rows): one `search.json?title=<title>&limit=5` per item at the
     connector's rate; a candidate is taken only when exactly one result has the same
     normalised title (case, punctuation and leading article folded), the year agrees or
     the stub has none, and no other item holds that Open Library id. Without `--apply`
     it prints a table (stub, candidate, authors, year, reason when none); with it the
     id is added to the stub and the stub is refreshed. Nothing is merged and nothing is
     guessed on a tie: what is left stays a title-only book the person can still track,
     and gets a cover address and a page count by hand through "Edit details" (4a).
   - First step of the package, before any code: count the owner's rows
     (`SELECT e.namespace, i.state, count(*) ...` for kind book) and look at ten
     identifiers of each namespace: work ids (`...W`) or edition ids (`...M`)? The
     details call takes both; the count decides whether `match-books` is worth building
     (with fewer than about ten `google_books` rows: skip it, the owner uses "Edit
     details" or adds the book again).

**Audiobooks: not in this package.** Why: (a) the audiobook kind is tracked by position
against a total length (`ProgressModel::Timed`, `duration_seconds`); Open Library's
documented fields have no length and no narrator, so the connector could give an
audiobook a title and a cover but not the one number its tracking and its stats need;
(b) the same work would be found under both kinds and the catalogue matches external ids
per kind, so one Open Library id would end up on two items; (c) editions by ISBN are the
honest route to an audiobook edition (openlibrary.md: "audiobook editions by ISBN
later"), and edition-level items are exactly what this package leaves out to stay
work-level. In v1 an audiobook is added by hand (4a). A source for lengths (MusicBrainz
release groups, d6) is after v1.

### 3.2 Files and crates

`crates/connectors/src/openlibrary.rs` (new), `crates/connectors/Cargo.toml` (feature),
`crates/connectors/fixtures/openlibrary/` (new), `crates/catalog-core` (`ConnectorInfo`
`contact`, `GovernedHttp` contact handle, `ConnectorPolicy`), `apps/server/src/lib.rs`
(registry, contact handle), `crates/api/src/admin/mod.rs` and the admin sources page in
`crates/web/src/admin/` (the setting), `crates/web/src/source_hints.rs` (state
`needs_contact`), `crates/app/src/catalog.rs` (the `user` provenance rule, set page
count), `crates/api/src/routes.rs` plus a server function (set page count),
`crates/web/src/log_sheet.rs` (`PagesInline`: Finished with a total, the Change control),
`crates/web/src/time_tiles.rs` or `stats_extras.rs` (the footnote), `apps/cli/src/main.rs`
(`refresh-failed`, `match-books`), `config/.env.example`, `locales/*`,
`docs/connectors/openlibrary.md`, `docs/modules/ROOT/pages/operations.adoc` (upgrade
note), the "What v1 covers" page.

### 3.3 Tests to write first

- `connectors` (fixtures; synthetic from the documented examples until
  `cargo xtask record openlibrary` has run, then recorded, trimmed, with URL and date):
  `search_maps_title_authors_year_pages_and_cover`,
  `a_doc_without_cover_i_has_no_image`, `an_isbn_query_asks_the_isbn_endpoint_and_returns
  _its_work` (ISBN-10, ISBN-13, hyphens; an invalid check digit is a title search),
  `details_reads_a_description_that_is_a_string_or_an_object`,
  `details_takes_an_edition_id`, `negative_cover_ids_are_skipped`,
  `a_404_is_not_found`, `every_request_goes_to_an_allowed_host`.
- `catalog-core`: `a_connector_that_wants_the_contact_sends_it_in_the_user_agent`,
  `other_connectors_never_send_the_contact`,
  `a_required_contact_that_is_missing_blocks_the_call_without_a_request`,
  `a_contact_with_a_line_break_is_refused`.
- `app`: `a_refresh_keeps_a_page_count_a_person_set`,
  `a_refresh_replaces_a_page_count_from_the_source`,
  `refresh_failed_requeues_only_items_a_registered_source_knows`.
- `web` / `log_sheet`: Finished with a total logs the last page; without one it does not.
- `cli` test for `match-books`: one exact hit is taken, two equal titles are not, a hit
  whose id another item holds is not; without `--apply` nothing is written.
- `kinds/tests/builtins.rs`: book still names `openlibrary` first (exists; keep).
- Playwright `books.spec.ts` against the mock server the other connector specs use: with
  the source on, search "Dune" in the Read scope, add it, the item page shows cover,
  authors and "of N pages"; log page 100, then Finished; Stats shows the pages tile with
  N; the footer credits Open Library. `sources.spec.ts`: the contact field saves and the
  sources page shows Open Library as working.

### 3.4 Migrations

None expected: instance settings are key and value, provenance is a map on the item,
`failed` to `stub` is a data change made by the command. If the settings store turns out
to be a typed table: one migration for the contact column.

### 3.5 Risks

- `number_of_pages_median` and the `key:` query are not in the documented example; the
  docs say the schema "is not guaranteed to be stable". Both are read defensively (a
  missing field is no page count, not an error) and re-checked when the fixtures are
  recorded.
- Whether a URL-only contact gets Open Library's identified tier is unverified; with
  1 request per second it does not matter.
- Cover redirects leave `covers.openlibrary.org` for numbered `ia*.us.archive.org`
  hosts. Today the client follows redirects without checking the allowlist (v1-status
  SEC-4), so storing covers works; package sec2 (redirect confinement) must allow the
  `archive.org` suffix for this connector or stored covers stop. Written into the
  connector doc. The browser path (DEC-640) is not affected.
- The page-count correction changes a shared item. Fine for a household; the hosted-mode
  rule (admins only) is the guard.
- The search fills the catalogue with every result row (as for the other sources); Open
  Library has many near-duplicate works per title. The palette's order (source order,
  DEC-613) keeps the best-known first; nothing new to do.
- A search sends what the person types to openlibrary.org, as for every source.

### 3.6 Acceptance (owner)

- Admin > Data sources shows the operator contact field and Open Library as working.
- Palette, Read scope: a book title finds books with covers; an ISBN from a book on the
  shelf finds that book.
- The item page shows cover, authors and "of N pages"; Change sets another total and it
  is still there the next day.
- Log a page, then Finished: Stats shows pages read for the month.
- After `cli refresh-failed --kind book` the books of the Ryot import have covers and
  authors (those with an Open Library id); `cli match-books` lists the rest with a
  candidate or a reason.
- The item page of a book names Open Library with a link.

---

## 4. v1-boardgames

Two packages. Common decisions:

- **No BGG** (DEC-530): no connector, no panel. The BGG id is an identifier only: it comes
  from Wikidata (P2339) as the external id `("bgg", "<n>")` and as `attributes.bgg_id`,
  and is used for an outbound link and for import matching later.
- **No hub** (DEC-641): the Wikidata rules of DEC-192 (CC0 seed on demand, User-Agent
  from the brand, serial requests, maxlag, backoff) are carried out by an instance-level
  connector now. The hub (h1) later takes over the same connector code as its seed
  client; d2 generalises the property map to the other kinds.
- **Images: none from Wikimedia Commons in v1** (DEC-642). Why: a Commons file needs its
  own licence and author read from `imageinfo`/`extmetadata`, stored with the asset and
  shown as a credit line next to the picture (DEC-194; a new asset column, a second API,
  a credit line in every card context: about a week); Commons advises against
  hotlinking, so the one-rule ruling for covers does not carry over; and what it would
  buy is small: 607 of 4,748 board game items have an image, and it is a photo of a game
  in play, not the box (board-game-sources.md section 5). A board game shows the
  placeholder with its title, or the cover address a member typed (4a). DEC-194 stays
  the rule for the day Commons images are built.
- **"Suggest to Wikidata"** (DEC-196) is not in v1 (DEC-642).
- **"Mark owned"** (DEC-420) is not in v1: see 4.3.

### 4a. v1-manual - manual item entry for every kind

**Scope**

1. Create: `POST /api/v1/items` with `kind`, `title`, optional `year`, optional
   `cover_url`; answers 201 with the item. A server function for the web app calls the
   same use-case (`App::create_manual_item`). Any signed-in member may create (as an
   import row of our own format already can, `crates/importers/src/lib.rs:309-315`).
   - Checks: the kind is enabled; title 1 to 500 characters after trimming (the limit of
     `check_untrusted`, `crates/app/src/catalog.rs:187`); year absent or 1 to the current
     year plus 5; `cover_url` absent or an `https` URL up to 2,000 characters without
     user info. In hosted mode `cover_url` is refused for members (another member's
     browser would load an address a member chose); admins may set it.
   - The item: no external ids, state `full`, provenance `user` for `title`, `year` and
     `cover_url`, attributes empty.
   - The server never fetches a member's cover address: the asset job
     (`apps/server/src/handlers.rs:50-70`) skips a cover whose provenance is `user`
     (today it would register it under an empty connector name and fail). The browser
     loads it under the rule of DEC-640, with `referrerpolicy="no-referrer"`.
   - An item without external ids is never queued for a refresh: `App::detail`
     (`crates/app/src/catalog.rs:74-97`) skips the enqueue when no registered source
     could answer. Today such an item would be queued on every view and marked failed
     every hour.
   - The new item is in the catalogue search index at once (the palette lists it under
     "In your instance", `crates/web/src/palette.rs:393`, a group that exists for
     hand-entered items).
2. Where a person finds it: the palette. Under the results of every search, and on the
   no-results card, one row "Add "<query>" by hand...". It opens a centred dialog (the
   log dialog's shell, OT-10) with Title (the query), Kind (the scope's kind when the
   scope has one; else a select of the enabled kinds), Year, Cover address (optional,
   with the sentence "The picture is loaded from this address by every browser that
   shows it"), and the buttons "Add to library" and Cancel. Before saving, items of the
   same kind with the same normalised title are listed ("Already here: ... Use this
   one"), so a second "Catan" is a choice, not an accident. Saving creates the item, adds
   it to the person's library with the kind's default status and opens its page.
3. Edit: the item page's menu gets "Edit details" for items that no source answers for
   (no external id in a registered connector's namespace: hand-entered items, title-only
   import stubs, the unmatched Ryot books of 3.1.4). Same dialog; `PATCH
   /api/v1/items/{id}` with title, year, cover address; fields become provenance `user`.
   Home mode: any member; hosted mode: admins. For kinds with a page total the "Change"
   control of v1-books is the way to set it; other attributes (players, playing time,
   length) are not editable in v1.
4. `App::apply_catalog_refresh` keeps `user` values of title, year and cover address
   (extends the rule of 3.1.3), so an item that later gets a source id keeps what a
   person typed.
5. The generic export and import keep kind, title and year of a hand-entered item
   (`crates/importers/src/generic.rs:55-56`); the cover address is added as an optional
   column `cover_url` if the format has none (confirm; additive).
6. Required work from the review of v1-books (docs/reviews/2026-11-13-v1-books-review.md,
   BOOK-10): the rule "a field whose provenance is `user` keeps its value and its
   provenance" holds in one of four writers today (`App::store_refresh`: a full refresh and
   a partial fill). v1-books marks only `attributes.page_count` of a book or comic as
   `user`, and those kinds have no cache-limited source, so nothing fails yet. It fails as
   soon as this package lets a member set the title, year or cover of an item a
   cache-limited source answers for. Before "Edit details" ships, with a failing test
   first for each:
   - Retention purge (`crates/store-pg/src/repos/catalog.rs` `purge_provider_data`,
     DEC-453, DEC-466): it sets `attributes = '{}'`, `provenance = '{}'` and clears
     title, year, description and cover. It has to keep every field whose provenance is
     `user`, and those provenance entries, or the next refetch cannot know the field was a
     person's (one `jsonb` expression in the UPDATE, or the purge done in the application
     through the same `kept()` rule). Test: `crates/catalog-core/tests/retention.rs` with a
     `user` title, a `user` cover address and a `user` page total; after the purge and the
     refetch all three are the person's.
   - Merge writer (`crates/app/src/catalog.rs` `update_existing` in mode `Merge`, reached
     by `upsert_catalog_item`): it replaces attributes and title. Route it through
     `store_refresh`, or apply `kept()` there. Test in `crates/app/tests/books.rs` beside
     `a_refresh_keeps_every_field_a_person_set`.
   - Cover job (`apps/server/src/handlers.rs`, the asset job takes its connector from
     `provenance["cover_url"]`): `user` is no connector, the governed client would answer
     `Blocked`. Skip a cover whose provenance is `user` (DEC-640: never fetched by the
     server); this is the same change as the bullet "The server never fetches a member's
     cover address" of scope 1 and B5 of section 7, named here so it is not built twice.
   - The lost update between a person's save and a refresh: `set_item_page_count` and
     `store_refresh` read the item, change it and write the whole row; a refresh that read
     the item just before a person saved writes the old attributes and provenance over it
     (a window of milliseconds). "Edit details" widens the window to every field: write
     the `user` fields with a statement that touches only them, or re-read inside the
     transaction of the refresh.
   - Decide once for all editable fields (review BOOK-19): an audit entry and an undo for a
     change of the shared item (v1-books changes the page total without either), and
     exclude items whose title provenance is `user` from `cli match-books` (review
     BOOK-9), or a hand-made book is offered to the title match.

**Files and crates**: `crates/app/src/catalog.rs`, `crates/api/src/routes.rs`,
`crates/api/src/ops.rs`, `crates/contracts`, `crates/web/src/palette.rs`,
`crates/web/src/item.rs`, a new `crates/web/src/manual_item.rs` (the dialog),
`crates/web/src/api.rs` and `rest.rs`, `apps/server/src/handlers.rs`,
`crates/importers/src/generic.rs`, `locales/*`, the OpenAPI snapshot, the docs page.

**Tests first**

- `app`: `a_member_creates_an_item_by_hand` (state full, provenance user, no ids),
  `a_blank_or_too_long_title_is_refused`, `a_cover_address_must_be_https`,
  `a_disabled_kind_is_refused`, `hosted_mode_refuses_a_members_cover_address`,
  `an_item_without_ids_is_not_queued_for_a_refresh`,
  `a_refresh_keeps_title_year_and_cover_a_person_set`,
  `edit_details_is_refused_for_an_item_a_source_answers_for`.
- `apps/server`: `the_asset_job_never_fetches_a_cover_a_member_typed` (the mock HTTP
  port sees no request).
- `api`: HTTP-level tests of `POST /items` and `PATCH /items/{id}` (201, 400 with the
  field, 401 without a session, 403 in hosted mode for a member's cover address).
- `web`: `palette::tests::the_add_by_hand_row_is_last_and_carries_the_query`; the dialog
  pre-selects the scope's kind.
- Playwright `manual.spec.ts`: search a title no source knows, "Add ... by hand" as a
  board game with a year, the item page opens with the title box and the play button;
  log a play with two people; search again: the item is under "In your instance"; a
  second attempt with the same title shows "Already here"; "Edit details" sets a cover
  address and the card shows the picture. Keyboard only once (dialog focus, Escape).
- Importer: a generic row with `cover_url` round-trips.

**Migrations**: none expected. **Size**: M (3 days).

**Risks**

- A hand-entered item is visible to every member of the instance in the palette ("In
  your instance"). That is today's behaviour for import stubs; in hosted mode it shows
  one member's titles to others. Home v1: accepted; the docs page says so.
- A cover address is a third-party host every viewer's browser contacts. Home mode:
  accepted under DEC-640; hosted mode: admins only.
- Duplicates when a source later knows the title (a hand-entered "Catan" and Wikidata's):
  no merge of two items exists and none is built here. The "Already here" list works in
  one direction only (before a hand entry); the reverse case leaves two items and the
  person removes one. Merging items is a later package.
- No "delete item": a hand-entered item nobody has in a library stays in the catalogue
  and in the search. A cleanup of unreferenced `user` items can ride with the retention
  job later.

**Acceptance (owner)**

- Search for something no source has (a small-press game, a zine): the last row offers
  to add it by hand; fill in kind, year and a picture address; it is in the library and
  its page opens.
- Log a play (board game), a page (book), a position (audiobook) on such items; the
  stats count them.
- Search the title again: it is found; adding it by hand again warns.
- "Edit details" changes title, year and picture; a title-only book from the Ryot import
  can be given a picture the same way.

### 4b. v1-wikidata - Wikidata connector for board games

**Scope**

1. `docs/connectors/wikidata.md` first: licence (CC0 for structured data), terms pages
   and the date they were re-read, User-Agent policy, the rate limits of 2026 (200
   requests per minute with a compliant User-Agent, 10 without, at most 3 concurrent),
   etiquette, endpoints, properties, the class list with its date, "not used: SPARQL,
   dumps, Commons, any write". MediaWiki and Wikibase are GPL software: only their public
   API documentation and policy pages are read (DEC-169), no code.
2. Connector `wikidata` (cargo feature `wikidata`, part of `all`), kind `board_game`
   only, capabilities search and details. `contact: Required` (3.1.1). Host
   `www.wikidata.org` only. Policy: 2 requests per second, burst 2, one at a time
   (serial, as API:Etiquette asks; 120 per minute at most, under the 200); `maxlag=5` on
   background-lane calls only; `Retry-After` on 429 and 503 is honoured by the governed
   client (`crates/catalog-core/src/http.rs:188`); cache 24 hours.
   `kinds::builtin::BoardGame::default_connectors` changes from `bgg` to `wikidata`
   (`crates/kinds/src/builtin/play.rs:139`; `crates/kinds/tests/builtins.rs:71` first).
3. Search, two requests in series:
   - `GET /w/api.php?action=query&list=search&srnamespace=0&srlimit=20&format=json
     &formatversion=2&srsearch=<q> haswbstatement:P31=<class>|P31=<class>...` - the full
     text search over labels and aliases of every language, limited to items that are an
     instance of one of the board game classes. The class list is a constant in the
     connector: Q131436 (board game), Q60474521 (board game expansion) and the subclasses
     in use, produced once at development time by one manual query (`?c wdt:P279*
     wd:Q131436` with the number of instances per class), written with its date into the
     connector doc. A constant, because resolving the class tree per search would need
     SPARQL.
   - `GET /w/api.php?action=wbgetentities&ids=Q1|Q2|...&props=labels|descriptions|claims
     |sitelinks&languages=<lang>|en|mul&format=json` for the hits (up to 20 of the 50
     ids a call allows). Each entity is mapped as below; the search results are therefore
     already complete except for the labels of linked items.
   - Fallback, decided when the fixtures are recorded: the keyword itself works on
     Wikidata (round 1 counted with `list=search` and `haswbstatement:<P=Q>` on
     2026-11-09, open-data-sources.md lines 11 and 444 on `wip/fable-reviews`); not yet
     seen are its use next to free text and several classes joined by `|`. If that does
     not work as written:
     `wbsearchentities` (label prefix search in the user's language, then English, 20
     hits each) and the same `wbgetentities` call, keeping only entities whose P31 is in
     the class list. Its weakness is recall for short titles ("Root", "Azul": the game is
     not among the first 20 things of that name), which is why it is the fallback.
4. Details: `wbgetentities` for the one id, then one more `wbgetentities` for the labels
   of the linked items (genre, mechanics; up to 50 ids, `props=labels`).
5. Mapping (statements: preferred rank, else the first of normal rank; deprecated rank
   and "unknown value" are skipped):

   | Ours | Wikidata |
   |---|---|
   | external id `wikidata` | the QID |
   | title | label in the user's language, else `en`, else `mul`, else any |
   | original title | the `en` label when it differs from the title |
   | description | the item's description in the user's language, else `en` (a short phrase, CC0) |
   | year | P577, the earliest date |
   | `players_min`, `players_max` | P1872, P1873 |
   | `playtime_min_minutes`, `playtime_max_minutes` | P2047: amount in minutes (unit Q7727) or hours times 60 (Q25235); lower and upper bound when the statement has them, else both the amount; two statements give min and max; any other unit is skipped |
   | `min_age` | P2899 (years) |
   | `mechanics`, `categories` | labels of P4151 and P136 (details only) |
   | `bgg_id`, external id `bgg` | P2339, when it is a number; an identifier, nothing is fetched from BGG |
   | external ids `gtin` | P3962, up to five |
   | popularity | the number of sitelinks (a tie-break, DEC-611) |
   | `weight`, `players_best` | none: BGG community figures, not in Wikidata |
   | image | none (DEC-642); P18 is not read |

   Designers, artists and publishers (P287, P170, P12969, P110, P123) are not stored:
   the kind models people as catalogue relations (`play.rs:92-93`) and there is no such
   relation store yet. An expansion (P31 Q60474521 or a P8646 statement) is an item of
   its own and is never merged with its base (DEC-195); the relation is not stored in v1.
6. Attribution: "Data from Wikidata (CC0)." linking https://www.wikidata.org/ on the item
   page footer and the sources page. The item page shows the outbound links "Wikidata"
   (`https://www.wikidata.org/wiki/<QID>`, which is also the "way to report issues in the
   data" the data access page asks for) and "BoardGameGeek"
   (`https://boardgamegeek.com/boardgame/<id>`, a plain link).
7. Player count and playing time on the item page: the facts row already renders a
   kind's facets generically (`crates/web/src/item.rs:1061`); add "2-4 players",
   "60-90 min" and "from 10 years" as words (en, de) instead of four raw numbers.
8. The sources page and the palette footer show "Board games: operator contact missing"
   until the contact of 3.1.1 is set; the Home nudge for admins names it.

**Files and crates**: `docs/connectors/wikidata.md` (new),
`crates/connectors/src/wikidata.rs` (new; or a `wikidata/` module with `classes.rs`),
`crates/connectors/Cargo.toml`, `crates/connectors/fixtures/wikidata/`,
`crates/kinds/src/builtin/play.rs`, `apps/server/src/lib.rs` (registry),
`crates/web/src/item.rs` and `item_words.rs` (facts in words, links),
`crates/web/src/source_hints.rs`, `locales/*`, `docs/connectors/bgg.md` (one line: the
id now arrives from Wikidata), the docs page.

**Tests first**

- `connectors`, from synthetic entity JSON written by hand in the documented shape (CC0
  facts of Catan Q17271; trimmed), replaced by recorded responses once the contact is
  set: `an_entity_maps_players_time_age_year_and_ids`,
  `the_label_falls_back_from_the_user_language_to_english`,
  `a_preferred_statement_wins_and_a_deprecated_one_is_skipped`,
  `playing_time_reads_minutes_hours_and_bounds`, `an_unknown_unit_gives_no_time`,
  `a_bgg_id_that_is_not_a_number_is_dropped`, `p18_is_never_read` (the snapshot has no
  image whatever the entity holds), `search_sends_the_class_filter_and_asks_the_hits_in
  _one_batch`, `an_empty_search_asks_no_entities`,
  `background_calls_carry_maxlag_and_interactive_ones_do_not`,
  `every_request_goes_to_www_wikidata_org`.
- `catalog-core`: the `Required` contact tests of 3.3 cover this connector; one registry
  test that `wikidata` answers for `board_game`.
- `kinds`: the default connector of `board_game` is `wikidata`.
- `web`: `item_words` tests for "2-4 players", "2 players", "60 min", "60-90 min" in en
  and de.
- Playwright `boardgames.spec.ts` against the mock: with the contact set, search "Catan"
  in the Play scope, add it, the page shows players, time, year, the Wikidata credit and
  the two links and no picture request to a Wikimedia host; log a play with scores; Stats
  shows the play and the hours. Without the contact: the footer says why and the "by
  hand" row still works.

**Migrations**: none. **Size**: M (4 days), one more if the fallback search is needed.

**Risks**

- The class-filtered search (free text plus `haswbstatement` with several classes) is
  the open technical point; the fallback is specified above.
- Coverage: about 4,700 board game items, players on about 2,200, time on about 800,
  German labels on about a third (board-game-sources.md 2.1). Many searches will find
  nothing or a row with a title and a year only; that is the source, not a defect, and
  4a is the answer. The docs page says so in one sentence.
- Wikidata is an open wiki: a vandalised label or number arrives as it is. The values
  are refreshed after seven days. A person cannot correct them in v1: "Edit details" is
  for items without a source and covers title, year and picture only. Accepted;
  per-field edits for sourced items are a later item.
- The class list ages (a new subclass of board game is missed). It is a constant with a
  date; regenerate it with a release.
- Wikimedia's limits are "subject to experimentation and change"; the client backs off
  on 429 and the circuit breaker shows the state on the sources page.
- Every search in the Play scope sends the typed text to wikidata.org with the
  operator's contact in the header: stated next to the contact field.

**Acceptance (owner)**

- With the operator contact set, the palette's Play scope finds "Catan", "Carcassonne",
  "Wingspan" as board games, in German where Wikidata has a German label.
- The item page shows player count and playing time in words, the year, "Data from
  Wikidata (CC0)" and a link to the Wikidata item; no cover, the title box instead.
- A game Wikidata does not have is added by hand in the same palette.
- A play with people and scores is logged on both; Stats shows plays, hours and the win
  rate.
- Admin > Data sources lists Wikidata as working; no BGG entry anywhere.

### 4.3 "Mark owned" (DEC-420): not in v1

It is not small. A library entry is a set of last-writer-wins fields that sync
(`crates/domain/src/library.rs:52-61`); an owned flag is a fifth such field: a migration,
the domain type, the sync payload, both DTO sets, the export and the generic import, a
bulk operation and its bar button, a toggle on the item page, a Library filter, strings in
two languages and their tests. About two to three days, and it touches the files of 4a at
the same time. For v1 a list does the job with code that exists: a list named "Owned"
(the bulk bar has "Add to list"; a list can be filtered and shared). DEC-420 stays as it
is; the flag is the first item after v1 for board games, or comes with h1 as DEC-531 says.
If the owner wants it in v1 after all: a package "v1-owned" of size M after 4b, with one
migration.

---

## 5. v1-acceptance - the owner's run

No build. One evening on the home instance (podman). The commands for upgrade, backup and
restore are the ones of the runbook v1-ops writes (operations.adoc); this list names what
to check, not the commands. Tick each line; anything that fails goes back as a finding
before the tag.

**A. Upgrade and first run**

- [ ] Back up first: database dump and the secrets key, by the runbook.
- [ ] Pull or build the new image; `compose up`; healthy within 60 seconds (OWN-8).
- [ ] The migrations ran (log line); the app opens without a hard reload showing the old
      bundle (OPS-8).
- [ ] `cli reindex` from the image finishes (the ot1b upgrade note).
- [ ] The secrets key is at its new place outside the blob directory; the admin overview
      shows the banner until you confirm the key is backed up; after confirming it is
      gone (OPS-3+4 a).
- [ ] Admin > Data sources: set the operator contact; TMDB, IGDB, Podcast Index, Open
      Library and Wikidata show as working.
- [ ] `cli refresh-failed`, then `cli match-books` and, if the table looks right,
      `cli match-books --apply` (section 3.1.4).

**B. Proxy**

- [ ] Sign in through the reverse proxy from another device, by the "Behind a reverse
      proxy" page (`PUBLIC_URL`, `TRUSTED_PROXIES`, `SECURE_COOKIES`).
- [ ] Live updates arrive through the proxy: log something on the phone, the open page on
      the desktop changes without a reload (SSE).
- [ ] The session cookie is `Secure`; the security headers of the proxy page are there
      (browser dev tools, any page).
- [ ] A 3 MiB post to `/_fn/sign_in` is refused (sec0).

**C. Backup and restore**

- [ ] Restore the dump and the key into an empty database and data directory on a second
      compose project, in the runbook's order; run the runbook's check.
- [ ] Sign in there; an item with a stored cover shows it; an integration or notification
      target that holds a secret still works (the key fits).
- [ ] `cli reindex --clear` there; the palette finds a library title.

**D. The six kinds: add, log, stats** (each row: find and add through the palette, log,
then look at Stats and History)

| Kind | Add | Log | Stats shows |
|---|---|---|---|
| Movie | a film by title; cover in the palette row | Watched, once more as a rewatch | count and hours |
| Show | a series; pick the right one of two with the same name | three episodes, a bulk mark of a season, one rewatch run | episodes, hours where the source gives a runtime, else "runtimes not known yet" |
| Video game | a game | a session with duration, two people, a winner | plays, hours, win rate |
| Podcast | a podcast | two episodes (rows say "Listened") | episodes |
| Book | by title and by ISBN | a page, then Finished | pages read |
| Board game | one from Wikidata, one by hand | a play with three people and scores; a co-op play without a result | plays, hours, win rate |

- [ ] All six rows done.
- [ ] One item of another kind (an audiobook or a comic) added by hand and logged once.
- [ ] The Ryot import's books: those with an Open Library id have covers and authors.
- [ ] The Ryot and Trakt imports of the first test still import into a fresh account
      (preview, run, no row errors beyond the known ones).

**E. Around**

- [ ] Phone (390 px): palette, log dialog, a play, Stats.
- [ ] German: the same pages once; no English leftovers in the new texts.
- [ ] A second member of the household: sign-up or invite, their own library, a play
      shared with a linked account.
- [ ] The docs: "What v1 covers", the proxy page, the backup runbook read as you did
      them.
- [ ] Then: tag `v1`.

---

## 6. Order, dependencies, what may run alongside

- sec0 and v1-polish do not depend on each other; sec0 first because it is a day and
  touches `apps/server`, which v1-ops also touches (start it when v1-ops is merged).
- v1-polish before v1-books: the palette's cover rule (P7) is what makes book results
  show covers, and the docs page exists for the later packages to update.
- v1-books before 4a and 4b: it brings the operator contact (4b needs it) and the `user`
  provenance rule (4a extends it).
- 4a before 4b: both touch the palette and the item page; 4a is the one every kind
  needs.
- Docs-only work that may run next to a building package: `docs/connectors/wikidata.md`
  with the class list (needs one manual query and a re-read of the terms pages), the
  "What v1 covers" page text, the count of the owner's Ryot book rows (3.1.4).

## 7. What else blocks or endangers a home v1 (not in v1-status.md)

| # | Finding | Evidence | Where it is handled |
|---|---|---|---|
| B1 | A stub whose first refresh fails is `failed` for good: nothing ever refreshes it again. It hits every item imported while its source had no key or was down, not only books: an import run before the TMDB key was set leaves title-only films forever | `crates/app/src/catalog.rs:173-175`, `crates/domain/src/catalog.rs:103` | `cli refresh-failed` (v1-books 3.1.4); a general command, in the upgrade note and the acceptance run. Blocking for books (the owner's Ryot books would stay bare) |
| B2 | An item without external ids is queued for a refresh on every view and marked failed every hour | `crates/app/src/catalog.rs:74-97`, `crates/catalog-core/src/refresh.rs:81-84` | v1-manual scope 1. Today it only hits stubs of our own import format; with manual entry it would hit every hand-entered item |
| B3 | A refresh replaces an item's attributes as a whole: nothing a person corrects survives seven days | `crates/app/src/catalog.rs:130-136` | the `user` provenance rule (3.1.3, 4a.4). Blocking for "the user can always overwrite the page count" |
| B4 | The app sends no contact in its User-Agent unless the brand sets `support_url`; Wikimedia then treats the client as unidentified (10 requests per minute, possibly 403) | `crates/brand/src/lib.rs:133-137`; attribution-and-connectors.md B1 | operator contact (3.1.1). Blocking for board games from Wikidata |
| B5 | The asset job would try to fetch a cover address under an empty connector name | `apps/server/src/handlers.rs:52-57` | v1-manual scope 1 (never fetch a member's address) |
| B6 | Podcasts were never owner-tested and the source gives an episode count only, no list | v1-status section 1 | the acceptance run row "Podcast"; if the episode list is unusable, that is a finding before the tag, not a known-good kind |
| B7 | With board games in v1 the export's missing plays (G2) weighs more: plays are most of what a board game tracker holds. The export is not a backup | DEC-602 | not built for v1; the docs page and the runbook say "the dump is the backup"; plays in the export stay first after v1 |
| B8 | Books imported from Ryot carry a status only: no finish dates, no pages. With page totals known they still add nothing to "pages read" | `crates/importers/src/legacy.rs:270-274` | not built; the docs page says so. Owner question Q-C below |
| B9 | sec1's CSP must allow pictures from any https host (DEC-640) and sec2's redirect confinement must allow archive.org for Open Library covers | this plan 2 (P7), 3.5 | notes for sec1 and sec2, after v1 |

Not blocking, noticed: `kinds/tests/builtins.rs:71` pins `bgg` as the board game source
(changed in 4b); the Ryot importer treats lots other than anime, book, movie, show and
video game as row errors (`legacy.rs:111-117`), unchanged.

## 8. Owner questions

None blocks a package; each has a default that is built unless the owner says otherwise.

| # | Question | Default |
|---|---|---|
| Q-A | Board game pictures: v1 shows no pictures from Wikimedia Commons (few exist, they are photos of play, each needs its own credit line); a game shows its title box or a picture address you type. Good enough for v1? | Yes: none in v1 (DEC-642); Commons images by DEC-194 after v1 |
| Q-B | "Mark owned" is two to three days and not small. A list named "Owned" in v1, the flag right after? | Yes: list in v1 |
| Q-C | Books that the Ryot import set to Completed have no date. Should they count their pages in "pages read" (on the import day, or not at all)? | Not at all; only what is logged counts |
| Q-D | The operator contact (your e-mail address or a URL) is sent to Open Library and Wikidata with every request. Wikidata does not answer usefully without one. Which value? | Asked on Admin > Data sources; until set, books work at the slower rate and board games are by hand only |
| Q-E | Open Library asks applications to register their use (a form). Do it? | Not done; nothing depends on it |
| Q-F | The podcast wording "Listened" / "Gehoert" for episode rows is new text | As proposed |

## 9. Not verified

- Nothing was built, run or fetched. No request was sent to Open Library or Wikidata for
  this plan; every statement about their APIs comes from `docs/connectors/openlibrary.md`,
  `docs/research/board-game-sources.md` and `attribution-and-connectors.md` (read
  2026-11-06 and 2026-11-10). To confirm when the fixtures are recorded: the `key:` query
  of Open Library's search and a query for several keys at once; the type of a work's
  `description`; `haswbstatement` next to free text and with several classes on Wikidata
  (the keyword alone was used on 2026-11-09); the class list under Q131436; that the
  Action API's `list=search` counts against the same limits.
- Which identifier form the owner's Ryot export holds for Open Library (work or edition)
  and how many `google_books` rows there are: the first step of v1-books.
- Whether axum's `DefaultBodyLimit` applies to `leptos_axum::handle_server_fns_with_
  context`, and whether `tower-http` is in the dependency tree (sec0's first test).
- Whether instance settings are a key and value store (no migration for the contact) and
  whether the generic format has a cover column: both read from one pattern each, not
  from the schema.
- Whether a hand-entered item reaches the search index through the same path as an
  import stub (expected; the Playwright test of 4a shows it).
- The worked file lists are where the work is expected from the crate layout; a
  package's first step is to confirm them. Sizes are estimates in the scale of
  next-packages.md.
- v1-ops is being built by another agent in the same checkout; what its runbook and its
  DEC entries (618 to 639) say was not read. Section 5 refers to it by content only.
- The hub design (`hub-design.md`) and `open-data-sources.md` on branch
  `wip/fable-reviews` were searched for the Wikidata search and seed parts only and
  otherwise used through next-packages.md, the owner's answers and
  attribution-and-connectors.md B1: nothing of the hub is in v1.

## Addendum 2026-11-12: v1 is internet-facing (owner ruling, HANDOFF 4.8)

The owner will host v1 for themselves, reachable from the internet; there is no existing
instance to upgrade. Build order after v1-ops therefore becomes:

1. sec0 (as specified above)
2. sec1 - security headers and CSP (SEC-1, next-packages.md "sec1"); the CSP must allow the
   provider image hosts used for covers (DEC-640), and archive.org redirects for Open Library
   covers (v1-plan B9)
3. sec2 - sign-in throttling and outbound confinement (SEC-5 and the rest of next-packages.md
   "sec2"); per-process counters (one web process is the supported shape, OPS-12); the
   confinement must allow the cover hosts and the connectors' API hosts
   - also from sec0 (DEC-653): a read timeout for request bodies and collecting the body only
     for `/_fn/` (stalled chunked bodies must not pin memory without a session)
4. v1-polish, v1-books, v1-manual, v1-wikidata (as specified above)
5. v1-acceptance on the owner's internet-facing instance (add: TLS at the proxy, headers seen
   from outside, sign-in throttling, a backup and a restore rehearsal)

sec3 (account deletion) and sec4 (session cap) stay after v1 while the owner is the only user;
they become blockers before anyone else gets an account.

## Addendum 2 (2026-11-12): greenfield until v1, release process from v1 on

Owner: nothing is live anywhere; until the `v1` tag that both agree on, everything is
greenfield. From `v1` on every public release tag needs release notes, a changelog, upgrade docs
and preferably a staging environment to test the upgrade against.

Rules until v1:
- No compatibility code or docs for pre-v1 builds or data. Packages do not add upgrade notes for
  pre-v1 states; migrations may be changed freely until the squash.

New last package before the acceptance run, **v1-release** (M):
1. Remove pre-v1 shims: the blob move from the old layout (`storage.fs_legacy_root`, v1-ops
   DEC-619/643) unless a fresh install needs it, the "Coming from a build before 0034" section,
   the one-way note for 0032 and the reindex note for ot1b, the pre-0034 sentences in
   `.env.example`, install.adoc, README.md and compose.yml.
2. Squash all migrations into one baseline `0001_v1_baseline.sql` (OWN-9 a); keep the sqlx
   offline data consistent; a test that a fresh database from the baseline equals the schema
   built from the old chain (compare `pg_dump --schema-only`), run once before the old files go.
3. `CHANGELOG.md` (Keep a Changelog format, cite the URL) with a v1 entry summarising what v1
   covers; a release notes template (`docs/modules/ROOT/pages/releases/`) with sections
   highlights, upgrade steps (routine + release-specific), known issues, rollback.
4. Upgrade docs: the routine procedure in operations.adoc plus a per-release page; a
   "Staging" page: a second compose project on the same host (as the restore rehearsal), a
   production backup restored into it, upgrade staging first, checklist, then production.
5. `cargo xtask upgrade-test <previous-tag>`: builds the previous tag's schema and seed data in
   a local Postgres database (from `git show <tag>:...` migrations or a stored fixture of the
   tag's schema), runs the current migrations and a read-back smoke over the main repos; added
   to CI for tags after v1 (before v1 there is no previous tag: the command skips with a message).
6. Release checklist in docs/HANDOFF.md section 1 (changelog, notes, upgrade-test green, staging
   upgrade done by the owner, tag, push).

Build order: v1-ops -> sec0 -> sec1 -> sec2 -> v1-polish -> v1-books -> v1-manual ->
v1-wikidata -> v1-release -> v1-acceptance (owner, internet-facing instance) -> tag `v1`.

Addendum 3 (2026-11-13, after the rename to hordwyn): v1-release also renames the provider key
variables from the `KK_` prefix (initials of the former name, e.g. `KK_TMDB_API_KEY`) to a
neutral prefix that carries no brand (white-label rule 9), in code, `config/.env.example`, CI
and docs; greenfield rule: no alias for the old names.

Addendum 4 (2026-11-13): v1-release also makes `store_pg::testing` drop the `test_<uuid>`
database a test created when the test ends (also on success; keep it only on failure or with
`KEEP_TEST_DB=1`), so parallel lanes and repeated runs stop leaving thousands of databases
behind (2,971 were dropped by hand on 2026-11-13).
