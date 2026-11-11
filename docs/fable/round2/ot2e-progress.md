# ot2e progress (rewatch run, OT-9 model A, OT-15 server part)

Resume notes for a cut-off run. Brief: docs/handoff/fable-briefs/PKG-implement.md.
Design in one place: DEC-587 (the trace and the marker `Progress::Rewatch`).
Nothing is committed. Build commands: `nix develop -c bash -c 'cd src/rust; CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 cargo test ...'`.

| # | Item | State | Tests (red first) | Files | DEC |
|---|---|---|---|---|---|
| 1 | Trace: sync clients, offline queue stamps, importers | done | - | docs/DECISIONS.md | 587 |
| 2 | domain: `Progress::Rewatch`, run derivation (`rewatch::runs`, `Run::phase`, `covers`), status rule `status_after` | done | `domain` `rewatch::tests::*` (6): 5 red against stubs, then `cargo test -p domain`: 26 passed | crates/domain/src/rewatch.rs (new), progress.rs, lib.rs | 587 |
| 3 | app: log rules (start, finishing episode), run-aware next step and fraction, `rewatch()`, `times` counts runs, `pass_at` (OT2A-7) | done | `crates/app/tests/rewatch.rs` (4, new): written first, first run after the code, then seen red (4 failed) with the rules switched off; green with `--test rewatch --test next_step --test undo --test library` (10+7+4+4 passed) | crates/app/src/next_step.rs, browse.rs, library.rs, stats.rs, lib.rs | 587 |
| 4 | contracts + api: `ProgressSummaryDto.rewatch` (`RewatchDto`), `ItemPageDto.times_at` | done | `api` `rest::the_item_page_carries_the_rewatch_run`: red with the mapping off, then `--test rest --test sync`: 19 + 1 passed | crates/contracts/src/browse.rs, lib.rs; crates/api/src/browse.rs, ops.rs; crates/api/tests/rest.rs | - |
| 5 | server: offline queue with a run started offline; SSR page test of a run | done | `it` `offline::a_rewatch_run_started_offline_replays_once`, `item_page::a_rewatch_run_shows_in_the_header_the_next_step_and_the_list`: written after the code; their first run failed on two mistakes in the tests themselves (stamp order, pill matcher), then `--test it -- offline:: item_page:: library_page:: home_page:: auth_pages:: admin_pages::`: 26 passed. Not seen red against missing behaviour | apps/server/tests/it/offline.rs, item_page.rs | - |
| 5b | undo of a whole run (found by the browser test) | done | `app` `rewatch::undoing_a_whole_run_gives_completed_back`: red (`in_progress`), then `cargo test -p domain -p app`: all green | crates/domain/src/progress.rs (`from_stamp`, `StatusChange::carried`), crates/app/src/library.rs | 588 |
| 6 | web: fact line (run, stopped, day of the pass OT2A-7), start/stop in the menu, episode list in a run, bulk answer starts a run (own dates, OT2A-10), "Completed again" toast, history word, en + de | code done, unit tests green; SSR tests, clippy, e2e still to run | `web` lib: `item_words::tests::{the_fact_line_of_a_rewatch, the_count_line_names_the_day_of_its_pass, a_rewatch_is_named_per_verb}`, `episode_plan::tests::{a_running_rewatch_counts_only_its_own_episodes, the_bulk_answer_starts_a_run_on_a_completed_entry}`, `next_step::tests::{the_progress_bar_reads_as_words (extended), the_toast_says_when_a_log_finished_a_rewatch}`, `widgets::tests::progress_reads_as_words_in_each_locale (extended)`: 7 red against stubs (a_rewatch_is_named_per_verb is a key table, green from the start), then `cargo test -p web --lib`: 197 passed. Also `ProgressEventDto.status` (api rest test extended, red then green) | crates/web/src/item_words.rs, next_step.rs, episode_plan.rs, episode_list.rs, item.rs, widgets.rs, log_sheet.rs, locales/en.ftl, de.ftl; crates/contracts/src/lib.rs; crates/api/src/dto.rs | - |
| 7 | Playwright item.spec.ts: start, mark two, stop, a whole run by the bulk answer, Undo, axe light and dark | done | "a rewatch of a completed show is a run: start it, mark two episodes, stop; a whole one counts": first run failed on its last line only (the test counted stored instead of effective events), then passed together with "a bulk mark on watched episodes ..." (database `ot2e_1`). Written after the code: not seen red in the browser | src/web/tests/item.spec.ts | 589 |
| 8 | Gates: clippy, fmt, locale tests, SSR page tests, whole chromium project | done | see "Final runs" | - | - |

## Log
- Trace written as DEC-587 before any code.
- Items 2 to 4 done (see table). Not yet run: clippy, store-pg, server tests.
- Item 6 code done; `cargo fmt --all --check` clean; clippy pedantic native (`--workspace
  --all-targets`, CI feature set) and wasm (`-p web -p ui --all-targets --target
  wasm32-unknown-unknown --features web/hydrate,web/gallery`) clean; `cargo test -p xtask
  --test whitelabel --test locales`: 2 + 5 passed. (State before the undo fix below.)
- Written, first run pending: `apps/server/tests/it/offline.rs`
  `a_rewatch_run_started_offline_replays_once`, `item_page.rs`
  `a_rewatch_run_shows_in_the_header_the_next_step_and_the_list`, Playwright `item.spec.ts`
  "a rewatch of a completed show is a run: ...".
- Found while writing the browser test: the Undo of "Log N again" (episodes newest first, then
  the start) left the entry In progress: the undo of the finishing episode rewrites the status
  with a new stamp, so the start's undo no longer saw "its" write (DEC-208 rule). Test
  `crates/app/tests/rewatch.rs::undoing_a_whole_run_gives_completed_back` written; fix: the
  move an undo causes is stored on the undo event, and `StatusChange` remembers the stamp it
  replaced (`from_stamp`), so a write restored by an undo still counts as carried.
- Playwright by hand: `cargo leptos build --bin-features gallery,all-connectors --lib-features
  hydrate,gallery`, `createdb ot2e_<n>` on 55432, then in `src/web`: `E2E_PORT=3110
  TMDB_MOCK_PORT=3111 OIDC_MOCK_PORT=3112 MAIL_SINK_SMTP_PORT=3113 MAIL_SINK_HTTP_PORT=3114
  E2E_DATABASE_URL=postgres://app@127.0.0.1:55432/ot2e_<n> npx playwright test --project chromium`.
- Undo fix done (DEC-588), choices recorded (DEC-589), `docs/modules/ROOT/pages/sync.adoc`
  has a "Rewatch runs" section.
- After the fix: `cargo test -p api -p store-pg -p importers --features importers/all -p
  contracts -p testkit` with the service env: all green (api 13 + 19 + 1, contracts 11,
  importers 12 + 7 + 8 (1 ignored) + 1 + 1 + 3, store-pg 1 + 2 + 2 + 1 + 1 + 2 + 9 + 2 + 3 + 5 + 3).
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok
  (after the undo fix; no Rust source changed since).

## Final runs (2026-11-11), all items done

- `cargo fmt --all --check`: clean. Clippy pedantic native (`--workspace --all-targets`, CI
  feature set): clean, rerun after the undo fix. Clippy wasm (`-p web -p ui --all-targets
  --target wasm32-unknown-unknown --features web/hydrate,web/gallery`): clean (run after the
  last change to `web`, `ui` and `contracts`).
- `cargo test -p domain -p app`: all green (24 result lines, 0 failed). `cargo test -p web
  --lib`: 197 passed (default features, not `--features ssr`). `cargo test -p xtask --test
  whitelabel --test locales`: 2 + 5. api, store-pg, importers, contracts: see the log above.
- Server `it` (`offline:: item_page:: library_page:: home_page:: auth_pages:: admin_pages::`,
  service env): 26 passed. Run before the undo fix (which changed `domain` and `app` only);
  the same paths ran afterwards in the browser run.
- Whole Playwright `chromium` project on the final build (database `ot2e_2`, 2 workers, ports
  3110 to 3114): 142 passed (setup included), 0 failed, 0 flaky, 6.9 minutes; 0 `panicked at`
  lines in the server log of the run. No Rust source or locale file changed after that build.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the wasm
  browser tests of `ui`, `cargo test -p web --features ssr`, a load run (no SQL changed; a log
  of an episode on an In progress show now reads the item's history once more,
  `ProgressRepo::for_item`).
- No migration, no SQL change: `cargo xtask sqlx-prepare` not needed.
- Left in place: `target/site` and `target/debug/server` (the final cargo-leptos build). Test
  executables in `target/debug/deps` deleted, the `test_*` and `ot2e_*` databases dropped.

Noticed, not built (out of scope):

- The generic export writes undo events into `history` (`api/src/ops.rs::export_rows` reads
  the stored events, not the effective ones); an import of such a file would fail on them.
- A run that is running at export time is not running after a re-import (DEC-589, limits).
- The bulk question says "as a rewatch" also where its answer logs extra views (a show that
  is In progress).
- An episode logged in the "Watched with..." dialog for "Yesterday" while a run started today
  is dated before the start and does not count in the run.
- `docs/design/ux/design.md` still describes the pre-ot2 labels (as noted in DEC-564).

## Review fixes (docs/reviews/2026-11-11-ot2e-review.md), 2026-11-11

Resume notes: findings in the order worked. DEC numbers are written at the end (590 and up);
"planned" means the DEC text is not in docs/DECISIONS.md yet.

- OT2E-2 domain (done): `Progress::RewatchStop` (`{"type":"rewatch_stop"}`), `Run.stopped`,
  `Run::is_open`, `phase` and `status_after` know the marker. Tests in
  `crates/domain/src/rewatch.rs`, seen red against a stub (4 failed):
  `a_stopped_run_does_not_come_back_with_in_progress`, `views_after_a_stop_do_not_join_the_run`,
  `two_stopped_runs_stay_apart`, `undo_takes_a_start_a_finish_and_a_stop_back`; then
  `cargo test -p domain --lib`: 34 passed.
- OT2E-6 (done): four unit tests of `StatusChange::carried` in `crates/domain/src/progress.rs`
  (green from the start, they pin existing code); the domain undo test no longer checks a
  status its own helper computed (`Show::undo` only appends the event).
- OT2E-7 c (done): domain `a_run_can_cover_every_episode_without_a_finishing_event`, app
  `a_run_that_covers_every_episode_is_over_without_a_finishing_event` (both green from the start).
- OT2E-1 server, OT2E-2 app, OT2E-12 (done): `crates/app/src/run_rules.rs` (new),
  `App::log_from`, `App::import_progress`; a stop is no completion (`app/src/stats.rs`).
  Tests in `crates/app/tests/rewatch.rs`, seen red (4 failed) before the code:
  `a_stop_marker_ends_the_run_for_good`, `a_view_dated_before_the_start_of_a_running_run_is_refused`,
  `a_start_cannot_be_dated_before_the_newest_view`, `what_is_not_a_run` (changed); also
  `undoing_a_stop_reopens_the_run`. `cargo test -p app`: all green.
- OT2E-5 (test done, DEC planned): `crates/app/tests/undo.rs`
  `an_undo_chain_needs_two_moving_events_and_only_runs_have_them`. The review's book example
  is not reachable (`finished` is refused for a paged kind).
- Still to do at this point: SQL (completions without stops, sqlx-prepare), importer and
  export (OT2E-7 a, OT2E-8), api tests, web (OT2E-1 dialog, stop marker, OT2E-10, 11, 13),
  DECs, sync.adoc, all gates.
- OT2E-7 a, OT2E-8, OT2E-2 c (done): `api/src/ops.rs::export_rows` writes the effective
  history; `importers/src/runner.rs` logs through `App::import_progress`, gives a start the
  Completed entry it was logged on and leaves a refused marker out. `App::check_progress`
  keeps "watched with" from logging a session for a refused view. Tests in
  `crates/api/tests/rest.rs`, seen red (2 failed): `export_then_import_keeps_rewatch_runs_and_leaves_undone_logs_out`,
  `a_rewatch_run_refuses_what_it_could_not_count`. Then `cargo test -p api -p importers
  --features importers/all`: api 13 + 21 + 1, importers 12 + 7 + 8 (1 ignored) + 1 + 1 + 3.
- SQL (done): completions leave `rewatch_stop` out in `store-pg/src/repos/library.rs`,
  `goals_groups.rs`, `stats_cache.rs`; `cargo xtask sqlx-prepare` run (3 files in `.sqlx`
  replaced). `store-pg/tests/phase1.rs::stats_on_postgres_match_the_in_memory_rules` extended
  with a stopped and a finished run: red with the old SQL (4 completions against 3), then
  `cargo test -p store-pg`: all green.
- Next: web (OT2E-1 dialog, stop marker in the menu, OT2E-10, 11, 13, 7 b), server `it`,
  DECs 590 to 594, sync.adoc, gates, Playwright.
- Web (done): OT2E-1 dialog (`log_form::within_run`, `FormError::BeforeRun`,
  `ItemInfo.run_start`, `item_words::run_start`), stop marker in the menu
  (`item.rs::RunMenu`, `next_step::log_run_marker`, `rewatch_stop`), OT2E-10 wording (en +
  de, `RewatchWords.{stopped_toast, finished}`, `LoggedStep.verb`, `said_title` per verb),
  OT2E-11 (`episode_plan::date_choice_shown`, `episodes-again-now`), OT2E-13 (`EpisodeList`
  split into `single_marks`, `BulkBar`, `DateChoice`; `overflow` uses `RunMenu`), OT2E-7 b
  (`offline::queued_log`, shared by the marker and the bulk mark). `cargo test -p web
  --features ssr --lib`: 6 red before the code and the locale lines
  (`item_words::tests::{a_rewatch_is_named_per_verb, the_fact_line_of_a_rewatch}`,
  `log_form::tests::a_day_before_the_start_of_a_running_rewatch_is_refused`,
  `log_sheet::tests::a_day_before_a_rewatch_names_the_day_it_started`,
  `next_step::tests::the_toast_says_when_a_log_finished_a_rewatch`,
  `widgets::tests::progress_reads_as_words_in_each_locale`), then 202 passed.
- DEC-590 (stop marker, replaces wording of DEC-587), DEC-591 (OT2E-1, OT2E-12), DEC-592
  (OT2E-5, OT2E-6), DEC-593 (OT2E-7, OT2E-8), DEC-594 (OT2E-3 deferred, OT2E-4 limit,
  OT2E-9/14 accepted, OT2E-10, 11, 13) written; `docs/modules/ROOT/pages/sync.adoc` section
  "Rewatch runs" rewritten.
- Changed with the code, first run pending at this point: server `it`
  `offline::a_rewatch_run_started_offline_replays_once` (the stop is the marker; a plain
  status write in between), `item_page::a_rewatch_run_shows_...` (a view of yesterday is 422;
  the stop marker and its history row), `src/web/tests/item.spec.ts` "a rewatch of a completed
  show is a run ..." (dialog refusal, stop toast and Undo, no revival, hidden date choice,
  "Rewatch finished").
- `cargo fmt --all`; clippy pedantic native (`--workspace --all-targets`, CI feature set) and
  wasm (`-p web -p ui --all-targets --target wasm32-unknown-unknown --features
  web/hydrate,web/gallery`): clean.
- Found while checking the callers of the log: (a) `integrations/src/lib.rs` logs a media
  server's play with its own `watched_at`; under DEC-591 (1) a late play during a running run
  would have been answered with an error. It now uses `App::import_progress` (stored, not
  counted). Test `integrations/tests/watched.rs::a_late_play_during_a_rewatch_run_is_recorded`,
  seen red with the old call. (b) `App::check_progress` answered ok for a marker on an item
  without an entry; it now refuses like the log (`run_rules::without_entry`); assertion in
  `app/tests/rewatch.rs::what_is_not_a_run`, seen red. DEC-591 (6) names both.
- Final rounds on the final code: `cargo test -p domain -p app -p contracts -p testkit`: all
  green (domain 34; app rewatch 10, undo 5; contracts 11); `-p integrations --features
  integrations/all`: 5 passed in `watched`; `cargo test -p web --features ssr --lib`: 202;
  `cargo test -p xtask --test whitelabel --test locales`: 2 + 5; `cargo fmt --all --check`
  clean; clippy pedantic native clean after the last change.

### Review fixes: final runs (2026-11-11), every finding applied or deferred

| Finding | State | DEC |
|---|---|---|
| OT2E-1 | applied (server and dialog) | 591 |
| OT2E-2 | applied, option (1): stop marker | 590 |
| OT2E-3 | deferred (hand completions write no event) | 594 |
| OT2E-4 | recorded as a limit | 594 |
| OT2E-5 | test added; no change outside runs possible | 592 |
| OT2E-6 | applied | 592 |
| OT2E-7 | a, c applied; b: unit test of the queued log, async branches left for the offline pack | 593 |
| OT2E-8 | applied (export writes the effective history) | 593 |
| OT2E-9 | accepted, sentence added to sync.adoc | 594 |
| OT2E-10 | applied | 594 |
| OT2E-11 | applied | 594 |
| OT2E-12 | applied (refused; a start dated before the newest event is refused) | 591 |
| OT2E-13 | applied | 594 |
| OT2E-14 | accepted, no action | 594 |

- After the last source change of `app`, `integrations` and `contracts`: `cargo test -p api -p
  importers --features importers/all` (api 13 + 21 + 1; importers 12 + 7 + 8 (1 ignored) + 1
  + 1 + 3), `-p integrations --features integrations/all -p app` (all green), server `it`
  (`offline:: item_page:: library_page:: home_page:: auth_pages:: admin_pages::`, service env):
  26 passed, twice (before and after those last changes). `cargo test -p store-pg`: all green
  after the SQL change and `sqlx-prepare`; not run again after the two later one-line changes
  in `app` (no SQL in them).
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok,
  run twice (the second run confirmed nothing was stale).
- Playwright, that build: `item.spec.ts` "a rewatch of a completed show ...", "a bulk mark on
  watched episodes ...", "offline, a mixed bulk mark ..." (database `ot2e_3`): 4 passed with
  setup, on the first run (the changed test was not seen red in the browser). Whole `chromium`
  project (database `ot2e_4`, 2 workers, ports 3110 to 3114): 142 passed, 0 failed, 0 flaky,
  7.1 minutes; 0 `panicked at` lines in the server log.
- After that build one thing changed in Rust source: `cargo fmt` rewrapped one assertion in
  the test module of `web/src/item_words.rs` (whitespace, `#[cfg(test)]`; German test literals
  had been turned into `\u{..}` escapes while the build ran, the second build covered that).
  `cargo test -p web --features ssr --lib`: 202 passed afterwards; `cargo fmt --all --check`
  clean. Clippy pedantic: native clean after the last non-test change; wasm clean after the
  last change to `web` code (the escapes and the rewrap came later, test module only).
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the wasm
  browser tests of `ui`, a load run.
- Left in place: `target/site`, `target/debug/server`, `.sqlx` (3 files replaced). Test
  executables in `target/debug/deps` deleted; the `test_*` and `ot2e_*` databases dropped.

Noticed, not built:

- A bulk "Log N again" whose start went through online and whose episodes are queued after a
  network failure sends the episodes with the browser's time; with a browser clock behind the
  server by more than the gap the flush refuses them (`invalid`), where they were silently
  not counted before.
- "Watched with..." offers "Yesterday" while a run that started today is running and refuses
  it on Save; hiding the chip would be the next step (owner's call).
- The stats cache (`stats_item_months`) needs no rebuild: no stop marker exists in stored data.
- `docs/design/ux/design.md` still describes the pre-ot2 labels (as noted in DEC-564).
