# Package ot2a - progress log

Brief: `docs/handoff/fable-briefs/PKG-implement.md`, scope: `docs/fable/round2/next-packages.md`
section "ot2a". Nothing is committed. One block per item: state, tests and results, files,
DECs. A later run continues from the first item that is not `done`.

How the tests are run here (the orchestrator runs `cargo xtask ci` / `e2e`, not this package):

- everything inside `nix develop`, from `src/rust`, with `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0`;
- unit: `cargo test -p <crate> ...`;
- Playwright by hand on own ports and an own database (as in the ot1a log):
  `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`,
  `createdb ot2a_<n>` on 55432, then in `src/web`: `E2E_PORT=3110 TMDB_MOCK_PORT=3111
  OIDC_MOCK_PORT=3112 MAIL_SINK_SMTP_PORT=3113 MAIL_SINK_HTTP_PORT=3114
  E2E_DATABASE_URL=postgres://app@127.0.0.1:55432/ot2a_<n> npx playwright test <spec> -g "<title>"`.

## Items

| # | Item | State | DEC |
|---|---|---|---|
| 1 | OT-5 imperative labels, past-tense toasts | done | 554 |
| 2 | OT-7 `times` field, count line, sheet line | done | 553, 554 |
| 3 | OT-4 two add buttons, shared toast with Undo (item page, palette) | done | 553, 554 |
| 4 | OT-5 status pill and fact line in the header | done | 554 |
| 5 | OT-6 key family "Watched / Listened / Read / Played with..." | done | 555 |
| 6 | OT-15 bulk marks ask "Log these N again as a rewatch?" | done | 556 |
| 7 | OT-16 duration `1:30` or `90`, prefill, scores behind "More" for video games | done | 557 |
| 8 | OT-12 card-footer rule, sweep over hand-styled links | done | 558 |
| 9 | OWN-7 sign-up lands on Home | done | 558 |

## Log

### State after the first pass of code (2026-11-10, before the first build)

All nine items are written; `cargo test -p web --features ssr` 161 + 2 passed, `-p ui --test
render` 11, `-p app --test next_step` 6, `-p contracts --lib` (the new test) 1, `cargo check -p
api` ok. NOT yet done: `cargo leptos build`, the Playwright runs on the new build, clippy
(native and wasm), the xtask locale test, the DEC entries in `docs/DECISIONS.md`.

Baseline (red) runs, before any Rust change:

- Playwright against the build of `main` (own database `ot2a_0`): 8 failed. `auth.spec`
  "signing up ..." (lands on /library); with the landing check relaxed for that run the other 7
  for their own reason: no "hours:minutes or minutes" hint; no group "Played with"; no button
  "Mark as watched"; no `episodes-again`; the channel card's Delete differs from Test (ghost);
  People card actions 32 px at 390 px; no toast "Added to your library as Planned".
- Unit tests, red as compile errors for the missing functions: `app::first_step`, `app::times`
  (17 errors), `ItemPageDto.times` (unknown field), web (119 errors: `Verb`, `times_text`,
  `fact_line`, `again_line`, `BulkPlan`, `standing_again`, `parse_duration`,
  `format_duration`, `logged_step_label`, `card_step_label`, `want_key`, ...), `ui::CardActions`.
- `crates/web/tests/button_classes.rs` red with 9 hand-written button looks (lists, imports,
  home x2, sign_in, shell x4), green after the sweep.

What was built, by file:

- contracts: `ItemPageDto.times` (serde default); `next_step` is now also sent for an item
  without an entry.
- app `next_step.rs`: `first_step`, `times` (finished events; whole passes of a show;
  plays; at least 1 when completed). api `ops.rs::item_page` fills both.
- web new `item_words.rs` (verb per kind group, `times_text`, `fact_line`, `again_line`, key
  pickers); `next_step.rs` (`logged_step_label`, `card_step_label`, shared `toast_added`,
  the non-compact `NextStepButton` is a `ui::Button` / `ButtonLink`); `item.rs` (`AddBar`
  with two buttons, status pill, fact line, no "Rewatches" row); `log_sheet.rs` (dialog title
  = step, "Watched with" legend and hint, "watch number N", duration `1:30`/`90` with
  prefill, scores behind "More" unless a board game, discard prompt uses `ui::Button`);
  `log_form.rs` (`parse_duration`, `format_duration`); `episode_plan.rs` (`BulkPlan`,
  `standing_again`); `episode_list.rs` (`AskAgain`, skipped count in the toast);
  `palette.rs` (shared toast); `auth_pages.rs` (sign-up goes to `/`); the sweep in
  `imports.rs`, `lists.rs`, `home.rs`, `shell.rs`, `sign_in.rs`; `ui::CardActions` in
  `people.rs`, `game_groups.rs`, `notify_channels.rs`.
- ui: `CardActions` (button.rs), `Sheet` / `Dialog` title is a `Signal<String>`.
- locales en + de: new and changed keys (see the diff); removed `item-rewatches`,
  `episodes-nothing-*`, `search-added-planned`.
- tests: `src/web/tests` item, shell, people, notifications, auth specs (new tests), label
  renames in home, library, offline, sharing, slice, i18n specs, `helpers.ts` `signUp` expects
  Home.

### Final state (2026-11-10): all nine items done, nothing committed

After the first build:

- First Playwright run of the five affected specs (database `ot2a_1`): 44 passed, 3 failed.
  Fixed: (a) the With... test reopened the dialog under its old name (it is named after the
  step now, "Watched S1 E3" the second time): test changed; (b) the duration field wrapped
  under the date chips because of its hint: the field has a fixed width of 7.5 rem and the hint
  is "h:mm or minutes" (DEC-557): code changed; (c) the bulk test opened a row menu right
  after "Log 10 again", while the page still refetched for the ten events, and the menu was
  detached: the test now brings the question up with "Mark season watched" (a button that
  survives the re-render). The page re-rendering under an open row menu after a bulk log is
  as before this package; not changed here.
- clippy: one test over 100 lines, split. xtask locale test: the test id `item-status-pill`
  looked like a key (now `ip-status`); the family `logged-step-` listed in
  `xtask/tests/locales.rs`; my key `card-step-episode` made the existing id prefix `card-`
  look like a key family, renamed to `next-step-episode-short`.
- Second run, whole `chromium` project (database `ot2a_2`): 126 passed, 1 failed
  (`i18n.spec` expected the library's heading right after sign-up; it opens the library
  itself now).

Final runs, all on the last code:

- `cargo fmt --all --check`: clean.
- clippy pedantic, native (`--workspace --all-targets`, the CI feature set) and wasm (`-p web
  -p ui`, `web/hydrate,web/gallery`): both without a finding.
- `cargo test`: `-p web --features ssr` 162 + 2 (`tests/button_classes.rs`); `-p ui`
  14 + 31 + 11; `-p app --test next_step` 6; `-p contracts` 10; `-p xtask` 13 + 1 + 6 + 2 + 5.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Playwright, every project (`setup`, `chromium`, `sso`), 2 workers, fresh database
  `ot2a_3`, own ports 3110..3114: 138 passed, 0 failed, 0 flaky; the server log of the run
  has no `panicked at`.
- Not run: `cargo xtask ci`, `cargo xtask e2e` (the orchestrator's), cargo-deny, the sqlx
  offline check (no SQL changed, so no `sqlx-prepare`), the other `-p app` / `-p api` /
  `-p server` test binaries (they need the database; only `cargo check -p api` and the clippy
  pass compiled them), the ui wasm tests, the load run.
- The databases `ot2a_0..3` are dropped. Disk: 8.4 GB free at the end.

DECs: 553 (times, first step), 554 (labels, two buttons, pill, fact line), 555 (verb family,
dialog title), 556 (bulk question), 557 (duration, prefill, scores), 558 (card footer, sweep,
landing).

Noticed, out of scope, not built:

- The episode row menu says "Watched with..." for a podcast too (`episodes-with`).
- The toast of a video game session reads "Logged: Play" (`log-play-step`).
- An open row menu is closed by the refetch that follows a bulk log (see (c) above).
- "Played 14 times, 31 h in total" in the header needs the durations (ot2f).
- `docs/design/ux/design.md` / the mockups still describe the old labels and the side sheet.
- No API-level test asserts `times` / `next_step` of an item without an entry (DEC-553).

## Review fixes

Review: `docs/reviews/2026-11-10-ot2a-review.md` (OT2A-1 to OT2A-16). One block per finding,
written as it is finished; nothing is committed. Browser runs as above, databases
`ot2ar_<n>`. Red runs of the browser tests were made against the package's last build (the
tree's sources were older than `target/site`), before any Rust change.

Browser tests seen red first (database `ot2ar_0`, old build): `item.spec.ts` "an item outside
the library ..." (no "added to your library" in the toast; with that line taken out: after
Undo the bar still has no "Want to watch", the entry stayed), "a bulk mark on watched
episodes ..." (after Cancel the row's menu button is not focused; with that line taken out:
"Log 10 again" is focused again after a rating refetched the page; with that one out too: the
row menu opened after "Log 10 again" is gone after the refetch).

### OT2A-1 (app part done, web part below)

- Rule (b) of the review: a completion set by hand and the first logged finish are one pass.
  `app::next_step` / `first_step` say "again" only once a finish is logged
  (`step_for` no longer reads the status); `times` unchanged.
- Test first: `app/tests/next_step.rs` `a_completion_by_hand_and_the_first_finish_are_one_pass`,
  red (`Finish { again: true }` for a completed entry without events), then green;
  `cargo test -p app --test next_step --test browse`: 7 + 6 passed.
- Files: `app/src/next_step.rs`, `app/src/browse.rs` (two call sites), `app/tests/next_step.rs`.

### OT2A-16 (API-level test) and the "not verified" point `App::kind` - done

- `api/tests/rest.rs` (in-memory adapters, no database needed, contrary to the package log):
  `the_item_page_has_a_first_step_and_the_count_without_an_entry` (green at once: it pins
  what exists; it fails when `.or(first)` or `times` is dropped) and
  `the_item_page_of_a_switched_off_kind_still_answers`, red: 422 `unknown kind` for a stored
  item whose kind the deployment no longer has (before ot2a the page answered: the kind was
  only read with an entry). Fix: `api/src/ops.rs::item_page` reads the kind with `.ok()`;
  without it no first step and `times` 0.
- `cargo test -p api --test rest`: 18 passed.
- Not done from OT2A-16: nothing to change in the five swept files (review: class sources only).

### Code state after the first pass of fixes (web unit tests green, build and browser runs pending)

Web unit tests seen red first (5 in the lib, 1 in `tests/button_classes.rs`), then
`cargo test -p web --features ssr`: 167 + 3 passed.

- OT2A-1 web part: the dialog line is fed from the header's count for a step that says
  "again" (`item_words::again_count`, test `the_dialog_line_counts_like_the_header` red:
  1 instead of 0 for a hand-completed entry). Files: `web/src/item_words.rs`, `item.rs`
  (`info_of`), `log_sheet.rs` (doc).
- OT2A-2: `Facts.completed`; a show is counted only when Completed, else "Up to date: N of
  N episodes" / the plain progress. Test `the_fact_line_follows_the_progress_model` changed
  to the two rows of the approved table (red: "Watched once, finished on ..." for a show
  that is not completed). Files: `web/src/item_words.rs`, `item.rs`.
- OT2A-3: `next_step::run_first_step` / `toast_logged_added`: the toast's second line is
  "<title>, added to your library" (`item-logged-added`, en + de), Undo undoes the event and
  removes the entry while it is untouched (`item_words::entry_untouched`: planned, no event,
  rating, note, tag). `AddBar` uses it. Tests: `the_first_log_says_that_the_item_was_added`
  (red), `only_an_untouched_entry_goes_with_the_undo`, `item.spec.ts` (red, see above).
- OT2A-4: `EpisodeUi.ask_focus` (one shot, set by `bulk()`), `Bulk::asker()` (selector of the
  control that asked), Cancel focuses it, "Log N again" stores it in `EpisodeUi.focus_back`
  and the rebuilt list focuses it (`focus_after_log`). The buttons carry `data-ep-bulk`.
  Unit test `a_bulk_mark_knows_the_control_that_asked`; `item.spec.ts` (red, see above).
- OT2A-5: the cause is not one refetch per event (the page only refetches on an
  `entity.updated` of its own item id, `web/src/events.rs::mentions`; a progress event
  carries the event's id): it is the one refetch `log_episodes` asks for at its end, 300 ms
  after the toast, which rebuilt the list and with it the open menu. Fix: the open row menu
  is state of `EpisodeUi` (`menu`), each row hands `ui::DropdownMenu` its `open` signal
  (the component had the prop for this, the library grid uses it), so the rebuilt row
  renders its menu open. The test uses the row menu again, right after the toast.
- OT2A-6: the conditional is replaced by `toHaveAttribute("aria-expanded", "false")`.
- OT2A-9: `item_words::again_ask_key` / `skipped_key` (`-listen` variants, en + de), the row
  menu uses `with_button_key(verb)`; `episodes-with` removed. Test
  `the_bulk_question_follows_the_verb` (red).
- OT2A-10 (half): the question is cleared only when the date choice can be used (the answer
  checks `bulk_dates` first). The release-date half is deferred.
- OT2A-11: the offline toast gets the same second line (`item.spec.ts` "offline, a mixed
  bulk mark still says what it skipped", red on the old build).
- OT2A-12: `CompanionPicker` `described_by`, the hint has an id (`item.spec.ts` With...
  test, red on the old build: empty accessible description).
- OT2A-13: `tests/button_classes.rs` scans string literals with a small state machine (char
  literals, escapes, raw strings, line comments) and knows `bg-secondary` / `border-border`
  looks; test `the_detector_is_not_thrown_off_by_other_quotes` (red). No new finding in the
  crate.
- OT2A-14: `parse_duration("0")` and `"0:00"` are `None` (test changed, red).
- OT2A-15: `log_episodes` lost its label building (`mark_words`); line counts are checked
  by clippy below.

### Final state of the review fixes (2026-11-10): nothing committed

Per finding: OT2A-1 applied (DEC-559), OT2A-2 applied (DEC-560), OT2A-3 applied (DEC-561),
OT2A-4 and OT2A-5 applied (DEC-562), OT2A-6, 9, 11, 12, 13, 14, 16 and the `App::kind` point
applied (DEC-563), OT2A-10 half applied (DEC-563) and half deferred, OT2A-7, 8 and the rest
of 15 deferred (DEC-564).

Runs on the last code:

- `cargo fmt --all --check`: clean.
- clippy pedantic, native (`--workspace --all-targets`, the CI feature set) and wasm
  (`-p web -p ui`, `web/hydrate,web/gallery`): no finding (one on the way: a test over 100
  lines, split into `the_fact_line_follows_the_progress_model` and
  `the_fact_line_of_plays_and_pages`).
- `cargo test -p web --features ssr`: 168 + 3; `-p app --test next_step --test browse`:
  7 + 6; `-p api --test rest`: 18; `-p xtask --test locales`: 5.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Playwright on the new build, database `ot2ar_1`, 2 workers: `item.spec.ts` 17 passed (with
  setup); `shell`, `people`, `notifications`, `auth`, `home`, `offline`, `library` specs:
  55 passed (with setup), 0 failed, 0 flaky.
- Not run: `cargo xtask ci` / `e2e`, the `sso` project and the specs not named above
  (`sharing`, `slice`, `i18n`, `keyboard`, ...), the other app / api / server test binaries,
  the ui tests (no ui file changed by the fixes), cargo-deny, the load run.
- Databases `ot2ar_0`, `ot2ar_1` dropped. Disk: 5.1 GB free at the end (it was at 5.04 GB
  during the clippy runs; stale test executables in `target/debug/deps` were deleted).

Files changed by the fixes: `app/src/next_step.rs`, `app/src/browse.rs`,
`app/tests/next_step.rs`, `api/src/ops.rs`, `api/tests/rest.rs`, `web/src/item_words.rs`,
`item.rs`, `next_step.rs`, `episode_list.rs`, `episode_plan.rs`, `log_form.rs`,
`log_sheet.rs`, `widgets.rs`, `web/locales/en.ftl`, `de.ftl`,
`web/tests/button_classes.rs`, `src/web/tests/item.spec.ts`, `docs/DECISIONS.md`.

Owner-visible choices: a Completed item without a logged watch offers "Mark as watched"
instead of "Log a rewatch" (DEC-559); a show with every known episode watched reads "Up to
date: N of N episodes" until it is set to Completed (DEC-560); the first log of an item
outside the library says "<title>, added to your library" and its Undo removes it again
(DEC-561); a podcast's bulk question says "listened to" and has no "as a rewatch" (DEC-563).

### Server SSR tests brought up to the new labels (2026-11-11): nothing committed

Five tests in `apps/server/tests` still asserted the labels from before ot2a; all five were
stale (no code change, no new DEC: the behaviour is DEC-554, 555, 559).

- `item_page.rs` `an_item_outside_the_library_offers_to_add_it`: the bar has "Mark as
  watched" and "Want to watch", no "Add to library", no "Watched with..." (DEC-554 (4)).
- `item_page.rs` `a_show_has_...`: `aria-label="Watched with...: Northern Lights"` (DEC-555).
- `item_page.rs` `the_action_bar_follows_the_progress_model`: a movie completed by hand
  offers "Mark as watched", not "Log a rewatch" (DEC-559). The "again" label it used to
  cover is in the new test `a_watched_movie_offers_the_rewatch` (a logged finish, then
  "Log a rewatch"; green at once, it pins what exists).
- `library_page.rs`: the pill's name is "Mark as watched: Harbour Lights"; after the undo
  "Mark S1 E1 watched: Downstream".
- Grep over `apps/server/tests` and `crates/*/tests` for the old strings: no other hit
  (`ui/tests/components.rs` passes "Log with details" as a title of its own).
- Runs: `cargo test -p server --features gallery,all-connectors --test item_page --test
  library_page --test home_page --test auth_pages`: 7 + 4 + 4 + 4 passed (auth_pages,
  home_page, item_page, library_page); clippy pedantic `-p server --all-targets` with the
  same features: no finding; `cargo fmt --all --check`: clean (after rustfmt on the two
  files, which was run after the test and clippy runs; whitespace only).
