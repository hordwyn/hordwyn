# sec2 progress (lane A2, worktree kithkeep-lane-sec2, branch lane/sec2)

Scope: next-packages.md "sec2", v1-plan.md addenda (sec2 notes), security-audit.md SEC-4 and
SEC-5, DEC-653 (read timeout, collect only for `/_fn/`). Decisions: sec2-decisions.md
(LA-1..11). No migrations (counters are per process, HANDOFF 4.8; LA-4). Nothing committed.

Commands: `nix develop -c bash -c 'cd src/rust; CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 cargo ...'`
after `free -g` (wait below 6 GB available).

## Items

| # | Item | State |
|---|------|-------|
| 1 | Sign-in throttling: counters behind `auth::AttemptCounters`, wait per account and address, budget per address, verification cap (`crates/auth/src/throttle.rs`) | done, tests green |
| 2 | Throttle on every check (sign-in, local sign-in, "Is this your account?", re-auth, password change, TOTP enrolment, reset link, emailed link); client address from the request layer (`auth::client`, `api::request_meta`); warning for an untrusted forwarder | done; server `it` green (item 8) |
| 3 | Outbound: redirects re-checked per hop, private-range filter on the governed client, suffix host entries, operator allowlist (`catalog-core/src/http.rs`, `resolve.rs`) | done, tests green |
| 4 | Outbound: OIDC endpoints and mail host by hosting mode (DEC-534), categorised errors (`auth/src/oidc.rs`, `notify/src/channels/email.rs`, `api/src/admin/mail.rs`, web texts) | done; api, web, `it` green (item 8) |
| 5 | Request read timeouts (`apps/server/src/serve.rs`, `read_timeout.rs`), body collected only for `/_fn/` | done; `it` green (item 8); red run in the review fixes (SEC2-12) |
| 6 | Config values (`auth.throttle`, `server.*_timeout_secs`, `outbound.allow_private_hosts`, `notify.allow_private_targets` optional), configuration reference, reverse-proxy, operations, notifications, connectors pages, `.env.example` | done (reference regenerated once; again after the last config change) |
| 7 | Web: wait text in seconds, reason sentences for mail and provider tests; Playwright `shell.spec.ts` (wait text), `signin.spec.ts` (break-glass wait) | done; web tests and Playwright green (item 8) |
| 8 | Final runs: clippy, it modules, page modules, cargo leptos build, Playwright chromium + sso | done (see "Item 8 runs") |

## Tests added and seen red

- `crates/auth/tests/throttle.rs` (11 new): first run against the old lock, 9 of 12 failed
  as expected. `the_admin_name_cannot_be_locked_by_guesses_from_another_address` passed in
  that run by accident (the test advanced the clock past the old 15 minute lock); it was
  tightened afterwards (advance at most 4 s per refusal) and not run red again.
- `crates/auth/src/throttle.rs` unit tests (4): written with the module, not seen red.
- `crates/catalog-core/tests/governed.rs` (8 new): all 8 red against the client without
  confinement, green after.
- `crates/catalog-core/src/http.rs` `host_entries_cover_...`: written with the code.
- `crates/auth/tests/oidc.rs` (3 new): 2 red (hosted mode), the home-mode one green from
  the start (it states what must keep working).
- `crates/notify/src/channels/email.rs` `a_mail_server_on_a_private_address_...`: red, green.
- `crates/config` `throttle_timeouts_and_outbound_have_safe_defaults_and_are_checked`:
  written with the settings, not seen red.
- `apps/server/tests/it/instance.rs` (3 new), `read_timeout.rs` (5 new): red run done in the review fixes (SEC2-12 below).

## Runs so far (only what ran)

- `cargo test -p auth --lib --test throttle`: 42 + 12 passed.
- `cargo test -p auth --test oidc`: 8 passed.
- `cargo test -p catalog-core --lib --test governed`: 12 + 19 passed.
- `cargo test -p config` (with `UPDATE_DOCS=1`): 18 passed.
- `cargo test -p notify --features all --lib email`: 2 passed.
- `cargo clippy -p catalog-core -p auth -p config --all-targets` pedantic: clean (before the
  OIDC change); `-p notify --features email`: clean.

## Noticed, not built

- `/auth/username-available` keeps its instance-wide budget of 30 per minute (SEC-5 d: one
  client can use it up for everyone); `/shared/lists/{token}` has no limit per address.
- Sign-up invite tokens are not counted by the throttle.
- No Postgres store for the counters (LA-4).
- HTTP/2 connections have no head timeout (LA-11).

## Item 8 runs (2026-11-13, resumed by a second agent)

- `TEMP-RED` lines: none left in the source at resume; whether the red run of the server
  tests happened is not recorded (the first agent stopped before logging it).
- `cargo fmt --all --check`: clean.
- clippy pedantic native (workspace, all targets, CI features): clean.
- clippy pedantic wasm (`-p web -p ui --all-targets`, wasm32, `web/hydrate,web/gallery`): clean.
- `cargo test -p auth -p catalog-core -p config -p xtask`: all green (auth 42 lib + throttle 12
  + oidc 8, catalog-core governed 19, config 18, ...).
- `cargo test -p notify --features all -p api -p web`: all green (web lib 217, api incl.
  `stored_result_tests`).
- Whole server `it` binary (`--features gallery,all-connectors`, services env): 128 passed,
  0 failed, 2 ignored (load test, live provider) in 45.6 s; load average 6.3 afterwards.
  170 `test_` databases appeared during the run (list in the scratchpad `sec2-la/dbs-new-it-run.txt`);
  not dropped, because another lane's `it` run started in the same window.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Playwright `--project=chromium --project=sso` (server `la-server`, ports 3600-3604,
  database `la_e2e`), 11:28-11:48, load average 7 at the start and 14 at the end (other lanes
  building): 155 passed, 3 failed, 12 (`sso`) not run because chromium failed. No panic in
  the server log. The new sec2 specs (shell wait text, admin mail reason) passed. Failures,
  all waits of 5 s or the test timeout under load, none in code sec2 touched:
  `item.spec.ts:745` (pages: the "Enter a page number from 1 on." text not seen within 5 s),
  `keyboard.spec.ts:34` (the shortcuts dialog not open within 5 s after `?`),
  `people.spec.ts:91` (the "Delete" menu item not found; the people menu race of the
  hydration diagnosis on main).
- Rerun of those three alone (`--project=chromium`, fresh database), load average 13-16:
  5 passed (setup included).
- `sso` alone (fresh database, `--project=setup`, then `--project=sso --no-deps`), load
  average 14-15: 12 passed, including the new break-glass wait spec. No panic in the log.

Item 8: done. Nothing needed a fix.

## Waiting (resolved)

The pending red run of the server tests was done by the review fixes (section "Review
fixes", SEC2-12); nothing is waiting.

## Incident 2026-11-13 about 05:50 (to report to the orchestrator)

The scratchpad directory is shared by all lanes. My wrapper `scratchpad/dev.sh` was
overwritten at 05:50:01 by another lane's script of the same name (it changes into
`kithkeep-lane-books`). One command of mine went through it afterwards and therefore ran in
the books lane's worktree: `cargo fmt --all`, `cargo clippy --workspace --all-targets
--features ...` and `cargo test -p xtask --test locales --test whitelabel` (my run
`bzudepl9u`). Effect there: `cargo fmt --all` may have reformatted files of that lane (I
cannot tell which), and a second cargo ran beside theirs. Nothing else of mine touched that
worktree; my own worktree shows only my files. My scripts now live in
`scratchpad/sec2-la/` and name the sec2 worktree by absolute path.

## Review fixes (docs/reviews/2026-11-13-sec2-review.md, lane A2 second agent)

Scripts in the scratchpad `lane-a2-sec2fix/` (worktree named by absolute path).

| Finding | State | Tests (red first) | Files | Decision |
|---------|-------|-------------------|-------|----------|
| SEC2-1 | done | `auth` lib `wrong_codes_are_bounded_per_account_whatever_the_address` red (17 of 40 wrong codes checked... failed at the count assert), green after; `wrong_passwords_do_not_touch_the_code_count` guard, green from the start | `crates/auth/src/throttle.rs`, `lib.rs`, `src/tests.rs` | LA-12 |
| SEC2-5 | done | `throttle::tests::over_long_names_make_bounded_keys` red (keys of 6 MB), green | `throttle.rs` | LA-13 |
| SEC2-6 | done | `throttle::tests::a_full_map_keeps_address_budgets_before_pairs` red, green | `throttle.rs` | LA-13 |
| SEC2-14 | done | `throttle::tests::free_failures_at_the_maximum_do_not_overflow` red (overflow panic), green | `throttle.rs`, decisions LA-1 sentence, LA-14 | LA-14 |

Run: `cargo test -p auth --lib --test throttle --test oidc`: 47 + 12 + 8 passed.
| SEC2-2 | done | `it` `read_timeout::a_request_head_that_stalls_closes_the_connection` extended (nothing sent; part of and the whole HTTP/2 preface; bounded wait of 10 s inside the test via `closed_after`): red with the auto builder (the "nothing" case open after 10 s), red again with `auto::Builder::http1_only()` (hyper-util ignores it for `serve_connection_with_upgrades`), green with hyper's `http1::Builder` | `apps/server/src/serve.rs`, `Cargo.toml` (hyper/hyper-util without `http2`, `server-auto`), `tests/it/read_timeout.rs` | LA-15 |
| SEC2-7 | done | `config` `import_upload_time_and_private_host_entries` red (no field), green; `it` `the_import_upload_only_has_the_idle_limit` extended with a 12 s trickle: red (not 408), green (408 after the 5 s of the test setting); `an_import_upload_without_a_session_reads_nothing`: guard, green from the start (`Authed` is checked before the body extractor) | `crates/config/src/lib.rs`, `apps/server/src/read_timeout.rs`, `tests/it/read_timeout.rs` | LA-16 |

Run: `it -- read_timeout::` 6 passed (12.7 s).
| SEC2-8 | done | `domain` `net::tests::special_ranges_and_embedded_ipv4` (table) red at `0.1.2.3`, green | `crates/domain/src/net.rs` | LA-17 |
| SEC2-3 | done | `auth --test oidc` `private_hosts_on_the_operator_list_are_called_in_hosted_mode` red (`jwks: a private address`), green; `notify --test channels` `a_private_target_on_the_operator_list_is_called` red, green; `notify` lib `a_mail_server_on_a_private_address_is_refused_unless_allowed` extended (listed mail host by address and by name, another port not listed) red, green; `catalog-core` lib `names_of_private_addresses_are_refused_at_connect_time` extended (listed name, other port) red, green; `domain` `private_host_entries` and `server` lib `outbound::tests::one_list_for_every_outgoing_request`: written with the code (new types), not seen red | `crates/domain/src/net.rs` (`PrivateHosts`, `PrivateAllow`), `crates/auth/src/oidc.rs`, `crates/notify/src/channels/{email,http}.rs`, `crates/catalog-core/src/{http,plain}.rs`, `apps/server/src/outbound.rs` (new), `apps/server/src/lib.rs`, `crates/config/src/lib.rs` (docs of the setting), docs | LA-18 |
| SEC2-9 | done | `catalog-core --test no_proxy` (own binary; proxy variables set to a counting listener) red (plain client went to the proxy), green | `crates/catalog-core/src/{http,plain}.rs`, `tests/no_proxy.rs` | LA-19 |
| SEC2-10 | done | `notify` lib `each_checked_address_is_tried_until_a_server_answers` (the helper `try_in_order`): written with the helper, green; the wiring into `Email::transports`/`deliver` needs a name with a public unreachable and a public reachable address, not testable here | `crates/notify/src/channels/email.rs` | LA-20 |
| SEC2-11 | done | `config` `import_upload_time_and_private_host_entries` (lower case, `*.`, leading dot refused) red, green; `catalog-core` lib `host_entries_cover_...` switched to `*.archive.org` red, green | `crates/config/src/lib.rs`, `crates/catalog-core/src/http.rs`, `crates/domain/src/net.rs`, `docs/connectors/openlibrary.md`, connectors page | LA-21 |
| SEC2-13 | done | refactor, covered by the `it` binary; `Server::build` 218 -> 90 lines (helpers `connect`, `core`, `storage_budget`, `sign_in`, `api_state`, `first_checks`, `request_guards`; outbound wiring in the new module `outbound.rs`) | `apps/server/src/lib.rs`, `apps/server/src/outbound.rs` | - |
| SEC2-4 | done | `api` lib `request_meta::tests::an_unlisted_proxy_on_a_private_address_is_recorded` and `admin::probes::tests::the_reverse_proxy_row_warns_while_an_unlisted_proxy_is_seen` red (missing), green; `it` row counts 8 -> 9 (`admin_fns.rs:100`, `admin.rs:244`, `:1000`); Playwright `admin.spec.ts` count 9 and the row's "fine" text | `crates/api/src/request_meta.rs`, `crates/api/src/admin/{probes,mod}.rs`, `crates/web/src/admin/overview.rs`, `web/locales/{en,de}.ftl`, install and reverse-proxy pages | LA-22 |

Runs: `cargo test -p auth --test oidc` 9 passed; `-p notify --features all --lib --test channels` 20 + 8 passed; `-p catalog-core --lib --test no_proxy --test governed` 12 + 1 + 19 passed; `-p domain --lib net::` 3 passed; `-p server --lib` 22 passed; `-p api --lib` 15 passed.
| SEC2-12 | done | Red run of the server tests with four lines marked `TEMP-RED` (read-timeout layer taken out of `Server::guarded`; bodies to every path collected; the request layer handing no client address to `auth`; the mail rule always allowing): `it -- read_timeout:: instance::a_mail instance::the_private instance::sign_in_is` 7 failed as expected (`a_mail_server_on_a_private_address_follows_the_hosting_mode` at instance.rs:1266, `the_private_target_switch_overrides_the_hosting_mode` at :1317, `sign_in_is_slowed_per_account_and_client_address` at :1386; `a_body_that_stalls_is_given_up`, `a_body_that_trickles_...`, `a_body_to_a_page_is_not_read`, `the_import_upload_only_has_the_idle_limit` each stopped by the harness's own bound, `support::send`'s 90 s `REQUEST_LIMIT`, instead of hanging), 3 passed (the head test, whose red run is SEC2-2; the unauthenticated upload guard). `TEMP-RED` lines removed afterwards (grep: none left). The head test got its own 10 s bound (`closed_after`). The stale "Waiting" section is marked resolved | `docs/fable/round2/sec2-progress.md`, `tests/it/read_timeout.rs` | - |
| SEC2-14 | done | see SEC2-1 row (`free_failures_at_the_maximum_do_not_overflow` red with an overflow panic, green); the parallel-burst point and the `oidc_blocked_target` category recorded in LA-14 | `throttle.rs` | LA-14 |

### Review fixes: final runs (2026-11-13)

- `cargo fmt --all --check`: clean.
- clippy pedantic native (`--workspace --all-targets`, CI features): first run 4 findings in
  `domain/src/net.rs` (short names, backticks), fixed; second run clean.
- clippy pedantic wasm (`-p web -p ui --all-targets`, wasm32, `web/hydrate,web/gallery`): clean.
- `UPDATE_DOCS=1 cargo test -p config --lib` (configuration reference regenerated), then
  `cargo test -p config --lib`: 19 passed.
- `cargo test -p auth -p catalog-core -p config -p domain -p xtask` and
  `-p notify --features all -p api -p web`: all green (auth lib 47, throttle 12, oidc 9;
  catalog-core lib 12, governed 19, no_proxy 1; notify lib 20, channels 8; web lib 217; ...).
- Whole server `it` binary (`--features gallery,all-connectors`, services env), load average
  about 7-9 (two other lanes' `it` runs at the same time): 128 passed, 1 failed, 2 ignored:
  `admin::overview_reads_the_probe_rows_and_never_probes` still expected eight rows from
  `run_probes` (two asserts at admin.rs:979 and :984 I had missed); fixed to nine, then
  `it -- admin::` 5 passed.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`: ok.
- Test databases: the server `it` runs of this lane created `test_<uuid>` databases, but
  other lanes' `it` runs ran in the same windows, so the new names cannot be told apart;
  none dropped (candidate list in the scratchpad `lane-a2-sec2fix/maybe-dbs.txt`).
- Playwright `--project=chromium --project=sso` (server `la2-server`, ports 3600-3604,
  database `la2_e2e`), load average 9.7 / 12.4 / 13.7 at the end (other lanes building and
  testing): 154 passed, 4 failed, 12 (`sso`) not run because chromium failed. The new
  checks (Overview with nine rows and the "Reverse proxy" row; the shell wait text and the
  break-glass wait of the first agent) passed. Failures, all at the 30 s test timeout:
  `admin.spec.ts:426` (axe run), `gallery.spec.ts:49`, `settings.spec.ts:31`,
  `sharing.spec.ts:55`.
- The four alone (load 9.5-12.9): `admin.spec.ts:426` and
  `gallery.spec.ts:49` passed, `settings.spec.ts:31` and `sharing.spec.ts:55` failed again
  at 30 s. `settings.spec.ts:31` alone (load 8.8): passed. `sharing.spec.ts:55` alone (load
  10.6): failed again, waiting for the click on the "Merge with..." menu item, which was
  found (the people menu race known on main; lanes B and C report the same test at 30 s
  under load: v1-polish LC-18 / POL-5, v1-books progress). Nothing in that path was changed
  by sec2 or these fixes; left to the orchestrator's gate.
- `sso` alone (fresh database, `--project=setup`, then `--project=sso --no-deps`, load 7.4):
  12 passed. No panic in the server log.
