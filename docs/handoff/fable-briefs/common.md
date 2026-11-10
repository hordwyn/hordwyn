COMMON RULES FOR FABLE ROUND 2 (repo <repo>). Model: `fable`
(resolves to claude-fable-5-2, DEC-495). Prepend this file to every F-brief.

- READ-ONLY on the code. You write exactly one output file, named in your brief, under
  `docs/fable/round2/`. Do not edit any other file, do not commit, branch, stash or push.
- NO BUILDS: never run cargo, npm, Playwright or the dev server. The VM is shared with another
  agent that uses most of the RAM. Read files with Read/Grep/git only.
- Read first: `docs/HANDOFF.md` sections 0, 2 (KICKOFF rules, binding) and 4 (owner rulings,
  they win), then `docs/handoff/owner-test-2026-11-10.md`.
- Round-1 reviews are on branch `wip/fable-reviews`, not on main. Read them with
  `git show wip/fable-reviews:docs/fable/<name>.md` (security-audit, hub-design,
  architecture-review, open-data-sources, decisions-current-state, perf-proposals,
  ops-readiness, test-audit). Do not check out that branch.
- Do not hallucinate: cite `file:line` for every code claim and URL + date for every web
  claim. If you cannot verify something, say so.
- No GPL/AGPL/LGPL code, schemas or UI text (rule 6); public API docs and terms of GPL
  services are allowed (R-P1). Check a project's licence before reading its code.
- ASCII only (no dashes other than `-`, no smart quotes, no arrows; write `->`).
- No AI attribution and no model names in the output file.
- Design constraints that win: WCAG 2.2 AA, density budget OD-01 (5 inputs / 7 rows), no pace
  wording on goals (R-04), icons only from the vendored libraries (Lucide first, rule 11),
  every string in Fluent en/de.
- Final report (short): the output path, the top 5 points, anything you could not verify.
