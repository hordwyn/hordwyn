TBIN - Merge the server integration tests into one test binary (infrastructure)

Why: `cargo xtask ci` builds ~200 test binaries, 10.8 GB; each file in
`src/rust/apps/server/tests/` (25 files) links the whole server (~550 MB each). The build host
(57 GB disk) can no longer hold one full test build. One binary saves ~7-8 GB and link time.

Do:
1. Turn `apps/server/tests/*.rs` into modules of ONE integration test target, e.g.
   `apps/server/tests/it/main.rs` with `mod admin; mod admin_fns; ...` and the existing
   `support/` as a module; set `autotests = false` plus one `[[test]] name = "it"` in
   `apps/server/Cargo.toml` (check how `support` is included today and keep it working).
   Keep test names unchanged (they become `module::name`).
2. Find and fix anything that relied on one process per file: process-global state
   (`std::env::set_var`, statics/OnceLock/lazy, global tracing subscriber, fixed ports, files
   in a shared temp dir, a shared database name). Tests run in parallel threads of one
   process now. No `serial` hacks unless a DEC explains why a test must be serial.
3. `load.rs` and `latency.rs`: keep `#[ignore]` behaviour; check `cargo xtask load` still
   finds its test (grep xtask for the test target / file name and update it).
4. Update every place that names a server test target: `xtask`, `.gitlab-ci.yml`,
   `docs/HANDOFF.md` section 8 notes, agent briefs (`docs/handoff/agent-briefs/*.md`,
   `docs/handoff/fable-briefs/PKG-implement.md` uses `--test item_page` etc. -> the new form,
   e.g. `--test it -- item_page::`). Grep for `--test ` across the repo.
5. Also check other crates for the same pattern only if a crate has more than 5 integration
   test files that each link the server; report sizes, do not change them.
6. DEC entry (next free number, check right before writing).

Verify: `cargo test -p server --features gallery,all-connectors --no-fail-fast` with the
service env (REQUIRE_SERVICES=1 TEST_DATABASE_URL=postgres://app@127.0.0.1:55432/postgres
TEST_MEILI_KEY=dev-only-master-key-0123456789 TEST_MEILI_URL=http://127.0.0.1:55434
TEST_S3_ENDPOINT=http://127.0.0.1:55433): same count as before (106 passed, 2 ignored as of
ot2a) and 0 failed; run it twice to catch new parallelism flakes; clippy pedantic on
`-p server --all-targets --features gallery,all-connectors`; `cargo fmt --all --check`.
Report the size of `target/debug/deps` test executables before/after.

Rules: `docs/handoff/agent-briefs/rules.md`; build limits in PKG-implement.md. DISK: check
`df -h /` before each cargo run; delete `src/rust/target/debug/deps` executables after each
test run; the old per-file binaries will not be rebuilt once merged. Do not commit. Progress
log: `docs/fable/round2/tbin-progress.md`.
