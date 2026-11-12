# Review: package v1-ops (2026-11-12)

Scope: the uncommitted working tree against HEAD for package 1 "v1-ops"
(docs/fable/round2/v1-status.md section 5; next-packages.md "ops1"; HANDOFF 4.8, OPS-3+4
option a; round-1 findings OPS-1/2/3/4/7/8/9/12 and SEC-6). DEC-618..625, progress log
docs/fable/round2/v1-ops-progress.md. docs/fable/round2/v1-plan.md and DEC-640..642 were
ignored (another package).

Method: reading only. No cargo, npm, Playwright, docker or podman command was run, nothing
was edited besides this file. Everything about containers below is derived from the files
and from the documentation cited; none of it was executed.

Counts: 1 high, 4 medium, 5 low. Must fix before commit: V1OPS-1, V1OPS-2, V1OPS-3.

## Answers to the review questions (short)

- Fresh install with `docker compose --env-file config/.env -f src/docker/compose.yml up -d
  --build`: no defect found by reading. Empty values of a copied `.env.example` are
  harmless: compose hands them to the container through `env_file` (they override the
  image's `ENV`), `config::apply_env` skips empty values (crates/config/src/lib.rs:573-576),
  and the relative default `data/secrets.key` resolves to `/data/secrets.key` because of
  `WORKDIR /` (Dockerfile:60-61). `environment:` wins over `env_file:`
  (https://docs.docker.com/compose/how-tos/environment-variables/envvars-precedence/), so
  the empty `APP__DATABASE__URL=` of the env file does not blank the compose value.
- The owner's existing instance after `git pull`: does NOT come up without steps the
  runbook does not name. See V1OPS-1.
- Blob move (`storage.fs_legacy_root`): a restart in the middle and two processes at once
  are handled (one atomic rename per directory, a missing source with an existing target is
  accepted, backend.rs:53-60). A read-only volume fails at `create_dir_all(/data/blobs)`
  with the path in the message, before anything moves; nothing is half done. Rootless
  podman: the rename is inside one volume and inside one user namespace, no ownership
  change is involved; no defect found. Gaps: the directory `orig` (V1OPS-3) and old and new
  images on one volume (V1OPS-5).
- Owner ruling "move the secrets key out of the blob directory": met in effect. The two
  failure modes of OPS-3 (syncing the blob root ships the key; clearing the blob root
  deletes the key) are both gone: the blob root is `/data/blobs`, the key is its sibling.
  This is also the literal recommendation of ops-readiness.md (table row 5, "`/data/blobs`").
  What it does not give: a separate volume. Whoever backs up, syncs or removes the whole
  `app-data` volume still handles key and blobs together; the runbook backs the key up by
  itself, so this is acceptable for v1. DEC-619 states the trade-off honestly.
- Backup and restore: the order (services, key, database, uploads, start, reindex, check)
  is right and the commands work with the required variables (pg_dump and pg_restore run
  inside the postgres container over the local socket, no password needed). Gaps: V1OPS-2
  (no way to rehearse beside the live instance), V1OPS-4 (way back from this release),
  V1OPS-6, V1OPS-10.
- `cli`: built with the server's feature set (`server/all-connectors`, defaults are empty,
  apps/server/Cargo.toml:57-65). `cli help` returns before the configuration is loaded
  (apps/cli/src/main.rs:237-240), so the `RUN` self-check needs no database. `dc exec app
  cli reindex --clear` inherits the container's environment (`APP__SEARCH__MEILI_URL`,
  master key from compose.yml:27-28; the search backend defaults to Meilisearch,
  crates/config/src/lib.rs:162) and reaches Meilisearch; plain `cli reindex` only writes
  the queue in Postgres. No defect found.
- 304 on `/pkg`: the comparison is byte equality of `If-Modified-Since` with the answer's
  `Last-Modified`, only for a `200`; a `206` (Range) and a `404` pass through; HEAD gets the
  same answer as GET; an existing `Cache-Control` is kept; the layer covers the fallback
  file handler. `Last-Modified` is a weak validator, which is sufficient for a 304 on GET
  (RFC 9110, 13.1.3). The handler emits no `ETag`, so `If-None-Match` precedence does not
  arise. There are no content-hashed files yet (hash-files stays with sec1) and no service
  worker in the tree. No defect found. One dependency to keep in mind: the scheme relies on
  the files' mtime changing with every image build; a reproducible build that zeroes
  timestamps would make every release answer 304. The CI kaniko call
  (.gitlab-ci.yml:170) does not pass `--reproducible`; do not add it without hashed names.
- Banner: admin only, fresh proof, audit row with key version and proof method in the same
  transaction, idempotent, returns for a new active version, en and de present, not shown
  for the ephemeral key. No defect found in the logic. One accessibility note: V1OPS-8.
- Migration 0034: one nullable column, additive. Not reversible by starting the older
  build (as for every migration here); the manual reverse is missing from the note
  (part of V1OPS-4).
- Rules: ASCII holds in every new and changed file except de.ftl (allowed); no brand name
  outside config/brand and docs; no attribution text. The two new files under
  `src/rust/.sqlx/` and the other untracked files must be part of the commit: the image
  build sets `SQLX_OFFLINE=true` and CI builds from git.

## Must fix before commit

### V1OPS-1 (high) The upgrade of an existing instance fails; the runbook does not say what to do

Files: docs/modules/ROOT/pages/operations.adoc:42-62 and :92-95;
src/docker/compose.yml:26, :34, :82; config/.env.example:6-8;
docs/modules/ROOT/pages/install.adoc:33-37.

Problem: three things change for an instance that was started with the compose file before
this package, and the Upgrade section and the 0034 upgrade note name none of them.

1. `POSTGRES_PASSWORD` had the default `app` (old compose.yml, `${POSTGRES_PASSWORD:-app}`).
   An instance started without a `config/.env` has a database whose user `app` has the
   password `app`. The variable is now required, and `.env.example` and install.adoc tell
   the reader to generate one (`openssl rand -hex 24`). The postgres image applies
   `POSTGRES_PASSWORD` only when it initializes an empty data directory
   (https://hub.docker.com/_/postgres , "Initialization scripts" and the note under
   `POSTGRES_PASSWORD`: an existing database is left untouched).
2. The port moved from `3000:3000` (all interfaces) to `127.0.0.1:3000`.
3. Every `dc` command needs `config/.env` to exist (`--env-file` of a missing file is an
   error), also step 1 "take the backup first", which runs before `git pull`. And the
   documented upload backup (`tar -C /data/blobs ... up`, operations.adoc:151) does not
   work before the upgrade: the old image has the uploads in `/data/up`.

Failure scenario (the acceptance run, step "upgrade the home instance"): the owner pulls,
copies `.env.example`, sets a fresh `POSTGRES_PASSWORD` as the file says, runs `dc up -d`.
Postgres starts on the old volume and keeps the password `app`; the app container exits
with "password authentication failed for user app" and restarts in a loop; `dc ps` never
shows it healthy. If the owner's reverse proxy runs on another machine, a second failure
follows after the first is solved: the proxy gets "connection refused" because the port is
now bound to loopback only.

Suggested fix: a block "Coming from a build before 0034" at the top of Upgrade (or in the
0034 row):

- create `config/.env` before step 1; set `POSTGRES_PASSWORD` to the password the database
  already has (`app` if it was never set), or change it first with
  `dc exec postgres psql -U app -c "ALTER USER app PASSWORD '...'"` and then put the same
  value into `config/.env`;
- `MEILI_MASTER_KEY` may be new (the scoped key is looked up with the master key at every
  start, crates/search/src/meili.rs:165-171), say so;
- set `APP_PUBLISH_ADDR` if the proxy is not on the same host;
- for the pre-upgrade backup use `tar -C /data -cf - up`.

Also add one sentence to the `POSTGRES_PASSWORD` rows of `.env.example` and install.adoc:
"an existing database keeps its password".

### V1OPS-2 (medium) The runbook offers no restore rehearsal beside the live instance

Files: docs/modules/ROOT/pages/operations.adoc:159-161 and :167-178;
src/docker/compose.yml:12 (`name: tracker`) and :34 (host port 3000 is fixed).

Problem: the runbook asks for "one restore into an empty instance after the installation"
and the acceptance run has "one backup and one restore into an empty database by the
runbook". The only two ways the Restore section names are a new host or "after `dc down`
and removing the volumes".

Failure scenario: the owner has one host. Following the text, the live volumes are removed
in order to test a backup that has never been restored before. If a step of the untested
procedure fails (none of the commands was ever run, DEC-625), the home instance's data is
gone. The safe variant, a second compose project on the same host, is not described and
does not work as written: the project name is fixed in the file and host port 3000 is
taken by the live instance.

Suggested fix: add the rehearsal variant to Restore, for example
`dcr() { APP_PUBLISH_ADDR=127.0.0.2 docker compose -p tracker-restore --env-file config/.env -f src/docker/compose.yml "$@"; }`
(`-p` overrides the top-level `name`,
https://docs.docker.com/compose/how-tos/project-name/ ; volumes and the network get the
project prefix), run steps 1 to 7 with `dcr`, check on `http://127.0.0.2:3000`, then
`dcr down -v`. Alternatively make the host port a variable (`APP_PUBLISH_PORT`). State
explicitly: never remove the live volumes for a first rehearsal.

### V1OPS-3 (medium) The blob move leaves the directory `orig` behind

File: src/rust/crates/assets/src/backend.rs:19 (`TOP_LEVEL = ["up", "img", "http"]`);
writers of the fourth prefix: src/rust/crates/assets/src/service.rs:389 and :547
(`orig/<hash>`, written when `storage.keep_originals = true`).

Problem: the store has four top-level prefixes, the move knows three. The docs repeat the
three (operations.adoc:223, configuration.adoc, Dockerfile:59).

Failure scenario: an instance with `APP__STORAGE__KEEP_ORIGINALS=true` upgrades. `up`,
`img`, `http` move to `/data/blobs`, `/data/orig` stays. The storage index still lists the
`orig/...` objects, the store no longer finds them: the kept originals (the reason the
setting exists) are unreadable, their bytes stay on disk outside the budget and are never
evicted, and the reported usage counts objects that are not there. Later originals are
written to `/data/blobs/orig`; from then on a corrected move would find the target and
leave the old directory in place for good, so this cannot be repaired cleanly in a later
release.

Suggested fix: add `"orig"` to `TOP_LEVEL`, extend the unit test with one `orig/h` object,
and name four directories in the three doc places. Better still, derive the list from one
constant next to the key builders so a fifth prefix cannot be forgotten.

## May defer

### V1OPS-4 (medium) "Going back" from this release is incomplete

File: docs/modules/ROOT/pages/operations.adoc:61-62 and :92-95.

Problem and scenarios:

1. After the first start of the new image the blobs are in `/data/blobs`. "Going back" says
   restore the dump, check out and build the old version, `dc up -d`. The old image reads
   `/data/up`, `/data/img`, `/data/http`. Result: stored import files are not found, every
   cached image is fetched again, and `/data/blobs` stays on the volume uncounted. One line
   is missing: move the directories back before starting the old build
   (`mv /data/blobs/* /data/` in a `dc run --entrypoint sh`).
2. The 0034 row gives no manual reverse, unlike the 0032 row. For someone who has no dump:
   `ALTER TABLE secrets_keys DROP COLUMN backup_confirmed_at; DELETE FROM _sqlx_migrations
   WHERE version = 34;`.
3. "Restore the dump (Restore, step 3)" runs `pg_restore --clean --if-exists` on the
   migrated database. `--clean` drops only the objects that are in the dump
   (https://www.postgresql.org/docs/current/app-pgrestore.html , option `--clean`). A table
   that a newer migration created stays. Harmless for 0034 (a column of a table the dump
   contains), but with the next release that adds a table: go back, later upgrade again,
   the migration's `CREATE TABLE` fails with "already exists" and the server does not
   start. Fix: for the way back, empty the database first
   (`dc exec postgres psql -U app -d app -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'`)
   and then `pg_restore` without `--clean`.

### V1OPS-5 (medium) Old and new image on the same volume see different blob roots

Files: src/docker/compose.yml:50-74 (commented `worker` with its own `build:`);
docs/modules/ROOT/pages/operations.adoc:46 (`dc build app`);
src/rust/crates/assets/src/backend.rs:84-86 (every `RawStore::open` moves, also from the CLI).

Scenario 1: someone uncomments the `worker` service. It has its own `build:`, so compose
tags a second image for it. Upgrade step 2 builds only `app`. After `dc up -d` the web
process runs the new image (blob root `/data/blobs`, moves the directories), the worker
still runs the old one (blob root `/data`): the worker stores fetched covers under
`/data/img`, the web process looks under `/data/blobs/img`, finds nothing and queues the
fetch again. At the next start both `/data/img` and `/data/blobs/img` exist and the move
logs "left in place" forever. Fix: give the commented worker `image:` of the app build
(or write `dc build` without a service name in step 2).

Scenario 2: a CLI command of the new image that opens the store (`storage report`,
`storage evict`) run through `dc run` while the old container still serves, moves the
directories under the running old server, with the same split as above. `storage report
--dry-run` promises "nothing is changed". Fix: one sentence in Upgrade ("before `dc up -d`
run only `migrate --dry-run` from the new image"), or let only the server role perform the
move.

### V1OPS-6 (low) The backup table places photos and avatars under `up/`; the code puts image uploads under `img/`

Files: docs/modules/ROOT/pages/operations.adoc:115-121 and :150;
src/rust/crates/assets/src/service.rs:366-374 and :405-420.

Problem: `AssetService::upload` stores its variants with `object_key` (`img/<hash>/...`,
class `Upload`). Only import files are written under `up/` (crates/importers/src/runner.rs:84).
The table says "uploads (photos, avatars, import files): `/data/blobs/up`: yes" and
"`img`: no, regenerated on demand". Today `upload` has no caller outside tests, so nothing
is lost in v1. Scenario: the first release that wires an image upload: a restore by this
runbook loses every uploaded picture (no source URL to fetch from). Fix now: write "import
files" in the table; and put a note at `AssetService::upload` that the backup rule must
change (or key uploads under `up/`) when it gets a caller.

### V1OPS-7 (low) nginx snippet: `http2 on` and `$host`

File: docs/modules/ROOT/pages/reverse-proxy.adoc:83 and :98, :107.

1. `http2 on;` exists since nginx 1.25.1
   (https://nginx.org/en/docs/http/ngx_http_v2_module.html#http2). Scenario: Debian 12
   ships nginx 1.22; `nginx -t` fails with "unknown directive http2" and the reload is
   refused. Fix: a comment "nginx before 1.25.1: `listen 443 ssl http2;` instead".
2. The page says "pass the `Host` header unchanged", the snippet sets `$host`, which drops
   the port (https://nginx.org/en/docs/http/ngx_http_core_module.html#var_host). Scenario:
   the instance is reached as `https://tracker.example.org:8443` and
   `APP__SERVER__PUBLIC_URL` is not yet right (the first-run mistake of OPS-2): the
   fallback comparison of `Origin` with `Host` (apps/server/src/csrf.rs:37-40) fails as
   well and every form answers 403. Fix: `proxy_set_header Host $http_host;`.

### V1OPS-8 (low) Keyboard focus is lost after confirming the banner

File: src/rust/crates/web/src/admin/overview.rs:190-213.

Problem: the confirmation refetches the overview, the banner with the focused button is
removed, nothing takes the focus. Scenario: a keyboard or screen-reader user presses "I
have backed up the key", passes the re-auth dialog, the dialog returns focus to a button
that no longer exists; focus falls back to the document, the next Tab starts at the top of
the page (WCAG 2.4.3 Focus Order). The success text is rendered above the removed block,
but it is not where the focus is. Fix: give the outcome block `tabindex="-1"` and focus it
after a successful confirmation. The hand check in the progress log covered axe and
reflow, not focus.

### V1OPS-9 (low) A 140-line test function under a module-wide `too_many_lines` allow

File: src/rust/apps/server/tests/it/ops.rs:8 and :127-266.

Problem: rules.md asks for functions of at most 100 lines; the file switches the lint off
for the whole module. There is precedent (admin.rs:9, oidc.rs:11), so this is a repeat,
not a new kind of exception. Scenario: the one test covers REST, two SSR pages, the gate,
the audit, idempotence, the server function and rotation; the first failing assertion
hides the state of the remaining six. Fix: split at the comment lines (reminder shown;
gate and audit; idempotence and rotation) with the shared `start()`.

### V1OPS-10 (low) Restore step 1 does not wait for Postgres

File: docs/modules/ROOT/pages/operations.adoc:176-177 and :191.

Problem: `dc up -d postgres meilisearch` returns as soon as the containers are created.
On a fresh volume the postgres image runs initdb first and restarts the server once.
Scenario: on a slow disk step 3 (`pg_restore`) runs while the database is still starting
and fails with "the database system is starting up" or a refused connection; because
`dc exec` needs a running container this looks like a broken dump. Fix:
`dc up -d --wait postgres meilisearch` (waits for the healthchecks,
https://docs.docker.com/reference/cli/docker/compose/up/ ).

## Checked, no finding

- Compose: `${VAR:?msg}` fails on unset and on empty
  (https://docs.docker.com/reference/compose-file/interpolation/ ); `:-` falls back on
  empty too, so the empty `APP_PUBLISH_ADDR=` and `CARGO_BUILD_JOBS=` of a copied example
  are fine. Interpolation covers the whole file, so moving RustFS into its own file was the
  safe choice. `env_file` with `required: false` is in the specification
  (https://docs.docker.com/reference/compose-file/services/#env_file ). `stop_grace_period`,
  `start_period`, `restart` are plain.
- Podman override: the published port moved with the same loopback default; the required
  password is repeated; S3 fragments moved to their own override.
- Dockerfile: `cargo build -p cli -p server --bin cli --no-default-features --features
  server/all-connectors` is a valid selection under resolver 3; `VOLUME /data` after the
  `chown`; `cli help` runs as root before `USER app` and touches nothing. Whether the
  second cargo call reuses the server library's artefacts cannot be told without a build;
  if not, the cost is build time, not a failure.
- Runbook commands: `docker compose exec -T` and `run -T` keep stdin, `run --entrypoint`
  with the arguments after the service name, `kill -s HUP`; `pg_dump -Fc`, `pg_restore
  --list` from stdin, `--no-owner`; `tar` is part of the Debian base image; the key is
  written before the first start and owned by `app`.
- Caddy snippet: `reverse_proxy` keeps `Host`, adds `X-Forwarded-*` and flushes event
  streams (https://caddyserver.com/docs/caddyfile/directives/reverse_proxy ); `encode` and
  `header` blocks are well formed.
- `pending_migrations` is read-only (a missing bookkeeping table is treated as "never
  migrated"); the store-pg test deletes rows and drops the table to prove it and can fail.
- The xtask checks are text checks and say so; each has a counter-example test, so they
  can fail.
- `exposure_warnings`: quiet for https and for loopback; the compose default (public URL
  unset, bind 0.0.0.0) logs one warning at every start until `PUBLIC_URL` is set, which is
  the intent of SEC-6.
- Out of scope: nothing beyond the package found in the diff (REPORT.adoc one line follows
  the S3 file move).

## Not covered by this review

- Nothing was built or started; the progress log itself lists `cargo xtask ci`, the whole
  server `it` binary, the `sso` browser project, an image build and every runbook command
  as not run.
- `podman compose` with the podman-compose provider (support for `!reset` and the long
  `env_file` form) was not checked against its documentation.
