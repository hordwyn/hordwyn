# ot2f progress (plays on game pages and in the stats; OT-18, OT-19, OT-20)

Resume notes for a cut-off run. Brief: docs/handoff/fable-briefs/PKG-implement.md.
Nothing is committed. Build commands: `nix develop -c bash -c 'cd src/rust; CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 cargo test ...'`.
Playwright by hand as in ot2e-progress.md (ports 3110 to 3114, databases `ot2f_<n>`).
Service env for store-pg and the server `it` binary:
`TEST_DATABASE_URL=postgres://app@127.0.0.1:55432/postgres REQUIRE_SERVICES=1`.

| # | Item | State | Tests (red first) | Files | DEC |
|---|---|---|---|---|---|
| 1 | OT-18 co-op play without a result: `CoopOutcome::NoResult`, stored and sent as `none`; migration 0032 widens the check; API takes `coop: "none"`; play dialog: "Co-op game" switch, then "How it ended" with No result (default) / We won / We lost; rows read "Played together" | done | domain `session::tests::*` (3): red against stubs, green. app `plays::a_coop_play_without_a_result_is_kept_and_decides_nothing`: written after the domain code, green from its first run (not seen red). store-pg `play_totals_on_postgres_match_the_rule_in_rust`: red without migration 0032 (check constraint `sessions_coop_outcome_check`), green with it. web `log_form::tests::the_play_sheet_sends_players_scores_and_the_winner` (extended; written with the code, not seen red), `history::tests::a_play_row_says_who_won_the_scores_and_how_long` (extended): red ("90 min"), green | domain/src/session.rs; store-pg/migrations/0032_coop_no_result.sql, src/conv.rs; api/src/ops.rs, browse.rs; app/src/people.rs; web/src/log_form.rs, log_sheet.rs, history.rs; locales | 595 |
| 2 | Play totals: `Session::decided` / `result_for` (domain), port `SessionRepo::play_totals` with the rule in Rust as default (`fold_play_totals`), one grouped statement in store-pg, `App::item_play_totals` / `play_totals` (`app/src/plays.rs`) | done | app `tests/plays.rs` (3): 2 red against stubs, green; `--test people` 9 passed. store-pg see item 1 (the statement is compared with the rule for four sets of own entries and three scopes); `--test phase1`: 10 passed. `cargo xtask sqlx-prepare` run (1 new file in `.sqlx`) | app/src/plays.rs (new), ports.rs, lib.rs; app/tests/plays.rs (new); store-pg/src/repos/sessions.rs, tests/phase1.rs; .sqlx | 596 |
| 3 | contracts + api: `PlayTotalsDto`, `ItemPageDto.play_totals` (played kinds only), `StatsCompareDto.play` | done | api `rest::a_play_with_players_scores_and_a_winner` extended by `play_totals_are_read` (written with the mapping, not seen red): `--test rest` 21 passed | contracts/src/browse.rs, lib.rs, goals.rs; api/src/ops.rs, browse.rs, goals.rs; api/tests/rest.rs | - |
| 4 | web: "Plays" tab on game pages (strip + table, cards on a phone), the page opens on it once a play exists; fact line ends with the time played | done | web `plays_tab::tests::*` (2): red against stubs, green. `item_words` fact line test extended for `with_play_time` (written with the code, not seen red) | web/src/plays_tab.rs (new), item.rs, item_words.rs, history.rs (`play_words`), lib.rs; locales | 597 |
| 5 | web: win rate on the Stats Play tab (`PlayResults`, under the totals row) | done | browser test only (item 10) | web/src/stats_extras.rs; locales | 596 |
| 6 | web: compact score table with a "Won" column (ux-owner-test 2.3); "Use the highest score" takes a choice back; the "Winner" select is gone | done | browser test (item 10) | web/src/log_sheet.rs; locales (`log-winner`, `log-winner-auto` removed) | 598 |
| 7 | OT-19: hero and tiles on one grid by a container query (`@2xl:grid-cols-4`), hero spans two tracks | done (also: top bar between 760 and 790 px; stray line not found) | browser test (item 10) | web/src/time_tiles.rs | 599 |
| 8 | OT-20: a tile with counts but no known time leads with the count and says what is missing | done | web `time_tiles::tests::a_tile_with_counts_but_no_known_time_says_so`: red, green | web/src/time_tiles.rs; locales | 600 |
| 9 | OT2C-4 (DEC-578) totals row against chart at a rounding edge | not built: ot2f does not change the stats read; stays with ot2g (DEC-601) | - | - | 601 |
| 10 | Playwright item.spec.ts (score table; Plays tab, co-op without a result, keyboard, axe, reflow, phone), stats.spec.ts (win rate), home.spec.ts (OT-19 edges, OT-20 text); server SSR `item_page::a_game_page_has_the_plays_tab_with_time_and_the_win_rate` | done | - | src/web/tests/item.spec.ts, stats.spec.ts, home.spec.ts; apps/server/tests/it/item_page.rs | - |
| 11 | Gates: fmt, clippy native + wasm, locale tests, server page tests, whole chromium project | done | see "Final runs" | - | - |

## Log

- OT-19 looked at first on the ot2e build (database `ot2f_0`, temporary spec
  `src/web/tests/zz-ot19.spec.ts`, TO BE REMOVED before the end):
  Home and Stats at 700 to 1280 px, light and dark. The four tiles never left their column
  (grid tracks are `minmax(0, 1fr)`), no line beside "All-time stats" in either theme. Found
  instead: (a) the hero tile is `md:w-1/2` of the block while the tiles sit on a grid with a
  gap, so the hero's right edge is 6 px right of tile 2's edge ("not aligning"); (b) at 770 px
  the document is 780 px wide (the top bar overflows by 10 px between 760 px, where the nav
  appears, and about 790 px), so the page scrolls sideways there; (c) the tiles follow the
  viewport (`md:`), not their column.
- Web unit tests after items 1 to 8: `cargo test -p web --lib` 204 passed; `cargo test -p xtask
  --test whitelabel --test locales` 2 + 5 passed (a `format!` key family was refused by the
  locale check, the tile notes use static keys now). `cargo fmt --all` run.
- Still to do: clippy native + wasm, `cargo leptos build`, server `it` page tests, new browser
  tests, whole chromium project, DEC entries (next free: 595), drop `ot2f_*` databases, remove
  the temporary spec.
- Clippy pedantic native (`--workspace --all-targets`, CI feature set) and wasm (`-p web -p ui
  --all-targets --target wasm32-unknown-unknown --features web/hydrate,web/gallery`): clean
  (one `similar_names` finding in `history.rs` fixed). Run before the last small edits
  (score column widths, top bar, stacked table border); to be rerun at the end.
- First `cargo leptos build`, then the new and changed browser tests alone (database
  `ot2f_1`): the Plays tab test failed three times on mistakes in the test itself (a video
  game's button is "Log a session"; a stacked cell's text includes its hidden column name;
  the arrow keys start from the selected tab), then passed. Screenshots looked at (1280 and
  390 px): the score field was as wide as its cell (`w-24` lost against the field's `w-full`;
  now `max-w-24` and fixed column widths), and the last card of the stacked table had no
  border (fixed in `ui`, DEC-601).
- OT-19, top bar: new assertion in `home.spec.ts` (no sideways scroll at 760, 770, 790 px)
  seen red on the build before the fix ("document by 20px" at 760 px), green after.
- Server `it` page tests (`item_page:: library_page:: home_page:: auth_pages:: admin_pages::`,
  service env): 23 passed, with the new `item_page::a_game_page_has_the_plays_tab_with_time_and_the_win_rate`
  (written after the code, not seen red).
- `cargo test -p web --lib`: 204 passed; `cargo test -p ui`: 17 + 31 + 17 passed (after the
  last edits). `cargo fmt --all --check`: clean.
- NOTE for whoever runs the browser tests: `cargo test -p server --test it` rebuilds
  `target/debug/server` with other features (563 MB); pages then do not hydrate against
  `target/site`. Run `cargo leptos build --bin-features gallery,all-connectors --lib-features
  hydrate,gallery` again before Playwright (done here; the first whole run failed in the
  setup for this reason, 1 failed, nothing else ran).
- DEC-595 to DEC-601 written. Temporary spec removed; databases `test_*`, `ot2f_0`, `ot2f_1`
  dropped; test executables in `target/debug/deps` deleted after each run.

## Final runs (2026-11-11), all items done

- Final build: `cargo leptos build --bin-features gallery,all-connectors --lib-features
  hydrate,gallery` after the last Rust and locale change; no Rust source or locale file
  changed after it (two expectations in `item.spec.ts` did).
- Whole Playwright `chromium` project on that build, first complete run (database `ot2f_2`,
  2 workers, ports 3110 to 3114): 142 passed, 2 failed, 0 `panicked at` lines. Both failures
  were older tests in `item.spec.ts` that counted the removed "Winner" select as an input
  ("the play sheet shows the duration in its first row ...", "the log dialogs are centered
  ..."); they now count the one "Won" radio of the score table instead and passed alone
  (database `ot2f_3`, with "in a low window ..."): 4 passed with the setup.
- Whole Playwright `chromium` project again on the same build (database `ot2f_4`): 144 passed
  (setup included), 0 failed, 0 flaky, 7.2 minutes; 0 `panicked at` lines in the server log.
- Clippy pedantic native and wasm, rerun after the last edits: clean. `cargo fmt --all
  --check`: clean. `target/debug/server` not touched by clippy (same time stamp).
- `cargo test -p domain -p app -p api -p contracts -p store-pg -p testkit` with the service
  env: 45 result lines, 0 failed (domain 37; app `plays` 3, `people` 9, `activity` 7 and the
  rest; api 13 + 21 + sync; contracts 11; store-pg `phase1` 10 and the rest).
- Server `it` page tests: 23 passed (see the log; run before the final cargo-leptos build,
  which changed no source).
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the
  wasm browser tests of `ui`, `cargo test -p web --features ssr`, importers, the other
  modules of the server `it` binary, a release load run (DEC-601).
- Left in place: `target/site` and `target/debug/server` (the final build). Test executables
  in `target/debug/deps` deleted; the `test_*` and `ot2f_*` databases dropped; the temporary
  spec `zz-ot19.spec.ts` removed.

Noticed, not built (out of scope):

- The stray line beside "All-time stats" (OT-19) was not found; needs the owner's screenshot
  (DEC-599).
- There is no way to edit a logged play: a play logged as "Lost together" before OT-18 stays
  so until it is undone and logged again.
- `/stats` still marks Home as current in the top bar (noticed in ot2d).
- The Plays table has no place and notes columns; `ItemPlayDto` does not carry them.
- The win rate is all time; the Stats page has no year filter for it.
- `docs/design/ux/design.md` 5.4 still describes the winner as a derived, overridable field
  beside a co-op switch (input document, not edited).
- A full `cargo test -p server --test it` after `cargo leptos build` replaces
  `target/debug/server` by a build that does not hydrate against `target/site`.

## Review fixes (2026-11-11, review docs/reviews/2026-11-11-ot2f-review.md)

Resume notes: nothing committed. Playwright by hand as above (databases `ot2f_r<n>`, ports
3110 to 3114). DEC entries for the review start at 602 (check the last number first).

- Browser tests written first and run on the build of the package (before any fix, database
  `ot2f_r1`), all red as expected:
  - OT2F-1 (temporary spec with the steps alone): after Ben left and Dana joined, a click on
    the padding of Dana's "won" target checked Cleo's radio ("Expected: not checked,
    Received: checked").
  - OT2F-3 (same temporary spec): after Enter on "Use the highest score" the checked radio
    was not focused ("Received: inactive").
  - OT2F-2 `stats.spec.ts` "plays without details are counted ...": "Expected: Plays 3,
    Received: Plays 0".
  - OT2F-4/6/11 `item.spec.ts` "the score table: one id per player ...": no "No winner"
    radio; the two density tests: 1 radio instead of 2.
  - OT2F-10 `i18n.spec.ts` (German top bar at 760 px): "document by 11px", a real defect
    (the German search label). The English widths 760 to 832 px in `home.spec.ts` passed.
  The temporary spec `zz-red.spec.ts` is removed.
- Web unit tests, red against stubs (`cargo test -p web --lib -- log_form plays_tab
  stats_extras`: 4 failed, 20 passed), then green (`cargo test -p web --lib`: 208 passed):
  `log_form::tests::a_single_score_names_no_winner_and_nobody_can_be_chosen` (OT2F-4),
  `log_form::tests::a_chosen_winner_who_left_the_table_is_no_choice` (OT2F-5),
  `plays_tab::tests::plays_without_details_are_counted_and_named` (OT2F-2),
  `stats_extras::tests::the_results_count_the_plays_of_the_totals_row` (OT2F-2).
- Code so far (web): `log_form.rs` (`WinnerChoice`: Auto / Nobody / Player; one score alone
  marks nobody; a chosen winner who left is no choice), `log_sheet.rs` (ids from the
  companion id; "No winner" row; hint tied to the table; announcement; focus after "Use the
  highest score"; `table-fixed`), `plays_tab.rs` (`play_count`, `bare_note`), `item.rs` (tab
  count from the history), `stats_extras.rs` + `home.rs` ("Your results" counts the plays
  of the totals row), `shell.rs` (search label is cut when the bar is full), locales en/de
  (`log-winner-none`, `log-winner-hint`, `log-winner-live`, `log-winner-live-none`,
  `plays-bare`; `plays-rate-detail` ends "in your own log", OT2F-8).
- Still to do at this point: OT2F-13/14/15 (words, test split, `win_rate`), the play read
  per case + sqlx-prepare + EXPLAIN, server SSR test, operations.adoc (OT2F-9), DECs, clippy,
  cargo-leptos build, Playwright (new tests, whole chromium, sso once).

### Review fixes: backend, done

- OT2F-13: words in `contracts/src/sharing.rs` (`PersonDto.decided`, in the OpenAPI document)
  and `app/src/people.rs` say "a co-op game won or lost".
- OT2F-14: the two long tests are split (`app/tests/plays.rs`: helpers `log`, `plays`;
  `store-pg/tests/phase1.rs`: helper `tally_plays`); both test functions are under 100 lines.
- OT2F-15: `app::PlayTotals::win_rate` removed (tests only); the web rounds the rate.
- OT2F-4, app: `plays::a_solo_play_has_a_result_only_when_it_is_marked_won` (the rule was
  already in the domain; green from its first run, not seen red; the dialog's side was seen
  red in `log_form` and in the browser).
- OT2F-2, server: `item_page::plays_without_details_are_counted_on_the_plays_tab` (written
  after the code; the same behaviour was seen red in the browser).
- Play read per case: `SessionRepo::play_totals` of store-pg runs `play_totals_of_item`
  (`WHERE s.owner_id = $1 AND s.item_id = $3`) or `play_totals_of_owner`; no
  `($3 IS NULL OR ...)`. `cargo xtask sqlx-prepare`: two new files in `.sqlx`
  (`query-82a395fe...`, `query-b040fe11...`), the old `query-3293cf2b...` is gone.
- Runs: `cargo test -p app --test plays --test people` 4 + 9 passed; `cargo test -p store-pg
  --test phase1` 10 passed (service env); `cargo test -p api --test rest` 21 passed; `cargo
  test -p contracts` 11 passed; `cargo test -p xtask --test whitelabel --test locales` 2 + 5
  passed; clippy pedantic native and wasm clean (one finding fixed: `winner_live` took
  `PlayState` by value); server `it` page tests (`item_page:: library_page:: home_page::
  auth_pages:: admin_pages::`): 24 passed.
- OT2F-9: `docs/modules/ROOT/pages/operations.adoc`, new section "Upgrade notes" (0032 is
  one-way for data written after it; the two statements to go back without a restore).

### EXPLAIN ANALYZE of the play read (2026-11-11)

Database `ot2f_explain` (migrated to 0032, seeded by SQL, analyzed, dropped afterwards),
PostgreSQL 18.6: 21 users, 300 board games, 45 000 sessions, 134 500 participants. The owner
looked at has 5 000 plays (1 000 of item 1, 13 or 14 of each other item), three own entries
(the self entry and two merged into it, so `ANY($2)` has three ids), one play in ten without
them; 20 other owners with 2 000 plays each over the same items. Every statement was run as a
prepared statement with `plan_cache_mode = force_generic_plan` (the plan a driver's prepared
statement can end up with), all buffers in cache; the numbers are of the second of two runs
(the first: 14.2 ms, 0.44 ms, 0.95 ms, 74.3 ms).

| Case | Statement | Sessions scan | Buffers | Time |
|---|---|---|---|---|
| A: item with 1 000 plays of the owner | per item (new) | BitmapAnd of `sessions_item` (1 120 rows) and `sessions_owner_recent` (5 000) | 9 855 | 14.9 ms |
| B: item with 13 plays of the owner | per item (new) | the same BitmapAnd, 13 rows | 181 | 0.44 ms |
| C: the same item, old form `($3 IS NULL OR item_id = $3)` | old | `sessions_owner_recent`, 5 000 rows, 4 987 removed by filter | 229 | 0.95 ms |
| D: every item, 5 000 plays | per owner (new) | `sessions_owner_recent`, 5 000 rows | 49 103 | 74.5 ms |

Per session two probes of `session_participants_pkey` (0.004 ms and 0.009 ms). The per-item
statement no longer reads the sessions of the other items (C against B). D is linear in the
owner's plays: about 15 ms per 1 000 plays on this host, on every `GET /stats/compare`.
The full output:

```
== A: one item with 1000 plays of the owner (new statement)
                                                                                   QUERY PLAN
 GroupAggregate  (cost=182.52..594.90 rows=1 width=48) (actual time=14.861..14.862 rows=1.00 loops=1)
   Buffers: shared hit=9855
   ->  Nested Loop  (cost=182.52..594.78 rows=7 width=27) (actual time=0.255..14.483 rows=1000.00 loops=1)
         Buffers: shared hit=9855
         ->  Nested Loop  (cost=166.47..482.27 rows=7 width=42) (actual time=0.244..4.870 rows=1000.00 loops=1)
               Buffers: shared hit=3955
               ->  Bitmap Heap Scan on sessions s  (cost=118.16..143.90 rows=7 width=40) (actual time=0.231..0.435 rows=1000.00 loops=1)
                     Recheck Cond: ((item_id = $3) AND (owner_id = $1))
                     Heap Blocks: exact=13
                     Buffers: shared hit=55
                     ->  BitmapAnd  (cost=118.16..118.16 rows=7 width=0) (actual time=0.181..0.182 rows=0.00 loops=1)
                           Buffers: shared hit=42
                           ->  Bitmap Index Scan on sessions_item  (cost=0.00..5.42 rows=150 width=0) (actual time=0.025..0.026 rows=1120.00 loops=1)
                                 Index Cond: (item_id = $3)
                                 Index Searches: 1
                                 Buffers: shared hit=2
                           ->  Bitmap Index Scan on sessions_owner_recent  (cost=0.00..112.49 rows=2143 width=0) (actual time=0.150..0.151 rows=5000.00 loops=1)
                                 Index Cond: (owner_id = $1)
                                 Index Searches: 1
                                 Buffers: shared hit=40
               ->  Aggregate  (cost=48.32..48.33 rows=1 width=2) (actual time=0.004..0.004 rows=1.00 loops=1000)
                     Buffers: shared hit=3900
                     ->  Index Scan using session_participants_pkey on session_participants p  (cost=0.42..48.31 rows=1 width=1) (actual time=0.003..0.003 rows=0.90 loops=1000)
                           Index Cond: ((session_id = s.id) AND (companion_id = ANY ($2)))
                           Index Searches: 1000
                           Buffers: shared hit=3900
         ->  Aggregate  (cost=16.05..16.06 rows=1 width=1) (actual time=0.009..0.009 rows=1.00 loops=1000)
               Buffers: shared hit=5900
               ->  Bitmap Heap Scan on session_participants p_1  (cost=4.44..16.05 rows=3 width=1) (actual time=0.005..0.006 rows=2.90 loops=1000)
                     Recheck Cond: (session_id = s.id)
                     Heap Blocks: exact=2900
                     Buffers: shared hit=5900
                     ->  Bitmap Index Scan on session_participants_pkey  (cost=0.00..4.44 rows=3 width=0) (actual time=0.003..0.003 rows=2.90 loops=1000)
                           Index Cond: (session_id = s.id)
                           Index Searches: 1000
                           Buffers: shared hit=3000
 Planning Time: 0.042 ms
 Execution Time: 14.900 ms
(38 rows)

== B: one item with 13 plays of the owner (new statement)
                                                                                  QUERY PLAN
 GroupAggregate  (cost=182.52..594.90 rows=1 width=48) (actual time=0.407..0.408 rows=1.00 loops=1)
   Buffers: shared hit=181
   ->  Nested Loop  (cost=182.52..594.78 rows=7 width=27) (actual time=0.213..0.399 rows=13.00 loops=1)
         Buffers: shared hit=181
         ->  Nested Loop  (cost=166.47..482.27 rows=7 width=42) (actual time=0.201..0.269 rows=13.00 loops=1)
               Buffers: shared hit=105
               ->  Bitmap Heap Scan on sessions s  (cost=118.16..143.90 rows=7 width=40) (actual time=0.189..0.200 rows=13.00 loops=1)
                     Recheck Cond: ((item_id = $3) AND (owner_id = $1))
                     Heap Blocks: exact=13
                     Buffers: shared hit=55
                     ->  BitmapAnd  (cost=118.16..118.16 rows=7 width=0) (actual time=0.181..0.181 rows=0.00 loops=1)
                           Buffers: shared hit=42
                           ->  Bitmap Index Scan on sessions_item  (cost=0.00..5.42 rows=150 width=0) (actual time=0.010..0.011 rows=153.00 loops=1)
                                 Index Cond: (item_id = $3)
                                 Index Searches: 1
                                 Buffers: shared hit=2
                           ->  Bitmap Index Scan on sessions_owner_recent  (cost=0.00..112.49 rows=2143 width=0) (actual time=0.164..0.164 rows=5000.00 loops=1)
                                 Index Cond: (owner_id = $1)
                                 Index Searches: 1
                                 Buffers: shared hit=40
               ->  Aggregate  (cost=48.32..48.33 rows=1 width=2) (actual time=0.004..0.005 rows=1.00 loops=13)
                     Buffers: shared hit=50
                     ->  Index Scan using session_participants_pkey on session_participants p  (cost=0.42..48.31 rows=1 width=1) (actual time=0.004..0.004 rows=0.85 loops=13)
                           Index Cond: ((session_id = s.id) AND (companion_id = ANY ($2)))
                           Index Searches: 13
                           Buffers: shared hit=50
         ->  Aggregate  (cost=16.05..16.06 rows=1 width=1) (actual time=0.009..0.010 rows=1.00 loops=13)
               Buffers: shared hit=76
               ->  Bitmap Heap Scan on session_participants p_1  (cost=4.44..16.05 rows=3 width=1) (actual time=0.005..0.007 rows=2.85 loops=13)
                     Recheck Cond: (session_id = s.id)
                     Heap Blocks: exact=37
                     Buffers: shared hit=76
                     ->  Bitmap Index Scan on session_participants_pkey  (cost=0.00..4.44 rows=3 width=0) (actual time=0.003..0.003 rows=2.85 loops=13)
                           Index Cond: (session_id = s.id)
                           Index Searches: 13
                           Buffers: shared hit=39
 Planning Time: 0.055 ms
 Execution Time: 0.443 ms
(38 rows)

== C: the same item with the old form ($3 IS NULL OR item_id = $3)
                                                                                     QUERY PLAN
 GroupAggregate  (cost=1860.41..1860.94 rows=17 width=48) (actual time=0.908..0.910 rows=1.00 loops=1)
   Group Key: s.item_id
   Buffers: shared hit=229
   ->  Sort  (cost=1860.41..1860.46 rows=18 width=27) (actual time=0.899..0.901 rows=13.00 loops=1)
         Sort Key: s.item_id
         Sort Method: quicksort  Memory: 25kB
         Buffers: shared hit=229
         ->  Nested Loop  (cost=176.86..1860.04 rows=18 width=27) (actual time=0.319..0.890 rows=13.00 loops=1)
               Buffers: shared hit=229
               ->  Nested Loop  (cost=160.81..1570.71 rows=18 width=42) (actual time=0.307..0.756 rows=13.00 loops=1)
                     Buffers: shared hit=153
                     ->  Bitmap Heap Scan on sessions s  (cost=112.49..700.64 rows=18 width=40) (actual time=0.296..0.687 rows=13.00 loops=1)
                           Recheck Cond: (owner_id = $1)
                           Filter: (($3 IS NULL) OR (item_id = $3))
                           Rows Removed by Filter: 4987
                           Heap Blocks: exact=63
                           Buffers: shared hit=103
                           ->  Bitmap Index Scan on sessions_owner_recent  (cost=0.00..112.49 rows=2143 width=0) (actual time=0.169..0.170 rows=5000.00 loops=1)
                                 Index Cond: (owner_id = $1)
                                 Index Searches: 1
                                 Buffers: shared hit=40
                     ->  Aggregate  (cost=48.32..48.33 rows=1 width=2) (actual time=0.004..0.005 rows=1.00 loops=13)
                           Buffers: shared hit=50
                           ->  Index Scan using session_participants_pkey on session_participants p  (cost=0.42..48.31 rows=1 width=1) (actual time=0.003..0.004 rows=0.85 loops=13)
                                 Index Cond: ((session_id = s.id) AND (companion_id = ANY ($2)))
                                 Index Searches: 13
                                 Buffers: shared hit=50
               ->  Aggregate  (cost=16.05..16.06 rows=1 width=1) (actual time=0.010..0.010 rows=1.00 loops=13)
                     Buffers: shared hit=76
                     ->  Bitmap Heap Scan on session_participants p_1  (cost=4.44..16.05 rows=3 width=1) (actual time=0.005..0.007 rows=2.85 loops=13)
                           Recheck Cond: (session_id = s.id)
                           Heap Blocks: exact=37
                           Buffers: shared hit=76
                           ->  Bitmap Index Scan on session_participants_pkey  (cost=0.00..4.44 rows=3 width=0) (actual time=0.003..0.003 rows=2.85 loops=13)
                                 Index Cond: (session_id = s.id)
                                 Index Searches: 13
                                 Buffers: shared hit=39
 Planning Time: 0.042 ms
 Execution Time: 0.945 ms
(39 rows)

== D: every item of the owner, 5000 plays (new statement)
                                                                                      QUERY PLAN
 GroupAggregate  (cost=138847.63..138893.49 rows=300 width=48) (actual time=73.108..74.306 rows=300.00 loops=1)
   Group Key: s.item_id
   Buffers: shared hit=49103
   ->  Sort  (cost=138847.63..138852.99 rows=2143 width=27) (actual time=73.096..73.330 rows=5000.00 loops=1)
         Sort Key: s.item_id
         Sort Method: quicksort  Memory: 392kB
         Buffers: shared hit=49103
         ->  Nested Loop  (cost=177.39..138729.07 rows=2143 width=27) (actual time=0.247..70.234 rows=5000.00 loops=1)
               Buffers: shared hit=49103
               ->  Nested Loop  (cost=161.34..104283.07 rows=2143 width=42) (actual time=0.205..24.311 rows=5000.00 loops=1)
                     Buffers: shared hit=19603
                     ->  Bitmap Heap Scan on sessions s  (cost=113.02..695.81 rows=2143 width=40) (actual time=0.178..0.971 rows=5000.00 loops=1)
                           Recheck Cond: (owner_id = $1)
                           Heap Blocks: exact=63
                           Buffers: shared hit=103
                           ->  Bitmap Index Scan on sessions_owner_recent  (cost=0.00..112.49 rows=2143 width=0) (actual time=0.165..0.165 rows=5000.00 loops=1)
                                 Index Cond: (owner_id = $1)
                                 Index Searches: 1
                                 Buffers: shared hit=40
                     ->  Aggregate  (cost=48.32..48.33 rows=1 width=2) (actual time=0.004..0.004 rows=1.00 loops=5000)
                           Buffers: shared hit=19500
                           ->  Index Scan using session_participants_pkey on session_participants p  (cost=0.42..48.31 rows=1 width=1) (actual time=0.003..0.003 rows=0.90 loops=5000)
                                 Index Cond: ((session_id = s.id) AND (companion_id = ANY ($2)))
                                 Index Searches: 5000
                                 Buffers: shared hit=19500
               ->  Aggregate  (cost=16.05..16.06 rows=1 width=1) (actual time=0.009..0.009 rows=1.00 loops=5000)
                     Buffers: shared hit=29500
                     ->  Bitmap Heap Scan on session_participants p_1  (cost=4.44..16.05 rows=3 width=1) (actual time=0.005..0.006 rows=2.90 loops=5000)
                           Recheck Cond: (session_id = s.id)
                           Heap Blocks: exact=14500
                           Buffers: shared hit=29500
                           ->  Bitmap Index Scan on session_participants_pkey  (cost=0.00..4.44 rows=3 width=0) (actual time=0.003..0.003 rows=2.90 loops=5000)
                                 Index Cond: (session_id = s.id)
                                 Index Searches: 5000
                                 Buffers: shared hit=15000
 Planning Time: 0.286 ms
 Execution Time: 74.461 ms
(37 rows)

```

### Review fixes: final runs (2026-11-11), every finding applied

| Finding | State | DEC |
|---|---|---|
| OT2F-1 ids from the companion id | applied, seen red | 603 |
| OT2F-2 counts from the history events, plays without details named | applied, seen red | 602 |
| OT2F-3 focus after "Use the highest score" | applied, seen red | 603 |
| OT2F-4 solo play no result, "No winner" (orchestrator default, owner may reverse) | applied, seen red | 603 |
| OT2F-5 chosen winner who left | applied, seen red | 603 |
| OT2F-6 hint and announcement | applied, seen red | 603 |
| OT2F-7 tests not seen red | recorded | 606 |
| OT2F-8 own log against People card | explained in the text | 604 |
| OT2F-9 0032 one-way | operations.adoc | 606 |
| OT2F-10 border, top bar 760 to 832 px, German, `sso` | applied; German defect found and fixed | 606 |
| OT2F-11 320 px score table | applied, seen red | 603 |
| OT2F-12 density helper | applied | 603 |
| OT2F-13 words | applied | 606 |
| OT2F-14 test split | applied | 606 |
| OT2F-15 `win_rate` | removed | 606 |
| Play read per case, EXPLAIN, load run to W2 | applied | 605 |

- After the first cargo-leptos build the new and changed browser tests ran (database
  `ot2f_r2`): 10 passed, 2 failed: the German top bar still 11 px over at 760 px (the cut
  label alone did not help; the button's minimum width was the cause, fixed) and a mistake
  in the new stats test (without time the totals row leads with "Plays"; a tile says
  "1 hour"). Screenshots of the dialog at 1280 and 320 px and of the German bar at 760 to
  900 px looked at (temporary spec, removed).
- Final build: `cargo leptos build --bin-features gallery,all-connectors --lib-features
  hydrate,gallery` after the last Rust change (`shell.rs`); only `stats.spec.ts`
  expectations changed after it. Clippy native and wasm were run before the last `shell.rs`
  class change (a string of classes), not after.
- Whole Playwright run on the final build, `--project chromium --project sso`, 2 workers,
  database `ot2f_r3`: 157 passed (setup 1, chromium 145, sso 11), 0 failed, 0 flaky,
  8.0 minutes; 0 `panicked at` lines in the server log.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, cargo-deny, the wasm browser tests of `ui`,
  `cargo test -p web --features ssr`, importers, the other modules of the server `it`
  binary, the release load run (DEC-605: moved to W2). The server `it` page tests and the
  crate tests ran before the `shell.rs` change of the top bar.
- Left in place: `target/site` and `target/debug/server` (the final build). Databases
  `ot2f_r1..3`, `ot2f_explain` and `test_*` dropped; test executables deleted; temporary
  specs removed.
- Noticed, not built: the export format carries no plays (DEC-602); "lowest score wins"
  (DEC-603); the read over all items is 74 ms at 5 000 plays (DEC-605, W2).
