# v1-books progress (lane B, worktree kithkeep-lane-books, branch lane/v1-books)

Spec: docs/fable/round2/v1-plan.md section 3 (and 7: B1, B3, B4). Lane rules:
docs/handoff/fable-briefs/LANES.md (main checkout). Decisions of this lane:
docs/fable/round2/v1-books-decisions.md (L-B-n). Nothing is committed.

## First step: the owner's rows (3.1.4)

The owner's export (`owner-inbox/legacy/export.json`, 135 rows): 10 books, 4 with source
`openlibrary` (all work ids, `OL...W`: OL276558W, OL27969006W, OL1914022W, OL26413598W) and
6 with `google_books`. The dev database `app_dev` holds no book items. The plan says "fewer
than about ten google_books rows: skip match-books"; the orchestrator's brief asks for the
match with a dry run first, so it is built (L-B-2).

## Open Library API, checked live on 2026-11-12 (10 requests, one per 1.3 s, no contact)

- `search.json?q=dune&fields=...&lang=en&limit=3`: 200, docs hold exactly the fields asked.
- `search.json?q=key:/works/OL82563W&fields=...&limit=1`: 200, one doc (the `key:` query
  works).
- `search.json?q=key:(/works/OL82563W OR /works/OL45804W OR /works/OL893415W)&limit=50`:
  200, two docs (the multi-key form works; a key that is no work in the index is absent).
- `/works/OL45804W.json`: `description` is a plain string; `covers` holds `-1`.
- `/works/OL82563W.json`: `description` is `{"type": "/type/text", "value": ...}`.
- `/isbn/9780140328721.json`: one redirect to `/books/OL7353617M.json`.
- `/works/OL1W.json`: 404 `{"error": "notfound", "key": "/works/OL1W"}`.
- `search.json?title=Fantastic Mr Fox&fields=...&limit=5`: 200, 5 docs, four of them with
  the same normalised title (so that stub would stay unmatched: a tie).

## Items

| # | Item | State |
|---|---|---|
| 1 | Operator contact (config, setting, User-Agent, needs_contact, admin field) | in work |
| 2 | Connector openlibrary (fixtures, tests, registry, docs) | tests and fixtures written |
| 3 | Page totals, Finished, Change, user provenance survives a refresh | open |
| 4 | cli refresh-failed, cli match-books | open |
| 5 | Playwright books.spec.ts, sources.spec.ts; docs | open |

## Log

- catalog-core: `contact.rs` (ContactUse, OperatorContact, check_contact), field
  `ConnectorInfo::contact`, `ConnectorPolicy::contact`, `CredentialState::NeedsContact`;
  tests in `tests/governed.rs` (3) and `tests/credentials.rs` (1), unit tests in contact.rs.
- connectors: fixtures `fixtures/openlibrary/` (11 recorded, 3 synthetic),
  `tests/openlibrary.rs`; the implementation waits for the red run.
- Red and green runs (2026-11-12, `CARGO_BUILD_JOBS=1`):
  - `cargo test --no-fail-fast -p catalog-core -p connectors --features connectors/all`:
    red with skeletons (3 catalog-core tests: `a_source_that_requires_the_contact_is_skipped_
    until_one_is_set`, `a_required_contact_that_is_missing_blocks_the_call_without_a_request`,
    `a_connector_that_wants_the_contact_sends_it_in_the_user_agent`; all 12 of
    `tests/openlibrary.rs` against a stub module), then green (catalog-core 13+5+2+14+7+1+2+3,
    connectors openlibrary 12, the other connector tests unchanged). Not seen red:
    `other_connectors_never_send_the_contact` (holds without the change) and the unit tests
    of `contact.rs` (written with the code).
  - `cargo test -p app --test books`: red (5 of 6) with skeleton methods, green (6) after;
    `--test catalog_audience` 9 passed.
- FINDING (differs from the plan, section 3.1.4 and B1): an import stub is NOT `failed`. The
  importer creates it with `find_or_create_catalog_item`, which stores state `partial`;
  `mark_catalog_refresh_failed` turns only `stub` into `failed`. So the owner's Ryot books are
  `partial`, never fetched, and are retried only when their page is opened (one hour after the
  last try). `failed` is reached by purged items (state `stub`) whose refetch failed. The
  command therefore takes both: state `failed`, or never fetched and in somebody's library
  (`CatalogRepo::unfetched_items`); search results nobody added are left alone. L-B-3.
- app: `USER_PROVENANCE`, `store_refresh` (the rule for every field), `set_item_page_count`,
  `apply_catalog_partial`, `requeue_failed`, `unfetched_items`, `items_without_id_from`;
  ports `CatalogRepo::unfetched_items`, `items_without_namespace` (store-pg and testkit).
- config: `instance.operator_contact`, short form `KK_OPERATOR_CONTACT`; migration
  `0100_operator_contact.sql`; auth `StoredSettings::operator_contact`; api setting
  `instance.operator_contact` (code `invalid_contact`), state word `needs_contact`; server
  wiring (registry, governed client, admin API share one `OperatorContact`); web
  `admin/contact.rs` on Admin > Data sources, `source_note`, locale strings en + de;
  `config/.env.example`. NOT yet compiled or tested beyond catalog-core/connectors/app.
- Still open: sqlx-prepare, api route + server fn for the page count, the Change control,
  Finished, stats footnote, server `book_stubs` module + cli commands, xtask record, the
  Open Library mock for Playwright, books.spec.ts, sources.spec.ts, docs, decisions file.
- Written, first compile of api/web/server running (2026-11-12 late): page total (contracts
  `ItemPageDto::can_edit`, `PageCountDto`; api `ops::set_page_count`, route
  `PUT /items/{id}/page-count`; web `Backend::set_page_count`, server fn, `log_form`
  (`PageTotal`, `page_total`, `parse_page_total`, `finish_page`), `log_sheet::PagesInline`
  with the Change form, `next_step::run_step_then`, `InlineDraft::{total, refocus}`), the
  stats footnote (`time_tiles`), `apps/server/src/book_stubs.rs` (refresh_failed,
  match_books, Catalog), `escape::shell_app`, cli commands `refresh-failed [--kind]` and
  `match-books [--apply]`, `xtask record openlibrary`, `src/web/tests/openlibrary-mock.mjs`,
  `playwright.config.ts` (OPENLIBRARY_MOCK_PORT, default 3015), `tests/books.spec.ts`
  (3 tests), docs (connectors/openlibrary.md, connectors.adoc, operations.adoc section
  "Titles that never got their data"), decisions file LB-1..LB-12.
- Databases of this lane: `lb_dev` (sqlx macros: build with
  `DATABASE_URL=postgres://app:app@127.0.0.1:55432/lb_dev`), later `lb_e2e`.
- To do after the compile: run red for the web unit tests (log_form stubs return
  None/false on purpose: `finish_page`, `parse_page_total`, `page_total`), implement them,
  server tests (lib `book_stubs::`, it: admin, setup_page, item_page, home_page, cli,
  imports ...), `cargo xtask sqlx-prepare`, `UPDATE_DOCS=1 cargo test -p config`, the
  Playwright chromium project on ports 3210..3215, clippy on the touched crates.
- 2026-11-13 runs:
  - `cargo test -p server --features gallery,all-connectors --lib --test it` (whole binary,
    `TEST_DATABASE_URL` set): lib 11 passed (3 new `book_stubs::tests`, written with the code,
    not seen red), it 113 passed, 2 ignored. Before `tests/it/books.rs` existed.
  - `cargo test -p web --features ssr,gallery`: red as planned, 220 passed, 3 failed
    (`log_form::tests::finished_logs_the_last_page_when_the_total_is_known`,
    `a_page_total_is_a_number_from_1_to_100000`,
    `only_a_total_from_an_estimating_source_is_an_estimate`) against stubs; the helpers are
    implemented now, green run pending.
  - Added after that: `apps/server/tests/it/books.rs` (3 tests: page total over HTTP and
    SSR, hosted rule; operator contact setting and User-Agent to a mock; configured contact
    read-only, unusable contact stops the start). Not run yet.
  - `cargo test -p web --features ssr,gallery` after implementing: 223 lib + 3
    `button_classes` passed.
  - `cargo clippy --workspace --all-targets --features <native> -- -D warnings -W
    clippy::pedantic`: three rounds (PagesInline 105 lines -> `PageTotalLine` split out; cli
    `parse` 101 lines -> `command` split out; the match-books test 110 lines -> `shelf()` and
    two tests), then clean. `cargo fmt --all` run.
  - `cargo xtask sqlx-prepare` (DATABASE_URL = lb_dev): `.sqlx` updated (2 files removed, 4
    added).
  - `UPDATE_DOCS=1 cargo test -p config`: 18 passed, configuration.adoc regenerated.
  - Running: the tests of every touched crate with `SQLX_OFFLINE=true` (catalog-core,
    connectors, app, auth, api, store-pg, kinds, testkit, importers, cli, xtask, web, server).
  - Then: wasm clippy for web, `cargo leptos build`, Playwright chromium (ports 3210-3215,
    database lb_e2e).
  - Tests of every touched crate, `SQLX_OFFLINE=true`, `TEST_DATABASE_URL` set, one command
    (`cargo test --no-fail-fast -p catalog-core -p connectors -p app -p auth -p api -p store-pg
    -p kinds -p testkit -p importers -p cli -p xtask -p web -p server --features
    connectors/all,importers/all,server/gallery,server/all-connectors`): all passed except
    xtask `locales::web_keys_in_code_exist_and_every_key_is_used` (the id `sources-contact` of
    a kept message looked like a locale key; renamed `src-contact`, then `cargo test -p xtask
    --test locales`: 5 passed). Counts: catalog-core 13+5+2+14+7+1, connectors 2+3+12+2+6 (+tmdb),
    app books 6 and the other app tests, auth 38+2+4+5+5+8+2, api 13+24+1, store-pg 1+2+2+1+1+
    1+2+2+3+5+3, kinds 5+2, importers 12+7+8+1+1+3, cli 3, xtask 20+1+6+3+19+2, web 223+3,
    server lib 12, server it 116 passed (2 ignored), including the 3 new `books::` tests
    (written after the code, not seen red).
  - wasm clippy (`cargo clippy -p web -p ui --all-targets --target wasm32-unknown-unknown
    --features web/hydrate,web/gallery -- -D warnings -W clippy::pedantic`): clean.
  - Running: `cargo leptos build`, then Playwright chromium.
- 2026-11-13, orchestrator's defect "0 books finished" (LB-13): rule and code as in the
  decisions file. Red first: app `stats::a_status_set_to_completed_by_hand_is_a_completion`
  (0 vs 2), store-pg `phase1::completions_set_by_hand_count_on_postgres_like_the_rust_rules`
  (0 vs 2). After the change: `cargo test -p store-pg` all green (found on the way: the cache
  dropped a month without events; `stats_race::updates_of_events_...` caught it), `cargo test
  -p app -p testkit -p importers` green after one expectation in `rewatch.rs` (0 -> 1).
  Migration 0101 applied to lb_dev. STILL TO DO after this: sqlx-prepare again, fmt, clippy,
  api/server/web tests, expectations in SSR and browser tests that said "0 ... finished" for
  Completed entries, `cargo leptos build`, the whole Playwright chromium project again.
- First Playwright chromium run (before LB-13) started 2026-11-13 on ports 3210-3215,
  database lb_e2e; result: see below when it ended.
- First Playwright chromium run (build before LB-13): 150 passed, 1 flaky, 4 failed in
  24.2 min. books.spec "a book from Open Library" (Stats said "0 books finished": the defect
  of LB-13; its retry met the page total the first try had set on the shared item, so the
  spec now picks its own total and checks the estimate on another book); admin.spec "a source
  is switched off and on again" and (flaky) "a data source is tested": axe target-size on the
  "Open <source>" links of the attention block (20 px high, 23.6 px of space): the links are
  24 px high now (`min-h-6`); settings.spec "every settings page reflows" and sharing.spec
  "promote, accept ...": the 30 s test timeout while three other lanes compiled (the settings
  test was on its last page); not caused by this package as far as seen, rerun below.
- After LB-13: `cargo fmt --all --check` clean; `cargo xtask sqlx-prepare`; pedantic clippy
  (native) clean; the tests of every touched crate in one command (`SQLX_OFFLINE=true`):
  exit 0, 94 test binaries ok, no failure; `cargo leptos build` done. The whole Playwright
  chromium project is running on that build.
- FINAL browser run (build with LB-13, 2026-11-13, ports 3210-3215, fresh lb_e2e, `CI=1 npx
  playwright test --project=chromium`): 153 passed, 2 failed in 31.3 min, no panic in the
  server log. The two: settings.spec "every settings page reflows and passes axe at 360 px"
  and sharing.spec "promote, accept, see history, unlink, merge and undo the merge", both
  "Test timeout of 30000ms exceeded" with no failed assertion (load average 7 to 11 on 4
  cores, three other lanes compiling). Rerun alone on a fresh database with one worker:
  settings.spec all passed; the sharing test timed out again at 30 s and passed in 23.4 s
  with `--timeout=120000`. Neither touches code of this package. All three books.spec tests
  and both admin.spec source tests passed.
- State: every scope item done, LB-13 done; nothing committed. `lb_e2e` dropped, `lb_dev`
  kept (migrated to 0101) for the next package of this lane.

## Review fixes (2026-11-13, docs/reviews/2026-11-13-v1-books-review.md, BOOK-1..22)

Decisions: LB-14 .. LB-22 in v1-books-decisions.md. Nothing is committed. Migrations keep
the numbers 0100 and 0101 (both rewritten in place; `lb_dev` was dropped and created again).
While the orchestrator's gate flag was set, only code work and tests of single crates ran
(`CARGO_BUILD_JOBS=1`); the server `it` binary, `cargo leptos build` and Playwright waited.

| Finding | State |
|---|---|
| BOOK-1, BOOK-3 | done (LB-14): `LibraryEntry::completed`, columns `completed_at` / `completed_undated`, guard trigger |
| BOOK-2 | done as the orchestrator's default (LB-15): the newest logged step, else undated; one line on the import result and a Stats footnote |
| BOOK-4 | done (LB-16): backfill and `UPDATE stats_users SET zone = ''` in 0101 |
| BOOK-5 | done (LB-17): `page_total_editable`, `App::takes_page_total` |
| BOOK-6 | done (LB-18): the page log and Completed are queued in order |
| BOOK-7 | done (LB-20): dry run by default, a failed batch is a report line |
| BOOK-8 | done (LB-19): one pace per keyless source in the database |
| BOOK-9 | done in part (LB-21): 20 rows, first author, log of applied matches with `--applied`; undo deferred |
| BOOK-10 | deferred to v1-manual, written into v1-plan.md 4a point 6 |
| BOOK-11 | done: `store-pg/tests/book_stubs.rs`, and the cases in `phase1.rs` |
| BOOK-12 | done (hint en + de, docs) |
| BOOK-13 | part 1 done (both key forms); part 2 deferred (LB-22) |
| BOOK-14 | documented; the redirect part is sec2's |
| BOOK-15 | test split and lint allowance done; red runs deferred (LB-22) |
| BOOK-16 | Playwright env done; server test configurations deferred |
| BOOK-17 | done |
| BOOK-18 | done |
| BOOK-19 | plural, description of the page field, plain text contact field done; the rest deferred |
| BOOK-20 | `stats_item_changed`, `hlc_time` gone, no scan per item; the rest is by design |
| BOOK-21 | note in `books.spec.ts` (no `tests/fixtures.ts` on this branch); numbers kept |
| BOOK-22 | nothing to do here |

Tests seen red before the change (2026-11-13, `CARGO_BUILD_JOBS=1`, on the tree before the
fix):
- app `--test completions`: 5 of 6 failed (`completed_written_again_keeps_its_day`,
  `a_bulk_completed_over_completed_entries_keeps_their_day`,
  `a_removed_and_restored_entry_keeps_its_completion`,
  `a_completion_by_hand_lies_on_the_newest_logged_step`,
  `a_completed_entry_without_a_logged_step_is_undated`); the sixth
  (`a_completion_by_an_event_is_not_doubled_by_a_status_write`) held before and holds after.
- app `--test rewatch`: `a_rewatch_keeps_the_first_completion_where_it_was` failed.
- importers `--test generic`: `importing_twice_keeps_every_completion_in_its_month` failed
  (`[("2026-01", 2)]` instead of `[("2025-01", 1)]`).
- store-pg `--test phase1`: `a_completion_by_hand_keeps_its_day_on_postgres` failed
  (`2026-01` instead of `2025-03`).
- BOOK-5, with the rest of the fix in place and `set_item_page_count` unchanged: app
  `books::a_paged_kind_without_a_page_count_takes_no_total` failed (the raw text of the
  attribute check).
- BOOK-4, with the last statement of 0101 taken out: store-pg
  `phase1::the_completion_migration_dates_existing_entries_and_rebuilds_the_cache` failed
  (the cached read without the completions of the two books).
Not seen red (written with the code): domain `library::tests::*`, catalog-core
`governed::a_source_without_a_key_...` and `pace::*`, web `offline::tests::finished_offline_...`
and the footnote assertions, server `book_stubs::tests::*` (new and changed), store-pg
`tests/book_stubs.rs`, connectors `openlibrary::tests::a_work_key_...`, xtask `record::tests`.

Expectations of older tests that changed, and why:
- app `stats::a_status_set_to_completed_by_hand_is_a_completion` and store-pg
  `completions_set_by_hand_count_on_postgres_like_the_rust_rules`: the book without any
  logged step is undated (LB-15): 2 in the totals as before, 1 in the month and the window.
- store-pg `stats_on_postgres_match_the_in_memory_rules`: 3 -> 4 completions in the series
  (the show completed by hand keeps its completion when its rewatch run is finished, LB-14).
- server `book_stubs` tests: the title search asks for 20 rows; "Fantastic Mr Fox" is
  answered by a synthetic row list in the test (the recorded fixture asked for 5 and stays
  for the connector test).

Runs: see "Review fixes: runs" below.

### Review fixes: runs (2026-11-13, `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0`, gate flag set)

- `cargo test --no-fail-fast -p domain -p app -p importers -p catalog-core -p connectors
  -p store-pg -p testkit -p api -p contracts -p xtask -p cli -p kinds -p config@0.1.0
  --features connectors/all,importers/all` (`TEST_DATABASE_URL` set, sqlx macros against
  the recreated `lb_dev`): 85 test binaries ok, 2 failed and fixed: config
  `reference_doc_is_current` (the comment of `operator_contact` changed: `UPDATE_DOCS=1 cargo
  test -p config@0.1.0`, configuration.adoc regenerated, then 18 passed) and contracts
  `the_stats_totals_carry_kinds_and_default_to_none` (the new field `undated_completions`;
  expectation extended, then 11 passed). Among them: domain 40 unit tests, app `completions`
  6, `rewatch` 11, `stats`, `books` 7, `bulk`, `bulk_undo`, importers `generic` 8, catalog-core
  `governed` and `pace`, connectors `openlibrary` (unit and fixtures), store-pg `phase1` 13,
  `book_stubs` 3, `stats_race`, `migrations`, xtask (`locales`, `record`, `arch`), cli 3.
- `cargo test -p server --lib --features gallery,all-connectors`: 14 passed (the
  `book_stubs` tests: 12 before, 2 new).
- `cargo test -p web --features ssr,gallery`: 224 lib + 3 `button_classes` passed.
- `cargo xtask sqlx-prepare` (`DATABASE_URL` = `lb_dev`): `.sqlx` rewritten.
- `cargo fmt --all --check`: clean. Pedantic clippy, native (`--workspace --all-targets
  --features connectors/all,importers/all,integrations/all,notify/all,server/gallery,
  server/all-connectors -- -D warnings -W clippy::pedantic`): clean after one round (the
  helper `contact` of `tests/it/books.rs`). Pedantic clippy wasm (`-p web -p ui --target
  wasm32-unknown-unknown --features web/hydrate,web/gallery`): clean.
- Test databases: the runs created `test_<uuid>` databases on the shared server; none was
  dropped by this lane (their owner cannot be told from the name, LB-11). `lb_dev` was
  dropped and created twice (the migrations changed).

### Review fixes: runs after the gate flag was gone (2026-11-13, about 08:05)

- `cargo test --no-fail-fast -p server --features gallery,all-connectors --test it` (the
  whole binary, `TEST_DATABASE_URL` set): 118 passed, 2 ignored, in 52 s. 116 before; the
  contact test is two now and `books::a_kind_without_a_page_count_offers_no_page_total` is
  new (the manga's action bar has the page field and no "Set page total").
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`:
  done (7 min).
- The whole Playwright `chromium` project (`CI=1 npx playwright test --project=chromium`,
  server `target/debug/lb-server` (a hard link, `E2E_SERVER_CMD`), ports 3210-3215, fresh
  database `lb_e2e`): 153 passed, 2 failed in 23.6 min, no `panicked at` in the server log
  (480 lines). The two are the two of the run before the review, both "Test timeout of
  30000ms exceeded" without a failed assertion, on the first try and on the retry:
  settings.spec "every settings page reflows and passes axe at 360 px" and sharing.spec
  "promote, accept, see history, unlink, merge and undo the merge" (load average 12 to 14 on
  4 cores during the run; review BOOK-22 found nothing in this package that slows them).
  Rerun of the two specs alone, one worker, fresh database, same load: settings.spec 7 of 7
  passed (the reflow test in 28.0 s of its 30 s), sharing.spec 2 passed and the promote and
  merge test timed out again at 30 s (both tries). No timeout was changed (lane rule: the
  orchestrator judges it in the main-checkout gate). All three `books.spec` tests,
  `imports.spec`, `stats.spec`, `home.spec` and `offline.spec` passed.
- `lb_e2e` dropped again, `lb_dev` kept (migrated to the rewritten 0100 and 0101). No
  `lb-server` process is left; the hard link `target/debug/lb-server` stays.

State: every finding of the review is applied or deferred with its reason (LB-14 to LB-22);
nothing committed.
