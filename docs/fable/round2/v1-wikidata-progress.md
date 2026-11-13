# v1-wikidata progress (lane B, worktree kithkeep-lane-books, branch lane/v1-wikidata)

Spec: docs/fable/round2/v1-plan.md section 4 and 4b. Lane rules:
docs/handoff/fable-briefs/LANES.md (main checkout). Decisions of this package:
docs/fable/round2/v1-wikidata-decisions.md (LB-23 onwards). Nothing is committed.
Wrapper scripts: <scratchpad>/lane-b-wikidata/
(lbw-cargo.sh names the worktree by absolute path).

## Items

| # | Item | State |
|---|---|---|
| 1 | docs/connectors/wikidata.md (licence, terms, UA policy, limits, endpoints, classes) | done |
| 2 | Connector `wikidata` (feature, info, policy, registry, kinds default) | done |
| 3 | Search (class-filtered `list=search`, then one `wbgetentities`) | done (no fallback, LB-23) |
| 4 | Details (entity, then labels of linked items) | done |
| 5 | Mapping table | done |
| 6 | Attribution, outbound links Wikidata / BoardGameGeek | done |
| 7 | Facts in words (players, time, age) en + de | done |
| 8 | needs_contact on sources page / palette footer / Home nudge | done |
| 9 | Playwright mock + boardgames.spec.ts, contact.spec.ts | written, run pending |
| 10 | docs (bgg.md line, connectors page, v1-scope) | done |

## Log

- 2026-11-13 (first agent, cut off by the usage limit): live checks with a generic
  User-Agent (scratch `wd/`): the class tree of Q131436 with instance counts (one SPARQL
  query at development time, as the plan allows), the class-filtered `list=search` with
  free text and several `P31=` joined by `|` WORKS (Catan 49 hits, Wingspan 1, Zug um Zug 3,
  Azul 7, nothing 0), `maxlag` error shape (HTTP 200 + error object), `no-such-entity`
  shape, unknown `languages` value gives a warning only. So the fallback
  (`wbsearchentities`) is NOT needed. Fixtures recorded (9 files, trimmed claims),
  `tests/wikidata.rs` (22 tests) and a skeleton `src/wikidata.rs` written.
- 2026-11-13 (this agent, resumed): no progress log existed; written now.
  - Red runs (CARGO_BUILD_JOBS=1): `cargo test -p connectors --features wikidata --test
    wikidata` against the skeleton: 20 of 20 failed; then the new test
    `search_syntax_in_the_typed_text_is_sent_as_words` alone: failed. kinds
    `builtins::descriptor_schema_and_facets` (expects `wikidata`): failed. web
    `item_words::` 3 new tests and `source_hints::` 2 (one new, one extended): 5 failed.
  - Implemented `src/wikidata.rs` (live check of the class labels: Q1150710 is "strategy
    game", Q1272194 "tile-based game", not what the first agent's notes guessed). Green:
    connectors wikidata 22 + 3 unit tests (`the_registry_asks_wikidata_for_board_games_once_
    the_contact_is_set` written after the code, not seen red), kinds 5+2, web item_words and
    source_hints 29.
  - Server: feature `wikidata` (in `all-connectors`), registry with `connectors.wikidata.
    base_url`. Web: About tab facts in words, outbound links in the attribution footer,
    footer row with kinds, `Nudge::NeedsContact` (before NoSource). Locales en + de. Contact
    hint names Wikidata and the typed words. `.env.example`, config comment
    (configuration.adoc regenerated, config 18 passed).
  - Playwright: `tests/wikidata-mock.mjs` (port 3016), `boardgames.spec.ts`,
    `contact.spec.ts` in a new project `contact` after chromium (LB-32); global setup sets
    the contact; books.spec restores it; sources.spec moved to Listen.
  - Docs: docs/connectors/wikidata.md (policy pages re-read live as wikitext, rate limits,
    class list), connectors.adoc, v1-scope.adoc (Book and Board game rows), bgg.md line.
    Decisions LB-23..LB-33.
  - fmt done; pedantic clippy native: one doc-markdown error fixed, then clean; wasm clippy
    (web, ui): clean.
  - `UPDATE_DOCS=1 cargo test -p config@0.1.0`: 18 passed (configuration.adoc regenerated).
  - One command, `SQLX_OFFLINE=true`, service env (`TEST_DATABASE_URL=postgres://app@
    127.0.0.1:55432/postgres REQUIRE_SERVICES=1 TEST_MEILI_*`): `cargo test --no-fail-fast
    -p connectors -p kinds -p catalog-core -p config@0.1.0 -p xtask -p web -p server
    --features connectors/all,server/gallery,server/all-connectors,web/ssr`: all passed
    (catalog-core 13+5+2+15+1+7+1, connectors 6 unit + igdb 3, openlibrary 12, podcastindex
    2, tmdb 10, tvdb 6, wikidata 22; kinds 5+2; config 18; web 235+3; xtask 22+1+6+3+5+19+2
    incl. `locales` 5; server lib 26, server `it` 126 passed, 2 ignored, which includes the
    SSR page modules item_page, library_page, home_page, auth_pages, admin_pages).
  - `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`:
    done.
  - Playwright (server `target/debug/lb-server`, ports 3220-3226, fresh `lb_e2e`), first
    whole run (`--project=contact`, i.e. setup + chromium + contact): 154 passed, 2 flaky, 10
    failed, contact not run; all ten "Test timeout of 30000ms exceeded" (load average 11 to
    15, other lanes compiling), in admin.spec (2), books.spec (contact test, at axe),
    gallery.spec (6), integrations.spec (1). Rerun of admin, books, gallery, integrations,
    boardgames and sources specs on a fresh database: 40 passed in 4.6 min. contact.spec
    alone after a setup run: 1 passed.
  - Test databases: 250 new `test_%` databases appeared between before and after the cargo
    run on the shared server, mixed with other lanes' runs; none dropped (owner cannot be
    told from the name).
  - FINAL whole run on the final build (`CI=1 npx playwright test --project=contact`: setup
    + the whole chromium project + contact; fresh `lb_e2e`, ports 3220-3226, load 9 to 10):
    165 passed, 1 failed in 21.0 min, no `panicked at` in the server log. The one: sharing.spec
    "promote, accept, see history, unlink, merge and undo the merge", "Test timeout of 30000ms
    exceeded" at a click, on both tries: the same test timed out in both v1-books runs
    (LB-22, reported there); not changed (lane rule). Because of it the `contact` project did
    not run in this run; contact.spec passed on its own on the same build (above).
    boardgames.spec, books.spec, sources.spec passed. `lb_e2e` dropped; no lb-server left.

State: every scope item done except the "by hand" checks (v1-manual, LB-33). Nothing committed.

## Review fixes (docs/reviews/2026-11-13-v1-wikidata-review.md, WD-1..13)

Decisions LB-34..LB-42 in v1-wikidata-decisions.md. Nothing committed; main not merged in.

| Finding | State | Tests (seen red first) |
|---|---|---|
| WD-1 class list | done: +8 classes (39); coverage 5155/5251 = 98.2 % (was 92.4 %), measured live; filter 513 chars, longest search accepted; method in wikidata.md | `the_class_list_holds_the_sibling_classes_of_common_games` (red) |
| WD-2 language | done: member's chosen locale on search, else instance language (brand `default_locale`, on the `Registry`); refresh asks in the instance language; Wikidata asks `<lang>\|en\|mul` + 18 more, a German-only hit is kept; docs and LB-28 superseded by LB-35 | connectors `without_a_language_a_hit_with_a_german_label_only_is_kept`, catalog-core `the_search_carries_the_members_language_else_the_instances`, `a_refresh_asks_in_the_instance_language`, server `search_language::` (all red) |
| WD-3 matching keys | done: only the QID is an external id; `bgg_id` and new `gtins` are attributes (kind schema regenerated); BGG link from the attribute | `two_items_that_share_a_bgg_id_stay_separate` (red), updated recorded tests, web `a_game_links_to_wikidata_and_boardgamegeek` (red) |
| WD-4 maxlag pause | done: `HttpFetch::pause`, connector pauses for Retry-After / lag / 5 s; governed client holds or fails fast | `a_maxlag_answer_pauses_the_source`, `a_pause_asked_by_a_connector_holds_the_next_request` (red) |
| WD-5 whole chain | see the run below | - |
| WD-6 gzip | done: reqwest `gzip` for the workspace | `requests_ask_for_a_compressed_answer` (red) |
| WD-7 contact once | done (refactor in `fetch`) | no red test possible (race window); covered by existing contact tests |
| WD-8 spec log | done: mock logs words and ids; spec reads only its own requests after its start and compares the exact contact | e2e |
| WD-9 cache key | documented (LB-42), not changed | - |
| WD-10 "no" answer | done: "Don't remind me" on the contact nudge (per browser), hint names switching Wikidata off | `only_the_contact_nudge_can_be_declined_and_a_declined_one_stays_hidden` (red) |
| WD-11 wording | done: de label "Mitspielende" | web game-facts tests (red) |
| WD-12 stale names | done: `bgg` in web test data renamed | - |
| WD-13 red runs | every new behaviour above was seen red; contact.spec red run not done (needs two extra full builds) | - |

The sharing.spec timeout (first planned as an orchestrator exception) was withdrawn by the
orchestrator: the failure is the people-menu race fixed in lane R; sharing.spec is unchanged.

Fixtures: request URLs rewritten for the 39 classes and the wider languages (bodies as
recorded, noted in each fixture and in wikidata.md).

Runs (CARGO_BUILD_JOBS=1, `SQLX_OFFLINE=true`, wrappers in the lane's scratch directory):
- Red: connectors wikidata 7 failed of 26; catalog-core credentials 1, fanout 1, governed 2;
  web 4 (item_words 3, source_hints 1); server `it -- search_language::` 1 (`[None]`).
- Green: connectors `--features all` (unit 6, igdb 3, openlibrary 12, podcastindex 2, tmdb
  10 + 1 ignored, tvdb 6, wikidata 26); catalog-core (13 + credentials 6, fanout 3,
  governed 17, pace 1, retention 7, stats 1); kinds 4 + 2 after `UPDATE_SCHEMAS=1`
  (board_game.json gains `gtins`); web `--features ssr --lib` 236; api 13 + rest 24 + sync 1;
  xtask incl. `locales` 5, all passed; server `it -- item_page:: library_page:: home_page::
  auth_pages:: admin_pages:: search_language:: books::` 31 passed.
- `cargo fmt --all`; clippy pedantic native (workspace, NATIVE_FEATURES) and wasm (web, ui):
  clean.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Playwright, whole chain (setup -> chromium -> contact -> sso), `lb-server`, ports
  3220-3226, fresh `lb_e2e`: run 1: 163 passed, 1 flaky (shell.spec late-batch palette test),
  2 failed, 12 did not run (contact, sso): sharing.spec "promote, accept, ..." (the known
  people-menu race, fixed in lane R) and settings.spec "every settings page reflows and passes
  axe at 360 px" (30 s timeout inside axe, both tries, load 10-16; not touched by this
  package). Run 2 as the orchestrator directed, `--grep-invert "promote, accept, see
  history"`: **178 passed, 0 failed, 0 flaky in 25.1 min**, contact and sso included; no
  `panicked at` in the server log. `lb_e2e` dropped; no lb-server left.
- Test databases: 32 new `test_%` databases between before and after the server `it` run on
  the shared server (other lanes ran at the same time); none dropped.
