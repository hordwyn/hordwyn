# sec0 progress (body limit, webhook error text, the notifications page race)

Resume notes for a cut-off run. Brief: docs/handoff/fable-briefs/PKG-implement.md.
Scope: docs/fable/round2/v1-plan.md section 1 (SEC-2, SEC-9 of the security audit on
`wip/fable-reviews`), plus the flaky `notifications.spec.ts:113` from the v1-ops gate.
Nothing is committed.
Build commands: `nix develop -c bash -c 'cd src/rust; CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 cargo test ...'`.
Service env: `TEST_DATABASE_URL=postgres://app@127.0.0.1:55432/postgres REQUIRE_SERVICES=1
TEST_MEILI_URL=http://127.0.0.1:55434 TEST_MEILI_KEY=dev-only-master-key-0123456789`.
DEC entries written: DEC-647 (SEC-2), DEC-648 (SEC-9), DEC-649 (the notifications race).

| # | Item | State | Tests (red first) | Files |
|---|---|---|---|---|
| 1 | SEC-2: body limit on `/_fn/*`, pages, fallback | done, tests green | `apps/server/tests/it/body_limit.rs` (4 tests; red: 500 "Deserialization|length limit exceeded" for a server function with or without a declared length, 303 for a page; the import guard was green before and after) | `apps/server/src/lib.rs` (`body_limit`), `apps/server/Cargo.toml` (`futures` from dev to normal dependency), `crates/api/src/lib.rs` (exports the two limits) |
| 2 | SEC-9: generic 500 for webhook storage errors | done | `crates/api/tests/rest.rs::a_storage_failure_behind_the_webhook_answers_with_the_generic_text` (red: the body held the SQL text), `integrations::tests::the_card_keeps_no_storage_error_text` (red: the card kept "integration storage: ...") | `crates/api/src/hooks.rs`, `crates/api/src/error.rs` (request id in the log line), `crates/integrations/src/lib.rs` (`card_error`) |
| 3 | Flaky notifications e2e: the app race | done: notify test red then green; web unit test green; both browser tests red on the build without the fix (5 s at "Last delivery failed") and green with it; `notifications.spec.ts --repeat-each=10` on the final build: 51 passed | `crates/notify/tests/engine.rs::a_written_outcome_is_announced_to_the_channels_owner`; `web` `notifications::tests::a_finished_delivery_is_shown_in_place_and_other_changes_build_the_page_anew` (written with the function, not seen red); `src/web/tests/notifications.spec.ts` two new tests at the end | `crates/notify/src/{engine,deliver,lib}.rs`, `crates/web/src/{events,notifications,notify_channels}.rs`, `src/web/tests/notifications.spec.ts` |
| 4 | Gates | done: see "Final state" | | |

## Findings so far

- SEC-2: `server_fn` 0.8.13 reads the body with `into_limited_body()`
  (`server_fn-0.8.13/src/request/axum.rs`), which applies axum's `DefaultBodyLimit` and,
  without one, axum's built-in 2 MiB. So the audit's "any size" does not hold for this
  version; what the red run shows decides (status and bytes read).
- SEC-9, more than the plan names: the integration card (Settings > Integrations) and the
  `integration.call` event kept `IntegrationError::to_string()` for a 500, that is the
  storage text, for the token's owner. `card_error` now says "internal error" for `Port` and
  `App(Repo)` and keeps every other text.
- Race of item 3: the page reads once and refetches only for `notification.new` (the inbox
  row), which is published before the deliveries are even queued. The outcome of a delivery
  was announced to nobody. The spec reloads until the log shows one "Failed" row; with two
  channels the page can be read between the two deliveries, and the second card then keeps
  "Never used" for good. Second gap: what is published between the server-side read of a
  page load and the opening of its event stream reaches nobody.
  Fix as built: `notify::DELIVERY_TOPIC` (`notification.delivery`) published by the
  `Deliverer` after every written outcome (a digest once). The page reads again, paced, for
  that topic and whenever the event stream has opened since its latest read began (`events::StreamOpens`,
  `events::refresh_on_topics`), and shows the result IN PLACE (`NotifyState::live`: health
  per channel, log rows keyed by id + state) instead of building the page anew, because a
  rebuild drops a half-filled form and an unfolded error; only when the later read differs
  in something else (inbox, channels, rules) the page is refetched as before
  (`notifications::later`; the deliveries show in place at once in that case too). `notification.new` still refetches the whole page as before.
- The first spec looked at "the first failed row" twice; with a log that follows deliveries
  the second failed delivery sorts above the first, so that row is now named by
  `data-delivery` (new attribute on the log rows). No timeout, retry or wait was added.

## Log

- api + integrations tests red then green (see table).
- server `body_limit::` 4/4 green; notify `engine` 9/9 green; `cargo test -p web --lib -- notifications:: events:: notify_` 11/11 green.
- Browser, build without the page fix: the two new tests red (expected), setup green.
- First design of the page fix refetched nothing in place when the later read also had a
  new inbox row, and the browser test held the page's read for good: a resource runs its
  reads one after the other (`reactive_graph-0.2.16`, `arc_async_derived.rs`), so the held
  read blocked the next one. Now the deliveries are always shown in place and the held read
  is released at the end of the test.
- `notifications.spec.ts --repeat-each=10` on the fixed build: first run 50 passed, 1 failed:
  "a channel is edited in place ..." waited for the "ntfy" radio while the page had opened
  on the inbox (the import that this test starts before loading the page had finished, so
  "Import finished" was unread). A race of the test, older than this package: the test now
  opens the Channels tab itself. Second run: 51 passed (3.6 min).
- fmt --check: clean. clippy workspace (native features, pedantic, -D warnings): clean.
  clippy wasm (web, ui): clean.

- Whole Playwright run on that build: 150 passed, 1 failed, sso did not run. The failure was
  the new test "a delivery that fails while the page is open ...": it kept back "the first
  read", and under load that was the read the page makes when its stream opens, not the one
  for the inbox row, so the page was built anew and the half-typed name was gone. Two
  changes: the test opens a fresh tab and waits for the read on opening before it keeps a
  read back; and the rule for that read became exact (`read_at`: the page reads again when
  the stream has opened since its latest read began, instead of "when the count moved after
  the effect first ran").

## Final state (all on the final sources)

- `cargo fmt --all --check`: clean. clippy workspace (native features, pedantic,
  `-D warnings`) and clippy wasm (web, ui): clean (run again after the last change).
- `cargo test -p integrations -p notify -p api --features integrations/all,notify/all` with
  the service env: all green (api `rest` 23, notify `engine` 9, integrations lib 5).
- `cargo test -p web --lib`: 215 passed (run again after the last change).
- `cargo test -p server --features gallery,all-connectors --test it`: 112 passed, 2 ignored
  (run before the last change to `web/src/events.rs` and `notifications.rs`; not run again).
- `cargo leptos build`, then `notifications.spec.ts --project=chromium --repeat-each=10`:
  51 passed; then the whole Playwright run (setup, chromium, sso): 162 passed, 8.4 min.
- Red runs seen: api webhook test; integrations card test; server `body_limit::` (3 of 4;
  the import guard is green before and after by design); notify announcement test; both
  new browser tests on a build without the page fix. The browser test "a delivery that
  fails while the page is open ..." was seen red in its first form (the first read kept
  back for good, no wait for the read on opening); its final form waits for a read that a
  build without the fix never makes, so there it would fail at that wait instead.
  The web unit test of `later` was written with the function.
- Test databases dropped, executables in `target/debug/deps` deleted, 7.2 GB free.

## Out of scope, noticed

- Home, Library, History, People and the lists (`events::refresh_on_updates`) have the same
  gap as the notifications page had: an `entity.updated` published between the server-side
  read of a page load and the opening of the event stream is not seen. `StreamOpens` and
  `refresh_on_topics` could close it at the cost of one more read per page load.
- Settings > Notifications still builds the whole page anew for `notification.new` (and for
  its own saves): a half-filled "Add a channel" form is lost when an inbox row arrives.
- No log line carried the request id before; only the three generic 500 answers of
  `api::error` do now. A request span for all log lines would be the general answer.
- The Playwright runs of this package did not use the `E2E_SERVER_LOG` panic scan of
  `cargo xtask e2e`.

## Review fixes

Review: docs/reviews/2026-11-12-sec0-review.md (SEC0-1..9). Nothing is committed. Resume
notes: one row per finding, written as the work goes. Browser runs use a database of their
own (`createdb sec0r_<n>` on 55432) and the ports 3110..3114 as the packages before.

| Finding | State | Tests (red first) | Files |
|---|---|---|---|
| SEC0-1 | code done, red seen, green run pending | `api` `rest::a_storage_failure_behind_the_notifications_routes_answers_with_the_generic_text` with a `BrokenNotifyStore` (red: `GET /api/v1/notifications/inbox` answered `"message":"notifications db: error returned from database: relation ..."`) | `crates/api/src/notifications.rs` (`port` removed, the 22 call sites use `ApiError::from`, which logs with the request id), `crates/api/tests/rest.rs` |
| SEC0-2 | code written, red seen on the old build (line "Automation: the last delivery failed"), not built yet | `notifications.spec.ts` "a delivery that fails while the page is open ..." (new assertion on `getByRole("status")`); `web` `notifications::tests::a_later_read_says_whose_health_changed` | `crates/web/src/notifications.rs` (`health_news`, `NotifyState::news`, the `role="status"` paragraph), `locales/{en,de}.ftl` (`notify-live-failing`, `notify-live-working`) |
| SEC0-3 | code written, red seen on the old build (the Add form's name was "" after the inbox row), not built yet | `notifications.spec.ts` "an open form keeps its text and the focus when an inbox row arrives and after a save on the page" | `crates/web/src/{notifications,notify_channels,notify_rules_view}.rs`: the page is built once, `NotifyState::page` holds the latest read, every part reads it (keyed lists); `state.refresh()` replaces every `data.refetch()` |
| SEC0-4 | by the same change: an answer is kept whether or not the page is built (`NotifyState::show`) | none of its own | `notifications.rs` |
| SEC0-5 | code written: log rows keyed by id, state and error read from a `Memo` per row | none of its own (the first spec unfolds an error) | `notifications.rs` (`Log`, `LogRow`) |
| SEC0-6 | bare 400 now `ErrorDto` (`bad_request`); the read timeout: to be deferred by DEC | `server` `it` `body_limit::a_body_that_breaks_off_is_answered_as_an_error_document` (written with the change, not seen red) | `apps/server/src/lib.rs`, `apps/server/tests/it/body_limit.rs` |
| SEC0-7 | open: red run with the delivery topic removed | | |
| SEC0-8 | open: a new DEC that corrects DEC-647/648 | | |
| SEC0-9 | code written | `web` `events::tests::a_lagging_stream_concerns_every_page` | `crates/web/src/events.rs` (`LAGGED`, `concerns`, `refresh_when_lagged`), `item.rs`, `imports.rs`, `integrations.rs`; `crates/api/src/routes.rs` (doc comment of the SSE route) |

Audit for SEC0-1 (other places that could forward store text): every `INTERNAL_SERVER_ERROR`
of `crates/api/src` and `apps/server/src` and every `map_err` with `to_string()` / a format
of the error was read. All others answer the fixed text "internal error" (or no body) and
log the detail: `error.rs` (`AppError::Repo`, auth, port), `admin/mod.rs` (`port`, `pe`,
`job_err`, storage report, eviction), `ops.rs` (`import_error`), `sso_routes.rs`
(back-channel logout), `apps/server/src/lib.rs` (assets). `admin::reload_keys` and
`admin::send_mail` return a `String` that goes to the log only (SIGHUP; the sign-in link
answers a fixed `mail_failed`). Channel tests and the admin's mail test show what the
channel / the SMTP server answered, by design. Server functions pass the error code only
(`web::api::server::code`). No second place found.

Log of the review fixes (states above are as of the first writing; this log is newer):
- SEC0-1: `cargo test -p api --features integrations/all,notify/all --test rest`: 24 passed.
- web: `cargo test -p web --lib -- notifications:: events:: notify_`: 13 passed.
- fmt applied; clippy wasm (web, ui) and clippy workspace (native features, pedantic,
  `-D warnings`): clean.
- SEC0-7 red run: with `notification.delivery` taken out of the topics of the page
  (`refresh_on_topics(&["notification.new"], ..)`), `cargo leptos build`, then
  `notifications.spec.ts --project=chromium`: 6 passed, 1 failed, the failure being "a
  delivery that fails while the page is open ..." at the claim itself
  (`expect(automation.getByText(/^Last delivery failed/)).toBeVisible()`, the line after
  `failedDelivery`), not at the wait for the read on opening. Topic put back.
- SEC0-2 / SEC0-3 red runs on the build before the fixes (binary and site of 19:52): the
  status assertion failed (no element), and the new form test failed at
  `expect(name).toHaveValue("Half typed")` (received "") after the inbox row.
- DEC-650 (SEC0-1, SEC0-8), DEC-651 (SEC0-3, 4, 5), DEC-652 (SEC0-2), DEC-653 (SEC0-6,
  deferral to sec2), DEC-654 (SEC0-9) written.
- Still to run on the final build: `cargo leptos build`, `notifications.spec.ts
  --repeat-each=10`, whole chromium + sso, server `it`, final fmt/clippy.

### Final state of the review fixes (all on the final sources)

All nine findings are closed: SEC0-1, 2, 3, 4, 5, 7, 8, 9 applied, SEC0-6 applied for the
400 answer and deferred to sec2 for the read timeout and the narrower collecting (DEC-653).

- `cargo fmt --all --check`: clean. clippy workspace (native) and clippy wasm: clean (run
  before the last two edits, which were the topic list put back and one doc comment).
- `cargo test -p api --features integrations/all,notify/all --test rest`: 24 passed.
- `cargo test -p web --lib -- notifications:: events:: notify_`: 13 passed.
- `cargo leptos build` (final), then `notifications.spec.ts --project=chromium
  --repeat-each=10`: 61 passed (4.4 min).
- Whole Playwright run (setup, chromium, sso) with `E2E_SERVER_LOG`: 163 passed (8.4 min);
  no `panicked at` line in the server log (685 lines).
- `cargo test -p server --features gallery,all-connectors --test it`: 113 passed, 2 ignored
  (includes the new `body_limit::a_body_that_breaks_off_is_answered_as_an_error_document`).
- Not run: `cargo test -p web --lib` as a whole, notify and integrations tests (those crates
  were not touched by the review fixes), `cargo xtask ci` / `e2e`.
- Test databases dropped, executables in `target/debug/deps` deleted. Disk: 4.5 GB free
  after the clean-up (2.5 GB right after the `it` build), below the 5 GB mark of the brief.

Out of scope, noticed during the review fixes:
- The rules matrix is still built anew when a rule or a channel changes, so the switch that
  was pressed loses the focus (as before the package).
- After "Save" or "Cancel" in a channel's edit form the focus is not put back on "Edit".
- The search palette does not react to `stream.lagged` (DEC-654).
- sec2 has to take the body read timeout and the narrower collecting of DEC-653; its brief
  does not name them yet.
