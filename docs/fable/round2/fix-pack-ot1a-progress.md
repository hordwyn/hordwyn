# Fix pack ot1a - progress log

Brief: `docs/handoff/fable-briefs/F5-fix-pack-ot1a.md`. Nothing is committed. One block per
item: state, tests run and their result, files touched, DEC numbers. A later run continues
from the first item that is not `done`.

How the tests are run here (the orchestrator runs `cargo xtask ci` / `e2e`, not this pack):

- everything inside `nix develop`, from `src/rust`, with `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`;
- unit: `cargo test -p <crate> ...`; browser unit: `cargo test -p ui --target wasm32-unknown-unknown`;
- Playwright by hand on own ports and an own database, so a parallel `cargo xtask e2e` is not
  disturbed: `cargo leptos build --bin-features gallery,all-connectors --lib-features
  hydrate,gallery`, `createdb` `f5e2e_<n>` on 55432, then in `src/web`:
  `E2E_PORT=3110 TMDB_MOCK_PORT=3111 OIDC_MOCK_PORT=3112 MAIL_SINK_SMTP_PORT=3113
  MAIL_SINK_HTTP_PORT=3114 E2E_DATABASE_URL=postgres://app@127.0.0.1:55432/f5e2e_<n>
  npx playwright test <spec> -g "<title>"`.

## Log

### Baseline (2026-11-10)

All eight new Playwright tests were written first and run against the build of `main`
(8 failed, each for its own reason: gap 0 px; "More" names the duration; no `episodes-note`;
`row.play` undefined; link 36 px / 14 px against 32 px / 12 px; `main` 1152 px wide; no option
for a typo; palette see item 6).

### 1. OT-8 History tab label spacing - done

- Fix: `gap-1.5` on `TabsTrigger` (the `" "` in `item.rs` stays: it is the space in the
  accessible name).
- Tests: `ui/tests/render.rs` `a_tab_trigger_keeps_its_children_apart` (red, then green);
  Playwright `item.spec.ts` "the History tab shows its count apart from the label" (red: gap
  0 px; green).
- Files: `crates/ui/src/tabs.rs`, `crates/ui/tests/render.rs`, `src/web/tests/item.spec.ts`.
- DEC: none (the fix the diagnosis names).

### 3. OT-12 card action sizes on People - done

- Fix: new `ui::ButtonLink` (an `<a>` with the classes of `Button`, minus `disabled:`); the
  People card's "History" link is `ButtonLink size=Sm variant=Outline`.
- Tests: `ui/tests/render.rs` `button_link_is_a_link_that_looks_like_the_button_of_its_size`
  (red: no `ButtonLink`; green); Playwright `people.spec.ts` "the actions of a person card
  share one size" (red: 36 px / 14 px against 32 px / 12 px; green, with axe);
  `people.spec.ts` "cards and the person page show the stats..." still green.
- Files: `crates/ui/src/button.rs`, `crates/ui/src/lib.rs`, `crates/web/src/people.rs`,
  `crates/ui/tests/render.rs`, `src/web/tests/people.spec.ts`.
- DEC: see the DEC list at the end (ButtonLink).

### 4. OT-14 shell max width 80 rem - done

- Fix: `max-w-6xl` -> `max-w-7xl` (80 rem) on `main`, the top bar and the offline line
  (`shell.rs:358`, `:404`, `:825`), so they stay one column.
- Tests: Playwright `shell.spec.ts` "wide screens: the page, the top bar and the status line
  are 80 rem wide" (red: 1152; green).
- Files: `crates/web/src/shell.rs`, `src/web/tests/shell.spec.ts`.
- DEC: see the DEC list at the end (one width everywhere, not the 96/80 split of the UX pass).

### 2. OT-16 duration in the first row of the play sheet - done

- Fix: the duration field moved out of "More" into row 1 of `PlayFields`, beside the date
  chips (wraps under them on a phone); the "More" summary no longer names it; label
  "Duration (min)" / "Dauer (Min.)" and a 6 rem field so date and duration fit the 398 px of
  the desktop sheet (measured: chips 261 px). Only `PlaySheet` has a duration; the watched
  sheet is unchanged.
- OD-01: a play of one person opens with 5 inputs (date, duration, people finder, score,
  winner); the test counts them.
- Tests: Playwright `item.spec.ts` "the play sheet shows the duration in its first row,
  without opening More" (red: summary names the duration, field hidden; green);
  "the play sheet logs players, scores and the winner" still green.
- Files: `crates/web/src/log_sheet.rs`, `crates/web/locales/en.ftl`, `de.ftl`,
  `src/web/tests/item.spec.ts`.
- DEC: see the DEC list at the end (label, what stays behind "More").

### 5. Palette extra defects - done

- Client filter: `ui::Command` has a new prop `filter` (default: filter as typed); the palette
  turns it off for server results and keeps it for the idle list (commands, recent items).
- `search_id` race: `palette.rs` `Stream` holds `search.results` events that arrive while a
  search is in flight and hands the ones of that search over when the answer names its id
  (bounded at 32, cleared by a newer search). No API change.
- Tests: wasm `command_shows_items_as_given_when_filtering_is_off` (red against the old
  component, green); web unit `palette::tests::a_batch_that_beats_the_answer_waits_for_it`
  and `nothing_is_held_without_a_search_in_flight_and_a_new_search_starts_clean` (red: no
  `Stream`; green); Playwright `shell.spec.ts` "palette: a typo still lists what the server
  found" (red: no option; green).
- Files: `crates/ui/src/command.rs`, `crates/ui/tests/wasm.rs`, `crates/web/src/palette.rs`,
  `src/web/tests/shell.spec.ts`.
- DEC: see the DEC list at the end (buffer instead of a client-made id).

### 6. OT-1 palette keyboard - done

- Baseline of the Playwright test (soft assertions, one run): arrow keys DO move the active
  option and `aria-activedescendant`, focus stays in the field, and a pointer resting over
  the list does not take the row (candidate 4 NOT proven in Chromium, no change made for it).
  Proven: candidate 3 (the late batch moved the active row back to the first), candidate 1
  (no outline: the tint alone), candidate 2 (last row: viewport ratio 0).
- Fix in `ui::Command`: the active option is kept by item id (a new query starts at the
  first row; an item that leaves the list falls back to the first row); the active row has
  `outline-2 -outline-offset-2 outline-ring` on top of the tint (token `--ring`; the test
  measures >= 3:1 against the row and the list in light and dark); arrow keys scroll the row
  into view (`scrollIntoView({block: "nearest"})`, again on the next frame because the key
  hints make the active row higher; the first row also scrolls the list to its top so the
  group heading shows).
- Test support: `tmdb-mock.mjs` answers a show search that starts with "fight club" 1.5 s
  late with one synthetic show ("Fight Club Nights", id 990002).
- Tests: Playwright `shell.spec.ts` "palette: arrow keys move through provider results; ..."
  (red as above; green); wasm `command_keeps_the_active_option_when_items_are_appended` and
  `command_scrolls_the_active_option_into_view_and_marks_it` (red against the old component,
  green); the whole `ui` wasm suite (20) green; `gallery.spec.ts` (13), `keyboard.spec.ts`
  (6) and the other palette test of `shell.spec.ts` green.
- Files: `crates/ui/src/command.rs`, `crates/ui/Cargo.toml` (web-sys features
  `ScrollIntoViewOptions`, `ScrollLogicalPosition`), `crates/ui/tests/wasm.rs`,
  `src/web/tests/shell.spec.ts`, `src/web/tests/tmdb-mock.mjs`.
- DEC: see the DEC list at the end.

### 7. OT-15 minimum: a bulk mark with nothing unwatched says so - done

- Fix: `episode_plan::Bulk` / `plan_bulk` (episodes to log, or the message key);
  `episode_list.rs` shows the message in a `role="status"` line (`data-testid="episodes-note"`),
  cleared by the next mark that logs and by choosing another season. Strings
  `episodes-nothing-up-to|season|show` in en and de. No rewatch marking.
- Tests: web unit `episode_plan::tests::a_bulk_mark_with_nothing_open_says_so` (red: no
  `Bulk`; green, 10 episode tests green); Playwright `item.spec.ts` "a bulk mark with nothing
  left to mark says so" (red: no status line; green); the three existing `episodes: ...`
  tests green.
- Files: `crates/web/src/episode_plan.rs`, `crates/web/src/episode_list.rs`,
  `crates/web/locales/en.ftl`, `de.ftl`, `src/web/tests/item.spec.ts`.
- DEC-501.

DEC numbers so far: DEC-496 (OT-14), DEC-497 (OT-12), DEC-498 (OT-16), DEC-499 (palette
filter and race), DEC-500 (OT-1), DEC-501 (OT-15). OT-8 needed none.

### 8. OT-11 read side: score, win, outcome and people on play rows - done

- Data: `ActivityPersonDto.score` / `.winner`, `ActivityRowDto.play {duration_minutes, coop}`,
  `ItemPageDto.plays` (all with serde defaults); `app::ActivityPlay`, `ItemPlay`,
  `App::item_plays`; mapping in `api/src/browse.rs`, `ops.rs`. No migration, no new SQL.
- Display: `web::history::play_line` adds a line to the activity row used by History, Home
  and the item History tab; the item History tab fills people and play from `page.plays`
  (`item::history_rows`). Strings `history-play-*` in en and de.
- Tests: app `tests/activity.rs`
  `a_play_row_carries_scores_the_winner_the_duration_and_the_outcome` (red: fields and
  `item_plays` missing; green); api `tests/rest.rs` `a_play_with_players_scores_and_a_winner`
  extended through the helper `play_is_read` (green; its red was the same missing contract,
  seen in the app test and in Playwright); web unit
  `history::tests::a_play_row_says_who_won_the_scores_and_how_long` (red: no `play_line`;
  green, en and de) and `item::tests::the_history_rows_of_plays_carry_their_people_and_scores`
  (written after the function, so not seen red; green); Playwright `item.spec.ts` "a play
  shows its people, scores, winner and duration on the item History tab and on History"
  (red: `row.play` undefined; green). The OT-16 Playwright test now also checks that the
  duration typed in row 1 is stored and shown (green).
- Files: `crates/contracts/src/browse.rs`, `lib.rs`; `crates/app/src/activity.rs`, `lib.rs`,
  `tests/activity.rs`; `crates/api/src/browse.rs`, `ops.rs`, `tests/rest.rs`;
  `crates/web/src/history.rs`, `history_state.rs` (test row), `item.rs`, `locales/en.ftl`,
  `de.ftl`; `src/web/tests/item.spec.ts`.
- DEC-502.

### After all items (2026-11-10)

- `cargo fmt --all --check`: clean.
- clippy pedantic as in `xtask ci`: workspace native (`--all-targets`, the native feature
  set) and wasm (`-p web -p ui`, `web/hydrate,web/gallery`): both exit 0.
- `cargo test`: `-p ui` (native 14 + 31 + 10; wasm 20), `-p contracts` (9), `-p app` (all
  green, activity 6), `-p api` (13 + 16 + 1), `-p web --features ssr` (151), `-p xtask`
  (locale parity and key use, white-label, architecture): all green.
- `cargo test -p server --features gallery,all-connectors --test item_page --test shell
  --test home_page --test sharing` with `TEST_DATABASE_URL` and `REQUIRE_SERVICES=1`: 15
  passed.
- Not run: `cargo xtask ci`, `cargo xtask e2e` (the orchestrator's), cargo-deny, the sqlx
  offline check (no SQL changed), the `sso` Playwright project, the load run.

- Final Playwright run on the last build (project `chromium`, 2 workers, own database):
  `item`, `shell`, `people`, `home`, `sharing`, `gallery`, `keyboard`, `i18n`, `library`,
  `lists`, `offline` specs: 82 passed, 0 failed. The other specs (`admin`, `auth`,
  `imports`, `integrations`, `notifications`, `settings`, `signin`, `slice`, `sources`,
  `stats`) were not run.

State: all eight items done, nothing committed. Open points are in the DEC entries
(DEC-496..502) under "alternatives".

## F8 review fixes

Brief: `docs/handoff/fable-briefs/F8-ot1a-review-fixes.md`, review
`docs/reviews/2026-11-10-ot1a-review.md` (OT1A-1..11). Nothing is committed. One block per
finding, written when it is done; a later run continues with the findings that have no block.
DEC numbers of this pack: DEC-503..529 only (530 upward belongs to another agent).

### OT1A-3 batch read of the plays - done (code and tests; DEC text see the end)

- Fix: port `SessionRepo::get_many(&[SessionId])` (default: one `get` per id, used by the
  in-memory store; Postgres: the existing `sessions_where_ids`, answer sorted back into the
  order asked). `App::item_plays` calls it once. No new SQL, no `.sqlx` change.
- Tests: app `tests/activity.rs`
  `the_plays_of_an_item_are_read_in_one_batch_newest_first_and_capped` (red: 50 single reads;
  green); store-pg `tests/phase1.rs` `sessions_are_read_by_ids_in_the_order_asked_on_postgres`
  (written with the Postgres body, so not seen red; green against Postgres on 55432).
- Files: `crates/app/src/ports.rs`, `crates/app/src/activity.rs`,
  `crates/store-pg/src/repos/sessions.rs`, `crates/app/tests/activity.rs`,
  `crates/store-pg/tests/phase1.rs`.

### OT1A-10 (a) item_plays test can fail on order and cap - done

- `a_play_row_carries_...` now asserts the ids as `[together, scored]`; the new test above
  logs `MAX_ITEM_PLAYS + 1` plays and expects the newest 50 in order.
- Proof by mutation (each run once, then restored): without `.rev()` both tests fail; without
  `.take(MAX_ITEM_PLAYS)` the new test fails.
- (b) see the block of the web tests below.

### OT1A-10 (b) the history rows test seen red - done

- `history_rows` set back to its old body once (`session_id`, `participants`, `play` empty):
  `item::tests::the_history_rows_of_plays_carry_their_people_and_scores` fails (151 passed,
  1 failed); body restored.

### OT1A-4 German shared win - done (code and unit test)

- Fix: `history-play-won` in `de.ftl` is "Gewonnen: { $names }" (takes the nominative "Du");
  one winner reads "Gewonnen: Ben". English unchanged.
- Tests: `history::tests::a_play_row_says_who_won_the_scores_and_how_long` has the `de` tie
  ("Gewonnen: Du, Ben") and the changed single winner; green in the 151 + 1 run above.
- Files: `crates/web/locales/de.ftl`, `crates/web/src/history.rs`.

### OT1A-1 stale results - code and unit test done, Playwright green still to run

- Fix: `palette::filter_as_typed(query, answered)`: the client filter is off only while the
  answer on screen (`Asked.query`) is the one of the text in the field. Rows of an earlier
  query, and the rows left after a failed search, are filtered as typed.
- Tests: web unit `palette::tests::results_of_another_query_are_filtered_as_typed` (red: no
  such function; green); Playwright `shell.spec.ts` "palette: rows of the previous query are
  filtered by the new text until its answer is there; Enter opens no stale row" (red on the
  F5 build: 22 options where 0 are expected; the search request is held by `page.route`, so
  the order is fixed, then aborted for the failed-search case).
- Files: `crates/web/src/palette.rs`, `src/web/tests/shell.spec.ts`.

### OT1A-2 and OT1A-5 the OT-15 note - code and unit test done, Playwright green still to run

- Fix: `EpisodeUi.note` holds the bulk mark that logged nothing (`Bulk`), not its text; the
  line shows `episode_plan::standing_note(rows, note)`, so a refetch whose rows no longer
  support the message drops it. `EpisodeUi::reset()` (note, error, ask, season) runs in the
  id-change effect of `ItemPage`. OT1A-5: `say_nothing_to_mark` empties the line and fills
  it on the next animation frame, so the same mark is a change of the live region again.
- Found on the way (on HEAD too, not from the review): a client-side navigation from one item
  page to another (the palette on an item page) kept showing the FIRST item: the keyed
  `<For>` of `ItemPage` had only `(version, kinds)` as its key. The key now holds the item
  id (`item::item_key`). The review's scenario "the note follows to another show" could not
  even be reached before this; the Playwright test navigates that way and needs it.
- Not as the review assumed: an undo on another device does not refetch an open item page
  (it hears `entity.updated` of the item only). The test undoes an episode through the API
  and then logs the next step on the page, which refetches.
- Tests: web unit `episode_plan::tests::a_bulk_mark_with_nothing_open_says_so` extended
  (red: no `standing_note`; green); Playwright `item.spec.ts` "a bulk mark with nothing left
  to mark says so" extended, each part seen red on the F5 build in a run of its own: the
  repeated mark (no mutation of the status line), the refetch (note still up at 9/10), the
  other show (page still shows the first show).
- Files: `crates/web/src/episode_list.rs`, `episode_plan.rs`, `item.rs`,
  `src/web/tests/item.spec.ts`.

### OT1A-6, OT1A-7 - test changes made, runs still to do

- OT1A-6: `shell.spec.ts` asserts that "Fight Club Nights" is absent right after the two
  arrow keys. OT1A-7: the mock delays only `/^fight club \d+$/` (the palette test's query).

### OT1A-8 kept active row scrolls into view - done

- Fix: `ui::Command` `follow_chosen`: when the position of the chosen item changes while the
  item stays the same (the list changed under it), the row is revealed on the next frame.
  Key and pointer moves are not followed (they change the item); rows added elsewhere do
  not scroll.
- Tests: wasm `command_scrolls_the_kept_active_option_into_view_when_the_list_is_replaced`
  (red: "the list did not follow the active row"; green); the `ui` wasm suite: 21 passed.
- Files: `crates/ui/src/command.rs`, `crates/ui/tests/wasm.rs`.

### OT1A-9 class-name assertions - applied

- `render.rs`: the tab trigger test asks for `inline-flex` and any `gap-*`; `wasm.rs`: the
  two `outline-ring` assertions are gone (the Playwright test measures the outline in both
  themes).

### Playwright on the F8 build (2026-11-10)

- Build: `cargo leptos build --bin-features gallery,all-connectors --lib-features
  hydrate,gallery`; fresh database `f8e2e_2`, ports 3110..3114, project `chromium`, 2 workers.
- `item`, `shell`, `people`, `imports`, `slice`: 38 passed, 0 failed (this closes OT1A-1,
  OT1A-2, OT1A-5, OT1A-6 and OT1A-7: `imports` and `slice` are green with the narrowed mock).
- Other specs were not run.

### OT1A-11 hold buffer - deferred (DEC-510)

- No code change: both proposed evictions only move which event is lost; the real fix is a
  search id made by the client (API change, DEC-499 alternatives).

DEC entries of F8: DEC-503 (OT1A-1), DEC-504 (OT1A-2, OT1A-5), DEC-505 (item to item
navigation, found on the way), DEC-506 (OT1A-3; the text of DEC-502 is marked as corrected),
DEC-507 (OT1A-4), DEC-508 (OT1A-8), DEC-509 (OT1A-6, -7, -9, -10), DEC-510 (OT1A-11
deferred). They stand after DEC-502 in `docs/DECISIONS.md` (DEC-530 upward is another pack).

### After all findings (2026-11-10)

- `cargo fmt --all --check`: clean. Clippy pedantic as in `xtask ci`, workspace native
  (`--all-targets`, the native feature set) and wasm (`-p web -p ui`): both exit 0 (one
  `let ... else` in `episode_list.rs::bulk` fixed after the first Playwright run).
- `cargo test`: `-p web --features ssr` (152), `-p ui` (native 14 + 31 + 10; wasm 21),
  `-p app` (all files; activity 7), `-p api` (13 + 16 + 1), `-p xtask` (locales, white-label,
  architecture and the others), `-p store-pg` with `TEST_DATABASE_URL` and
  `REQUIRE_SERVICES=1` (all files; phase1 9): all green.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, cargo-deny, the sqlx offline check (no SQL
  changed), `-p server` tests, the Playwright specs other than the five named above, the
  `sso` project, the load run.
- Final Playwright run on the last build (after the `let ... else` change; fresh database,
  project `chromium`, 2 workers): `item`, `shell`, `people`, `imports`, `slice`: 38 passed,
  0 failed. `cargo test -p server --features gallery,all-connectors --test item_page --test
  shell` with the test database: 3 + 5 passed. The databases `f8e2e_1..3` are dropped.

State: OT1A-1..10 applied, OT1A-11 deferred (DEC-510), one extra fix (DEC-505). Nothing
committed.
