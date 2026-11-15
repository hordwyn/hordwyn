STATUS: COMPLETE (2026-11-09)

= Test-suite audit of kithkeep (main = ux4)

Read-only audit, 2026-11-09. Nothing was compiled or run; every statement comes from reading
the sources. Rust paths drop the `src/rust/` prefix; Playwright paths are under `src/web/`.
Inputs read first: `docs/HANDOFF.md` sections 2 and 8; `docs/DECISIONS.md` entries on flakes,
deferred tests and the test-driven records (DEC-114, 237, 281, 288, 307, 335, 383, 396, 407,
415, 421, 430, 434, 438, 445, 451, 457, 460, 482, 485).

== 0. Summary

The suite is large (about 840 Rust tests plus 121 Playwright tests and a setup project) and
mostly written at the right layer: use-cases against the in-memory adapters in `crates/app`,
the same scenarios again on Postgres in `crates/store-pg`, HTTP through the real router in
`apps/server/tests`, and the browser for what only a browser can show. Pure tautologies are
rare. The weaknesses are structural:

1. The Playwright suite runs fully parallel against one server, one database and two shared
   admin accounts, and several specs mutate instance-wide state (hosting mode, the admin's
   time zone, sessions of `REAUTH_ADMIN`, a source switch, an instance-wide notification
   default). Those are order-dependent today and only pass because the windows are short.
2. CI retries every failed Playwright test once (`playwright.config.ts:22`), so a flake that
   the owner never sees locally passes the pipeline. Nothing records how often that happens.
3. A strictly ordered shared import/refresh job lane couples every spec's wall time to every
   other spec's; the 90-240 s timeouts are the symptom.
4. A dozen tests were written together with the code and never seen red (the DEC records say
   so honestly); nobody has checked that their assertions can fail.
5. Nine REST routes have no HTTP-level test at all; the web app reaches the same use-cases
   through server functions, so a CLI or Tauri client would be the first to hit them.
6. Without `TEST_DATABASE_URL`, about 480 tests print "skipping" and pass. `cargo test` on a
   developer machine is green while testing roughly a third of the suite.

== 1. Inventory

Counts are `#[test]`/`#[tokio::test]`/`#[wasm_bindgen_test]` occurrences (integration = files
under `tests/`, unit = in `src/`). Layers: D = domain rules, A = app use-cases on MemStore,
P = Postgres adapters, H = HTTP through the axum router, S = SSR page markup, B = browser.

=== 1.1 Rust integration tests (about 480, 4 ignored)

[cols="2,1,1,3"]
|===
| Crate / binary | Tests | Layer | What it covers

| `apps/server/tests` (25 binaries)
| 101 (1 ignored)
| H, S
| Full server (`Server::build` on a fresh database per test): admin API and pages, auth pages,
  CLI subcommands, instance settings/audit/mail, OIDC (16 tests, 2586 lines, one `#[ignore]`
  live-provider test), imports, notifications, offline sync, usage funnel, webhooks, sharing,
  member PIN, retention, latency (SSE ordering), the DEC-384 hang guard (`ssr_routes.rs`) and
  the `#[ignore]` load test (`load.rs`).

| `crates/api/tests` | 17 | H (in-memory adapters) | REST flows against testkit fakes: auth,
  library, sharing, notifications, imports/export, body limits, lock-out, phase-1 pages, plays,
  offline convergence (`sync.rs`).

| `crates/app/tests` (19 files) | 88 | A | Every use-case family on MemStore: library, browse
  (keyset), bulk and undo, lists, people/companions/sessions, episodes, goals, game groups,
  stats, activity feed, catalog audience, notifications fan-out, next step.

| `crates/store-pg/tests` (11 files) | 29 | P | Postgres parity: phase-1 scenarios, use-case
  round trips, keyset paging, stats cache vs full statement vs Rust rules, two race tests
  (`catalog_race.rs`, `stats_race.rs`), outbox, retention, windows, episodes, goals/groups,
  catalog audience, connection retry.

| `crates/auth/tests` | 26 | A, P | Store round trips on Postgres, admin store, key reload,
  OIDC client (wiremock issuers), sign-up rules, throttle.

| `crates/catalog-core/tests` | 25 | A | Governed HTTP client (wiremock), credentials, search
  fan-out, retention, shared source stats on Postgres.

| `crates/connectors/tests` | 22 (1 ignored) | A | TMDB, TheTVDB, IGDB, Podcast Index against
  recorded fixtures; one live TMDB test.

| `crates/importers/tests` | 21 (1 ignored) | A, P | Generic JSONL, legacy export, Trakt,
  schema, Postgres import store; one `#[ignore]` full-owner-export test.

| `crates/assets/tests` | 26 | A, P | Budget/eviction, asset service, Postgres index, one S3 test.

| `crates/notify/tests` | 17 | A, P | Channels (real HTTP via reqwest to wiremock), engine,
  Postgres store.

| `crates/jobs/tests` | 11 | P | Queue ordering, leases, dedupe, cron, worker loop.

| `crates/ui/tests` | 56 | S, B | 39 SSR markup/ARIA tests (`components.rs`, `render.rs`) and 17
  wasm-bindgen tests in headless Chromium (`wasm.rs`).

| `crates/brand/tests` | 12 | D | Contrast and fonts.
| `crates/kinds/tests` | 7 | D | Built-in kinds, schema currency, dummy kind.
| `crates/search/tests` | 3 | P/Meili | Indexer, suite (gated by `TEST_MEILI_*`).
| `crates/integrations/tests` | 4 | A | Watched hook.
| `xtask/tests` | 15 | meta | Architecture rule (`cargo metadata`), white-label scan, locale
  parity, e2e log scan, load args.
|===

=== 1.2 Rust unit tests (about 358)

`web` 146 (state reducers, i18n, offline queue, funnel, keyboard, forms), `auth` 38, `domain`
20, `notify` 18, `config` 17, `ui` 14, `api` 13, `xtask` 13, `assets` 12, `importers` 12,
`catalog-core` 11, `contracts` 9, `server` 6, `app` 5, `brand` 5, `realtime` 4,
`integrations` 4, `search` 3, `connectors` 2, `jobs` 2, `cli` 2, `store-pg` 1, `telemetry` 1,
`kinds` 0, `testkit` 0.

=== 1.3 Playwright (121 tests + 1 setup, 21 specs)

`admin` 12, `gallery` 13, `signin` 11 (own serial project), `shell` 10, `item` 8, `library` 8,
`offline` 8, `keyboard` 6, `home` 6, `auth` 6, `settings` 7, `people` 5, `sources` 4,
`imports` 3, `sharing` 3, `i18n` 2, `integrations` 2, `lists` 2, `notifications` 2, `stats` 2,
`slice` 1. Three node mocks (`tmdb-mock.mjs`, `oidc-mock.mjs`, `mail-sink.mjs`) and the real
server binary with Postgres search, memory storage and an ephemeral secrets key
(`playwright.config.ts:63-80`). Axe (WCAG A/AA) runs 84 times across the specs, mostly in
`[1280, 360] x [light, dark]` loops.

=== 1.4 What is gated and what silently skips

Every Postgres-backed test calls `store_pg::testing::fresh_db_url()` (`crates/store-pg/src/testing.rs:30-37`)
and returns early without `TEST_DATABASE_URL` (the `let Some(..) = .. else { return }` pattern
appears about 190 times across the test files). `cargo xtask ci` and GitLab set
`REQUIRE_SERVICES=1`, which turns the skip into a failure, so CI is honest. A plain
`cargo test` is not: all 101 server tests, 29 store-pg, 11 jobs, 9 auth Postgres, 3 search,
3 assets and more pass as skipped. See 7 for the fix.

== 2. Tests that cannot fail, or test the code against itself

Findings here are modest; the suite does not have `assert!(true)` style tests.

=== 2.1 Differential tests whose oracle is production code

* `crates/store-pg/tests/phase1.rs:489-519` `stats_on_postgres_match_the_in_memory_rules`:
  the expected totals and series are produced by `testkit::MemStore` through the same `App`.
  If the Rust rules and the SQL share a misunderstanding (for example both counting a
  compensating undo event), the test agrees with itself. Mitigation present: two literal
  asserts (`pages == 270`, `finished == 3`, lines 501-502). Extend the literal asserts to one
  series point per bucket.
* `phase1.rs:523-560` `rust_rules`/`same_stats`: the full SQL statement is compared with
  `app::month_amounts` (production) and the cache with the full statement. Three production
  paths agreeing is strong evidence of consistency, not of correctness; the literal checks in
  `stats_scenario` carry the correctness. Same for `phase1.rs:911`
  `window_amounts_and_the_year_comparison_on_postgres_match_the_in_memory_rules`.
* `crates/store-pg/tests/windows.rs:45,179,253` compare windowed reads with the full read of
  the same store; fine as an invariant test, but no independent expected value.

=== 2.2 Assertions that are type checks or presence checks only

* `apps/server/tests/setup_page.rs:157` `assert!(st["groups_without_source"].as_array().is_some())`:
  only checks that the field is an array; does not check the Play group is in it (the whole
  point of the "no source" state). Assert the array content.
* `crates/app/tests/bulk.rs:152` `assert!(out.is_ok())` after a bulk call; the per-id report is
  the behaviour, assert on it (the surrounding test may; the `is_ok` alone is weak).
* `apps/server/tests/oidc.rs:1542,2397` `assert!(r.cookie("_session").is_some())` as the only
  evidence of a sign-in; add a `GET /auth/me` with that cookie, as other tests in the file do.
* `crates/catalog-core/tests/governed.rs:259`, `stats_shared.rs:60`,
  `crates/integrations/tests/watched.rs:116`, `crates/notify/tests/pg.rs:178`: `is_some()` on
  timestamps/secrets. Acceptable as part of larger tests; listed for completeness.

=== 2.3 Substring assertions on rendered HTML that match more than intended

`crates/ui/tests/components.rs:20` `assert!(html.contains("disabled"))` is satisfied by
`aria-disabled="false"` or a class name; `render.rs:21` `contains("h-8")` is satisfied by
`h-80`. Use a boundary (` disabled` / `disabled=` / class token split) or parse the markup
(`scraper` is already a dev tool in the SSR page tests of the server, check before adding).
These are not failing today but they will not catch the regression they describe.

=== 2.4 Status-only checks

68 `status == 200`-style asserts in Rust tests and 22 `ok()`/`status 200 -> toBeTruthy` in the
specs. Almost all are set-up guards followed by body assertions, which is correct. The only
tests that are status-only by design are `apps/server/tests/ssr_routes.rs:84-91` (the DEC-384
hang guard: 150 renders must answer 200 within 10 s each) and the SSR smoke parts of
`admin_pages.rs`/`auth_pages.rs`, which do also check H1 and content. No change needed.

=== 2.5 Tests that check what the test itself wrote

* `src/web/tests/admin.spec.ts:293-310`: expected time text is built with `Intl.DateTimeFormat`
  in the same zone; independent oracle, good.
* `src/web/tests/helpers.ts:66-80` `totp()`: a second TOTP implementation in the test; good.
* `src/web/tests/keyboard.spec.ts` and `library.spec.ts` bulk tests assert the client's `PUT`
  body (DEC-430 explains why: the API redacts the stored secret). Acceptable, the keep itself
  is covered by `notify` unit tests.

== 3. Timing- and order-dependent tests

Format: location, why it depends on time or order, the app race it could hide, fix.

=== 3.1 Playwright: shared state across parallel specs

The config is `fullyParallel: true` with one server, one database, one TMDB mock and the two
shared accounts `ADMIN` and `REAUTH_ADMIN` (`playwright.config.ts:21`, `helpers.ts:37-38`).
Only `signin.spec.ts` is isolated in a serial project (`playwright.config.ts:37-44`).

[cols="3,3,3"]
|===
| Location | Dependence and the race it can hide | Fix

| `admin.spec.ts:229-252` hosting mode Home -> Hosted -> Home
| While Hosted, members see `needs_account` instead of "no source" (DEC-247) and the admin's
  no-results card changes. Runs in parallel with `sources.spec.ts:25-44` (no-results card),
  `sources.spec.ts:62-80` and `settings.spec.ts:73-85` (Home nudge), and every member search.
  A hosted-mode text appearing in those tests would look like a flake of the palette.
| Move instance-mutating tests into a serial project after `chromium` (like `sso`), or run
  them against a second server on another port and database. Cheapest: a third project
  `instance` with `fullyParallel: false` and `testMatch` on a `*.instance.spec.ts` name.

| `admin.spec.ts:286,313` `UPDATE users SET time_zone = 'Pacific/Kiritimati'` on `ADMIN`
| `ADMIN` is used concurrently by `notifications.spec.ts:39`, `settings.spec.ts:74`,
  `sources.spec.ts:37,62`. Day grouping and "today" on any admin page flips to UTC+14 for
  the window. Hides nothing, but makes an unrelated failure look like a date bug.
| Use a throw-away admin (sign up, promote via `PUT /admin/users/{id}/role`) in that test.

| `admin.spec.ts:85` and `admin.spec.ts:323` `ageSessions(REAUTH_ADMIN)`
| `ageSessions` ages every session of the account (`helpers.ts:61-66`). Two tests in one file
  may run in two workers at once: test A's proof refreshes its own row, test B's `ageSessions`
  ages A's row again, A's next gated change opens an unexpected re-auth dialog. This is the
  kind of result DEC-288 investigated ("Saved." lost after the dialog).
| One admin account per test (create and promote), or `test.describe.configure({ mode:
  "serial" })` around the two re-auth tests, and make `ageSessions` take a session id.

| `admin.spec.ts:352-372` Podcast Index switched off and on
| The comment says no other spec searches Listen; the palette footer "Data sources" list and
  the Home nudge (`sources.spec.ts:33,67`) do change text for everyone during the window.
  Assertions there are `toContainText`, so it passes today by luck of wording.
| Same serial `instance` project.

| `notifications.spec.ts:37-44,129-130` instance default subscription `boardgame.loan_due`
  for all users, deleted in `finally`
| Every user signing up in parallel inherits the default during the window. No spec asserts
  on `loan_due`, so it is harmless today; it is an invisible coupling.
| Same serial project, or a per-event default that only the spec's users can trigger.

| `admin.spec.ts:343`, `signin.spec.ts:390-392` reset `auth.sign_in_mode = 'page'`
| Correct clean-up. `signin.spec.ts:392` falls back to raw SQL when the API refuses, which
  means a failed test can leave the instance half-configured for the next test in the serial
  project.
| Keep; add the same SQL fallback to `admin.spec.ts:343`.

| `global.setup.ts:71-95` reused server: signs in instead of running the wizard
| With `reuseExistingServer: !CI` (`playwright.config.ts:49,56,62,69`) a stale server binary
  from an earlier build keeps serving; the specs then test old code. The wizard's a11y checks
  are also skipped on a reused server (DEC-457 names this as cut).
| `cargo xtask e2e` should refuse a listener on the port (or kill `server` on it) before
  starting Playwright; or set `reuseExistingServer: false` for the server entry only.
|===

=== 3.2 Playwright: the shared job lane

`helpers.ts:115-131` `importDone` waits up to 120 s; `item.spec.ts:49-57` polls 90 s for a
catalog refresh "behind other specs' jobs on the same lane" (UX3-22); `imports.spec.ts:24,60,93`
30 s each; 25 tests raise `test.setTimeout` to 120-240 s. The import lane is strictly ordered
(DEC-029), so e2e wall time grows with the number of workers, and a slow import in one spec
shows up as a timeout in another. That is not a product bug, but it means the suite's timing
budget is a function of the machine. Fix: a per-user or per-import lane for imports (the
product would benefit too: one member's 64 MiB import should not block another's), or run
`imports.spec.ts`, `item.spec.ts` and `home.spec.ts` seeds through a direct test endpoint.

=== 3.3 Playwright: retries, polls, sleeps and negative assertions

[cols="3,3,3"]
|===
| Location | Dependence | Fix

| `playwright.config.ts:22` `retries: process.env.CI ? 1 : 0`
| A test that fails once in CI is reported "flaky" and the job passes; locally there is no
  retry. Flakes are therefore visible only to whoever reads the HTML report. DEC-485 shows
  how much work one such flake costs once it is noticed.
| Either `retries: 0` everywhere and a nightly `--repeat-each 3` job, or keep the retry but
  fail (or at least print) when the JSON reporter has any `flaky` outcome, so the count is on
  the job page. The `e2e-scan` step is the natural place.

| `gallery.spec.ts:8-11` click "Open dialog" until it opens, `toPass` 15 s
| Covers for a missing hydration marker on `/dev/gallery`; a handler that attaches late on a
  real page would pass here. The gallery is dev-only, so the hidden race is low-risk.
| Add `body[data-hydrated]` to the gallery shell and use `hydrated(page)`.

| `notifications.spec.ts:102-107` reload the page until "Failed" shows, `toPass` 30 s
| Polls the worker through full page reloads; the UI assertion then runs on a page that was
  reloaded an unknown number of times, which hides a page that fails to update live.
| Poll `GET /api/v1/notifications/deliveries` through the API until the row is `failed`, then
  load the page once and assert; or assert the live update (SSE) explicitly.

| `signin.spec.ts:172-174` `waitForLoadState("networkidle")` then "no request to the
  provider" (OD-20)
| Negative assertion by 500 ms of network quiet; a silent redirect fired later (for example
  from a timer-based retry) passes. DEC-320's retry cap is exactly such a timer path.
| Ask the OIDC mock for its `/authorize` hit count at the end of the test (it already has
  an HTTP API), and keep the networkidle wait as the pacing step only.

| `keyboard.spec.ts:191-195` wait for `/_fn/home`, then two `requestAnimationFrame`s, then
  assert focus
| "The answer is rendered a few microtasks after the response" is a framework timing
  assumption; a slower render passes the focus check before the re-render that would move
  it. This is the DEC-490 regression the test exists for.
| Expose a render counter (`data-render` attribute on the Home root incremented by an
  effect) and wait for it to change, then assert focus.

| `home.spec.ts:247-251`, `library.spec.ts:250-253,340-343`, `offline.spec.ts:232`
| `waitForResponse` is registered before the action (correct); the assertions after it depend
  on the response having been applied to the DOM. `library.spec.ts:253` waits for
  `aria-busy != true`, which is the right pattern; `home.spec.ts:249` relies on `frames()`.
| Use the `aria-busy` pattern in `home.spec.ts` too.

| `keyboard.spec.ts:67-70` chord window 1.5 s, `toHaveText("", { timeout: 3_000 })`
| Real timer; asserts only that the hint clears within 3 s, not that it stayed for about
  1.5 s. Harmless.
| Leave.

| `auth.spec.ts:18,54` 5 s for the debounced availability check
| Fixed timeouts on a debounce plus a server call; under CI load the 5 s is the first thing
  to break. Hides nothing.
| Raise to the default `expect` timeout or wait for the request instead.

| `library.spec.ts` 360 px select-all (DEC-485)
| The `toPass` was removed and the checkboxes are `disabled` until an effect runs. The root
  cause was not found ("the change handler did not select anything"). The hardening makes the
  symptom impossible, not the cause. A click on a disabled box is dropped silently by the
  browser, so a slow `live` effect now shows as a Playwright timeout waiting for `enabled`
  (visible, good) instead of a silent pass.
| Keep the hardening; add a `--repeat-each 10` run of `library.spec.ts` to the nightly job so
  the next occurrence is caught with a trace.
|===

=== 3.4 Rust: sleeps, wall-clock asserts and polls

[cols="3,3,3"]
|===
| Location | Dependence | Fix

| `apps/server/tests/member_pin.rs:213` `sleep(200 ms)` then "the instance key is not marked
  rejected"
| A negative assertion after a fixed sleep cannot prove absence: a rejection recorded 300 ms
  later passes. That is precisely the DEC-247 rule the test protects (a member's bad PIN must
  not disable the instance key).
| The rejection, if any, is written in the governed fetch path, which has returned by the
  time `logins(&tvdb, 1)` resolves (line 211). Drop the sleep and assert at once; then run a
  second admin search and assert the mock saw the instance key (positive evidence).

| `crates/catalog-core/tests/governed.rs:122` `elapsed < 2 s`, `:148` `elapsed < 50 ms`
| Upper bounds on wall time in a process that runs its tests in parallel threads on a 4-vCPU
  VM. The 50 ms bound (circuit open -> fail fast) is the likeliest spurious failure in the
  Rust suite.
| The request counts on the mock (`:124-127`) already prove the fail-fast; drop the time
  bounds or use `#[tokio::test(start_paused = true)]` with `tokio::time::advance`.

| `governed.rs:97-104` `elapsed >= 1 s` after `Retry-After: 1`, and `:194` `sleep(1100 ms)`
  to expire a 1 s cache TTL
| Lower bounds are safe but cost real seconds (about 2.2 s per run of the binary). The TTL
  test depends on the cache reading the system clock.
| Inject the clock into the HTTP cache (the app already has a `Clock` port) and advance it.

| `crates/jobs/tests/queue.rs:268-272` lease 1 ms, `sleep(20 ms)`, reclaim; `:411-434` a
  1.5 s handler against a 300 ms lease
| Compares the Postgres `now()` with a lease written moments before; a client/server clock
  skew of more than 20 ms (containers in CI) fails the first. The second burns 1.5 s of
  wall time per run.
| Make the lease deadline come from the database clock on both sides (it may already; the
  test cannot tell), or set the lease in the past with an explicit `UPDATE` in the test.

| `crates/store-pg/tests/use_cases.rs:198-214` connect to `127.0.0.1:1`, patience 1600 ms,
  assert waited in [1400 ms, 10 s)
| Real network timing; a host where port 1 is filtered (DROP) instead of refused answers
  after the TCP connect timeout, which may exceed the patience. Upper bound 10 s protects
  the run.
| Keep the upper bound; replace the lower bound with an attempt counter exposed by
  `connect_with_retry` (returns how many attempts were made).

| `crates/store-pg/tests/stats_race.rs:94,106` jittered sleeps to provoke interleavings
| Probabilistic: if the DEC-460 race returned, this test would be green most of the time.
  The deterministic variant is `:31` (two explicit connections, an uncommitted writer); good.
| Keep both; mark `:73` as a stress test in its doc comment so a future reader does not
  take its green as proof.

| `apps/server/tests/latency.rs:22,68,76,235` `SLOW = 10 s` connector, assert
  `entity.updated >= 9 s`
| The test asserts ordering (the event arrives after the refresh), but pays 10 s of wall
  time to do so and the 40 s SSE loop (`:214-215`) can add more.
| `SLOW = 1 s` keeps the ordering proof; the assertion becomes `>= SLOW - 100 ms`.

| `apps/server/tests/notifications.rs:139-147`, `retention.rs:105-112`, `instance.rs:1100-1122`
| 100 x 100 ms polls of the real worker (10 s cap). Bounded and honest; under CI load 10 s
  may be short for a worker polling every 250 ms (DEC-029) behind other jobs.
| Use a 30 s cap with the poll; the failure message already names what was awaited.

| `apps/server/tests/ssr_routes.rs:66-80` a `spawn_blocking` busy loop building routers while
  150 pages render
| Deliberately order-sensitive (it reproduces DEC-384). It is also the most CPU-hungry test in
  the suite and runs in the same `cargo test` invocation as every other server binary.
| Keep; see 6 for scheduling.

| Process-wide state: `leptos_router`'s process flag (DEC-384), the per-process throttles of
  DEC-155/DEC-184/DEC-320 and the per-process `SourceStats` (DEC-247)
| Tests inside one binary run on parallel threads; each builds its own `Server`, so per-
  instance state is isolated, but anything `static` in `leptos`/`leptos_router` is shared.
  `ssr_routes.rs` guards one known case; there is no general protection.
| Nothing to add now; when the next "never finishes" appears, `ssr_routes.rs` is the template.
|===

No test mutates environment variables (`set_var`/`remove_var` do not appear), and no test
binds a fixed port (all `TcpListener::bind("127.0.0.1:0")`). Good.

== 4. Behaviours with no test

=== 4.1 REST routes without any HTTP-level test

Cross-check of the 150 `utoipa::path` annotations in `crates/api/src` against every test file.
The use-cases behind most of these are tested in `crates/app` and the pages in Playwright
through server functions; the REST handler (DTO mapping, ownership check, status code) is not.

* `DELETE /admin/jobs/{id}` and `POST /admin/jobs/{id}/retry` (`crates/api/src/admin_routes.rs:438-451`):
  no test. DEC-298 lists job retry/discard as audited after the change; that audit row is
  untested too.
* `GET /stats/compare` (`goals_routes.rs:67`): no test at any HTTP layer; the year comparison
  is covered in `app`/`store-pg` only.
* `PUT`/`DELETE /stats/goals/{id}` (`goals_routes.rs:43,55`) and `PUT`/`DELETE
  /game-groups/{id}` (`goals_routes.rs:97,110`): no HTTP test. The ownership question "can
  user B delete A's goal through REST" is tested only at the use-case layer.
* `POST /link-requests/{id}/decline` (`routes.rs:541`): only `app/tests/people.rs`.
* `POST /shared/lists/{token}/library` (`people_routes.rs:179`): no HTTP test (the token-only
  read is tested; adding everything from a shared list is not).
* `GET /notifications/web-push-key` (`routes.rs:825`): one mention in `api/tests/rest.rs`;
  no web-push channel test anywhere (`crates/notify/tests/channels.rs` covers webhook/mail).
* `DELETE /admin/invites/{id}`: only through the UI in `admin.spec.ts`.
* One reference only (one assertion, often a status): `/admin/notification-defaults`,
  `/admin/oidc/providers/{id}/secret`, `/admin/secrets/rewrap`, `/admin/storage/evict`,
  `/auth/logout`, `/companions/{id}/unmerge`, `PUT /library/{item_id}/status`,
  `/lists/{id}/library`, `/notifications/subscriptions/{id}`, `/sources/ask-admin`,
  `POST /stats/goals`.

=== 4.2 Deferred and cut tests still open (from the DEC records)

DEC-457 cut these "for a later hardening package"; none has been added since:

* `reload_keys` on an ephemeral key, a malformed or missing key file, and in the worker role.
* A locked stored SMTP password over the API; `PgMailSecret::rewrap_batch`.
* `ClientConfig.today` over HTTP.
* The back-channel logout budget 429.
* `/sign-in?error=` with the loop-guard cookie (DEC-320).
* The DEC-338 500 attributed with its status.
* The mail step through the re-auth dialog (Playwright).
* Axe at 360 px with a provider panel open in both themes (one theme is covered).
* Server start with encrypted rows and no key over HTTP; `key_unavailable` over HTTP.
* The silent wizard skip on a reused server (`global.setup.ts`).
* A candidate source Test with a live key as `#[ignore]`.

DEC-237's "Saved offline, will sync" and the keyboard map were built in phase 4 and have tests
(`offline.spec.ts`, `keyboard.spec.ts`); "Mark show" (DEC-363) is covered by `item.spec.ts`.

=== 4.3 Tests written with the code and never seen red

The DEC records are explicit about this; the risk is an assertion that cannot fail (for
example asserting a default value that the code also defaults to). Each should be checked once
by breaking the code path on purpose (a five-minute "mutation smoke") before it is trusted:

* DEC-415: `admin.rs` `overview_reads_the_probe_rows_and_never_probes`,
  `catalog-core/tests/stats_shared.rs`, `auth/tests/pg.rs` `locked_totp_count` change.
* DEC-421: `app/tests/browse.rs` `keyset_pages_walk_every_order`, `store-pg/tests/phase1.rs`
  `keyset_pages_walk_every_order_on_postgres`, cursor unit tests, REST cursor checks in
  `api/tests/rest.rs`, `remove_keeps_history_and_bulk_undo_on_postgres`,
  `bulk_edits_500_items_and_refuses_more`.
* DEC-430: `ui/tests/wasm.rs` `command_ctrl_enter_runs_the_secondary_action...` and
  `roving_index_and_chord_run_in_the_browser`; web unit tests in `keyboard.rs`, `recent.rs`,
  `palette.rs`, `notify_channels.rs`.
* DEC-438: all four tests of `apps/server/tests/usage.rs`; the clock test in `offline.rs`; the
  `api::usage` and config unit tests.
* DEC-445: `store-pg/tests/phase1.rs` `window_amounts_...`, `store-pg/tests/goals_groups.rs`,
  domain `goal`/`game_group` unit tests, `stats.spec.ts` (2), `people.spec.ts` (+2).
* DEC-451: `connectors/tests/tmdb.rs` and `tvdb.rs` episode tests, `store-pg/tests/episodes.rs`,
  the `item.spec.ts` episodes test.
* DEC-457: "most passed on their first run" for the whole package (they test existing
  behaviour, which is the expected outcome, but the same check applies).

=== 4.4 Other gaps found while cross-checking

* No test runs the server as two roles (`--role web` + `--role worker`) in separate processes;
  `ServerRole::Worker`/`Web` do not appear in any test. DEC-247 and DEC-407 describe behaviour
  that differs between the roles (per-process stats, jti replay). `oidc.rs`'s two-process test
  uses two `All` servers.
* The `fs` storage backend has no test (`FsBlob`/`backend = "fs"` appear in no test); e2e and
  server tests use `memory`, one test uses S3. The fs backend is the compose default (DEC-202).
* Meilisearch is never exercised end to end: e2e sets `APP__SEARCH__BACKEND=postgres`; the
  Meili path has 3 gated tests in `crates/search/tests`.
* SSE reconnection: `Last-Event-ID` appears in no test; whether a reconnecting client misses
  events during the gap is untested (the offline tests cover the HTTP sync path, not SSE).
* CSRF on cookie-authenticated form posts (`POST /api/v1/auth/logout` is a plain form post,
  DEC-326): no test sends a cross-origin `Origin`/`Sec-Fetch-Site` header to a state-changing
  route. The cookie is `SameSite=Lax` (`routes.rs:161`), which is the defence; a test should
  pin it (the attribute string is easy to lose in a refactor).
* `crates/kinds` has no unit tests and `testkit` none (acceptable: testkit is test code).
* The legacy importer's full-export test is `#[ignore]` and depends on a file outside the repo
  (`importers/tests/legacy.rs:460`); the committed fixture path covers the format.
* Down-migrations or a "migrate from the previous release's schema" test do not exist; every
  test starts from an empty database.

== 5. MemStore vs PgStore parity

The port surface is 22 traits in `crates/app/src/ports.rs` plus the adapter-owned stores.
`testkit::MemStore` implements the 10 repository traits (`crates/testkit/src/store.rs`), and
`PgStore` the same 10 plus `ChangeFeed` (`crates/store-pg/src/lib.rs:109`). Every other port
has a Mem and a Pg implementation in its own crate (auth, notify, assets, importers,
integrations, catalog-core cache and stats, jobs has Pg only with the testkit `MemJobs` fake).

What is covered on both sides:

* Phase-1 library query/counts/tags, undo/activity/stats, up-next columns, keyset paging,
  remove-with-history and bulk undo, stats totals/series/windows, episodes, goals and game
  groups, catalog audience, retention (`store-pg/tests/phase1.rs`, `use_cases.rs`,
  `windows.rs`, `episodes.rs`, `goals_groups.rs`, `catalog_audience.rs`, `retention.rs`).

Parity gaps (behaviour tested on MemStore in `crates/app/tests` with no Postgres twin):

* `ChangeFeed` is Postgres-only: no in-memory implementation exists, so every use-case that
  reads the change feed (offline sync, `api/tests/sync.rs`) runs on MemStore without it or
  through a fake; the Postgres path is covered only by `apps/server/tests/offline.rs` (3) and
  `store-pg/tests/outbox.rs` (2).
* People and sessions: `app/tests/people.rs` (9 tests: merge suggestions, merge undo, twin
  tie, cooperative win, most-played tie-break, link requests, suggestions never pairing
  linked accounts) has one Postgres counterpart, `use_cases.rs::companion_lifecycle_without_rewriting_history`.
  Statistics that follow merges (`stats_follow_merges_and_include_sessions_a_linked_person_shared`)
  are SQL-heavy and have no Postgres test.
* Activity feed: `app/tests/activity.rs`, `activity_person.rs`, `activity_pairing.rs` (7
  tests, person filter through merges, paging, undone events skipped) versus
  `store-pg/tests/windows.rs:253` `the_person_feed_matches_the_full_read_on_postgres` only.
* Lists: `app/tests/lists.rs` visibility transitions and link revocation (5 tests) versus
  `use_cases.rs::lists_keep_order` only; the share-token behaviour on Postgres is covered
  indirectly by `apps/server/tests/sharing.rs`.
* Companions/catalog: `app/tests/companions.rs` (8) and `catalog_audience.rs` (8) versus 2
  Postgres tests each.
* Undo: `app/tests/undo.rs` and `bulk_undo.rs` (9) versus the undo parts of `phase1.rs:224,843`.
* Next step: `app/tests/next_step.rs` (4) has no Postgres test; it reads episodes/pages
  through `ProgressRepo` and `CatalogRepo`, whose SQL ordering decides the answer.
* Notifications fan-out (`app/tests/notifications.rs`, 3) runs on `RecordingNotifications`;
  the real engine on `PgNotifyStore` is tested in `notify/tests/pg.rs` (2) and through the
  server.

Mem-side simplifications worth a comment in `testkit/src/store.rs`: `purge_provider_data`
(`:375-390`) blanks fields in place while Postgres runs a statement with its own ordering;
`provider_data_before` (`:349-372`) sorts by `(at, id)` which the SQL must match for the
retention tests to be comparable (they are compared in `store-pg/tests/retention.rs`, good).

Concrete suggestion: a `store-pg/tests/parity.rs` that reuses the `app/tests` scenario
functions through a trait object (`common::env()` returns a `PgEnv` whose `app` has the same
type), so each `app` test becomes a Postgres test with one line. The `phase1.rs` file already
does this by hand for stats; generalising it costs little.

== 6. Slow tests and CI time sinks

Nothing here is measured (nothing was run); the estimates come from the code.

* 25 server integration binaries (`apps/server/tests/*.rs`), each linking the full `server`
  crate with Leptos SSR. Link time dominates `cargo test -p server` on the 4-vCPU VM (HANDOFF
  8: one `rustc` on `server` uses about 2.9 GB). Merging them into 4-6 binaries by theme
  (auth, admin, pages, sync, jobs) removes most links; the `support/mod.rs` sharing already
  points that way.
* One fresh database per test (`testing.rs:46-56`: `CREATE DATABASE` plus all migrations). With
  about 480 gated tests that is 480 database creations per `cargo test` and the databases are
  not dropped (HANDOFF 8 tells the owner to drop `test_*` by hand; `xtask` drops only `e2e_*`
  before an e2e run, `services.rs:300`). Options: a template database (`CREATE DATABASE ...
  TEMPLATE test_template`, migrations once per run) or `DROP DATABASE` in a guard.
* Real-time sleeps in the Rust suite: `latency.rs` 10 s, `governed.rs` about 2.2 s,
  `jobs/queue.rs` about 1.6 s, `use_cases.rs` 1.6 s, polls up to 10 s in four server tests,
  `ssr_routes.rs` a CPU-saturating loop. Roughly 20-40 s of pure waiting plus the CPU test.
* Playwright: `global.setup.ts` runs the wizard with 16 axe passes and 16 reloads before any
  spec (240 s budget) on every run. Then 84 axe passes across the specs, 13 of them in
  `[1280, 360] x [light, dark]` loops. Axe on a hydrated Leptos page costs seconds each.
  The accessibility loops are the suite's main cost; they could run in a nightly project and
  the default run keep one width and one theme.
* 25 tests with `test.setTimeout` of 120-240 s; the budget is sized for the shared job lane
  (3.2), so the suite's worst case is minutes of waiting on a loaded runner.
* `wasm-test` job installs Chromium and builds `ui` for wasm separately from `e2e`, which
  also installs Chromium and builds the site; two Chromium installs and two wasm builds per
  pipeline.
* GitLab `test` job: `cargo test --workspace` without `--no-fail-fast`; a failure in one
  binary stops the run and hides later failures (DEC-438 reports exactly this: `usage.rs`
  was never run because `offline` failed first).

== 7. Prioritised plan

1. *Make flakes visible (half a day).* `playwright.config.ts`: keep `retries: 1` in CI but
   add a JSON reporter and let `cargo xtask e2e-scan` fail or print a prominent line when any
   test was `flaky`. Add `--no-fail-fast` to the CI `cargo test` line.
2. *Isolate instance-mutating specs (one day).* Third Playwright project `instance`
   (serial, after `chromium`) for the hosting switch, the time zone test, the source switch,
   the two re-auth gate tests and the instance-default subscription; per-test admin accounts
   instead of `ADMIN`/`REAUTH_ADMIN` where a test changes the account (3.1).
3. *Mutation smoke of the never-red tests (one day).* For each test in 4.3, break the code
   path once, confirm red, revert. Record the result in one DEC entry so the list closes.
4. *Negative assertions after sleeps (two hours).* `member_pin.rs:213`, `signin.spec.ts:173`,
   `governed.rs:122,148` as described in 3.3/3.4.
5. *REST coverage for the untested routes (one day).* One test each in `api/tests/rest.rs`
   (in-memory, cheap) for 4.1, with the ownership refusals (user B on A's goal/group/job).
6. *Parity harness (one to two days).* `store-pg/tests/parity.rs` running the `app/tests`
   scenarios for people, activity, lists, undo and next step on Postgres (5).
7. *Test-database template and binary merge (one day).* Template database in
   `testing.rs`, `DROP` of `test_*` at the start of `cargo xtask ci`, and merging the 25
   server binaries into about 5 (6).
8. *Job lane for tests (half a day, product-visible).* Per-user import lane or a test-only
   worker concurrency, then lower the 120-240 s timeouts to 60 s so slowness is reported
   instead of absorbed (3.2).
9. *Nightly job (half a day).* Axe loops in both themes and widths, `--repeat-each 3` of
   `library.spec.ts`, `keyboard.spec.ts`, `home.spec.ts`, the `#[ignore]` load test, and the
   DEC-457 cut list as it gets built.
10. *Role split, fs backend, SSE reconnect, SameSite pin (one day).* One server test each
    for `--role web` + `--role worker`, the `fs` blob store, `Last-Event-ID` resume, and the
    cookie attributes (4.4).

STATUS: COMPLETE (2026-11-09)
