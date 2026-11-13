# Review: v1-books (lane B, branch lane/v1-books), 2026-11-13

Scope: the uncommitted working tree of <repo>-lane-books against its
base cb2f7a0 (nothing is committed on the branch; 72 changed files, 20 new). Inputs:
v1-plan.md section 3 and 7 (B1, B3, B4), docs/connectors/openlibrary.md, LB-1..13, the
progress log, HANDOFF 2 and 4, agent-briefs/rules.md.

Method and limits: read-only. No cargo, npm or Playwright was run by the reviewer, so every
statement about behaviour is from reading the code; where a finding depends on something
that was not read to the end it says so. The Open Library documentation pages were read
live on 2026-11-13 (/developers/api, /dev/docs/api/search, /dev/docs/api/books,
/dev/docs/api/covers, /developers/licensing). Paths are relative to the worktree; Rust
paths are under src/rust/.

## Summary

| Severity | Count | Findings |
|---|---|---|
| major | 3 | BOOK-1, BOOK-2, BOOK-3 |
| medium | 8 | BOOK-4 .. BOOK-11 |
| low | 9 | BOOK-12 .. BOOK-20 |
| info | 2 | BOOK-21 (merge), BOOK-22 (browser timeouts) |

Must fix before merge: BOOK-1, BOOK-3 (one fix), BOOK-2 (a ruling, not necessarily code),
BOOK-4, BOOK-5. Everything else may be deferred with a recorded decision; BOOK-6 and BOOK-8
are small and worth doing in the same pass.

Load run: not needed before the merge; see "Load-run verdict" at the end.

What is in order (checked, no finding): the contact is validated in one place
(`catalog_core::check_contact`: length, printable ASCII, no space, no parentheses, e-mail or
https URL) on every way in (admin API, configuration at start, the cli's `Catalog::open`);
it is sent only for connectors whose policy is not `ContactUse::None` (`http.rs`
`contact_agent`), TMDB, TheTVDB, IGDB and Podcast Index declare `None`; request headers are
not logged anywhere in `catalog-core/src/http.rs`, the two warnings that concern the contact
log the error and not the value; fixtures hold no address and no User-Agent; the tests reach
Open Library only through `ReplayHttp`, wiremock or the Node mock (the integration tests use
`AppConfig::from_sources(.., Vec::new())`, so no variable of the developer's shell reaches
them, and no worker runs in them); rate limit 1 per second, burst 1, one at a time; covers by
cover id only with `?default=false`; ids are checked before they go into a path; both
migrations are additive; en and de hold the same new keys; no product name and no non-ASCII
character in the new code.

## Open Library: what the live documentation confirms and what it does not

Confirmed word for word on 2026-11-13: "1 request per second" (non-identified), "3 requests
per second" (identified, "with User-Agent and email"); the User-Agent sentence and the
example `MyLibraryApp (contact@example.org)`; the Do and Do-not lists including "Make
hundreds of single-book requests (use search.json for batch results)"; `fields`, `lang`
("influences but doesn't exclude search results") and `limit`; `/works/OL..W.json`,
`/books/OL..M.json`, `/isbn/<isbn>.json` and that the ISBN address redirects to the edition;
the cover pattern `/b/id/<id>-L.jpg`, `?default=false` ("returns a 404 instead"), "100
requests/IP are allowed for every 5 minutes" for ids other than CoverID and OLID, the
courtesy link, "do not crawl our cover API"; the licensing sentence.

Not in the documentation (the connector relies on recorded behaviour only; the lane says so
for most of them, listed here so the owner sees the whole set):

- `number_of_pages_median` (not named on the search page; it is the only source of the page
  total).
- `q=key:/works/..` and the `key:(A OR B ..)` batch form (the page only says "The solr
  query"); details (authors, year, pages) and `cli refresh-failed` depend on it.
- `title=` as a parameter (it appears in one example address only); `cli match-books`
  depends on it.
- The type of a work's `description` (string or object), `covers` holding `-1`, and
  `works[].key` on an edition (example only).
- The redirect of a cover to archive.org hosts.
- A URL as contact: the page names "contact email or phone number" and "email"; nothing says
  a URL identifies a client (BOOK-12).
- The form of `key` in a search row: the documented example shows `"key": "OL27448W"` in one
  place and `"/works/OL166894W"` in another (BOOK-13).

## Findings

### BOOK-1 | major | must fix | The day of a completion set by hand moves whenever the status is written again

Where: `crates/store-pg/src/repos/library.rs:402-414`, `stats_cache.rs:246-259`,
`goals_groups.rs:208-211,229-242` (all read `hlc_time(e.status_hlc)`),
`crates/app/src/stats.rs:174-225`; cause in `crates/domain/src/hlc.rs:67-71` and
`crates/app/src/library.rs:133`.

Problem: LB-13 dates a completion by the wall clock of the entry's LWW status stamp. That
stamp is not "the day the entry became Completed": `Lww::set` takes every write with a newer
stamp, also one that writes the value the entry already has, and `App::set_status` passes
each call through. So any later write of Completed re-dates the completion.

Failure scenarios (each from the code as read):
1. The owner runs the Ryot import a second time (after `match-books`, or after a fix).
   `importers/src/runner.rs:548` calls `set_status(actor, item, s, None)` for every row with
   a status. Every book and show that is Completed by hand gets a new stamp; all of them
   leave the months they were in and appear in the month of the second run. A goal "finish
   12 books in 2026" jumps, the year comparison loses the earlier year.
2. Bulk "set status: Completed" on a selection that contains entries already Completed
   (`app/src/bulk.rs:203-204` stamps each one): the same move, for the whole selection.
3. Choosing Completed again in the status select of one item: its completion moves to today.
4. Bulk remove and Undo: `EntrySnapshot` (`bulk.rs:334-343`) holds no stamp, so a restored
   entry is stamped at the undo (not read to the end: the restore path was not opened; the
   snapshot alone shows the old stamp cannot come back).

Fix: do not derive the day from the stamp. Add a nullable column
`library_entries.completed_at timestamptz` in 0101, maintained by a BEFORE INSERT OR UPDATE
row trigger: set to `hlc_time(NEW.status_hlc)` when the status becomes `completed` from
another status (or on insert as completed), kept unchanged while it stays `completed`, NULL
when it leaves. The three statements and `stats::history` read that column (the domain
entry gets the field) instead of the stamp. The AFTER trigger then compares `completed_at`
instead of `status_hlc`, which also stops marking the cache for rewrites that change
nothing. Add tests (Rust rules and Postgres): Completed written twice keeps the month; an
import run twice keeps the month.

### BOOK-2 | major | must decide before merge (owner ruling) | Every imported Completed entry counts on the import day; this now includes shows

Where: LB-13 (3) "books imported as Completed count on the import day";
`importers/src/runner.rs:546-549`.

Problem: before LB-13 an entry completed by hand counted nowhere (the defect "0 books
finished"). Now each counts once on the day its status was written. For an import that is
the import day, and it is not only books: `finished` is refused for episodic kinds
(DEC-592), so every show the file says is Completed is completed "by hand" too.

Failure scenario: the owner's acceptance run imports 135 rows in November 2026. Home and
Stats show every Completed show and book of the last years as finished in 2026-11; the
year comparison 2026 against 2025 is wrong in both columns; a 2026 goal counted in
completions is met by the import. DEC-594 and DEC-601 both say this measure "needs a
ruling"; LB-13 made the ruling inside the books package.

Fix (choose one, record it as a DEC at merge):
a. Date a completion set by hand by the newest effective event of that entry when one
   exists and is older than the status write, else by the status write. Imported shows carry
   dated episodes and imported books dated page logs, so they land in the month they were
   finished; "Finished" on a book lands today as now. One more correlated subquery per
   by-hand row in the three statements, and the same in `stats::history`.
b. Leave the rule and say it on the import summary and in "What v1 covers" ("titles imported
   as Completed count as finished on the day of the import").
With BOOK-1's column, (a) is a different value written by the trigger or by the importer and
no change to the statements.

### BOOK-3 | major | must fix (same change as BOOK-1) | A rewatch takes the earlier completion out of the history and a stop re-dates it

Where: `crates/app/tests/rewatch.rs:377-385` (expectation changed 0 -> 1),
`crates/app/src/stats.rs:209-221`, the `e.status = 'completed'` condition of the three
statements.

Problem: a completion set by hand exists only while the entry is Completed. "Start a
rewatch" on such a show writes Completed -> In progress (DEC-587). From then on the show has
no completion at all: the month it was completed in loses it. When the run finishes, the
finishing episode is a completing event and the show counts once, at the end of the rewatch
(the first watch is gone for good). When the run is stopped, the stop marker gives Completed
back; a stop is no completing event (DEC-590), so the entry counts by hand again, dated by
the stop marker's stamp. The changed test asserts the number 1 after a stop and not where it
lies.

Failure scenario: a show imported as Completed (counted, say, in 2026-11 by BOOK-2). In
March 2027 the owner starts a rewatch: the 2026 total drops by one and a 2026 goal that was
met reads one short. He stops after three episodes: the completion comes back in 2027-03,
although DEC-590 says a stop completes nothing, and the page says "Watched once; rewatch
stopped after 3 of 24 episodes".

Fix: with the `completed_at` column of BOOK-1, keep it when the move away from Completed is
the start of a rewatch run and when Completed comes back by a stop marker (the application
knows both: `log_progress` writes those moves; it can pass the kept value), and clear it
only for a status written by hand. The statements then count `completed_at IS NOT NULL`
instead of `status = 'completed'` for the by-hand row. Tests: start a rewatch on a show
completed by hand (the completion stays where it was), stop it (still there, same month),
finish it (the event adds a second one, as for a show whose first run has an event).

### BOOK-4 | medium | must fix (one line) | Migration 0101 leaves existing statistics caches without the new completions

Where: `crates/store-pg/migrations/0101_hand_completions.sql`;
`crates/store-pg/src/repos/stats_cache.rs:102-113` (a full rebuild only when the zone stamp
differs).

Problem: the cache (`stats_item_months`, `stats_months`) of a user who has read their
statistics before the migration holds no completion set by hand, and nothing marks those
items: the new trigger fires on later writes only. The goal window
(`pg_window_amounts`) and `month_amounts_full` are not cached and count them at once.

Failure scenario: any database that exists before this migration (the owner's test instance
of 2026-11-10, the dev database, every lane database): Home and Stats keep saying "0 books
finished" for the three Completed books of the screenshot that started LB-13, while a goal
counts them. It heals item by item when an entry is touched, or when the person changes
their time zone. "Greenfield until v1" covers production data, not the databases the
acceptance run uses.

Fix: end 0101 with `UPDATE stats_users SET zone = '';` (the next read of each user rebuilds
in full; this is the mechanism a zone change already uses).

### BOOK-5 | medium | must fix | "Set page total" is offered on kinds that cannot hold one

Where: `crates/web/src/item.rs:739`, `crates/web/src/log_form.rs:419-428` (`page_total`
looks at provenance and `can_edit` only), `crates/web/src/log_sheet.rs` `PageTotalLine`
(the button whenever `can_edit`), `crates/app/src/catalog.rs:590-611`
(`set_item_page_count` checks `ProgressModel::Paged` only),
`crates/kinds/src/builtin/read.rs:67` (a second paged kind without a `page_count`
attribute; LB-6 (4) names manga).

Problem: LB-6 (4) says only kinds whose attributes allow `page_count` take a total. The
page shows the button for every item whose next step is a page, to everybody who may edit.

Failure scenario: a member opens a manga on a home instance, presses "Set page total", types
180, Save. Either the attribute check refuses it and the form shows the raw validation
text, or (if the kind's attribute type ignores unknown fields; not verified, nothing was
run) the server answers 200, the toast says "Page total set to 180." and nothing was
stored. Both are wrong.

Fix: the server decides: `ItemPageDto` gets the answer (for example `can_edit` stays and a
new `page_total_editable`, true when the kind's schema has `page_count`), `PageTotalLine`
shows the button only then, and `set_item_page_count` refuses a kind without the attribute
with the existing "this kind has no pages" error. Test: app test for the manga kind, one
assertion in the SSR test that the manga action bar has no "Set page total".

### BOOK-6 | medium | should fix with BOOK-5 | "Finished" offline logs the page and never completes, without a word

Where: `crates/web/src/next_step.rs:290-299` (`then` runs only in the success branch; the
offline branch queues the log), `crates/web/src/log_sheet.rs` `finish` in `PagesInline`.

Problem: LB-7 states it ("Offline the page log is queued and the entry is not completed").
The person pressed "Finished" and sees "Saved offline, will sync"; after the sync the book
stands at its last page, In progress, and Stats does not count it as finished. Before this
package the status call ran next to the page log and at least showed its own error. The
button also has no busy state: a second press while the page log is under way logs the last
page twice and completes twice (the second log adds no pages; cosmetic).

Fix: in the offline branch of a step that carries `then`, say so (an error line under the
field: "The page is saved. Set the book to Completed when you are online again."), or queue
the status write if the sync command set has one (not checked). Disable Finished while
`busy`.

### BOOK-7 | medium | may defer (decide) | `refresh-failed` has no dry run and reports a failed batch as "0 filled"

Where: `apps/server/src/book_stubs.rs:290-336`, `apps/cli/src/main.rs` (`RefreshFailed`).

Problem: `match-books` writes nothing without `--apply`; `refresh-failed` writes at once:
it stores batch answers on the items (title, authors, year, cover, page count), turns
`failed` into `stub` and queues one job per item of every kind. The plan does not ask for a
dry run here, the review brief does. On a big library the first thing the operator learns
is the list of what was already done. A batch that fails is a `tracing::warn!`
(`book_stubs.rs:174-177`) and the command prints "0 filled from Open Library" with no
reason (whether the cli installs a tracing subscriber was not checked).

It is idempotent as far as read: a filled book has `fetched_at` and is no longer selected;
a queued item is deduplicated by `catalog.refresh:<id>` while its job is queued or running
(`jobs/src/queue.rs:76`); the batch size is capped twice (chunks of 50, `take(BATCH)`).

Fix: `--apply` as in `match-books` (without it: the three counts and the first lines), and
print a failed batch as a line of the report ("batch of 50 works failed: <error>; left to
the refresh jobs").

### BOOK-8 | medium | should fix (runbook line or a slower client) | The commands and the running server each allow themselves 1 request per second

Where: `apps/server/src/book_stubs.rs:489-493` (an own `GovernedHttp` with the connector's
policy), docs/modules/ROOT/pages/operations.adoc "Titles that never got their data",
v1-plan.md 5 (acceptance: `cli refresh-failed`, then `cli match-books`).

Problem: the limit of the source is per address ("1 request per second" without an e-mail).
The cli process and the server's worker do not share a limiter.

Failure scenario: the acceptance order. `cli refresh-failed` queues a refresh job for every
book the batch did not answer and for every edition id; the worker then asks two or three
requests per book at 1 per second. The operator runs `cli match-books` right after, which
asks one title per second from the same address: 2 per second for as long as both run, above
the anonymous limit, with "aggressive rate limiting or blocking" as the documented answer.

Fix: the cli's own policy at one request per two seconds (`RateLimit` is per connector
policy; override it in `Catalog::open`), or one sentence in the runbook: set the operator
contact as an e-mail address first and let the worker finish before `match-books`.

### BOOK-9 | medium | may defer | `match-books`: "exactly one of five" is not "exactly one", and `--apply` has no way back

Where: `apps/server/src/book_stubs.rs:85-109` (`pick`), `:211` (`search_title(.., 5)`),
`:259-261` (apply).

Problem: uniqueness is checked among the first five rows only. A common title has more
works of the same name than five rows show ("Emma", "Dune" as a graphic novel or a
companion book); when exactly one of them is in the first five it is taken. Authors are not
compared (a Ryot stub has none). With `--apply` the row's title, authors, year, cover and
page count replace the stub's and the id is added; there is no command and no page that
removes an external id again. After v1-manual every book made by hand has no Open Library
id either and is offered to the same command.

Failure scenario: a stub "Persuasion" without a year; the first five rows hold one work of
exactly that title that is a study guide; `--apply` turns the owner's book into it.

Mitigation that exists: the dry run is the default and prints id, title, year and authors.

Fix: ask `limit=20` (one request either way) and require uniqueness among those; print
`numFound` beside a match so a crowded title is visible; let `--apply` take ids
(`--apply OL893415W,..`) or skip rows whose match has no year on the stub's side; exclude
items whose title provenance is `user` once v1-manual marks it.

### BOOK-10 | medium | may defer to v1-manual, must be in its brief | The `user` rule holds in one of four writers

Where: LB-5 ("built once"); `crates/app/src/catalog.rs:133-195` (the rule),
`crates/store-pg/src/repos/catalog.rs:236-243` (`purge_provider_data`: `attributes = '{}'`,
`provenance = '{}'`, title, year, description, cover cleared),
`crates/app/src/catalog.rs:415-436` (`update_existing` in mode `Merge` replaces attributes
and title), `apps/server/src/handlers.rs:51-57` (the cover job takes its connector from
`provenance["cover_url"]`; `user` is no connector, the governed client would answer
`Blocked`).

Problem: a refresh and a partial fill keep a field a person set. The retention purge
(DEC-453, DEC-466) and the merge writer do not, and the purge also deletes the provenance
map, so the next refresh cannot know the field was a person's. DEC-466 says "the person's
own data is untouched"; a title or page total a person typed is the person's data.

Today nothing fails: the only field ever marked `user` is `attributes.page_count` of a
book or comic, those kinds have no cache-limited source, and `upsert_catalog_item` has no
caller outside tests as far as grep shows. It fails with v1-manual: a member corrects the
title or the cover of a film from TMDB; after 150 to 175 days without a possible refresh
the purge blanks it, and the refetch brings the source's value back as if nobody had typed
anything.

Also in this area (low, same owner): `set_item_page_count` and `store_refresh` read the
item, change it and write the whole row back; a refresh that read the item just before a
person saved a total writes the old attributes and the old provenance over it. The window
is milliseconds.

Fix (v1-manual, or here if cheap): the purge keeps every field whose provenance is `user`
and those provenance entries (one `jsonb` expression in the UPDATE, or do the purge in the
application through the same `kept()` rule); `Merge` goes through `store_refresh`; the
cover job skips a cover whose provenance is `user` (DEC-640: never fetched by the server).
Tests: `retention.rs` with a `user` field.

### BOOK-11 | medium | may defer | The two new catalogue statements and several LB-13 cases have no Postgres test

Where: `crates/store-pg/src/repos/catalog.rs:374-404` (`unfetched_items`,
`items_without_namespace`); `crates/store-pg/tests/phase1.rs:1195-1266`.

Problem: `refresh_failed` and `match_books` are tested against the in-memory store only
(`app/tests/books.rs`, `book_stubs::tests`); the SQL that selects the items is checked by
the sqlx macro for types, not for meaning (the `EXISTS` on `library_entries`, the order).
The LB-13 Postgres test covers: set, set a second book, an event, move away. Not covered on
Postgres or in the Rust rules: an entry deleted while Completed (the trigger's DELETE
branch), an undo that takes the completing event away (rule 2 then applies), a zone change
(full rebuild), the month boundary in a zone other than UTC, and the three cases of BOOK-1
and BOOK-3.

Fix: one store-pg test for the two statements (a failed item, a never fetched item in a
library, a never fetched search row nobody added, a kind filter), and extend
`completions_set_by_hand_count_on_postgres_like_the_rust_rules` with the cases above using
its `same_stats` helper.

### BOOK-12 | low | may defer | A URL as operator contact is offered as equal to an e-mail address

Where: `crates/catalog-core/src/contact.rs:61-74`, `locales/en.ftl` `admin-contact-hint`,
docs/connectors/openlibrary.md (says "unverified", which is honest).

Problem: Open Library names "contact email or phone number" and gives the identified rate
for "User-Agent and email". With a URL the instance may count as non-identified. At 1
request per second that changes nothing today. For Wikidata (v1-wikidata, `Required`) check
the Wikimedia User-Agent policy before relying on the URL form.

Fix: none needed now; the hint could say "an e-mail address (preferred) or an https
address".

### BOOK-13 | low | may defer | A search row whose key has no `/works/` prefix is dropped without a trace; a work the index does not hold loses authors and page total on refresh

Where: `crates/connectors/src/openlibrary.rs:130-131,291-293` (`work_id` wants the prefix),
`:482-501` (details with `doc == None`).

Problem 1: the documentation shows both `"key": "OL27448W"` and `"/works/OL166894W"` for a
search row. All recorded rows have the prefix. If the service answers the bare form for
some rows or one day for all, `snapshot` returns `None` and the search is silently empty.
Fix: accept both forms in `work_id`.

Problem 2: `details` returns `complete: true` with empty authors, no year and no page count
when the `key:` search answers no row (recorded: a work that is not in the index). A full
refresh then replaces the item's attributes with that, so a book that had authors and a
median from an earlier search row or from `refresh-failed` loses them, and its "of N pages"
with the progress bar, for the seven days until the next refresh. A page total a person set
is kept (B3). Fix: when the row is missing, return what the work record has and mark the
snapshot `complete: false`, or keep the stored authors and page count in `store_refresh`
when the source gives none (the plan's wording "replaces as a whole" predates B3).

### BOOK-14 | low | may defer (goes with sec2) | The contact travels along redirects; a separate worker keeps a removed contact

Where: `crates/catalog-core/src/http.rs:377-379` (the header is set per request),
docs/connectors/openlibrary.md "Hosts" (redirects are followed without a second allowlist
check, SEC-4); LB-4 (6).

Problem 1: a stored cover is fetched from `covers.openlibrary.org`, which redirects to
`archive.org` and on to `ia*.us.archive.org`. The request header goes along (the HTTP
library drops credentials on a cross-host redirect, not the User-Agent, as far as known; not
run). All three hosts belong to the Internet Archive, so today the contact stays with the
party it was meant for; until sec2 confines redirects, any other redirect target would get
it too. Note for sec2: when it adds the redirect check, keep the `archive.org` suffix for
this connector (the doc already says so).

Problem 2: with web and worker as two processes, the worker reads the contact at start. An
admin who removes the contact (the page answers "Removed. Requests carry no contact now.")
keeps sending it from the worker until that process restarts. Fix: say so in the cleared
message or the operations page, or let the worker re-read the setting with the stored
source keys.

### BOOK-15 | low | may defer | Tests written after the code and not seen red; a lint allowance for long test functions

Where: docs/fable/round2/v1-books-progress.md lines 52-54, 96-97, 103-105, 128-129;
`apps/server/tests/it/books.rs:8`.

Problem: the lane says it plainly, which is right; the rule (rules.md: write the failing
test, see it fail) was not met for: all three tests of `apps/server/tests/it/books.rs`
(page total over HTTP, the User-Agent to a mock, the configured contact), the three
`book_stubs::tests`, `governed::other_connectors_never_send_the_contact` and the unit tests
of `contact.rs`. `tests/it/books.rs` carries `#![allow(clippy::too_many_lines)]`; its first
two tests are about 100 and 145 lines (rule: functions of at most 100 lines; the lane split
three other functions for the same lint).

Fix: for the User-Agent test a red run is cheap (set `ContactUse::None` on the connector and
see `ends_with(" (ops@example.org)")` fail); split the second test (settings part, header
part) and drop the allowance.

### BOOK-16 | low | may defer | The default registry now holds a source that answers without a key

Where: `apps/server/src/lib.rs:194-199`; every `Server::build(.., Overrides::default())` in
`apps/server/tests/it/` (about 30 places); `src/web/playwright.config.ts:96-97`.

Problem: until now a test server could not reach a provider by accident (no key, no
request). With Open Library registered by default, a future integration test that searches
without a kind, with `kind=book` or `group=read` on such a server asks openlibrary.org. No
test does today (checked: the searches in `tests/it` are `kind=movie`, `kind=show`,
`group=play`, or run on an empty registry; `books.rs` points the connector at wiremock). The
Playwright server inherits the developer's environment, so a `KK_OPERATOR_CONTACT` of the
owner's shell reaches the e2e server; it is sent to 127.0.0.1 only as long as the
`BASE_URL` line stays.

Fix: in `playwright.config.ts` set `KK_OPERATOR_CONTACT: ""` and
`APP__INSTANCE__OPERATOR_CONTACT: ""` in the server's `env` (an empty value is "not set",
`config/src/lib.rs` `apply_env`); in the shared test configuration string add
`[connectors.openlibrary]\nenabled = false` where the test does not need books, or give
`OpenLibrary::new()` under `cfg(test)`-free code a guard in the test support instead (a
registry override that fails the test on any request to a non-loopback host).

### BOOK-17 | low | may defer | `xtask record openlibrary`: the contact is not checked, cut lists grow back

Where: `xtask/src/record.rs:90-97` (`-A tracker-fixtures/<v> (<contact>)`), `:44-56`
(`trimmed`).

Problem: the variable goes to curl's `-A` unchecked (no shell is involved; a line break in
the value would still end up in a header). `trimmed` keeps the top-level fields of the old
fixture but not its cuts ("covers cut to the first 6", "subjects to the first 24"), so a
re-record stores the full lists while the header note still says they are cut, and it sets
`synthetic = false` and a new source line on every file it touches.

Fix: run the value through `catalog_core::check_contact` (xtask may depend on it) or the
same character test; cut arrays to the length the old fixture has.

### BOOK-18 | low | may defer | Documentation that does not match the tree

- docs/connectors/openlibrary.md "Fixtures": "eleven recorded ... Three are marked
  synthetic". The directory holds 14 files, 10 with `"synthetic": false` and 4 with `true`
  (`isbn-0140328726`, `isbn-9780306406157-404`, `search-key-OL999001W-synthetic`,
  `work-OL999001W-synthetic`). The same numbers are in the progress log.
- "Ten requests" (LB-1, the doc): the doc lists eight; `search-key-OL45804W.json` is marked
  recorded and is not in the list. Probably the ninth; say which the ten were.
- `config/.env.example` and `InstanceConfig::operator_contact` say "Wikidata is not asked
  without one": true once v1-wikidata exists, not on this branch (LB-4 (4) says no
  connector is `Required` yet).
- LB-6 (1) "of N pages when ... an import brought it": no importer brings a page count
  (`importers/src` has none), and a total without provenance is replaced by the median at
  the first refresh. Harmless, but the sentence promises more than exists.
- LB-12: the "What v1 covers" line for books is left to the merge. Put it on the merge
  checklist (BOOK-21).

### BOOK-19 | low | may defer | Wording and accessibility details

No WCAG blocker was found by reading; the lane's axe runs cover the item page, the form and
the sources page. Small things:
- `log-of-pages` / `log-of-pages-estimate`: "of 1 pages" / "von 1 Seiten" for a total of 1;
  use a plural selector as `stats-also-events` does.
- The text "of N pages (estimate)" (`data-testid="pages-of"`) stands beside the page field
  and is not part of its description; a screen reader user in the field does not hear the
  total. Give the span an id and add it to `described_by` of `pages-at`.
- `admin/contact.rs`: `inputmode="email"` on a field that also takes an https address
  (`inputmode="url"` shows `/` and `:`; plain text is the neutral choice).
- `stats-finished-note` is shown as soon as one book is finished, also when every finished
  book has a total (LB-7 says so on purpose); it reads as if something were missing.
  Condition it on a finished book without `page_count` if the totals read can tell, else
  leave.
- No audit entry and no undo for a changed page total, although it changes the shared item
  for everyone (hosted: admins only). Acceptable for v1; v1-manual's "Edit details" should
  decide it once for all fields.

### BOOK-20 | low | may defer | Smaller points in LB-13

- `stats_item_changed` (migration 0029) marks the users who have events of an item. A user
  whose only trace of the item is a completion set by hand is not marked when the item's
  kind changes; the cached row keeps the old kind. Kinds of stored items change rarely if
  ever; add `UNION SELECT user_id FROM library_entries WHERE item_id = NEW.id AND status =
  'completed'` when the function is next touched.
- `hlc_time` is declared IMMUTABLE and built from `timestamptz + interval`, which Postgres
  itself only calls STABLE. For a millisecond interval the result does not depend on the
  session, so this is safe; `to_timestamp(<bigint> / 1000.0)` says the same without the
  question. A stamp that does not start with 16 hex digits makes the whole statistics
  statement fail; the column is NOT NULL and only the application writes it, and client
  stamps are bounded ahead (`app/src/lib.rs:109`), so this is theoretical. The Rust rule
  answers 1970 for a wall clock it cannot convert; the SQL would raise. Not worth code.
- `stats::history` looks up the entry of each item with a scan over all entries
  (`stats.rs:209-213`): quadratic, in the in-memory store and the comparison tests only.
- History shows no row for a completion set by hand (no event). Consistent with LB-13; the
  owner should know that "finished" in Stats and the History list can differ by these.

### BOOK-21 | info | merge checklist with current main

Main moved by 102 files since the base (sec0, sec1, the rename). Files changed on both
sides: `config/.env.example`, `docs/modules/ROOT/pages/configuration.adoc` (generated: run
`UPDATE_DOCS=1 cargo test -p config` after the merge, do not merge by hand),
`operations.adoc`, `apps/server/Cargo.toml`, `apps/server/src/lib.rs`,
`apps/server/tests/it/main.rs`, `crates/config/src/lib.rs`, `locales/en.ftl`,
`locales/de.ftl`, `crates/web/src/item.rs`, `crates/web/src/log_sheet.rs`,
`src/web/playwright.config.ts`.

1. `src/web/tests/books.spec.ts:1` imports `test, expect` from `@playwright/test`. On main
   every spec imports them from `./fixtures` (sec1). Change the line, or the new spec runs
   without whatever the fixture adds (the content security policy check, as far as the
   import line tells).
2. Covers under the policy of sec1: main's `img-src` is `'self' data: https:` plus `http:`
   only when the server allows http images. The e2e cover comes from
   `http://127.0.0.1:3015`; `books.spec.ts` asserts `naturalWidth > 0`. Run the spec on the
   merged tree; if the picture is blocked the test fails at that poll.
3. Migrations: main ends at 0034. Renumber 0100 and 0101 to 0035 and 0036, then
   `cargo xtask sqlx-prepare` on the merged tree (the lane's `.sqlx` set was made against
   the lane's base: 6 files removed, 8 added). `lb_dev` has 0100 and 0101 applied under the
   old numbers and has to be recreated.
4. `KK_OPERATOR_CONTACT` (the prefix v1-release renames): `crates/config/src/lib.rs`
   (`OPERATOR_CONTACT_ENV`, the doc comment of `InstanceConfig::operator_contact`, the test
   `the_operator_contact_has_a_short_variable`), `config/.env.example`,
   `xtask/src/record.rs` (`RECORDINGS`, module comment, one test),
   `docs/connectors/openlibrary.md`, `docs/modules/ROOT/pages/connectors.adoc`,
   `configuration.adoc` (generated), LB-4 and LB-10. Give v1-release this list. The long
   form `APP__INSTANCE__OPERATOR_CONTACT` is what the admin page prints as the variable.
5. `apps/server/src/lib.rs`: the lane changes `default_registry`, `Server::build` (contact
   handle, `LiveSettings`) and `governed_http`; sec1 changed the same file. `book_stubs.rs`
   calls `crate::default_registry` and `crate::escape::shell_app`: both must survive the
   merge with these names.
6. The rename: the new code holds neither product name. `record.rs` sends
   `tracker-fixtures/<version>` as the product of the recording requests, not the brand
   ident; fine.
7. LB-1..13 become DEC entries (next free number on main is after DEC-669); LB-13 changes
   what DEC-594 and DEC-601 deferred and should name both. The "What v1 covers" line for
   books (LB-12) is added at the merge with v1-polish.
8. Out of scope but in the diff, all small and defensible: `escape.rs` (`shell_app` taken
   out of `Escape::open`), the cli's `parse` split, `min-h-6` on the "Open <source>" links
   of the attention block (an axe target-size failure the lane met), the changed expectation
   in `app/tests/rewatch.rs`, and LB-13 as a whole (asked for by the orchestrator; it is a
   statistics change, not a books change, and carries the three major findings).

### BOOK-22 | info | The two browser timeouts (`settings.spec` reflow, `sharing.spec` promote and merge)

Nothing in this package was found that would slow either by seconds:
- `settings.spec` "every settings page reflows" visits six `/settings/*` pages; the only
  change it meets is one more row (Open Library) on `/settings/sources`. The lane's rerun
  alone passed.
- `sharing.spec` opens the palette once (`sharing.spec.ts:64-66`, "fight club", no scope).
  That search now also asks the Open Library mock, on a lane that lets one request through
  per second for the whole server. The test waits for the TMDB row, not for all sources, so
  this should not hold it; under a parallel run every unscoped palette search of every
  worker queues on that one-per-second lane, which adds load to the server but not a wait to
  this test as far as read. The entry trigger and the extra `UNION ALL` branch cost
  microseconds per write and read at this size.
- The test needs 23.4 s alone on this host against a limit of 30 s (the lane's own number).
  That margin is too thin with or without this package. Compare with the same test alone on
  main; if it is also above 20 s there, give it `test.setTimeout(90_000)` as `books.spec`
  does for its long test.
- What the package does add to a full run: three tests (one with a 180 s budget), one more
  web server process (the mock) and, per BOOK-16, a 1-per-second queue shared by all
  unscoped searches.

## Load-run verdict

Not needed before the merge. Reasons: the trigger on `library_entries` is a row trigger
with a constant-time test and at most one upsert into `stats_dirty`, the pattern the
progress-event triggers already use; the new branch of the three statements reads the
user's Completed entries through `UNIQUE (user_id, item_id)` and probes
`progress_events_user_item (user_id, item_id, occurred_at)` once per entry; the two new
catalogue statements run only from the cli. None of it is on the search or library paths
the existing load test measures (`tests/it/load.rs` runs on an empty registry and has no
Completed entries without events).

Add to the release load run the plan already has (DEC-601), because it was reasoned and not
measured here: one user with 5 000 entries imported as Completed (5 000 dirty marks from
one import, then the first `GET /stats`, `GET /home` and a goals read: the per-item
recompute with 5 000 ids, and `pg_window_amounts` once per goal with `hlc_time` evaluated
for every Completed entry), and a bulk status change of 500 entries. If BOOK-1's column is
built, the window read can use an index on `(user_id, completed_at)` and the question goes
away.
