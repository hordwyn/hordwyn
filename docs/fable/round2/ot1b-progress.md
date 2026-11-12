# ot1b progress (search ranking OT-3 steps 1 and 2, palette covers OT-2, cover flash OT-21)

Resume notes for a cut-off run. Brief: docs/handoff/fable-briefs/PKG-implement.md.
Nothing is committed. Build commands: `nix develop -c bash -c 'cd src/rust; CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 cargo test ...'`.
Service env: `TEST_DATABASE_URL=postgres://app@127.0.0.1:55432/postgres REQUIRE_SERVICES=1
TEST_MEILI_URL=http://127.0.0.1:55434 TEST_MEILI_KEY=dev-only-master-key-0123456789`.
Playwright by hand as in ot2e-progress.md (ports 3110 to 3114, databases `ot1b_<n>`).
Next free DEC at the start: 607.

| # | Item | State | Tests (red first) | Files | DEC |
|---|---|---|---|---|---|
| 1 | OT-3 step 2: tie-breaks after the words (the title that is the query, then titles that start with it) | done | `search/tests/suite.rs` `title_ties` on both backends: red on Postgres, green on Meilisearch 1.56 unchanged (no settings change needed) | crates/search/src/pg.rs, tests/suite.rs; .sqlx (one file replaced) | 607 |
| 2 | OT-3 step 1 rest: the library leads the palette, kept copies sit in their source's group, a source's answer orders its group | done | api `rest::search_names_the_hits_in_the_searchers_library` (red: `null`); web `palette::tests::library_entries_lead_and_a_source_keeps_its_order` (red once against a wrong first `merge`); Playwright shell.spec.ts "palette: the library leads, a source's answer keeps its order, rows lead with a cover box" (red on the build of HEAD) | crates/contracts/src/lib.rs, crates/api/src/ops.rs, tests/rest.rs; crates/web/src/palette.rs, locales en + de (`search-library`) | 609 |
| 3 | OT-2: palette rows lead with a cover box (stored covers only, else the kind icon) | done | ui wasm `command_option_renders_its_leading_slot` and web `palette::tests::to_item_keeps_the_cover`: written before the code, first run after it (not seen red); the Playwright test of item 2 | crates/ui/src/command.rs, lib.rs, tests/wasm.rs (+ `leading: None` in components.rs, render.rs, web gallery.rs); crates/web/src/palette.rs | 608 |
| 4 | OT-21: covers flash during an import | done | Playwright home.spec.ts "covers stay while an import runs: cards keep their pictures, refetches are paced" (red on the build of HEAD at its first new assertion, the transparent alternative text; the element identity and pacing assertions were not reached there); web `events::tests::a_burst_of_events_refetches_once_per_quiet_time`, `home::tests::the_sections_outlive_answers_with_rows_and_offline_keeps_the_last_one` (changed with the code, not seen red) | crates/web/src/events.rs, widgets.rs, home.rs, library.rs, history.rs | 610 |
| 5 | Search latency: EXPLAIN ANALYZE old against new on two seeded databases; the library lookup | done | - | docs/DECISIONS.md | 607, 609 |
| 6 | Gates: fmt, clippy native + wasm, locale tests, server page tests, whole chromium project | done | see "Final runs" | - | - |

## Log

- 2026-11-12: brief, rules, HANDOFF 0/2/4/8, plan section, diagnosis OT-2/OT-3, DEC-493/499 read.
  The owner's answers file has no ruling on hotlinking (only OT-3 "re-check after DATA-0"):
  the conservative default of the plan applies (cached covers only), to be recorded in a DEC.
- Item 1 code + test done: `search/tests/suite.rs` `title_ties` red on the Postgres fallback
  (`["d4", "d3", "d2", "c4"]`), green on Meilisearch 1.56 as it is (its default rules
  wordPosition and exactness already give the order; no settings change). `pg.rs`: `whole`,
  `lead` before the score in the `exact` CTE and the outer ORDER BY. `cargo xtask sqlx-prepare`
  run (one file replaced in `.sqlx`). `cargo test -p search`: 3 + 1 + 2 passed.
- Item 5 measured: databases `ot1b_explain_0` (the load test's 50 551 title-only documents) and
  `ot1b_explain_1` (same with 600-character descriptions), scripts in the scratchpad (seed.sql,
  explain.sql); numbers go into the DEC. Summary: +6 to 7 ms on the two one-word load queries
  that match 10 000 documents ("granit", "indigo"), no change on the three others.
- Item 2: `SearchResponseDto.in_library` (api test `rest::search_names_the_hits_in_the_searchers_library`
  red: `null`, then green, `--test rest` 22 passed); `palette.rs` `local_rows`, `merge`,
  `asked_source`, `Placed`; Fluent `search-library` en + de.
- Item 3: `ui::CommandLeading`, `Leading` slot in `command.rs`; `palette.rs` `thumb` (stored
  covers only). wasm test `command_option_renders_its_leading_slot` written, not run yet.
- Item 4: `events.rs` `Pace` + `refresh_on_updates` (Home, Library, History); `widgets.rs`
  `CoverOf`, `StableCover`, `keyed`, `PosterCard` split (`PosterCardRest`), `text-transparent`
  on the cover `img`; `home.rs` sections on a memo of the latest answer, keyed Continue and Up
  next cards, `Shown::next` keeps the key between answers with rows.
- `cargo test -p web --lib`: 211 passed (merge test seen red once: rows went after a row of
  another group; the other new unit tests were written with the code).
- Playwright tests written: home.spec.ts "covers stay while an import runs ...", shell.spec.ts
  "palette: the library leads ...". Both run on the old build (HEAD, database `ot1b_0`): 2 failed
  (cover `img` colour not transparent; "tmdb - Movie" group empty on the second search).
- Next: fmt, clippy native + wasm, ui wasm test, server page tests, `cargo leptos build`, the
  two browser tests, whole chromium project, DECs (607 ranking + EXPLAIN, 608 hotlink policy,
  609 library leads / in_library, 610 OT-21), drop `ot1b_*` and `test_*` databases.
- fmt clean; clippy pedantic wasm (`-p web -p ui --all-targets --target wasm32-unknown-unknown
  --features web/hydrate,web/gallery`) and native (`--workspace --all-targets`, CI features):
  clean. `cargo test -p ui --target wasm32-unknown-unknown`: 23 passed (new test green; it was
  written before the component code but first run after it). `cargo test -p web --lib`: 211,
  `cargo test -p ui`: 17 + 31 + 17, `-p xtask --test whitelabel --test locales`: 2 + 5 passed.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
  The two new browser tests on it (database `ot1b_1`): 2 passed with the setup.
- EXPLAIN of the library lookup (`entries_for`, 20 ids, 50 551 entries seeded into `ot1b_1`):
  Index Scan on `library_entries_user_id_item_id_key`, 0.13 ms.
- DEC-607 (ranking + EXPLAIN + load run), DEC-608 (cached covers only), DEC-609 (library leads,
  `in_library`, merge), DEC-610 (OT-21) written.
- Running: server `it` page tests and the modules that call `/search`. Then `cargo leptos
  build` again (the `it` build replaces `target/debug/server`), whole chromium project.
- Server `it`: `item_page:: library_page:: home_page:: auth_pages:: admin_pages:: member_pin::
  instance:: admin:: setup_page:: latency::`: 49 passed (first build of the package).
- Whole Playwright `chromium` project on the second cargo-leptos build (database `ot1b_2`,
  2 workers, server log captured): 148 passed (setup 1 + chromium 147), 0 failed, 0 flaky,
  0 `panicked at`.
- After that run one more code change: Home's activity rows and stats block are rendered
  afresh through `<For>` with a fresh key when they change (house pattern UX1-2) instead of a
  bare reactive closure. fmt, clippy wasm + native clean, `cargo test -p web --lib` 211 passed.
- Running now: server `it` modules again, `-p api --test rest`, `-p search`; then the final
  `cargo leptos build` and the whole chromium project once more (database `ot1b_3`).
- Disk: after the `it` build 3.0 GB were free; executables in `target/debug/deps` deleted and
  all `test_*`, `ot1b_0`, `ot1b_1`, `ot1b_explain_*` databases dropped (5.0 GB, 7.0 GB after
  the leptos build replaced the server binary).

## Final runs (2026-11-12, on the final code)

- `cargo fmt --all`: clean. Clippy pedantic wasm (`-p web -p ui --all-targets --target
  wasm32-unknown-unknown --features web/hydrate,web/gallery`) and native (`--workspace
  --all-targets`, CI feature set): clean.
- `cargo test -p web --lib`: 211 passed. `cargo test -p api --test rest`: 22 passed.
  `cargo test -p search` (Postgres and Meilisearch): 3 + 1 + 2 passed.
- Server `it` (`item_page:: library_page:: home_page:: auth_pages:: admin_pages:: member_pin::
  instance:: admin:: setup_page:: latency::`): 49 passed.
- Run before the last Home change and not repeated after it (the change does not touch them):
  `cargo test -p ui` (17 + 31 + 17), `cargo test -p ui --target wasm32-unknown-unknown` (23),
  `cargo test -p xtask --test whitelabel --test locales` (2 + 5).
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`,
  then the whole Playwright `chromium` project (database `ot1b_3`, 2 workers, server log
  captured): 148 passed (setup 1 + chromium 147), 0 failed, 0 flaky, 0 `panicked at`.
- Not run: `cargo xtask ci`, `cargo xtask e2e` (the orchestrator's), the `sso` browser
  project, the release load run (15 GB of disk; DEC-607 says why it is not required).
- Left in place: `target/site` and `target/debug/server` (the final cargo-leptos build).
  Executables in `target/debug/deps` deleted; all `test_*` and `ot1b_*` databases dropped.

## Noticed, not built

- The re-check of the ranking after DATA-0. (Popularity from the sources, OT-3 step 3, was
  built with the review fixes: DEC-611.)
- Every page loads a cover that is not stored yet from the provider's host: `widgets.rs`
  `Cover` uses `src=img.url`, and it is what the item page, the Library cards and Home's
  Continue and Up next cards render (during an import: every new card in every open browser
  of that user). The stored-only rule of the palette (DEC-608) is a privacy default, not a
  requirement of the providers' terms; the owner's ruling would cover these pages too
  (corrected after review OT1B-2).
- A palette row does not change when its cover is stored while the palette is open; recent
  items in the idle list keep no kind or cover, so they have no leading box.
- A library title our index did not find appears in its source's group, not under "In your
  library" (DEC-609).
- The typo phase of the Postgres search has no title tie-break.
- Corrected after review OT1B-2: Notifications does not listen to `entity.updated` (it
  refetches on `notification.new`). The pages that still refetched for every
  `entity.updated` were People, the person page and both list pages
  (`people::refetch_on_update`); they are paced since the review fixes (DEC-614).
- Under a forced generic plan the search statement takes 42 to 56 ms on the load data, more
  than DEC-493's psql timings; which plan the application gets was not measured (DEC-607).
- The dev Meilisearch holds 171 indexes: the test prefixes of `search/tests/suite.rs` runs are
  never deleted.

## Review fixes (2026-11-12, docs/reviews/2026-11-12-ot1b-review.md)

Resume notes: nothing committed. Scratch scripts (seed.sql, explain5.sql, clean.sh) are in the
session scratchpad and may be gone; the statements are in `search/src/pg.rs`. Next free DEC at
the start: 611. Order of work: OT1B-3, 4 (done, below), then web (5, 7, 8, 9, 11, 12, 13),
then OT1B-1 mutation runs, OT1B-2 texts, DECs, gates, whole chromium project.

### OT1B-3 (applied, DEC-611) popularity as the tie-break after the title rules

- Sources checked: TMDB search and detail answers carry `popularity` (number) and `vote_count`
  (integer) (reference page search-movie read 2026-11-12; our fixtures are TMDB's example
  answers and hold both). TheTVDB `SearchResult` (swagger.yml 4.8.0, read 2026-11-12) has no
  such field; `SeriesBaseRecord.score` exists on details only ("no assumptions should be made
  about the meaning of this value"). IGDB docs answered 403 again: not read, nothing built.
- Built: `catalog_items.popularity double precision` (migration 0033, additive),
  `CatalogItem.popularity`, `NewCatalogItem.popularity`, `CatalogRefresh.popularity`,
  `Snapshot.popularity`, `Merged.popularity`; TMDB sets it from `vote_count` (search and
  details, movies and shows); a search result fills a missing value on a kept copy and leaves
  a stored one to the refresh; a purge clears it. The catalog search document carries
  `popularity`. Postgres: `ORDER BY whole, lead, pop DESC NULLS LAST, score, id`. Meilisearch:
  `popularity:desc` appended to the server's own ranking rules of the catalog index at
  connect (after exactness). `pg::weighted` leaves out an original title equal to the title.
- Tests: `search/tests/suite.rs` `popularity_breaks_title_ties` (the owner's pair plus two):
  RED on both backends before the code (Postgres `["p2","p1","p3","p4"]`, Meilisearch
  `["p2","p3","p1","p4"]`, the derivative first on both), green after. Written before the
  code but first run after it (not seen red): connectors `tmdb.rs` (four `popularity`
  assertions), app `catalog_audience::a_search_result_fills_a_missing_popularity_and_keeps_a_stored_one`,
  store-pg `retention.rs` (stored, refreshed, purged), search `indexer.rs` (document field).
- Runs: `cargo test -p search` 3 + 1 + 2 passed; `-p connectors --features tmdb --test tmdb`
  10 passed (1 ignored); `-p app --test catalog_audience` 9; `-p catalog-core --lib` 11;
  `-p store-pg --test retention --test catalog_race --test catalog_audience` 2 + 2 + 2.
  `cargo xtask sqlx-prepare` run.
- EXPLAIN (ANALYZE, BUFFERS), PostgreSQL 18.6, same seed as DEC-607 (50 551 documents, half
  of them with a popularity; second database with 600-character descriptions, 194 MB),
  prepared, `force_generic_plan`, warm, four runs, ot1b statement / final statement in ms.
  Titles only: "amber cedar" 41.9-42.5 / 42.9-43.4; "cedar ember 4242" 49.5-51.1 / 49.2-50.2;
  "granit" 62.3-63.7 / 72.8-74.1; "juniper delta 777" 51.5-51.9 / 49.9-51.4; "indigo"
  59.1-59.8 / 69.3-70.1. With descriptions: 65.6-68.1 / 67.3-68.2; 71.6-73.6 / 71.3-72.9;
  85.0-86.3 / 98.0-100.6; 70.4-72.6 / 70.8-73.8; 80.6-81.6 / 91.8-92.8. So +10 to 13 ms on
  the two one-word queries that match 9 600 documents, nothing on the others; same scan and
  join nodes (Seq Scan + join filter, top-N heapsort), two Subquery Scan nodes added. Of that
  the popularity term is about 1 ms, the article rule (one `regexp_replace` per matching row)
  the rest; variants measured and not faster: LATERAL without `OFFSET 0` (76 ms, expressions
  evaluated several times), an article list compared from the query side (67-71 ms).
- Owner's case: holds on both backends once the two copies carry a value. Copies kept before
  this change have none; they get it the next time TMDB names them in a search answer or
  refreshes them. Until then their order is the old one.

### OT1B-4 (applied on Postgres, DEC-612) leading articles

- `pg.rs`: `whole` and `lead` also compare the title without one leading article (the, a, an,
  der, die, das, ein, eine); the title as written is compared too ("the thing" finds "The
  Thing" first).
- Test `suite.rs` `leading_articles`: on Postgres red before the code is not shown separately
  (it ran after `popularity_breaks_title_ties`, which failed first), green after: "avengers"
  gives "The Avengers" first and "Lego Avengers" last, "pate" gives "Der Pate" first, "insel"
  "Eine Insel" first. Meilisearch 1.56 (recorded by the same test, not changed): "The
  Avengers" comes after the titles that start with the word, level with "Lego Avengers"
  (`["a2","a3","a4","a1"]`); "pate" gives "Pate wider Willen" before "Der Pate".
- OT1B-4 mutation run (article pattern replaced by one that never matches, `sqlx-prepare`,
  `cargo test -p search --test suite postgres`): RED, `found[0]` was "a3" instead of "a1"
  ("the title that is the query but for its article"). Restored, `sqlx-prepare` again.

### OT1B-5, 7, 8 (applied, DEC-613) the palette adds rows, it does not move them

- `palette.rs`: `local_rows` returns the rows to list and the rows held back: copies of a
  source that is being asked wait for that source's answer (`HOLD_MS` = 700 ms; an event of
  that source and kind releases them, the timer releases the rest in our order). `merge`
  never moves a row on screen: new rows go to the end of their group when that is below the
  `pinned` rows (everything down to the active row; the first row while nothing is chosen),
  else to the end of the list. `contiguous` keeps each group in one run (OT1B-7). `Placed`
  is gone (membership in the list is the check).
- `ui::Command`: rows are keyed (`<For>` by item id, label, heading, leading), so rows that
  stay keep their elements and covers (OT1B-8); the active mark follows the item id; new
  optional prop `active_item` tells the caller which item is active.
- Tests. web `palette::tests::library_entries_lead_and_a_source_keeps_its_order` (rewritten:
  held rows, nothing moved, one heading per group) and new
  `a_late_answer_adds_rows_below_the_active_row`: both RED in a mutation run (`merge`
  ignoring `pinned`: `["homeless","stone","chamber","b1"]` instead of `[..,"b1","chamber"]`;
  `local_rows` without `contiguous`: "show" stood between "man" and "note"), green after.
  ui `command::tests::a_heading_leads_each_run_of_a_group` (not seen red). ui wasm
  `command_rows_keep_their_elements_when_the_list_grows`: RED in a mutation run (a row key
  that changes per render: "w-cmd6-opt-heat kept its picture element" failed), green after.
- DEC-426/499/500/503/508: untouched code paths (`item_page` at key time, `Stream`,
  `filter_as_typed`, `follow_chosen`); their tests still pass (ui wasm 24, web lib 213).

### OT1B-9 (applied, DEC-614) own actions are not paced; OT1B-11 (applied) People and lists

- `events.rs`: `OwnAction` (a counter in context), `Pace::own`; `refresh_on_updates`
  refetches at once when the counter moves and does not count the event that was the last
  one when the page was built. `next_step.rs`: `StepCtx::acted`, called when an action went
  through and no page asked for a refresh (the palette's Ctrl+Enter add and step, the Undo
  of their toasts). Test `events::tests::an_own_action_counts_as_a_refetch` (written with
  the code, not seen red). The effect itself (mount run skipped, refetch on the counter) has
  no unit test; the browser suite covers the pages.
- `people::refetch_on_update` calls `events::refresh_on_updates`: People, the person page
  and both list pages are paced like Home.

### OT1B-12 (applied), OT1B-13 (applied)

- New `crates/web/tests/wasm.rs` (browser, `cargo test -p web --target
  wasm32-unknown-unknown --features hydrate --test wasm`; dev-dependency
  `wasm-bindgen-test` for wasm32; added to `xtask ci` as step `test-wasm-web`, not run
  through xtask here): `a_cover_stays_for_equal_data_and_changes_with_its_picture`,
  `a_keyed_row_keeps_its_shell_and_leaves_with_its_key`. First run: the cover test failed on
  my own wrong assertion ("a new picture, a new element"): the renderer patches the `img` in
  place when `StableCover` is given another picture, it does not make a new element. The
  assertion now checks what the element shows (src, source, alt, the box without a
  picture). 2 passed.
- `widgets::Cover` doc comment: callers show the title as text next to or under every cover.

### Runs so far (this section's code)

- `cargo test -p web --lib` 213 passed; `cargo test -p ui` 18 + 31 + 17; `cargo test -p ui
  --target wasm32-unknown-unknown` 24 passed; web wasm 2 passed.
- Clippy pedantic wasm (`-p web -p ui --all-targets`, hydrate + gallery) and native
  (`--workspace --all-targets`, CI features): clean. `cargo fmt --all`: clean.
- Disk: 4.8 GB free after the web wasm test build (1.1 GB); its artefacts in
  `target/wasm32-unknown-unknown/debug/deps` (libweb, web.wasm, test wasm files) deleted,
  5.1 GB free. Test databases and this run's Meilisearch test indexes deleted after each run.

### OT1B-1 (applied, DEC-617) mutation runs of the OT-21 browser test

Each on a full `cargo leptos build`, test `home.spec.ts` "covers stay while an import runs".
- `Pace::QUIET_MS = 0.0`: RED at the pacing bound: 34 Home refetches for 81 events in
  948 ms, bound 3.
- `StableCover` reading `of.get()` instead of its memo: NOT red (2 passed). The renderer
  patches the `img` in place; the memo is not what keeps the element. Said in the doc
  comment of `StableCover`, in DEC-610 and DEC-617.
- `widgets::keyed` with a key that changes per answer (the review's other form): RED at
  the identity checks ("the show's card": "replaced").
- Test changes: floor `homeCalls - calls >= 1`; the card's quick menu is opened after the tag
  arrived and its tag dialog shows "Remove tag rainy"; the picture is still the same element.
- All three mutations reverted (files compared with their saved copies).

### OT1B-2 (applied) texts

DEC-608 and DEC-610 corrected in place, "Noticed, not built" above corrected (Library and
Home cards load uncached covers from the provider's host too; stored-only is a privacy
default; Notifications does not listen to `entity.updated`; People, person page, lists).

### OT1B-6 (deferred, DEC-615), OT1B-10 (deferred, DEC-616)

Both wait for the W2 load run. No code.

### After the first whole browser run

- First whole chromium run on the review-fix build: 147 passed, 1 FAILED: shell.spec.ts
  "palette: the library leads ...": the first row was "Breaking Bad (2008)". Cause in the
  app, not the test: with the hold, the group of whichever source answered first stood
  first. Fix: `palette::Hold`: answers that arrive while copies are held wait with them and
  are listed in one addition, groups in the order of our own hits (DEC-613). Test
  `palette::tests::answers_wait_while_copies_are_held_and_keep_the_order_of_our_hits`, RED in
  a mutation run (release without the order), green after. No test was changed for this.
- Meilisearch: its page on custom ranking rules says the order is undefined for documents
  without the attribute, so the indexer writes `popularity` as a number in every catalog
  document (0 when unknown); operations.adoc "Upgrade notes" asks for one `cli reindex`.

### Final runs (2026-11-12, review fixes)

- `cargo fmt --all --check`: clean. Clippy pedantic: wasm (`-p web -p ui --all-targets`,
  hydrate + gallery) clean on the final code; native `--workspace --all-targets` (CI
  features) clean before the last two changes (indexer `unwrap_or(0.0)`, `palette::Hold`),
  after them `-p search --all-targets` and `-p web --all-targets` clean.
- `cargo xtask sqlx-prepare`: run after the last SQL change.
- Tests, all passed: domain 37; kinds 5 + 2; catalog-core 11 + 4 + 2 + 11 + 7 + 1;
  connectors (all features) 2 + 3 + 2 + 10 (1 ignored) + 6; app 5 + 22 test files (119);
  store-pg 1 + 34 in 12 files; search 3 + 1 + 2 (again after the indexer change); api
  13 + 22 + 1; importers (all) 12 + 7 + 8 (1 ignored) + 1 + 1 + 3; integrations 4 + 5;
  xtask whitelabel 2, locales 5; ui 18 + 31 + 17; ui wasm 24; web wasm 2; web lib 214 (final).
- Server `it` (`item_page:: library_page:: home_page:: auth_pages:: admin_pages::
  member_pin:: instance:: admin:: setup_page:: latency:: retention:: imports::`): 56 passed
  (run before the `palette::Hold` change, which touches client code of the palette only).
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`,
  then the whole Playwright `chromium` project on that final build (database
  `ot1bfix_full2`, 2 workers, server log captured): 148 passed (setup 1 + chromium 147),
  0 failed, 0 flaky, 0 `panicked at`, 7.4 minutes.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, the release load
  run. The new `xtask ci` step `test-wasm-web` was not run through xtask (its command was).
- Disk: lowest 1.8 GB free during the server `it` build; 5.0 GB at the end. Deleted: test
  executables in `target/debug/deps`, the web wasm test artefacts, all `test_*` and
  `ot1bfix_*` databases, the Meilisearch test indexes created by this run (18). Left:
  `target/site`, `target/debug/server` (final build), `src/web/test-results` (traces of the
  failed runs; a removal was refused by the tool's safety check).
