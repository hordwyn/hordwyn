# race progress (lane R)

Package: the six-step fix of `docs/fable/round2/race-diagnosis.md` (F1 to F6), test-first.
Worktree `kithkeep-lane-race`, branch `lane/race`, `CARGO_BUILD_JOBS=1`, ports 3700-3799,
database prefix `lr_`, server binary `target/debug/lr-server` (hard link, `E2E_SERVER_CMD`).
Decisions: `docs/fable/round2/race-decisions.md` (LR-1 ...).

Build commands (one at a time, `free -g` first, gate flag checked):
`nix develop -c bash -c 'cd src/rust && CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo ...'`.
Site: `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`.
Playwright: `E2E_PORT=3710 TMDB_MOCK_PORT=3711 OIDC_MOCK_PORT=3712 MAIL_SINK_SMTP_PORT=3713
MAIL_SINK_HTTP_PORT=3714 E2E_DATABASE_URL=postgres://app@127.0.0.1:55432/lr_e2e_<n>
E2E_SERVER_CMD=target/debug/lr-server` (a fresh `lr_e2e_<n>` per run, dropped after it).

Red runs: the browser tests were all written before any app code was changed and run against
a build of the unchanged code (`target/lr-base/lr-server-base` with its `hash.txt`, site
`target/lr-base/site`; `E2E_SERVER_CMD="env LEPTOS_SITE_ROOT=target/lr-base/site
target/lr-base/lr-server-base"`).

## Red run (2026-11-13, base build, 11 new browser tests, 3 workers): 11 failed, each for its reason

- F2 `library.spec.ts` "a page whose hydration panics reloads once ...": `#lib-select-all`
  stays disabled after the panic at `library.rs:454:18` (no reload; the guard lists the
  hydration error and the panic). "... panics again after the reload ...":
  `body[data-hydration-failed]` never attached.
- F4 `people.spec.ts` "a change from elsewhere keeps the open menu ...": the menu "More
  actions for Zed" is gone after the answer with "Elsewhere". `item.spec.ts` "... keeps the
  open actions menu and the open list menu": the menu is gone after the held `item_page`
  answer. `integrations.spec.ts` "a call that arrives under the open menu ...": the menu is
  gone after the refused call.
- F3 `people.spec.ts` "a refetch with the same answer ...": Zed's marked card is a new node
  (0 marked cards).
- F5 `people.spec.ts` / `library.spec.ts` "the event that announces ...": the second read
  started 97 ms / 88 ms after the own one (expected >= 1900 ms).
- F6 `library.spec.ts` "a refetch keeps the kind chips ...": the marked tablist is gone while
  the counts are held. `home.spec.ts` "history keeps the counts of its side panel ...": the
  marked `dl` is gone. `item.spec.ts` "... keeps the lists of the item ...": "Add to list" is
  gone while the lists are held.
- F1 source rule `crates/web/tests/server_memos.rs` (`cargo test -p web --test server_memos`):
  red, `library.rs:357/361/445: reads kinds_now(` (the three header memos). The server
  detector `library_page::every_page_of_many_at_once_has_its_header_rows` was not run before
  F1 (non-deterministic by nature, diagnosis 2.5 T1b); it runs after F1.
- F3 unit test `settings::tests::an_equal_answer_keeps_the_key_of_its_render` (added in this
  session, LR-3's `render_key`): red against a stub that always gave a new key.

## Steps

| Step | State | Tests | Files |
|---|---|---|---|
| F2 | app done, green | `library.spec.ts` "a page whose hydration panics ..." (2): first green; the second failed once on a test-side timing (it read the failure list before the reloaded page had panicked: the marker it waited for was the first page's); the test now waits for the second panic | `web/lib.rs` (`hydration` module), `web/app.rs`, `tests/fixtures.ts`, `tests/library.spec.ts` |
| F1 | app done, green | `server_memos` green, detector green | `web/library.rs` (`Lib::header`, `derived` for `chips`, `all_count`, `tabs`), `web/pages.rs` (`derived`) |
| F4 | app done, green | the three menu tests | `web/people.rs`, `web/game_groups.rs`, `web/integrations.rs`, `web/admin/users.rs`, `web/item.rs` (`ItemMenus`) |
| F3 | app done, green | unit test + `people.spec.ts` "same answer" | `web/settings.rs` (`render_key`, `loaded` needs `T: PartialEq`; every call site already had it) |
| F5 | app done, green | both "the event that announces ..." | `web/events.rs` (`OwnRefresh`), `web/people.rs`, `web/library.rs`, `web/home.rs`, `web/history.rs`, `web/lists.rs` |
| F6 | app done, green | Library chips/tabs and History side panel green; item lists cell: a `Transition` inside the item's keyed render is a new one per refetch and showed its fallback, so the cell now has no boundary of its own and reads under the page's Transition | `web/library.rs` (5 places), `web/history.rs` (`Side`), `web/item.rs` |

Checks so far: `cargo fmt --all`; clippy pedantic wasm (`-p web -p ui`, hydrate,gallery) clean;
clippy pedantic native (`-p web -p ui -p server --all-targets`, server/gallery,
server/all-connectors) clean; `cargo leptos build` ok (twice); the 11 new browser tests (13
with setup) all green on the second build.

Server page tests (`TEST_DATABASE_URL` set, `REQUIRE_SERVICES=1`): `item_page:: library_page::
home_page:: auth_pages:: admin_pages::` 26 passed, the detector among them. (A first run
without `TEST_DATABASE_URL` skipped them all; not counted.) 26 `test_` databases appeared during
the run; other lanes create such databases at the same time, so none was dropped.

Whole `chromium` project (2 workers, other lanes building): 172 passed, 1 failed:
`sharing.spec.ts:55` "promote, accept, ... undo the merge" hit its 30 s test timeout at line
128 (31.5 s; alone it takes 12 s). Run alone with `--repeat-each=4`: 4/4 green, 12-13 s each.
Reported as a load timeout, no timeout changed.

`library.spec.ts` and `people.spec.ts` with `--repeat-each=10` (2 workers, same build): 232
passed, 0 failed, 0 flaky.

Final checks (after the last code change): `cargo fmt --all --check` clean; clippy pedantic wasm
and native clean; `cargo test -p web --lib --test server_memos`: 225 + 2 passed.

State: all six steps done and green. Nothing committed (the orchestrator merges).

## Review fixes (2026-11-13, review `docs/reviews/2026-11-13-race-review.md`)

Scripts in `<scratchpad>/lane-r-fix/` (worktree named by absolute path). Red runs of the browser
tests against the lane's last build before these fixes, kept as
`target/lr-fixbase/{lr-server-fixbase,hash.txt,site}`.

| Finding | State | Tests (red seen) | Files |
|---|---|---|---|
| RACE-1 | done | the detector with F1 reverted under `stress -c 4`: 6 of 97 runs red (kind chips 4x, status tabs 2x; run 98 cut by the time limit); fixed code, same load: 100 of 100 green. Recorded in the test's doc comment and LR-1 | `apps/server/tests/it/library_page.rs` (doc), race-decisions LR-1 |
| RACE-2 | done | `people.spec.ts` "a menu kept open across a refetch leaves the focus ..." red on the base (the second item lost its focus after the differing answer) | `ui/src/dropdown.rs` (`may_take_focus`, `close_on_outside_press`, `active=`), `ui/Cargo.toml`, `web/settings.rs` (`KeptMenu`, `kept_menu`), `web/people.rs`, `web/game_groups.rs`, `web/integrations.rs`, `web/admin/users.rs`, `web/item.rs` (LR-7) |
| RACE-3 | done | `server_memos` new cases red against the old rule (3 of 4 new tests: alias parameter, `Signal::derive` indirection, method scope); green after; the crate scan is clean (one false lead, `history.rs` `latest_page` behind `cfg(hydrate)`, made the rule read the server view) | `web/tests/server_memos.rs` (LR-11) |
| RACE-4 | done | `library.spec.ts` "... panics again after the reload is not marked hydrated and says so" red on the base (no notice) | `web/lib.rs` (`hydration::show_notice`), `web/app.rs` (`body` `data-start-*`), `locales/en.ftl`, `de.ftl` (LR-10) |
| RACE-5 | done | `admin.spec.ts` "a gated change ..." red on the base (`045` stayed after an equal answer) | `web/settings.rs` (`render_again`), `web/admin/security.rs`, `storage.rs`, `instance.rs` (LR-9) |
| RACE-6 | done | `people.spec.ts` "an open menu does not come back ..." red on the base (Zed's menu open again after the undone merge) | `web/settings.rs` (sweep in `loaded`), `web/people.rs` (`pp-page-menu-<id>`, `pp-menu-<tab>-<id>`) (LR-8) |
| RACE-7 | orchestrator | - | - |
| RACE-8 | noted | - | LR-6 |
| RACE-9 | done | the "panics again" test waits for the notice (only the branch that does not reload shows it), then counts main-frame loads (2) and reads the retry key ("1"); no fixed wait | `tests/library.spec.ts` |
| RACE-10 | kept as is | the strong check after the release stays next to the two-frame check | - |
| RACE-11 | done | `watchPolicy`'s `failures` is required | `tests/fixtures.ts` |
| RACE-12 | done | the two reload tests count `panicked at` only (a release build prints no detailed hydration message) | `tests/library.spec.ts` |
| RACE-13 | done | `library.spec.ts` "a page whose hydration panics reloads once ..." red on the base (the text typed before the reload was gone) | `web/lib.rs` (`save_drafts`, `restore_drafts`, `announce_reload`), `web/Cargo.toml` (web-sys features) |
| RACE-14 | done | `ItemPage` 87 lines (`follow_item_events`, `reset_on_other_item`) | `web/item.rs` |

Two things the first full browser run on the fixed code showed, both fixed in the app:
- `sharing.spec.ts:55` "Unlink Carl?" never opened: a linked person has a card on "All" and
  on "Linked accounts" with one kept key; the press into the visible menu was outside the hidden
  one, which closed the shared flag. Card menus are keyed by their tab now (LR-8).
- `library.spec.ts:526`: the typed text was put back at once after `hydrate_body`, and the
  effects hydration had queued set the field from its signal again (a debug run logged both
  writes). The text is put back in a timer now (LR-10).

Test databases: the 98 + 1 + 100 `test_<uuid>` databases of the detector runs are mine (names in
`<scratchpad>/lane-r-fix/det-*.log.new`); dropping them was refused by the session's permission
check, so they are left for the orchestrator. `lr_e2e_r1` to `lr_e2e_r4` (and the final run's)
are this lane's e2e databases, also left.

Runs after the last code change (one cargo or Playwright command at a time, `CARGO_BUILD_JOBS=1`):
- `cargo fmt --all --check` clean; clippy pedantic wasm (`-p web -p ui --all-targets`,
  `web/hydrate,web/gallery`) and native (`-p web -p ui -p server --all-targets`,
  `server/gallery,server/all-connectors`) clean.
- `cargo test -p web --lib --test server_memos`: 225 + 5 passed.
- Server page tests (`item_page:: library_page:: home_page:: auth_pages:: admin_pages::`,
  `TEST_DATABASE_URL`, `REQUIRE_SERVICES=1`): 26 passed.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery` ok;
  served as `target/debug/lr-server` (hard link), ports 3710-3714.
- Whole `chromium` project (2 workers, other lanes building): 175 passed, 0 failed.
- `library.spec.ts people.spec.ts --repeat-each=10` (2 workers, same build): 252 passed, 0 failed,
  0 flaky.
- An earlier whole run on the build before the last two app fixes: 170 passed, 5 failed: the two
  fixed above, `settings.spec.ts:31` and `sharing.spec.ts:150` (hydration marker not within its
  20 s, load average about 9 on 4 CPUs; both green when run again), and `item.spec.ts:1392` once
  (`duration_minutes` stored as null after "1:30" was typed into the log sheet; green alone and in
  the final whole run; not touched by this package, reported). `sharing.spec.ts:55` alone,
  `--repeat-each=3`: 3/3 green in 20-27 s (12 s on a quiet host): the load timeout noted before.

State: review findings applied (RACE-7 is the orchestrator's gate). Nothing committed.
