# RustFS as the S3-compatible dev/test service (DEC-008 reconsider)

Research note, 2026-11-06. Question from `docs/HANDOFF.md` 4.5: can RustFS replace
versitygw as the S3-compatible service in `src/docker/compose.yml`, `cargo xtask services`
and `.gitlab-ci.yml`? Owner comment on DEC-008: "RustFS". Rule: licence first, then docs.

Nothing from RustFS is linked into or shipped with our code. It would only run as a
separate process (dev shell), a container (compose) and a CI service container.

## 1. Recommendation

**Switch, in all three places, with one verification gate.** RustFS is Apache-2.0 at every
level that matters to us (repository, Cargo metadata, console UI, `rc` client, Docker Hub
image description, docs site), it has stable 1.0.x releases since 2026-09-16, the exact
version (1.0.4) is already in the nixpkgs revision locked by our `flake.lock` and comes from
the binary cache, and its S3 coverage is well above the four calls our `assets` crate makes
through opendal.

The gate: this research could not run the binary (the sandbox refused executing a downloaded
release binary), so the first implementation step must be `cargo xtask services up` plus
`cargo test -p assets --test s3` with `TEST_S3_ENDPOINT` set, and the memory footprint of the
idle RustFS process on the 4 GB build host must be looked at (section 4.6). If that fails or
is unreasonable, the fallback is "compose and CI only, versitygw stays for xtask" (section 6).

Honest trade-off: for a test stub versitygw is the lighter tool (one small Go binary, plain
files on disk). RustFS is a full MinIO-class server (648 MiB nix closure, erasure-coded
on-disk format, background scanner, embedded console). We switch because the owner asked for
it and the licence allows it, not because the test stub needs it.

## 2. Licence check (done first)

| Item | Finding | Source (checked 2026-11-06) |
|---|---|---|
| Repository licence | Apache-2.0: `LICENSE` is the verbatim Apache License 2.0 text; GitHub metadata `license.spdx_id = "Apache-2.0"` | https://raw.githubusercontent.com/rustfs/rustfs/main/LICENSE ; https://api.github.com/repos/rustfs/rustfs |
| Cargo metadata | workspace `license = "Apache-2.0"` (Cargo.toml line 75), every file carries the Apache header | https://raw.githubusercontent.com/rustfs/rustfs/main/Cargo.toml |
| Web console (bundled into the binary by nixpkgs) | separate repo `rustfs/console`, Apache-2.0 | https://api.github.com/repos/rustfs/console |
| `rc` client (`rustfs/cli`) | Apache-2.0 | https://api.github.com/repos/rustfs/cli |
| Docker image `rustfs/rustfs` | Docker Hub description: "High-performance distributed object storage, an alternative to MinIO, Apache 2.0 license"; image is built by the repo's own `Dockerfile` (alpine 3.24.2 base, musl release zip, `rc` copied to `/usr/bin/rc`) | https://hub.docker.com/v2/repositories/rustfs/rustfs/ ; https://raw.githubusercontent.com/rustfs/rustfs/main/Dockerfile |
| Release binaries | GitHub release assets (zip/deb/rpm for linux gnu+musl x86_64/aarch64, macOS, Windows), plus SBOM and provenance; no separate EULA on the release page | https://api.github.com/repos/rustfs/rustfs/releases/latest |
| Docs site licence page | "RustFS is released under the Apache 2.0 license"; appendix notice "Copyright 2024-2026 RustFS, Inc."; no dual licence, no enterprise edition, no separate console licence mentioned | https://docs.rustfs.com/en/developer/license |
| nixpkgs package | `meta.license = asl20` (`pkgs/by-name/ru/rustfs/package.nix`) | nixpkgs rev 1d060086d60cb76a40cf37109f979d88a5bd6396 (our flake.lock) |

Consequences for an MIT project:

- Running an Apache-2.0 program in dev, test and CI imposes nothing on our code. We do not
  copy, link, vendor or redistribute it. `cargo-deny` is unaffected (no crate dependency).
- Apache-2.0 section 6 grants no trademark rights. We name "RustFS" only to identify the
  tool in docs, compose and DECISIONS, which the licence expressly allows ("as required for
  reasonable and customary use in describing the origin of the Work").
- The repo also has a `CLA.md` (relevant only for contributing upstream, not for use).
- The repository root lists `crates/license` ("License and entitlement provider contracts").
  The docs site and README mention no paid tier, licence key or feature gate for the server;
  nothing we use requires one. Recorded as a watch item, not a blocker.
- Since the project is Apache-2.0, rule 6 does not apply: its docs, scripts and source may be
  read. This note read the README, docs site, `Dockerfile`, `entrypoint.sh`,
  `docker-compose-simple.yml`, `nix/rustfs-module.nix` and `docs/architecture/
  s3-compatibility-matrix.md`; no RustFS source code was copied anywhere.

## 3. What RustFS is and how it is run

### 3.1 Maturity and releases

- Latest release: **1.0.4**, published 2026-10-25, not a pre-release. Stable line: 1.0.0
  (2026-09-16), 1.0.1, 1.0.2, 1.0.3, 1.0.4; before that rc.1-rc.6 (Aug-Sep 2026), beta.1-12
  (Apr-Jul 2026), alpha up to .99. Source: https://api.github.com/repos/rustfs/rustfs/releases
  (2026-11-06).
- Repository active (pushed 2026-11-06), 35.8k stars, not archived.
- Docker Hub: tags `1.0.4`, `1.0.4-glibc`, `latest`, `latest-glibc`, plus `-preview.N`
  tags; 20.9 M pulls; image last updated 2026-10-25.
- Docs site: https://docs.rustfs.com/en (pages carry no version or date; footer
  "Copyright 2025 RustFS"). The repo's `docs/` folder holds the engineering docs
  (architecture, operations, testing, postmortems).

### 3.2 Topologies

Single Node Single Disk (SNSD) = one server, one data path, no redundancy, "intended for
development, testing, and small non-critical workloads". Exactly our case. README note:
an SNSD deployment "is supported only as a standalone local path. It cannot expand in place
or be added as a Pool." Source: https://docs.rustfs.com/en/installation/linux ;
README "Quickstart" (https://github.com/rustfs/rustfs, 2026-11-06).

### 3.3 Single local process (xtask on NixOS, no docker)

CLI (https://docs.rustfs.com/en/reference/cli): `rustfs server [OPTIONS] <VOLUMES>...`,
`server` is the default subcommand. Flags and their env equivalents; "the flag wins when
both are set":

| Flag | Env | Default | Meaning |
|---|---|---|---|
| `<VOLUMES>...` | `RUSTFS_VOLUMES` | required | data path(s); one local absolute path for SNSD |
| `--address` | `RUSTFS_ADDRESS` | `:9000` | S3 API bind address `IP:PORT` |
| `--access-key` | `RUSTFS_ACCESS_KEY` | `rustfsadmin` | root access key (`_FILE` variant exists) |
| `--secret-key` | `RUSTFS_SECRET_KEY` | `rustfsadmin` | root secret key (`_FILE` variant exists) |
| `--console-enable` | `RUSTFS_CONSOLE_ENABLE` | `true` | embedded web console on its own listener |
| `--console-address` | `RUSTFS_CONSOLE_ADDRESS` | `:9001` | console bind address |
| `--region` | `RUSTFS_REGION` | unset -> `us-east-1` | region reported to clients |
| - | `RUSTFS_OBS_LOG_DIRECTORY` | unset -> stdout | log directory |
| - | `RUSTFS_OBS_LOGGER_LEVEL` | unset | log level (`info`, `debug`, ...) |

Sources: https://docs.rustfs.com/en/reference/cli and
https://docs.rustfs.com/en/reference/environment-variables (2026-11-06). The NixOS module in
the repo (`nix/rustfs-module.nix`) starts the service as `ExecStart = <pkg>/bin/rustfs` with
`RUSTFS_VOLUMES`, `RUSTFS_ADDRESS`, `RUSTFS_CONSOLE_ENABLE`, `RUSTFS_CONSOLE_ADDRESS` and
`RUSTFS_ACCESS_KEY_FILE`/`RUSTFS_SECRET_KEY_FILE` in the environment, which confirms the
env-only start without positional arguments. The docs' systemd unit uses
`ExecStart=/usr/local/bin/rustfs $RUSTFS_VOLUMES`
(https://docs.rustfs.com/en/installation/linux/prerequisites-and-service).

Health: `GET http://<host>:9000/health` (Docker page; the repo compose healthcheck uses the
same path and `/rustfs/console/health` on 9001).

Other knobs that matter on a 2 vCPU / 4 GB host:

- The background scanner (usage accounting, lifecycle, heal, bitrot) runs by default;
  `RUSTFS_SCANNER_SPEED=slowest` and `RUSTFS_SCANNER_IDLE_MODE=true` (default) reduce idle
  CPU; `RUSTFS_SCANNER_BITROT_CYCLE_SECS=off` disables periodic deep scans. Source:
  https://github.com/rustfs/rustfs/blob/main/docs/operations/scanner-runtime-controls.md
  (2026-11-06).
- `RUSTFS_UNSAFE_BYPASS_DISK_CHECK` (repo `docker-compose-simple.yml`, comment: "Keep strict
  disk topology checks enabled by default. For local testing only, set ... =true explicitly").
  Not documented on the docs site; semantics unknown beyond the comment. An SNSD path should
  not need it. Watch item for the verification gate.
- Docs minimum for a test environment: 2 GB RAM; production 128 GB; XFS recommended, "ext4,
  Btrfs, and ZFS should be avoided" (production guidance;
  https://docs.rustfs.com/en/installation/linux/prerequisites-and-service).
- Credentials: the quick start says "Use an uppercase alphanumeric access key" and warns
  against unmodified Base64 in `RUSTFS_ACCESS_KEY`
  (https://docs.rustfs.com/en/installation/linux/quick-start). Whether a lowercase key like
  our current `test` is rejected is not stated; see section 7.

### 3.4 Bucket creation

RustFS has no "auto-create bucket" setting in the documented variables. Options, in order of
preference for us:

1. **curl with SigV4** (no extra tool; curl 8.21.0 in the dev shell and in the CI image
   supports `--aws-sigv4`):
   `curl -fsS --aws-sigv4 "aws:amz:us-east-1:s3" --user test:testsecret -X PUT http://127.0.0.1:55433/assets`
   A repeated PUT answers `BucketAlreadyOwnedByYou` (HTTP 409) on S3-compatible servers; the
   xtask step must accept 200 and 409. (Verify RustFS's exact answer at the gate.)
2. **`rc`** (RustFS client, Apache-2.0; nixpkgs `rustfs-cli` 0.1.36, `mainProgram = rc`):
   `rc alias set local http://127.0.0.1:55433 <key> <secret>` then
   `rc bucket create local/assets --ignore-existing` (the `--ignore-existing` flag is used by
   the image's own `entrypoint.sh`; the docs page for `rc` shows only `alias set` and
   `bucket list`). Sources: https://docs.rustfs.com/en/operations/rc ;
   https://raw.githubusercontent.com/rustfs/rustfs/main/entrypoint.sh (2026-11-06).
3. **Container bootstrap**: the official image's entrypoint reads
   `${RUSTFS_BOOTSTRAP_DIR:-/etc/rustfs/bootstrap}/buckets.json` (a JSON array of bucket
   names) and creates them with `rc bucket create --ignore-existing` against
   `RUSTFS_BOOTSTRAP_ENDPOINT` (must be an http(s) endpoint; explicit credentials required).
   Useful for compose (bind-mount one file), not for GitLab services (no mounts).
4. Console UI (`Create Bucket`), manual only.

### 3.5 Container

Image `rustfs/rustfs:1.0.4` (musl build on alpine 3.24.2; `-glibc` variant exists). Runs as
non-root `rustfs` (UID/GID 10001:10001); `/data` is a declared `VOLUME` owned by that user;
image defaults `RUSTFS_VOLUMES=/data`, `RUSTFS_OBS_LOG_DIRECTORY=/logs`,
`RUSTFS_OBS_LOGGER_LEVEL=warn`; `EXPOSE 9000 9001`; `curl`, `jq` and `rc` are present in the
runtime stage. Bind-mounted host paths must be writable by 10001:10001, otherwise startup
fails with permission errors (README, Docker page). Sources:
https://raw.githubusercontent.com/rustfs/rustfs/main/Dockerfile ;
https://docs.rustfs.com/en/installation/container/docker ; README (2026-11-06).

Minimal run from the docs: `docker run -d -p 9000:9000 -p 9001:9001 -v $(pwd)/data:/data
-v $(pwd)/logs:/logs rustfs/rustfs:1.0.4` with `RUSTFS_ACCESS_KEY`/`RUSTFS_SECRET_KEY`.

### 3.6 Availability

- **nixpkgs**: `rustfs` 1.0.4 (Apache-2.0, `mainProgram = rustfs`, bundles console
  0.1.37) in the nixpkgs revision our `flake.lock` pins
  (1d060086d60cb76a40cf37109f979d88a5bd6396); `nix build --dry-run` reports "will be
  fetched (194.2 MiB download, 647.8 MiB unpacked)", so no local compile. `rustfs-cli`
  0.1.36 (`rc`) is also packaged. The registry-default `nixpkgs` on this host is older and
  only has 1.0.0-beta.9, so the flake lock matters. (Checked with `nix eval` on 2026-11-06;
  search.nixos.org is JavaScript-only and was not used.)
- **Static binary**: `rustfs-linux-x86_64-musl-v1.0.4.zip` (102 MB zip, 276 MB binary) from
  the GitHub release; docs point to
  `https://dl.rustfs.com/artifacts/rustfs/release/rustfs-linux-x86_64-musl-latest.zip`.
  Usable by `src/sh/ci-tools.sh` for a non-Nix host, but CI should use the service container.
- `nix run github:rustfs/rustfs` builds from source (README option 5): not for our host.

## 4. S3 API coverage against our use

### 4.1 What our storage crate does on S3

`src/rust/crates/assets/src/backend.rs` (opendal 0.59.5, `services-s3`, MIT-licensed
project code of ours; opendal is Apache-2.0):

- `Operator::write(key, Vec<u8>)` -> opendal's `MultipartWriter` with the whole body known
  at `close()` and no multipart started -> a single `PutObject`
  (`opendal-core-0.59.5/src/raw/oio/write/multipart_write.rs` lines 333-334: "Call
  write_once if there is no upload_id"). Multipart would only start for bodies written in
  chunks >= 5 MiB (`write_multi_min_size`), which we never do.
- `Operator::read(key)` -> `GetObject` (404 mapped to `None`).
- `delete_iter(keys)` -> `DeleteObjects` (POST `?delete`, batches up to
  `DEFAULT_BATCH_MAX_OPERATIONS`). opendal always sets `Content-MD5`
  (`opendal-service-s3-0.59.5/src/core.rs` line 1403: "Set content-md5 as required by API").
- Builder options: `bucket`, `region`, `root` (prefix), `endpoint`, static keys,
  `disable_config_load`, `disable_ec2_metadata`. Path-style vs virtual-host: opendal uses
  path-style for a custom endpoint unless `enable_virtual_host_style` is set (not set).
- Not used anywhere in `assets`: presign, list, stat/HEAD (DEC-054: "no HEAD before PUT"),
  conditional writes (`if_match`/`if_not_exists`), versioning, tagging. The "redirect"
  paths in `budget.rs`/`service.rs` redirect to the provider URL, not to presigned S3 URLs.

The test `src/rust/crates/assets/tests/s3.rs` writes one small object under a per-run
`root` prefix, reads it, batch-deletes it and reads again. Credentials/bucket from env
`TEST_S3_ENDPOINT`, `TEST_S3_BUCKET` (default `assets`), `TEST_S3_ACCESS_KEY` (`test`),
`TEST_S3_SECRET_KEY` (`testsecret`).

### 4.2 RustFS coverage

From `docs/architecture/s3-compatibility-matrix.md` (repo, 2026-11-06), whose source of
truth is the Ceph s3-tests lists under `scripts/s3-tests/`:

| Needed by us | RustFS status |
|---|---|
| Bucket create/delete/list/head | Supported |
| Object put/get/delete/copy/head | Supported |
| `DeleteObjects` (bulk) | Supported; without `Content-MD5` answers `400 MissingContentMD5` (same as AWS/MinIO). opendal sends it, so fine. Conditional (ETag) bulk delete also supported; we do not use it. |
| `GetObject` on a missing key | standard `NoSuchKey`/404 (opendal maps to NotFound) |
| Region | `RUSTFS_REGION`, default `us-east-1`; our `S3Config.region` default is `us-east-1` |

Beyond our needs, also listed as supported: ListObjects/ListObjectsV2 (prefix, delimiter,
marker, max-keys), multipart create/upload/complete/abort, presigned GET and PUT URLs,
range and conditional reads, tagging, bucket policy, public access block, user metadata,
SSE-C, "selected versioning, object-lock, checksum, CORS, raw request, and conditional
write behavior". Not yet passing (not needed by us): some multipart listing edge cases
(`ListParts` may answer 503 `SlowDownRead` when a participant is unavailable). Deviations
to remember if our usage grows: `CopyObject` with `MetadataDirective=REPLACE` clears
`Content-Type` unless resent; object keys with C0 control characters are rejected with
`400 InvalidArgument`; uploads slower than 64 KiB per 60 s are cut with `400 RequestTimeout`.

The matrix states the claim carefully: "broad S3 API compatibility for supported features.
It does not claim complete coverage of every standard or vendor-specific S3 behavior."

### 4.3 Verdict on coverage

Everything the `assets` crate sends is in the "Supported" rows, and the one known trap
(`MissingContentMD5` on bulk delete) is handled by opendal. Presign and ListObjectsV2 are
available should D-8 later need redirect-to-S3 or a reconciliation listing.

### 4.4 Comparison with versitygw (current)

versitygw: Apache-2.0 (GitHub metadata), latest v1.10.0 (2026-10-22), nixpkgs 1.9.0 in our
lock, Docker Hub `versity/versitygw` (tags `latest`, `v1.10.0`, ...). Posix backend: the
bucket is a directory under the data root, created by `std::fs::create_dir_all` in
`services.rs` before start, so no S3 call is needed. That is why DEC-056 left compose and CI
without an S3 service ("the posix backend needs the bucket directory created first").
With RustFS the bucket must be created through the API in every environment (section 3.4),
which is a small, uniform init step in all three places.

### 4.5 On-disk format

versitygw posix stores objects as plain files under `.dev/s3/data/<bucket>/<key>`, handy to
inspect. RustFS uses its own MinIO-compatible erasure/xl layout (see the repo's
`docs/architecture/minio-file-format-compat.md`); objects are not browsable as plain files.
For tests this does not matter; `.dev/s3` must be wiped on the switch.

### 4.6 Resource footprint (uncertain, must be measured at the gate)

- Nix closure 648 MiB unpacked (versitygw is a single Go binary, tens of MB). Disk on the
  20 GB host (DEC-055) is tight but this fits.
- RAM: docs say 2 GB minimum for a test environment. No idle-memory figure is published.
  The host has 4 GB and also runs Postgres, Meilisearch and cargo. If RustFS idles above
  a few hundred MB, keep it out of the default `cargo xtask services up` path (start it only
  when `cargo xtask ci` or a flag asks for it) or fall back to section 6.
- CPU: scanner knobs in 3.3; set `RUSTFS_SCANNER_SPEED=slowest` for the test instance.

## 5. How versitygw is wired now and exactly what changes

### 5.1 Current wiring (all 2026-11-06, git HEAD 71399ec)

| Place | Today |
|---|---|
| `flake.nix` line 34 | `pkgs.versitygw` with comment "S3-compatible test server (Apache-2.0), see DEC-008 and DEC-056" |
| `src/rust/xtask/src/services.rs` lines 109-186 | constants `S3_PORT = 55433`, `S3_ACCESS_KEY = "test"`, `S3_SECRET_KEY = "testsecret"`, `S3_BUCKET = "assets"`; `s3_up()` returns `Ok(true)` if the port is open, `Ok(false)` with a message if `versitygw --version` fails, else creates `.dev/s3/data/assets` and `.dev/s3/iam`, spawns `versitygw --port 127.0.0.1:55433 --iam-dir <iam> posix <data>` with `ROOT_ACCESS_KEY`/`ROOT_SECRET_KEY`, logs to `.dev/s3/s3.log`, writes `.dev/s3/pid`, polls the port 5 s; `s3_down()` kills the pid |
| `src/rust/xtask/src/main.rs` lines 16, 30, 92-110 | usage text "start or stop local Postgres and versitygw (S3)"; `services up` calls `s3_up`; `ci` sets `TEST_S3_ENDPOINT` to `s3_url()` when `s3_up()` succeeded (empty string otherwise) |
| `src/rust/crates/assets/tests/s3.rs` lines 1-2, 15 | comments and skip message name versitygw |
| `src/docker/compose.yml` | no S3 service at all (app uses the fs backend) |
| `.gitlab-ci.yml` test job (lines 76-92) | services postgres and meilisearch only; no `TEST_S3_*`; S3 test skips with a message |
| `docs/DECISIONS.md` | DEC-008 (MinIO gone, pick an Apache/MIT S3 server in M04), DEC-056 (versitygw 1.9.0 chosen; "Not in compose or GitLab CI yet"; alternatives "SeaweedFS, RustFS"), DEC-064 (no docker on the build host; "locally Postgres and versitygw run as processes") |
| `docs/REPORT.adoc` lines 69, 112 | mention versitygw and the missing compose/CI S3 service |
| `docs/HANDOFF.md` 4.5, 8 | the owner's request; environment note "S3 55433" |

### 5.2 Changes for the switch

1. **flake.nix**: `pkgs.versitygw` -> `pkgs.rustfs` (1.0.4 from the locked nixpkgs, binary
   cache). Comment updated to the new DEC. `pkgs.rustfs-cli` only if `rc` is preferred over
   curl for bucket creation (not recommended: one more 100 MB-class package).
2. **xtask `services.rs`** (`s3_up`): detect `rustfs` on PATH (`rustfs --help`, since
   `--version` is not documented); spawn `rustfs` with env
   `RUSTFS_VOLUMES=<.dev/s3/data>`, `RUSTFS_ADDRESS=127.0.0.1:55433`,
   `RUSTFS_ACCESS_KEY=test`, `RUSTFS_SECRET_KEY=testsecret`,
   `RUSTFS_CONSOLE_ENABLE=false`, `RUSTFS_REGION=us-east-1`,
   `RUSTFS_SCANNER_SPEED=slowest`, no `RUSTFS_OBS_LOG_DIRECTORY` (stdout/stderr -> `s3.log`
   as today); wait for `GET /health` = 200 (or port open) with a longer budget than 5 s
   (first start formats the volume); then create the bucket with
   `curl --aws-sigv4 "aws:amz:us-east-1:s3" --user test:testsecret -X PUT http://127.0.0.1:55433/assets`
   accepting 200 and 409. Keep "not installed -> tests skip" behaviour. Remove the
   `--iam-dir` and `posix` arguments and the `create_dir_all(data/assets)`. `s3_down` is
   unchanged (SIGTERM via pid file). Wipe `.dev/s3` once (format change). Section comment
   `S3 (versitygw)` -> `S3 (RustFS)`.
3. **xtask `main.rs`**: usage text; the `ci` wiring is unchanged.
4. **`assets/tests/s3.rs`**: comments and skip message.
5. **`src/docker/compose.yml`**: new service under a profile `s3` (the app keeps the fs
   backend by default; D-8 allows both):
   `s3: image: rustfs/rustfs:1.0.4`, env `RUSTFS_ACCESS_KEY`/`RUSTFS_SECRET_KEY` from
   `${S3_ACCESS_KEY:-...}`/`${S3_SECRET_KEY:-...}`, `RUSTFS_CONSOLE_ENABLE=false`,
   `RUSTFS_REGION=us-east-1`, volume `s3-data:/data` (named volume, owned by 10001 inside
   the image, no chown helper needed), healthcheck `curl -fsS http://localhost:9000/health`,
   port not published by default. Bucket: either an `s3-init` one-shot service on the same
   image running `rc alias set` + `rc bucket create --ignore-existing` with
   `depends_on: s3: condition: service_healthy`, or the entrypoint bootstrap with a
   bind-mounted `src/docker/rustfs/buckets.json` (`["assets"]`) and
   `RUSTFS_BOOTSTRAP_ENDPOINT=http://127.0.0.1:9000`. The one-shot service is the
   documented-command route; prefer it. App env for the profile:
   `APP__STORAGE__BACKEND=s3`, `APP__STORAGE__S3__ENDPOINT=http://s3:9000`,
   `APP__STORAGE__S3__BUCKET=assets`, `APP__STORAGE__S3__REGION=us-east-1`,
   `APP__STORAGE__S3__ACCESS_KEY_ID`, `APP__STORAGE__S3__SECRET_ACCESS_KEY` (already in
   `config/.env.example`). Add `S3_ACCESS_KEY`/`S3_SECRET_KEY` names to `.env.example`.
   Compose is only exercised by the GitLab kaniko/compose check (DEC-064), so this is
   validated in CI, not on the build host.
6. **`.gitlab-ci.yml` test job**: add service
   `- name: rustfs/rustfs:1.0.4` `alias: s3` `variables: RUSTFS_ACCESS_KEY: test,
   RUSTFS_SECRET_KEY: testsecret, RUSTFS_REGION: us-east-1` (leave the console enabled so
   both EXPOSEd ports 9000 and 9001 open; the runner's service readiness check waits on
   exposed ports). Job variables `TEST_S3_ENDPOINT: http://s3:9000`, `TEST_S3_BUCKET:
   assets`, `TEST_S3_ACCESS_KEY: test`, `TEST_S3_SECRET_KEY: testsecret`. In `script`,
   before `cargo test`, create the bucket with the same curl SigV4 PUT against
   `http://s3:9000/assets` (curl is installed in `.rust` `before_script`). The image's
   `/data` VOLUME is an anonymous writable volume in the service container; no mount
   needed. This turns the S3 test from "skipped" to "runs" in CI.
7. **Docs**: DECISIONS new entry (draft below), DEC-056/DEC-064 left as history;
   `docs/REPORT.adoc` lines 69 and 112; `docs/HANDOFF.md` section 8 ("S3 55433" stays,
   tool name changes); nothing in `docs/modules` names versitygw.
8. **Memory file / host notes**: measure RSS of the idle RustFS process and record it with
   the cargo-jobs note if it is significant.

### 5.3 What does not change

Ports (55432/55433/55434), the keys `test`/`testsecret` (pending section 7 item 1), the
bucket name `assets`, `TEST_S3_*` variables, the `assets` crate, `S3Config`, opendal
version, the "skip when not installed" contract, `cargo xtask ci` as the single entry point.

## 6. Fallback: "compose and CI only"

If the gate shows that RustFS is too heavy for the 4 GB host next to cargo, Postgres and
Meilisearch: keep versitygw for `cargo xtask services` (process on the host) and use
RustFS only as the compose profile and the CI service. Both speak the same S3 subset we
use, and `TEST_S3_*` makes the test indifferent to which one answers. Cost: two tools to
keep in step; the DEC should then say so explicitly.

## 7. Open uncertainties (to settle at the verification gate)

1. **Access key format.** The quick start says "Use an uppercase alphanumeric access key";
   whether `test` is accepted is not documented. If rejected, change the xtask/CI/compose
   constants together (e.g. `TESTKEY1`); the test reads them from env, so no code change.
2. **Not executed here.** The sandbox refused to run the downloaded release binary, so the
   start sequence, `/health` timing, the exact 409 on a repeated `PUT /bucket`, the
   `RUSTFS_UNSAFE_BYPASS_DISK_CHECK` semantics for a plain directory volume, and idle RSS
   are unverified. First real step: `nix develop` with `pkgs.rustfs`, `cargo xtask services
   up`, `cargo test -p assets --test s3` with `TEST_S3_ENDPOINT=http://127.0.0.1:55433`.
3. **Filesystem.** Docs recommend XFS and advise against ext4/Btrfs/ZFS for production
   data disks. The build host's filesystem is whatever NixOS uses (likely ext4); for a test
   instance this is a performance note, not a correctness issue, but it was not tested.
4. **GitLab service readiness.** The runner waits for the image's exposed ports; with the
   console disabled, 9001 never opens and the wait may time out. Hence "console enabled in
   CI" above; confirm on the first pipeline.
5. **`crates/license` in the RustFS workspace.** No licence key or entitlement is mentioned
   in the user docs; re-check on upgrades that the server does not grow a gated mode.
6. **Docs pages are undated.** All docs.rustfs.com pages show only a 2025 copyright; the
   repo `docs/` files are versioned with the code. Statements above are as read on
   2026-11-06 for release 1.0.4.

## 8. Draft DECISIONS entry (not applied)

```
DEC-191 | DEC-008 reconsidered (owner, 2026-11-06): RustFS instead of versitygw as the S3-compatible dev/test service? | Switch to RustFS 1.0.4 (Apache-2.0: LICENSE, Cargo.toml, console and rc repos, Docker Hub image, nixpkgs asl20) in all three places: `pkgs.rustfs` from the locked nixpkgs (binary cache, 648 MiB closure) started by `cargo xtask services up` on 127.0.0.1:55433 (env RUSTFS_VOLUMES/.dev/s3/data, RUSTFS_ADDRESS, RUSTFS_ACCESS_KEY=test, RUSTFS_SECRET_KEY=testsecret, RUSTFS_CONSOLE_ENABLE=false, RUSTFS_REGION=us-east-1, RUSTFS_SCANNER_SPEED=slowest; bucket `assets` created with `curl --aws-sigv4` PUT, 200 or 409 accepted); compose profile `s3` with image rustfs/rustfs:1.0.4 and an `rc bucket create --ignore-existing` init service; GitLab test job service rustfs/rustfs:1.0.4 (alias s3, console left on so both exposed ports open) with TEST_S3_* set, so the S3 test runs in CI. Our only S3 calls (PutObject, GetObject, DeleteObjects with Content-MD5 via opendal 0.59.5) are in RustFS's supported matrix. Gate before merging: local `cargo test -p assets --test s3` green and idle RSS acceptable on the 4 GB host; if not, keep versitygw for xtask and use RustFS in compose and CI only | keep versitygw 1.9.0 (lighter, plain files on disk); SeaweedFS; MinIO old tag (DEC-008) | restore `pkgs.versitygw` and the `versitygw ... posix` spawn in xtask services.rs (git history at 71399ec), drop the compose profile and CI service; TEST_S3_* keeps the test backend-agnostic | docs/research/rustfs.md; github.com/rustfs/rustfs LICENSE and api.github.com/repos/rustfs/rustfs (Apache-2.0, release 1.0.4 of 2026-10-25); docs.rustfs.com/en/reference/cli and /reference/environment-variables; rustfs docs/architecture/s3-compatibility-matrix.md; nixpkgs 1d060086 rustfs 1.0.4; opendal-service-s3-0.59.5 core.rs (Content-MD5 on DeleteObjects); all read 2026-11-06
```
