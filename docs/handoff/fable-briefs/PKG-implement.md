PKG - Implement one package from docs/fable/round2/next-packages.md (reusable brief)

The orchestrator names the package (e.g. ot2a). Implement exactly its section in
`docs/fable/round2/next-packages.md` (scope, files, tests first, migrations), with the design
detail from `docs/fable/round2/ux-owner-test.md` and the owner's rulings in
`docs/fable/round2/owner-answers-2026-11-10.md` / HANDOFF 4.7 (they win over the plan).

Rules: `docs/handoff/agent-briefs/rules.md` (test-driven: failing test seen red first; DEC
entries at the next free number, check right before writing; ASCII; Fluent en + de for every
string; WCAG 2.2 AA; OD-01 density; icons only from the vendored set; no "hordwyn" (or the
former "kithkeep") in code;
no attribution; do not commit). Build limits: `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0`, one
cargo/Playwright command at a time, targeted tests only, check `free -g` (wait below 6 GB
available) and `df -h /` (stop and report below 5 GB free; delete only executables in
`src/rust/target/debug/deps` and your own test databases). Never `cargo xtask ci`/`e2e`.
After SQL changes run `cargo xtask sqlx-prepare`.
Any change to rendered text, labels or accessible names: also run the server SSR page tests
(`cargo test -p server --features gallery,all-connectors --test it -- item_page::
library_page:: home_page:: auth_pages:: admin_pages::` with the service env vars; the server
integration tests are modules of one test binary `it`, DEC-565) and grep
`apps/server/tests/it` for the old strings; they are not covered by Playwright or `-p web`.

Keep a progress log in `docs/fable/round2/<package>-progress.md` (item, state, tests and
results, files, DECs) so a cut-off run can be resumed. Out of scope items you notice: list
them, do not build them. Final report: per scope item done / not done, tests added and seen
red, test runs and results (only what you ran), DECs, files changed, owner-visible choices.

Greenfield until v1 (owner, 2026-11-12): nothing is live anywhere. Do not write migration
paths, compatibility code or upgrade notes for earlier builds or data; change migrations freely
if that is simpler (they are squashed into one v1 baseline in package v1-release). Only a fresh
install has to work. This rule ends with the `v1` tag; from then on every package adds a
CHANGELOG entry and release-specific upgrade notes.
