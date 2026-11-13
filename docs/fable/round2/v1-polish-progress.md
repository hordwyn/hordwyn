# v1-polish progress (lane C)

Resume notes for a cut-off run. Briefs: docs/handoff/fable-briefs/PKG-implement.md and
LANES.md (lane C: worktree `kithkeep-lane-polish`, branch `lane/v1-polish`,
`CARGO_BUILD_JOBS=1`, ports 3300-3399, database prefix `lc_`). Scope:
docs/fable/round2/v1-plan.md section 2 (P1 to P8) plus the two focus items sec0 left open.
Nothing is committed. Decisions: docs/fable/round2/v1-polish-decisions.md (LC-1 to LC-9 the
package, LC-10 to LC-22 the review fixes; see "Review fixes" at the end).

Build commands (one at a time, `free -g` first):
`nix develop -c bash -c 'cd src/rust && CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo ...'`.
Service env for the server tests: `TEST_DATABASE_URL=postgres://app@127.0.0.1:55432/postgres
REQUIRE_SERVICES=1` (the tests make their own `test_*` databases).
Playwright: `E2E_PORT=3310 TMDB_MOCK_PORT=3311 OIDC_MOCK_PORT=3312 MAIL_SINK_SMTP_PORT=3313
MAIL_SINK_HTTP_PORT=3314 E2E_DATABASE_URL=postgres://app@127.0.0.1:55432/lc_e2e_<n>`.

| # | Item | State | Tests (red first) | Files |
|---|---|---|---|---|
| P1 | `/stats` marked Home | done | `web` `shell::tests::paths_map_to_their_section` (red: `"/"`), `five_sections_and_an_admin_entry_only_for_admins` (extended); `shell.spec.ts` "single-key shortcuts ..." (extended) | `crates/web/src/shell.rs` (`section_of`, `account_entry`, `aria-current` in the account menu). LC-1 |
| P2 | History year filter in the person's zone | done | `app/tests/activity.rs::the_year_filter_follows_the_persons_time_zone` (red: 2024 held "first", the play of 2025-01-01 00:00 Berlin) | `crates/app/src/activity.rs` (`year_bounds`). LC-8 |
| P3 | podcast rows say "Listened"; session toast | done | `web` `item_words::tests::a_podcasts_episode_rows_say_listened`, `the_toast_of_a_session_names_a_session_and_of_a_board_game_a_play` (both red); server `item_page::the_episode_rows_of_a_podcast_say_listened` (new); `item.spec.ts` two toasts "Logged: Session" | `crates/web/src/{item_words,episode_list,log_sheet}.rs`, `locales/{en,de}.ftl`. LC-2 |
| P4 | palette key hints at 390 px | done | `ui/tests/wasm.rs::the_key_hints_of_the_active_row_are_hidden_on_narrow_screens`; `shell.spec.ts` "palette at 390 px ..." (new) | `crates/ui/src/command.rs`. LC-4 |
| P5 | a cover that fails to load | done | `ui/tests/wasm.rs::a_command_row_whose_picture_fails_keeps_the_icon_and_shows_no_text` (red: "the failed picture is taken away"), `web/tests/wasm.rs::a_cover_that_fails_to_load_shows_the_title_in_its_box`; `shell.spec.ts` "a cover that does not load ..." (new) | `crates/web/src/widgets.rs` (`Cover`), `crates/ui/src/command.rs` (`Leading`). LC-5 |
| P6 | bulk question on an In progress show | done | `web` `item_words::tests::the_bulk_question_names_a_rewatch_only_where_one_starts` (red); `item.spec.ts` "a bulk mark on watched episodes ..." (texts changed; the Completed case in "a rewatch of a completed show ..." keeps "as a rewatch") | `crates/web/src/{item_words,episode_list}.rs`, `locales/*`. LC-3 |
| P7 | covers by one rule (DEC-640) | done | `web` `palette::tests::thumb_falls_back_to_the_provider_address` (red), `thumb_prefers_the_smallest_stored_variant`, `the_empty_palette_lists_recent_items_then_the_suggestions` (red: no leading box); `shell.spec.ts` "palette: the library leads ..." and "palette: Ctrl+Enter ..." (extended) | `crates/web/src/{palette,recent,item}.rs`. LC-6 |
| P8 | docs page "What v1 covers" | done | - | `docs/modules/ROOT/pages/v1-scope.adoc` (new), `nav.adoc`, `operations.adoc` (one sentence in Backup). LC-9 |
| F1 | rules matrix: focus on the pressed switch | done | `notifications.spec.ts` "channels, rules ..." (extended) | `crates/web/src/notify_rules_view.rs`, `keyboard.rs` (`focus_is_nowhere`). LC-7 |
| F2 | focus back on "Edit" after Save / Cancel | done | `notifications.spec.ts` "a channel is edited in place ..." (extended) | `crates/web/src/notify_channels.rs`. LC-7 |

## Test runs so far

- `cargo test -p app --test activity --test activity_person`: 8 + 1 passed (after the red
  run of the new test).
- `cargo test -p ui --target wasm32-unknown-unknown --test wasm`: red on the picture test,
  then 26 passed. The hints test was written before the change but the red run stopped at
  the first failure, so its own red was not seen.
- `cargo test -p web --lib`: six tests red on stubs with the old behaviour (shell, three of
  item_words, two of palette), then 221 passed.

- `cargo fmt --all --check`: clean (after `cargo fmt --all`).
- Clippy pedantic as errors: `cargo clippy -p web -p ui -p app -p server --all-targets
  --features server/gallery,server/all-connectors` clean; the wasm run (`-p web -p ui
  --all-targets --target wasm32-unknown-unknown --features web/hydrate,web/gallery`) clean.
  (`-p web -p ui -p app` without the server features does not build: `web` needs `ssr`.)
- `cargo test -p xtask`: 5 + 19 + 2 passed (locale parity, white-label).
- `cargo test -p web --target wasm32-unknown-unknown --features hydrate --test wasm`: 3
  passed (the new cover test was written after `Cover` was changed: not seen red there; the
  browser test below was).
- Red run of the browser tests: a `cargo leptos build` with the visible changes switched
  off again (section of `/stats`, hints class, both `error` handlers, `thumb` fallback and
  the recent items' box, the two wordings, both focus calls; sources restored right after
  the build). The nine changed or new tests of `shell.spec.ts`, `item.spec.ts` and
  `notifications.spec.ts` all failed, each at its new assertion; setup passed.
- Server page tests on the final code (`--test it -- item_page:: library_page:: home_page::
  auth_pages:: admin_pages::`): 25 passed, with the new
  `item_page::the_episode_rows_of_a_podcast_say_listened`.

## Host notes

- A first browser run lost its server in the middle of the setup wizard (connection
  refused, no line in the server log, no OOM in the cgroup). HANDOFF section 8 tells every
  lane to stop servers with `pkill -x server`; that matches the server of every lane. The
  runs of this lane start a hard link `target/debug/lc-server` (`E2E_SERVER_CMD`), which
  that command does not match.
- The server test databases are named `test_<uuid>` by `store_pg::testing` (no lane
  prefix possible without a code change); this lane dropped none of them. Its browser
  databases are `lc_e2e_*`.

- Final `cargo leptos build --bin-features gallery,all-connectors --lib-features
  hydrate,gallery` on the restored sources, then the whole `chromium` project three times
  (lane ports, a fresh `lc_e2e_*` database each, dropped afterwards):
  1. 152 passed, 2 failed: two new assertions of this package expected the pictures at
     `image.tmdb.org`; the server of the browser tests takes them from the TMDB mock (the
     image base follows the API base). Specs corrected (`IMAGES` in `shell.spec.ts`), no
     code change, no rebuild.
  2. 153 passed, 1 failed: `sharing.spec.ts:55` "promote, accept, see history, unlink,
     merge and undo the merge" ran into its 30 s test timeout (in `hydrated` on History).
  3. 153 passed, 1 failed: the same test, the same timeout, at another line ("Copied to
     your log.").
  That test has the default 30 s, took 29.9 s in run 1 (passed) and 13.1 s when run with
  nine others; runs 2 and 3 took 21 to 23 minutes against 11 for run 1 (load average 6 to
  8, other lanes building). Not touched here (the file is not in this package and lane A
  edits every spec); it needs `test.setTimeout` like its neighbours (TEST-8).
  All tests this package changed or added passed in runs 2 and 3.

## Open

- Nothing of the scope. Not committed.

## Follow-ups named by the plan, not built here

- operations.adoc: a paragraph "Which hosts a browser contacts for pictures" (image hosts of
  the sources, `assets.fetch_policy = eager` reduces it).
- sec1 (lane A): CSP with `img-src 'self' https: data:` (DEC-640). A cover the CSP blocks
  now shows the title box / the kind icon (LC-5) instead of a broken picture.

## Review fixes (2026-11-13)

Review: docs/reviews/2026-11-13-v1-polish-review.md (POL-1 to POL-20). Two more items came
from the orchestrator while the fixes were built (O-1, O-2). Nothing is committed.

| Finding | State | What | Tests (red first unless said) | Files | LC |
|---|---|---|---|---|---|
| POL-1 (must) | applied | the current entry of the account menu is heavier and tinted; focused it looks focused and keeps the weight; acceptance line of v1-plan.md section 2 rewritten | `shell.spec.ts` "single-key shortcuts ..." (weight, tint, contrast, focused look): red on the build before the change | `web/src/shell.rs` (`MENU_ITEM`), `docs/fable/round2/v1-plan.md` | LC-10 |
| POL-2 (must) | applied | de: "Ein weiteres Mal eintragen?" / "Alle ein weiteres Mal eintragen?" | `item_words::tests::the_bulk_question_names_a_rewatch_only_where_one_starts`: red on the old text | `web/locales/de.ftl`, `web/src/item_words.rs` | LC-11 |
| POL-3 | applied | a search answer (and each streamed source answer) rewrites the cover of the recent rows it names; a stored cover is never given up for a provider's address | `recent::tests::a_fresh_read_replaces_the_remembered_cover` (red on a stub; three assertions on "stored is kept" were added with the code); `shell.spec.ts` "palette: Ctrl+Enter adds ..." (a stale address put into `localStorage`): red on the old build | `web/src/recent.rs`, `palette.rs` | LC-12 |
| POL-4 | applied | the switch is remembered only when it had the focus when pressed; the focus is handed back without scrolling; the selector is kept in the `Offer` | `notifications.spec.ts` "channels, rules ...": a dispatched click (focuses nothing) leaves the focus on the page: red on the old build. The offer answered after a look at another tab: green with and without the kept selector (that view is not rendered afresh by a tab change), a guard only | `web/src/notify_rules_view.rs`, `keyboard.rs` (`focus_is_on`, `focus_in_place`), `web/Cargo.toml` (web-sys `FocusOptions`) | LC-13 |
| POL-5 | recorded, no change | no timeout added; the orchestrator judges `sharing.spec.ts:55` in the gate of the main checkout | - | - | LC-18 |
| POL-6 | applied | podcast hints of the date choice say "Mark podcast" | `item_words::tests::the_date_hints_of_a_podcast_name_its_own_buttons`: red | `web/src/item_words.rs`, `episode_list.rs`, `locales/{en,de}.ftl` | LC-14 |
| POL-7 | applied | browser test that holds the page's code back until the cover has failed | `shell.spec.ts` "a cover that failed before the page's code ran ...": green on the code as it was, red on a build with the check at mount switched off (so was "a cover that does not load ...", which shows it took that path on `/library`) | `src/web/tests/shell.spec.ts` | LC-20 |
| POL-8 | applied | (1) the hints wasm test that restated its class is deleted; (2) the cover wasm test seen red with the `error` listener switched off; (3) the server test `item_page::the_episode_rows_of_a_podcast_say_listened` seen red with the podcast words switched off; (5) the palette test takes a provider cover that is there and one that is not from the rows, no `if`; (4), (6) noted, nothing to change | see left | `ui/tests/wasm.rs`, `src/web/tests/shell.spec.ts` | LC-20 |
| POL-9 | applied | a day starts at the first instant its zone shows; an impossible year lists nothing | `app` `activity::tests::a_day_starts_at_its_first_instant_on_the_zones_clocks`, `tests/activity.rs::the_year_filter_follows_the_persons_time_zone` (impossible years): both red | `app/src/activity.rs` | LC-15 |
| POL-10 | applied | Los Angeles, Sydney, Santiago | `tests/activity.rs::the_year_filter_west_of_utc_and_in_southern_daylight_time`: a guard, green from the start | `app/tests/activity.rs` | LC-15 |
| POL-11 | recorded as known | `http://` covers on an https instance show the icon / title box until stored; in operations.adoc and v1-scope.adoc | - | docs | LC-16 |
| POL-12 | checked, not changed | at 320 px the hints are in the field's description and in the shortcut help, which reflows | `shell.spec.ts` "palette at 390 px ..." (320 px part): a check, green from the start | `src/web/tests/shell.spec.ts` | LC-17 |
| POL-13 | applied | operations.adoc "Which hosts a browser contacts for pictures"; link from "Limits to know" | - | `docs/modules/ROOT/pages/operations.adoc`, `v1-scope.adoc` | LC-19 |
| POL-14 | applied (1, 3); (2) for the merge of lane B | anime counts as a show; `source_id`, `kind` and `title`. The books line of v1-scope.adoc has to be updated when lane B (v1-books) merges: merge checklist of lane B | - | `v1-scope.adoc` | LC-19 |
| POL-15 | confirmed | the two focus items sec0 left open (F1: focus on the pressed switch; F2: focus back on "Edit") were built in this package (table above, LC-7) and reworked by POL-4 (LC-13). `aria-current` on every entry of the account menu is part of P1 (LC-1, LC-10) | - | - | - |
| POL-16 | applied (1 to 3), deferred (4) | (1) LC-5 worded as what happens; (2) `on:load` in `command::Leading`; (3) `account_entry` by prefix; (4) "Log a play of <title>" and the tab "Plays" of a video game: not built (outside P3, an owner wording) | `ui/tests/wasm.rs::a_command_row_whose_picture_fails_...` (extended), `shell::tests::five_sections_and_an_admin_entry_only_for_admins` (extended): both red | `ui/src/command.rs`, `web/src/shell.rs` | LC-20 |
| POL-17 to 20 | info | nothing to do here. For the merge: `shell.spec.ts` line 1 from main, line 3 from the lane; the comment at `IMAGES` says the server of the tests is on http | - | - | - |
| O-1 | applied | radio buttons are drawn by the page (`ui::RADIO`): empty ring, thick ring when chosen; all seven radios of the app | `item.spec.ts` "the winner radios of the play sheet are rings ...": red (`appearance: auto`) | `ui/src/form.rs`, `ui/src/lib.rs`, `web/src/{log_sheet,list_share,setup}.rs`, `web/src/admin/{users,sso,instance}.rs` | LC-21 |
| O-2 | applied | a board game's row says "Play logged" / "Partie eingetragen" (Home, History, item History tab) | `history::tests::a_board_game_row_says_play_logged`, `item.spec.ts` "a play shows its people ...": both red | `web/src/history.rs`, `locales/{en,de}.ftl` | LC-22 |

Notes:

- The tests whose red needed code switched off were run against sources with that code
  disabled (marked, restored right after; `grep MUTATION` is empty): the `error` listener of
  `Cover` (web wasm test), `EpisodeWords::of` (server page test), the check at mount of
  `Cover` and the kept selector of the offer (one `cargo leptos build`, browser tests).
- The pointer check of POL-4 sits before the click on "Lent game due back": after that
  click turning "Link request" on again asks the defaults offer (existing behaviour: the
  channel counts as new and a suggested event is off), which a first version of the check ran
  into.
- Noticed, not built: the native checkboxes are still drawn by the browser (LC-21);
  `natural_width() == 0` also holds for an SVG without a size (for v1-manual, LC-20);
  the docs sentence about the content security policy describes main after sec1 (LC-19);
  DEC-640 and the plan name `assets.fetch_policy`, the key is `storage.fetch_policy`.
- Databases: browser runs used `lc_e2e_r1` to `lc_e2e_r4` and `lc_e2e_final*`, each created
  and dropped by the run. The server tests leave their `test_<uuid>` databases behind
  (other lanes' too: 342 of them appeared during one build of this lane). Dropped here were
  only the ones of this lane's three runs, told apart by the list before and after each run,
  the time-ordered id and the creation time in the seconds the run's tests ran, with no
  connection: 1 (`test_01a1d289...`, the red run), 25 (`test_01a1d2cd...`) and 25
  (`test_01a1d2ff...`). No other `test_%` database was touched.

### Runs of the review fixes (final code unless said)

- `cargo fmt --all --check`: clean. Clippy with `-D warnings`: `-p web -p ui -p app -p server
  --all-targets --features server/gallery,server/all-connectors` clean; `-p web -p ui
  --all-targets --target wasm32-unknown-unknown --features web/hydrate,web/gallery` clean.
- `cargo test -p web --lib`: 224 passed. `cargo test -p ui`: 18 + 31 + 17 passed.
  `cargo test -p ui --target wasm32-unknown-unknown --test wasm`: 25 passed (26 before; one
  deleted, POL-8). `cargo test -p web --target wasm32-unknown-unknown --features hydrate
  --test wasm`: 3 passed. `cargo test -p xtask`: all passed (locale parity with the new keys,
  white-label). These three ran after the two orchestrator items and before the last change
  to `history::what`. `cargo test -p app --lib activity::`: 1
  passed; `--test activity --test activity_person`: 9 + 1 passed.
- Server page tests (`--test it -- item_page:: library_page:: home_page:: auth_pages::
  admin_pages::`, services required): 25 passed, twice (after the review fixes and again on
  the final code).
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`:
  three times (with two fixes switched off for the red run; with everything; again after the
  last change to `history.rs`). The server of the browser tests is the hard link
  `target/debug/lc-server`, ports 3310 to 3314.
- Red runs of the browser tests: on the build from before the review (POL-1, POL-3, POL-4
  failed at their new assertions), on the build with the check at mount switched off (POL-7
  and "a cover that does not load ..." failed), on that build again for O-1 and O-2 (both
  failed).
- Whole `chromium` project, run 1 (the build before the last change to `history.rs`; a
  `cargo test` of this lane ran beside it for a minute, by mistake): 153 passed, 3 failed:
  `item.spec.ts` "a play shows its people ..." (the fix of O-2 did not yet cover the event
  row: fixed after it), `settings.spec.ts:31` and `sharing.spec.ts:55` (both at their 30 s).
- Whole `chromium` project, run 2, on the final build, nothing else of this lane running
  (31.3 min, load average 12 to 13 from the other lanes): 153 passed, 3 failed:
  `sharing.spec.ts:55` (30 s, POL-5), `settings.spec.ts:31` "every settings page reflows and
  passes axe at 360 px" (30 s, inside axe), `people.spec.ts:91` "add a person with a note,
  edit, and delete ..." (120 s: the card menu was opened and then taken out of the page
  again, "element was detached", before "Edit" could be clicked). Every test this package
  changed or added passed.
- The three alone on the same build: `sharing.spec.ts:55` passed (22.7 s); the other two
  failed the same way (load average 9 to 11). Then `people.spec.ts:91` and
  `settings.spec.ts:31` twice each with one worker: all four passed (15 to 16 s; 22 to 24 s).
  Reported, not changed (LANES.md): `settings.spec.ts:31` has the default 30 s and needs 22 s
  on a quiet moment, the same shape as `sharing.spec.ts:55`; `people.spec.ts:91` passed in
  run 1 and in every earlier run of this lane, the People page and the dropdown are not
  touched by this package, and its failure is a menu closed by a late re-render of the cards.
  The orchestrator judges all three in the gate of the main checkout (LC-18).
