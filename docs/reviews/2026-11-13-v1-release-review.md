# Review: v1-release (lane/v1-release, uncommitted tree on top of 9f552b6)

Date: 2026-11-13. Independent review, read-only: no cargo, npm or Playwright run, no
database touched. Everything below comes from reading the diff, the untracked files, the 37
removed migrations (`git show HEAD:...`), the lane's decisions (LV-1..9) and progress log.
Paths are relative to the repository root.

Scope of attention: what breaks a fresh install, the first production upgrade after v1, or
the release process.

## Summary

| Severity | Count | Ids |
|---|---|---|
| Critical | 0 | - |
| High | 0 | - |
| Medium | 7 | REL-1 .. REL-7 |
| Low | 8 | REL-8 .. REL-15 |

Must fix before merge: REL-1, REL-2 (both are wrong instructions in the release process this
package delivers; each is a few lines of docs).

Must fix before the `v1.0.0` tag, may merge first: REL-3, REL-4 (what the tag freezes for the
first real upgrade test).

May defer: REL-5 .. REL-15.

No defect was found in the baseline migration, the shim removal, the prefix rename or the
test-database cleanup. What was checked and found in order is listed at the end, with the
limits of a read-only check.

## Must fix before merge

### REL-1 (Medium) The staging checklist has a line that fails on a correct upgrade

- Where: `docs/modules/ROOT/pages/staging.adoc:87` (checklist), with `:36-45` (`APP_ROLE=web`).
- Problem: the checklist asks for `dcs exec app cli reindex --clear` and then "a search finds
  a title you know". `cli reindex` only queues the documents
  (`src/rust/apps/cli/src/main.rs:336-341`, "queued {n} documents; the worker's indexer picks
  them up"), and the staging copy runs `--role=web`, so no indexer ever runs.
  `reindex --clear` empties the Meilisearch indexes first. `operations.adoc:271-274` says the
  same for the rehearsal in so many words ("in the rehearsal the search index is not rebuilt")
  and skips Restore step 6 for that reason.
- Failure: with the compose default (Meilisearch) the owner runs the line, the search finds
  nothing, and the page says "If a line fails, do not upgrade production". A good release is
  blocked, or the owner learns that the checklist may be ignored.
- Fix: either drop the line and say what the rehearsal says (search is regenerated data, not
  checked on the copy), or make the copy index for the check on purpose: after the other
  lines, `APP_ROLE=all` for one `dcs up -d` is not acceptable (it would send notifications),
  so the honest variant is to drop the search check from staging and keep "open a title" as
  the read check. Adjust `the_release_process_is_written_down` if it should guard this (for
  example: staging.adoc must not contain `reindex`).

### REL-2 (Medium) The release checklist and the staging page contradict each other about the tag

- Where: `docs/HANDOFF.md:178-184` (steps 6 and 8), `docs/modules/ROOT/pages/staging.adoc:28`,
  `docs/modules/ROOT/pages/operations.adoc` "Upgrade" step 2.
- Problem: step 6 says "The owner upgrades the staging copy ... No tag before that
  confirmation", step 8 creates the tag. The staging page builds the copy with
  `git worktree add ../tracker-staging vX.Y.Z`, a tag that does not exist at step 6.
- Failure: at the first release after v1 the owner runs the page as written and
  `git worktree add` fails with "invalid reference: vX.Y.Z"; or the tag is created early to
  make the page work, and a staging failure then means a public tag that must not be used
  (and no migration in a tag may change any more).
- Fix: say in step 6 what the copy is built from before the tag: the release commit
  (`git worktree add ../tracker-staging <commit of steps 1-5>`), or a candidate tag
  `vX.Y.Z-rc.N` (it does not match `is_release_tag` nor the CI rule, which is what is wanted),
  and add one sentence to staging.adoc: "an operator uses the tag; before a release exists
  the copy is built from the release commit". Keep `vX.Y.Z` on the page for operators.

## Must fix before the v1.0.0 tag (may merge first)

### REL-3 (Medium) The upgrade seed that v1.0.0 freezes covers 14 of 65 tables

- Where: `src/rust/crates/store-pg/tests/fixtures/upgrade-seed.sql`,
  `src/rust/crates/store-pg/tests/upgrade.rs` (`write_the_upgrade_seed`, `read_back`),
  `src/rust/xtask/src/upgrade.rs:117-121` (the seed is read from the previous tag only).
- Problem: the next release's upgrade test takes its data from the `v1.0.0` tag. Once the tag
  exists that seed cannot be improved. It has rows in users, companions, catalog_items,
  library_entries, external_ids, lists, list_items, outbox, progress_events, reviews,
  sessions, session_participants, user_change_seq, user_changes. It has no row in:
  credentials and auth_sessions (the accounts cannot sign in), TOTP and recovery codes, OIDC
  providers and identities, connector_credentials and other encrypted secrets,
  instance_settings beyond the default row, notifications and channels, integrations and
  their tokens, imports, assets and the storage index, goals and game groups, catalog
  episodes and seasons, tags, invitations and link requests, jobs and cron_schedules,
  admin_audit, the statistics cache.
- Failure: 0002 adds `NOT NULL` without a default, a CHECK, or a unique index to one of the
  empty tables (say auth_sessions or notifications). The upgrade test is green because the
  table is empty; the owner's instance has rows and the migration fails at start, after the
  old containers were replaced. This is exactly the case the test exists for.
- Fix (before the tag): extend `write_the_upgrade_seed` to put at least one row into every
  table a household instance has rows in (sign-in with a password and TOTP, an OIDC identity,
  one connector key through the secrets path, a notification with a channel, an integration
  token, an import, an asset, a goal, a game group, a show with episodes, a tag, an
  invitation), and add an assertion that names the tables the seed leaves empty on purpose
  (a short allow-list), so a new table without seed rows fails the always-on xtask test
  `the_seed_of_this_build_is_in_the_tree` or a new store-pg test. Also consider letting
  `extract` add an optional in-tree file (`tests/fixtures/upgrade-extra/<tag>.sql`) after the
  tag's seed, so a gap found later can still be closed.

### REL-4 (Medium) The upgrade test has never failed and never ran with a pending migration

- Where: `src/rust/crates/store-pg/tests/upgrade.rs:117-152`, progress log item 6.
- Problem: its only run used this build as the "previous release": `store_pg::migrate` had
  nothing to apply, so the three things it is for (a changed released migration is refused,
  rows survive a migration that rewrites them, an upgraded schema equals a fresh one) were
  never exercised, neither red nor with real work. The CI job has never run on a runner
  either (its `git fetch --tags`, service and cache setup are unproven).
- Failure: a mistake in the harness (for example the migrator of `built_from` and the
  embedded one agreeing by construction, or the row comparison matching nothing) stays
  invisible until the first release after v1, when it is the only gate before the owner's
  data.
- Fix (throwaway, nothing committed but the result in the progress log): on a scratch branch
  tag the tree `v0.0.1`, then (a) change one byte of the baseline and run
  `cargo xtask upgrade-test v0.0.1`: must fail with sqlx's "previously applied but has been
  modified"; (b) restore it, add a scratch `0002` with an `ALTER TABLE ... ADD COLUMN ... NOT
  NULL DEFAULT` and a backfill `UPDATE`, regenerate schema.txt: must be green; (c) make the
  scratch `0002` delete rows of a seeded table: must fail on "rows of <table>"; (d) make the
  fresh schema differ (edit the scratch 0002 after "release"): must fail. Delete the tag and
  branch. Sqlx behaviour assumed here (checksum validation in `Migrator::run`, no
  `ignore_missing`): `src/rust/crates/store-pg/src/lib.rs:22,137` use the defaults, which is
  right.

## May defer

### REL-5 (Medium) A changed released migration is found by machine only after the tag is public

- Where: `.gitlab-ci.yml:120-134` (rule: release tags only), `docs/HANDOFF.md:169-177`
  (steps 4 and 5 are manual), `tests/upgrade.rs` `a_fresh_database_has_the_stored_schema`.
- Problem: the brief's rule "no migration may change after it is in a tag" has no always-on
  guard. The stored-schema test only sees changes of the resulting schema; a comment or
  whitespace edit of `0001_v1_baseline.sql` changes its sqlx checksum and nothing else. The
  job that would see it runs on the tag pipeline, after the push.
- Failure: a later package tidies a comment in the baseline. All of CI is green on main. The
  checklist steps 4-5 are skipped or run on another commit. The tag is pushed, the tag job
  fails (too late), and an operator who upgrades gets "migration 1 was previously applied but
  has been modified": the server refuses to start with the old containers already replaced.
- Fix: run the job on the default branch and merge requests as well
  (`rules: - if: $CI_COMMIT_TAG =~ ... - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH -
  if: $CI_MERGE_REQUEST_IID`); before v1 it skips in a second, after v1 every pipeline proves
  the upgrade from the newest release tag, and the job itself is exercised long before a
  release day. Cheaper still, in addition: a store-pg test that compares a stored SHA-384 per
  released migration file (`tests/fixtures/migrations.sha`, appended at each release) with the
  embedded `MIGRATOR` checksums; it needs no git history.

### REL-6 (Medium) "No data lost" is row counts only

- Where: `src/rust/crates/store-pg/tests/upgrade.rs:128-141` and `read_back` (`:155-190`).
- Problem: the test compares the number of rows per table before and after, and reads
  library, items, progress, lists, people and plays through the use-cases without comparing
  any value. A migration that empties or rewrites a column passes. Two of the 37 pre-v1
  migrations did exactly that kind of thing (`UPDATE stats_users SET zone = ''`,
  `UPDATE catalog_items SET provider_data_at = ...`), so it is a realistic shape.
- Failure: 0002 moves ratings to a new scale with a wrong expression, or drops
  `list_items.note` and recreates it; counts equal, read-back decodes, test green, the
  owner's ratings or notes are gone.
- Fix: assert the seeded values after the upgrade through the use-cases (rating 80, review
  "Tense.", list note "signed", score 61 and the winner, the finished events, the year of an
  item), and for every table and column that exists before and after compare a content hash
  (`md5(string_agg(t::text, ',' ORDER BY ...))` over the common columns), with an explicit
  list in the test for columns a release rewrites on purpose. Note the reverse case too: a
  migration that legitimately clears a cache table (stats_dirty and the statistics cache get
  rows from the triggers while the seed loads) fails "rows of <table>" today; the same
  explicit list solves it.

### REL-7 (Medium) Staging with the S3 backend runs a newer build on the production bucket

- Where: `docs/modules/ROOT/pages/staging.adoc:29` (`cp config/.env`), `:3-9` ("Production
  is not stopped or changed at any point of this page").
- Problem: the copy takes the production settings. With `APP__STORAGE__BACKEND=s3` that is
  the production bucket and keys: the staging web process of the NEW version reads and
  writes there (uploads of an import tried on the copy, cover fetches on a page view, `cli
  storage` commands). `operations.adoc` "Upgrade" step 4 itself says two builds can disagree
  about the blob store. The restore rehearsal has the same property, but with the same build.
- Failure: an operator with S3 tries an import on the copy or runs `dcs exec app cli storage
  evict`; objects of production are written or removed by a build production does not run
  yet, against the storage index of a database copy.
- Fix: one paragraph in "Prepare": with the S3 backend set `APP__STORAGE__S3__BUCKET` (or the
  backend to `fs`) in the copied `config/.env` before the first `dcs up`, and do not run
  `cli storage evict` on the copy. The fs default is safe (own `app-data` volume per project).

### REL-8 (Low) The staging page restores from paths that are in the other checkout

- Where: `docs/modules/ROOT/pages/staging.adoc:47,55-58`; `operations.adoc` Restore steps 2-4
  (`< backup/secrets.key`, `< backup/app-<date>.dump`, `< backup/up-<date>.tar`).
- Problem: "From here on, work in the staging checkout", then "steps 1 to 4 of Restore with
  `dcs`". The backup of "Prepare" step 2 was written in the production checkout; in the
  worktree `backup/` does not exist.
- Failure: `bash: backup/secrets.key: No such file or directory` at the second command;
  harmless, but the page promised commands that work as written.
- Fix: say "the backup files are in the production checkout: use `../tracker/backup/...`", or
  set `B=../tracker/backup` once and name it.

### REL-9 (Low) What `store_pg::schema::describe` does not see

- Where: `src/rust/crates/store-pg/src/schema.rs:24-116`.
- For the squash this does not matter: the lane compared `pg_dump --schema-only` and the
  data once (LV-4), and the old chain has none of the things below (checked: no GRANT, no
  owner change, no storage parameter, no collation, no RLS, no comment, no view, no domain;
  one extension; the only data row is `instance_settings (id) = true`, all three `UPDATE`s
  are backfills of empty tables). It matters from now on, because `describe` is the only
  comparison of "upgraded equals fresh" (upgrade test) and of the stored schema:
  - row contents: only counts (`row_counts`); a migration that sets a value in the
    `instance_settings` row (an `UPDATE`, not a column default) is invisible to schema.txt;
  - sequences: no `OWNED BY`, no current value;
  - columns: no collation, no storage parameters (`reloptions`), no UNLOGGED flag;
  - row level security: policies are listed without their roles, and the table flags
    `relrowsecurity` / `relforcerowsecurity` are not (a policy without the flag does nothing);
  - comments on anything but tables and columns (functions, types, constraints);
  - domains: listed by name only (`types`), their constraints are excluded by
    `conrelid <> 0`;
  - not covered at all: aggregates (`prokind = 'a'`), rules, event triggers, extended
    statistics, operators and casts, schemas other than `public`, extension versions,
    privileges and default privileges, ownership, invalid indexes (`indisvalid`).
- Two stability points of the fixture: (1) the sections with `ORDER BY 1` sort text with the
  database's default collation; the fixture is in C order (the dev cluster is created with
  `--no-locale`, the alpine image sorts bytewise). A test database on a glibc `en_US.UTF-8`
  cluster orders `catalog_episode_seasons` and `catalog_episodes` the other way round and the
  test fails with a confusing first difference. Add `COLLATE "C"` to those `ORDER BY`s.
  (2) The text is the output of Postgres 18 (`pg_get_*def`, and the NOT NULL rows in
  `pg_constraint` exist from 18 on); the fixture must be regenerated with a new major
  version. One sentence in the test's header is enough.
- Fix: add the cheap ones now (owned-by, RLS flags, reloptions, collation, `COLLATE "C"`),
  and a content hash of `instance_settings` to the snapshot.

### REL-10 (Low) Small inconsistencies in the release pages

- `docs/modules/ROOT/pages/releases/template.adoc:18`: "`git pull` to the tag"; operations
  "Upgrade" step 2 and staging say `git fetch --tags`, `git checkout vX.Y.Z`. `git pull` on a
  detached HEAD (which is where the previous upgrade left the checkout) fails. Use the words
  of operations.adoc.
- `docs/modules/ROOT/pages/operations.adoc:102`: "with a build that has the admin tool in the
  image" is a pre-v1 distinction (every release has it); item 1 of the brief removes such
  sentences.
- `docs/HANDOFF.md` 4.8 and v1-plan say tag `v1`; LV-7 and the checklist say `v1.0.0`. Both
  pass `is_release_tag`, but the release notes page, the changelog heading and release.toml
  are named for `v1.0.0`. When the orchestrator takes LV-7 into DECISIONS.md, 4.8 should say
  `v1.0.0`.
- `docs/HANDOFF.md:181-182` (step 7, website): the comment in `release.toml` says book,
  board_game and manual_entry must be switched from "coming" to "shipped" in features.toml
  and home.toml in the same change (LV-8); the checklist step does not mention it, so the v1
  site would call three shipped things "Coming". Add it to step 7 for v1.
- `releases/template.adoc` is a published page (reachable from the index) with angle-bracket
  placeholders; acceptable, or mark it `:page-noindex:` / keep it under `partials/`.

### REL-11 (Low) The version bump leaves Cargo.lock behind

- Where: `docs/HANDOFF.md:167-168` ("`Cargo.lock` follows with the next build").
- Problem: the tag commit then carries a lock file with the workspace members at the old
  version; every build from the tag rewrites it. Nothing uses `--locked` today (Dockerfile,
  CI, xtask: checked), so nothing breaks, but the tree of a tag is dirty after any local
  build, and the day `--locked` is added to the image build (a common hardening step) the tag
  cannot be built.
- Fix: step 3: "run `cargo check` and commit `Cargo.lock` in the same commit".

### REL-12 (Low) Test-database cleanup: sound for the present callers, three edges

- Where: `src/rust/crates/store-pg/src/testing.rs:100-176`.
- Checked: no test binary shares a database between tests (no `OnceLock`/`LazyLock`/static
  pool around `fresh_db` in any `tests/` directory or testkit), the `multi_thread` tests
  create their database in the body (which `block_on` runs on the test's own thread), no
  non-test binary calls the module. So `DROP ... WITH (FORCE)` at the end of a thread cannot
  hit a database another test uses, and parallel tests are independent (one registry per
  thread). The destructor does not panic (errors are printed), so it cannot turn a reported
  success into "panicked after reporting success".
- Edges, none hit today: (1) a database created inside `spawn_blocking` belongs to a
  blocking-pool thread that ends after 10 s idle, so a longer test would lose its database
  mid-run with FORCE; the module comment names the `tokio::spawn` case but not this one, and
  this one fails in a confusing way ("database ... does not exist" / terminated connection).
  (2) Any panic on the test's thread marks it failed, also one that is caught (a
  `catch_unwind`, an expected panic of a helper): the database is kept; harmless. (3) Kept
  databases of failing runs still add up and nothing sweeps them; a `cargo xtask` command or
  a note in HANDOFF ("drop `test_%` older than a day") would finish addendum 4.
- Fix: a sentence for (1) in the module comment; optional sweep for (3).

### REL-13 (Low) `cli migrate --dry-run` says "nothing to do" where the start will refuse

- Where: `src/rust/crates/store-pg/src/lib.rs:102-124` (`pending_migrations` compares
  versions only), used by staging.adoc "Upgrade the copy" step 2 and operations "Upgrade"
  step 3.
- Problem: on a database whose applied migrations do not match this build (a pre-v1 test
  database: versions 1..37 with other checksums; later, a changed released migration) the dry
  run lists nothing or only the new ones, and `dc up -d` then fails in `MIGRATOR.run` with
  a checksum or missing-version error. It fails closed, so no data is at risk; the dry run
  just gives the wrong comfort at the one point the docs call "see what it will do".
- Fix: let the dry run compare the checksums of the applied versions with the embedded ones
  (read-only: `SELECT version, checksum FROM _sqlx_migrations`) and report a mismatch or an
  unknown version as an error with the sentence from the release notes ("a database made by
  a build from before 1.0.0 cannot be upgraded").

### REL-14 (Low) Tests that were never seen red

- `xtask/tests/ops.rs` `the_release_process_is_written_down`: written after the pages
  (LV-6 says so). It checks for strings; REL-1 and REL-2 passed it. Acceptable as a
  presence guard; do not read it as a check of the procedure.
- `src/web/tests/boardgames.spec.ts` (new test) and the added line in `contact.spec.ts`:
  passed on first run, they describe existing behaviour (LV-9). The assertions are specific
  (About selected, "No details yet.", no `item-links`, no Wikidata attribution, "One play"),
  so they can fail; a one-minute mutation (assert the Wikidata credit is present) would
  prove the selectors match what they claim (`getByTestId("attribution")` with `toHaveCount(0)`
  passes trivially if the test id is wrong).
- `xtask/tests/whitelabel.rs` `former_env_prefix_not_used`: not red on the tree, but its
  unit test proves detection; fine.
- `a_fresh_database_has_the_stored_schema`: green from its first run by construction; the
  same comparison was seen red in the ignored chain test (negative control without 0037);
  fine.
- `the_previous_release_upgrades_to_this_one` and the CI job: see REL-4.

### REL-15 (Low) The prefix guard reads git-ignored files

- Where: `src/rust/xtask/src/whitelabel.rs` `former_env_prefix_offenders` (walks the
  checkout, skips `SKIPPED_DIRS` and the record directories only).
- Problem: it also reads `config/.env` and `owner-inbox/` when they exist. Today neither
  holds a hit (checked in the main checkout: no local env file, no hit outside tracked
  files that this package renames). If the owner later keeps the old names in a local
  `config/.env`, `cargo test -p xtask` fails on a file that is not in the repository; the
  message names file and line, never the value, so no secret is printed.
- Fix: none needed; if it annoys, skip `config/.env` and `owner-inbox/` like the other
  local state.

## Checked and found in order

- Baseline vs old chain (by reading, not by rebuilding: this host has no `psql`/`pg_dump`
  on the path and the review is read-only, so the lane's pg_dump comparison was not
  repeated): the 37 files contain one extension (pg_trgm), 13 functions (5 of them replaced
  later by 0029/0036), 12 triggers, 2 explicit sequences plus one identity column, one
  deferred foreign key, one data row; the baseline has `CREATE EXTENSION IF NOT EXISTS
  pg_trgm`, 13 functions, the triggers incl. the eight statistics triggers and the two of
  0036, `outbox_id_seq` with `OWNED BY` and its default, `stats_dirty_gen`, the identity of
  `catalog_match_log`, `users_self_companion_fk ... DEFERRABLE INITIALLY DEFERRED`, and
  `INSERT INTO instance_settings (id) VALUES (true)` at the end. The fixture has 12 trigger
  lines, 3 sequences, `instance_settings 1`. No GRANT/REVOKE/OWNER/COMMENT in the chain, so
  `--no-owner --no-privileges` dropped nothing. The three `UPDATE`s of the chain (0015, 0028,
  0036) are backfills and write nothing into an empty database.
- `SET LOCAL check_function_bodies = false` is right for a migration sqlx runs in a
  transaction, and needed (`library_activity` is `LANGUAGE sql` and names a later table).
- `crates/store-pg/build.rs` re-runs on a change of `migrations/`, so the lane's test runs
  did use the baseline (also visible from the two tests that broke on "one migration").
- A pre-v1 database cannot be started by this build (version 1 has another checksum,
  versions 2..37 are missing): sqlx refuses, nothing is half-migrated. Stated in the release
  notes and in the progress log. Local throwaway databases must be recreated.
- Shim removal: `RawStore` still creates `fs_root` on a fresh volume; the Dockerfile keeps
  `APP__STORAGE__FS_ROOT=/data/blobs` and `APP__SECRETS__KEY_FILE=/data/secrets.key`; the
  compose file is unchanged apart from a comment; nothing a fresh install needs went away.
  No reference to a removed migration file name is left in code, docs, CI or tests.
- Prefix rename: no occurrence of the former prefix (upper or lower case, word start) outside
  the record directories, in tracked or untracked files; CI never used it; the website
  content has none. The docs name variable names only, no values. `SOURCE_` does not collide
  with `SOURCE_DATE_EPOCH` (the fallback needs the `_API_KEY` suffix).
- `xtask upgrade-test`: `is_release_tag` and the CI regex agree; package tags (`v1-polish`)
  and `m<NN>` are excluded; `git show <tag>:<file>` writes the blob bytes unchanged, so the
  checksums recorded by `Migrator::new(dir)` are the released ones and a changed or removed
  released migration makes `store_pg::migrate` fail; `GIT_DEPTH: "0"` and
  `git fetch --tags --force` are set; the tag of HEAD itself is skipped, also when annotated.
- Staging: project `tracker-staging`, port 3002, image `localhost/tracker-staging-app:local`
  do not collide with production (`tracker`, 3000) or the rehearsal (`tracker-restore`,
  3001, `localhost/tracker-restore-app:local`); `dcs down -v` is scoped by `-p`; only the app
  publishes a host port. The xrefs and section anchors used by the new pages exist.
- CHANGELOG.md: Keep a Changelog 1.1.0 form with the URL, `Unreleased` on top, the v1 entry
  agrees with v1-scope.adoc in the points sampled.
- `.sqlx` is untouched (no query changed); `release.toml` stays empty with the prepared list
  as a comment.
