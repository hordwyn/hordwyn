TBIN progress log (one server integration test binary)

Brief: docs/handoff/fable-briefs/TBIN-one-test-binary.md. Nothing is committed. DEC-565
(written last; check the number is still free).

Baseline (old layout, 2026-11-11, tree = main 9614675)
- No test executables were left in target/debug/deps, so the 25 per-file binaries were built
  and run in 5 batches of 5 (`--test a --test b ...`, executables deleted after each batch;
  free disk went from 8.7 GB to 5.3 GB during the run).
- Result: 25 binaries, 100 passed, 0 failed, 2 ignored (load, oidc live provider).
  The brief's "106 passed" includes the unit tests of the server lib/bin targets.
- Size: 25 integration executables = 8.36 GB (15 at 522-524 MB, 10 at 100-128 MB: the ones
  that never build the page router), plus the `server` executable 549 MB (stays).
- Afterwards stale duplicate rlib/rmeta/.d files were removed with the HANDOFF section 8
  command (no cargo running): 5.5 GB -> 9.0 GB free.

Done
- [x] `apps/server/tests/*.rs` moved to `apps/server/tests/it/` (plain mv, not staged),
  `support/` to `tests/it/support/`, new `tests/it/main.rs` with one `mod` per file;
  `mod support;` -> `use crate::support;` in the 20 files that had it.
  Cargo.toml: `autotests = false`, `[[test]] name = "it" path = "tests/it/main.rs"`.
- [x] xtask load: `cargo test -p server --test it -- --ignored --nocapture load::`; new test
  `xtask/tests/load.rs::the_cargo_run_selects_the_load_module_of_the_one_test_binary`
  (seen red, then green, 3 passed).
- [x] comments: tests/it/load.rs, tests/it/support/mod.rs, crates/web/src/offline.rs.

Process-global review (reading, confirmed by the runs below)
- tests: no `set_var`/`remove_var`/`set_current_dir`, no statics, every listener binds port 0,
  temp dirs carry a UUID, every test gets its own database (`store_pg::testing::fresh_db_url`).
- server lib: `page_routes()` OnceLock (DEC-384) is per process and safe (every page render
  needs a router, which is built after the listing); `api::bulk` UndoKeys static is a
  process-random key, harmless; `telemetry::init` is only called from `main.rs`;
  `opendal::install_default` is idempotent.
- `latency::pages_never_wait_on_providers` asserts < 200 ms for one request; it used to run
  alone in its process and now competes with other test threads. Passed all 11 runs, also
  with 12 test threads. Not changed; fallback if it ever flakes is in DEC-565.
- Result: nothing relied on one process per file; no code fix, no serial test.

Verify (2026-11-11, service env of the brief, CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0)
- Run 1 `cargo test -p server --features gallery,all-connectors --no-fail-fast`: lib unit
  tests 6 passed, bin 0, `it` 100 passed / 0 failed / 2 ignored (21.5 s) = 106 passed, 2 ignored.
- Run 2, same command: 6 + 100 passed, 0 failed, 2 ignored (24.1 s).
- Six more runs of the `it` executable directly: 100 passed / 2 ignored each (19.7-23.4 s).
  Three runs with `--test-threads 12`: 100 passed each.
- A first attempt to run the executable directly without `RUST_MIN_STACK=8388608` (cargo sets
  it, DEC-188) overflowed the stack in the first page render, 6 times: 6 core dumps of
  104-134 MB (0.7 GB) now sit in /var/lib/systemd/coredump, root-owned, I cannot delete them.
- Size after: `it` 579 159 880 bytes (552 MB) instead of 25 executables with 8.36 GB.
  Beside it, unchanged: `server` bin 549 MB, lib unit tests 33 MB, bin unit tests 22 MB.
- `cargo fmt --all --check`: clean. `cargo clippy -p server -p xtask --all-targets --features
  gallery,all-connectors -- -D warnings -W clippy::pedantic`: clean.
- `cargo test -p xtask --test load`: 3 passed.
- `cargo test -p server --test it -- --ignored --list load::` lists exactly
  `load::search_and_library_with_many_items`; `--test it -- --list item_page:: library_page::`
  lists the 8 tests of the two modules. The load test itself was not run.
- Cleanup: test executables deleted after each run; all `test_*` databases dropped (452 had
  piled up, about 4 GB); free disk at the end 8.8 GB.

Docs
- [x] docs/handoff/fable-briefs/PKG-implement.md (new `--test it -- item_page:: ...` form)
- [x] docs/HANDOFF.md section 8 (new bullet "Server integration tests")
- [x] docs/REPORT.adoc, docs/modules/ROOT/pages/{provider-pipeline,white-labelling}.adoc (paths)
- [x] DEC-565
- `.gitlab-ci.yml` names no test target (`cargo test --workspace`): unchanged.
- Not changed on purpose (history): DEC-150/399, docs/reviews/*, phase3/phase4 plans, old
  progress logs, which still say `apps/server/tests/<file>.rs` or `--test load`.

Other crates (brief item 5): none with more than 5 integration test files links the server
(`app` 19, `store-pg` 10, `auth` 6); nothing built or measured there.

Noticed, not done
- `cargo test -p server` without features prints 1 warning for the server lib (seen while
  building for the `--list` check; not looked into, not from this change).
- HANDOFF section 0 still says "In flight: TBIN" (orchestrator's line).
- The moves were done with plain `mv`, so git shows 26 deletions and an untracked
  `apps/server/tests/it/`; `git add -A` records them as renames.
