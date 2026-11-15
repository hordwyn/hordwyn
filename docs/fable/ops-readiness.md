STATUS: COMPLETE (2026-11-09)

# kithkeep operations-readiness review

Reviewer: independent (did not write any of the code). Date: 2026-11-09. Scope: branch
`main` (= tag `ux4` plus commit f1eeefa, the Docker build fix). Read-only review of the
Dockerfile and compose files, CI, shell scripts, the 31 migrations, the server and CLI entry
points, the configuration and the operator-facing docs. Nothing was compiled or run; where a
statement rests on library behaviour outside the repo (sqlx, cargo-leptos, BuildKit, kaniko)
it is marked "not executed" and should be checked against that tool's documentation.

Paths are relative to the repository root; Rust paths drop the `src/rust/` prefix. Line
numbers were read from the files on 2026-11-09.

Security findings are in `docs/fable/security-audit.md` (SEC-1 to SEC-12) and are referenced
here, not repeated. Findings of this review are numbered OPS-n.

## Contents

0. Context: the failing native podman build
1. Image build
2. First run
3. Upgrades
4. Backups and restore (runbook)
5. Observability
6. Resource needs and scaling
7. Documentation gaps for self-hosters
8. Prioritised plan
9. Findings index (OPS-n)

## 0. Context: the failing native podman build

The owner's rootless podman build on Ubuntu 26.04 (16 GB) was OOM-killed while compiling the
`web` crate. Commit f1eeefa split `cargo leptos build --release` into `--frontend-only` then
`--server-only` and added `ARG CARGO_BUILD_JOBS=2` (`src/docker/Dockerfile:22-29`). The
rebuild still failed later; the log tail has not been seen yet. What this review can say
without the log:

- The split removed the worst overlap (wasm fat-LTO rustc next to the native release rustc of
  `web`/`server`), but every step after it still runs inside one `RUN` (`Dockerfile:25-29`),
  so a failure anywhere in that chain discards the whole layer, including the 20-40 minutes
  of dependency compilation before it (section 1.2).
- Memory peaks that survive the split, in the order they run: (a) the wasm build of `web`
  with `lto = true`, `codegen-units = 1`, `opt-level = "z"` (`Cargo.toml:129-134`): fat LTO
  in a single rustc holds the whole program in memory; (b) `wasm-opt` on the output, which
  cargo-leptos runs for release builds (not executed; check the cargo-leptos 0.3.11 docs for
  the switch that disables it); (c) the native release rustc of `web` (ssr) and of `server`,
  which the owner measured at about 2.9 GB each in *debug* (`docs/HANDOFF.md:800-803`);
  release with 16 codegen units and LLVM optimisation is usually higher, not lower;
  (d) the final link of `server` (672 lock-file entries, `Cargo.lock`), where the system
  linker holds every object in memory at once.
- `CARGO_BUILD_JOBS` caps the number of rustc processes, not the memory of one rustc. With
  `-j2` two of the (c) compilations can still overlap (for example `web` ssr and a late
  dependency), and a `-j1` build is the only setting that guarantees one compiler at a time.
- The runtime stage copies `src/rust/crates/ui/icons/vendor`, `config/brand`, `config/certs`
  (`Dockerfile:35-40`); all exist in the checkout, so a missing-file `COPY` is not the later
  failure.
- Rootless podman puts no memory limit on the build container; the limit is the host. A
  16 GB desktop with a browser and an IDE open leaves perhaps 8-10 GB for the build, which
  is the range the Dockerfile comment itself names as the need (`Dockerfile:19-21`).

First thing to do with the log: find the last `Compiling`/`Finished` line and whether the
kill happened in `cargo leptos build --release --frontend-only` (then (a)/(b)) or in
`--server-only` (then (c)/(d)); `dmesg | grep -i oom` names the process. Section 1.6 has
the Dockerfile that makes either answer survivable.

## 1. Image build

### 1.1 What the build does today

`src/docker/Dockerfile`:

- :3 base `rust:1.99.1-slim-trixie`; :4-5 apt `curl ca-certificates pkg-config libssl-dev`.
- :7-8 extra CA certificates from `config/certs/` (operator-controlled, documented in
  `docs/modules/ROOT/pages/operations.adoc:134-140`).
- :10-11 `src/sh/ci-tools.sh leptos wasm-bindgen tailwind`: downloads cargo-leptos 0.3.11,
  wasm-bindgen 0.2.130 and tailwindcss 4.3.3 from GitHub releases by version, no checksum
  (`src/sh/ci-tools.sh:5-25`; SEC-8).
- :12 `COPY src/rust src/rust` copies the whole workspace (including `.sqlx`, 1.4 MB) before
  any cargo command runs.
- :14-16 Tailwind input, static files, `config/`.
- :18 `SQLX_OFFLINE=true`, so no database is needed at build time (good).
- :22-23 `ARG CARGO_BUILD_JOBS=2` -> `ENV`.
- :24 `rustup show active-toolchain || rustup toolchain install`: the image ships 1.99.1 and
  `rust-toolchain.toml:2` pins 1.99.1, so nothing is downloaded today; the day the pin moves
  ahead of the image, this line downloads a full toolchain inside the build.
- :25-29 one `RUN`: wasm target, frontend build, server build, copy to `/out`.
- :31-51 runtime `debian:trixie-slim` + `ca-certificates curl`, user `app` uid 10001, `/data`,
  `server` to `/usr/local/bin`, site to `/srv/site`, brand and vendored icons, `ENV` for
  Leptos and the app, `ENTRYPOINT ["/usr/local/bin/server"]`, `CMD ["--role=all"]`.

`.dockerignore` keeps `.git`, `target`, `node_modules`, `config/.env` and build output out of
the context (`.dockerignore:2-11`); `docs/` (2.5 MB) and `owner-inbox/` (480 KB) still go in
but are not copied into any layer, so they only cost context upload time.

### 1.2 Layer caching: the main weakness

Dependencies are not compiled in their own layer. Because `COPY src/rust src/rust`
(`Dockerfile:12`) precedes every `cargo` call, any change to any source file invalidates the
layer that compiles all 672 crates of the lock file, twice (native and wasm). Each rebuild
after a one-line change is a full cold build. Nothing in the file uses BuildKit cache mounts
(`RUN --mount=type=cache`), so the cargo registry and the target directory are also lost
between builds.

The standard fix is a dependency-only layer first: either `cargo-chef` (MIT OR Apache-2.0;
`cargo chef prepare` writes a recipe from the manifests, `cargo chef cook` compiles only the
dependencies, and the layer is reused as long as `Cargo.toml`/`Cargo.lock` do not change),
or the hand-rolled variant (copy manifests, create stub `src/lib.rs`/`main.rs` files, build,
then copy the real sources). With a 23-crate workspace the stubs are tedious and brittle;
cargo-chef is the better fit. The dependency layer must be cooked twice: once for the native
`server` build (`--release -p server --features all-connectors`) and once for the wasm
`web` build (`--profile wasm-release --target wasm32-unknown-unknown -p web --features
hydrate`), matching what cargo-leptos passes (`Cargo.toml:153-157`).

Expected effect: a cold build still compiles everything once; every later build with the
same lock file compiles only the workspace crates (115 k lines in `crates/`, 38 k of them in
`web` + `ui`), which is the part the owner measured at 2.9 GB per rustc. The memory peak does
not go down, but a failure in the last step no longer costs the dependency time, and a second
attempt with `CARGO_BUILD_JOBS=1` starts from the cached layer.

cargo-chef works with kaniko (plain layers), so the CI job (`.gitlab-ci.yml:161-170`) benefits
too once kaniko is given `--cache=true` and a cache repository; today it builds from scratch
on every pipeline and does not push (`:169`). BuildKit cache mounts, by contrast, are a
BuildKit/buildah feature and not a kaniko one (not executed; verify with the kaniko release
notes before relying on them in CI).

### 1.3 Memory and time

What can be said from the files:

| Step | Memory driver | Mitigation |
|---|---|---|
| dependency compile, native | up to `CARGO_BUILD_JOBS` rustc processes, mostly small | `-j2` is fine |
| dependency compile, wasm | same, but `wasm-release` applies LTO only to the final crate | `-j2` fine |
| `web` wasm, fat LTO, cgu 1 (`Cargo.toml:129-134`) | one rustc, whole program | `lto = "thin"` or `"fat"` only in CI; `opt-level = "s"` is a smaller swing |
| `wasm-opt -Oz` (cargo-leptos release) | one process, whole module | disable in the image build, run in CI only (switch name: check cargo-leptos docs) |
| `web` ssr + `server` native release | owner: ~2.9 GB each in debug; release higher | `-j1` for the server step only (`CARGO_BUILD_JOBS` is per step in the proposal below) |
| link of `server` | linker + 672 crates of objects | `strip = "symbols"` lowers output not peak; `lld` or `mold` lowers peak and time |

No `[profile.release]` is defined (`Cargo.toml:119-134` has only `dev` and `wasm-release`),
so the native release build uses cargo defaults: 16 codegen units, no LTO, no stripping,
`debug = false`. Adding `strip = "symbols"` costs nothing at build time and shrinks the
binary (section 1.5). Adding `lto = "thin"` would cut the binary further but raise memory;
not recommended until the build is reliable.

Time (estimate, not measured; 4 vCPU, `-j2`): cold dependency compile 20-40 minutes for both
targets, `web` wasm 10-20 minutes with fat LTO, `server` release 10-15 minutes, link 1-2
minutes. With the dependency layer cached, an incremental image build drops to the last
three. The GitLab `leptos-build` job (`.gitlab-ci.yml:153-159`) and the `docker-image` job
(`:161-170`) compile the same release artefacts twice per pipeline.

### 1.4 Reproducibility

Pinned: toolchain 1.99.1 (`rust-toolchain.toml:2`), tool versions (`src/sh/ci-tools.sh:5-8`),
base image tags, `Cargo.lock` committed. Not pinned or not enforced:

- `cargo leptos build` is not run with `--locked`, so a lock-file drift would be silently
  resolved inside the image (cargo-leptos forwards cargo flags; not executed).
- Base images by tag, not digest (`Dockerfile:3,31`); `rust:1.99.1-slim-trixie` is rebuilt by
  Docker Hub when Debian updates, which is desirable for security but makes two builds of the
  same commit differ.
- Tool tarballs without checksums (SEC-8).
- `apt-get install` without version pins (acceptable; the packages are not in the artefact
  except `curl` and `ca-certificates`).
- The `--server-only` build passes `bin-features = ["all-connectors"]` and
  `bin-default-features = false` from the metadata (`Cargo.toml:153-154`), so the image has
  every compiled-in connector; `gallery` is not included (good, it is dev-only).

### 1.5 Image size

Runtime layers: `debian:trixie-slim` plus `ca-certificates` and `curl`, the `server` binary,
`/srv/site` (wasm, JS glue, CSS, fonts under `config/brand/default/fonts`), the brand
directory (120 KB) and the vendored icons (340 KB, kept for attribution,
`Dockerfile:40`). Not measured. The server binary is the variable part: a Leptos SSR +
axum + sqlx + image-processing binary without `strip` is typically well over 100 MB
(estimate); `strip = "symbols"` in `[profile.release]` usually removes most of that.
`curl` stays only for the compose healthcheck (`src/docker/compose.yml:31`); a
`server --healthcheck` subcommand would let it go and remove one more package from the
attack surface, but that is a nicety.

Missing from the image (not a size point but found here): the `cli` binary. The build
copies only `target/release/server` (`Dockerfile:29,37`). Every operator procedure in
`docs/modules/ROOT/pages/operations.adoc` that starts with `cli ...` (`:17-18, :33, :70-95`),
the escape hatches (`apps/cli/src/main.rs:15-31`: promote an admin, password reset link,
remove TOTP, leave SSO-only mode, reopen the wizard, key rotation) and `cli migrate` cannot
be run by anyone who deploys with the image. **OPS-1 (high).**

### 1.6 Proposed Dockerfile (text only, not applied)

Goals: dependency layer that survives source edits; two independent build stages so a failure
in one does not redo the other; per-step job counts; `--locked`; stripped binary; the `cli`
in the image; checksum-verified tools. BuildKit cache mounts are left out so the same file
works under kaniko; they can be added as an optional `--mount=type=cache` on the three
`cargo` lines for local podman/buildah builds.

```
# syntax=docker/dockerfile:1
# Build context: repository root.  podman build -f src/docker/Dockerfile .
ARG RUST_IMAGE=rust:1.99.1-slim-trixie

# ---------------------------------------------------------------- tools
FROM ${RUST_IMAGE} AS tools
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends \
      curl ca-certificates pkg-config libssl-dev >/dev/null && rm -rf /var/lib/apt/lists/*
COPY config/certs/ /usr/local/share/ca-certificates/extra/
RUN update-ca-certificates >/dev/null
WORKDIR /repo
COPY src/sh/ci-tools.sh src/sh/ci-tools.sh
# ci-tools.sh: add a sha256 per tarball and `sha256sum -c` before extracting (SEC-8)
RUN sh src/sh/ci-tools.sh leptos wasm-bindgen tailwind
RUN cargo install cargo-chef --locked --version 0.1.71   # pin; MIT OR Apache-2.0
RUN rustup target add wasm32-unknown-unknown

# ---------------------------------------------------------------- recipe
FROM tools AS planner
COPY src/rust src/rust
WORKDIR /repo/src/rust
RUN cargo chef prepare --recipe-path /recipe.json

# ---------------------------------------------------------------- deps (cached while Cargo.toml/Cargo.lock are unchanged)
FROM tools AS deps
WORKDIR /repo/src/rust
COPY --from=planner /recipe.json /recipe.json
ARG DEPS_JOBS=2
ENV CARGO_BUILD_JOBS=${DEPS_JOBS} SQLX_OFFLINE=true
RUN cargo chef cook --recipe-path /recipe.json --release \
      -p server --no-default-features --features all-connectors
RUN cargo chef cook --recipe-path /recipe.json --profile wasm-release \
      --target wasm32-unknown-unknown -p web --no-default-features --features hydrate

# ---------------------------------------------------------------- frontend (wasm, css, static)
FROM deps AS frontend
COPY src/rust src/rust
COPY src/web/style src/web/style
COPY src/web/public src/web/public
COPY config config
ARG FRONTEND_JOBS=1
ENV CARGO_BUILD_JOBS=${FRONTEND_JOBS}
# small hosts: --build-arg WASM_LTO=thin  (cargo reads CARGO_PROFILE_<PROFILE>_LTO)
ARG WASM_LTO=fat
ENV CARGO_PROFILE_WASM_RELEASE_LTO=${WASM_LTO}
RUN cargo leptos build --release --frontend-only -- --locked \
    && mkdir -p /out && cp -r target/site /out/site

# ---------------------------------------------------------------- server + cli (native)
FROM deps AS server
COPY src/rust src/rust
COPY config config
ARG SERVER_JOBS=1
ENV CARGO_BUILD_JOBS=${SERVER_JOBS}
RUN cargo leptos build --release --server-only -- --locked \
    && cargo build --release --locked -p cli \
    && mkdir -p /out && cp target/release/server target/release/cli /out/

# ---------------------------------------------------------------- runtime
FROM debian:trixie-slim AS runtime
RUN apt-get update -qq && apt-get install -y -qq --no-install-recommends ca-certificates curl >/dev/null \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data app && mkdir -p /data && chown app /data
COPY config/certs/ /usr/local/share/ca-certificates/extra/
RUN update-ca-certificates >/dev/null
COPY --from=server   /out/server /out/cli /usr/local/bin/
COPY --from=frontend /out/site /srv/site
COPY config/brand /etc/app/brand
COPY src/rust/crates/ui/icons/vendor /usr/share/doc/app/icons
ENV LEPTOS_SITE_ROOT=/srv/site LEPTOS_OUTPUT_NAME=app LEPTOS_SITE_ADDR=0.0.0.0:3000 \
    LEPTOS_ENV=PROD APP__BRAND__DIR=/etc/app/brand/default \
    APP__STORAGE__FS_ROOT=/data/blobs APP__SECRETS__KEY_FILE=/data/secrets.key
USER app
EXPOSE 3000
VOLUME /data
ENTRYPOINT ["/usr/local/bin/server"]
CMD ["--role=all"]
```

Notes on the proposal:

- Two things must be checked before use (not executed): that `cargo leptos build` forwards
  arguments after `--` to cargo (otherwise drop `-- --locked` and run `cargo fetch --locked`
  in the `deps` stage instead), and the exact cargo-chef `cook` flags for a profile other
  than `release` (`--profile` is supported in recent versions). The `cli` build re-uses the
  native dependency layer because `cli` depends on the same crates as `server`
  (`apps/cli/Cargo.toml`, not shown; `apps/cli/src/main.rs:237-244` uses `config`,
  `store_pg`, `search`, `assets`, `server::escape`).
- `APP__STORAGE__FS_ROOT` moves from `/data` to `/data/blobs` so the key file is not inside
  the blob root (section 2.3). This is a change for existing volumes: document a one-time
  `mkdir /data/blobs && mv /data/up /data/img /data/http /data/blobs/` (the sub-directory
  names are from `operations.adoc:26-28`; verify against `crates/assets` before writing the
  note).
- Add to `Cargo.toml`: `[profile.release] strip = "symbols"`. Consider `lld` via
  `.cargo/config.toml` `[target.x86_64-unknown-linux-gnu] rustflags = ["-C",
  "link-arg=-fuse-ld=lld"]` plus `apt-get install lld` in `tools`; that is the single
  largest time and memory saving for the link step (estimate).
- `frontend` and `server` are independent stages. BuildKit and buildah build independent
  stages in parallel by default, which is exactly the overlap f1eeefa removed; on a small
  host build with `podman build --jobs 1` (buildah's stage parallelism switch; verify the
  flag name for the installed version) or run the two stages as two `podman build
  --target` invocations.
- For CI, give kaniko `--cache=true --cache-repo <registry>/kithkeep/cache` so the `deps`
  layers persist across pipelines, and push a tagged image; today `docker-image` discards
  its output (`.gitlab-ci.yml:169-170`).

## 2. First run

### 2.1 What an operator must set

Nothing is strictly required to start `compose.yml`: every service has a default credential
(`src/docker/compose.yml:14,16,41,53`; SEC-6). The app reads `config/.env` if present
(`compose.yml:18-20`). For a deployment that is reached from anywhere other than the docker
host itself, these are required and are not in `config/.env.example`:

| Key | Why | Where documented |
|---|---|---|
| `APP__SERVER__PUBLIC_URL` | default `http://localhost:3000` (`configuration.adoc:157`); used for links in mail, OIDC redirects, share links, and as the Origin the CSRF guard accepts (`apps/server/src/lib.rs:446-449`). With the default, every browser POST from `https://tracker.example.org` is refused. | configuration reference only |
| `APP__AUTH__SECURE_COOKIES` | default `true` (`configuration.adoc:16`); over plain HTTP from a LAN address the browser drops the `Secure` session cookie and nobody can sign in. Either terminate TLS in front or set `false` for a LAN-only test. | configuration reference only |
| `APP__SERVER__TRUSTED_PROXIES` | without it, audit rows record the proxy's address (`configuration.adoc:159`) | configuration reference only |
| `POSTGRES_PASSWORD`, `MEILI_MASTER_KEY` | replace the compose defaults | `.env.example:4-5` (empty) |

**OPS-2 (high):** `config/.env.example` (`:4-37`) lists secrets and connector keys but not
the three deployment keys above, and no page walks an operator from `compose up` to a
working sign-in behind TLS. The first-run experience on anything but `localhost` is a
sign-in page that silently fails (Secure cookie) or a 403 on every form (CSRF/Origin).

Other defaults worth knowing before the first start: `signup = invite`
(`configuration.adoc:19`; the very first account is always allowed and becomes admin,
`crates/auth/src/lib.rs:382-383`); `search.backend = meilisearch` with a silent fallback to
Postgres search if Meilisearch or its key is missing at start (`apps/server/src/lib.rs:
749-766`, logged as an error, backend shown in the admin overview); storage cap 20 GiB with
5 GiB minimum free disk (`configuration.adoc:179-180`); `jobs.concurrency = 4`
(`:96`); `database.max_connections = 20` per process (`:67`); logs as text at `info,sqlx=warn`
(`:210-211`).

### 2.2 Start-up sequence

`apps/server/src/main.rs:99-150` and `apps/server/src/lib.rs:224-425`:

1. config (file + `APP__*`), telemetry (`main.rs:101-102`);
2. brand directory (`lib.rs:225-227`);
3. Postgres with retry for 60 s on network errors, immediate failure on auth errors
   (`lib.rs:229-236`, `crates/store-pg/src/lib.rs:58-83`);
4. **all pending migrations** (`lib.rs:236`);
5. secrets key ring: file, then `APP__SECRETS__KEYS`, else generate the file (`lib.rs:237`,
   `apps/server/src/admin_wiring.rs:45-55`); refuses to start when the database holds
   encrypted values and no key is found (`crates/auth/src/secrets.rs:55-58`);
6. event bus (local for `all`, `LISTEN/NOTIFY` for `web`/`worker`, `lib.rs:242-246`), queue,
   notifications, storage budget, connectors, governed HTTP, assets, auth, OIDC, search;
7. start-up data migration: plain-text secrets from older schemas are sealed
   (`lib.rs:371-372`);
8. first source health probes (`lib.rs:399-403`);
9. then, and only then, the web role binds its port (`main.rs:120-127`). The worker role binds
   nothing (`main.rs:141-144`).

Consequence: `/healthz` answers nothing until migrations and probes are done. The compose
healthcheck allows 12 x 5 s (`compose.yml:30-34`); a slow first migration or a slow provider
probe pushes the container to `unhealthy` (compose does not restart on unhealthy, so the
process keeps running and recovers; `depends_on` consumers would not).

### 2.3 secrets.key

Generation: `create_new` with mode 0600 (`crates/auth/src/secrets.rs:597-601`) at
`APP__SECRETS__KEY_FILE`, in the image `/data/secrets.key` (`Dockerfile:47`), one line
`v1:<base64 32 bytes>`. The server logs one line; the admin overview reminds after two years
(`operations.adoc:52-61`).

Observations:

- The key file lives in the same directory as the blob store root (`Dockerfile:46-47`:
  `FS_ROOT=/data`, key `/data/secrets.key`). Any operator who syncs `/data` to an off-site
  cache bucket for the images also ships the key; any `rm -rf /data/*` to reclaim cache space
  takes the key with it. Separate them (`/data/blobs`, section 1.6) and say so in the docs.
  **OPS-3 (medium).**
- There is no first-start message in the UI that the key was generated and must be backed
  up; the wizard (`crates/web/src/setup_state.rs:9`: account, hosting, sources, done) has no
  backup step. A "download or copy this key now" step, or at least a persistent banner until
  an admin confirms the backup, is the cheapest way to prevent the one unrecoverable loss
  (section 4). **OPS-4 (medium).**
- Rotation through the CLI (`cli secrets rotate`, then SIGHUP, `apps/cli/src/main.rs:342-346`)
  is unavailable in the image (OPS-1); the manual procedure in `operations.adoc:59-68`
  remains possible with any editor.
- `APP__SECRETS__KEYS` in the environment is documented as second choice
  (`operations.adoc:48-50`); podman secrets mount as files, which fits the key file path.

### 2.4 The setup wizard

Steps: `account` (first account, becomes admin), `hosting` (home/hosted, unless fixed by
`APP__INSTANCE__HOSTING`), `sources` (connector keys, stored encrypted), `done`
(`crates/web/src/setup_state.rs:9`; API `crates/api/src/admin_routes.rs:183-229`). Resumable
by step (`crates/auth/src/admin.rs:33-35`). `cli setup reopen` reopens it (unavailable in the
image, OPS-1). Nothing in the wizard checks `public_url`, cookie security or the reverse proxy;
a "this instance is reached at X, is that right?" check with the request's `Host`/
`X-Forwarded-Proto` against `server.public_url` would catch OPS-2 at the one moment the
operator is looking.

## 3. Upgrades

### 3.1 How migrations run

- 31 SQL files, 1181 lines, embedded with `sqlx::migrate!` (`crates/store-pg/src/lib.rs:20`)
  and applied by **every process at start** (`apps/server/src/lib.rs:236`,
  `operations.adoc:17-18`), also by `cli migrate` (`apps/cli/src/main.rs:246-249`).
- Forward-only: there are no `*.down.sql` files and no revert command. sqlx runs each file in
  its own transaction unless the file opts out (none does: no `-- no-transaction` marker
  found), takes a database-level lock while migrating, and refuses to run a binary whose
  embedded set lacks a version the database has already applied (`VersionMissing`, the
  default without `ignore_missing`) - library behaviour, not executed. The practical effect
  of the last point is in 3.4.
- Concurrent starts (several web replicas) are serialised by that lock; the second process
  waits, then finds nothing to do. Fine.

### 3.2 What the existing migrations do that matters at scale

Mostly additive DDL (new tables, `ADD COLUMN` nullable or with a constant default, new
indexes). The exceptions:

| File | Statement | Effect on a big table |
|---|---|---|
| `0015_phase1_bounds.sql:48-49` | `UPDATE library_entries ... FROM library_activity(e.user_id, e.item_id)` | rewrites every row and runs a per-row function over `progress_events`; O(entries x events-per-entry), full-table lock for the duration of the transaction |
| `0028_catalog_retention.sql:6` | `UPDATE catalog_items SET provider_data_at = ...` | rewrites every catalog row in one transaction |
| `0023_stats_cache.sql:66` | `CREATE INDEX progress_events_item` (not `CONCURRENTLY`) | blocks writes to the busiest table while it builds |
| `0023_stats_cache.sql:84-144`, `0029_stats_dirty_gen.sql:14-83` | triggers on `progress_events`, `sessions`, `catalog_items` | every later write pays the trigger; fine, but a full stats rebuild writes "tens of thousands of rows" per user (`0023:8-9`) |
| `0023_stats_cache.sql:168` | `DROP INDEX sessions_owner` | an old binary still running would plan without it |
| `0031_catalog_audience.sql:4-6` | three plain `CREATE INDEX` on `library_entries`, `list_items`, `sessions` | write lock per table while building |
| `0001_core.sql:5`, `0007_search.sql:4` | `CREATE EXTENSION pg_trgm` | needs the extension available; `postgres:18.6-alpine` ships it (`compose.yml:37`) |

For the libraries this product targets (one household to a few hundred users) these finish
in seconds; the point is that they run inside the start-up path of the web process, before
the port is bound, and with the healthcheck clock running (section 2.2). At a few million
`progress_events` the 0015 pattern would take minutes and hold the healthcheck past its 60 s
budget. A rule for future migrations: backfills over the two hot tables (`progress_events`,
`library_entries`) run in batches from a job, not inline; indexes on them are created
`CONCURRENTLY` in a `-- no-transaction` file. **OPS-5 (medium, forward-looking).**

### 3.3 Code-level migrations

Two data migrations live in Rust, not SQL:

- `admin_wiring::seal_plaintext` (`apps/server/src/lib.rs:371-372`) encrypts plain-text
  secrets left by older schemas at every start; `0016_admin.sql:6-10` says the plain TOTP
  column "is emptied by the start-up migration and dropped in a later release".
- `oidc.install_env` (`lib.rs:732-734`) writes the environment's OIDC provider as a row.

They are idempotent by design, but they are invisible to anyone reading the migrations
directory to judge what an upgrade does, and the first one is not reversible (3.4).

### 3.4 Rollback story

There is none beyond "restore the dump". Specifically:

1. An older `server` binary started against a database migrated by a newer one refuses to
   start (`VersionMissing`, 3.1), so a rollback of the image alone does not work.
2. Even if it did, the TOTP sealing (3.3) means the old binary would read an emptied plain
   column: every authenticator stops working after a rollback across the 0016 boundary.
3. Therefore the only supported rollback is: stop the app, `pg_restore` the dump taken before
   the upgrade, start the old image. This is acceptable for the target audience, but it has
   to be written down, together with "take the dump first" (section 4.3). **OPS-6 (medium).**

Low-cost improvements, in order: (a) document the above; (b) print the migration list that
will run at start (`info` log per applied version) so an operator sees it before the port
binds; (c) keep the expand/contract discipline the 0016 comment already follows (drop a
column only one release after the code stopped reading it) and write it into the
engineering standard; (d) a `cli migrate --dry-run` that lists pending versions.

### 3.5 Zero or low downtime

- The roles make it possible: several `--role=web` processes behind a proxy plus one or more
  `--role=worker` (`operations.adoc:7-15`), events across processes over `LISTEN/NOTIFY`
  (`crates/realtime/src/lib.rs:3-5`), jobs leased with `FOR UPDATE SKIP LOCKED` and a
  heartbeat (`crates/jobs/src/queue.rs:114-122`, `worker.rs:154-168`), sessions in Postgres,
  no sticky-session requirement found.
- A rolling restart of web replicas works only if the new schema is compatible with the old
  binary for the overlap. Of the 31 files, only `0023:168 DROP INDEX` and the `CREATE OR
  REPLACE FUNCTION` bodies in 0029 change something an old binary uses, and both are benign.
  Keep it that way (3.4 c).
- Graceful stop: SIGTERM cancels the token (`main.rs:45-63`), axum drains connections and the
  bus closes SSE streams (`main.rs:135-139`), the worker waits `jobs.shutdown_grace_secs`
  (default 20, `configuration.adoc:97`; wired at `apps/server/src/handlers.rs:369-370`).
  `compose.yml` sets no `stop_grace_period`, so compose sends SIGKILL after the default 10 s,
  before the worker's grace is over. Jobs are reclaimed via `locked_until`
  (`queue.rs:119`), so nothing is lost, but an import at its checkpoint is redone. Set
  `stop_grace_period: 30s`. **OPS-7 (low).**
- Static assets after an upgrade: the shell references `/pkg/app.css` by fixed name
  (`crates/web/src/app.rs:195`) and the hydration scripts by `LEPTOS_OUTPUT_NAME=app`
  (`Dockerfile:42`); `hash-files` is not set in `Cargo.toml:138-157`, and the Leptos file
  handler (`apps/server/src/lib.rs:810-813`) sends no `Cache-Control` (only `/assets` and
  `/brand/fonts` do, `lib.rs:565,586`). Browsers that heuristically cache `app.js`/`app.wasm`
  from the old release will hydrate new HTML with old wasm until a hard reload. Enable
  cargo-leptos `hash-files = true` (names carry a content hash; verify the key name for
  0.3.11) or add `Cache-Control: no-cache` on `/pkg/*`. **OPS-8 (medium).**
- Meilisearch and Postgres upgrades: Meilisearch indexes are derived data; the documented
  path after any Meilisearch version change is to wipe `meili-data` and `cli reindex
  --clear` (`operations.adoc:33`), which needs the CLI (OPS-1). Postgres major upgrades need
  `pg_dump`/`pg_restore` or `pg_upgrade`; `compose.yml:37` pins 18.6 and mounts
  `/var/lib/postgresql` (the PG 18 image layout), which is correct for 18 but a note is due
  that changing the major tag does not upgrade the data directory.

## 4. Backups and restore

### 4.1 What holds state

| Store | Contents | Derived? | Where (compose) |
|---|---|---|---|
| Postgres | everything: users, library, jobs, outbox, storage index, audit, settings, encrypted secrets | no | volume `pg-data` |
| `secrets.key` | key-encryption keys for every encrypted column | no; **without it the dump's secrets are locked** (`secrets.rs:55-58`) | volume `app-data`, `/data/secrets.key` |
| blob store `up/` | user uploads (photos, avatars) | no | `app-data`, `/data/up/` (`budget.rs:591`) |
| blob store `img/`, `http/` | image variants, cached provider responses | yes, regenerated on demand (`operations.adoc:26-28`) | `app-data` |
| Meilisearch | search indexes | yes, `cli reindex --clear` | volume `meili-data` |
| brand directory | names, logos, fonts, strings | operator-authored, in git or on disk | image layer or bind mount |
| RustFS (profile `s3`) | the blob store when `backend = s3` | as above | volume `s3-data` |

`operations.adoc:20-33` lists these correctly but gives no commands, no restore order and no
verification step. The docs say "the database dump and the key file are one backup unit"
(`:30-31`); nothing enforces or checks it.

### 4.2 Runbook: backup (compose, fs backend)

```
# 1. database (consistent snapshot, custom format, compressible)
docker compose -f src/docker/compose.yml exec -T postgres \
  pg_dump -Fc -U app app > backup/kithkeep-$(date +%F).dump

# 2. the key file (tiny; store it with the dump, never in the blob sync)
docker compose -f src/docker/compose.yml cp app:/data/secrets.key backup/secrets.key

# 3. user uploads only (variants and the HTTP cache are regenerated)
docker compose -f src/docker/compose.yml cp app:/data/up backup/up

# 4. optional: the brand directory if it is not in version control
```

Hot backup is fine: `pg_dump` is transactionally consistent; uploads are content-addressed
under `up/` and never rewritten in place (inferred from the key scheme at
`crates/assets/src/budget.rs:589-591`; verify before promising immutability). Run it from a
cron on the host, keep at least the last 7 daily dumps and one monthly, and test a restore
(4.4) once after installation and after every major upgrade.

With the `s3` backend, step 3 becomes a bucket-level copy of the `up/` prefix with the S3
tool of choice; the storage index stays in Postgres.

### 4.3 Runbook: before an upgrade

1. `pg_dump` as in 4.2 step 1 (this is the rollback, section 3.4).
2. Copy `secrets.key` if it is not already in the backup set.
3. Pull or build the new image.
4. Start; watch the log until `listening` (`main.rs:128`) appears; migrations run before it.
5. Smoke test: sign in, open an item, trigger a search.
6. If it fails: stop, `pg_restore` (4.4 step 3) the dump from step 1, start the previous image.

### 4.4 Runbook: restore to an empty host

1. Start only Postgres and Meilisearch: `docker compose up -d postgres meilisearch`.
2. Create the key file **before** the first app start: copy `backup/secrets.key` into the
   `app-data` volume at `/data/secrets.key`, mode 0600, owned by uid 10001 (`docker compose
   cp` or `podman volume mount`). If the app starts first with an empty database and no key,
   it generates a new `v1` and the restored dump's secrets will be locked; the server then
   refuses to start as soon as the database holds rows it cannot open
   (`secrets.rs:55-58`), which is the intended safety net, but it costs a confusing restart.
3. `pg_restore --clean --if-exists -U app -d app backup/kithkeep-<date>.dump` (via
   `docker compose exec -T postgres`).
4. Copy `backup/up` back to `/data/up`.
5. Start the app. It migrates forward if the image is newer than the dump.
6. Rebuild search: `cli reindex --clear` - which needs the CLI in the image (OPS-1); until
   then the only way is to wipe `meili-data` and... there is no second way: the indexer
   drains the outbox, it does not re-enqueue existing rows without the CLI
   (`apps/cli/src/main.rs:284-290` calls `Indexer::enqueue_all`). **This makes OPS-1 a
   backup-and-restore blocker, not only an inconvenience.**
7. Verify: sign in, check Admin > Security shows every key version as loaded (the "NOT
   LOADED" state in `cli secrets status`, `main.rs:365-373`, is the symptom of a wrong key),
   open an item with a cover (variants regenerate), search for it.

### 4.5 Gaps

- No documented commands (4.2-4.4 above are the proposal). **OPS-9 (medium).**
- No in-app backup indicator: the admin overview could show "last dump seen" if the operator
  writes a marker, or at least a permanent note that the key file exists and where. Low cost.
- No key escrow guidance: suggest the operator print `secrets.key` once (a 44-character line).

## 5. Observability

### 5.1 Logs

`crates/telemetry/src/lib.rs:11-29`: `tracing_subscriber::fmt`, filter from `RUST_LOG`
else `telemetry.filter` (default `info,sqlx=warn`), text or JSON (`flatten_event`), and
`FmtSpan::CLOSE` so every closed span logs its duration. No request-logging layer was found
(`TraceLayer`/`tower_http` absent from `apps/server` and `crates/api`), so there is no
per-request access line; the client address and request id are attached for audit rows only
(`apps/server/src/lib.rs:450-453`, `configuration.adoc:159`). For a self-hoster this is
acceptable; for `hosted` mode a one-line access log (method, path class, status, duration,
request id) at `info` would be the single most useful addition for incident work.

Start-up logs the listening address (`main.rs:128`), the search backend fallback as an error
(`lib.rs:751,763`), brand contrast warnings (`lib.rs:533-552`), key generation
(`operations.adoc:52-54`), and the SIGHUP key reload (`main.rs:75-79`).

### 5.2 Metrics

By decision (DEC-060, `telemetry/src/lib.rs:2`) metrics are `tracing` events with target
`metrics` until an exporter exists. Found: `storage.refused`, `storage.hard_cap`,
`storage.evicted` (`crates/assets/src/budget.rs:353,359,545`) and the usage funnel
(`crates/api/src/usage.rs:323`). Nothing for request rate or latency, job queue depth, job
failures, outbox lag, provider call counts or cache hit ratio at the metrics target; several
of those exist as data in Postgres and in the admin UI (lane counts, dead jobs, cron list:
`crates/jobs/src/admin.rs:42-131`; source counters flushed every 60 s,
`apps/server/src/lib.rs:33,500-519`). A `/metrics` endpoint in Prometheus text format that
reads those tables would be a small, dependency-light step and is the first thing a
self-hoster with Grafana will ask for. **OPS-10 (low).**

### 5.3 Health endpoints and probes

- `/healthz` returns the constant `ok` (`apps/server/src/lib.rs:430`). It proves the process
  accepted a connection after start-up, nothing about Postgres, the search backend or the
  blob store. It sits behind the CSRF and request-meta layers (`lib.rs:440`), which is
  harmless for GET.
- There is no readiness endpoint distinct from liveness; because the port is bound only after
  migrations (2.2), "port open" is in practice readiness, and the compose healthcheck
  (`compose.yml:30-34`) uses it that way.
- The worker role has **no HTTP listener at all** (`main.rs:141-144`), so a `--role=worker`
  container cannot be health-checked by compose, Kubernetes or systemd watchdogs except by
  "process alive". A worker whose Postgres connection died stays "alive" while doing nothing.
  Minimal fix: a `jobs.heartbeat` row per worker process (name, pid, last seen) and a
  `/healthz` on a small side port for the worker, or an `admin.probes` result for "workers
  seen in the last 2 minutes" in the admin overview, which already runs every minute
  (`handlers.rs:337`). **OPS-11 (medium).**
- Source probes run at start and every minute via cron (`lib.rs:399-403`,
  `handlers.rs:337`); results feed the admin overview, not an HTTP endpoint.

### 5.4 Scheduled jobs an operator should know exist

`apps/server/src/handlers.rs:325-349`: `admin.audit_retention` 03:17, `secrets.rewrap` 03:37,
`admin.probes` every minute, `usage.funnel_daily` 03:47, `catalog.retention` hourly at :41;
plus eviction (`handlers.rs:139`) and the cron ticker every 30 s (`lib.rs:470-485`). Times
are UTC (`chrono::Utc::now()`, `handlers.rs:326`); nothing in the docs says so.

## 6. Resource needs and scaling

### 6.1 Processes and what is per process

| State | Scope | Source | Consequence for N processes |
|---|---|---|---|
| sign-in throttle | in-memory per process | `crates/auth/src/lib.rs:200` | N x the attempts (SEC-5) |
| provider rate limiter (`governor`) | per process, per connector | `crates/catalog-core/src/http.rs:107,120` | N x the provider quota; two web replicas can breach a TMDB or IGDB limit that one respects. Mitigation: keep connector traffic in the worker (refreshes already are, `handlers.rs:14-15`), or move the limiter to Postgres/Meilisearch-free shared state later |
| event bus | local broadcast for `--role=all` (`lib.rs:242-246`, `realtime/src/lib.rs:39-40`); `LISTEN/NOTIFY` for `web`/`worker` | two `--role=all` processes never see each other's events: SSE updates from a job claimed by process B do not reach a browser on process A. Rule: more than one process means `web` + `worker` roles, never two `all`. Not in the docs. **OPS-12 (medium)** |
| source counters | per process, flushed every 60 s and merged in Postgres | `lib.rs:33,500-519` | fine |
| image CPU pool | `available_parallelism` threads per process | `crates/assets/src/pool.rs:46-47` | a web replica also decodes images unless `fetch_policy` and the jobs keep that in the worker; with cgroup CPU limits `available_parallelism` reads the quota on recent Rust (not executed) |
| Postgres pool | `database.max_connections` default 20 per process | `configuration.adoc:67`, `store-pg/src/lib.rs:44-48` | 2 web + 2 worker + CLI = 82 of Postgres' default 100 `max_connections`; the docs do not say that this knob is per process. Set 10 for web replicas or raise Postgres |
| `pg_notify` payload | 7900-byte cap, larger events lose the payload | `realtime/src/lib.rs:25-26` | browsers then refetch; fine |
| job leases | shared table, `SKIP LOCKED`, heartbeat | `jobs/src/queue.rs:114-122,140-150` | scales to many workers as designed |
| cron scheduling | shared table, `SKIP LOCKED` | `queue.rs:259` | every worker registers the same entries at start (`handlers.rs:325-366`, idempotent by name) |

### 6.2 Sizing (estimates; nothing measured in this review)

- **App, `--role=all`, one household:** a Rust SSR process of this size typically idles at
  100-300 MB RSS; image decoding of a 4096x4096 source (`configuration.adoc:178`) adds tens
  of MB per concurrent job, times `jobs.concurrency` (4). 1 vCPU, 512 MB-1 GB is a sane
  compose default; the compose file sets no limits at all (`compose.yml:5-34`).
- **Postgres 18:** the schema is small; the stats cache (`0023`) and `progress_events` grow
  with use. 512 MB-1 GB and the default `shared_buffers` are enough for the target scale.
- **Meilisearch 1.56:** memory-mapped indexes; it will take what the host gives. Index size
  scales with catalog items x locales. 512 MB floor.
- **Disk:** blob cap 20 GiB plus 5 GiB minimum free (`configuration.adoc:179-180`), HTTP
  cache 2 GiB inside the cap (`:175`), Postgres and Meilisearch on top. The storage report
  (`cli storage report`, `operations.adoc:88-99`) shows the actual numbers but needs the CLI
  (OPS-1); the admin "Storage and jobs" page shows the same in the UI (OD-14).
- **Build host:** 8 GB free for the image build as the Dockerfile states (`Dockerfile:19-21`),
  4 GB free with `CARGO_BUILD_JOBS=1` and thin LTO (estimate), 20+ GB disk for the build
  cache (`HANDOFF.md:805-806` describes the target directory growth).

Put a "Sizing" table with these orders of magnitude into `operations.adoc` and replace the
estimates with measurements from the owner's instance after a week (`cli storage report
--json` plus `docker stats`).

### 6.3 Scaling path

1. One container, `--role=all` (today's compose). Fine up to a family or club.
2. Split: `web` + `worker` containers, same image, `command: ["--role=web"]` /
   `["--role=worker"]`; the bus switches to `LISTEN/NOTIFY` automatically (`lib.rs:242-246`).
   Nothing in compose shows this; a commented second service pair would make it a copy-paste
   step. Worker health (OPS-11) becomes relevant here.
3. Several `web` replicas behind a proxy: works for sessions and CSRF (public URL based), but
   note the per-process state above (throttle, rate limiter). SSE is one long-lived
   connection per open tab per process; a proxy must not buffer `text/event-stream`
   (section 7).
4. Several workers: supported as designed; `jobs.concurrency` x workers is the cap on
   concurrent provider calls, bounded by the per-process limiter.

## 7. Documentation gaps for self-hosters

What `docs/modules/ROOT/pages/operations.adoc` and the README cover today: roles, that
migrations run at start, what to back up (no commands), secrets at rest and rotation, search
reindex, storage report, S3 with compose, TLS-inspecting proxies at build time, rootless
podman DNS trouble. What is missing, in the order a new operator hits it:

1. **Reverse proxy and TLS** (nothing anywhere: grep for `reverse proxy`, `caddy`, `nginx`,
   `traefik` over `docs/modules` finds only the SMTP `tls` enum). Needed: that the app
   speaks plain HTTP on 3000 and expects TLS in front; `APP__SERVER__PUBLIC_URL` must be the
   external `https://` URL (CSRF Origin check, `lib.rs:446-449`; OIDC redirect; mail links);
   `APP__SERVER__TRUSTED_PROXIES` for the proxy's address; `secure_cookies` stays `true`;
   pass `Host`, `X-Forwarded-For`, `X-Forwarded-Proto`; **do not buffer SSE** (Caddy
   `flush_interval -1`, nginx `proxy_buffering off` for `/api/v1/events` or the SSE path -
   verify the path in `crates/api/src/routes.rs`); body size for imports is 64 MiB
   (`operations.adoc:132`), so the proxy's limit must be at least that; HTTP/2 or keep-alive
   for the SSE connection count. A Caddy snippet (three lines) and an nginx snippet would
   settle 90 % of first-day issues. (OPS-2.)
2. **First run end to end**: copy `.env.example`, set the four keys from 2.1, `compose up`,
   open `/setup`, back up `secrets.key`. Today the README's quick start is one line
   (`README.md:11-13`) and stops before the sign-in problem.
3. **Backup and restore commands** and the restore order (section 4). (OPS-9.)
4. **Upgrade procedure and rollback** (section 3.4, 4.3). (OPS-6.)
5. **Podman specifics**: `podman compose` vs `docker compose` with the podman socket; the
   existing DNS override (`compose.podman.yml`, `operations.adoc:142-153`) is good and
   unusual to find; add: named volumes work rootless without extra flags, bind mounts need
   `:Z` on SELinux hosts and `:U` or `podman unshare chown 10001:10001` because the image
   runs as uid 10001 (`Dockerfile:34,48`); the build memory story (section 0 and 1.6:
   `--build-arg DEPS_JOBS=1 FRONTEND_JOBS=1 SERVER_JOBS=1 WASM_LTO=thin`); `podman build`
   does not share layers with `podman compose build` unless the same storage is used
   (rootless: it is).
6. **Health and logs**: `/healthz` semantics (5.3), `APP__TELEMETRY__LOG_FORMAT=json` for log
   shippers, `RUST_LOG` precedence (`telemetry/src/lib.rs:9-15`), the cron times in UTC (5.4).
7. **Sizing** (6.2) and the per-process knobs (6.1).
8. **Which roles may be duplicated** (OPS-12).
9. **Compose hardening**: the SEC-6 items (real credentials, bind `127.0.0.1:3000:3000`
   behind a proxy), `stop_grace_period` (OPS-7), resource limits, pinning the app image to a
   tag once images are published.
10. **Where the CLI is** - once OPS-1 is fixed: `docker compose exec app cli storage report`.

## 8. Prioritised plan

Ordered by (operator pain) x (cost to fix). Effort: S under a day, M a few days.

| # | Action | Fixes | Effort |
|---|---|---|---|
| 1 | Ship `cli` in the image (`cargo build --release -p cli`, copy next to `server`) | OPS-1, unblocks 4.4 step 6 and every `operations.adoc` procedure | S |
| 2 | Add `APP__SERVER__PUBLIC_URL`, `APP__SERVER__TRUSTED_PROXIES`, `APP__AUTH__SECURE_COOKIES` to `.env.example` with one-line comments; write the reverse proxy page with Caddy and nginx snippets and the SSE note | OPS-2, item 7.1-7.2 | S |
| 3 | Dockerfile from 1.6: cargo-chef dependency layer, split frontend/server stages, per-step job args, `--locked`, `strip`, checksums in `ci-tools.sh` | build reliability on 16 GB hosts, rebuild time, SEC-8 | M |
| 4 | Backup/restore/upgrade runbook into `operations.adoc` (sections 4.2-4.4, 3.4); state the rollback rule | OPS-6, OPS-9 | S |
| 5 | Move the key file out of the blob root (`/data/blobs`), add a "back up this key now" step or banner after the wizard | OPS-3, OPS-4 | S-M |
| 6 | `hash-files = true` (or `Cache-Control: no-cache` on `/pkg/*`) | OPS-8 | S |
| 7 | Compose: `stop_grace_period: 30s`, commented `web`/`worker` service pair, resource limits, SEC-6 defaults, bind to loopback | OPS-7, OPS-12, SEC-6 | S |
| 8 | Worker liveness: heartbeat row + admin overview "workers seen", or a side-port `/healthz` for `--role=worker`; make `/healthz` touch the pool (`SELECT 1`) and report the search backend | OPS-11 | M |
| 9 | Log the migration versions applied at start; migration rule for hot tables (batched backfill, `CONCURRENTLY` + `-- no-transaction`) into the engineering standard | OPS-5, OPS-6 b/c | S |
| 10 | `/metrics` in Prometheus text format from the existing queue, outbox and source-counter tables; one access-log line per request behind a config switch | OPS-10, 5.1 | M |
| 11 | CI: kaniko `--cache`, push tagged images on tags, drop the duplicate `leptos-build` release compile or make it the artefact the image stage copies | 1.2-1.3 | S-M |

## 9. Findings index

| Id | Severity | One line | Where |
|---|---|---|---|
| OPS-1 | high | `cli` binary is not in the image; every documented operator procedure and the restore's reindex step are impossible for image users | `Dockerfile:29,37`; `operations.adoc:17-95`; `apps/cli/src/main.rs:15-31` |
| OPS-2 | high | No reverse-proxy/TLS docs; `public_url`, `trusted_proxies`, `secure_cookies` absent from `.env.example`; default `public_url` makes every form fail from any other host | `config/.env.example:4-37`; `configuration.adoc:16,157,159`; `lib.rs:446-449` |
| OPS-3 | medium | `secrets.key` lives inside the blob root `/data` | `Dockerfile:46-47` |
| OPS-4 | medium | No prompt or banner to back up the generated key | `setup_state.rs:9`; `operations.adoc:52-54` |
| OPS-5 | medium (forward) | Inline full-table backfills and non-concurrent indexes on the hot tables run inside start-up before the port binds | `0015:48-49`; `0028:6`; `0023:66`; `0031:4-6`; `lib.rs:236`; `main.rs:120-127` |
| OPS-6 | medium | No rollback path except restoring a dump; older binary refuses a newer schema; TOTP sealing is one-way; none of this is documented | `store-pg/src/lib.rs:90-92`; `lib.rs:371-372`; `0016:6-10` |
| OPS-7 | low | compose `stop_grace_period` (10 s default) shorter than `jobs.shutdown_grace_secs` (20) | `compose.yml:5-34`; `configuration.adoc:97` |
| OPS-8 | medium | Unhashed `/pkg/app.*` without cache headers: stale wasm after upgrades | `app.rs:195`; `Cargo.toml:138-157`; `lib.rs:810-813` |
| OPS-9 | medium | Backup docs name what but not how; no restore order, no verification | `operations.adoc:20-33` |
| OPS-10 | low | No `/metrics`, four metric events total, no access log | `telemetry/src/lib.rs`; `budget.rs:353,359,545`; `usage.rs:323` |
| OPS-11 | medium | Worker role has no health signal; `/healthz` is a constant | `main.rs:141-144`; `lib.rs:430` |
| OPS-12 | medium | Two `--role=all` processes do not share events; not documented | `lib.rs:242-246`; `realtime/src/lib.rs:39-40` |

Checked and holds (no finding): migrations are embedded and versioned with checksums;
start-up refuses to run without the key when encrypted rows exist; jobs are leased with
heartbeat and reclaimed after a crash; the indexer marks outbox rows processed only after the
index accepted the batch (`search/src/indexer.rs:1-3`); SIGTERM/SIGINT drain the HTTP server
and close SSE streams; Postgres connection retry waits only for network errors and fails
fast on bad credentials; the compose healthchecks for Postgres and Meilisearch are correct;
no provider key or secret is written to the image; the image runs as a non-root user with a
writable `/data` only.

Not checked: actual build time, memory and image size (nothing was built); cargo-leptos and
cargo-chef flag details named in 1.6; the SSE route path for the proxy snippet; Meilisearch
1.56 upgrade behaviour; the exact Rust behaviour of `available_parallelism` under cgroup
limits.
