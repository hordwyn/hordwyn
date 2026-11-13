COMMON RULES FOR THIS WORK PACKAGE (repo <repo>, project "hordwyn"):
- Read docs/HANDOFF.md sections 2 (KICKOFF rules, verbatim, binding) and 4 (owner rulings, they win) first.
- Key rules: no hallucination (cite sources, docs.rs etc.); never block, pick a default and record it in
  docs/DECISIONS.md (next free number, format `DEC-NNN | question | choice | alternatives | how to reverse | source`;
  check the last number right before writing, other agents append too); ASCII only in code/docs/commits/file names
  (except non-English .ftl files); TEST-DRIVEN: write the failing test, run it, see it fail, then implement;
  no GPL/AGPL/LGPL code ever; icons only from the vendored free libraries, never hand-drawn SVG paths;
  white-label: the product name "hordwyn" (and the former codename "kithkeep") must not appear in code outside
  config/brand/ and docs/;
  no AI attribution anywhere; clippy pedantic as errors; #![deny(unsafe_code)]; thiserror in libs.
- Pedantic gotchas: no assert!(x.is_empty()) (use assert_eq!(x.len(), 0)), functions <= 100 lines.
- Build host: 2 vCPU, 4 GB RAM, little disk. Other agents work in the SAME checkout at the same time on other
  files; touch only the files of your package. Run only targeted builds/tests, always with
  `CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0`
  (MEMORY: since 2026-11-07 the VM has 16 GB RAM and 4 vCPU; one rustc on the web/server crates reaches 2.9 GB, so keep at most 3 jobs; at 4 GB two in parallel made the OOM killer take down the whole session three times.
  Never run two cargo commands at once; never run cargo while Playwright and a dev server run unless needed) (cargo workspace at src/rust). Never `cargo clean`, never run
  `cargo xtask ci` or `cargo xtask e2e` (the orchestrator does). If cargo waits on a file lock, that is
  another agent building: just wait. `rtk` rewrites commands; use `rtk proxy <cmd>` for raw output.
  Dev tools come from the nix dev shell (`nix develop` at the repo root, or tools already on PATH).
- DO NOT COMMIT, do not create branches, do not push, do not use git stash/reset/checkout on others' files.
- Final report (short): files changed, tests added and their result, DEC entries added, anything left undone
  or uncertain (say so plainly; never claim something passed that you did not run).
