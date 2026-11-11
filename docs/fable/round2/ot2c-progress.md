# Package ot2c - progress log

Brief: `docs/handoff/fable-briefs/PKG-implement.md`, scope: `docs/fable/round2/next-packages.md`
section "ot2c" (OT-17 step 1, DEC-542), design `docs/fable/round2/ux-owner-test.md` section 4.
Nothing is committed. One block per item: state, tests and results, files, DECs. A later run
continues from the first item that is not `done`.

How the tests are run here (the orchestrator runs `cargo xtask ci` / `e2e`, not this package):
everything inside `nix develop`, from `src/rust`, with `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0`;
Playwright by hand on own ports and an own database, as in the ot2b log.

## Items

| # | Item | State | DEC |
|---|---|---|---|
| 1 | app: per-kind totals in `StatsTotals` (`kinds`), group and overall minutes are sums of the kinds | done | 572 |
| 2 | contracts + api: `StatsTotalsDto.kinds` (additive, `serde(default)`) | done | 572 |
| 3 | web: hour formatting, `Measure::Hours`, time-first measure order, hour axis of the column chart | done | 573 |
| 4 | web: hero tile and four tiles (Home block, top of the Stats page), "Also" line, notes (`time_tiles.rs`) | done | 574 |
| 5 | web: status counts move down (Home: one line per group; Stats: "Library by status" disclosure) | done | 574 |
| 6 | locales en + de | done | 573, 574 |
| 7 | Playwright `home.spec.ts`, `stats.spec.ts`, `i18n.spec.ts`; server page tests `home_page::` | done | - |
| 8 | icon `piggy-bank`: already vendored and registered (icons.toml, ICONS.adoc, VERSIONS); used by the hero tile | done (no file change) | - |
| 9 | checks: fmt, clippy native + wasm, xtask locales, cargo leptos build, whole chromium project, DEC entries | done | - |

## Log

### First pass (2026-11-11)

Red runs seen before the code:

- `cargo test -p app --test stats`: compile error (no field `kinds`); with the field added but
  the old rounding (seconds to minutes per group): `totals_per_kind_add_up_to_the_hero_figure`
  failed, "the hero figure: left 367, right 366". Green after the fold per kind: 2 passed.
- `cargo test -p contracts the_stats_totals`: compile error (no field `kinds`), then 1 passed.
- `cargo test -p web --features ssr --lib`: 46 compile errors (`Measure::Hours`, `hours_of`,
  `axis`, `banked`, `library_line`, `CompareLine.measure`, ...), then 179 passed (170 before).
- `cargo test -p api --test rest phase1_library`: 1 passed (the `kinds` assertion was added
  together with the mapping, not seen red on its own).

Files so far: `crates/app/src/stats.rs`, `crates/app/src/lib.rs`, `crates/app/tests/stats.rs`,
`crates/contracts/src/browse.rs`, `crates/contracts/src/lib.rs` (test), `crates/api/src/browse.rs`,
`crates/api/tests/rest.rs`, `crates/web/src/{charts,home,stats_extras,time_tiles,lib}.rs`,
`crates/web/locales/{en,de}.ftl`, `apps/server/tests/it/home_page.rs`,
`src/web/tests/{home,stats,i18n}.spec.ts`.

Design choices taken (to become DEC entries at the end, next free number checked then):

- Minutes: seconds become whole minutes per kind; a group and the overall figure are sums of
  their kinds (before: per group). Listening kinds can differ by under a minute per kind.
- `Measure::Hours` replaces minutes in every chart, table, totals row and in the year
  comparison; goals keep their minute targets and the name "Minutes" (out of scope).
- Hours: one decimal below 10, whole from 10 (`hours_of`); the words are "{n} hours" in full
  (no "h"), so the visible text is the text equivalent.
- The four tiles are always shown (0 hours when empty); Games = video games + board games;
  other kinds with time or pages go into the "Also" line with the singular kind name.
- No tints and no kind icons on the tiles: the tints are ot2d. The icon sits on `bg-muted`
  (there is no `primary-soft` token).
- Stats page: the block sits above the tabs and does not change with the tab; the totals row
  of a tab shows the measures only (hours first); entries by status are in the disclosure
  "Library by status" below the charts. Key `stats-entries` removed (unused).
- The year comparison on the All tab shows a group's first measure that has anything (hours
  when there are any, else the first count), so a library without runtimes keeps its lines.

### Second pass (2026-11-11): browser, page tests, checks. All items done.

- Server page tests, first run on the new code: `home_page::stats_has_a_tab_per_group_and_a_table_per_chart`
  failed twice on assertions of the new test text itself: (1) "hero on top" compared positions
  in the streamed HTML, where the parts of a page arrive out of order (the order is checked in
  the browser instead); (2) "no minutes": the goal form still offers the measure "Minutes"
  (goals are out of scope), so the assertion now names the totals row and the chart.
- Playwright, first run of `home.spec.ts stats.spec.ts i18n.spec.ts` on the new build: 12 passed,
  1 failed: the new stats test, axe `target-size` on the Play tab at 360 px. Cause: the test had
  scrolled to the bottom at 1280 px and then only resized, which left the tab strip half under
  the fixed bottom bar. No app change; the test now loads `/stats?group=play` at 360 px (as the
  other specs do). Not a race, no retry or wait added. The browser tests were written before
  the first build but had no red run of their own: their first run was on the finished code.
- clippy pedantic found `Stats()` at 105 lines: the URL state moved into `stats_url_state()`.

Final runs, all on the final source (after the last edit: the split of `Stats()` and two
module doc comments):

- `cargo fmt --all --check`: clean.
- clippy pedantic, wasm (`-p web -p ui --all-targets --target wasm32-unknown-unknown
  --features web/hydrate,web/gallery`) and native (`--workspace --all-targets`, the CI feature
  set): no findings.
- `cargo test -p app`: all ok (`--test stats` 2). `cargo test -p contracts the_stats_totals`: 1.
  `cargo test -p api --test rest phase1_library`: 1. `cargo test -p web --features ssr`:
  179 + 3. `cargo test -p xtask --test locales`: 5.
  `cargo test -p store-pg --test phase1 stats` (Postgres): 3 (the cache equals the in-memory
  rules, `kinds` included).
- Server page tests (`--test it -- item_page:: library_page:: home_page:: auth_pages::
  admin_pages::`, `TEST_DATABASE_URL`, `REQUIRE_SERVICES=1`): 21 passed.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Whole Playwright `chromium` project on that build (database `ot2c_2`, 2 workers, ports 3110
  to 3114): 136 passed (setup included), 0 failed, 6.6 minutes.
- Not run: `cargo xtask ci`, `cargo xtask e2e` (and with it the panic guard on the server log),
  the `sso` browser project, cargo-deny, `cargo xtask sqlx-prepare` (no SQL changed), the release
  load run.
- Load run: the stats read was not changed in SQL. `stats_totals` runs the same two statements
  (`library.counts`, `progress.month_amounts`) and folds the same rows once more in memory (one
  row per kind and month); the answer grows by at most twelve small objects. A release load
  run is not needed for this package on technical grounds.
- Left in place: `target/site` and `target/debug/server` are the final `cargo leptos build`.
  Test executables in `target/debug/deps` deleted, the `test_*` and `ot2c_*` databases dropped
  (`e2e_1794380267` is not from this package and was left). Free disk at the end: 6.2 GB.

Noticed, not built (out of scope):

- Goals still count time in minutes (measure "Minutes", minute targets). Hours there would
  change what a stored target means.
- The time series and the year comparison divide seconds per bucket and group, not per kind
  (DEC-572); only the listening kinds can differ, by under a minute.
- A book finished without a known page count adds no pages, a play without a duration no time
  (ux-owner-test.md 4.3); the footnote only speaks of titles without a length. Step 2 (per-episode
  runtimes, the `untimed` column) is ot2g.
- On a phone, content scrolled under the fixed bottom bar is partly covered (axe `target-size`
  sees it when a check runs on a scrolled page). Any page, not new.
- Tints on the tiles and the hero circle are ot2d; the hero uses `bg-muted` until then.

## Review fixes (2026-11-11, review docs/reviews/2026-11-11-ot2c-review.md)

Nothing is committed. One line per finding; the runs are listed below the table.

| Finding | State | DEC |
|---|---|---|
| OT2C-1 sentence from the rounded hours of the figure, no "and 0 hours" | applied | 575 |
| OT2C-2 a kind without time and pages is in the "Also" line with its count | applied | 576 |
| OT2C-3 no "0 hours" for time that is there; DEC-572 "add up exactly" reworded | applied | 577 |
| OT2C-4 totals row against chart at a rounding edge | deferred (predates the package; ot2f or ot2g) | 578 |
| OT2C-5 a group without time leads with its counts (totals row, charts, year comparison) | applied | 578 |
| OT2C-6 "1 page" in the "Also" line | applied | 576, 578 |
| OT2C-7 HANDOFF 4.2 row | left to the orchestrator | 578 |
| OT2C-8 test-first of the first pass | accepted as reported, no action | 578 |
| OT2C-9 load run | left to the orchestrator | 578 |

Red runs seen before the code (`cargo test -p web --features ssr --lib`, 178 passed, 7 failed):

- `time_tiles::the_sentence_uses_the_hours_of_the_figure_and_leaves_out_zero_hours`: "That is
  1 day and 23 hours ..." against "That is 2 days ..." for 2,870 minutes (OT2C-1).
- `time_tiles::a_kind_without_time_and_pages_is_named_with_its_count`: "Also: Manga One page"
  (OT2C-2, OT2C-6); `the_hero_and_the_four_tiles_lead_with_time`: no "Music 3 logged steps".
- `time_tiles::time_below_one_hour_reads_in_minutes_never_as_zero_hours` and
  `charts::time_below_what_is_shown_never_reads_as_zero`: "0 hours" (OT2C-3).
- `charts::a_group_without_any_time_leads_with_its_counts` (against a stub `measures_led`) and
  `stats_extras::a_group_without_time_in_either_year_compares_its_counts_first` (OT2C-5).
- `home::the_chart_sentence_gives_the_total_as_the_chart_shows_it`: added after an edit of
  mine had dropped the `measure` argument of the chart sentence; red on that ("{$measure} in
  Listen ..."), then green.

Code: `crates/web/src/charts.rs` (`Measure::text`, `Measure::counted`, `measures_led`),
`time_tiles.rs` (`hero_sentence`, `also_amounts`), `home.rs` (`TotalsLine`, `SeriesCharts
timed`, `chart_summary`), `stats_extras.rs` (`compare_lines`), `locales/{en,de}.ftl` (nine new
keys: `stats-time-minutes`, `stats-hours-under`, `stats-hero-days-only`, `stats-also-pages`,
`-episodes`, `-plays`, `-finished`, `-completions`, `-events`).

Runs after the code:

- `cargo test -p web --features ssr`: 186 + 3 passed. `cargo fmt --all --check`: clean.
- clippy pedantic, wasm (`-p web -p ui --all-targets --target wasm32-unknown-unknown
  --features web/hydrate,web/gallery`) and native (`--workspace --all-targets`, the CI feature
  set): no findings. `cargo test -p xtask --test locales`: 5 passed.
- Server page tests (`--test it -- item_page:: library_page:: home_page:: auth_pages::
  admin_pages::`, `TEST_DATABASE_URL`, `REQUIRE_SERVICES=1`): 21 passed, no test file changed
  (grep of `apps/server/tests/it` for the changed strings: none used there).
- Browser tests extended: `home.spec.ts` (a visual novel session without a length: "Also:
  Audiobook 2 hours, Visual novel 1 play"; the Play line has two entries now), `stats.spec.ts`
  (a library without runtimes: "Activity, Completed entries, Hours" on All, "Watched, Episodes,
  Hours" in the totals row, the charts and the comparison of the Watch tab). Their first run
  was on the finished code (no red run of their own; the unit tests above were red).
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
  `home.spec.ts stats.spec.ts i18n.spec.ts` on it (database `ot2cfix_1`): 13 passed.
- Whole Playwright `chromium` project on that build, no source edit after it (database
  `ot2cfix_2`, 2 workers, ports 3110 to 3114): 136 passed, 0 failed, 6.6 minutes.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the
  app, api, contracts and store-pg tests (no file of those crates changed in the review
  fixes), `cargo xtask sqlx-prepare` (no SQL changed), the release load run (OT2C-9).
- Left in place: `target/site` and `target/debug/server` are the final build. Test
  executables in `target/debug/deps` deleted, the `test_*` and `ot2cfix_*` databases dropped
  (`e2e_1794380267` is not from this package). Free disk at the end: 8.5 GB.

All review findings are applied or deferred (DEC-575 to DEC-578). Open for the orchestrator:
OT2C-7 (HANDOFF 4.2 row) and OT2C-9 (load run).
