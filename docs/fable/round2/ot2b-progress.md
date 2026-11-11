# Package ot2b - progress log

Brief: `docs/handoff/fable-briefs/PKG-implement.md`, scope: `docs/fable/round2/next-packages.md`
section "ot2b" (OT-10, DEC-539), plus OT2A-8 deferred to this package by DEC-564. Nothing is
committed. One block per item: state, tests and results, files, DECs. A later run continues
from the first item that is not `done`.

How the tests are run here (the orchestrator runs `cargo xtask ci` / `e2e`, not this package):

- everything inside `nix develop`, from `src/rust`, with `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0`;
- unit: `cargo test -p <crate> ...`; browser unit: `cargo test -p ui --target wasm32-unknown-unknown`;
- Playwright by hand on own ports and an own database (as in the ot2a log):
  `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`,
  `createdb ot2b_<n>` on 55432, then in `src/web`: `E2E_PORT=3110 TMDB_MOCK_PORT=3111
  OIDC_MOCK_PORT=3112 MAIL_SINK_SMTP_PORT=3113 MAIL_SINK_HTTP_PORT=3114
  E2E_DATABASE_URL=postgres://app@127.0.0.1:55432/ot2b_<n> npx playwright test <spec> -g "<title>"`.

## Items

| # | Item | State | DEC |
|---|---|---|---|
| 1 | ui: `FormDialog` (centered from 640 px, bottom sheet below, inner scroll, fixed footer), `content-start` on the grid panels | done | 566 |
| 2 | web: the watched and the play dialog use it; footer with Cancel and Save | done | 566, 567 |
| 3 | gallery page shows the form dialog | done | 566 |
| 4 | Playwright: centered at 1280, bottom sheet at 390, footer with ten players, OD-01 counts | done | 566, 567 |
| 5 | OT2A-8: one add at a time in `AddBar`; the add toast's Undo keeps a changed entry | done | 568 |

## Log

### State after the first pass of code (2026-11-11): web unit tests green, build and browser runs pending

All five items are written. NOT yet done: the ui wasm tests, `cargo leptos build`, the
Playwright runs on the new build, clippy (native and wasm), fmt, the xtask locale test, the
server page tests, the DEC entries.

Red runs seen (before the code):

- `cargo test -p ui --test render`: compile errors (`FormDialog`, `DialogWidth` missing); with
  `FormDialog` written and `content-start` still missing: `grid_panels_do_not_stretch_their_rows`
  failed ("content-start missing" on the side sheet), the other 14 passed.
- Playwright against the build of `main` (database `ot2b_0`): `item.spec.ts` "the log dialogs
  are centered ..." failed at the first geometry check (the panel's centre was 416 px from the
  window's: the side sheet); "one add at a time ..." failed at `expect(mark).toBeDisabled()`,
  and with that line taken out at the missing toast "Kept in your library: ..." (the Undo
  removed the Completed entry).
- `cargo test -p web --features ssr --lib item_words`: compile errors (`add_undo`, `AddUndo`).

What was built, by file:

- ui `dialog.rs`: `FormDialog` and `DialogWidth` (`Regular` 32 rem, `Wide` 42 rem). The panel
  is a column: title, `[data-dialog-body]` (scrolls), `[data-dialog-footer]` (a `ViewFn`,
  outside the scroller). Below 640 px a bottom sheet (`max-h-[85dvh]`), from 640 px centered
  (`max-h-[calc(100dvh-2rem)]`, `w-[calc(100%-2rem)]`). `Dialog` and `Sheet` got
  `content-start` on their grid. `SheetSide::Adaptive` stays (the "Add a person" and "New
  list" sheets use it). `lib.rs` exports.
- ui tests: `tests/render.rs` four tests (place, widths, body and footer, `content-start`);
  `tests/wasm.rs` `form_dialog_traps_focus_over_body_and_footer_and_returns_it`.
- web `log_sheet.rs`: `WatchedSheet` (Regular) and `PlaySheet` (Wide) are `FormDialog`s; new
  `LogFooter` (discard question, errors, Cancel, Save with `form=<id>`); `OfflineSave` is
  gone; the forms have the ids `watched-form` / `play-form`. Below 640 px Save is 44 px high
  and full width and Cancel is hidden (the close button stays).
- web `gallery.rs`: "Open form dialog" (twelve score fields, Cancel and Save).
- OT2A-8: `item_words.rs` `AddUndo` / `add_undo`; `next_step.rs` `undo_add`, `toast_added`
  asks the item page first and says `item-added-kept` when the entry changed; `item.rs`
  `AddBar`: both buttons are disabled while either runs.
- locales en + de: `log-cancel`, `item-added-kept`.
- browser tests: `item.spec.ts` two new tests, `gallery.spec.ts` one.

Runs so far: `cargo test -p ui --test render` 15 passed (after `content-start`);
`cargo test -p web --features ssr` 169 + 3 passed.

### After the build (2026-11-11)

- `cargo test -p ui`: 14 + 31 + 15 passed; `cargo test -p ui --target wasm32-unknown-unknown`:
  22 passed (the new focus test passed at its first run: the component existed by then; it
  was red only as "does not compile" before).
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Playwright on the new build (database `ot2b_1`): the three new tests. First run: 2 passed,
  1 failed for a wrong number in the test (Save is 340 px wide at 390 px: 24 px padding and
  the 1 px border on each side; the test said 341); test corrected, then 3 passed.
- Looked at screenshots of both dialogs at 1280 and 390 px (a throwaway spec, deleted): the
  dialog is centered, rows keep their height, the discard question and the error sit above
  the buttons.

Still to do: the other browser specs, clippy (native, wasm), fmt, xtask locale test, server
page tests, DEC entries.

### Final state (2026-11-11): all five items done, nothing committed

- Whole `chromium` project on the new build (database `ot2b_1`, 2 workers): 130 passed,
  1 failed, 0 `panicked at` in the server log. The failure was the new dialog test at its
  390 px step: `boundingBox()` of a score field was null. Cause: the item page refetches
  after the play that the test saved just before, and a refetch renders the open dialog once
  more (its state lives in the page and is kept; the elements are new). Test changed: it
  waits for "One play" in the bar and repeats the measuring (`toPass`); it also saves with
  Enter in the duration field now (Save is outside the form). Then the test alone,
  `--repeat-each=4` on a fresh database `ot2b_2`: 4 passed. The whole project was NOT run
  again after this test-only change.
- `cargo fmt --all --check`: clean (after `cargo fmt --all`; whitespace only, in
  `ui/tests/render.rs` and the files written here). The build in `target/site` is of the
  code before that formatting.
- clippy pedantic, native (`--workspace --all-targets`, the CI feature set) and wasm
  (`-p web -p ui --all-targets`, `web/hydrate,web/gallery`): no finding.
- After fmt: `cargo test -p ui` 14 + 31 + 15; `cargo test -p web --features ssr` 169 + 3;
  `cargo test -p xtask --test locales` 5. ui wasm suite (before fmt): 22.
- Server page tests: `cargo test -p server --features gallery,all-connectors --test it --
  item_page:: library_page:: home_page:: auth_pages:: admin_pages::` with the service
  variables: 21 passed. Grep of `apps/server/tests/it` for strings of this package: nothing
  to change (no existing label changed; "Cancel" and the kept-toast are new and not in a
  server-rendered page, a closed dialog renders nothing).
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the
  sqlx check (no SQL changed), the app / api / other server test modules, the load run.
- Databases `ot2b_0..2` and the 21 `test_01a1c8f1*` databases of the server test run are
  dropped. 158 older `test_01a1c8a9*` / `test_01a1c8aa*` databases (1.7 GB, not from this
  package) and `e2e_179436804*` are left. Disk: 4.6 GB free at the end (below the 5 GB mark;
  3.8 GB right after the server test build; test executables in `target/debug/deps` deleted).

DECs: 566 (the `FormDialog`, widths, `content-start`), 567 (footer, Cancel, Save and the
form, phones), 568 (OT2A-8).

Noticed, out of scope, not built:

- A refetch of the item page renders an open log dialog again (state kept, elements new):
  focus goes back to the panel while the person is in a field. As before this package
  (`item.rs` mounts the dialogs inside the refetched view).
- The score rows are one column of fields; the compact score table with a "won" column
  (ux-owner-test 2.3) is not built. Ten players need scrolling.
- "Add a person" and "New list" still use the adaptive side sheet (now without stretched
  rows); the duration field still wraps under the date chips at 390 px.
- `docs/design/ux/design.md` 5.4 / 7.3 and the mockups still say "sheet"; the components are
  still named `WatchedSheet` / `PlaySheet` in `log_sheet.rs`.
- OT2A-8 leftovers named in DEC-568: the check-then-remove of the add toast's Undo is two
  requests; the two add buttons are enabled for a moment between the add's answer and the
  refetched page.

## Review fixes

Review: `docs/reviews/2026-11-11-ot2b-review.md` (OT2B-1 to OT2B-7). Nothing is committed.
One block per finding, written as the work goes; a later run continues at the first block
that does not say `done`. Browser runs as above (ports 3110-3114, databases `ot2bfix_<n>`).

### Red runs (2026-11-11, on a build of the tree as reviewed, before any fix)

`cargo leptos build` of the reviewed tree first (the `server` binary in `target/debug` was a
later test build and did not hydrate). Then `item.spec.ts`, new or changed tests:

- "a refetch of the page leaves an open log dialog alone ..." (new, OT2B-1): failed at
  `expect(last).toBeFocused()` after the held refetch arrived (the score field was rendered
  anew, "inactive").
- "another item opened while a log dialog is open closes the dialog ..." (new, OT2B-1):
  failed at `expect(page.getByRole("dialog")).toHaveCount(0)` after `goBack()` (the dialog of
  the item left stayed open over the other item).
- "in a low window (320 x 256) ..." (new, OT2B-2 and OT2B-3): with the discard question the
  "Today" chip was in view at a ratio of 0.71 only (the body had about 20 px); in an earlier
  order of the test the three equal lines of the error list (two scores, one duration: three
  items, two of them the same) failed first.
- "one add at a time ..." (extended, OT2B-4): failed at `expect(want).toBeDisabled()` while
  the refetch after the add was held.
- "the log dialogs are centered ..." without its `toPass` (OT2B-1, OT2B-7 with `toBe(2)` and
  `toBe(5)`): passed on the old build in this run (the race it was bent around did not fire).

Note on the review's scenarios: a change of the entry from elsewhere (a tag, a status, a
rating through the API) does NOT refetch the item page. `item.rs` listens for
`entity.updated` events that name the item id; the library publishes the entry id
(`library_entry`), a log the event id (`progress_event`). Only `catalog_item` (enrichment,
catalog refresh) arrives. So "a save causes two refetches" does not hold; the refetch that
meets an open dialog is the page's own (300 ms after a save, a rating, a status) or the
enrichment of a stub. The test therefore holds the answer of the page's own refetch (after
a rating) with `page.route`, opens the dialog, and lets the answer arrive.

### OT2B-1 (high): done (code and targeted tests; see the final runs below)

- `web/src/item.rs`: `WatchedSheet` and `PlaySheet` are mounted once in `ItemPage`, after
  the `Transition`, next to `TagEditor` and `ListPicker`; always, not keyed (a closed dialog
  renders nothing, so the server-rendered page and the hydrated one agree whatever arrives
  first). `ItemUi` got `info: RwSignal<Option<ItemInfo>>` and `play_title`; every render of
  `ItemView` writes them. On another item id the page closes an open dialog without asking
  (`WatchedState::dismiss`, `PlayState::dismiss`). The buttons that open the dialogs have the
  ids `ip-log-with` / `ip-log-play`.
- `web/src/log_sheet.rs`: the dialogs take the info as a signal and read it untracked when
  they open (reset, fields: verb, board game, "again" line) and when they save (rating,
  note, item id); the description is a memo over the title; `return_to` names the opener's
  id, so focus returns to the button the refetch rendered in place of the one that opened
  the dialog. `funnel::sheet_flow` runs once per page now.
- `ui/src/dialog.rs`: `FormDialog`'s `description` is a `MaybeProp<String>` (a signal is
  followed); `Dialog` and `Sheet` keep their plain text.
- Tests: `item.spec.ts` "a refetch of the page leaves an open log dialog alone ..." (play
  and watched: focus, the next key, the text typed into "Add a person", `scrollTop`, focus
  on the opener after Escape and Discard, and the funnel: one `sheet` flow per opening with
  2 and 3 taps) and "another item opened while a log dialog is open ..."; the `toPass` and
  its comment are gone from "the log dialogs are centered ..."; `ui/tests/render.rs`
  `a_dialog_is_described_by_its_description_only_when_it_has_one`.
- DEC: see below (written at the end).

### OT2B-2 (medium): done

- `ui/src/dialog.rs`: `FORM_PANEL` has `overflow-y-auto`, `FORM_BODY` `min-h-32` instead of
  `min-h-0`. Normal windows: as before (the body shrinks first and scrolls inside); below
  the floor the panel scrolls as a whole.
- Tests: `ui/tests/render.rs` `form_dialog_scrolls_as_a_whole_when_the_window_is_too_low_for_its_body`
  (written with the change, not seen red by itself; the old assertion "no `overflow-y-auto`
  on the panel" is gone); `item.spec.ts` "in a low window (320 x 256) ..." (seen red, see
  above): first field, a score field and Save can be scrolled into view as the dialog opens,
  with the discard question, with the errors, with both; axe in that state.

### OT2B-3 (low): applied

- One line per kind of error (`log_sheet::error_lines`; ten wrong scores were ten equal
  lines), the wrong score fields and the duration get `aria-invalid` and name the list in
  `aria-describedby`; the list has no maximum height and no scroller of its own any more.
  The `role="alert"` moved from the `ul` to a `div` around it (axe `listitem`, found by the
  new low-window test: a `ul` with that role is no list).
- Tests: unit `log_sheet::tests::errors_are_one_line_per_kind_in_the_order_of_the_form`;
  the low-window browser test asserts the two lines and the marks.

### OT2B-4 (low): applied

- `item.rs` `AddBar`: a signal `done`, set when the add or the first log went through, part
  of `either`; never cleared (the refetched page replaces the bar).
- Test: "one add at a time ..." holds the refetch after the add and expects both buttons
  disabled without a spinner (seen red).

### OT2B-5 (low): deferred as the review judges it (DEC below); no code

### OT2B-6 (low): applied

- `ui/tests/wasm.rs`: the focus is put on the first field before Shift+Tab; a Tab and a
  Shift+Tab from the middle stop (Save) are not prevented and move nothing.

### OT2B-7 (low): applied

- `item.spec.ts`: `toBe(2)` for the watched dialog, `toBe(5)` for the play dialog.

### Runs so far (fixed code)

- `cargo test -p ui --test render`: 17 passed. `cargo test -p ui --target
  wasm32-unknown-unknown`: 22 passed (wasm.rs). `cargo test -p web --features ssr`: 170 + 3
  passed (before the last change to the error list's markup).
- clippy pedantic wasm (`-p web -p ui --all-targets`, `web/hydrate,web/gallery`) and native
  (`--workspace --all-targets`, the CI features): no finding. `cargo fmt --all` applied.
- `cargo leptos build` ok; `item.spec.ts` the five dialog and add tests: 5 passed (two of
  them after two corrections: the funnel beacon is sent 5 s after the flow, so the test
  waits for it before leaving the page; the alert markup above).

### Final state of the review fixes (2026-11-11): all seven findings handled, nothing committed

DECs: 569 (OT2B-1), 570 (OT2B-2), 571 (OT2B-3 to OT2B-7; OT2B-5 deferred).

Two corrections after the first whole run (133 passed, 1 failed):

- OT2B-4, first version: the first log's `on_done` was a callback made in `AddBar` that set
  `done` and refetched. The Undo of the toast runs `on_done` again after the bar is gone;
  the callback was disposed with the bar and the page did not refetch ("an item outside the
  library ..." failed at `expect(want).toBeVisible()` after Undo). Now `run_first_step`
  takes the `done` signal and sets it when the log went through (`try_set`; not for a log
  queued offline), and `on_done` is the page's own `refresh` again. That test got a step for
  this case (both buttons disabled while the refetch after the first log is held); the step
  was not seen red.
- The ids of the two opening buttons are `ip-log-with` / `ip-log-play` (`item-log-*` read as
  Fluent keys to `xtask --test locales`).

Runs on the final code:

- `cargo fmt --all --check`: clean. clippy pedantic, wasm and native (the CI arguments): no
  finding.
- `cargo test -p ui --test render`: 17 passed; `cargo test -p ui --target
  wasm32-unknown-unknown`: `tests/wasm.rs` 22 passed (run before the OT2B-4 rework and the
  id rename; `ui` did not change after it).
- `cargo test -p web --features ssr`: 170 + 3 passed. `cargo test -p xtask --test locales`:
  5 passed.
- Server page tests (`--test it -- item_page:: library_page:: home_page:: auth_pages::
  admin_pages::`, `TEST_DATABASE_URL`, `REQUIRE_SERVICES=1`): 21 passed.
- Whole Playwright `chromium` project on the final build (database `ot2bfix_4`, 2 workers):
  134 passed (the setup test and 133), 0 failed, 0 `panicked at` in the server log.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the
  sqlx check (no SQL changed), the other server test modules, `-p app` / `-p api`.
- Left in place: `target/site` and `target/debug/server` are a `cargo leptos build` of the
  final code. Databases `ot2bfix_0..4` and the `test_*` databases of the two server test
  runs are dropped; test executables in `target/debug/deps` deleted. Disk: 8.5 GB free.

Noticed, out of scope, not built:

- The item page does not hear of changes to its entry from elsewhere (see the note above:
  the events carry the entry id or the event id, the page compares with the item id).
- After Save in a log dialog the focus returns to the opener, and the refetch that follows
  the save replaces that button (focus on the body), as after every one-click step.
- A first log queued offline leaves the add bar enabled (as before): a second click queues
  a second log.
- Not covered by a browser test: focus after closing a watched dialog that was opened from
  an episode's menu and outlived a refetch (it goes to the "Watched with..." button of the
  action bar by `return_to`; read from the code).
