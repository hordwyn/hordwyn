# v1-ops progress (image, first run, secrets key, runbooks, compose, /pkg cache)

Resume notes for a cut-off run. Brief: docs/handoff/fable-briefs/PKG-implement.md.
Scope: docs/fable/round2/v1-status.md section 5 package 1, next-packages.md "ops1",
HANDOFF 4.8 (OPS-3+4 option a). Nothing is committed.
Build commands: `nix develop -c bash -c 'cd src/rust; CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 cargo test ...'`.
Service env: `TEST_DATABASE_URL=postgres://app@127.0.0.1:55432/postgres REQUIRE_SERVICES=1
TEST_MEILI_URL=http://127.0.0.1:55434 TEST_MEILI_KEY=dev-only-master-key-0123456789`.
DEC range for this package: 618..639 (last entry at the start: 617).

| # | Item | State | Tests (red first) | Files | DEC |
|---|---|---|---|---|---|
| 1 | `cli` in the image, self-check in the image build, Dockerfile test | done | see "Final state" | see "Final state" | see "Final state" |
| 2 | `.env.example` first-run keys + test | done | see "Final state" | see "Final state" | see "Final state" |
| 3 | OPS-3: key out of the blob directory (image: blobs to `/data/blobs`, move at start) | done | see "Final state" | see "Final state" | see "Final state" |
| 4 | OPS-4: banner on Admin > Overview until the key backup is confirmed | done | see "Final state" | see "Final state" | see "Final state" |
| 5 | Runbook: backup, restore, upgrade; reverse-proxy page; first run | done | see "Final state" | see "Final state" | see "Final state" |
| 6 | Compose: required variables, loopback, stop grace 30 s, healthchecks, web/worker pair | done | see "Final state" | see "Final state" | see "Final state" |
| 7 | OPS-12: "one web process plus workers" in the docs | done | see "Final state" | see "Final state" | see "Final state" |
| 8 | `Cache-Control: no-cache` on `/pkg` | done | see "Final state" | see "Final state" | see "Final state" |
| 9 | OPS-6 rest: log pending migrations, `cli migrate --dry-run` | done | see "Final state" | see "Final state" | see "Final state" |
| 10 | SEC-6 rest: start-up warning (plain http and not loopback) | done | see "Final state" | see "Final state" | see "Final state" |
| 11 | Gates: fmt, clippy, locale tests, server page tests, whole chromium project | done | see "Final state" | see "Final state" | see "Final state" |

## Design choices (to become DEC entries)

- OPS-3: the key stays at `/data/secrets.key` (same volume, nothing to lose when an old
  compose file meets the new image); the blobs move to `/data/blobs` (the layout of the
  application defaults `data/blobs` + `data/secrets.key`). New setting
  `storage.fs_legacy_root` (`APP__STORAGE__FS_LEGACY_ROOT=/data` in the image): at start the
  fs backend moves the directories `up`, `img`, `http` from there into `fs_root` (rename,
  same volume) when `fs_root` does not have them yet. Not chosen: a second volume for the
  key (a fresh install with an old compose file would write the key into the container
  layer).
- OPS-4: `secrets_keys.backup_confirmed_at` (migration 0034); the banner shows while the
  active key version has no confirmation (so a rotation asks again); not for the temporary
  key of test harnesses. `POST /admin/secrets/backup-confirmed`, admin, fresh proof, audit
  action `secrets.backup_confirmed`.

## Log

- 2026-11-12: brief, rules, HANDOFF 0/2/4/8, v1-status 3 and 5, next-packages ops1,
  ops-readiness.md and security-audit.md SEC-6 (branch wip/fable-reviews), owner answers read.
- Items 1, 2, 5, 6, 7 (files and docs side) done: `xtask/src/ops.rs` + `xtask/tests/ops.rs`
  (10 tests; the 5 that read the repository files seen red first: Dockerfile 4 problems,
  compose 10 problems, missing install.adoc, runbook lacks `pg_restore`, no
  FS_LEGACY_ROOT; then green, `cargo test -p xtask --test ops` 10 passed).
  Files: src/docker/Dockerfile (cli built with the server's features, copied, `RUN cli help`,
  FS_ROOT=/data/blobs, FS_LEGACY_ROOT=/data, WORKDIR /, VOLUME /data), compose.yml (required
  variables, `${APP_PUBLISH_ADDR:-127.0.0.1}:3000:3000`, stop_grace_period 30s, start_period,
  restart for postgres/meilisearch, commented worker), NEW compose.s3.yml and
  compose.podman.s3.yml (the RustFS services left the base file: required variables are
  checked for every service of a file), compose.podman.yml, config/.env.example,
  docs pages install.adoc (new), reverse-proxy.adoc (new), operations.adoc (roles, admin
  tool, upgrade, backup, restore, legacy blob directory), nav.adoc, README.md.
  The docs already describe what items 3, 4, 9 build (fs_legacy_root, banner,
  `cli migrate --dry-run`, migration log): STILL TO BUILD below.
- Facts checked on the web (Caddy docs reverse_proxy/encode/request_body, Docker interpolation
  docs): Caddy passes Host, sets X-Forwarded-*, flushes text/event-stream; `:?` fails on
  unset or empty. Not found in the docs: whether compose checks required variables of
  services in an inactive profile (hence the separate S3 file), which `.env` is read by
  default with `-f` (hence `--env-file` in every command).
- Item 3 code: `storage.fs_legacy_root` (crates/assets/src/config.rs), `adopt_legacy_root` in
  crates/assets/src/backend.rs with 2 unit tests (the first seen red: read gave `None`
  before the function was called; then `cargo test -p assets --lib backend` 2 passed).
- Red runs seen before the code of items 4, 8, 9, 10: `store-pg --test migrations`
  (`[]` instead of `[32, 33]`), `server --lib exposure` (0 warnings instead of 1),
  `server --test it -- ops::` (overview has no `key_backup`; `/pkg/app.js` without
  Cache-Control).
- Code written for items 4, 8, 9, 10 (NOT YET RUN GREEN at this line): migration
  0034_secrets_key_backup.sql; auth AdminStore `key_backup_confirmed` / `confirm_key_backup`
  (mem + pg); api `admin::confirm_key_backup`, `key_backup_due`, route
  `POST /admin/secrets/backup-confirmed`; contracts `KeyBackupDto`, `AdminOverviewDto.key_backup`;
  web admin_backend, admin_api `confirm_key_backup`, admin/overview.rs `KeyBackup`, audit
  filter action; locales en + de (`admin-key-backup-*`, `admin-audit-secrets_backup_confirmed`);
  server lib.rs `pkg_no_cache`, infra.rs `exposure_warnings` (logged in main.rs);
  store-pg `pending_migrations` + log in `migrate`; cli `migrate --dry-run`.
  `cargo xtask sqlx-prepare` run (2 new files in .sqlx). DEC drafts 618..625 are in the
  session scratchpad, not yet in DECISIONS.md.
- Disk fell to 3.8 GB after sqlx-prepare; freed to 8.8 GB (test executables, duplicate
  rlibs, test databases).
- GREEN after the code: `store-pg --test migrations` 1 passed; `cargo test -p cli` 2 passed
  (parse test changed with the code, not seen red); `server --lib` 8 passed;
  `server --test it -- ops:: admin:: admin_pages:: admin_fns:: cli:: item_page::
  library_page:: home_page:: auth_pages:: setup_page::` 42 passed, 0 failed.
  `UPDATE_DOCS=1 cargo test -p config` (configuration.adoc +1 line, fs_legacy_root).
- DEC-618..625 inserted in docs/DECISIONS.md after DEC-617 (the other agent's 640..642
  follow them).
- Open: gates (fmt, clippy native + wasm, xtask tests, auth/api/assets/web tests), cargo
  leptos build, whole Playwright chromium project, a look at the banner in a browser.
- Gates run on the code before the 304 change: `cargo fmt --all --check` clean; clippy
  pedantic native (`--workspace --all-targets`, CI feature set) clean (two fixes in new test
  files); clippy wasm (`-p web -p ui`, hydrate+gallery) clean; `cargo test -p xtask` all
  green (ops 10, locales 5, whitelabel 2, ...); `-p auth` green; `-p assets` green (14 lib,
  13, 2, 1, 10); `-p api` 13 + 22 + 1; `-p web --features ssr` 215 + 3; `-p contracts -p
  config` 17 + 11.
- `cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery` ok.
  A hand-started server (key file, port 3120, database `v1ops_hand`) logged 34
  "applying database migration" lines and "generated a new secrets key"; curl showed
  `cache-control: no-cache` and `last-modified` on `/pkg/app.js`, BUT a matching
  `If-Modified-Since` got `200`: the Leptos file handler does not answer conditional
  requests. Test extended (`ops::unhashed_bundle_files_are_revalidated`, seen red: 200
  instead of 304), `pkg_no_cache` now answers the 304 itself; DEC-623 says so.
- Note for the next run: `psql` exists only inside `nix develop`; test databases must be
  dropped from there (scratch script hand/start.sh does it).
- Open: rerun `it ops::` green + clippy server, cargo leptos build again, the banner in a
  browser (temporary spec, not committed), whole Playwright chromium project.

## Final state (2026-11-12): all items done, nothing committed

| # | Item | Tests | DEC |
|---|---|---|---|
| 1 | `cli` in the image | xtask `ops::the_image_contains_the_cli_and_keeps_the_key_out_of_the_blob_directory` (red first), `an_image_without_the_cli_or_with_the_key_in_the_blob_root_is_refused`; in CI the image build runs `cli help` | 618 |
| 2 | `.env.example` | xtask `ops::the_env_example_lists_every_first_run_key` (red first), `a_missing_first_run_key_is_reported` | 622 |
| 3 | OPS-3 blob layout | assets `backend::tests::the_earlier_blob_directories_are_moved_and_stay_readable` (red first), `an_existing_directory_wins_and_the_same_root_is_left_alone`; xtask `the_image_names_the_earlier_blob_root` (red first) | 619 |
| 4 | OPS-4 banner | server it `ops::the_key_backup_reminder_stays_until_an_admin_confirms_it` (red first; REST, SSR page en + de, gate, audit, rotation, server function); hand check in Chromium (temporary spec, not committed) | 620 |
| 5 | Runbook, proxy page, first run | xtask `ops::the_runbook_names_its_commands` (red first) | 622, 625 |
| 6 | Compose | xtask `ops::compose_requires_its_credentials_and_publishes_on_loopback` (red first), `defaults_for_credentials_and_open_ports_are_refused` | 621 |
| 7 | OPS-12 statement | part of the compose and runbook tests | 625 |
| 8 | `/pkg` no-cache and 304 | server it `ops::unhashed_bundle_files_are_revalidated` (red twice: no header; then 200 instead of 304) | 623 |
| 9 | Migration log, dry run | store-pg `migrations::the_dry_run_lists_the_pending_migrations_and_changes_nothing` (red first); cli `parses_commands` (changed with the code) | 624 |
| 10 | Start-up warnings | server `infra::exposure_tests` (one of two red first) | 624 |

Final runs on the final sources:

- `cargo fmt --all --check` clean. Clippy pedantic native over the workspace (CI features)
  clean before the 304 change, `-p server --all-targets` clean after it (only
  `apps/server/src/lib.rs` and the test changed in between). Clippy wasm clean (no `web`,
  `ui` or `contracts` change after it).
- `server --test it -- ops::` 2 passed after the 304 change. The larger server run (42
  passed: ops, admin, admin_pages, admin_fns, cli, item_page, library_page, home_page,
  auth_pages, setup_page) is from before the 304 change.
- `cargo test -p xtask --test ops --test whitelabel --test locales`: 10 + 2 + 5 on the final
  files.
- `cargo leptos build` (gallery, all-connectors) after the 304 change, then the whole
  Playwright `chromium` project (database `v1ops_1`, 2 workers, ports 3110 to 3114):
  148 passed (setup included), 0 failed, 7.6 minutes, 0 `panicked at` lines in the server log.
- Hand check on that build with a key file (port 3120): 34 migration log lines, the "generated
  a new secrets key" line, `/pkg/app.js` with `cache-control: no-cache` and a `304` for a
  matching `If-Modified-Since`; the banner at 1280 and 390 px in both themes (axe, reflow),
  confirmed through the re-auth dialog, gone after a reload, the audit row in words.
- Not run: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project, cargo-deny, the
  whole server `it` binary, an image build, compose, any command of the runbook, Antora.

Noticed, not built:

- The setup wizard's "Ready" step shows the key file path as a hint without a break
  opportunity: with a 110-character path it overflows by 19 px at 360 px (the global setup
  of the browser suite failed on it in the hand check until a short path was used). The
  image's `/data/secrets.key` fits. One class on the hint in `ui`.
- The audit detail shows raw field names (`key_version: 1`), as for the other actions.
- `KK_BGG_TOKEN` is still listed in `.env.example` although BGG is out (HUB-7).
- The wizard does not check the public URL against the request (ops-readiness 2.4).

## Review fixes (docs/reviews/2026-11-12-v1-ops-review.md, V1OPS-1..10)

Resume notes; nothing is committed. DEC entries for this part start after DEC-642.

- Tests written and SEEN RED before any fix (2026-11-12):
  - assets `backend::tests::the_earlier_blob_directories_are_moved_and_stay_readable`
    (`orig/h1` read `None`, V1OPS-3) and
    `the_directories_a_start_would_move_are_named_and_left_alone` (stub returned `[]`, V1OPS-5).
  - xtask `--test ops`: 8 new tests red (V1OPS-1 section missing, V1OPS-2, V1OPS-3 docs lack
    `orig`, V1OPS-4, V1OPS-5 compose/upgrade, V1OPS-6, V1OPS-7, V1OPS-10); 11 older ones green.
- Code written, NOT YET RUN GREEN at this line:
  - V1OPS-3: `assets::prefix` (`up`, `img`, `http`, `orig`, `ALL`), used by the key builders,
    `classify` and the move.
  - V1OPS-5: `assets::legacy_dirs_pending`; cli `refuse_pending_move` is still a stub with
    its test (to be seen red), then the check goes in.
  - V1OPS-8: `#adm-key-backup-outcome` (tabindex -1) in web admin/overview.rs, WITHOUT the
    focus call yet (the Playwright test in src/web/tests/admin.spec.ts must be seen red);
    api `confirm_key_backup` asks for the fresh proof before the temporary-key shortcut.
  - V1OPS-9: server it `ops.rs` split into three tests, `too_many_lines` allow removed.
- Still to write: the docs and compose changes for V1OPS-1, 2, 4, 5, 6, 7, 10, DEC entries.
- cli `tests::storage_commands_refuse_to_move_the_earlier_blob_directories` SEEN RED (stub
  returned Ok), then `refuse_pending_move` written.
- Docs and compose written: operations.adoc (Upgrade, new "Coming from a build before 0034",
  new "Going back", 0034 note with the manual reverse, Backup table, Restore with
  recreate-then-restore and `--wait`, new "Restore rehearsal beside the live instance", blob
  section with four directories), install.adoc, reverse-proxy.adoc (`$http_host`, nginx
  1.25.1), README.md, config/.env.example (`APP_PUBLISH_PORT`, `APP_IMAGE`, `APP_ROLE`),
  compose.yml (`image:` on app, worker runs that image, port and role variables),
  compose.podman.yml, Dockerfile comment, configuration.adoc regenerated.
- GREEN (15:19): `UPDATE_DOCS=1 cargo test -p config` 17; `cargo test -p assets -p cli -p xtask`:
  assets 15 lib + 13 + 2 + 1 + 10, cli 3, xtask ops 19 (11 old + 8 review), locales 5,
  whitelabel 2, architecture 1.
- The database part of the runbook was run against the local Postgres 18.6 (scratch database
  `v1ops_restore_check`, dropped again): pg_dump -Fc of `app_dev` (migrated to 0034),
  restore, an extra table, then `dropdb --force --if-exists --maintenance-db=postgres`,
  `createdb`, `pg_restore --no-owner` (rc 0, the extra table gone, 34 migrations), and the
  manual reverse of 0034 (rc 0, max version 33). No compose command was run.
- Disk: the combined `-p assets -p cli -p xtask` run built a feature set of its own (4.9 GB
  in target/debug/deps); free space fell to 3.8 GB. Deleted the test executables, the two
  test databases and the 351 files that run had written (after 15:11); 7.1 GB free. Run one
  package per cargo command from here on.
- DEC-643..646 appended to docs/DECISIONS.md (after DEC-642).
- Open: clippy/fmt, server it `ops:: admin:: admin_pages::`, cargo leptos build WITHOUT the
  focus call + the new Playwright test red, then the focus call, build, whole chromium project.
- cargo leptos build WITHOUT the focus call, then the new Playwright test alone (database
  `v1ops_r1`, dropped): SEEN RED at `toBeFocused` on `#adm-key-backup-outcome` (the gate, the
  dialog, the vanished reminder and the "Recorded" text all passed before it).
- Focus call added (`keyboard::when_present` on the shown block). On these sources:
  `cargo fmt --all --check` clean; clippy pedantic native (`--workspace --all-targets`, CI
  features) clean; clippy wasm (`-p web -p ui`, hydrate+gallery) clean;
  `server --test it -- ops:: admin:: admin_pages::` 11 passed (ops 4, admin 5, admin_pages 2);
  `cargo test -p api` 13 + 22 + 1.
- Disk again at 3.5 GB after the server test binary; deleted executables, test databases and
  the finished cli run's variants (15:00 to 15:10); 5.8 GB.
- Open: `cargo test -p web --features ssr`, cargo leptos build WITH the focus call, the new
  Playwright test green, the whole chromium project.
- `cargo test -p web --features ssr` 215 + 3. cargo leptos build with the focus call
  (15:54), then the whole Playwright `chromium` project on that build (database `v1ops_r2`,
  dropped; 2 workers; ports 3110 to 3114): 149 passed (setup included, the new test among
  them), 0 failed, 7.6 minutes, 0 `panicked at` lines in the server log.
- After the last doc edits: `cargo test -p xtask` green again (ops 19, locales 5,
  whitelabel 2, architecture 1, e2e_log 6, load 3, lib 17), `cargo fmt --all --check` clean.

### Review fixes: final state (2026-11-12), all ten findings applied, nothing committed

| Finding | State | What | Tests (red first unless noted) | DEC |
|---|---|---|---|---|
| V1OPS-1 | applied | operations.adoc "Coming from a build before 0034" (env file first, the passwords the instance has, `APP_PUBLISH_ADDR`, old upload path, then pull; password change afterwards); sentence in `.env.example`, install.adoc, README, compose.yml header | xtask ops `the_upgrade_of_an_instance_from_before_0034_is_spelled_out` | 644 |
| V1OPS-2 | applied | `APP_PUBLISH_PORT` in compose.yml and compose.podman.yml; "Restore rehearsal beside the live instance" (`-p tracker-restore`, own port, own image name, `APP_ROLE=web`) | xtask ops `a_restore_can_be_rehearsed_beside_the_live_instance` | 644 |
| V1OPS-3 | applied | `assets::prefix::ALL` with `orig`; four directories in operations.adoc, the configuration reference, the Dockerfile comment | assets `the_earlier_blob_directories_are_moved_and_stay_readable`; xtask ops `the_docs_name_all_four_blob_directories` | 643 |
| V1OPS-4 | applied | "Going back" (recreate the database, no `--clean`, blob directories back, manual reverse of 0034); Restore uses the same recreate-then-restore | xtask ops `the_way_back_is_complete`; the SQL side run on the local Postgres | 644 |
| V1OPS-5 | applied | worker runs the app's image (`APP_IMAGE`), `dc build` for everything, `APP_ROLE`; cli storage commands refuse while directories are still to be moved | assets `the_directories_a_start_would_move_are_named_and_left_alone`; cli `storage_commands_refuse_to_move_the_earlier_blob_directories`; xtask ops `every_container_runs_the_image_the_app_service_builds` | 643 |
| V1OPS-6 | applied | backup table says import files; note at `AssetService::upload` | xtask ops `the_backup_table_names_what_the_upload_directory_holds` | 645 |
| V1OPS-7 | applied | `$http_host`; `http2 on;` needs nginx 1.25.1, older form named | xtask ops `the_nginx_snippet_keeps_the_host_header_and_names_its_version` | 646 |
| V1OPS-8 | applied | focus to `#adm-key-backup-outcome` after a recorded confirmation; the gate comes before the temporary-key shortcut | Playwright admin.spec.ts "Overview: after the key backup is confirmed the focus is on the confirmation" | 646 |
| V1OPS-9 | applied | three tests instead of one, no `too_many_lines` allow | the three tests themselves (a refactoring: not seen red) | 646 |
| V1OPS-10 | applied | `dc up -d --wait postgres meilisearch` | xtask ops `the_restore_waits_for_the_services` | 644 |

Not run, as before: `cargo xtask ci`, `cargo xtask e2e`, the `sso` browser project,
cargo-deny, the whole server `it` binary (only `ops:: admin:: admin_pages::`), an image
build, any `docker compose` or `podman compose` command, Antora. Not seen red:
`a_section_ends_at_the_next_heading_of_its_level` (helpers written with their test).
The assets and cli tests ran before `cargo fmt` reformatted and before clippy; nothing but
formatting changed in those crates afterwards.

Noticed, not built:

- `importers/src/runner.rs:84` still writes the literal `up/`; it could use
  `assets::prefix::UPLOAD` if the crate may depend on `assets`.
- A second copy started with `APP_ROLE=all` would deliver queued notifications of the dump
  again; there is no setting that switches delivery off for a copy.
- `podman compose` with the podman-compose provider (`!reset`, the long `env_file` form,
  `pull_policy`, `--wait`) is still unchecked.
