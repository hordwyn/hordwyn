LANES - parallel implementation (since 2026-11-12, disk 200 GB, RAM 16 GB)

Worktrees (git worktree, one branch each, never commit, never push - the orchestrator merges):
- Lane A: <repo> (main checkout): sec1, then sec2. CARGO_BUILD_JOBS=2.
- Lane B: <repo>-lane-books, branch lane/v1-books: v1-books, then
  v1-wikidata. CARGO_BUILD_JOBS=1. Playwright/dev server ports 3200-3299. Database names
  prefixed `lb_`, Meilisearch index prefixes `lb_`.
- Lane C: <repo>-lane-polish, branch lane/v1-polish: v1-polish, then
  v1-manual. CARGO_BUILD_JOBS=1. Ports 3300-3399. Database and index prefix `lc_`.

Rules on top of PKG-implement.md:
- Work only inside your lane's worktree; never touch another worktree.
- RAM: before every cargo or Playwright command run `free -g`; if "available" is below 6 GB,
  wait (sleep 60 and check again, up to 30 min) - other lanes are building. Never more than one
  cargo command at a time in your lane. `CARGO_INCREMENTAL=0`.
- Disk: check `df -h /`; stop and report below 20 GB free.
- Shared local services (Postgres 55432, Meilisearch 55434, RustFS 55433): do not run
  `cargo xtask services down`; drop only databases and indexes with your lane's prefix.
- Never run `cargo xtask ci` or `cargo xtask e2e` (orchestrator only, main checkout).
- DEC numbers: write new entries into your lane's own file
  `docs/fable/round2/<package>-decisions.md` numbered `L<B|C>-1, L<B|C>-2, ...`; the
  orchestrator renumbers them into docs/DECISIONS.md when merging. Do not edit docs/DECISIONS.md.
- `cargo xtask sqlx-prepare` and migrations: allowed; use the next migration number after the
  highest on main plus your lane offset (lane B: +100 range start 0100, lane C: 0200) so lanes
  never collide; the orchestrator renumbers at merge (greenfield rule: migrations are squashed
  before v1 anyway).
- Before you finish: the targeted tests, the server page tests and the WHOLE Playwright
  `chromium` project on your lane's final build (your ports).
