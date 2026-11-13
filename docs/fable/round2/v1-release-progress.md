# v1-release progress (lane release, worktree kithkeep-lane-release, branch lane/v1-release)

Brief: docs/handoff/fable-briefs/V1-RELEASE.md (items 1-9), v1-plan.md addenda 2-4.
Decisions: docs/fable/round2/v1-release-decisions.md (LV-1, LV-2, ...).
Scripts: <scratchpad>/lane-release/ (cargo.sh). Ports 3800-3899, database prefix `lv_`.

| Item | State | Tests (red -> green) | Files |
|---|---|---|---|
| 1 pre-v1 shims | done | xtask ops `no_pre_v1_upgrade_shims_are_left` red (23 hits) -> green; ops tests adjusted (the 0034 section test replaced by `an_existing_database_keeps_its_password`) | assets backend/config/lib, cli main, Dockerfile, compose.yml, .env.example, README, install/operations/configuration.adoc, xtask tests/ops.rs |
| 2 key prefix `SOURCE_` | done | config `source_key_variables_are_a_fallback`, `origins_...`, `the_operator_contact_has_a_short_variable` red -> green; xtask whitelabel `former_env_prefix_not_used` + unit test (guard written with the rename, not seen red on the tree; its unit test proves detection) | config lib, connectors tmdb/igdb/tvdb/podcastindex, connectors tests/tmdb, server it/oidc (TEST_OIDC_LIVE_*), xtask record + whitelabel, playwright.config.ts, capture.mjs, .env.example, docs connectors/*.md, connectors.adoc, configuration.adoc |
| 3 test db drop | done | store-pg tests/testing.rs `a_passing_test_drops_its_database_and_a_failing_one_keeps_it` red (db left) -> green; `keep_test_db_is_read_as_a_flag`; whole store-pg suite: 0 new test_ dbs | store-pg src/testing.rs, tests/testing.rs |
| 4 squash | done (sqlx-prepare run: .sqlx unchanged) | fixture `tests/fixtures/schema.txt` written from the OLD chain (UPDATE_SCHEMA=1), then the baseline compared against it: green; ignored proof `a_chain_of_migrations_builds_the_schema_of_this_build` with SCHEMA_FROM_DIR=old chain: green (1427 lines); negative control (chain without 0037) fails as it should; pg_dump --schema-only of chain vs baseline database: identical (3095 normalised lines), data-only: same rows (instance_settings 1); migrations.rs dry-run test generalised to one migration | store-pg migrations/0001_v1_baseline.sql (37 files removed), src/schema.rs, tests/upgrade.rs, tests/fixtures/schema.txt, tests/migrations.rs, tests/common/mod.rs (env_on), comments naming old migration files (importers, integrations, auth, catalog-core, assets, notify, search, store-pg) |
| 5 changelog, notes, staging | done | xtask ops `the_release_process_is_written_down` (written after the pages; green) | CHANGELOG.md, docs pages releases/index.adoc, releases/template.adoc, releases/v1.0.0.adoc, staging.adoc, operations.adoc (Upgrade = routine procedure; Upgrade notes -> release notes), nav.adoc, README.md |
| 6 upgrade-test | done | xtask tests/upgrade.rs (4 tests) red -> green, then `v1-polish` (a package tag) added as a non-release case: red -> green; store-pg ignored `the_previous_release_upgrades_to_this_one` run with this build as "previous release": green (2 accounts, 5 entries, 3 events, 1 list, 3 people, 1 play read back); `cargo xtask upgrade-test` today: skips with a message (no v<n> tag) | xtask src/upgrade.rs, src/main.rs, src/lib.rs, tests/upgrade.rs, .gitlab-ci.yml job upgrade-test, store-pg tests/fixtures/upgrade-seed.sql |
| 7 release checklist | done | - | docs/HANDOFF.md section 1 "Release checklist" |
| 8 board-game by-hand checks | done (no code change needed; checks passed on first run, they cover existing behaviour) | boardgames.spec new test, contact.spec by-hand row | src/web/tests/boardgames.spec.ts, contact.spec.ts |
| 9 website release.toml | done (left empty, ids prepared as a comment) | - | src/rust/apps/website/content/release.toml |

## Final runs (in order)

- fmt --all --check: green.
- clippy pedantic native (workspace, all targets, NATIVE_FEATURES of xtask ci): green (after
  two fixes: doc backticks in schema.rs, extension comparison in xtask upgrade.rs).
- clippy pedantic wasm (-p web -p ui, hydrate+gallery): green.
- cargo test -p xtask -p web (all binaries): green (web 245+3+5, xtask lib 23, ops 20,
  upgrade 4, whitelabel, locales, architecture, e2e_log, load).
- cargo test (TEST_DATABASE_URL) -p config -p assets -p cli -p store-pg -p connectors
  --features connectors/all, --no-fail-fast: 34 binaries green, 0 test_ databases left. The
  first run failed in store-pg phase1 `the_completion_migration_dates_existing_entries_and_
  rebuilds_the_cache`: it replayed the pre-v1 data migration 0036 by name; removed (a pre-v1
  upgrade path, greenfield rule).
- server `it` binary, whole (gallery,all-connectors, REQUIRE_SERVICES=1, no Meilisearch/S3
  env): 139 passed, 2 ignored; test_ databases before 1154, after 1154 (none left).
- cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery:
  green.
- Playwright, all projects (setup, chromium, contact, sso), ports 3800-3806, db lv_e2e,
  server lv-server: 195 passed, exit 0; no panic in the server log. The two new LB-33 checks
  passed on their first run.
- Databases created by this lane and dropped: lv_chain, lv_base, lv_sqlx, lv_e2e, and the
  test_ databases two failing runs kept on purpose (negative control, the removed phase1 test).

## Owner / orchestrator
- Tag name `v1.0.0` (LV-7); set the date in CHANGELOG.md and releases/v1.0.0.adoc, workspace
  version 0.1.0 -> 1.0.0 at the tag (HANDOFF release checklist).
- Website: release.toml stays empty; features.toml/home.toml still say "coming" for book,
  board_game, manual_entry (LV-8).
- Rename local `.env` keys `KK_*` -> `SOURCE_*` (no alias).
- A database from a pre-v1 build cannot be migrated to v1 (baseline version 1 has another
  checksum than the old 0001_core): local throwaway databases must be recreated.

## Review fixes (docs/reviews/2026-11-13-v1-release-review.md, REL-1..15; decisions LV-10..19)

| Id | State | Tests (red -> green) | Files |
|---|---|---|---|
| REL-1 | applied (LV-10): `cli reindex --clear --now` indexes in the CLI process; staging keeps `APP_ROLE=web` | cli `parses_commands` (`--now`) red (no field) -> green; server it `cli::reindex_now_fills_the_index_without_a_worker` (seen red with `index_now` mutated to not drain: 0 hits); xtask ops `the_staging_page_and_the_role_it_uses_fit_together` red on the old checklist line -> green | apps/cli/src/main.rs, apps/server/src/escape.rs (`index_now`), apps/server/tests/it/cli.rs, staging.adoc, operations.adoc (Search index), xtask tests/ops.rs |
| REL-2 | applied (LV-11): release candidate tag `vX.Y.Z-rc.N` for staging, release tag on its commit; checklist reordered (website before the candidate) | xtask ops `the_release_checklist_and_the_staging_page_agree` (seen red by mutation of the checklist); xtask upgrade: rc tags are not releases (unit cases) | docs/HANDOFF.md section 1, staging.adoc, xtask tests/upgrade.rs |
| REL-3 | applied (LV-12): generator in the server test binary, `cargo xtask upgrade-seed`, seed regenerated (788 rows, 61 tables) | store-pg `the_seed_has_rows_in_every_table` red (47 tables without rows) -> green; xtask `a_dump_becomes_the_seed` red (missing fn; then red on a multi-line value) -> green; generator run: every table but http_cache, oidc_used_jti, oidc_used_states has rows | apps/server/tests/it/upgrade_seed.rs (+ main.rs), store-pg tests/upgrade.rs (store-pg generator removed), tests/fixtures/upgrade-seed.sql, xtask src/upgrade.rs + main.rs |
| REL-4 | done (rehearsal below; nothing left behind) | - | - |
| REL-5 | applied (LV-13): fixture of released checksums + CI job in every pipeline | store-pg `released_migrations_have_not_changed` red (no fixture) -> green, and red again with one baseline byte changed; xtask `ci_runs_the_upgrade_test_in_every_pipeline` red (rules) -> green | tests/fixtures/released-migrations.txt, tests/migrations.rs, .gitlab-ci.yml, xtask tests/upgrade.rs |
| REL-6 | applied (LV-14): per-table count + MD5 over the old columns, value read-back | rehearsal steps c/e/e2 below (red as wanted) | store-pg src/schema.rs (`table_columns`, `table_hashes`, `contents`), tests/upgrade.rs |
| REL-7 | applied (LV-15): required step on the staging page (S3 -> fs on the copy, or a bucket of its own; no `storage evict`) | covered by `the_release_checklist_and_the_staging_page_agree` | staging.adoc |
| REL-8 | applied: the staging checkout links the production backup (`ln -s "$PWD/backup" ../tracker-staging/backup`) | same ops test | staging.adoc |
| REL-9 | applied (LV-16) | store-pg `the_snapshot_sees_ownership_security_collation_domains_and_settings` red (sequence ownership) -> green; schema.txt regenerated (additive diff only, same order) | store-pg src/schema.rs, tests/fixtures/schema.txt |
| REL-10 | applied: template `git fetch --tags` / `git checkout vX.Y.Z`; operations "Going back" step without the pre-v1 sentence; checklist step Website names features.toml/home.toml for v1; template stays a page (acceptable per review); HANDOFF 4.8 `v1` -> `v1.0.0` left to the orchestrator (with LV-7) | ops test (template has no `git pull`) | releases/template.adoc, operations.adoc, HANDOFF.md |
| REL-11 | applied: checklist step 3 `cargo check`, Cargo.lock in the same commit | ops test (`Cargo.lock` in the checklist) | HANDOFF.md |
| REL-12 | applied as documentation, sweep deferred (LV-17) | - | store-pg src/testing.rs (module comment), HANDOFF.md "Kept test databases" |
| REL-13 | applied (LV-18) | store-pg `the_dry_run_names_applied_migrations_that_differ_from_this_build` red (missing fn) -> green | store-pg src/lib.rs (`mismatched_migrations`), apps/cli/src/main.rs |
| REL-14 | done: red runs of the never-red checks | ops `the_release_process_is_written_down` red with `-p tracker-staging` removed; boardgames.spec hand test red with the Wikidata credit asserted present (count 1); contact.spec red with the by-hand row name changed; all restored byte-identical | - |
| REL-15 | applied (LV-19) | xtask lib `the_former_env_prefix_guard_skips_ignored_files` red (config/.env and owner-inbox reported) -> green | xtask src/whitelabel.rs |

REL-4 rehearsal (scratch commits made with a temporary index and `git commit-tree`, tagged
`v0.0.1`/`v0.0.2` only for the runs, never on a branch; `cargo xtask upgrade-test <tag>`):
- baseline: this tree as "previous release" (seed of the first generator run): green, read back
  4 accounts, 26 entries, 85 events, 5 lists, 8 people, 1 play.
- a) one byte of `0001_v1_baseline.sql` changed after the tag: red, `VersionMismatch(1)`
  (sqlx: previously applied but modified); `released_migrations_have_not_changed` red too.
- b) scratch `0002` (ADD COLUMN ... NOT NULL DEFAULT on two tables + backfill UPDATE): green.
- c) scratch `0002` deleting list items: red, "rows of list_items after the upgrade" (1 vs 18).
- d) schema drift, a `0002` whose index depends on the data (`DO ... IF EXISTS ... CREATE
  INDEX`): red, "an upgraded database differs from a new one" (the extra index).
- d') a released `0002` (tag `v0.0.2`) edited afterwards: red, `VersionMismatch(2)`.
- e) a value rewritten with equal counts: red twice: list note upper-cased -> the read-back
  value check ("the note: [SIGNED]"); `reviews.contains_spoilers` flipped -> "content of
  reviews after the upgrade" (MD5 differs).
- f) a column dropped (`list_items.note`): red in the read-back (the code reads the column).
- Clean-up: scratch 0002 removed, baseline byte-identical to the kept copy, tags deleted (no
  `v0*` tag left), `target/upgrade-test/v0.0.*` removed, every test database of these runs
  dropped. Final: the ignored upgrade test with this tree and the regenerated seed: green.

Final runs (in order, CARGO_BUILD_JOBS=3, no gate flag seen):
- fmt --all --check: green.
- clippy pedantic native (workspace, all targets, NATIVE_FEATURES): green (after fixes:
  doc backticks, assert_eq for empty, usize counters, fold instead of format-collect, a
  module allow of too_many_lines in the seed generator).
- clippy pedantic wasm (-p web -p ui, hydrate+gallery): green.
- cargo test -p xtask -p web, all binaries: green (web 245+3+5, xtask lib 24, ops 22,
  upgrade 5, whitelabel 3, locales, architecture, e2e_log, load, main).
- cargo test -p store-pg -p cli (TEST_DATABASE_URL, REQUIRE_SERVICES=1): 18 binaries green;
  ignored `the_previous_release_upgrades_to_this_one` with this tree and the final seed:
  green; test_ databases before = after.
- server `it` binary, whole (gallery,all-connectors, REQUIRE_SERVICES=1): 140 passed,
  3 ignored; test_ databases before = after.
- cargo leptos build --bin-features gallery,all-connectors --lib-features hydrate,gallery:
  green.
- Playwright, all projects (setup, chromium, contact, sso), ports 3800-3806, db lv_e2e,
  server lv-server: 195 passed, exit 0; no panic in the server log; lv_e2e dropped.

Owner / orchestrator (added):
- Renumber LV-10..19 into DECISIONS.md; HANDOFF 4.8 should say `v1.0.0` (with LV-7).
- At v1: the checklist now has a candidate tag step (`v1.0.0-rc.1`, pushed) before staging.
- The upgrade seed must be regenerated (server test + `cargo xtask upgrade-seed`) in every
  change that adds a migration; the release fixture `released-migrations.txt` gets the new
  lines at each release.
- The CI job `upgrade-test` now runs in every pipeline (GIT_DEPTH 0, Postgres service); it has
  still never run on a runner.
