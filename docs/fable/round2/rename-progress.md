# rename: progress (lane/rename, worktree kithkeep-lane-rename)

Task: default brand renamed from the codename to hordwyn (owner, 2026-11-12). Not committed.

| # | Item | State |
|---|---|---|
| 1 | Brand file: `name`, `short_name` = "Hordwyn" | done. No description or URL field is set in the file (`support_url` stays unset, no domain yet); no logo files (the wordmark is the name in the brand font); no `[strings.*]` overrides exist |
| 2 | Guard: `codename()` = new name, `former_codename()` = old name, `offenders()` looks for both | done, see test runs |
| 3 | Current docs: README, antora.yml and playbook title, ROOT index and white-labelling page, HANDOFF (note in 2, line in 4.8, open item in 6), ux/design.md and addendum-1.md, agent-briefs/rules.md, fable-briefs/PKG-implement.md | done |
| 4 | History kept; KICKOFF.md and DESIGN.md get a note at the top; decisions in rename-decisions.md (L-R-1..4) | done |
| 5 | Left alone: GitLab path, directory names, image/crate/database names, `KK_` variable prefix | - |
| 6 | Final grep for the old name | see below |

Checked and unchanged: `config/.env.example`, `src/docker/compose.yml`, `.gitlab-ci.yml` and
`src/web/tests` do not name the product (no hit for the old name).

## Test runs

All in `nix develop`, `CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0`, from `src/rust`.

- Red: `cargo test -p xtask --lib whitelabel` with the new test
  `detects_the_current_and_the_former_name` and the old guard: 5 passed, 1 failed (only
  `src/old.rs` found, the new name not).
- `cargo fmt --all --check`: clean.
- `cargo test -p xtask`: lib 18, architecture 1, e2e_log 6, load 3, locales 5, ops 19,
  whitelabel 2 (the guard against this tree, both names) - all passed, 0 failed.
- `cargo test -p brand`: lib 7, contrast 18, fonts 5 - all passed.
- After the last comment edit: `cargo fmt --all --check` clean, `cargo test -p xtask --lib
  --test whitelabel` 18 + 2 passed.
- Not run: server page tests, Playwright (no test names the product; they read the brand
  file). The orchestrator's `cargo xtask ci` / `e2e` after the merge is the first run of
  those with the capitalised name.

## Remaining hits of the old name

- `docs/research/*`, `docs/reviews/*`: records, not rewritten.
- `docs/design/KICKOFF.md`, `docs/design/DESIGN.md`: input documents, note at the top.
- `docs/HANDOFF.md`: line 17 (dated status line of v1-ops), section 2 (verbatim KICKOFF
  text: rule 9, Q8; note above it), section 6 and section 8 (GitLab path, twice), section 8
  (systemd unit name in a kernel log line), the new notes themselves.
- `docs/handoff/fable-briefs/LANES.md`, `common.md`, `F9-record-owner-answers.md`,
  `agent-briefs/rules.md` line 1: checkout and scratchpad directory paths.
- `agent-briefs/rules.md` line 8, `fable-briefs/PKG-implement.md`: the rule that names the
  former codename as forbidden in code.
- `docs/fable/round2/rename-decisions.md`, this file.
- `.git` (worktree pointer file with the directory path).
- Code: none. The guard assembles both names from halves.

## Noticed, not changed

- The provider key variables carry the prefix `KK_` (`KK_TMDB_API_KEY`, ...), the initials of
  the former codename, in code, `config/.env.example` and the docs. The guard does not
  match it. Owner question (HANDOFF section 6).
