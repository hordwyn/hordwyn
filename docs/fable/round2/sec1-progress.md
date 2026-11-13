# sec1 progress (security headers, CSP, hashed bundle names)

Started 2026-11-12. Brief: docs/handoff/fable-briefs/PKG-implement.md; scope: next-packages.md
"sec1", v1-plan.md addendum, security-audit.md SEC-1 (branch wip/fable-reviews), DEC-536.

## Plan

| # | Item | State |
|---|---|---|
| 1 | Server tests per header (page, REST, server function, asset, refusals), nonce test | done |
| 2 | Middleware `security_headers` (CSP with nonce, nosniff, frame, referrer, permissions, HSTS, cache directives), config keys | done |
| 3 | Shell: nonce on the theme script and the brand style | done |
| 4 | OIDC: end-session origins in `form-action` | done |
| 5 | Playwright fixture that fails on any CSP violation; red with a too strict policy; whole suite green | done (final run: see below) |
| 6 | Hashed bundle names (cargo-leptos `hash-files`), cache directive for them, Dockerfile | done (image build not verifiable here) |
| 7 | Docs: reverse-proxy.adoc, configuration reference, operations; DEC entries | done (DEC-655..660) |
| 8 | Final runs: server `it` modules, web tests, clippy, Playwright `chromium` + `sso` on the final build | open |

## Log

### Items 1 to 4 (headers, policy, nonce, form targets)

- Red: `cargo test -p server --features gallery,all-connectors --test it -- security_headers::`
  0 passed, 6 failed (no header on any answer; unknown config keys).
- Built: `apps/server/src/security_headers.rs` (middleware `layer`, `Headers`, `CspNonce`,
  9 unit tests), layered outermost in `Server::guarded` (so CSRF refusals, 413 and 405 carry
  the headers too); `Server::web_context` hands the request's nonce to Leptos
  (`leptos::nonce::Nonce::from_value`, leptos feature `nonce`), `web::shell` puts it on the
  theme script and the brand style; `config`: `server.hsts_max_age_secs` (31536000),
  `server.csp_report_only` (false); `auth::oidc::Oidc::end_session_origins` (kept current in
  `allow`, which no longer returns early without a host allowlist) feeds `form-action`.
- Green: `--lib --test it -- security_headers:: rp_initiated ops:: brand:: shell:: body_limit::`
  9 unit + 23 integration passed (after two tests of mine were corrected: a signed-out page
  leads to `/setup` until the first account exists).
- The `form-action` assertion in `oidc::rp_initiated_logout_is_local_first` was written with
  the change, not seen red.
- Files: `apps/server/src/{lib.rs,security_headers.rs}`, `apps/server/Cargo.toml`,
  `apps/server/tests/it/{main.rs,security_headers.rs,oidc.rs}`, `crates/config/src/lib.rs`,
  `crates/auth/src/oidc.rs`, `crates/web/src/app.rs`, `config/.env.example`,
  `docs/modules/ROOT/pages/reverse-proxy.adoc`.

### Item 5 (Playwright guard)

- `src/web/tests/fixtures.ts`: `test` with an automatic fixture; collects
  `securitypolicyviolation` events and console messages naming the policy in every context
  of a test (also `browser.newContext()` / `browser.newPage()`), fails the test in teardown.
  All specs and `global.setup.ts` import from `./fixtures`; a setup test fails when a spec
  imports `test` from the Playwright package.
- Red 1 (policy without `'wasm-unsafe-eval'`, without `style-src-attr`, pictures https only):
  the setup test fails with `script-src blocked wasm-eval [enforce] at /pkg/app.js:1672` and
  the page never hydrates. So WebAssembly needs `'wasm-unsafe-eval'` (verified).
- Red 2 (with `'wasm-unsafe-eval'`, still without `style-src-attr 'unsafe-inline'` and with
  pictures over https only): whole `chromium` project, 84 passed, 69 failed, 11 did not run
  (`sso` depends on `chromium`). Exactly two kinds of violation in the whole run:
  `style-src-attr blocked inline` (553 events: the server renders `style` attributes for
  bar widths, the rating, cover placeholders and the brand swatches) and
  `img-src blocked http://127.0.0.1:3011/t/p/...` (the covers of the TMDB stand-in, loaded
  over http by an instance that itself runs on http). Nothing else: no script, connect,
  font, form or frame violation. Both are answered in the real policy (`style-src-attr
  'unsafe-inline'`; `http:` pictures only while the public URL is http).

### Item 6 (hashed bundle names)

- `hash-files = true` in the cargo-leptos metadata; `HashedStylesheet` in the shell in place
  of `<Stylesheet href="/pkg/app.css">` in `App`.
- Build output seen: `target/site/pkg/app.<22 chars>.js|.wasm|.css`, `target/debug/hash.txt`
  (`js:`, `wasm:`, `css:` lines).
- Red: `security_headers::hashed_bundle_files_are_kept_for_good` (a hashed name answered
  `no-cache`). Built: `hashed_bundle_file` and the directive in `pkg_no_cache` (lib.rs);
  `infra::bundle_hashes` / `hash_file_path` and their use in `main.rs` (hashed names
  whenever the hash file is there; refuse to start when asked for and missing);
  Dockerfile (`hash.txt` next to the executable, `LEPTOS_HASH_FILES=true`);
  `playwright.config.ts` (`LEPTOS_HASH_FILES`). Green with the rest (below).
- Not verified: the image build (no Docker here), see DEC-659.

### Item 7 (docs, decisions)

- `reverse-proxy.adoc`: the application sends the headers, the proxy none; table of values,
  the two settings, snippets without header lines, check "each once".
  `operations.adoc`: the upgrade check no longer speaks of `no-cache`.
  `configuration.adoc` regenerated (`UPDATE_DOCS=1 cargo test -p config`).
  `config/.env.example`: the two new keys (the xtask test asks for every `APP__` key the
  proxy page names).
- DEC-655 (headers), 656 (page policy, nonce, what was verified), 657 (HSTS), 658 (cache
  directives), 659 (hashed names), 660 (Playwright guard).

### Finding: `form-action` and the sign-out redirect

- Assumed: Chromium refuses the redirect of the sign-out form to the provider unless
  `form-action` names it. Tested on a build with `form-action 'self'` alone:
  `signin.spec.ts` "sign-out ends the session here, then at the provider" PASSED, no
  violation. So the Chromium of Playwright 1.64.1 does not check redirects. The origins stay
  in the policy as a precaution (CSP Level 3 describes the check); DEC-656, the proxy page
  and the comments say so. WebKit of the dev shell does not start on this host
  (`browserContext.newPage: Target page, context or browser has been closed`), so no browser
  that enforces it was tried.

### Item 8 (final runs)

- First whole run on the real policy (before the `form-action` wording fix, same policy):
  152 passed, 1 failed, 11 did not run (`sso` waits for `chromium`); 0 policy violations,
  0 panics. The failure: `item.spec.ts:333` "an item outside the library ...":
  `route.continue: Route is already handled!` at the spec's own held route (line 379). Not
  a policy violation. The same test alone, `--repeat-each=8`: 8 passed.
- Final chain on the final sources: `cargo fmt --all --check` ok; clippy pedantic
  (workspace, native features) ok; clippy wasm (`web`, `ui`) ok (run before the last
  comment-only edits); `cargo test -p xtask -p auth -p web --features web/ssr` ok (web lib
  218 passed); `cargo test -p config` 17 passed;
  `cargo test -p server --features gallery,all-connectors --lib --test it`: 19 + 120 passed,
  2 ignored.
- Final whole Playwright run on the final build (`chromium` + `sso`, fresh database):
  164 passed, 0 failed, 15.0 min; 0 `panicked at` lines in the server log; no policy
  violation in any test. `item.spec.ts:333` passed this time: the failure of the run before
  did not repeat (explained by review SEC1-1: a race in the spec between releasing its held
  route and `unroute`; fixed, see "Review fixes").
- Cleaned up: test and e2e databases dropped, test executables in `target/debug/deps`
  deleted.

## Out of scope, noticed

- operations.adoc has no paragraph "which hosts a browser contacts for pictures" yet (a
  follow-up DEC-640 wrote down for the operations page).
- No report endpoint for policy violations (`report-to`); the console is the only place.
- `item.spec.ts:333` failed once in three whole runs with `Route is already handled!`
  (explained and fixed: review SEC1-1, "Review fixes").
- The image build with `hash.txt` has to be tried on a host with Docker or podman.

## Review fixes

Review: docs/reviews/2026-11-13-sec1-review.md (SEC1-1..10). Nothing is committed. Commands:
`nix develop -c bash -c 'cd src/rust; CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 cargo ...'`
with the service env of sec0-progress.md. State per finding, written as each one is done.

### SEC1-1 (must): the `unroute` race in item.spec.ts - applied (edits done; runs see "Final runs")

- The one-off failure of `item.spec.ts:333` ("Route is already handled!") is EXPLAINED: a
  race in the spec, not in the application and not caused by the policy fixture. The spec
  released its held route and removed the handler at once; removing the last handler makes
  Playwright continue every route the handler still holds, and the handler's own
  `route.continue()` then finds the route handled (playwright-core 1.64.1,
  `Route.removeHandler` -> `continue({isFallback: true})`, `_startHandling` asserts).
- Fixed at both places (`item.spec.ts`, the "an item outside the library" test and the
  "both buttons keep waiting" test): `const shown = page.waitForResponse(...item_page);
  show(); await shown; await page.unroute(...)`, the pattern `heldRefetch` in the same file
  already had.
- Searched every spec for `unroute` and `page.route`: seven `unroute` calls. The two above
  were the only ones that release a held handler and remove it without waiting.
  `item.spec.ts` `heldRefetch`: already waits for the response. `notifications.spec.ts`
  (two): `unrouteAll({ behavior: "ignoreErrors" })` in `finally`, which is the documented
  way to drop handlers that may still run. `offline.spec.ts` (two): the handlers there hold
  nothing (they abort at once), and the outcome is awaited before the handler goes (the
  `requestfailed` event; the sign-out dialog that only shows after the push failed). Left as
  they are. Held routes that are never removed (admin, home, library, shell, the funnel
  routes) end with the page.

### SEC1-5: overlay risk of `style-src-attr 'unsafe-inline'` - recorded, rework deferred

- DEC-661 names what the directive allows (overlay, hiding, a request through
  `background:url`) and what not (selectors, scripts), and why the rework is not done here.
- Follow-up package `sec1b` in docs/fable/round2/next-packages.md (after sec1).

### SEC1-4: the 404 page under `/pkg` is `no-store` - applied (DEC-662)

- Red: `security_headers::hashed_bundle_files_are_kept_for_good` (new rows: a hashed name
  that does not exist, signed out and signed in, and `missing.wasm`): `left: Some("no-cache")
  right: Some("no-store")`.
- Fix: `pkg_no_cache` (apps/server/src/lib.rs) leaves a document alone
  (`security_headers::is_document`, now shared with `Headers::apply`), so the header layer
  answers `no-store`. The pinning rows of `security_headers.rs` and `ops.rs`
  (`unhashed_bundle_files_are_revalidated`, `/pkg/missing.wasm`) now ask for `no-store`.
- Green: see SEC1-9 (one run for the three server findings).

### SEC1-6: the hash file is checked against the site root - applied (DEC-663)

- Choice: hashed names only when the files `hash.txt` names are in `<site root>/pkg`;
  else the plain names with a warning when the site has `app.js`; else no start, with a
  message that names the missing files and the cause (different builds).
- Red: `infra::bundle_tests::a_hash_file_that_names_missing_files_is_not_followed`
  (`unwrap_err()` on `Ok(true)`: the old check followed the file).
- Files: `apps/server/src/infra.rs` (`bundle_hashes`, `hashed_names`, tests),
  `apps/server/src/main.rs` (passes the package directory and the output name).
- Tried with the real executable: `target/debug/server` with `LEPTOS_HASH_FILE_NAME` naming a
  hash file with three other hashes ends with status 1 and "the hash file ... names files
  that are not in target/site/pkg (missing: app.<hash>.js, ...wasm, ...css): the site root
  and the hash file come from different builds. ...".

### SEC1-9: server test rows for a redirect and the event stream - applied

- `security_headers::every_answer_carries_the_security_headers`: two more rows, a `303`
  (signed-out `/` before the first account, `Location: /setup`) and the head of
  `/api/v1/events` (new helper `head`: the body of a stream is not read).
  `answers_that_depend_on_the_session_are_not_stored`: the event stream has the closed
  policy and keeps `Cache-Control: no-cache`, the directive its own handler sets (my first
  assertion asked for `no-store` and was wrong; DEC-658 leaves a handler's directive alone).
- Not seen red: the rows hold what the layer already did (the review: "covers them by
  construction; no test holds it").
- Run (SEC1-4, SEC1-6, SEC1-9 together):
  `cargo test -p server --features gallery,all-connectors --no-fail-fast --lib --test it --
  bundle_tests security_headers:: ops::`: 12 unit + 11 integration passed, 0 failed.

### SEC1-7, SEC1-8, SEC1-10: reverse-proxy page - applied (docs only, DEC-664)

- SEC1-7: paragraph "Taking it back" under `APP__SERVER__HSTS_MAX_AGE_SECS` (`0` does not
  revoke; the proxy sends `max-age=0` over HTTPS for a while; RFC 6797 6.1.1).
- SEC1-8: the section says what "the proxy sets none" leaves bare (the proxy's own
  answers) and has a new optional part "Headers on the proxy's own answers" with Caddy's
  `header ?Field` defaults; never for the policy; nginx stays without. Not tried on a proxy.
- SEC1-10: the sign-out paragraph no longer states what one Chromium build does; it says a
  browser may check each redirect and that the policy therefore names the origin, which
  stays. The observation stays in DEC-656 as an observation.
- File: `docs/modules/ROOT/pages/reverse-proxy.adoc`. Check: `cargo test -p xtask` (the
  docs tests) in the final runs.

### SEC1-3: the "shared fixtures" check cannot be passed by accident - applied (DEC-665)

- `global.setup.ts`: `namesPlaywright` (a Playwright package named as a module, any quotes)
  over every script file under `tests/` except `fixtures.ts`; eleven samples that must match
  (single quotes, multi-line import, re-export, `require`, dynamic import, template string,
  `playwright/test`, `playwright`, `playwright-core`), three that must not (`./fixtures`,
  `@axe-core/playwright`, a comment). `helpers.ts` imports from `./fixtures` now.
- Red: the new samples against the old pattern: `Error: import { test } from
  '@playwright/test'; Expected: true Received: false`. A second red on the way: the new
  pattern found `global.setup.ts` itself (its samples); the package names are now put
  together at run time.
- Green: `npx playwright test --project=setup -g "every spec uses"`: 1 passed.

### SEC1-2: standing canary for the policy guard - applied (DEC-665)

- New `src/web/tests/policy-guard.spec.ts` (5 tests): three "sees" tests (both signs, on the
  built-in page, a `browser.newContext()`, a `browser.newPage()`; they empty the list) and
  two `test.fail()` tests that the teardown of the guard must fail.
- Run on the unchanged guard (`--project=chromium --no-deps policy-guard`): 5 passed (the two
  `test.fail()` tests failed as expected).
- Seen red, one mutation of `fixtures.ts` per run, restored after each (`cmp` with the kept
  copy at the end):
  - M1 teardown assertion removed: 2 failed, both "Expected to fail, but passed."
  - M3 console pattern changed: 3 failed ("the console line: ..." in the three "sees" tests).
  - M4 event name changed: 3 failed ("the event: ...").
  - M5 contexts a spec opens not watched: 3 failed (the two own-context "sees" tests and
    the own-context `test.fail()` test, "Expected to fail, but passed").
  - M2 the `context` override removed: NOT red, 5 passed. Playwright's built-in context is
    created through the patched `browser.newContext` too, so the guard still saw the
    violation: the override is a second path, not the only one.

### Final runs of the review fixes (2026-11-13, all on the final sources)

- `cargo fmt --all --check`: ok (after `cargo fmt -p server`).
- `cargo clippy -p server --all-targets --features gallery,all-connectors -- -D warnings -W
  clippy::pedantic`: ok. No other crate was touched by the fixes.
- `cargo test -p server --features gallery,all-connectors --no-fail-fast --lib --test it`
  (whole library tests and the whole `it` binary): 20 + 120 passed, 2 ignored, 0 failed.
- `cargo test -p xtask` (docs and `.env.example` checks, white-label): all passed.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery`:
  ok; the bundle hashes did not change (no change in `web`/`ui`).
- Whole Playwright run on that build, fresh database, default ports (`npx playwright test`:
  `setup`, `chromium`, `sso`): 169 passed, 0 failed, 16.0 min (164 as before plus the five
  canary tests); 0 `panicked at` lines in the server log (437 lines); no policy violation
  outside the canary. `item.spec.ts:333` and `:1157` passed.
- The two fixed tests alone, `--repeat-each=8`: 16 + 2 setup = 18 passed. (The race was
  rare before the fix too; these runs do not prove its absence, the reading of the
  Playwright source in SEC1-1 does.)
- Cleaned up: the `e2e_sec1fix_*` databases dropped. Test databases (`test_%`) and test
  executables left alone: other lanes were building and testing at the same time.

State: every finding SEC1-1..10 applied, except the rework behind SEC1-5 (deferred with
DEC-661 and package `sec1b`). DEC-661..665. Nothing committed.

Still open after the review fixes:

- The image build with `hash.txt` (no container runtime on this host), as before.
- The Caddy and nginx lines of the reverse-proxy page were not run on a proxy; whether
  Caddy's `?` defaults reach its own error answers is not checked (the page says so).
- SEC1-9 rows and the `oidc` `form-action` assertion were not seen red.
